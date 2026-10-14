//! DanCard — Raster theo khung transform (nút Raster của panel Công cụ bình), và phần vẽ ảnh dùng
//! chung với Object › Rasterize.
//!
//! Ảnh được vẽ dọc theo khung của object: card đặt nghiêng thì ảnh nghiêng theo, không dư góc
//! trong suốt; clip group chỉ lấy phần nhìn thấy (giao của nội dung với mask), không lấy phần bị
//! mask che. Tài liệu CMYK: ảnh phủ kín lưu thành TIFF CMYK, giữ nguyên mực (chữ đen K 100 vẫn chỉ
//! có mực K), không qua RGB.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeId, NodeKind, RasterEffectsSettings, Scaling};
use vectorcraft_geom::{Affine, Rect};
use vectorcraft_render::RenderOptions;
use vectorcraft_render::encode::{RasterExportOptions, RasterFormat};

use super::{frame_of, seen_bounds};
use crate::cmd::fileio;
use crate::cmd::menucmds::{MAX_PIXELS, isolated_doc, unique_key};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

/// Độ phân giải mặc định của nút Raster (như DanCard trên Illustrator).
const DANCARD_PPI: f64 = 500.0;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.rasterize",
        "DanCard: Raster theo khung transform",
        [],
        None,
        "{ids?: [id…] (mặc định: vùng chọn), ppi?: 1–2400 (mặc định 500), background?: transparent|white (mặc định transparent), antiAlias?: none|art|type (mặc định art), followAngle?: bool (mặc định true: card đặt nghiêng thì ảnh nghiêng theo khung transform)} mỗi object đang chọn thành một ảnh đúng vùng nhìn thấy: clip group lấy phần trong mask (chọn object con trong clip group thì raster cả clip group), không thêm clipping mask, không thêm lề. Tài liệu phải ở CMYK; ảnh phủ kín lưu TIFF CMYK giữ đúng mực, ảnh có chỗ trong suốt lưu PNG có kênh trong suốt. Một bước hoàn tác → {message, ids, images: [{id, width, height, ppi, cmyk}], errors}",
        has_selection,
        rasterize
    )]
}

/// Ảnh raster đã vẽ xong, chưa đưa vào tài liệu.
pub(crate) struct Raster {
    pub blob: ImageBlob,
    pub width: u32,
    pub height: u32,
    /// Pixel của ảnh → trang.
    pub xf: Affine,
    /// Lưu TIFF CMYK (mực thật) chứ không phải PNG.
    pub cmyk: bool,
}

/// Cách làm tròn vùng raster ra số pixel nguyên.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Fit {
    /// Làm tròn lên, giữ góc trên trái và đúng `ppi` (Object › Rasterize).
    Grow,
    /// Ảnh phủ đúng vùng: số pixel làm tròn gần nhất, `ppi` lệch đi một chút cho vừa (nút Raster
    /// DanCard: ảnh đúng khung transform, mép không bị nửa pixel trong suốt).
    Exact,
}

