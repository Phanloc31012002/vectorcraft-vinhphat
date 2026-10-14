//! DanCard — Công cụ bình (IN Vĩnh Phát), chạy native trong engine.
//!
//! Bản chạy thử đầu tiên khi chuyển DanCard từ CEP/ExtendScript của Illustrator sang VectorCraft.
//! Mỗi lệnh là một bước hoàn tác, gọi được từ panel, phím tắt, Actions, cổng điều khiển
//! (`--control`) và `vectorcraft-cli run`.
//!
//! Khác với ExtendScript: engine đọc thẳng cây tài liệu nên
//! - khung của clip group là khung clipping path (đúng phần nhìn thấy), lồng bao nhiêu tầng cũng đúng;
//! - khung transform đang chọn (kể cả khi đã xoay) có sẵn qua `Document::oriented_bounds`; file
//!   .ai/PDF không lưu góc thì `dancard.transform` tự dò góc từ hình dạng phần nhìn thấy.
//!
//! Mọi chữ hiển thị cho người dùng là tiếng Việt có dấu.

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, FillRule, PathEl, Point, Rect, Vec2, shapes};

use super::edit::selected_roots;
use super::*;
use crate::EngineError;

/// Số point trong 1 cm.
const PT_PER_CM: f64 = 72.0 / 2.54;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "dancard.transform",
            "DanCard: Đọc khung transform",
            [],
            None,
            "{ids?: [id…] (mặc định: vùng chọn), unit?: cm|mm|in|pt (mặc định cm)} → {unit, count, selection, objects: [{id, name, kind, …khung}]}; khung = {angle: độ ngược chiều kim đồng hồ, source: transform|detected|page, width, height (đo theo trục khung), center: [x, y], corners: [[x, y]×4] (trên trái, trên phải, dưới phải, dưới trái của khung), artboard: số thứ tự artboard chứa tâm | null, onArtboard?: {center, corners} so với góc trên trái artboard đó}. Đo phần NHÌN THẤY: clip group theo mask (giao với nội dung), group lồng nhau đi hết, bỏ object ẩn/guide/path không tô. Góc lấy từ VectorCraft khi object được xoay trong app (transform), không có thì tự dò từ hình dạng (detected) — dùng cho file .ai/PDF của khách",
            has_doc,
            transform_info
        ),
        cmd!(
            "dancard.rename",
            "DanCard: Đổi tên hàng loạt",
            [],
            None,
            "{prefix?: tiền tố, mode?: num|low|up (số, chữ thường, chữ HOA), start?: số hoặc chữ bắt đầu, pad?: đệm số 0, bottomUp?: bool (dưới cùng đánh số trước), repeat?: số object lặp mỗi nhãn (s1,s1,s2,s2)} đổi tên các object đang chọn, một bước hoàn tác → {message, count, first, last}",
            has_selection,
            rename
        ),
        cmd!(
            "dancard.clipToSize",
            "DanCard: Clip theo kích thước",
            [],
            None,
            "{width, height, unit?: cm|mm|in|pt (mặc định cm), followAngle?: bool (mask xoay theo khung transform của object đặt nghiêng, như dancard.transform đọc; mặc định false)} mỗi object đang chọn được bọc trong clipping mask chữ nhật width × height đặt đúng tâm vùng nhìn thấy thật của nó; object đã là clip group thì giữ nguyên → {message, ids, errors}",
            has_selection,
            clip_to_size
        ),
    ]
}

// ---------------------------------------------------------------- đơn vị và tham số

fn unit_size(unit: &str) -> Option<f64> {
    match unit {
        "cm" => Some(PT_PER_CM),
        "mm" => Some(PT_PER_CM / 10.0),
        "in" => Some(72.0),
        "pt" => Some(1.0),
        _ => None,
    }
}

fn unit_param<'a>(p: &'a Value, cmd: &str) -> Result<(&'a str, f64)> {
    let unit = str_param(p, "unit").filter(|u| !u.is_empty()).unwrap_or("cm");
    let size = unit_size(unit).ok_or_else(|| bad(cmd, format!("Đơn vị không hợp lệ: {unit}")))?;
    Ok((unit, size))
}

