//! DanCard — Auto Save PDF (tab Auto Save), cùng quy tắc `dcAutoSavePDF`, và Clip 9,2 × 5,6 tự
//! xoay (`dcClipCard926`).
//!
//! Các bài đang chọn xếp thành hàng: cột trái là mặt trước, cột phải là mặt sau; hàng chỉ có một
//! object là con 1 mặt. Mỗi con (với mỗi số hộp) lưu một PDF: 1 trang (1 mặt) hoặc 2 trang (trước,
//! sau), mỗi trang đúng khung clip của mặt đó. Tên file:
//! `<mã> - <STT> - <số hộp> hop[ - ghi chú][ - 1 mat] - <cán màng> - <ngày>.pdf`. Không sửa tài
//! liệu đang mở, không ghi đè file đã có.
//!
//! Khác DanCard: mỗi mặt được dựng thẳng theo khung transform trước khi đo trang (card xoay trong
//! app hay đặt nghiêng trong file .ai vẫn ra trang đúng khổ), và Clip card đo, dựng mask theo khung
//! transform (phần nghiêng lẻ) của phạm vi clip.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Artboard, Document, Node, NodeId, NodeKind, Scaling};
use vectorcraft_geom::{Affine, FillRule, Rect, Vec2, shapes};

use super::{number_param, roots_in_paint_order};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

const MM: f64 = 72.0 / 25.4;
/// Khoảng cách hai trang trong tài liệu tạm (pt).
const GAP: f64 = 20.0 * MM;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "dancard.autoSavePdf",
            "DanCard: Auto Save PDF",
            [],
            None,
            "{prefix?: mã (mặc định 1088), start?: STT bắt đầu (1), pad?: đệm số 0 (0–8), boxes?: số hộp (nhiều số thì ngăn bằng dấu phẩy, vd. \"100, 200\"), boxesInOrder?: bool (mỗi con một số hộp theo thứ tự; mặc định mỗi con × mỗi số), note?: ghi chú thêm vào tên, lamination?: cm|km (cán màng / không màng), date?: ngày đặt in, folder: thư mục lưu (phải có sẵn), dryRun?: bool (chỉ trả danh sách tên file)} lưu mỗi con card đang chọn thành PDF riêng (hàng: trái = trước, phải = sau; 1 mặt: 1 trang, 2 mặt: 2 trang, trang đúng khung clip), không sửa tài liệu, không ghi đè file có sẵn → {message, files: [tên], saved, oneSide, twoSides}",
            has_selection,
            auto_save_pdf
        ),
        cmd!(
            "dancard.clipCard",
            "DanCard: Clip card tự xoay",
            [],
            None,
            "{width?, height?: cm (mặc định 9,2 × 5,6)} clip mỗi object đang chọn thành card đúng khổ như dcClipCard926: khung tự xoay theo clip cha gần nhất hoặc chính object (nằm ngang: dài × ngắn; đứng dọc: ngắn × dài, như đang thấy trên trang), neo tâm theo clip cha (chọn object con thì clip cả clip group), không có thì theo nền đúng khổ trong group trông như một trang, không có nữa thì theo khung của object; mask nghiêng theo khung transform; mỗi card một lần, clip cha đã đúng khổ thì giữ; một bước hoàn tác → {message, ids, errors}",
            has_selection,
            clip_card
        ),
    ]
}

// ---------------------------------------------------------------- đo

/// Khung một mặt: clip group theo clipping path, còn lại theo khung hình học (như DanCard).
fn frame_of(n: &Node) -> Option<Rect> {
    if let NodeKind::Group { children, clip: true } = &n.kind
        && let Some(b) = children.first().and_then(|m| m.geometric_bounds())
    {
        return Some(b);
    }
    n.visual_bounds().or_else(|| n.geometric_bounds())
}

/// Một con card: mặt trước, mặt sau (con 1 mặt: không có).
struct Card {
    front: NodeId,
    back: Option<NodeId>,
}