/// Vẽ `nodes` (theo thứ tự vẽ) của `doc` ra ảnh ở `ppi`, phủ vùng `region` đo trong hệ trục
/// `to_local` (trang → hệ trục của khung; `Affine::IDENTITY` cho khung thẳng), làm tròn theo
/// `fit`. `finish`: hệ màu (xám, bitmap). `inks`: lấy mực CMYK của tài liệu; ảnh không có pixel
/// trong suốt thì lưu TIFF CMYK, còn lại PNG.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_raster(
    doc: &Document,
    nodes: Vec<Node>,
    region: Rect,
    to_local: Affine,
    ppi: f64,
    fit: Fit,
    opts: &RenderOptions,
    finish: &RasterEffectsSettings,
    inks: bool,
) -> Result<Raster> {
    let k = ppi / 72.0;
    let (fw, fh) = (region.width() * k, region.height() * k);
    if !(fw.is_finite() && fh.is_finite() && fw >= 0.0 && fh >= 0.0) {
        return Err(EngineError::Other("Không đo được vùng cần raster.".into()));
    }
    if fw * fh > MAX_PIXELS || fw > 65535.0 || fh > 65535.0 {
        return Err(EngineError::Other("Ảnh quá lớn: hãy hạ độ phân giải (ppi).".into()));
    }
    let (w, h, kx, ky) = match fit {
        Fit::Grow => (fw.ceil().max(1.0), fh.ceil().max(1.0), k, k),
        Fit::Exact => {
            let (w, h) = (fw.round().max(1.0), fh.round().max(1.0));
            (w, h, w / region.width().max(1e-9), h / region.height().max(1e-9))
        }
    };
    let (w, h) = (w as u32, h as u32);
    let view = Affine::scale_non_uniform(kx, ky) * Affine::translate((-region.x0, -region.y0)) * to_local;
    let xf = to_local.inverse() * Affine::translate((region.x0, region.y0)) * Affine::scale_non_uniform(1.0 / kx, 1.0 / ky);
    let tmp = isolated_doc(doc, nodes);
    let mut r = vectorcraft_render::Renderer::new();
    let mut img = r.render(&tmp, w, h, view, opts);
    finish.finish_pixels(&mut img.pixels);
    let opaque = covered(&img.pixels, w as usize, h as usize);
    if inks && opaque {
        let data = r.render_inks(&tmp, w, h, view, opts);
        if let Some(blob) = vectorcraft_doc::cmyk::Inks::new(w, h, data).and_then(|i| ImageBlob::cmyk_tiff(&i)) {
            return Ok(Raster { blob, width: w, height: h, xf, cmyk: true });
        }
    }
    let png = img.to_png().map_err(EngineError::Other)?;
    Ok(Raster { blob: ImageBlob::new("image/png", png), width: w, height: h, xf, cmyk: false })
}

/// Ảnh phủ kín: mọi pixel bên trong đục hẳn; pixel ở viền ngoài cùng (mép vùng rơi giữa pixel,
/// khử răng cưa) chỉ cần được phủ quá nửa.
fn covered(pixels: &[u8], w: usize, h: usize) -> bool {
    pixels.as_chunks::<4>().0.iter().enumerate().all(|(i, p)| {
        let (x, y) = (i % w.max(1), i / w.max(1));
        let rim = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
        p[3] == 255 || (rim && p[3] >= 128)
    })
}

/// Ảnh `r` thành node mới của `d` (ảnh nhúng), cùng tên và góc khung với object nó thay.
pub(crate) fn image_node(d: &mut Document, r: Raster, name: Option<String>, angle: f64) -> Node {
    let key = unique_key(d, "raster");
    d.images.insert(key.clone(), r.blob);
    let id = d.alloc_id();
    let mut node = Node::new(
        id,
        NodeKind::Image(Box::new(ImageObject { key, width: r.width, height: r.height, xf: r.xf, link: None, placement: Default::default() })),
    );
    node.name = name;
    node.bbox_angle = angle;
    node
}

/// Thứ được raster cho một object đang chọn: clip group gần nhất chứa nó (chọn ảnh con trong clip
/// group thì raster cả group để giữ mask, như DanCard), không có thì chính nó.
fn raster_target(d: &Document, id: NodeId) -> NodeId {
    let mut cur = Some(id);
    while let Some(c) = cur {
        match d.node(c).map(|n| &n.kind) {
            Some(NodeKind::Group { clip: true, .. }) => return c,
            Some(NodeKind::Layer { .. }) | None => break,
            _ => cur = d.parent_of(c),
        }
    }
    id
}

/// Các object cần raster: mỗi clip group một lần, bỏ object nằm trong một object khác cũng được raster.
fn raster_targets(d: &Document, ids: &[NodeId]) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = vec![];
    for id in ids.iter().map(|id| raster_target(d, *id)) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    let inside = |id: NodeId, others: &[NodeId]| {
        let mut up = d.parent_of(id);
        while let Some(p) = up {
            if others.contains(&p) {
                return true;
            }
            up = d.parent_of(p);
        }
        false
    };
    let all = out.clone();
    out.retain(|id| !inside(*id, &all));
    out
}