/// Số từ tham số: nhận số JSON hoặc chuỗi kiểu "9,2" (dấu phẩy thập phân như panel gửi).
fn number_param(p: &Value, key: &str) -> Option<f64> {
    match p.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        _ => None,
    }
    .filter(|v| v.is_finite())
}

/// `parseInt` của JavaScript: khoảng trắng đầu, dấu, rồi các chữ số liền nhau.
fn js_parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (sign, rest) = match t.as_bytes().first() {
        Some(b'-') => (-1, t.get(1..).unwrap_or("")),
        Some(b'+') => (1, t.get(1..).unwrap_or("")),
        _ => (1, t),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).take(15).collect();
    digits.parse::<i64>().ok().map(|v| sign * v)
}

fn int_param(p: &Value, key: &str) -> Option<i64> {
    match p.get(key)? {
        Value::Number(n) => n.as_f64().filter(|v| v.is_finite()).map(|v| v.trunc().clamp(-1e15, 1e15) as i64),
        Value::String(s) => js_parse_int(s),
        _ => None,
    }
}

// ---------------------------------------------------------------- đo vùng nhìn thấy

/// Path/compound path không tô và không viền thì không nhìn thấy (giống `isVisibleLeaf` của DanCard).
fn painted(n: &Node) -> bool {
    !(n.appearance.fill_paint().is_none() && n.appearance.stroke_paint().is_none())
}

/// Vùng nhìn thấy thật của một object, cùng quy tắc `visibleUnion` của Dàn Card:
/// clip group lấy giao của nội dung với mask (nội dung rỗng thì lấy mask), group thường lấy hợp
/// các con; object ẩn, guide và path không tô bị bỏ qua.
fn seen_bounds(n: &Node) -> Option<Rect> {
    if !n.visible {
        return None;
    }
    let union = |nodes: &[std::sync::Arc<Node>]| nodes.iter().fold(None, |acc: Option<Rect>, c| match (acc, seen_bounds(c)) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    });
    match &n.kind {
        NodeKind::Group { children, clip: true } | NodeKind::Layer { children, clip: true, .. } => {
            let mask = children.first().and_then(|m| m.geometric_bounds());
            let inner = children.get(1..).and_then(union);
            match (inner, mask) {
                (Some(i), Some(m)) => {
                    let hit = i.intersect(m);
                    Some(if hit.width() > 0.0 && hit.height() > 0.0 { hit } else { m })
                }
                (i, m) => i.or(m),
            }
        }
        NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } => union(children),
        NodeKind::Path { guide: true, .. } => None,
        NodeKind::Path { .. } | NodeKind::Compound { .. } if !painted(n) => None,
        _ => n.visual_bounds().or_else(|| n.geometric_bounds()),
    }
}

// ---------------------------------------------------------------- khung transform

/// Khung transform của object hoặc vùng chọn: hình chữ nhật (có thể xoay) ôm phần nhìn thấy.
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// Góc của khung, độ, ngược chiều kim đồng hồ (như `bbox_angle`), trong (-45°, 45°] khi dò.
    angle: f64,
    /// Rộng × cao đo theo trục của khung (pt).
    width: f64,
    height: f64,
    /// Tâm khung trên trang (pt, y hướng xuống, gốc là góc trên trái artboard đầu tiên).
    center: Point,
    /// "transform": góc VectorCraft lưu khi xoay trong app; "detected": dò từ hình dạng (file
    /// .ai/PDF không lưu góc); "page": khung thẳng.
    source: &'static str,
}

impl Frame {
    fn upright(r: Rect, source: &'static str) -> Self {
        Frame { angle: 0.0, width: r.width(), height: r.height(), center: r.center(), source }
    }
    /// Hệ trục của khung → trang (cùng quy ước với `OrientedBox::to_doc`).
    fn to_doc(&self) -> Affine {
        Affine::rotate(-self.angle.to_radians())
    }
    /// Bốn góc trên trang: trên trái, trên phải, dưới phải, dưới trái của chính khung.
    fn corners(&self) -> [Point; 4] {
        let a = self.to_doc();
        let c = a.inverse() * self.center;
        let (hw, hh) = (self.width / 2.0, self.height / 2.0);
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(dx, dy)| a * Point::new(c.x + dx, c.y + dy))
    }
}