/// Gom các object thành hàng (dung sai 45 % chiều cao object nhỏ nhất, ít nhất 4 pt), trên → dưới,
/// mỗi hàng trái → phải.
fn cards_of(d: &Document, ids: &[NodeId]) -> std::result::Result<(Vec<Card>, usize), String> {
    let mut items = vec![];
    for (i, id) in ids.iter().enumerate() {
        let b = super::upright_copy(d, *id)
            .and_then(|n| n.geometric_bounds())
            .filter(|b| b.width() > 0.0 && b.height() > 0.0 && b.area().is_finite())
            .ok_or_else(|| format!("Object {} không có khung hợp lệ. Hãy bỏ object lỗi/đang ẩn rồi thử lại.", i + 1))?;
        items.push((*id, b));
    }
    let min_h = items.iter().map(|(_, b)| b.height()).fold(f64::INFINITY, f64::min);
    let tol = (if min_h.is_finite() { min_h } else { 10.0 } * 0.45).max(4.0);
    items.sort_by(|a, b| a.1.center().y.total_cmp(&b.1.center().y));
    let mut rows: Vec<(f64, Vec<(NodeId, Rect)>)> = vec![];
    for (id, b) in items {
        let cy = b.center().y;
        match rows.last_mut() {
            Some((ry, row)) if (cy - *ry).abs() <= tol => {
                let n = row.len() as f64;
                *ry = (*ry * n + cy) / (n + 1.0);
                row.push((id, b));
            }
            _ => rows.push((cy, vec![(id, b)])),
        }
    }
    let (mut left, mut right, mut pairs) = (0.0, 0.0, 0usize);
    for (i, (_, row)) in rows.iter_mut().enumerate() {
        row.sort_by(|a, b| a.1.center().x.total_cmp(&b.1.center().x));
        if row.len() > 2 {
            return Err(format!("Hàng {} có {} object. Mỗi hàng chỉ đặt 1 object (1 mặt) hoặc 2 object (trước + sau).", i + 1, row.len()));
        }
        if let [a, b] = row.as_slice() {
            left += a.1.x0;
            right += b.1.x0;
            pairs += 1;
        }
    }
    let (lx, rx) = if pairs > 0 { (left / pairs as f64, right / pairs as f64) } else { (0.0, 0.0) };
    let mut cards = vec![];
    let mut one_side = 0;
    for (i, (_, row)) in rows.iter().enumerate() {
        match row.as_slice() {
            [a, b] => cards.push(Card { front: a.0, back: Some(b.0) }),
            [a] => {
                if pairs > 0 && (a.1.x0 - rx).abs() < (a.1.x0 - lx).abs() {
                    return Err(format!(
                        "Hàng {} chỉ có object ở gần cột phải. Hãy chọn thêm mặt trước ở cột trái, hoặc đặt con 1 mặt ở cột trái.",
                        i + 1
                    ));
                }
                cards.push(Card { front: a.0, back: None });
                one_side += 1;
            }
            _ => {}
        }
    }
    Ok((cards, one_side))
}

// ---------------------------------------------------------------- tên file

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c })
        .collect::<String>()
        .trim_end()
        .to_string()
}

struct Naming {
    prefix: String,
    start: u64,
    pad: usize,
    boxes: Vec<String>,
    in_order: bool,
    note: String,
    lamination: String,
    date: String,
}

