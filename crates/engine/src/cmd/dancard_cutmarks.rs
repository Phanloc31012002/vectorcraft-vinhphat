//! DanCard — Dấu cắt tự động (tab Dấu cắt), cùng quy tắc `dcThemDauCatTuDong`.
//!
//! Mỗi bài đang chọn (group đã dàn chứa từ 2 clip group trở lên thì mỗi clip group là một bài) được
//! thêm 8 nét cắt ở 4 góc: nét ngang và nét dọc dài `length`, cách mép bài `gap`. Nét hướng ra vùng
//! trống; bài nằm sát mép artboard (dưới `edge`) thì nét ở cạnh đó lật vào trong. Nét không đè lên
//! bài khác, không chạy dọc cạnh của bài kế bên, không ra ngoài artboard và không vẽ trùng nét đã
//! có (bấm lại nhiều lần vẫn an toàn). Nét nằm trên layer "Dau cat tu dong" (đưa lên trên cùng):
//! đường hở, không tô, viền K 100 % dày 1 pt, ghi chú `DANCARD_AUTO_CUT_MARK`.

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Point, Rect};

use super::number_param;
use crate::cmd::*;
use crate::{EngineError, Result, Session};

/// Tên layer chứa dấu cắt (giữ đúng tên của DanCard trên Illustrator).
const MARK_LAYER: &str = "Dau cat tu dong";
/// Ghi chú gắn cho mỗi nét (Attributes › Note).
const MARK_NOTE: &str = "DANCARD_AUTO_CUT_MARK";
const MM: f64 = 72.0 / 25.4;
/// Sai số so cạnh (pt).
const EPS: f64 = 0.05;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.cutMarks",
        "DanCard: Dấu cắt tự động",
        [],
        None,
        "{length?: mm (dài nét, mặc định 1,4), gap?: mm (cách bài, mặc định 0), edge?: mm (ngưỡng mép artboard để lật nét vào trong, mặc định 4)} thêm dấu cắt 4 góc cho các bài đang chọn (group đã dàn có từ 2 clip group: mỗi clip group một bài; clip group đo theo mask, bài khác theo viền nhìn thấy) trên layer \"Dau cat tu dong\", một bước hoàn tác; không vẽ trùng nét đã có → {message, drawn, cards, skipped}",
        has_selection,
        cut_marks
    )]
}

/// Số mm từ tham số (nhận "1,4"); `allow_zero`: được bằng 0.
fn mm_param(p: &Value, key: &str, label: &str, default: f64, allow_zero: bool) -> Result<f64> {
    let v = match p.get(key) {
        None | Some(Value::Null) => return Ok(default * MM),
        Some(Value::String(s)) if s.trim().is_empty() && allow_zero => return Ok(0.0),
        Some(_) => number_param(p, key),
    };
    match v {
        Some(n) if n > 0.0 || (allow_zero && n == 0.0) => Ok(n * MM),
        _ => Err(EngineError::Other(format!("{label} {}", if allow_zero { "phải là số từ 0 mm." } else { "phải lớn hơn 0 mm." }))),
    }
}

/// Khung của một bài: clip group theo clipping path, còn lại theo viền nhìn thấy.
fn card_bounds(n: &Node) -> Option<Rect> {
    if let NodeKind::Group { children, clip: true } = &n.kind
        && let Some(b) = children.first().and_then(|m| m.geometric_bounds())
    {
        return Some(b);
    }
    n.visual_bounds().or_else(|| n.geometric_bounds())
}

/// Nét dấu cắt đã vẽ: có ghi chú của DanCard, hoặc (bản cũ chưa gắn ghi chú) một đoạn thẳng
/// ngang/dọc hở, không tô, viền K 100 % dày 1 pt nằm trên layer dấu cắt.
fn is_cut_mark(n: &Node, on_mark_layer: bool) -> bool {
    if n.extras().attrs.as_deref().is_some_and(|a| a.note == MARK_NOTE) {
        return true;
    }
    if !on_mark_layer {
        return false;
    }
    let NodeKind::Path { path, .. } = &n.kind else { return false };
    let a = &n.appearance;
    let k100 = matches!(a.stroke_paint().color(), Some(Color::Cmyk { c, m, y, k }) if c == 0.0 && m == 0.0 && y == 0.0 && (k - 1.0).abs() < 1e-6);
    let [sp] = path.subpaths.as_slice() else { return false };
    let anchors: Vec<_> = path.anchors().map(|(_, _, an)| *an).collect();
    let [p, q] = anchors.as_slice() else { return false };
    let straight = [p, q].iter().all(|an| an.h_in.distance(an.p) < 1e-3 && an.h_out.distance(an.p) < 1e-3);
    let axis = (p.p.x - q.p.x).abs() <= EPS || (p.p.y - q.p.y).abs() <= EPS;
    !sp.closed && a.fill_paint().is_none() && k100 && (a.stroke_width() - 1.0).abs() < 1e-3 && straight && axis
}

