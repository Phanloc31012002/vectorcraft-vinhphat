//! DanCard — Dàn theo mẫu (tab Dàn theo mẫu): học một bố cục đã dàn tay rồi áp cho các con nguồn,
//! lưu PDF đã dàn và copy ghi chú, cùng quy tắc `dcHocMau`, `dcApMauCore`,
//! `dcLuuDanTheoMauPDF_SelectedLegacy` và `dcCopyToiUuNoteToArtboards` của DanCard.
//!
//! Mẫu đã học không ghi ra file tạm như DanCard: `dancard.learnTemplate` trả mẫu dạng JSON, panel
//! giữ nó và đưa lại cho `dancard.applyTemplate`. Mỗi object cấp đầu được chọn là một vị trí, đo
//! theo khung transform của nó (DanCard chỉ có khung thẳng của trang): độ lệch tâm so với tâm cả
//! bố cục, khung rộng × cao theo trục của khung, góc nghiêng của khung (`tilt`) và hướng artwork
//! (bội số 90°) so với con chuẩn, là con mẫu cùng chiều với các con nguồn. Hướng artwork đọc trên
//! bản đã dựng thẳng: từ ma trận ảnh và chữ trong card khi mọi vị trí đều có; còn lại so ảnh
//! 128 × 128 của từng vị trí với con chuẩn xoay 0/90/180/270°, như DanCard so ảnh preview (chỉ nét
//! đậm được tính).
//!
//! Áp mẫu: mỗi nguồn được dựng thẳng theo khung transform và giữ nguyên vector (DanCard raster
//! 500 ppi), xoay theo hướng artwork, co giãn đúng khung từng vị trí (không giữ tỉ lệ), nghiêng theo
//! khung vị trí; mỗi lô một artboard mới đúng khổ file pon, artwork của pon chép lên trên.
//! Mẫu 2 mặt: mặt sau là mặt trước đối xứng ngang, nguồn cột trái là mặt trước, cột phải là mặt sau.
//! Các tờ mới xếp từ góc trên trái canvas (± 7200 pt quanh gốc toạ độ), cách nhau 10 mm.
//!
//! Toạ độ trong mẫu là của VectorCraft (y hướng xuống): `dy > 0` là dưới tâm bố cục.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Artboard, Document, Node, NodeId, NodeKind, Scaling, TextKind};
use vectorcraft_geom::{Affine, PathData, Point, Rect, Vec2};
use vectorcraft_render::RenderOptions;

use super::{roots_in_paint_order, upright_affine, upright_copy};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

const MM: f64 = 72.0 / 25.4;
/// Khoảng cách giữa các tờ mới, giữa mặt trước và mặt sau, và từ mép canvas (10 mm).
const GAP: f64 = 10.0 * MM;
/// Nửa cạnh canvas của DanCard (pt), quanh gốc toạ độ.
const HALF: f64 = 7200.0;
const EPS: f64 = 0.01;
/// Sai lệch kích thước chấp nhận sau khi co giãn (pt).
const SIZE_TOL: f64 = 0.75;
/// Cạnh ảnh dùng để so hướng (pixel).
const N: usize = 128;
/// Layer chứa kết quả Áp mẫu (như DanCard).
pub(super) const LAYER: &str = "__DAN_THEO_MAU_KET_QUA__";
const FORMAT: &str = "danTheoMau";
const VERSION: u64 = 1;
const MAX_LEARN: usize = 2000;
const MAX_SLOTS: usize = 10_000;
const MAX_SOURCES: usize = 5000;
const MAX_SHEETS: usize = 2000;
const MAX_CARDS: usize = 50_000;
const MAX_ATTEMPTS: usize = 5000;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "dancard.learnTemplate",
            "DanCard: Học mẫu",
            [],
            None,
            "{mode: one|mirror (one: Học 1 mặt; mirror: Học 2 mặt - Mặt SAU đối xứng), ids?: [id…] (các con mẫu theo thứ tự vị trí; mặc định: các object đang chọn từ trên cùng xuống), reference?: id (con chuẩn: con mẫu cùng chiều với con nguồn; có thể là object nằm đúng chỗ một con mẫu)} học bố cục đã dàn tay: mỗi object là một vị trí (lệch tâm so với tâm bố cục, khung rộng × cao, góc 0/90/180/270° so với con chuẩn, đọc từ ma trận ảnh/chữ hoặc so ảnh). Không có reference: trả mẫu đang chờ (pending) và ids để chọn con chuẩn rồi gọi lại với ids + reference → {message, pending, template, ids, reference, angles, angleSource: geometry|render}",
            has_doc,
            learn_template
        ),
        cmd!(
            "dancard.applyTemplate",
            "DanCard: Áp mẫu",
            [],
            None,
            "{template: mẫu dancard.learnTemplate trả về, multiSourcePerArtboard?: bool (Dàn nhiều mẫu nguồn khác nhau vào 1 artboard: các nguồn lần lượt vào các vị trí, thiếu thì bù bằng con cuối rồi gần cuối; mặc định false: mỗi nguồn một artboard), ponFront: file pon (.ai/.pdf/.svg/.vectorcraft; artboard đầu là khổ tờ, artwork chép lên mỗi tờ), ponBack?: file pon MẶT SAU (mẫu 2 mặt)} dàn các con nguồn đang chọn theo mẫu (2 mặt: cột trái = mặt trước, cột phải = mặt sau): mỗi nguồn dựng thẳng theo khung transform (giữ vector), xoay, co giãn đúng khung và nghiêng theo từng vị trí, mỗi lô một artboard mới trên layer __DAN_THEO_MAU_KET_QUA__; dàn không lỗi thì xoá các artboard cũ; một bước hoàn tác → {message, sheets, firstArtboard, ids, notes}",
            has_selection,
            apply_template
        ),
        cmd!(
            query "dancard.saveTemplatePdf",
            "DanCard: Lưu PDF đã dàn",
            [],
            None,
            "{mode?: each|pair (each: Từng artboard - 1 PDF / artboard; pair: Mặt trước + mặt sau - 1 PDF / 2 artboard), suffix?: nội dung sau tên file 1 / file 2 …, folder: thư mục lưu (phải có sẵn)} lưu các artboard thành PDF 1.4 tên \"file 1 - <nội dung>.pdf\" theo thứ tự artboard; mỗi trang chép các object không khóa đang hiện chạm artboard đó; không ghi đè file có sẵn, không sửa tài liệu → {message, files, saved, folder}",
            has_doc,
            save_template_pdf
        ),
        cmd!(
            "dancard.copyNotes",
            "DanCard: Copy ghi chú",
            [],
            None,
            "{prefixes?: \"F1, F2…\" (nội dung thêm trước ghi chú theo thứ tự tờ; trống: copy nguyên ghi chú), twoSided?: bool (true: bài 2 mặt → artboard 1, 3, 5…; false: bài 1 mặt → 1, 2, 3…)} chép ghi chú đang chọn trên một artboard sang các artboard sau, cùng khoảng cách tới góc trên trái artboard; một bước hoàn tác → {message, ids, artboard}",
            has_selection,
            copy_notes
        ),
    ]
}

// ---------------------------------------------------------------- đo

fn positive(r: &Rect) -> bool {
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) && r.width() > 0.0 && r.height() > 0.0
}

/// Khung một con (`bnd` của DanCard): clip group theo mask, còn lại theo vùng nhìn thấy.
fn bnd(n: &Node) -> Option<Rect> {
    if let NodeKind::Group { children, clip: true } = &n.kind
        && let Some(m) = children.first().and_then(|m| m.geometric_bounds()).filter(positive)
    {
        return Some(m);
    }
    super::layout::vis(n).filter(positive)
}

/// Path kín (mỗi đoạn kín, ít nhất 3 điểm neo).
fn closed_shape(path: &PathData) -> bool {
    !path.subpaths.is_empty() && path.subpaths.iter().all(|sp| sp.closed && sp.anchors.len() >= 3)
}

/// Khung một vị trí khi học: path / compound path kín theo khung hình học (không tính viền),
/// còn lại như [`bnd`].
fn slot_frame(n: &Node) -> Option<Rect> {
    let contour = match &n.kind {
        NodeKind::Path { path, .. } => closed_shape(path),
        NodeKind::Compound { children, .. } => {
            !children.is_empty() && children.iter().all(|c| matches!(&c.kind, NodeKind::Path { path, .. } if closed_shape(path)))
        }
        _ => false,
    };
    if contour && let Some(b) = n.geometric_bounds().filter(positive) {
        return Some(b);
    }
    bnd(n)
}