/// Sai số khi làm phẳng đường cong (pt).
const FLATTEN_TOL: f64 = 0.05;
/// Phần chu vi khung phải nằm trên cạnh thẳng thật của hình để nhận góc dò được: card, tem, khung
/// clip đặt nghiêng có 4 cạnh thẳng trùng khung (bo góc vẫn trên 80%); một dòng chữ hay nhóm object
/// rời rạc chỉ chạm khung ở vài điểm nên giữ khung thẳng.
const MIN_EDGE_COVERAGE: f64 = 0.8;
/// Không giữ nhiều hơn chừng này điểm (ảnh rất phức tạp vẫn đo được nhanh).
const MAX_POINTS: usize = 400_000;

fn push_path(path: &vectorcraft_geom::PathData, out: &mut Vec<Point>) {
    vectorcraft_geom::kurbo::flatten(path.to_bezpath(), FLATTEN_TOL, |el| {
        if out.len() >= MAX_POINTS {
            return;
        }
        match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => out.push(p),
            _ => {}
        }
    });
}

fn push_rect(r: Rect, out: &mut Vec<Point>) {
    out.extend([Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)]);
}

/// Đường viền của một clipping path (không tô cũng tính).
fn mask_points(m: &Node, out: &mut Vec<Point>) {
    match &m.kind {
        NodeKind::Path { path, .. } => push_path(path, out),
        NodeKind::Compound { children, .. } => {
            for c in children {
                if let NodeKind::Path { path, .. } = &c.kind {
                    push_path(path, out);
                }
            }
        }
        _ => {
            if let Some(b) = m.geometric_bounds() {
                push_rect(b, out);
            }
        }
    }
}

/// Các điểm đường viền của phần NHÌN THẤY: clip group lấy giao (bao lồi) của nội dung với mask,
/// group lồng bao nhiêu tầng cũng đi hết; object ẩn, guide, path không tô bị bỏ qua.
fn visible_points(n: &Node, out: &mut Vec<Point>) {
    if !n.visible {
        return;
    }
    match &n.kind {
        NodeKind::Group { children, clip: true } | NodeKind::Layer { children, clip: true, .. } => {
            let Some(m) = children.first() else { return };
            let mut mask = Vec::new();
            mask_points(m, &mut mask);
            let mut inner = Vec::new();
            for c in children.iter().skip(1) {
                visible_points(c, &mut inner);
            }
            let mask = convex_hull(mask);
            let inner = convex_hull(inner);
            let hit = clip_convex(&inner, &mask);
            out.extend(if hit.len() >= 3 { hit } else { mask });
        }
        NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } => {
            for c in children {
                visible_points(c, out);
            }
        }
        NodeKind::Path { guide: true, .. } => {}
        NodeKind::Path { path, .. } => {
            if painted(n) {
                push_path(path, out);
            }
        }
        NodeKind::Compound { children, .. } => {
            if painted(n) {
                for c in children {
                    if let NodeKind::Path { path, .. } = &c.kind {
                        push_path(path, out);
                    }
                }
            }
        }
        NodeKind::Image(im) => {
            let r = Rect::new(0.0, 0.0, f64::from(im.width), f64::from(im.height));
            out.extend([Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)].map(|p| im.xf * p));
        }
        _ => {
            if let Some(b) = n.geometric_bounds() {
                push_rect(b, out);
            }
        }
    }
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

/// Bao lồi (monotone chain), các đỉnh theo một chiều, không lặp điểm đầu.
fn convex_hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.retain(|p| p.x.is_finite() && p.y.is_finite());
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
    if pts.len() < 3 {
        return pts;
    }
    let mut lower: Vec<Point> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Point> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Giao của hai đa giác lồi (Sutherland–Hodgman, `clip` là đa giác cắt).