impl Naming {
    fn parse(p: &Value) -> Result<Self> {
        let text = |k: &str| match p.get(k) {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let prefix = Some(text("prefix")).filter(|s| !s.is_empty()).unwrap_or_else(|| "1088".into());
        let start_text = Some(text("start")).filter(|s| !s.is_empty()).unwrap_or_else(|| "1".into());
        if !start_text.chars().all(|c| c.is_ascii_digit()) || start_text.len() > 12 {
            return Err(EngineError::Other("STT bắt đầu phải là số nguyên từ 0 trở lên.".into()));
        }
        let pad_text = Some(text("pad")).filter(|s| !s.is_empty()).unwrap_or_else(|| "0".into());
        let pad = pad_text.parse::<usize>().map_err(|_| EngineError::Other("Số đệm phải là số nguyên từ 0 đến 8.".into()))?;
        if pad > 8 {
            return Err(EngineError::Other("Số đệm tối đa là 8.".into()));
        }
        let mut boxes: Vec<String> = text("boxes").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if boxes.is_empty() {
            boxes.push(String::new());
        }
        Ok(Naming {
            prefix,
            start: start_text.parse().unwrap_or(1),
            pad,
            boxes,
            in_order: bool_or(p, "boxesInOrder", false),
            note: text("note"),
            lamination: Some(text("lamination")).filter(|s| !s.is_empty()).unwrap_or_else(|| "cm".into()),
            date: text("date"),
        })
    }

    fn name(&self, stt: u64, one_side: bool, boxes: &str) -> String {
        let stt = format!("{stt:0width$}", width = self.pad);
        let note = if self.note.is_empty() { String::new() } else { format!(" - {}", self.note) };
        let side = if one_side { " - 1 mat" } else { "" };
        sanitize(&format!("{} - {stt} - {boxes} hop{note}{side} - {} - {}.pdf", self.prefix, self.lamination, self.date))
    }
}

// ---------------------------------------------------------------- xuất

/// Tài liệu tạm của một con card: mặt trước (và mặt sau đặt bên phải, cùng mép trên), mỗi mặt một
/// artboard đúng khung của nó.
fn card_document(d: &Document, card: &Card) -> Result<Document> {
    let front = super::upright_copy(d, card.front).ok_or(EngineError::NoNode(card.front))?;
    let fb = frame_of(&front).ok_or_else(|| EngineError::Other("Không đo được mặt trước.".into()))?;
    let mut nodes = vec![front];
    let mut frames = vec![fb];
    if let Some(back) = card.back {
        let mut back = super::upright_copy(d, back).ok_or(EngineError::NoNode(back))?;
        let bb = frame_of(&back).ok_or_else(|| EngineError::Other("Không đo được mặt sau.".into()))?;
        let by = Vec2::new(fb.x1 + GAP - bb.x0, fb.y0 - bb.y0);
        back.transform(Affine::translate(by), Scaling { strokes: true, patterns: true, ..Scaling::default() });
        frames.push(bb + by);
        nodes.push(back);
    }
    for n in &mut nodes {
        n.visible = true;
    }
    let mut tmp = crate::cmd::menucmds::isolated_doc(d, nodes);
    tmp.artboards = frames
        .into_iter()
        .enumerate()
        .map(|(i, rect)| Artboard { id: i as u32 + 1, name: format!("Artboard {}", i + 1), rect, show_center_mark: false, show_cross_hairs: false })
        .collect();
    Ok(tmp)
}

fn auto_save_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.autoSavePdf";
    let naming = Naming::parse(p)?;
    let dry = bool_or(p, "dryRun", false);
    let folder = str_param(p, "folder").map(str::trim).unwrap_or("").to_string();
    if folder.is_empty() && !dry {
        return Err(EngineError::Other("Chưa chọn thư mục lưu. Hãy bấm \"Chọn…\" để chọn thư mục.".into()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if !dry && !std::path::Path::new(&folder).is_dir() {
        return Err(EngineError::Other(format!("Thư mục lưu không mở được: {folder}\nHãy bấm \"Chọn…\" để chọn đúng thư mục.")));
    }
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Chưa chọn object nào. Hãy chọn các object theo hàng (trái = trước, phải = sau).".into()));
    }
    let st = s.doc()?;
    let d = &st.doc;
    let (cards, one_side) = cards_of(d, &ids).map_err(EngineError::Other)?;
    if naming.in_order && naming.boxes.len() != cards.len() {
        return Err(EngineError::Other(format!(
            "Đang bật ghép số hộp theo thứ tự, nhưng có {} con card và {} số hộp. Hãy nhập đúng một số hộp cho mỗi con.",
            cards.len(),
            naming.boxes.len()
        )));
    }
    // Danh sách file: mỗi con × mỗi số hộp (hoặc mỗi con một số hộp theo thứ tự); STT tăng theo file.
    let mut plan: Vec<(usize, String)> = vec![];
    let mut stt = naming.start;
    for (ci, card) in cards.iter().enumerate() {
        let boxes: Vec<&String> = if naming.in_order { naming.boxes.get(ci).into_iter().collect() } else { naming.boxes.iter().collect() };
        for b in boxes {
            let name = naming.name(stt, card.back.is_none(), b);
            if name.is_empty() || name == ".pdf" {
                return Err(EngineError::Other("Tên file PDF rỗng sau khi chuẩn hoá.".into()));
            }
            if name.chars().count() > 220 {
                return Err(EngineError::Other(format!("Tên PDF quá dài ở STT {stt}. Hãy rút gọn ghi chú.")));
            }
            if plan.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) {
                return Err(EngineError::Other(format!("Có hai file trùng tên: {name}. Hãy đổi STT, số hộp hoặc ghi chú.")));
            }
            plan.push((ci, name));
            stt += 1;
        }
    }
    let names: Vec<String> = plan.iter().map(|(_, n)| n.clone()).collect();
    let two = cards.len() - one_side;
    if dry {
        return Ok(
            json!({"message": format!("{} file sẽ được lưu.", names.len()), "files": names, "saved": 0, "oneSide": one_side, "twoSides": two}),
        );
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (C, &plan, d);
        Err(EngineError::Other("Bản web không lưu được PDF vào thư mục.".into()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = std::path::Path::new(&folder);
        for (_, name) in &plan {
            if dir.join(name).exists() {
                return Err(EngineError::Other(format!("File đã tồn tại: {name}. Auto Save dừng để không ghi đè file cũ.")));
            }
        }
        let settings = json!({"compatibility": "1.4", "preserveEditing": false, "thumbnails": false});
        let mut saved = 0;
        let mut errors = vec![];
        for (ci, name) in &plan {
            let Some(card) = cards.get(*ci) else { continue };
            let path = dir.join(name);
            let r = card_document(d, card)
                .and_then(|tmp| crate::cmd::fileio::pdf::encode(C, &tmp, &settings))
                .and_then(|(bytes, _)| crate::cmd::fileio::write_file(&path.to_string_lossy(), &bytes));
            match r {
                Ok(()) => saved += 1,
                Err(e) => {
                    // File dở dang không được để lẫn với các file đã xuất xong.
                    let _ = std::fs::remove_file(&path);
                    errors.push(format!("{name}: {e}"));
                }
            }
        }
        let total = plan.len();
        let mut message = format!(
            "{}Đã lưu {saved}/{total} file PDF.\nNhận diện: {one_side} con 1 mặt, {two} con 2 mặt.\nThư mục: {folder}",
            if errors.is_empty() || saved > 0 { "OK: " } else { "ERR: " }
        );
        if two == 0 && !cards.is_empty() {
            message.push_str("\nKhông có hàng 2 cột: toàn bộ selection được coi là con 1 mặt.");
        }
        if !errors.is_empty() {
            message.push_str(&format!("\nLỗi:\n- {}", errors.join("\n- ")));
        }
        if saved == 0 {
            return Err(EngineError::Other(message.trim_start_matches("ERR: ").to_string()));
        }
        Ok(json!({"message": message, "files": names, "saved": saved, "oneSide": one_side, "twoSides": two, "errors": errors}))
    }
}

// ---------------------------------------------------------------- Clip 9,2 × 5,6 tự xoay

/// Số tầng tối đa khi dò clip cha / group có nền (như DanCard).
const SCOPE_DEPTH: usize = 12;

/// Neo của mask Clip card.
#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    /// Clipping path của clip group gần nhất chứa object (khung trang đã duyệt).
    Clip,
    /// Nền đúng khổ trong group gần nhất trông như một trang.
    Background,
    /// Khung hình học của chính object.
    Fallback,
}

/// Phạm vi Clip card của một object (`_dcClipToSizeCore` của DanCard): object được bọc, hệ trục đo
/// (trang → hệ trục phần nghiêng lẻ của khung transform của nó, góc đó), khung neo trong hệ trục đó.
#[derive(Clone, Copy)]
struct Scope {
    id: NodeId,
    to_local: Affine,
    tilt: f64,
    anchor: Rect,
    kind: Anchor,
}

/// Hệ trục đo của `id`: phần nghiêng lẻ (-45°, 45°] của khung transform, như các lệnh dàn (ngang ×
/// dọc như đang thấy trên trang).
fn axes_of(d: &Document, id: NodeId) -> (Affine, f64) {
    match super::frame_of(d, &[id], false).map(|f| super::tilt_of(f.angle)) {
        Some(t) if t.is_finite() && t != 0.0 => (Affine::rotate(t.to_radians()), t),
        _ => (Affine::IDENTITY, 0.0),
    }
}

/// `n` đưa sang hệ trục `to_local`.
fn in_axes(n: &Node, to_local: Affine) -> Node {
    let mut p = n.clone();
    if to_local != Affine::IDENTITY {
        p.transform(to_local, Scaling::default());
    }
    p
}

/// `id` rồi các group chứa nó, tối đa [`SCOPE_DEPTH`] tầng, dừng ở layer.
fn upward(d: &Document, id: NodeId) -> Vec<NodeId> {
    let chain = d.ancestry(id).unwrap_or_default();
    chain.iter().rev().take(SCOPE_DEPTH).map_while(|a| d.node(*a).filter(|n| !n.is_layer()).map(|n| n.id)).collect()
}

/// `findScopeByExistingClip`: clip group gần nhất (chính `id` hoặc group chứa nó) có clipping path.
fn clip_scope(d: &Document, id: NodeId) -> Option<Scope> {
    upward(d, id).into_iter().find_map(|a| {
        let n = d.node(a)?;
        let NodeKind::Group { children, clip: true } = &n.kind else { return None };
        let (to_local, tilt) = axes_of(d, a);
        let anchor = in_axes(children.first()?, to_local).geometric_bounds().filter(|b| b.width() > 0.0 && b.height() > 0.0)?;
        Some(Scope { id: a, to_local, tilt, anchor, kind: Anchor::Clip })
    })
}

/// `frameMatchesSize`: khung `b` đúng `w` × `h` (dung sai max(1,5 pt; 0,6 % cạnh ngắn)).
fn card_sized(b: Rect, w: f64, h: f64) -> bool {
    let tol = (w.min(h) * 0.006).max(1.5);
    (b.width() - w).abs() <= tol && (b.height() - h).abs() <= tol
}

/// `scanBackgroundFrames`: path, compound path, ảnh trong `n` (đi qua các group) có khung đúng khổ.
fn backgrounds(n: &Node, w: f64, h: f64, out: &mut Vec<Rect>) {
    match &n.kind {
        NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. } | NodeKind::Image(_) | NodeKind::PlacedDocument(_) => {
            if let Some(b) = n.geometric_bounds().filter(|b| card_sized(*b, w, h)) {
                out.push(b);
            }
        }
        NodeKind::Group { children, .. } => {
            for c in children {
                backgrounds(c, w, h, out);
            }
        }
        _ => {}
    }
}

/// `scopeLooksLikeOnePage`: tâm phạm vi lệch tâm nền tối đa 30 % mỗi chiều, phạm vi tối đa 1,8 lần nền.
fn one_page(scope: Option<Rect>, f: Rect) -> bool {
    let Some(s) = scope else { return false };
    let (sc, fc) = (s.center(), f.center());
    (sc.x - fc.x).abs() <= f.width() * 0.3
        && (sc.y - fc.y).abs() <= f.height() * 0.3
        && s.width() <= f.width() * 1.8
        && s.height() <= f.height() * 1.8
}

/// `findScopeByBackground`: group gần nhất (chính `id` hoặc group chứa nó) có đúng một nền `w` × `h`
/// (nhiều nền thì nền gần tâm group nhất) và trông như một trang.
fn background_scope(d: &Document, id: NodeId, w: f64, h: f64) -> Option<Scope> {
    upward(d, id).into_iter().find_map(|a| {
        let n = d.node(a).filter(|n| matches!(n.kind, NodeKind::Group { .. }))?;
        let (to_local, tilt) = axes_of(d, a);
        let probe = in_axes(n, to_local);
        let mut found = Vec::new();
        backgrounds(&probe, w, h, &mut found);
        let scope_box = probe.visual_bounds();
        let anchor = match found.as_slice() {
            [] => None,
            [one] => Some(*one),
            many => {
                let c = scope_box.map(|b| b.center());
                let dist = |r: &Rect| c.map_or(0.0, |c| (c.x - r.center().x).abs() + (c.y - r.center().y).abs());
                many.iter().copied().min_by(|a, b| dist(a).total_cmp(&dist(b)))
            }
        }
        .filter(|f| one_page(scope_box, *f))?;
        Some(Scope { id: a, to_local, tilt, anchor, kind: Anchor::Background })
    })
}

/// Khung hình học của chính `id` trong hệ trục của nó (`geometricBounds || visibleBounds`).
fn own_scope(d: &Document, id: NodeId) -> Option<Scope> {
    let n = d.node(id)?;
    let (to_local, tilt) = axes_of(d, id);
    let probe = in_axes(n, to_local);
    let positive = |b: &Rect| b.width() > 0.0 && b.height() > 0.0 && b.area().is_finite();
    let anchor = probe.geometric_bounds().filter(positive).or_else(|| probe.visual_bounds().filter(positive))?;
    Some(Scope { id, to_local, tilt, anchor, kind: Anchor::Fallback })
}

/// Bọc `scope` trong clip group mới: mask `w` × `h` (không tô, không viền) đặt tâm tại tâm khung neo,
/// dựng trong hệ trục của phạm vi rồi đưa ra trang (`clipOne` của DanCard).
fn wrap_scope(d: &mut Document, scope: &Scope, w: f64, h: f64) -> Result<NodeId> {
    let c = scope.anchor.center();
    let local = Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0);
    let path =
        if scope.to_local == Affine::IDENTITY { shapes::rectangle(local) } else { shapes::rectangle(local).transformed(scope.to_local.inverse()) };
    let (parent, index, _) = d.position(scope.id).ok_or(EngineError::NoNode(scope.id))?;
    let mask_id = d.alloc_id();
    let mut mask = Node::new(mask_id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: true, guide: false });
    mask.bbox_angle = scope.tilt;
    mask.appearance = vectorcraft_doc::Appearance::basic(vectorcraft_color::Paint::None, vectorcraft_color::Paint::None, 0.0).into();
    mask.name = Some("Khung clip".into());
    d.insert(parent, index + 1, mask)?;
    let gid = d.alloc_id();
    let tilt = scope.tilt;
    Ok(d.wrap(&[scope.id, mask_id], |mut children: Vec<Arc<Node>>| {
        children.rotate_right(1);
        let mut g = Node::new(gid, NodeKind::Group { children, clip: true });
        g.bbox_angle = tilt;
        g
    })?)
}