fn kind_of(n: &Node) -> &'static str {
    match &n.kind {
        NodeKind::Group { clip: true, .. } => "clip",
        NodeKind::Group { .. } => "group",
        NodeKind::Compound { .. } => "compound",
        NodeKind::Path { .. } => "path",
        NodeKind::Text(_) => "text",
        NodeKind::Image(_) => "image",
        _ => "object",
    }
}

/// Các object gốc đang chọn, trên cùng trước (thứ tự vùng chọn của Illustrator).
fn top_first(s: &Session) -> Result<Vec<NodeId>> {
    let mut ids = roots_in_paint_order(s)?;
    ids.reverse();
    Ok(ids)
}

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

// ---------------------------------------------------------------- mẫu

/// Một vị trí của mẫu: độ lệch tâm so với tâm bố cục và khung (pt, y hướng xuống; rộng × cao theo
/// trục của khung), góc (độ, ngược chiều kim đồng hồ, bội số 90°) xoay con nguồn cùng chiều con
/// chuẩn thành hướng artwork của vị trí (-1: chưa biết), rồi góc nghiêng `tilt` của khung vị trí.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Slot {
    pub dx: f64,
    pub dy: f64,
    pub w: f64,
    pub h: f64,
    pub angle: f64,
    pub tilt: f64,
}

impl Slot {
    /// `frame`: khung đã dựng thẳng (tâm là tâm khung transform), `tilt`: góc của khung transform.
    fn of(frame: Rect, centre: Point, angle: f64, tilt: f64) -> Self {
        let c = frame.center();
        Slot { dx: c.x - centre.x, dy: c.y - centre.y, w: frame.width(), h: frame.height(), angle, tilt }
    }

    /// Vị trí mặt sau, đối xứng ngang qua trục dọc giữa bố cục: 0°/180° giữ nguyên, 90°/270° đổi
    /// chiều, khung nghiêng ngược lại.
    pub(super) fn mirrored(self) -> Self {
        let angle = if self.angle < 0.0 { self.angle } else { (-self.angle).rem_euclid(360.0) };
        Slot { dx: -self.dx, angle, tilt: -self.tilt, ..self }
    }
}

/// Khung thẳng `r` nghiêng `tilt` độ quanh tâm: hộp thẳng ôm nó trên trang.
fn footprint(r: Rect, tilt: f64) -> Rect {
    if tilt == 0.0 || !tilt.is_finite() {
        return r;
    }
    let m = Affine::rotate_about(-tilt.to_radians(), r.center());
    let pts = [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)].map(|p| m * p);
    pts.iter().skip(1).fold(Rect::from_points(pts[0], pts[0]), |acc, p| acc.union_pt(*p))
}

/// Mẫu đọc từ JSON của `dancard.learnTemplate`.
struct Template {
    two: bool,
    back_mirror: bool,
    slots: Vec<Slot>,
}

fn template_json(two: bool, state: &str, reference: Option<usize>, slots: &[Slot], kinds: &[&str]) -> Value {
    let list: Vec<Value> = slots
        .iter()
        .zip(kinds)
        .map(|(s, k)| json!({"dx": s.dx, "dy": s.dy, "w": s.w, "h": s.h, "angle": s.angle, "tilt": s.tilt, "kind": k}))
        .collect();
    json!({
        "format": FORMAT,
        "version": VERSION,
        "state": state,
        "mode": if two { "two" } else { "one" },
        "backMirror": two,
        "reference": reference,
        "slots": list,
    })
}

fn parse_template(v: Option<&Value>) -> Result<Template> {
    let t = match v {
        None | Some(Value::Null) => return Err(other("Chưa có dữ liệu mẫu. Học mẫu trước.")),
        Some(t) => t,
    };
    if t.get("format").and_then(Value::as_str) != Some(FORMAT) || t.get("version").and_then(Value::as_u64) != Some(VERSION) {
        return Err(other("Dữ liệu mẫu cũ. Học mẫu lại để lưu biên dạng và hướng artwork."));
    }
    if t.get("state").and_then(Value::as_str) != Some("ready") {
        return Err(other("Học mẫu chưa hoàn tất nhận diện hướng artwork. Hãy chọn con chuẩn rồi bấm Xác nhận con chuẩn, hoặc bấm Học mẫu lại."));
    }
    let two = match t.get("mode").and_then(Value::as_str) {
        Some("one") => false,
        Some("two") => true,
        _ => return Err(other("Bố cục mẫu rỗng. Học mẫu lại.")),
    };
    let raw = t.get("slots").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    if raw.is_empty() {
        return Err(other("Bố cục mẫu rỗng. Học mẫu lại."));
    }
    if raw.len() > MAX_SLOTS {
        return Err(other(format!("Mẫu có quá nhiều vị trí ({}; tối đa {MAX_SLOTS}).", raw.len())));
    }
    let num = |s: &Value, k: &str| s.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let slots = raw
        .iter()
        .map(|s| Slot {
            dx: num(s, "dx"),
            dy: num(s, "dy"),
            w: num(s, "w"),
            h: num(s, "h"),
            angle: s.get("angle").and_then(Value::as_f64).unwrap_or(-1.0),
            // Mẫu học trước khi có góc nghiêng: khung thẳng.
            tilt: s.get("tilt").map_or(Some(0.0), Value::as_f64).unwrap_or(f64::NAN),
        })
        .collect();
    Ok(Template { two, back_mirror: t.get("backMirror").and_then(Value::as_bool).unwrap_or(false), slots })
}