/// Bài trong `n`: group thường có từ 2 clip group con trực tiếp thì mỗi clip group là một bài.
fn collect_cards(n: &Node, out: &mut Vec<NodeId>) {
    if is_cut_mark(n, false) {
        return;
    }
    if let NodeKind::Group { children, clip: false } = &n.kind {
        let direct: Vec<NodeId> = children.iter().filter(|c| !is_cut_mark(c, false) && c.clips()).map(|c| c.id).collect();
        if direct.len() >= 2 {
            out.extend(direct);
            return;
        }
    }
    out.push(n.id);
}

/// Artboard đầu tiên chứa tâm của `b`.
fn artboard_for(d: &Document, b: Rect) -> Option<usize> {
    let c = b.center();
    d.artboards.iter().position(|a| c.x >= a.rect.x0 && c.x <= a.rect.x1 && c.y >= a.rect.y0 && c.y <= a.rect.y1)
}

/// Khóa của một nét (đầu mút làm tròn 0,001 pt, không phân biệt chiều) trên artboard `board`.
fn line_key(p: Point, q: Point, board: usize) -> String {
    let (a, b) = (format!("{:.3},{:.3}", p.x, p.y), format!("{:.3},{:.3}", q.x, q.y));
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    format!("{board}|{a}|{b}")
}

struct Card {
    b: Rect,
    board: usize,
}

/// Các nét cần vẽ cho `cards` (bỏ nét đã có trong `drawn`) và số nét trùng đã bỏ.
fn plan_lines(d: &Document, cards: &[Card], length: f64, gap: f64, edge: f64, drawn: &mut HashSet<String>) -> (Vec<(Point, Point)>, usize) {
    let mut lines = vec![];
    let mut duplicates = 0;
    let fits = |p: Point, q: Point, r: Rect| [p, q].iter().all(|v| v.x >= r.x0 && v.x <= r.x1 && v.y >= r.y0 && v.y <= r.y1);
    // Nét đè lên bài khác (hay chạy dọc cạnh bài kế bên) thì bỏ; `allow_owner`: nét lật vào trong bài.
    let overlaps = |p: Point, q: Point, owner: usize, allow_owner: bool| {
        let (min_x, max_x, min_y, max_y) = (p.x.min(q.x), p.x.max(q.x), p.y.min(q.y), p.y.max(q.y));
        let Some(me) = cards.get(owner) else { return true };
        for (i, other) in cards.iter().enumerate() {
            if other.board != me.board || (i == owner && allow_owner) {
                continue;
            }
            let o = other.b;
            if i != owner {
                let horizontal = (p.y - q.y).abs() <= EPS;
                let vertical = (p.x - q.x).abs() <= EPS;
                let on_edge = (horizontal && ((p.y - o.y0).abs() <= EPS || (p.y - o.y1).abs() <= EPS) && max_x.min(o.x1) - min_x.max(o.x0) > EPS)
                    || (vertical && ((p.x - o.x0).abs() <= EPS || (p.x - o.x1).abs() <= EPS) && max_y.min(o.y1) - min_y.max(o.y0) > EPS);
                if on_edge {
                    if !allow_owner {
                        return true;
                    }
                    continue;
                }
            }
            if max_x > o.x0 && min_x < o.x1 && max_y > o.y0 && min_y < o.y1 {
                return true;
            }
        }
        false
    };
    for (i, card) in cards.iter().enumerate() {
        let Some(r) = d.artboards.get(card.board).map(|a| a.rect) else { continue };
        let b = card.b;
        // Hướng nét: ra ngoài, hay vào trong khi bài sát mép artboard (y hướng xuống).
        let left = if b.x0 - r.x0 < edge { 1.0 } else { -1.0 };
        let right = if r.x1 - b.x1 < edge { -1.0 } else { 1.0 };
        let top = if b.y0 - r.y0 < edge { 1.0 } else { -1.0 };
        let bottom = if r.y1 - b.y1 < edge { -1.0 } else { 1.0 };
        let (left_in, right_in, top_in, bottom_in) = (left > 0.0, right < 0.0, top > 0.0, bottom < 0.0);
        let candidates = [
            // Nét ngang ở bốn góc.
            (Point::new(b.x0 + left * gap, b.y0), Point::new(b.x0 + left * (gap + length), b.y0), left_in),
            (Point::new(b.x1 + right * gap, b.y0), Point::new(b.x1 + right * (gap + length), b.y0), right_in),
            (Point::new(b.x0 + left * gap, b.y1), Point::new(b.x0 + left * (gap + length), b.y1), left_in),
            (Point::new(b.x1 + right * gap, b.y1), Point::new(b.x1 + right * (gap + length), b.y1), right_in),
            // Nét dọc ở bốn góc.
            (Point::new(b.x0, b.y0 + top * gap), Point::new(b.x0, b.y0 + top * (gap + length)), top_in),
            (Point::new(b.x1, b.y0 + top * gap), Point::new(b.x1, b.y0 + top * (gap + length)), top_in),
            (Point::new(b.x0, b.y1 + bottom * gap), Point::new(b.x0, b.y1 + bottom * (gap + length)), bottom_in),
            (Point::new(b.x1, b.y1 + bottom * gap), Point::new(b.x1, b.y1 + bottom * (gap + length)), bottom_in),
        ];
        for (p, q, inside) in candidates {
            if !fits(p, q, r) || overlaps(p, q, i, inside) {
                continue;
            }
            if !drawn.insert(line_key(p, q, card.board)) {
                duplicates += 1;
                continue;
            }
            lines.push((p, q));
        }
    }
    (lines, duplicates)
}

