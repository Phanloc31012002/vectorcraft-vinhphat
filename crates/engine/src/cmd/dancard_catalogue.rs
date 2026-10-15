//! DanCard — Dàn Catalogue đóng gáy giữa (tab Catalogue), cùng quy tắc `dcRunCatalogueAuto`.
//!
//! Chọn các trang theo thứ tự đọc (số trang chia hết cho 4), nhập khổ một trang. Tờ PON 43 × 32,5
//! cm, vùng dàn an toàn 42,2 × 29,9 cm: ưu tiên 2 cuốn / tờ (nằm trái/phải, cắt một dao ở giữa),
//! xoay cuốn khi cần, không vừa thì 1 cuốn / tờ. Mỗi trang được dựng thẳng theo khung transform và
//! giữ nguyên vector (DanCard làm phẳng thành ảnh 500 ppi), co đúng khổ, rồi ghép cặp đóng gáy giữa: tờ thứ s mặt trước
//! [n − 2s, 1 + 2s], mặt sau [2 + 2s, n − 1 − 2s]. Ruột phía trong bóp dần 0,25 mm mỗi tờ quanh
//! gáy. PON chữ L vẽ quanh vùng đã dàn (2 cuốn thêm dấu giữa) trên layer riêng, khóa lại. Ghi chú
//! bìa / ruột thêm phía trên cuốn. Cả lần dàn là một bước hoàn tác.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Artboard, Document, Node, NodeId, Scaling};
use vectorcraft_geom::{Affine, Point, Rect};

use super::layout::{rotate_node, translate_node, vis};
use super::{number_param, roots_in_paint_order};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

const MM: f64 = 72.0 / 25.4;
/// Tờ PON (mm) và vùng dàn an toàn bên trong (trái/phải 4 mm, trên/dưới 13 mm).
const FORM_W: f64 = 430.0;
const FORM_H: f64 = 325.0;
const SAFE_W: f64 = 422.0;
const SAFE_H: f64 = 299.0;
const FIT_TOL: f64 = 0.05;
/// Số tờ mỗi cột artboard.
const MAX_ROWS: usize = 16;
/// Bóp ruột mỗi tờ (mm).
const SQUEEZE_PER_SHEET: f64 = 0.25;
pub(super) const PON_LAYER: &str = "Pon Catalogue tu dong";
const LAYOUT_LAYER: &str = "Dan Catalogue tu dong";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.catalogue",
        "DanCard: Dàn Catalogue",
        [],
        None,
        "{width, height: cm (khổ một trang), coverNote?: ghi chú bìa (artboard 1), insideNote?: ghi chú ruột (thêm sau \"RUỘT n\")} dàn các trang đang chọn (thứ tự đọc: trên → dưới, trái → phải; số trang chia hết cho 4) thành catalogue đóng gáy giữa trên tờ PON 43 × 32,5 cm: tự chọn 2 hay 1 cuốn / tờ, trang dựng thẳng theo khung transform, giữ vector, mỗi tờ hai artboard trước / sau, PON tự vẽ, một bước hoàn tác → {message, copies, rotate, faces, groups}",
        has_selection,
        catalogue
    )]
}

/// Cách đặt cuốn trên tờ: số cuốn, xoay 90°, khổ cuốn (mm) khi đã đặt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plan {
    pub copies: usize,
    pub rotate: bool,
    pub book_w: f64,
    pub book_h: f64,
}