/// `validateLearnedBlock`: mỗi vị trí cần độ lệch, góc hữu hạn và khung dương.
fn validate(slots: &[Slot], label: &str) -> Result<()> {
    for (i, s) in slots.iter().enumerate() {
        let finite = [s.dx, s.dy, s.angle, s.tilt, s.w, s.h].iter().all(|v| v.is_finite());
        if !(finite && s.w > 0.0 && s.h > 0.0) {
            return Err(other(format!("Mẫu {label} có dữ liệu không hợp lệ ở vị trí {}. Hãy Học mẫu lại.", i + 1)));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- dancard.learnTemplate

/// Một con mẫu đã đo: bản dựng thẳng theo khung transform (`node`) và khung vị trí của bản đó
/// (`seen`, để so ảnh với `node`), khung đó đặt về đúng tâm phần nhìn thấy trên trang (`frame`), góc
/// nghiêng của khung transform.
struct Item {
    id: NodeId,
    node: Node,
    seen: Rect,
    frame: Rect,
    tilt: f64,
    kind: &'static str,
}

fn learn_template(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.learnTemplate";
    let two = match str_param(p, "mode").unwrap_or("") {
        "one" => false,
        "mirror" => true,
        m => return Err(bad(C, format!("mode phải là one (Học 1 mặt) hoặc mirror (Học 2 mặt - Mặt SAU đối xứng), không phải '{m}'."))),
    };
    let ids = if p.get("ids").is_some() { checked_ids_param(s, p, "ids", C)? } else { top_first(s)? };
    if ids.is_empty() {
        return Err(other("Chưa chọn bản mẫu. Hãy chọn TẤT CẢ bản mẫu đã dàn tay."));
    }
    if ids.len() > MAX_LEARN {
        return Err(other(format!("Chọn quá nhiều bản mẫu ({}; tối đa {MAX_LEARN}).", ids.len())));
    }
    let reference = match p.get("reference") {
        None | Some(Value::Null) => None,
        Some(v) => Some(checked_id(s, v, C)?),
    };
    let st = s.doc()?;
    let d = &st.doc;
    // Object không đo được thì bỏ qua (như DanCard); thứ tự vị trí là thứ tự chọn.
    let items: Vec<Item> = ids
        .iter()
        .filter_map(|id| {
            let kind = kind_of(d.node(*id)?);
            let node = upright_copy(d, *id)?;
            let seen = slot_frame(&node)?;
            // Dựng thẳng xoay quanh tâm khung transform; khung vị trí đo trên bản đã dựng thẳng có thể
            // lệch tâm đó (con ẩn, path không tô). Đưa tâm khung vị trí về đúng chỗ của nó trên trang
            // để lúc áp mẫu, xoay nghiêng quanh tâm khung vị trí đặt đúng chỗ bản gốc.
            let (up, tilt) = upright_affine(d, *id).map_or((Affine::IDENTITY, 0.0), |(m, t, _)| (m, t));
            let c = seen.center();
            let frame = seen + ((up.inverse() * c) - c);
            Some(Item { id: *id, node, seen, frame, tilt, kind })
        })
        .collect();
    let Some(union) = super::layout::union_of(items.iter().map(|it| footprint(it.frame, it.tilt))) else {
        return Err(other("Không đo được bản mẫu nào."));
    };
    let centre = union.center();
    let measured: Vec<u64> = items.iter().map(|it| it.id.0).collect();
    let kinds: Vec<&str> = items.iter().map(|it| it.kind).collect();
    let Some(r) = reference else {
        let slots: Vec<Slot> = items.iter().map(|it| Slot::of(it.frame, centre, -1.0, it.tilt)).collect();
        return Ok(json!({
            "message": "Chọn 1 con mẫu cùng chiều nguồn → Xác nhận con chuẩn.",
            "pending": true,
            "template": template_json(two, "pending", None, &slots, &kinds),
            "ids": measured,
        }));
    };
    let ref_index = reference_index(d, r, &items)?;
    let (angles, source) = infer_angles(d, &items, ref_index)?;
    let slots: Vec<Slot> = items.iter().zip(&angles).map(|(it, a)| Slot::of(it.frame, centre, *a, it.tilt)).collect();
    let n = slots.len();
    let message = if two {
        format!(
            "OK: Đã HỌC 2 MẶT từ MẶT TRƯỚC ({n} vị trí). Mặt SAU đã tự đối xứng ngang; slot 0°/180° giữ nguyên, 90°/270° đổi chiều. Giờ chọn nguồn mặt trước rồi mặt sau, sau đó bấm Áp mẫu."
        )
    } else {
        let from = if source == "render" { "từ ảnh mẫu" } else { "từ ma trận ảnh/chữ" };
        format!("OK: Đã HỌC 1 mặt ({n} vị trí) và nhận diện hướng artwork {from}. Chọn CON NGUỒN rồi bấm Áp mẫu.")
    };
    Ok(json!({
        "message": message,
        "pending": false,
        "template": template_json(two, "ready", Some(ref_index), &slots, &kinds),
        "ids": measured,
        "reference": ref_index,
        "angles": angles,
        "angleSource": source,
    }))
}

/// Vị trí của con chuẩn `r` (`dcHocMauXacNhanConChuan`): chính một con mẫu, hoặc object có tâm
/// gần tâm một con mẫu nhất, cách không quá 35 % cạnh dài của nó + 1 pt.
fn reference_index(d: &Document, r: NodeId, items: &[Item]) -> Result<usize> {
    if let Some(i) = items.iter().position(|it| it.id == r) {
        return Ok(i);
    }
    let n = d.node(r).ok_or(EngineError::NoNode(r))?;
    let c = n
        .geometric_bounds()
        .filter(positive)
        .or_else(|| n.visual_bounds().filter(positive))
        .ok_or_else(|| other("Không đo được con mẫu đang chọn."))?
        .center();
    let mut best: Option<(usize, f64, Rect)> = None;
    for (i, it) in items.iter().enumerate() {
        let dist = it.frame.center().distance(c);
        if best.is_none_or(|(_, b, _)| dist < b) {
            best = Some((i, dist, it.frame));
        }
    }
    let (i, dist, f) = best.ok_or_else(|| other("Mẫu không có dữ liệu con chuẩn. Hãy Học mẫu lại."))?;
    if dist > f.width().max(f.height()) * 0.35 + 1.0 {
        return Err(other("Con đang chọn không nằm trong bố cục vừa học. Hãy chọn lại 1 con mẫu."));
    }
    Ok(i)
}

/// Hướng artwork của từng vị trí so với con chuẩn (0, 90, 180, 270), đọc trên các bản đã dựng
/// thẳng: từ ma trận ảnh/chữ khi mọi vị trí đều đọc được ("geometry"), không thì so ảnh ("render").
fn infer_angles(d: &Document, items: &[Item], reference: usize) -> Result<(Vec<f64>, &'static str)> {
    let nodes: Vec<&Node> = items.iter().map(|it| &it.node).collect();
    let quarters: Option<Vec<i64>> = nodes.iter().copied().map(content_quarter).collect();
    if let Some(q) = quarters
        && let Some(q0) = q.get(reference).copied()
    {
        return Ok((q.iter().map(|qi| ((qi - q0).rem_euclid(4) * 90) as f64).collect(), "geometry"));
    }
    let mut renderer = vectorcraft_render::Renderer::new();
    // Ảnh của bản dựng thẳng trong khung đo trên chính bản đó (khung đã đặt về trang có thể lệch).
    let signals: Vec<Vec<f32>> = nodes.iter().zip(items).map(|(n, it)| signal(&mut renderer, d, n, it.seen)).collect();
    let base = signals.get(reference).ok_or_else(|| other("Con mẫu chuẩn không khớp dữ liệu preview tổng."))?;
    let turned: Vec<Vec<f32>> = (0..4).map(|k| rotate_signal(base, k)).collect();
    let angles = signals
        .iter()
        .map(|sig| {
            // k phần tư vòng theo chiều kim đồng hồ trên ảnh = (4 − k) × 90° ngược chiều.
            let mut best = (0usize, f64::INFINITY);
            for (k, t) in turned.iter().enumerate() {
                let dist = signal_distance(sig, t);
                if dist < best.1 {
                    best = (k, dist);
                }
            }
            ((4 - best.0) % 4 * 90) as f64
        })
        .collect();
    Ok((angles, "render"))
}

/// Hướng artwork của `n` theo phần tư vòng (0–3, ngược chiều kim đồng hồ), bầu từ ma trận của các
/// ảnh và chữ bên trong (nặng theo căn diện tích); None khi không có hoặc không rõ.
fn content_quarter(n: &Node) -> Option<i64> {
    let mut bins = [0.0f64; 4];
    vote(n, &mut bins);
    let total: f64 = bins.iter().sum();
    if !(total.is_finite() && total > 0.0) {
        return None;
    }
    let (best, weight) = bins.iter().copied().enumerate().fold((0usize, f64::MIN), |acc, (i, w)| if w > acc.1 { (i, w) } else { acc });
    (weight >= 0.48 * total).then_some(best as i64)
}

fn vote(n: &Node, bins: &mut [f64; 4]) {
    if !n.visible {
        return;
    }
    match &n.kind {
        // Mask của clip group không phải artwork.
        NodeKind::Group { children, clip } | NodeKind::Layer { children, clip, .. } => {
            for c in children.iter().skip(usize::from(*clip)) {
                vote(c, bins);
            }
        }
        NodeKind::Image(im) => add_vote(im.xf, n, bins),
        NodeKind::Text(t) if !matches!(t.kind, TextKind::OnPath { .. }) => add_vote(t.xf, n, bins),
        _ => {}
    }
}

fn add_vote(xf: Affine, n: &Node, bins: &mut [f64; 4]) {
    let [a, b, c, d, _, _] = xf.as_coeffs();
    // Lật gương hay suy biến: không cho biết chiều xoay.
    if a * d - b * c <= 0.0 {
        return;
    }
    // Trục x của ảnh/chữ trên trang (y hướng xuống) → góc ngược chiều kim đồng hồ.
    let deg = (-b).atan2(a).to_degrees();
    let Some(r) = n.geometric_bounds() else { return };
    let weight = (r.width() * r.height()).sqrt();
    if !(deg.is_finite() && weight.is_finite() && weight > 0.0) {
        return;
    }
    let q = ((deg / 90.0).round() as i64).rem_euclid(4);
    if let Some(bin) = usize::try_from(q).ok().and_then(|q| bins.get_mut(q)) {
        *bin += weight;
    }
}

/// Ảnh N × N của `n` (chỉ nó, nền trong suốt) kéo giãn đúng khung `frame`: mỗi pixel là độ đậm
/// của nét (`alpha × max(0, (190 − L) / 190)`), nền trắng và màu nhạt bằng 0.
fn signal(r: &mut vectorcraft_render::Renderer, d: &Document, n: &Node, frame: Rect) -> Vec<f32> {
    let mut out = vec![0.0f32; N * N];
    if !positive(&frame) {
        return out;
    }
    let mut shown = n.clone();
    shown.visible = true;
    let tmp = crate::cmd::menucmds::isolated_doc(d, vec![shown]);
    let k = N as f64;
    let view = Affine::scale_non_uniform(k / frame.width(), k / frame.height()) * Affine::translate((-frame.x0, -frame.y0));
    let opts = RenderOptions { skip_templates: true, ..RenderOptions::default() };
    let img = r.render(&tmp, N as u32, N as u32, view, &opts);
    let px = img.to_straight();
    for (o, p) in out.iter_mut().zip(px.chunks_exact(4)) {
        let &[red, green, blue, alpha] = p else { continue };
        let l = 0.2126 * f32::from(red) + 0.7152 * f32::from(green) + 0.0722 * f32::from(blue);
        *o = f32::from(alpha) / 255.0 * ((190.0 - l) / 190.0).max(0.0);
    }
    out
}

/// `rotateSignal`: `turns` phần tư vòng theo chiều kim đồng hồ trên ảnh.
fn rotate_signal(src: &[f32], turns: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; N * N];
    let last = N - 1;
    for y in 0..N {
        for x in 0..N {
            let (sy, sx) = match turns % 4 {
                1 => (last - x, y),
                2 => (last - y, last - x),
                3 => (x, last - y),
                _ => (y, x),
            };
            if let (Some(o), Some(v)) = (out.get_mut(y * N + x), src.get(sy * N + sx)) {
                *o = *v;
            }
        }
    }
    out
}

/// `signalDistance`: sai khác trung bình nhỏ nhất khi xê dịch ±2 pixel (bỏ viền 2 pixel).
fn signal_distance(a: &[f32], b: &[f32]) -> f64 {
    let mut best = f64::INFINITY;
    for oy in 0..5 {
        for ox in 0..5 {
            let (mut sum, mut count) = (0.0f64, 0u32);
            for y in 2..N - 2 {
                let (ra, rb) = (y * N, (y + oy - 2) * N);
                for x in 2..N - 2 {
                    if let (Some(p), Some(q)) = (a.get(ra + x), b.get(rb + x + ox - 2)) {
                        sum += f64::from((p - q).abs());
                        count += 1;
                    }
                }
            }
            if count > 0 {
                best = best.min(sum / f64::from(count));
            }
        }
    }
    best
}

// ---------------------------------------------------------------- xếp tờ, lô nguồn

/// Một tờ mới: mặt trước và (2 mặt) mặt sau ngay bên phải, cùng mép trên.
pub(super) type Sheet = (Rect, Option<Rect>);

/// `dcDanTheoMauOutputPositions`: chỗ của `count` tờ trên canvas, từ góc trên trái, trái → phải
/// rồi xuống hàng, cách nhau 10 mm, tránh các artboard có sẵn (trừ khung làm việc phủ gần hết
/// canvas) và vùng các con nguồn.
pub(super) fn output_positions(
    front: (f64, f64),
    back: Option<(f64, f64)>,
    count: usize,
    existing: &[Rect],
    sources: Rect,
) -> std::result::Result<Vec<Sheet>, String> {
    let too_big = "Khổ PON/cặp hai mặt vượt giới hạn canvas. Hãy giảm khổ hoặc mở Large Canvas.";
    let area = Rect::new(-HALF + GAP, -HALF + GAP, HALF - GAP, HALF - GAP);
    let (fw, fh) = front;
    let ok = |v: f64| v.is_finite() && v > 0.0;
    if !(ok(fw) && ok(fh) && back.is_none_or(|(w, h)| ok(w) && ok(h))) || count == 0 {
        return Err(too_big.into());
    }
    let bundle_w = fw + back.map_or(0.0, |(w, _)| GAP + w);
    let bundle_h = back.map_or(fh, |(_, h)| fh.max(h));
    if bundle_w > area.width() + EPS || bundle_h > area.height() + EPS {
        return Err(too_big.into());
    }
    let work_frame = |r: &Rect| r.width() >= 0.92 * (area.width() + 2.0 * GAP) && r.height() >= 0.92 * (area.height() + 2.0 * GAP);
    let mut occupied: Vec<Rect> = existing.iter().filter(|r| !work_frame(r)).copied().collect();
    let holds = |r: &Rect, s: Rect| s.x0 >= r.x0 - EPS && s.x1 <= r.x1 + EPS && s.y0 >= r.y0 - EPS && s.y1 <= r.y1 + EPS;
    if !occupied.iter().any(|r| holds(r, sources)) {
        occupied.push(sources);
    }
    // Chạm mép thì được.
    let overlaps = |a: Rect, b: &Rect| a.x0 < b.x1 - EPS && a.x1 > b.x0 + EPS && a.y0 < b.y1 - EPS && a.y1 > b.y0 + EPS;
    let (mut cx, mut cy) = (area.x0, area.y0);
    let mut out = Vec::with_capacity(count.min(MAX_SHEETS));
    for _ in 0..count {
        let (mut x, mut y) = (cx, cy);
        let mut placed = None;
        for _ in 0..MAX_ATTEMPTS {
            if x + bundle_w > area.x1 + EPS {
                x = area.x0;
                y += bundle_h + GAP;
                continue;
            }
            if y + bundle_h > area.y1 + EPS {
                return Err("Không còn chỗ trong canvas để tạo đủ tờ. Hãy bớt artboard cũ hoặc mở Large Canvas.".into());
            }
            let bundle = Rect::new(x, y, x + bundle_w, y + bundle_h);
            if let Some(b) = occupied.iter().find(|o| overlaps(bundle, o)) {
                x = b.x1 + GAP;
                continue;
            }
            let f = Rect::new(x, y, x + fw, y + fh);
            placed = Some((f, back.map(|(w, h)| Rect::new(f.x1 + GAP, y, f.x1 + GAP + w, y + h)), bundle));
            break;
        }
        let Some((f, b, bundle)) = placed else {
            return Err("Không tìm được vị trí trong canvas để đặt artboard mới.".into());
        };
        out.push((f, b));
        occupied.push(bundle);
        (cx, cy) = (x + bundle_w + GAP, y);
    }
    Ok(out)
}

/// `fillSourceBatch`: lô `seed` đủ `slots` chỗ, bù bằng con cuối, gần cuối, cuối… Trả (lô, số chỗ bù).
pub(super) fn fill_batch<T: Copy>(seed: &[T], slots: usize) -> (Vec<T>, usize) {
    let mut filled = seed.to_vec();
    let tail = seed.len().min(2);
    let mut extra = 0usize;
    while filled.len() < slots {
        let Some(v) = extra.checked_rem(tail).and_then(|k| seed.len().checked_sub(1 + k)).and_then(|i| seed.get(i)) else { break };
        filled.push(*v);
        extra += 1;
    }
    (filled, extra)
}

/// Một con nguồn đã đo, cho thứ tự đọc.
struct Rec {
    id: NodeId,
    c: Point,
    h: f64,
    original: usize,
}

/// Phép so của `orderSourceItems`: hàng trên trước (cùng hàng khi tâm lệch không quá 35 % chiều
/// cao nhỏ hơn, ít nhất 1 pt), rồi trái → phải, rồi thứ tự chọn.
fn reading_cmp(a: &Rec, b: &Rec) -> std::cmp::Ordering {
    if (a.c.y - b.c.y).abs() > (a.h.min(b.h) * 0.35).max(1.0) {
        return a.c.y.total_cmp(&b.c.y);
    }
    if (a.c.x - b.c.x).abs() > 0.01 {
        return a.c.x.total_cmp(&b.c.x);
    }
    a.original.cmp(&b.original)
}

/// Sắp xếp chèn ổn định: phép so trên không bắc cầu với hàng so le, nên không dùng `sort_by`.
fn reading_order(recs: &mut [Rec]) {
    for i in 1..recs.len() {
        let mut j = i;
        while j > 0
            && let (Some(a), Some(b)) = (recs.get(j - 1), recs.get(j))
            && reading_cmp(a, b) == std::cmp::Ordering::Greater
        {
            recs.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Một lô: các nguồn cho từng vị trí của mặt trước (và mặt sau), số chỗ bù.
struct Batch {
    front: Vec<NodeId>,
    back: Vec<NodeId>,
    extra: usize,
}

// ---------------------------------------------------------------- pon

/// File pon: khổ tờ (artboard đầu), tâm artboard đó, tài liệu để chép artwork.
struct Pon {
    size: (f64, f64),
    centre: Point,
    doc: Document,
}

/// Đọc file pon; None khi không có đường dẫn hoặc pon không có artboard hợp lệ.
fn load_pon(path: &str) -> Result<Option<Pon>> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    let doc = read_pon(path)?;
    let Some(r) = doc.artboards.first().map(|a| a.rect).filter(positive) else { return Ok(None) };
    Ok(Some(Pon { size: (r.width(), r.height()), centre: r.center(), doc }))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_pon(path: &str) -> Result<Document> {
    let bytes = crate::cmd::fileio::read_file(path).map_err(|e| other(format!("Không đọc được file pon: {e}")))?;
    let name = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("pon.ai");
    Ok(crate::cmd::fileio::load(name, &bytes).map_err(|e| other(format!("Không mở được file pon {path}: {e}")))?.doc)
}

#[cfg(target_arch = "wasm32")]
fn read_pon(path: &str) -> Result<Document> {
    Err(other(format!("Bản web không đọc được file pon {path}.")))
}

/// Artwork của pon gom thành một group (layer con thành group), chép vào `d`; None: pon trắng.
fn pon_art(d: &mut Document, pon: &Document) -> Option<Node> {
    fn keep(c: &Node) -> bool {
        !c.is_layer() || (c.visible && !c.is_template())
    }
    fn convert(n: &Node) -> Node {
        match &n.kind {
            NodeKind::Layer { children, clip, .. } => {
                let kids = children.iter().filter(|c| keep(c)).map(|c| Arc::new(convert(c))).collect();
                let mut g = Node::new(n.id, NodeKind::Group { children: kids, clip: *clip });
                g.name = n.name.clone();
                g
            }
            _ => n.clone(),
        }
    }
    let mut art: Vec<Node> = pon
        .layers
        .iter()
        .filter(|l| l.visible && !l.is_template())
        .flat_map(|l| l.children().into_iter().flatten().filter(|c| keep(c)).map(|c| convert(c)))
        .collect();
    if art.is_empty() {
        return None;
    }
    crate::cmd::place::adopt::adopt(d, pon, &mut art);
    let children: Vec<Arc<Node>> = art.iter().map(|n| Arc::new(d.reid(n))).collect();
    let mut g = Node::group(d.alloc_id(), children);
    g.name = Some("PON".into());
    Some(g)
}

// ---------------------------------------------------------------- dancard.applyTemplate

/// Một artboard cần dàn.
struct Job<'a> {
    sources: &'a [NodeId],
    slots: &'a [Slot],
    rect: Rect,
    back: bool,
    tag: usize,
}

/// Lần dàn đang chạy: layer kết quả, bản dựng thẳng theo nguồn, các bản đã đặt, lưu ý.
struct Output {
    layer: NodeId,
    cache: HashMap<NodeId, Option<Node>>,
    ids: Vec<NodeId>,
    errors: Vec<String>,
}

fn output_layer(d: &mut Document) -> Result<NodeId> {
    let found = d.layers.iter().find(|l| l.name.as_deref() == Some(LAYER)).map(|l| l.id);
    let id = match found {
        Some(id) => id,
        None => {
            let n = crate::cmd::layer::make_layer(d, Some(LAYER));
            d.insert(None, usize::MAX, n)?
        }
    };
    if let Some(l) = d.node_mut(id) {
        l.locked = false;
        l.visible = true;
    }
    Ok(id)
}

/// Dọn kết quả lần trước: mọi object cấp đầu của layer kết quả, trừ các con nguồn (và object chứa chúng).
fn clear_previous_output(d: &mut Document, layer: NodeId, keep: &[NodeId]) -> Result<()> {
    let gone: Vec<NodeId> = d
        .node(layer)
        .and_then(Node::children)
        .into_iter()
        .flatten()
        .map(|c| c.id)
        .filter(|id| !keep.contains(id) && !keep.iter().any(|k| super::layout::inside(d, *k, *id)))
        .collect();
    if !gone.is_empty() {
        d.remove_many(&gone)?;
    }
    Ok(())
}

/// Nguồn đã dựng thẳng theo khung transform, giữ nguyên vector (DanCard raster 500 ppi đúng khung).
fn prepare_source(d: &Document, src: NodeId) -> std::result::Result<Node, String> {
    let mut n = upright_copy(d, src).ok_or("Không tìm thấy object nguồn.")?;
    bnd(&n).ok_or("Không đo được biên dạng object.")?;
    n.visible = true;
    n.locked = false;
    Ok(n)
}

/// Bản dựng thẳng của nguồn `src`, làm một lần cho cả lần dàn.
fn flat_of(d: &mut Document, out: &mut Output, src: NodeId, tag: usize, no: usize) -> Option<Node> {
    if let Some(hit) = out.cache.get(&src) {
        return hit.clone();
    }
    let flat = match prepare_source(d, src) {
        Ok(n) => Some(n),
        Err(e) => {
            out.errors.push(format!("Mẫu {tag}, nguồn {no}: {e}"));
            None
        }
    };
    out.cache.insert(src, flat.clone());
    flat
}

/// Co giãn `n` quanh tâm cho khung đúng `w` × `h` (không giữ tỉ lệ).
fn resize_to(n: &mut Node, w: f64, h: f64, sc: Scaling, missing: &str) -> std::result::Result<(), String> {
    let b = bnd(n).ok_or(missing)?;
    let (sx, sy) = (w / b.width(), h / b.height());
    if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
        return Err("Không tính được tỉ lệ resize.".into());
    }
    let c = b.center().to_vec2();
    n.transform(Affine::translate(c) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c), sc);
    Ok(())
}

/// Đặt bản `n` vào vị trí `slot` của tờ có tâm `centre`: xoay theo hướng artwork, co giãn đúng
/// khung, đặt tâm, rồi nghiêng theo khung vị trí.
fn place(mut n: Node, slot: Slot, centre: Point) -> std::result::Result<Node, String> {
    let sc = Scaling { patterns: true, ..Scaling::default() };
    let rot = if slot.angle >= 0.0 {
        let r = slot.angle.rem_euclid(360.0);
        if r > 180.0 { r - 360.0 } else { r }
    } else {
        // Mẫu cũ chưa có góc: xoay 90° khi chiều ngang/dọc của bản sao khác của vị trí.
        let b = bnd(&n).ok_or("Không đo được biên dạng bản sao.")?;
        let clear = |w: f64, h: f64| w.max(h) / w.min(h).max(1e-9) > 1.04;
        if clear(b.width(), b.height()) && clear(slot.w, slot.h) && (b.width() >= b.height()) != (slot.w >= slot.h) { 90.0 } else { 0.0 }
    };
    if rot.abs() > 0.01 {
        let c = bnd(&n).ok_or("Không đo được biên dạng bản sao.")?.center().to_vec2();
        n.transform(Affine::translate(c) * Affine::rotate(-rot.to_radians()) * Affine::translate(-c), sc);
    }
    resize_to(&mut n, slot.w, slot.h, sc, "Không đo được biên dạng trước khi resize.")?;
    let mut b = bnd(&n).ok_or("Không đo được biên dạng sau khi scale.")?;
    let off = |b: &Rect| (b.width() - slot.w).abs() > SIZE_TOL || (b.height() - slot.h).abs() > SIZE_TOL;
    if off(&b) {
        resize_to(&mut n, slot.w, slot.h, sc, "Không đo được biên dạng sau khi hiệu chỉnh.")?;
        b = bnd(&n).ok_or("Không đo được biên dạng sau khi hiệu chỉnh.")?;
        if off(&b) {
            return Err("Resize không đạt kích thước slot yêu cầu.".into());
        }
    }
    let target = Point::new(centre.x + slot.dx, centre.y + slot.dy);
    n.transform(Affine::translate(target - b.center()), sc);
    if slot.tilt.is_finite() && slot.tilt.abs() > 1e-9 {
        n.transform(Affine::rotate_about(-slot.tilt.to_radians(), target), sc);
    }
    Ok(n)
}

/// `danMotCon`: dàn một artboard. Bản của vị trí 1 nằm trên vị trí 2…; artwork pon trên cùng.
fn fill_sheet(d: &mut Document, out: &mut Output, job: &Job, art: Option<&Node>, pon_centre: Point) -> Result<()> {
    let tag = job.tag;
    if job.sources.len() != job.slots.len() {
        out.errors.push(format!("Mẫu {tag}: số nguồn ({}) không khớp số slot ({}).", job.sources.len(), job.slots.len()));
        return Ok(());
    }
    let centre = job.rect.center();
    let mut made = 0;
    for (i, (src, slot)) in job.sources.iter().zip(job.slots).enumerate() {
        let no = i + 1;
        let Some(flat) = flat_of(d, out, *src, tag, no) else {
            out.errors.push(format!("Mẫu {tag}, vị trí {no}: không chuẩn bị được nguồn cho slot này."));
            continue;
        };
        let copy = d.reid(&flat);
        match place(copy, *slot, centre) {
            Ok(node) => {
                out.ids.push(d.insert(Some(out.layer), 0, node)?);
                made += 1;
            }
            Err(e) => out.errors.push(format!("Mẫu {tag}, vị trí {no}: {e}")),
        }
    }
    if made > 0
        && let Some(art) = art
    {
        let mut copy = d.reid(art);
        copy.transform(Affine::translate(centre - pon_centre), Scaling { strokes: true, patterns: true, ..Scaling::default() });
        d.insert(Some(out.layer), usize::MAX, copy)?;
    }
    Ok(())
}

fn apply_template(s: &mut Session, p: &Value) -> Result<Value> {
    let sources = top_first(s)?;
    if sources.is_empty() {
        return Err(other("Chưa chọn con nguồn. Hãy chọn các CON NGUỒN cần dàn."));
    }
    if sources.len() > MAX_SOURCES {
        return Err(other(format!("Chọn quá nhiều con nguồn ({}; tối đa {MAX_SOURCES}).", sources.len())));
    }
    let tpl = parse_template(p.get("template"))?;
    let multi = bool_or(p, "multiSourcePerArtboard", false);
    let front = tpl.slots;
    let back: Option<Vec<Slot>> = match (tpl.two, tpl.back_mirror) {
        (false, _) => None,
        (true, true) => Some(front.iter().map(|s| s.mirrored()).collect()),
        (true, false) => return Err(other("Chưa học MẶT SAU. Mẫu 2 mặt nhưng chưa học MẶT SAU: hãy Học mẫu lại.")),
    };
    validate(&front, "MẶT TRƯỚC")?;
    if let Some(b) = &back {
        validate(b, "MẶT SAU")?;
    }
    let pon_f = load_pon(str_param(p, "ponFront").unwrap_or(""))?
        .ok_or_else(|| other("Chưa chọn pon MẶT TRƯỚC có artboard hợp lệ. Chưa tạo kết quả nào."))?;
    let pon_b = if back.is_some() {
        Some(
            load_pon(str_param(p, "ponBack").unwrap_or(""))?
                .ok_or_else(|| other("Chưa chọn pon MẶT SAU có artboard hợp lệ. Chưa tạo kết quả nào."))?,
        )
    } else {
        None
    };
    let n_slots = front.len().max(1);
    let st = s.doc()?;
    let d = &st.doc;
    let mut notes: Vec<String> = vec![];
    let batches: Vec<Batch> = if back.is_some() {
        // Hai cột: trái (tâm x nhỏ hơn trung bình) = mặt trước, phải = mặt sau; cặp theo thứ hạng từ trên xuống.
        let mut cols = vec![];
        for (i, id) in sources.iter().enumerate() {
            let b = d.node(*id).and_then(bnd).ok_or_else(|| other(format!("Nguồn {} không có biên dạng hợp lệ. Chưa tạo kết quả nào.", i + 1)))?;
            cols.push((*id, b.center()));
        }
        let mid = cols.iter().map(|c| c.1.x).sum::<f64>() / cols.len().max(1) as f64;
        let (mut left, mut right): (Vec<_>, Vec<_>) = cols.into_iter().partition(|c| c.1.x < mid);
        left.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        right.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        if left.len() != right.len() {
            return Err(other(format!(
                "Mẫu 2 mặt cần hai cột bằng nhau (trái=mặt trước, phải=mặt sau). Hiện có trái {}, phải {}. Chưa tạo kết quả nào.",
                left.len(),
                right.len()
            )));
        }
        let pairs: Vec<(NodeId, NodeId)> = left.iter().zip(&right).map(|(f, b)| (f.0, b.0)).collect();
        if multi {
            pairs
                .chunks(n_slots)
                .map(|c| {
                    let (f, extra) = fill_batch(&c.iter().map(|p| p.0).collect::<Vec<_>>(), n_slots);
                    let (b, _) = fill_batch(&c.iter().map(|p| p.1).collect::<Vec<_>>(), n_slots);
                    Batch { front: f, back: b, extra }
                })
                .collect()
        } else {
            pairs.iter().map(|(f, b)| Batch { front: vec![*f; n_slots], back: vec![*b; n_slots], extra: 0 }).collect()
        }
    } else {
        let mut recs = vec![];
        for (i, id) in sources.iter().enumerate() {
            match d.node(*id).and_then(bnd) {
                Some(b) => recs.push(Rec { id: *id, c: b.center(), h: b.height(), original: i }),
                None => notes.push(format!("Nguồn {}: không đo được biên dạng.", i + 1)),
            }
        }
        if recs.is_empty() {
            return Err(other("Không có nguồn nào có biên dạng hợp lệ. Chưa tạo kết quả nào."));
        }
        reading_order(&mut recs);
        let ordered: Vec<NodeId> = recs.iter().map(|r| r.id).collect();
        if multi {
            ordered
                .chunks(n_slots)
                .map(|c| {
                    let (f, extra) = fill_batch(c, n_slots);
                    Batch { front: f, back: vec![], extra }
                })
                .collect()
        } else {
            ordered.iter().map(|id| Batch { front: vec![*id; n_slots], back: vec![], extra: 0 }).collect()
        }
    };
    let faces = if back.is_some() { 2 } else { 1 };
    if batches.len() > MAX_SHEETS || batches.len().saturating_mul(n_slots).saturating_mul(faces) > MAX_CARDS {
        return Err(other(format!(
            "Quá nhiều bản cần dàn ({} artboard × {n_slots} vị trí; tối đa {MAX_SHEETS} tờ, {MAX_CARDS} bản). Hãy chia nhỏ nguồn.",
            batches.len() * faces
        )));
    }
    let filled: usize = batches.iter().map(|b| b.extra).sum::<usize>() * faces;
    let existing: Vec<Rect> = d.artboards.iter().map(|a| a.rect).collect();
    let src_bounds = sources
        .iter()
        .map(|id| d.node(*id).and_then(bnd))
        .collect::<Option<Vec<Rect>>>()
        .and_then(|v| super::layout::union_of(v.into_iter()))
        .ok_or_else(|| other("Không đo được biên dạng bài nguồn để tránh khi dàn."))?;
    let positions = output_positions(pon_f.size, pon_b.as_ref().map(|b| b.size), batches.len(), &existing, src_bounds).map_err(EngineError::Other)?;
    let mut jobs = vec![];
    for (i, (batch, (front_rect, back_rect))) in batches.iter().zip(&positions).enumerate() {
        match (&back, back_rect) {
            (Some(back_slots), Some(back_rect)) => {
                jobs.push(Job { sources: &batch.front, slots: &front, rect: *front_rect, back: false, tag: 2 * i + 1 });
                jobs.push(Job { sources: &batch.back, slots: back_slots, rect: *back_rect, back: true, tag: 2 * i + 2 });
            }
            _ => jobs.push(Job { sources: &batch.front, slots: &front, rect: *front_rect, back: false, tag: i + 1 }),
        }
    }
    let back_centre = pon_b.as_ref().map_or(pon_f.centre, |b| b.centre);
    let (ids, errors, first_new, layer) = s.edit("Áp mẫu (DanCard)", |d, sel| {
        let layer = output_layer(d)?;
        let old = d.artboards.len();
        for job in &jobs {
            let id = d.next_artboard_id();
            d.artboards.push(Artboard { id, name: String::new(), rect: job.rect, show_center_mark: false, show_cross_hairs: false });
        }
        clear_previous_output(d, layer, &sources)?;
        let art_f = pon_art(d, &pon_f.doc);
        let art_b = pon_b.as_ref().and_then(|b| pon_art(d, &b.doc));
        let mut out = Output { layer, cache: HashMap::new(), ids: vec![], errors: notes };
        for job in &jobs {
            let (art, centre) = if job.back { (art_b.as_ref(), back_centre) } else { (art_f.as_ref(), pon_f.centre) };
            fill_sheet(d, &mut out, job, art, centre)?;
        }
        // Dàn không lỗi: xoá các artboard cũ (artwork của chúng vẫn ở trên canvas).
        let first_new = if out.errors.is_empty() {
            let removed: Vec<u32> = d.artboards.drain(..old).map(|a| a.id).collect();
            d.retain_guides(sel, |_, g| g.artboard.is_none_or(|id| !removed.contains(&id)));
            0
        } else {
            old
        };
        for (i, a) in d.artboards.iter_mut().enumerate().skip(first_new) {
            a.name = format!("Artboard {}", i + 1);
        }
        sel.set(out.ids.iter().copied());
        Ok((out.ids, out.errors, first_new, layer))
    })?;
    if let Some(st) = s.active_mut() {
        st.active_layer = Some(layer);
    }
    let (n_src, n_batches, two) = (sources.len(), batches.len(), back.is_some());
    let kind = if multi { "nhiều mẫu/1 artboard" } else { "1 mẫu/1 artboard" };
    let mut message = if two {
        format!("OK: Đã dàn 2 mặt {n_batches} cặp artboard ({kind}).")
    } else {
        format!("OK: Đã dàn 1 mặt {n_src} nguồn thành {n_batches} artboard ({kind}).")
    };
    if !errors.is_empty() {
        message.push_str(&format!(" ({} lưu ý) Chưa xóa artboard cũ vì dàn còn lỗi.", errors.len()));
    }
    let pairs = n_src / 2;
    let detail = match (two, multi) {
        (true, true) => format!(
            "Đã dàn 2 mặt {pairs} cặp nguồn thành {n_batches} cặp artboard (trái=trước, phải=sau); mỗi artboard gán các nguồn lần lượt vào slot mẫu."
        ),
        (true, false) => format!(
            "Đã dàn 2 mặt {pairs} cặp nguồn thành {n_batches} cặp artboard (trái=trước, phải=sau); mỗi artboard nhân 1 nguồn tương ứng vào toàn bộ slot mẫu."
        ),
        (false, true) => format!("Đã dàn {n_src} nguồn thành {n_batches} artboard; mỗi artboard gán các nguồn lần lượt vào {n_slots} slot mẫu."),
        (false, false) => format!("Đã dàn {n_src} nguồn thành {n_batches} artboard; mỗi artboard nhân 1 nguồn vào {n_slots} slot mẫu."),
    };
    message.push('\n');
    message.push_str(&detail);
    if filled > 0 {
        message.push_str(&format!("\nĐã bù {filled} slot còn thiếu bằng con cuối rồi gần cuối của từng lô nguồn."));
    }
    if !errors.is_empty() {
        message.push_str(&format!("\nLưu ý:\n- {}", errors.join("\n- ")));
    }
    Ok(json!({
        "message": message,
        "sheets": jobs.len(),
        "firstArtboard": first_new,
        "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(),
        "notes": errors,
    }))
}

// ---------------------------------------------------------------- dancard.saveTemplatePdf

fn pdf_name(k: usize, suffix: &str) -> String {
    if suffix.is_empty() { format!("file {k}.pdf") } else { format!("file {k} - {suffix}.pdf") }
}

/// Nội dung sau "file 1": bỏ khoảng trắng hai đầu, ký tự cấm trong tên file thành "-".
fn clean_suffix(s: &str) -> String {
    s.trim().chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect()
}

/// Các object cấp đầu (nhìn qua layer con) đang hiện, không khóa, chạm artboard `r`.
#[cfg(not(target_arch = "wasm32"))]
fn objects_on(d: &Document, r: Rect) -> Vec<Node> {
    let mut out = vec![];
    for layer in d.layers.iter().filter(|l| l.visible && !l.locked && !l.is_template()) {
        for id in layer.layer_art(true) {
            let Some(n) = d.node(id) else { continue };
            let Some(b) = n.visual_bounds().or_else(|| n.geometric_bounds()) else { continue };
            if b.x0 <= r.x1 && b.x1 >= r.x0 && b.y0 <= r.y1 && b.y1 >= r.y0 {
                out.push(n.clone());
            }
        }
    }
    out
}

/// Tài liệu tạm cho một PDF: mỗi artboard trong `pages` thành một trang cùng khổ (lớn nhất), nội
/// dung giữ khoảng cách tới góc trên trái artboard.
#[cfg(not(target_arch = "wasm32"))]
fn pdf_document(d: &Document, pages: &[usize], copy_errors: &mut Vec<String>) -> Result<Document> {
    let rects: Vec<(usize, Rect)> = pages.iter().filter_map(|i| d.artboards.get(*i).map(|a| (*i, a.rect))).collect();
    let w = rects.iter().map(|(_, r)| r.width()).fold(1.0, f64::max);
    let h = rects.iter().map(|(_, r)| r.height()).fold(1.0, f64::max);
    let mut tmp = crate::cmd::menucmds::isolated_doc(d, vec![]);
    let mut nodes: Vec<Arc<Node>> = vec![];
    let mut boards = vec![];
    let mut failed = false;
    for (k, (page, src)) in rects.iter().enumerate() {
        // Trang cách nhau đủ xa để phần tràn ra ngoài artboard không lẫn sang trang khác.
        let x = k as f64 * (2.0 * w + 100.0);
        let target = Rect::new(x, 0.0, x + w, h);
        boards.push(Artboard {
            id: k as u32 + 1,
            name: format!("Artboard {}", k + 1),
            rect: target,
            show_center_mark: false,
            show_cross_hairs: false,
        });
        let items = objects_on(d, *src);
        if items.is_empty() {
            let no = page + 1;
            copy_errors.push(format!("Artboard {no}: Lỗi: Artboard {no} không có object để lưu."));
            failed = true;
            continue;
        }
        let by = Vec2::new(target.x0 - src.x0, target.y0 - src.y0);
        for n in items {
            let mut c = tmp.reid(&n);
            c.transform(Affine::translate(by), Scaling { strokes: true, patterns: true, ..Scaling::default() });
            nodes.push(Arc::new(c));
        }
    }
    if failed {
        return Err(other("Không copy đủ object của artboard."));
    }
    if let Some(l) = tmp.layers.first_mut()
        && let Some(ch) = Arc::make_mut(l).children_mut()
    {
        ch.extend(nodes);
    }
    tmp.artboards = boards;
    Ok(tmp)
}

fn save_template_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.saveTemplatePdf";
    let st = s.doc()?;
    let d = &st.doc;
    let span = match str_param(p, "mode").filter(|m| !m.is_empty()).unwrap_or("each") {
        "each" => 1,
        "pair" => 2,
        _ => return Err(other("Kiểu lưu PDF không hợp lệ.")),
    };
    let n = d.artboards.len();
    if n == 0 {
        return Err(other("Không có artboard nào để lưu."));
    }
    if span == 2 && !n.is_multiple_of(2) {
        return Err(other("Lưu mặt trước + mặt sau cần số artboard chẵn."));
    }
    let suffix = clean_suffix(str_param(p, "suffix").unwrap_or(""));
    let folder = str_param(p, "folder").map(str::trim).unwrap_or("").to_string();
    if folder.is_empty() {
        return Err(other("Chưa chọn thư mục lưu PDF - đã huỷ."));
    }
    let jobs: Vec<(String, Vec<usize>)> =
        (0..n).step_by(span).enumerate().map(|(k, first)| (pdf_name(k + 1, &suffix), (first..(first + span).min(n)).collect())).collect();
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (C, &jobs, &folder);
        Err(other("Bản web không lưu được PDF vào thư mục."))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = std::path::Path::new(&folder);
        if !dir.is_dir() {
            return Err(other(format!("Thư mục lưu không mở được: {folder}")));
        }
        // Không ghi đè: kiểm tra mọi tên trước khi ghi file nào.
        if let Some((name, _)) = jobs.iter().find(|(name, _)| dir.join(name).exists()) {
            return Err(other(format!("File đã tồn tại: {name}.")));
        }
        let settings = json!({"compatibility": "1.4", "preserveEditing": false, "thumbnails": false, "fastWebView": true});
        let (mut copy_errors, mut save_errors, mut files) = (vec![], vec![], vec![]);
        for (name, pages) in &jobs {
            let path = dir.join(name);
            let r = pdf_document(d, pages, &mut copy_errors)
                .and_then(|tmp| crate::cmd::fileio::pdf::encode(C, &tmp, &settings))
                .and_then(|(bytes, _)| crate::cmd::fileio::write_file(&path.to_string_lossy(), &bytes));
            match r {
                Ok(()) => files.push(name.clone()),
                Err(e) => save_errors.push(format!("{name}: Lỗi: {e}")),
            }
        }
        let done = files.len();
        if done == 0 {
            let more = if save_errors.is_empty() { String::new() } else { format!(" {}", save_errors.join(" | ")) };
            return Err(other(format!("Không lưu được PDF nào.{more}")));
        }
        let mut message = format!("OK: Đã lưu {done} PDF theo thứ tự Artboard.");
        if !copy_errors.is_empty() {
            message.push_str(&format!(" Bỏ qua {} object lỗi khi copy.", copy_errors.len()));
        }
        if !save_errors.is_empty() {
            message.push_str(&format!(" Lỗi {} file: {}", save_errors.len(), save_errors.join(" | ")));
        }
        Ok(json!({"message": message, "files": files, "saved": done, "folder": folder}))
    }
}