/// Các nét dấu cắt đã có trên layer `layer` (khóa của chúng).
fn existing_marks(d: &Document, layer: &Node, out: &mut HashSet<String>) {
    fn walk(d: &Document, n: &Node, out: &mut HashSet<String>) {
        for c in n.children().into_iter().flatten() {
            if is_cut_mark(c, true)
                && let NodeKind::Path { path, .. } = &c.kind
            {
                let pts: Vec<Point> = path.anchors().map(|(_, _, a)| a.p).collect();
                if let ([p, q], Some(board)) = (pts.as_slice(), c.geometric_bounds().and_then(|b| artboard_for(d, b))) {
                    out.insert(line_key(*p, *q, board));
                }
            } else {
                walk(d, c, out);
            }
        }
    }
    walk(d, layer, out);
}

fn cut_marks(s: &mut Session, p: &Value) -> Result<Value> {
    let length = mm_param(p, "length", "Dài nét", 1.4, false)?;
    let edge = mm_param(p, "edge", "Ngưỡng mép", 4.0, false)?;
    let gap = mm_param(p, "gap", "Cách bài", 0.0, true)?;
    let roots = super::roots_in_paint_order(s)?;
    if roots.is_empty() {
        return Err(EngineError::Other("Chọn các bài cần tạo dấu cắt trước.".into()));
    }
    let st = s.doc()?;
    let d = &st.doc;
    let mut ids = vec![];
    for id in &roots {
        if let Some(n) = d.node(*id) {
            collect_cards(n, &mut ids);
        }
    }
    let mut cards = vec![];
    let mut skipped = 0;
    for id in ids {
        match d
            .node(id)
            .and_then(card_bounds)
            .filter(|b| b.width() > 0.0 && b.height() > 0.0)
            .and_then(|b| Some(Card { b, board: artboard_for(d, b)? }))
        {
            Some(c) => cards.push(c),
            None => skipped += 1,
        }
    }
    if cards.is_empty() {
        return Err(EngineError::Other("Không tìm thấy bài hợp lệ nằm trong artboard.".into()));
    }
    let mark_layer = d.layers.iter().find(|l| l.name.as_deref() == Some(MARK_LAYER)).map(|l| l.id);
    let mut drawn = HashSet::new();
    if let Some(layer) = mark_layer.and_then(|id| d.node(id)) {
        existing_marks(d, layer, &mut drawn);
    }
    let (lines, duplicates) = plan_lines(d, &cards, length, gap, edge, &mut drawn);
    let count = cards.len();
    if lines.is_empty() {
        let message = if duplicates > 0 {
            "Dấu cắt phù hợp đã tồn tại; không tạo nét trùng."
        } else {
            "Không có khoảng trống phù hợp để đặt dấu cắt."
        };
        return Ok(json!({"message": format!("OK: {message}"), "drawn": 0, "cards": count, "skipped": skipped}));
    }
    let drawn_count = lines.len();
    s.edit("Dấu cắt tự động (DanCard)", |d, _| {
        let layer = match mark_layer {
            Some(id) => id,
            None => {
                let n = crate::cmd::layer::make_layer(d, Some(MARK_LAYER));
                d.insert(None, usize::MAX, n)?
            }
        };
        let ink = Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0));
        let mut marks = vec![];
        for (p, q) in lines {
            let mut n = Node::path(d.alloc_id(), vectorcraft_geom::shapes::line(p, q), Appearance::basic(Paint::None, ink.clone(), 1.0));
            n.name = Some("Dấu cắt".into());
            n.edit_attrs(|a| a.note = MARK_NOTE.into());
            marks.push(Arc::new(n));
        }
        // Layer dấu cắt mở khóa, hiện, nằm trên cùng.
        let at = d.layers.iter().position(|l| l.id == layer).ok_or(EngineError::NoNode(layer))?;
        let mut l = d.layers.remove(at);
        let ln = Arc::make_mut(&mut l);
        ln.visible = true;
        if let Some(ch) = ln.children_mut() {
            ch.extend(marks);
        }
        d.layers.push(l);
        Ok(())
    })?;
    let mut message = format!("OK: Đã thêm {drawn_count} nét dấu cắt cho {count} bài.");
    if skipped > 0 {
        message.push_str(&format!(" Bỏ qua {skipped} object ngoài artboard."));
    }
    Ok(json!({"message": message, "drawn": drawn_count, "cards": count, "skipped": skipped}))
}