/// Cách đặt cho trang `page_w` × `page_h` mm (None: không vừa tờ PON).
pub(super) fn plan_for(page_w: f64, page_h: f64) -> Option<Plan> {
    let fits = |w: f64, h: f64| w <= SAFE_W + FIT_TOL && h <= SAFE_H + FIT_TOL;
    let (spread_w, spread_h) = (page_w * 2.0, page_h);
    // A5 tới 15 × 21,1 cm: cụm hai cuốn đã xoay vượt vùng an toàn 0,5 mm mỗi mép, PON vẫn ở ngoài.
    let two_a5_max = page_w <= 150.0 + FIT_TOL && page_h <= 211.0 + FIT_TOL && spread_h * 2.0 <= SAFE_W + FIT_TOL && spread_w <= 300.0 + FIT_TOL;
    if fits(spread_w * 2.0, spread_h) {
        Some(Plan { copies: 2, rotate: false, book_w: spread_w, book_h: spread_h })
    } else if fits(spread_h * 2.0, spread_w) || two_a5_max {
        Some(Plan { copies: 2, rotate: true, book_w: spread_h, book_h: spread_w })
    } else if fits(spread_w, spread_h) {
        Some(Plan { copies: 1, rotate: false, book_w: spread_w, book_h: spread_h })
    } else if fits(spread_h, spread_w) {
        Some(Plan { copies: 1, rotate: true, book_w: spread_h, book_h: spread_w })
    } else {
        None
    }
}

/// Một nét PON đen K 100 % dày 1 pt.
pub(super) fn pon_line(d: &mut Document, a: Point, b: Point) -> Node {
    let ink = Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0));
    let mut n = Node::path(d.alloc_id(), vectorcraft_geom::shapes::line(a, b), Appearance::basic(Paint::None, ink, 1.0));
    n.name = Some("PON".into());
    n
}

/// PON chữ L quanh vùng `r` đã dàn (trang, y hướng xuống); `two`: thêm dấu giữa cho 2 cuốn.
pub(super) fn dynamic_pon(d: &mut Document, r: Rect, two: bool) -> Vec<Node> {
    let (v, h) = (11.0, 4.803049);
    let (l, t, rr, b) = (r.x0, r.y0, r.x1, r.y1);
    let mut segs = vec![
        (Point::new(l, t - v), Point::new(l, t)),
        (Point::new(rr, t - v), Point::new(rr, t)),
        (Point::new(l - h, t), Point::new(l, t)),
        (Point::new(rr, t), Point::new(rr + h, t)),
        (Point::new(l, b), Point::new(l, b + v)),
        (Point::new(rr, b), Point::new(rr, b + v)),
        (Point::new(l - h, b), Point::new(l, b)),
        (Point::new(rr, b), Point::new(rr + h, b)),
    ];
    if two {
        let m = (l + rr) / 2.0;
        segs.push((Point::new(m, t - v), Point::new(m, t)));
        segs.push((Point::new(m, b), Point::new(m, b + v)));
    }
    segs.into_iter().map(|(a, b)| pon_line(d, a, b)).collect()
}

/// Layer tên `name` mới tinh (xoá layer cùng tên có sẵn), trên cùng.
pub(super) fn fresh_layer(d: &mut Document, name: &str) -> Result<NodeId> {
    let old: Vec<NodeId> = d.layers.iter().filter(|l| l.name.as_deref() == Some(name)).map(|l| l.id).collect();
    if !old.is_empty() && old.len() < d.layers.len() {
        d.remove_many(&old)?;
    }
    if let Some(l) = d.layers.iter().find(|l| l.name.as_deref() == Some(name)).map(|l| l.id) {
        // Layer duy nhất của tài liệu: dùng lại, dọn trống.
        if let Some(ch) = d.node_mut(l).and_then(Node::children_mut) {
            ch.clear();
        }
        return Ok(l);
    }
    let n = crate::cmd::layer::make_layer(d, Some(name));
    Ok(d.insert(None, usize::MAX, n)?)
}

/// Đặt tâm vùng nhìn thấy của `id` tại `c`.
pub(super) fn center_at(d: &mut Document, id: NodeId, c: Point) {
    if let Some(b) = d.node(id).and_then(vis) {
        translate_node(d, id, c - b.center());
    }
}