// ---------------------------------------------------------------- dancard.copyNotes

/// Khung ghi chú: vùng nhìn thấy (viền tính cả), không có thì khung hình học.
fn note_bounds(n: &Node) -> Option<Rect> {
    n.visual_bounds().or_else(|| n.geometric_bounds()).filter(|r| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()))
}

/// Các khung chữ trong `n` (chính nó, hoặc trong group lồng nhau), theo thứ tự con.
fn text_frames(n: &Node, out: &mut Vec<NodeId>) {
    match &n.kind {
        NodeKind::Text(_) => out.push(n.id),
        NodeKind::Group { children, .. } => {
            for c in children {
                text_frames(c, out);
            }
        }
        _ => {}
    }
}

fn nth_text(d: &Document, id: NodeId, k: usize) -> Option<NodeId> {
    let mut v = vec![];
    text_frames(d.node(id)?, &mut v);
    v.get(k).copied()
}

/// Đặt góc trên trái khung của `id` tại góc trên trái `board` + `off`.
fn anchor(d: &mut Document, id: NodeId, board: Rect, off: Vec2, missing: &str) -> Result<()> {
    let b = d.node(id).and_then(note_bounds).ok_or_else(|| other(missing))?;
    super::layout::translate_node(d, id, Vec2::new(board.x0 + off.x - b.x0, board.y0 + off.y - b.y0));
    Ok(())
}