/// Vùng nhìn thấy của `n` trong hệ trục `to_local` của khung (giao clip với nội dung, viền tính cả).
fn local_region(n: &Node, to_local: Affine) -> Option<Rect> {
    let mut probe = n.clone();
    probe.visible = true;
    if to_local != Affine::IDENTITY {
        probe.transform(to_local, Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
    seen_bounds(&probe).filter(|r| r.width() > 0.0 && r.height() > 0.0)
}

fn rasterize(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.rasterize";
    let st = s.doc()?;
    let doc = &st.doc;
    if doc.color_mode != vectorcraft_doc::ColorMode::Cmyk {
        return Err(bad(C, "Tài liệu đang ở RGB. Hãy đổi File › Document Color Mode › CMYK Color rồi bấm Raster."));
    }
    let picked: Vec<NodeId> = match p.get("ids").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_u64).map(NodeId).collect(),
        None => st.selection.objects.to_vec(),
    };
    if picked.is_empty() {
        return Err(bad(C, "Hãy chọn ít nhất 1 object để raster."));
    }
    let ppi = f64_or(p, "ppi", DANCARD_PPI);
    if !(1.0..=2400.0).contains(&ppi) {
        return Err(bad(C, "ppi phải từ 1 đến 2400."));
    }
    let background = match p.get("background") {
        Some(v) if !v.is_null() => fileio::background(v).map_err(|e| bad(C, e))?,
        _ => None,
    };
    let anti_alias = match str_param(p, "antiAlias") {
        Some(a) => fileio::anti_alias(a).map_err(|e| bad(C, e))?,
        None => vectorcraft_render::AntiAlias::Art,
    };
    let follow = bool_or(p, "followAngle", true);
    let raster = RasterExportOptions { ppi, background, anti_alias, ..Default::default() };
    let opts = raster.render_options(RasterFormat::Png);
    let finish = RasterEffectsSettings::default();

    let mut made = vec![];
    let mut errors = vec![];
    for (i, id) in raster_targets(doc, &picked).into_iter().enumerate() {
        let Some(n) = doc.node(id) else { continue };
        // Khung như bảng Transform đo (clip group: mask), kể cả góc dò được của card đặt nghiêng.
        let frame = frame_of(doc, &[id], false).filter(|_| follow);
        let angle = frame.map_or(0.0, |f| f.angle);
        let to_local = match frame {
            Some(f) if f.angle != 0.0 => Affine::rotate_about(f.angle.to_radians(), f.center),
            _ => Affine::IDENTITY,
        };
        let Some(region) = local_region(n, to_local) else {
            errors.push(format!("Object {}: không có phần nhìn thấy để raster.", i + 1));
            continue;
        };
        let mut node = n.clone();
        node.visible = true;
        match render_raster(doc, vec![node], region, to_local, ppi, Fit::Exact, &opts, &finish, true) {
            Ok(r) => made.push((id, r, n.name.clone(), angle)),
            Err(e) => errors.push(format!("Object {}: {e}", i + 1)),
        }
    }
    if made.is_empty() {
        return Err(EngineError::Other(format!("Không raster được object nào.\n{}", errors.join("\n"))));
    }
    let total = made.len() + errors.len();
    let cmyk = made.iter().filter(|m| m.1.cmyk).count();
    let images = s.edit("Raster (DanCard)", |d, sel| {
        let mut out = vec![];
        for (target, r, name, angle) in made {
            let (w, h, is_cmyk) = (r.width, r.height, r.cmyk);
            let node = image_node(d, r, name, angle);
            let id = d.wrap(&[target], |_| node)?;
            out.push((id, w, h, is_cmyk));
        }
        sel.set(out.iter().map(|o| o.0));
        Ok(out)
    })?;
    let count = images.len();
    let mut message = format!("Đã raster {count} object — {ppi} ppi, nền {}.", if background.is_some() { "trắng" } else { "trong suốt" });
    if cmyk == count {
        message.push_str(" Ảnh lưu CMYK, giữ đúng mực.");
    } else {
        message.push_str(&format!(" {} ảnh có chỗ trong suốt nên lưu PNG (màu chuyển sang CMYK khi xuất file in).", count - cmyk));
    }
    if !errors.is_empty() {
        message = format!("Raster được {count}/{total} object.\n{}\n{message}", errors.join("\n"));
    }
    Ok(json!({
        "message": message,
        "ids": images.iter().map(|o| o.0.0).collect::<Vec<_>>(),
        "images": images.iter().map(|(id, w, h, c)| json!({"id": id.0, "width": w, "height": h, "ppi": ppi, "cmyk": c})).collect::<Vec<_>>(),
        "errors": errors,
    }))
}