/// Co giãn `id` (quanh tâm) cho vùng nhìn thấy đúng `w` × `h` pt.
pub(super) fn resize_to(d: &mut Document, id: NodeId, w: f64, h: f64) {
    let Some(b) = d.node(id).and_then(vis).filter(|b| b.width() > 0.0 && b.height() > 0.0) else { return };
    let c = b.center().to_vec2();
    let m = Affine::translate(c) * Affine::scale_non_uniform(w / b.width(), h / b.height()) * Affine::translate(-c);
    if let Some(n) = d.node_mut(id) {
        n.transform(m, Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
}

/// Co giãn `id` theo chiều ngang `kx` quanh tâm.
fn squeeze_x(d: &mut Document, id: NodeId, kx: f64) {
    let Some(c) = d.node(id).and_then(vis).map(|b| b.center().to_vec2()) else { return };
    let m = Affine::translate(c) * Affine::scale_non_uniform(kx, 1.0) * Affine::translate(-c);
    if let Some(n) = d.node_mut(id) {
        n.transform(m, Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
}

/// Các trang theo thứ tự đọc: hàng trên trước (cùng hàng nếu mép trên lệch dưới 20 mm), trái → phải.
pub(super) fn reading_order(d: &Document, ids: &[NodeId]) -> Result<Vec<NodeId>> {
    let mut items = vec![];
    for id in ids {
        let n = d.node(*id).ok_or(EngineError::NoNode(*id))?;
        if n.locked {
            return Err(EngineError::Other("Mở khóa layer và object đã chọn trước khi dàn.".into()));
        }
        let b =
            super::upright_copy(d, *id).as_ref().and_then(vis).ok_or_else(|| EngineError::Other("Có artwork không đo được kích thước.".into()))?;
        items.push((*id, b));
    }
    items.sort_by(|a, b| {
        let dy = a.1.y0 - b.1.y0;
        if dy.abs() > 20.0 * MM { dy.total_cmp(&0.0) } else { a.1.x0.total_cmp(&b.1.x0) }
    });
    Ok(items.into_iter().map(|(id, _)| id).collect())
}

/// Một trang dựng thẳng theo khung transform (giữ vector) trên layer `layer`, co đúng `w` × `h` pt.
pub(super) fn place_page(d: &mut Document, id: NodeId, layer: NodeId, w: f64, h: f64) -> Result<NodeId> {
    if d.node(id).and_then(vis).is_none() {
        return Err(EngineError::Other("Có trang không đo được kích thước.".into()));
    }
    super::make_upright(d, id);
    // Chuyển sang layer dàn.
    let removed = d.remove_many(&[id])?;
    let node = removed.into_iter().next().map(|a| (*a).clone()).ok_or(EngineError::NoNode(id))?;
    let page = d.insert(Some(layer), usize::MAX, node)?;
    resize_to(d, page, w, h);
    Ok(page)
}

fn catalogue(s: &mut Session, p: &Value) -> Result<Value> {
    let (Some(pw_cm), Some(ph_cm)) = (number_param(p, "width"), number_param(p, "height")) else {
        return Err(EngineError::Other("Nhập kích thước 1 trang hợp lệ (cm).".into()));
    };
    if pw_cm <= 0.0 || ph_cm <= 0.0 {
        return Err(EngineError::Other("Nhập kích thước 1 trang hợp lệ (cm).".into()));
    }
    let (page_w, page_h) = (pw_cm * 10.0, ph_cm * 10.0);
    let plan = plan_for(page_w, page_h).ok_or_else(|| {
        EngineError::Other(format!("Khổ {pw_cm:.2} x {ph_cm:.2} cm không nằm trong vùng dàn 42.2 x 29.9 cm của tờ PON 43 x 32.5 cm."))
    })?;
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Chọn các trang catalogue trước khi dàn.".into()));
    }
    if ids.len() % 4 != 0 {
        return Err(EngineError::Other(format!("Số trang catalogue phải là bội số của 4. Đang chọn {} trang.", ids.len())));
    }
    let cover_note = str_param(p, "coverNote").map(str::trim).unwrap_or("").to_string();
    let inside_note = str_param(p, "insideNote").map(str::trim).unwrap_or("").to_string();
    let st = s.doc()?;
    let order = reading_order(&st.doc, &ids)?;
    let sel_area = super::layout::union_of(order.iter().filter_map(|id| st.doc.node(*id).and_then(vis)))
        .ok_or_else(|| EngineError::Other("Có artwork không đo được kích thước.".into()))?;
    let n_pages = order.len();
    let num_sheets = n_pages / 4;
    let num_faces = num_sheets * 2;
    let (form_w, form_h) = (FORM_W * MM, FORM_H * MM);
    let imposed_w = plan.book_w * MM * plan.copies as f64;
    let imposed_h = plan.book_h * MM;
    let (page_wpt, page_hpt) = (page_w * MM, page_h * MM);
    s.begin_named_undo_group(Some("Dàn Catalogue (DanCard)".into()));
    let result = s.edit("Dàn Catalogue (DanCard)", |d, sel| {
        let layout_layer = {
            let n = crate::cmd::layer::make_layer(d, Some(LAYOUT_LAYER));
            d.insert(None, usize::MAX, n)?
        };
        let mut pages = vec![];
        for id in &order {
            pages.push(place_page(d, *id, layout_layer, page_wpt, page_hpt)?);
        }
        // Artboard: mỗi tờ hai artboard (trước, sau) cạnh nhau, tối đa 16 tờ một cột.
        let pon_layer = fresh_layer(d, PON_LAYER)?;
        let mut boards = vec![];
        let mut pons = vec![];
        for fi in 0..num_faces {
            let (sheet, side) = (fi / 2, fi % 2);
            let (block, row) = (sheet / MAX_ROWS, sheet % MAX_ROWS);
            let left = sel_area.x0 + block as f64 * form_w * 2.0 + side as f64 * form_w;
            let top = sel_area.y0 + row as f64 * form_h;
            let rect = Rect::new(left, top, left + form_w, top + form_h);
            let c = rect.center();
            pons.extend(dynamic_pon(
                d,
                Rect::new(c.x - imposed_w / 2.0, c.y - imposed_h / 2.0, c.x + imposed_w / 2.0, c.y + imposed_h / 2.0),
                plan.copies == 2,
            ));
            boards.push((rect, format!("Catalogue - {} {}", if side == 0 { "Mat truoc" } else { "Mat sau" }, sheet + 1)));
        }
        if let Some(ch) = d.node_mut(pon_layer).and_then(Node::children_mut) {
            ch.extend(pons.into_iter().map(Arc::new));
        }
        // PON lên trên cùng, khóa lại.
        if let Some(at) = d.layers.iter().position(|l| l.id == pon_layer) {
            let mut l = d.layers.remove(at);
            Arc::make_mut(&mut l).locked = true;
            d.layers.push(l);
        }
        let first_id = d.next_artboard_id();
        d.artboards = boards
            .iter()
            .enumerate()
            .map(|(i, (rect, name))| Artboard {
                id: first_id + i as u32,
                name: name.clone(),
                rect: *rect,
                show_center_mark: false,
                show_cross_hairs: false,
            })
            .collect();
        // Cặp trang của từng mặt (số trang tính từ 1).
        let mut faces = vec![];
        for sheet in 0..num_sheets {
            faces.push([n_pages - 2 * sheet, 1 + 2 * sheet]);
            faces.push([2 + 2 * sheet, n_pages - 1 - 2 * sheet]);
        }
        let mut groups = vec![];
        for (fi, pair) in faces.iter().enumerate() {
            let Some((rect, _)) = boards.get(fi) else { continue };
            let c = rect.center();
            let squeeze = (fi / 2) as f64 * SQUEEZE_PER_SHEET * MM;
            let eff_page_w = page_wpt - squeeze / 2.0;
            let eff_book_w = if plan.rotate { plan.book_w * MM } else { eff_page_w * 2.0 };
            for copy in (0..plan.copies).rev() {
                let (Some(&l), Some(&r)) = (pages.get(pair[0].wrapping_sub(1)), pages.get(pair[1].wrapping_sub(1))) else {
                    return Err(EngineError::Other("Thiếu trang trong bộ dàn.".into()));
                };
                // Bản nhân bản cho cuốn thứ hai; trang gốc dùng cho cuốn đầu.
                let (l, r) = if copy > 0 {
                    let (ln, rn) = (d.node(l).cloned().ok_or(EngineError::NoNode(l))?, d.node(r).cloned().ok_or(EngineError::NoNode(r))?);
                    let (ln, rn) = (d.reid(&ln), d.reid(&rn));
                    (d.insert(Some(layout_layer), usize::MAX, ln)?, d.insert(Some(layout_layer), usize::MAX, rn)?)
                } else {
                    (l, r)
                };
                let book_cx = if plan.copies == 2 { c.x + if copy == 0 { -eff_book_w / 2.0 } else { eff_book_w / 2.0 } } else { c.x };
                squeeze_x(d, l, eff_page_w / page_wpt);
                squeeze_x(d, r, eff_page_w / page_wpt);
                center_at(d, l, Point::new(book_cx - eff_page_w / 2.0, c.y));
                center_at(d, r, Point::new(book_cx + eff_page_w / 2.0, c.y));
                let gid = d.alloc_id();
                let gid = d.wrap(&[l, r], |children| {
                    let mut g = Node::group(gid, children);
                    g.name = Some(format!("Catalogue {} - cuon {}", fi + 1, copy + 1));
                    g
                })?;
                // Mặt trước xoay trái, mặt sau xoay phải; không lật ngang.
                if plan.rotate {
                    rotate_node(d, gid, if fi % 2 == 0 { 90.0 } else { -90.0 });
                }
                center_at(d, gid, Point::new(book_cx, c.y));
                groups.push(gid);
            }
        }
        sel.set(groups.iter().copied());
        Ok((groups, boards.into_iter().map(|(r, _)| r).collect::<Vec<_>>()))
    });
    let (groups, boards) = match result {
        Ok(v) => v,
        Err(e) => {
            s.end_undo_group(true);
            return Err(e);
        }
    };
    // Ghi chú: bìa trên artboard 1, ruột trên các mặt trước còn lại ("RUỘT n …"), ngay trên cuốn.
    let mut notes = vec![];
    let mut inside = 0;
    for (i, rect) in boards.iter().enumerate().step_by(2) {
        let text = if i == 0 {
            cover_note.clone()
        } else {
            inside += 1;
            if inside_note.is_empty() { format!("RUỘT {inside}") } else { format!("RUỘT {inside} {inside_note}") }
        };
        if text.is_empty() {
            continue;
        }
        let imposed_top = rect.center().y - imposed_h / 2.0;
        let x = rect.x0 + 20.0 * MM;
        let y = imposed_top - 6.0 * MM + 13.0 * 0.75;
        if let Err(e) = s.execute("text.create", &json!({"x": x, "y": y, "text": text, "size": 13, "color": {"c": 0, "m": 0, "y": 0, "k": 1}})) {
            notes.push(format!("Không thêm được ghi chú: {e}"));
            break;
        }
    }
    s.end_undo_group(false);
    if let Some(st) = s.active_mut() {
        st.selection.set(groups.iter().copied());
    }
    let message = format!(
        "OK: Đã dàn {} {}PON {} đã được vẽ tự động; {num_faces} artboard trước/sau.",
        plan.copies,
        if plan.copies == 2 { "cuốn giống nhau / tờ, xả 1 dao ở giữa. " } else { "cuốn / tờ. " },
        if plan.copies == 2 { "2-cuốn" } else { "1-cuốn" }
    );
    Ok(json!({
        "message": message,
        "copies": plan.copies,
        "rotate": plan.rotate,
        "faces": num_faces,
        "groups": groups.iter().map(|g| g.0).collect::<Vec<_>>(),
        "warnings": notes,
    }))
}