fn copy_notes(s: &mut Session, p: &Value) -> Result<Value> {
    let two = bool_or(p, "twoSided", false);
    let prefixes: Vec<String> =
        str_param(p, "prefixes").unwrap_or("").split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_string).collect();
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(other("Chọn ghi chú trên artboard đầu tiên cần điền."));
    }
    let st = s.doc()?;
    let d = &st.doc;
    let boards: Vec<Rect> = d.artboards.iter().map(|a| a.rect).collect();
    let mut records: Vec<NodeId> = vec![];
    let mut source: Option<usize> = None;
    for id in &ids {
        let Some(b) = d.node(*id).and_then(note_bounds) else { continue };
        let c = b.center();
        let on = |r: &Rect| (r.x0 - EPS..=r.x1 + EPS).contains(&c.x) && (r.y0 - EPS..=r.y1 + EPS).contains(&c.y);
        let Some(ai) = boards.iter().position(on) else {
            return Err(other("Ghi chú đang chọn nằm ngoài artboard. Đặt nó vào artboard đầu tiên cần điền."));
        };
        if source.is_some_and(|s0| s0 != ai) {
            return Err(other("Chỉ chọn ghi chú trên một artboard trong một lần copy."));
        }
        source = Some(ai);
        records.push(*id);
    }
    let Some(ai) = source else {
        return Err(other("Không đọc được ghi chú đang chọn."));
    };
    if two && !ai.is_multiple_of(2) {
        return Err(other("Ghi chú phải nằm trên artboard mặt trước (artboard số lẻ)."));
    }
    let step = if two { 2 } else { 1 };
    let targets: Vec<usize> = (ai + step..boards.len()).step_by(step).collect();
    let use_prefixes = !prefixes.is_empty();
    if targets.is_empty() && (two || !use_prefixes) {
        return Err(other(format!("Chưa có artboard {}tiếp theo để copy ghi chú.", if two { "mặt trước " } else { "" })));
    }
    // Khung chữ nhận nội dung: (con ghi chú, thứ tự khung chữ trong nó).
    let mut frame: Option<(usize, usize)> = None;
    if use_prefixes {
        if prefixes.len() != targets.len() + 1 {
            return Err(other(format!(
                "Cần {} nội dung cho artboard {}, {}, ...; hiện đang có {}.",
                targets.len() + 1,
                ai + 1,
                ai + 1 + step,
                prefixes.len()
            )));
        }
        let mut found = vec![];
        for (ri, id) in records.iter().enumerate() {
            let mut t = vec![];
            if let Some(n) = d.node(*id) {
                text_frames(n, &mut t);
            }
            found.extend((0..t.len()).map(|ti| (ri, ti)));
        }
        if found.len() != 1 {
            return Err(other("Khi nhập nội dung, hãy chọn một ghi chú có đúng một TextFrame (có thể nằm trong group)."));
        }
        frame = found.first().copied();
    }
    let src = boards.get(ai).copied().ok_or_else(|| other("Không tìm thấy artboard của ghi chú."))?;
    let target_rects: Vec<Rect> = targets.iter().filter_map(|t| boards.get(*t).copied()).collect();
    let n_records = records.len();
    let copies = s.edit("Copy ghi chú (DanCard)", |d, sel| {
        // Nội dung thêm vào ghi chú gốc trước, rồi mới đo khoảng cách tới góc artboard.
        let mut original = String::new();
        if let (Some((ri, ti)), Some(first)) = (frame, prefixes.first()) {
            let tid = records.get(ri).and_then(|id| nth_text(d, *id, ti)).ok_or_else(|| other("Không tìm được TextFrame của ghi chú."))?;
            if let Some(NodeKind::Text(t)) = d.node(tid).map(|n| &n.kind) {
                original = t.plain_text();
            }
            crate::cmd::textedit::set_plain_text(d, tid, &format!("{first} - {original}"))?;
        }
        let mut offsets = vec![];
        for id in &records {
            let b = d.node(*id).and_then(note_bounds).ok_or_else(|| other("Không đo được ghi chú sau khi thêm nội dung."))?;
            offsets.push(Vec2::new(b.x0 - src.x0, b.y0 - src.y0));
        }
        let mut copies = vec![];
        for (k, board) in target_rects.iter().enumerate() {
            for (ri, (id, off)) in records.iter().zip(&offsets).enumerate() {
                let n = d.node(*id).cloned().ok_or(EngineError::NoNode(*id))?;
                let (parent, index, _) = d.position(*id).ok_or(EngineError::NoNode(*id))?;
                let copy = d.reid(&n);
                let cid = d.insert(parent, index + 1, copy)?;
                anchor(d, cid, *board, *off, "Không đo được bản sao ghi chú.")?;
                if let Some((fri, fti)) = frame
                    && fri == ri
                {
                    let tid = nth_text(d, cid, fti).ok_or_else(|| other("Không tìm được TextFrame trên bản sao ghi chú."))?;
                    let pre = prefixes.get(k + 1).map_or("", String::as_str);
                    crate::cmd::textedit::set_plain_text(d, tid, &format!("{pre} - {original}"))?;
                    anchor(d, cid, *board, *off, "Không đo được bản sao sau khi thêm nội dung.")?;
                }
                copies.push(cid);
            }
        }
        if !copies.is_empty() {
            sel.set(copies.iter().copied());
        }
        Ok(copies)
    })?;
    let mut message = format!("OK: Đã copy {n_records} ghi chú từ artboard {} sang {}", ai + 1, targets.len());
    message.push_str(if two { " artboard mặt trước tiếp theo." } else { " artboard tiếp theo (bài 1 mặt)." });
    if use_prefixes {
        message.push_str(" Đã thêm nội dung theo thứ tự đã nhập.");
    }
    Ok(json!({
        "message": message,
        "ids": copies.iter().map(|i| i.0).collect::<Vec<_>>(),
        "artboard": targets.first(),
    }))
}

#[cfg(test)]
#[path = "dancard_mau_tests.rs"]
mod tests;