/// `dcClipCard926`: mỗi object đang chọn thành card đúng khổ, khung tự xoay theo object (nằm ngang:
/// dài × ngắn; đứng dọc: ngắn × dài, đo trên clip cha gần nhất hoặc chính object), rồi clip như
/// `_dcClipToSizeCore`: tìm phạm vi (clip cha, group có nền đúng khổ, hoặc chính object), mỗi phạm vi
/// một lần, mask đặt tâm tại khung neo. Đo theo khung transform: phần nghiêng lẻ của khung là trục của
/// mask, nên card đặt nghiêng vẫn clip đúng cạnh.
fn clip_card(s: &mut Session, p: &Value) -> Result<Value> {
    let cm = 72.0 / 2.54;
    let w = number_param(p, "width").filter(|v| *v > 0.0).unwrap_or(9.2) * cm;
    let h = number_param(p, "height").filter(|v| *v > 0.0).unwrap_or(5.6) * cm;
    let (long, short) = (w.max(h), w.min(h));
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Hãy chọn ít nhất 1 object để clip 9.2 x 5.6.".into()));
    }
    let total = ids.len();
    let (done, errors) = s.edit("Clip card tự xoay (DanCard)", |d, sel| {
        let mut done: Vec<NodeId> = vec![];
        let mut handled: Vec<NodeId> = vec![];
        let mut errors = vec![];
        for (i, id) in ids.iter().enumerate() {
            if d.node(*id).is_none() {
                continue;
            }
            // Hướng ngang/dọc theo clip cha gần nhất, không có thì theo khung của chính object.
            let clip = clip_scope(d, *id);
            let Some(reference) = clip.map(|c| c.anchor).or_else(|| own_scope(d, *id).map(|c| c.anchor)) else {
                errors.push(format!("Object {}: không đo được object.", i + 1));
                continue;
            };
            let (cw, ch) = if reference.height() > reference.width() { (short, long) } else { (long, short) };
            let Some(scope) = clip.or_else(|| background_scope(d, *id, cw, ch)).or_else(|| own_scope(d, *id)) else {
                errors.push(format!("Object {}: không đo được object.", i + 1));
                continue;
            };
            // Nhiều object con của cùng một trang: chỉ clip trang đó một lần.
            if handled.contains(&scope.id) {
                continue;
            }
            handled.push(scope.id);
            // Clip cha đã đúng khổ: giữ nguyên.
            if scope.kind == Anchor::Clip && (scope.anchor.width() - cw).abs() < 0.01 && (scope.anchor.height() - ch).abs() < 0.01 {
                done.push(scope.id);
                continue;
            }
            match wrap_scope(d, &scope, cw, ch) {
                Ok(g) => done.push(g),
                Err(e) => errors.push(format!("Object {}: {e}", i + 1)),
            }
        }
        if done.is_empty() {
            return Err(EngineError::Other(format!("Không clip được object nào.\n{}", errors.join("\n"))));
        }
        sel.set(done.iter().copied());
        Ok((done, errors))
    })?;
    let fmt = |v: f64| {
        let t = format!("{:.2}", v / cm);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut message = format!("OK: Đã clip {}/{total} object thành {} x {} (tự xoay, dùng chung logic Clip).", done.len(), fmt(long), fmt(short));
    if !errors.is_empty() {
        message.push_str(&format!("\n{}", errors.join("\n")));
    }
    Ok(json!({"message": message, "ids": done.iter().map(|g| g.0).collect::<Vec<_>>(), "errors": errors}))
}