fn clip_convex(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    if subject.len() < 3 || clip.len() < 3 {
        return Vec::new();
    }
    // Chiều của đa giác cắt: điểm "bên trong" có cùng dấu với diện tích.
    let area: f64 = clip.iter().zip(clip.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let mut out: Vec<Point> = subject.to_vec();
    for (i, &a) in clip.iter().enumerate() {
        let Some(&b) = clip.get((i + 1) % clip.len()) else { break };
        let input = std::mem::take(&mut out);
        let inside = |p: Point| sign * cross(a, b, p) >= -1e-9;
        let hit = |p: Point, q: Point| {
            let (d1, d2) = (cross(a, b, p), cross(a, b, q));
            let t = if (d1 - d2).abs() < 1e-12 { 0.0 } else { d1 / (d1 - d2) };
            p.lerp(q, t)
        };
        for (j, &q) in input.iter().enumerate() {
            let Some(&p) = input.get((j + input.len() - 1) % input.len()) else { continue };
            match (inside(p), inside(q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(hit(p, q)),
                (false, true) => {
                    out.push(hit(p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// Khung của các điểm `hull` khi trục x của khung đứng ở góc trang `phi` (radian, y hướng xuống).
fn frame_at(hull: &[Point], phi: f64) -> (f64, f64, f64, Point) {
    let (u, v, [u0, u1, v0, v1]) = extents(hull, phi);
    let (w, h) = (u1 - u0, v1 - v0);
    let c = (u * ((u0 + u1) / 2.0) + v * ((v0 + v1) / 2.0)).to_point();
    (w * h, w, h, c)
}

/// Trục của khung ở góc `phi` và khoảng [u0, u1] × [v0, v1] các điểm chiếm trên hai trục đó.
fn extents(hull: &[Point], phi: f64) -> (Vec2, Vec2, [f64; 4]) {
    let (u, v) = (Vec2::new(phi.cos(), phi.sin()), Vec2::new(-phi.sin(), phi.cos()));
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in hull {
        let (pu, pv) = (p.to_vec2().dot(u), p.to_vec2().dot(v));
        (u0, u1, v0, v1) = (u0.min(pu), u1.max(pu), v0.min(pv), v1.max(pv));
    }
    (u, v, [u0, u1, v0, v1])
}

/// Phần chu vi khung (ở góc `phi`) mà các cạnh của bao lồi nằm sát lên.
fn edge_coverage(hull: &[Point], phi: f64) -> f64 {
    let (u, v, [u0, u1, v0, v1]) = extents(hull, phi);
    let (w, h) = (u1 - u0, v1 - v0);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let tol = (w.min(h) * 0.002).max(0.25);
    let local = |p: Point| (p.to_vec2().dot(u), p.to_vec2().dot(v));
    let mut on_sides = 0.0;
    for (i, a) in hull.iter().enumerate() {
        let Some(b) = hull.get((i + 1) % hull.len()) else { break };
        let ((au, av), (bu, bv)) = (local(*a), local(*b));
        let flush = |x: f64, y: f64, side: f64| (x - side).abs() <= tol && (y - side).abs() <= tol;
        if flush(au, bu, u0) || flush(au, bu, u1) || flush(av, bv, v0) || flush(av, bv, v1) {
            on_sides += a.distance(*b);
        }
    }
    on_sides / (2.0 * (w + h))
}

/// Khung chữ nhật nhỏ nhất ôm các điểm (rotating calipers trên bao lồi). Chỉ nhận góc xoay khi
/// hình gần chữ nhật; còn lại trả khung thẳng.
fn detect_frame(points: Vec<Point>) -> Option<Frame> {
    let hull = convex_hull(points);
    let first = *hull.first()?;
    let page = hull.iter().fold(Rect::from_points(first, first), |r, p| r.union_pt(*p));
    if hull.len() < 3 {
        return Some(Frame::upright(page, "page"));
    }
    let mut best: Option<(f64, f64, f64, f64, Point)> = None;
    for (i, a) in hull.iter().enumerate() {
        let Some(b) = hull.get((i + 1) % hull.len()) else { break };
        let e = *b - *a;
        if e.hypot() < 1e-9 {
            continue;
        }
        let phi = e.y.atan2(e.x);
        let (area, w, h, c) = frame_at(&hull, phi);
        if best.is_none_or(|bst| area < bst.0) {
            best = Some((area, phi, w, h, c));
        }
    }
    let Some((area, phi, mut w, mut h, c)) = best else { return Some(Frame::upright(page, "page")) };
    let page_area = page.width() * page.height();
    let rectangular = area > 0.0 && edge_coverage(&hull, phi) >= MIN_EDGE_COVERAGE;
    // Khung thẳng gần như nhỏ bằng khung xoay: coi là không xoay.
    if !rectangular || area >= page_area * (1.0 - 1e-4) {
        return Some(Frame::upright(page, "page"));
    }
    // Góc khung (ngược chiều kim đồng hồ) đưa về (-45°, 45°], đổi rộng/cao theo.
    let mut angle = -phi.to_degrees();
    while angle > 45.0 {
        angle -= 90.0;
        std::mem::swap(&mut w, &mut h);
    }
    while angle <= -45.0 {
        angle += 90.0;
        std::mem::swap(&mut w, &mut h);
    }
    if angle.abs() < 1e-3 {
        return Some(Frame::upright(page, "page"));
    }
    Some(Frame { angle, width: w, height: h, center: c, source: "detected" })
}

/// Khung transform của `ids`: góc VectorCraft lưu (xoay trong app) nếu chúng cùng góc, không thì
/// dò từ hình dạng phần nhìn thấy.
fn frame_of(d: &Document, ids: &[NodeId]) -> Option<Frame> {
    if d.bbox_angle(ids) != 0.0 {
        let b = d.oriented_bounds(ids, false)?;
        return Some(Frame { angle: b.angle, width: b.rect.width(), height: b.rect.height(), center: b.center(), source: "transform" });
    }
    let mut pts = Vec::new();
    for id in ids {
        if let Some(n) = d.node(*id) {
            visible_points(n, &mut pts);
        }
    }
    if pts.is_empty() {
        return d.bounds_of(ids, false).map(|r| Frame::upright(r, "page"));
    }
    detect_frame(pts)
}

/// Khung của cả vùng chọn: các object cùng một góc thì khung xoay theo góc đó (ôm hết), khác góc
/// thì khung thẳng (như bảng Transform).
fn selection_frame(each: &[Frame]) -> Option<Frame> {
    let first = each.first()?;
    if each.len() == 1 {
        return Some(*first);
    }
    let same = each.iter().all(|f| (f.angle - first.angle).abs() < 0.05);
    let mut pts = Vec::new();
    for f in each {
        pts.extend(f.corners());
    }
    let hull = convex_hull(pts);
    let p0 = *hull.first()?;
    if !same || first.angle == 0.0 {
        let page = hull.iter().fold(Rect::from_points(p0, p0), |r, p| r.union_pt(*p));
        return Some(Frame::upright(page, "page"));
    }
    let phi = -first.angle.to_radians();
    let (_, w, h, c) = frame_at(&hull, phi);
    Some(Frame { angle: first.angle, width: w, height: h, center: c, source: first.source })
}

fn r4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

fn frame_json(d: &Document, f: &Frame, k: f64) -> Value {
    let pt = |p: Point| json!([r4(p.x / k), r4(p.y / k)]);
    let board = d.artboards.iter().position(|a| a.rect.contains(f.center));
    let mut v = json!({
        "angle": r4(f.angle),
        "source": f.source,
        "width": r4(f.width / k),
        "height": r4(f.height / k),
        "center": pt(f.center),
        "corners": f.corners().map(pt),
        "artboard": board,
    });
    if let Some(a) = board.and_then(|i| d.artboards.get(i)) {
        let o = a.rect.origin().to_vec2();
        v["onArtboard"] = json!({"center": pt(f.center - o), "corners": f.corners().map(|p| pt(p - o))});
    }
    v
}

// ---------------------------------------------------------------- dancard.transform

fn transform_info(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.transform";
    let (unit, k) = unit_param(p, C)?;
    let ids = if p.get("ids").is_some() { checked_ids_param(s, p, "ids", C)? } else { roots_in_paint_order(s)? };
    let d = &s.doc()?.doc;
    let mut frames = Vec::new();
    let mut objects = Vec::new();
    for id in &ids {
        let Some(n) = d.node(*id) else { continue };
        let Some(f) = frame_of(d, &[*id]) else { continue };
        let mut v = frame_json(d, &f, k);
        v["id"] = json!(id.0);
        v["name"] = json!(n.display_name());
        v["kind"] = json!(n.kind_label());
        objects.push(v);
        frames.push(f);
    }
    let selection = selection_frame(&frames).map_or(Value::Null, |f| frame_json(d, &f, k));
    Ok(json!({
        "unit": unit,
        "count": objects.len(),
        "selection": selection,
        "objects": objects,
    }))
}

/// Các object gốc đang chọn, từ dưới lên trên (thứ tự vẽ).
fn roots_in_paint_order(s: &Session) -> Result<Vec<NodeId>> {
    selected_roots(s)
}

// ---------------------------------------------------------------- dancard.rename

/// 1 → a, 2 → b … 27 → aa (`upper`: chữ HOA).
fn to_alpha(mut num: i64, upper: bool) -> String {
    let base = if upper { b'A' } else { b'a' };
    let mut out = Vec::new();
    while num > 0 {
        let r = ((num - 1) % 26) as u8;
        out.push((base + r) as char);
        num = (num - 1) / 26;
    }
    out.iter().rev().collect()
}

/// a → 1, b → 2 …; ký tự ngoài a–z cho 1 (như `fromAlpha` của DanCard).
fn from_alpha(s: &str) -> i64 {
    let mut num: i64 = 0;
    for c in s.to_lowercase().chars() {
        if !c.is_ascii_lowercase() {
            return 1;
        }
        num = num.saturating_mul(26).saturating_add(c as i64 - 'a' as i64 + 1);
    }
    num
}

/// Nhãn thứ `i` theo đúng quy tắc `dcRename`.
struct Labeler {
    prefix: String,
    mode: String,
    start: i64,
    pad: usize,
}

impl Labeler {
    fn label(&self, i: i64) -> String {
        let v = self.start.saturating_add(i);
        if self.mode == "num" {
            let n = v.to_string();
            let padded = if self.pad > 0 && n.len() < self.pad { format!("{}{n}", "0".repeat(self.pad - n.len())) } else { n };
            format!("{}{padded}", self.prefix)
        } else {
            format!("{}{}", self.prefix, to_alpha(v, self.mode == "up"))
        }
    }
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.rename";
    let prefix = str_param(p, "prefix").unwrap_or("").to_string();
    let mode = str_param(p, "mode").filter(|m| !m.is_empty()).unwrap_or("num").to_string();
    if !matches!(mode.as_str(), "num" | "low" | "up") {
        return Err(bad(C, format!("Kiểu đánh số không hợp lệ: {mode} (num, low hoặc up)")));
    }
    let start_text = match p.get("start") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(t)) => t.trim().to_string(),
        _ => String::new(),
    };
    let start = if mode == "num" {
        js_parse_int(&start_text).unwrap_or(1)
    } else if !start_text.is_empty() && start_text.chars().all(|c| c.is_ascii_alphabetic()) {
        from_alpha(&start_text)
    } else {
        js_parse_int(&start_text).unwrap_or(1)
    };
    let pad = int_param(p, "pad").unwrap_or(0).clamp(0, 32) as usize;
    let repeat = int_param(p, "repeat").filter(|r| *r >= 1).unwrap_or(1);
    let bottom_up = bool_or(p, "bottomUp", false);
    let labels = Labeler { prefix, mode, start, pad };

    // Thứ tự vẽ là dưới lên trên; mặc định đánh từ object trên cùng xuống.
    let mut ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Chưa chọn object nào.".into()));
    }
    if !bottom_up {
        ids.reverse();
    }
    let count = ids.len();
    s.edit("Đổi tên (DanCard)", |d, _| {
        for (k, id) in ids.iter().enumerate() {
            let name = labels.label(k as i64 / repeat);
            d.node_mut(*id).ok_or(EngineError::NoNode(*id))?.name = Some(name);
        }
        Ok(())
    })?;
    let first = labels.label(0);
    let last = labels.label((count as i64 - 1) / repeat);
    Ok(json!({
        "message": format!("OK: Đã đổi tên {count} object ({first} ... {last})."),
        "count": count,
        "first": first,
        "last": last,
    }))
}

// ---------------------------------------------------------------- dancard.clipToSize

/// Bọc `id` trong clip group: mask chữ nhật `w` × `h` đặt tâm tại tâm vùng nhìn thấy thật.
/// `follow_angle`: mask xoay theo khung transform của object (object đặt nghiêng vẫn cắt đúng
/// theo cạnh của nó), tâm là tâm khung transform.
fn clip_one(d: &mut Document, id: NodeId, w: f64, h: f64, follow_angle: bool) -> std::result::Result<NodeId, String> {
    let n = d.node(id).ok_or("không còn trong tài liệu")?;
    if matches!(n.kind, NodeKind::Group { clip: true, .. }) {
        // Clip group đã có thì giữ nguyên khung đã chuẩn (như Dàn Card).
        return Ok(id);
    }
    if matches!(n.kind, NodeKind::Layer { .. }) {
        return Err("không clip được cả layer".into());
    }
    let oriented = if follow_angle { frame_of(d, &[id]).filter(|f| f.angle != 0.0) } else { None };
    let (path, angle) = match oriented {
        Some(f) => {
            // Dựng mask trong hệ trục của khung đã xoay rồi đưa ra trang.
            let c = f.to_doc().inverse() * f.center;
            let local = Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0);
            (shapes::rectangle(local).transformed(f.to_doc()), f.angle)
        }
        None => {
            let b = seen_bounds(n)
                .or_else(|| n.visual_bounds())
                .filter(|b| b.width() > 0.0 && b.height() > 0.0)
                .ok_or("không đo được vùng hiển thị của object")?;
            let c = b.center();
            (shapes::rectangle(Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0)), 0.0)
        }
    };
    let (parent, index, _) = d.position(id).ok_or("không tìm thấy vị trí object")?;
    let mask_id = d.alloc_id();
    let mut mask = Node::new(mask_id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: true, guide: false });
    mask.appearance = Appearance::basic(Paint::None, Paint::None, 0.0).into();
    mask.name = Some("Khung clip".into());
    mask.bbox_angle = angle;
    // Mask nằm ngay trên object, rồi bọc cả hai: clipping path là con đầu tiên của clip group.
    d.insert(parent, index + 1, mask).map_err(|e| e.to_string())?;
    let gid = d.alloc_id();
    d.wrap(&[id, mask_id], |mut children| {
        children.rotate_right(1);
        let mut g = Node::new(gid, NodeKind::Group { children, clip: true });
        // Khung transform của clip group đứng cùng góc với mask.
        g.bbox_angle = angle;
        g
    })
    .map_err(|e| e.to_string())
}

fn clip_to_size(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.clipToSize";
    let (unit, k) = unit_param(p, C)?;
    let (Some(w), Some(h)) = (number_param(p, "width"), number_param(p, "height")) else {
        return Err(bad(C, "Thiếu kích thước Rộng x Cao."));
    };
    if w <= 0.0 || h <= 0.0 {
        return Err(bad(C, "Kích thước Rộng x Cao phải lớn hơn 0."));
    }
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Hãy chọn ít nhất 1 object để clip.".into()));
    }
    let (wp, hp) = (w * k, h * k);
    let follow_angle = bool_or(p, "followAngle", false);
    let total = ids.len();
    let (done, errors) = s.edit("Clip theo kích thước (DanCard)", |d, sel| {
        let mut done = Vec::new();
        let mut errors = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            match clip_one(d, *id, wp, hp, follow_angle) {
                Ok(g) => done.push(g),
                Err(e) => errors.push(format!("Object {}: {e}", i + 1)),
            }
        }
        if done.is_empty() {
            return Err(EngineError::Other(format!("ERR: Không clip được object nào.\n{}", errors.join("\n"))));
        }
        sel.set(done.iter().copied());
        Ok((done, errors))
    })?;
    let fmt = |v: f64| {
        let t = format!("{v:.3}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut message = format!("OK: Đã clip {}/{total} object thành {} x {} {unit}.", done.len(), fmt(w), fmt(h));
    if !errors.is_empty() {
        message = format!("{message}\n{}", errors.join("\n"));
    }
    Ok(json!({
        "message": message,
        "ids": done.iter().map(|g| g.0).collect::<Vec<_>>(),
        "errors": errors,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nhan_giong_dc_rename() {
        assert_eq!(to_alpha(1, false), "a");
        assert_eq!(to_alpha(26, true), "Z");
        assert_eq!(to_alpha(27, false), "aa");
        assert_eq!(from_alpha("aa"), 27);
        assert_eq!(from_alpha("a1"), 1);
        let l = Labeler { prefix: "s".into(), mode: "num".into(), start: 1, pad: 3 };
        assert_eq!(l.label(0), "s001");
        assert_eq!(l.label(1234), "s1235");
        assert_eq!(js_parse_int(" 12abc"), Some(12));
        assert_eq!(js_parse_int("abc"), None);
    }
}
