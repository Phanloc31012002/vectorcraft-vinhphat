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

#[path = "dancard_autosave.rs"]
mod autosave;
#[path = "dancard_be.rs"]
mod be;
#[path = "dancard_catalogue.rs"]
mod catalogue;
#[path = "dancard_ctl.rs"]
mod ctl;
#[path = "dancard_cutmarks.rs"]
mod cutmarks;
#[path = "dancard_kts.rs"]
mod kts;
#[path = "dancard_layout.rs"]
mod layout;
#[path = "dancard_mau.rs"]
mod mau;
#[path = "dancard_raster.rs"]
mod raster;
#[path = "dancard_regroup.rs"]
mod regroup;
#[path = "dancard_tutro.rs"]
mod tutro;
#[path = "dancard_varexport.rs"]
mod varexport;
#[path = "dancard_variable.rs"]
mod variable;
pub(crate) use raster::{Fit, image_node, render_raster};

/// Số point trong 1 cm.
const PT_PER_CM: f64 = 72.0 / 2.54;

// ---------------------------------------------------------------- mặc định của bản IN Vĩnh Phát

/// Phiên bản bộ mặc định DanCard (tăng khi thêm mặc định mới; [`Prefs::apply_dancard_defaults`]).
pub const DANCARD_DEFAULTS: u32 = 1;

/// Cho serde: tùy chọn chưa từng nhận mặc định DanCard thì không ghi trường này.
pub fn dancard_defaults_unset(v: &u32) -> bool {
    *v == 0
}

/// Mặc định DanCard: (khóa tùy chọn, giá trị). Nét và chữ giữ point như Illustrator.
fn dancard_values() -> [(&'static str, Value); 4] {
    [
        ("unitsGeneral", json!("centimeters")),
        ("forceDocumentUnits", json!(true)),
        ("ctrlDirectSelection", json!(true)),
        ("selectionToolAnchors", json!(false)),
    ]
}

impl crate::Prefs {
    /// Áp mặc định của bản IN Vĩnh Phát: đơn vị cm cho mọi file mở ra, giữ Ctrl là Direct
    /// Selection, Selection tool không hiện điểm neo. `category`: chỉ các tùy chọn của mục đó (như
    /// Reset của mục), `None`: tất cả.
    pub fn apply_dancard_defaults(&mut self, category: Option<&str>) {
        let mut obj = self.to_json();
        for (key, value) in dancard_values() {
            let in_category = category.is_none_or(|c| prefscmds::spec(key).is_some_and(|sp| sp.category.eq_ignore_ascii_case(c)));
            if in_category {
                obj[key] = value;
            }
        }
        obj["dancardDefaults"] = json!(DANCARD_DEFAULTS);
        if let Ok(p) = serde_json::from_value(obj) {
            *self = p;
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    let mut v = raster::specs();
    v.extend(cutmarks::specs());
    v.extend(layout::specs());
    v.extend(autosave::specs());
    v.extend(catalogue::specs());
    v.extend(kts::specs());
    v.extend(tutro::specs());
    v.extend(mau::specs());
    v.extend(ctl::specs());
    v.extend(be::specs());
    v.extend(variable::specs());
    v.extend(varexport::specs());
    v.extend([
        cmd!(
            query "dancard.transform",
            "DanCard: Đọc khung transform",
            [],
            None,
            "{ids?: [id…] (mặc định: vùng chọn), unit?: cm|mm|in|pt (mặc định cm), bounds?: preview|geometric (mặc định theo Use Preview Bounds, như bảng Transform)} → {unit, bounds, count, selection, objects: [{id, name, kind, …khung}]}; khung = {angle: độ ngược chiều kim đồng hồ, source: transform|page|detected, width, height (đo theo trục khung), center: [x, y] trên trang, rulerCenter: [x, y] theo thước (bằng X/Y của bảng Transform khi chọn điểm tâm), corners: [[x, y]×4], artboard: số thứ tự artboard chứa tâm | null, onArtboard?, transformBox? (khi detected: rộng, cao, tâm bảng Transform hiện)}. Khung đúng như bảng Transform đo: clip group theo mask, group lồng nhau đi hết. Riêng khi bảng Transform cho khung thẳng mà hình thật là chữ nhật đặt nghiêng (file .ai/PDF không lưu góc) thì trả khung dò được (detected)",
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
            "{width, height, unit?: cm|mm|in|pt (mặc định cm), followAngle?: bool (mask nghiêng theo khung transform của object, như dancard.transform đọc: card xoay hay đặt nghiêng vẫn clip đúng trục, width × height là ngang × dọc như đang thấy trên trang; mặc định true, false: khung thẳng của trang)} như nút Clip của DanCard: mỗi object đang chọn được bọc trong clipping mask chữ nhật width × height đặt đúng tâm phần nhìn thấy thật của nó (bỏ object ẩn, guide, path không tô), object đã là clip group thì giữ nguyên khung, rồi Expand Appearance; một bước hoàn tác → {message, ids, errors}",
            has_selection,
            clip_to_size
        ),
        cmd!(
            "dancard.resize",
            "DanCard: Resize theo kích thước",
            [],
            None,
            "{width, height, unit?: cm|mm|in|pt (mặc định cm), strokes?: bool (co giãn nét, mặc định theo Scale Strokes & Effects)} mỗi object đang chọn được co giãn thành đúng width × height đo theo khung transform của nó (như bảng Transform: clip group theo mask, card đặt nghiêng theo trục của card; width × height là ngang × dọc như đang thấy trên trang, object đã xoay 90° vẫn nhập theo hướng đang thấy), giữ nguyên tâm, một bước hoàn tác → {message, ids, errors}",
            has_selection,
            resize
        ),
    ]);
    v
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

/// Cạnh lớn nhất nhận cho Rộng × Cao (pt): 10 m, đủ mọi khổ in; số lớn hơn là gõ nhầm và sẽ làm
/// phẳng đường cong thành hàng triệu đoạn.
const MAX_SIDE_PT: f64 = 10_000.0 * 72.0 / 25.4;

/// Rộng × Cao (pt) từ tham số `width` / `height` theo đơn vị `k`, đã kiểm tra.
fn size_param(p: &Value, cmd: &str, k: f64) -> Result<(f64, f64)> {
    let (Some(w), Some(h)) = (number_param(p, "width"), number_param(p, "height")) else {
        return Err(bad(cmd, "Thiếu kích thước Rộng x Cao."));
    };
    if w <= 0.0 || h <= 0.0 {
        return Err(bad(cmd, "Kích thước Rộng x Cao phải lớn hơn 0."));
    }
    let (wp, hp) = (w * k, h * k);
    if !(wp <= MAX_SIDE_PT && hp <= MAX_SIDE_PT) {
        return Err(bad(cmd, "Kích thước Rộng x Cao tối đa 1000 cm."));
    }
    Ok((wp, hp))
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
    let union = |nodes: &[std::sync::Arc<Node>]| {
        nodes.iter().fold(None, |acc: Option<Rect>, c| match (acc, seen_bounds(c)) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        })
    };
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
    fn to_doc(self) -> Affine {
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

/// Các điểm đường viền đúng như khung transform đo (`Node::geometric_bounds`): clip group theo
/// clipping path, group lồng bao nhiêu tầng cũng đi hết, layer bỏ con đang ẩn.
fn outline_points(n: &Node, out: &mut Vec<Point>) {
    match &n.kind {
        NodeKind::Group { children, clip: true } | NodeKind::Layer { children, clip: true, .. } => {
            if let Some(m) = children.first() {
                mask_points(m, out);
            }
        }
        NodeKind::Layer { children, .. } => {
            for c in children.iter().filter(|c| c.visible) {
                outline_points(c, out);
            }
        }
        NodeKind::Group { children, .. } | NodeKind::Compound { children, .. } => {
            for c in children {
                outline_points(c, out);
            }
        }
        NodeKind::Path { path, .. } => push_path(path, out),
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

impl Frame {
    /// Khung như bảng Transform hiện: `source` "transform" khi nó đứng nghiêng, "page" khi thẳng.
    fn from_box(b: vectorcraft_doc::OrientedBox) -> Self {
        let source = if b.angle != 0.0 { "transform" } else { "page" };
        Frame { angle: b.angle, width: b.rect.width(), height: b.rect.height(), center: b.center(), source }
    }
}

/// Khung transform của `ids`, đúng như bảng Transform đo (`visual`: Use Preview Bounds). Chỉ khi
/// bảng Transform cho khung thẳng mà hình thật là chữ nhật đặt nghiêng (file .ai/PDF không lưu
/// góc) thì dùng khung dò được.
fn frame_of(d: &Document, ids: &[NodeId], visual: bool) -> Option<Frame> {
    let boxed = Frame::from_box(d.oriented_bounds(ids, visual)?);
    if boxed.angle != 0.0 {
        return Some(boxed);
    }
    let mut pts = Vec::new();
    for id in ids {
        if let Some(n) = d.node(*id) {
            outline_points(n, &mut pts);
        }
    }
    Some(detect_frame(pts).filter(|f| f.source == "detected").unwrap_or(boxed))
}

// ---------------------------------------------------------------- dựng thẳng theo khung transform

/// Xoay quanh tâm: nét và hoa văn đi theo bài.
fn upright_scaling() -> vectorcraft_doc::Scaling {
    vectorcraft_doc::Scaling { strokes: true, patterns: true, ..vectorcraft_doc::Scaling::default() }
}

/// Phần nghiêng lẻ của góc `angle` (độ), đưa về (-45°, 45°]. Dựng thẳng chỉ bỏ phần này: các lần
/// xoay 90° / 180° được giữ nguyên như người dùng đang thấy trên trang (DanCard đo theo khung của
/// trang), và góc VectorCraft lưu khi xoay trong app hay góc dò từ file .ai / PDF cho cùng kết quả.
pub(super) fn tilt_of(angle: f64) -> f64 {
    let mut a = angle - 90.0 * (angle / 90.0).round();
    if a <= -45.0 {
        a += 90.0;
    }
    if a > 45.0 {
        a -= 90.0;
    }
    a
}

/// Phép xoay quanh tâm khung đưa `id` về thẳng theo khung transform của nó (góc VectorCraft lưu khi
/// xoay trong app, hoặc góc dò được của hình chữ nhật đặt nghiêng trong file .ai/PDF), chỉ bỏ phần
/// nghiêng lẻ ([`tilt_of`]): (phép xoay, góc nghiêng đã bỏ, góc này dò từ hình dạng). None khi
/// khung đã thẳng, kể cả khi xoay đúng 90° / 180°.
///
/// DanCard trên Illustrator không đọc được góc nên đo bằng khung thẳng của trang và raster để sửa
/// khổ; ở đây các lệnh dàn dựng thẳng bài theo khung transform rồi mới đo, giữ nguyên vector.
pub(super) fn upright_affine(d: &Document, id: NodeId) -> Option<(Affine, f64, bool)> {
    let f = frame_of(d, &[id], false)?;
    let tilt = tilt_of(f.angle);
    let ok = tilt.is_finite() && tilt.abs() > 1e-9 && f.center.x.is_finite() && f.center.y.is_finite();
    ok.then(|| (Affine::rotate_about(tilt.to_radians(), f.center), tilt, f.source == "detected"))
}

/// Góc khung transform (`bbox_angle`) của `n` và mọi object con, theo thứ tự duyệt trước.
fn collect_angles(n: &Node, out: &mut Vec<f64>) {
    out.push(n.bbox_angle);
    for c in n.children().into_iter().flatten() {
        collect_angles(c, out);
    }
}

/// Góc khung `bbox_angle` chỉ lệch bội 90° một sai số tính toán thì đặt đúng bội đó, trong
/// (-180°, 180°], không để -0°: lần đo sau thấy đúng là đã thẳng.
fn snap_quarter(n: &mut Node) {
    let q = (n.bbox_angle / 90.0).round() * 90.0;
    if (n.bbox_angle - q).abs() < 1e-6 {
        let r = q.rem_euclid(360.0);
        let r = if r > 180.0 { r - 360.0 } else { r };
        n.bbox_angle = if r == 0.0 { 0.0 } else { r };
    }
}

/// Góc khung của `n` và mọi object con sau khi dựng thẳng, theo `before` của [`collect_angles`].
fn settle_angles(n: &mut Node, before: &mut std::vec::IntoIter<f64>, detected: bool) {
    match before.next() {
        // Chưa lưu góc (file .ai/PDF): nội dung giờ đã thẳng, khung thẳng như cũ.
        Some(a) if detected && a == 0.0 => n.bbox_angle = 0.0,
        _ => snap_quarter(n),
    }
    if let Some(children) = n.children_mut() {
        for c in children.iter_mut() {
            settle_angles(std::sync::Arc::make_mut(c), before, detected);
        }
    }
}

/// Dựng thẳng `n` bằng phép xoay `m` của [`upright_affine`]. Khung transform của `n` và mọi object
/// con đi theo phép xoay như mọi lần xoay khác (khung còn lại đúng bội 90°). Riêng khi góc dò từ
/// hình dạng (`detected`: file .ai/PDF không lưu góc), object nào chưa lưu góc giữ khung thẳng như
/// cũ, vì nội dung của nó giờ đã thẳng; object con có góc đã lưu vẫn đi theo phép xoay.
pub(super) fn apply_upright(n: &mut Node, m: Affine, detected: bool) {
    let mut angles = Vec::new();
    collect_angles(n, &mut angles);
    n.transform(m, upright_scaling());
    settle_angles(n, &mut angles.into_iter(), detected);
}

/// Bản sao của `id` đã dựng thẳng theo khung transform (cùng tâm khung); tài liệu không đổi.
pub(super) fn upright_copy(d: &Document, id: NodeId) -> Option<Node> {
    let mut n = d.node(id)?.clone();
    if let Some((m, _, detected)) = upright_affine(d, id) {
        apply_upright(&mut n, m, detected);
    }
    Some(n)
}

/// Dựng thẳng `id` ngay trên tài liệu theo khung transform; trả góc nghiêng đã bỏ (0 khi đã thẳng).
pub(super) fn make_upright(d: &mut Document, id: NodeId) -> f64 {
    let Some((m, angle, detected)) = upright_affine(d, id) else { return 0.0 };
    match d.node_mut(id) {
        Some(n) => {
            apply_upright(n, m, detected);
            angle
        }
        None => 0.0,
    }
}

/// Mở file .ai / PDF mà bản chỉnh sửa của Illustrator vẽ khác trang PDF (`fileio::load`, khi
/// VectorCraft dùng trang PDF thay cho bản chỉnh sửa): hình giữ đúng như trang `page`, các object
/// được gom lại thành group như bản chỉnh sửa `private` (dữ liệu `AIPrivateData` của file) — xem
/// `dancard_regroup.rs`. Không đọc được bản chỉnh sửa thì trang giữ nguyên.
pub(crate) fn keep_groups(private: &[u8], mut page: Document, mut warnings: Vec<String>, stale: bool) -> (Document, Vec<String>) {
    // Lời báo của DanCard đứng đầu: thanh trạng thái chỉ hiện lời báo đầu tiên.
    let mut notes = vec![];
    if stale {
        notes.push(STALE_NOTE.to_string());
    }
    // Chỉ cần khung và cấu trúc của bản chỉnh sửa: đọc riêng nó, không so lại với trang (đã so,
    // khác trang nên mới tới đây) và chữ đọc như chữ.
    if let Ok((editing, _)) = vectorcraft_eps::ai_alone(private) {
        let made = regroup::regroup(&mut page, &editing);
        if made > 0 {
            notes.push(format!("DanCard: đã gom lại {made} group theo bản chỉnh sửa lưu trong file; hình giữ đúng như trang PDF."));
        }
    }
    notes.append(&mut warnings);
    (page, notes)
}

/// Báo khi file mở theo trang PDF vì trang được sửa sau bản chỉnh sửa lưu kèm.
pub(crate) const STALE_NOTE: &str = "DanCard: trang PDF của file đã được sửa sau bản chỉnh sửa lưu kèm (có lẽ bằng chương trình khác), \
     nên file mở theo trang PDF: hình đúng như sẽ in, group gom lại theo bản chỉnh sửa.";

/// Mở file .ai / PDF theo bản chỉnh sửa lưu kèm file (`fileio::load`): giữ nguyên layer, group,
/// clip group như trong file cả khi VectorCraft vẽ nó khác trang PDF (`LoadOptions::keep_groups`,
/// mặc định; VectorCraft gốc khi đó mở theo trang PDF, rã hết group) — trừ khi trang được sửa sau
/// bản chỉnh sửa (ngày `LastModified` của trang muộn hơn của bản chỉnh sửa, ISO 32000-1 §14.5):
/// bản chỉnh sửa khi đó đã cũ, không còn là trang. → (dùng bản chỉnh sửa cả khi vẽ khác trang,
/// bản chỉnh sửa đã cũ).
pub(crate) fn trusts_editing_data(bytes: &[u8], opts: &fileio::LoadOptions) -> (bool, bool) {
    if opts.trust_editing_data {
        return (true, false);
    }
    let stale = opts.keep_groups && vectorcraft_pdf::page_newer_than_editing_data(bytes, opts.password.as_deref());
    (opts.keep_groups && !stale, stale)
}

/// Lời báo của VectorCraft khi file mở theo bản chỉnh sửa dù vẽ khác trang PDF, nói lại bằng tiếng
/// Việt: group giữ nguyên, hình khác trang bao nhiêu, xem lại trước khi in.
pub(crate) fn note_kept_groups(warnings: &mut Vec<String>) {
    let percent = |w: &str| {
        let why = w.strip_suffix(": they come from its editing data all the same")?;
        let (_, rest) = why.split_once("differs from the page's in ")?;
        rest.split_once('%').map(|(pct, _)| pct.to_string())
    };
    for w in warnings.iter_mut() {
        if let Some(pct) = percent(w) {
            *w = format!(
                "DanCard: giữ nguyên group, clip group như trong file (mở theo bản chỉnh sửa lưu kèm file). \
                 Hình VectorCraft vẽ ra khác trang PDF của file khoảng {pct}%: xem kỹ trước khi in."
            );
        }
    }
    // Lời báo của DanCard đứng đầu: thanh trạng thái chỉ hiện lời báo đầu tiên.
    let (mut ours, rest): (Vec<String>, Vec<String>) = std::mem::take(warnings).into_iter().partition(|w| w.starts_with("DanCard:"));
    ours.extend(rest);
    *warnings = ours;
}

/// Trục để raster `ids` theo khung transform (Object › Rasterize `followAngle`): (trang → hệ trục
/// của khung, góc của khung). Khung thẳng: (Identity, 0).
pub(crate) fn raster_axes(d: &Document, ids: &[NodeId]) -> (Affine, f64) {
    match frame_of(d, ids, false) {
        Some(f) if f.angle != 0.0 => (Affine::rotate_about(f.angle.to_radians(), f.center), f.angle),
        _ => (Affine::IDENTITY, 0.0),
    }
}

/// Hộp nhìn thấy của `ids` (viền tính cả, như Object › Rasterize đo) trong hệ trục `to_local`.
pub(crate) fn local_bounds(d: &Document, ids: &[NodeId], to_local: Affine) -> Option<Rect> {
    if to_local == Affine::IDENTITY {
        return d.bounds_of(ids, true);
    }
    let symbols = d.symbol_boxes();
    ids.iter().filter_map(|id| d.node(*id)).fold(None, |acc, n| {
        let mut probe = n.clone();
        probe.transform(to_local, vectorcraft_doc::Scaling { strokes: true, patterns: true, ..Default::default() });
        vectorcraft_geom::union_opt(acc, probe.visual_bounds_with(&symbols))
    })
}

/// Khung của cả vùng chọn: đúng bảng Transform; riêng khi mọi object cùng một góc dò được thì
/// khung xoay theo góc đó và ôm hết chúng.
fn selection_frame(d: &Document, ids: &[NodeId], each: &[Frame], visual: bool) -> Option<Frame> {
    let boxed = Frame::from_box(d.oriented_bounds(ids, visual)?);
    let Some(first) = each.first() else { return Some(boxed) };
    if boxed.angle != 0.0 || !each.iter().all(|f| f.source == "detected" && (f.angle - first.angle).abs() < 0.05) {
        return Some(boxed);
    }
    if each.len() == 1 {
        return Some(*first);
    }
    let mut pts = Vec::new();
    for f in each {
        pts.extend(f.corners());
    }
    let hull = convex_hull(pts);
    let (_, w, h, c) = frame_at(&hull, -first.angle.to_radians());
    Some(Frame { angle: first.angle, width: w, height: h, center: c, source: "detected" })
}

fn r4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

/// `transformBox`: khung bảng Transform hiện (để đối chiếu khi góc là dò được).
fn frame_json(d: &Document, f: &Frame, transform_box: &Frame, k: f64) -> Value {
    let pt = |p: Point| json!([r4(p.x / k), r4(p.y / k)]);
    let ruler = d.ruler_origin.to_vec2();
    let board = d.artboards.iter().position(|a| a.rect.contains(f.center));
    let mut v = json!({
        "angle": r4(f.angle),
        "source": f.source,
        "width": r4(f.width / k),
        "height": r4(f.height / k),
        "center": pt(f.center),
        "rulerCenter": pt(f.center - ruler),
        "corners": f.corners().map(pt),
        "artboard": board,
    });
    if f.source == "detected" {
        v["transformBox"] =
            json!({"width": r4(transform_box.width / k), "height": r4(transform_box.height / k), "rulerCenter": pt(transform_box.center - ruler)});
    }
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
    // Như bảng Transform: Use Preview Bounds quyết định đo có viền hay không.
    let visual = match str_param(p, "bounds") {
        None => s.prefs.use_preview_bounds,
        Some("preview") => true,
        Some("geometric") => false,
        Some(other) => return Err(bad(C, format!("bounds phải là preview hoặc geometric, không phải {other}"))),
    };
    let given = p.get("ids").is_some();
    let ids = if given { checked_ids_param(s, p, "ids", C)? } else { roots_in_paint_order(s)? };
    let st = s.doc()?;
    let d = &st.doc;
    // Vùng chọn đo đúng các object bảng Transform đo.
    let selected: Vec<NodeId> = if given { ids.clone() } else { st.selection.objects.clone() };
    let mut frames = Vec::new();
    let mut objects = Vec::new();
    for id in &ids {
        let Some(n) = d.node(*id) else { continue };
        let Some(f) = frame_of(d, &[*id], visual) else { continue };
        let tb = d.oriented_bounds(&[*id], visual).map_or(f, Frame::from_box);
        let mut v = frame_json(d, &f, &tb, k);
        v["id"] = json!(id.0);
        v["name"] = json!(n.display_name());
        v["kind"] = json!(n.kind_label());
        objects.push(v);
        frames.push(f);
    }
    let selection = selection_frame(d, &selected, &frames, visual)
        .map_or(Value::Null, |f| frame_json(d, &f, &d.oriented_bounds(&selected, visual).map_or(f, Frame::from_box), k));
    Ok(json!({
        "unit": unit,
        "bounds": if visual { "preview" } else { "geometric" },
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

/// Tâm phần nhìn thấy thật của `n` ([`seen_bounds`], như `visibleUnion` của DanCard; không đo được
/// thì khung nhìn thấy) trong hệ trục `to_local` (trang → hệ trục cần đo).
fn seen_centre(n: &Node, to_local: Affine) -> Option<Point> {
    let probe;
    let n = if to_local == Affine::IDENTITY {
        n
    } else {
        let mut p = n.clone();
        p.transform(to_local, vectorcraft_doc::Scaling::default());
        probe = p;
        &probe
    };
    seen_bounds(n).or_else(|| n.visual_bounds()).filter(|b| b.width() > 0.0 && b.height() > 0.0).map(|b| b.center())
}

/// Bọc `id` trong clip group như nút Clip của DanCard (`_dcClipToCardStyleCore`, cùng cách Dàn card
/// clip từng mẫu): mask chữ nhật `w` × `h` (không tô, không viền) đặt tâm tại tâm phần nhìn thấy
/// thật ([`seen_bounds`]: bỏ object ẩn, guide, path không tô; giao với clip mask); clip group đã có
/// thì giữ nguyên khung đã chuẩn; xong thì Expand Appearance. `follow_angle`: mask nghiêng theo khung
/// transform của object (card đặt nghiêng vẫn cắt đúng theo cạnh của nó, tâm phần nhìn thấy đo theo
/// trục khung), chỉ theo phần nghiêng lẻ (-45°, 45°], nên `w` × `h` vẫn là ngang × dọc như đang thấy
/// trên trang kể cả khi object đã xoay 90°.
fn clip_one(d: &mut Document, id: NodeId, w: f64, h: f64, follow_angle: bool) -> std::result::Result<NodeId, String> {
    let n = d.node(id).ok_or("không còn trong tài liệu")?;
    if matches!(n.kind, NodeKind::Layer { .. }) {
        return Err("không clip được cả layer".into());
    }
    if matches!(n.kind, NodeKind::Group { clip: true, .. }) {
        // Clip group đã có thì giữ nguyên khung đã chuẩn (như DanCard), chỉ Expand Appearance.
        layout::expand(d, id);
        return Ok(id);
    }
    let oriented = if follow_angle {
        frame_of(d, &[id], false).map(|f| Frame { angle: tilt_of(f.angle), ..f }).filter(|f| f.angle != 0.0 && f.angle.is_finite())
    } else {
        None
    };
    // Đo và dựng mask trong hệ trục của khung đã nghiêng (hoặc của trang), rồi đưa ra trang.
    let (to_local, to_doc, angle) = match oriented {
        Some(f) => (f.to_doc().inverse(), f.to_doc(), f.angle),
        None => (Affine::IDENTITY, Affine::IDENTITY, 0.0),
    };
    let c = seen_centre(n, to_local).ok_or("không đo được vùng hiển thị của object")?;
    let local = Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0);
    let path = if angle == 0.0 { shapes::rectangle(local) } else { shapes::rectangle(local).transformed(to_doc) };
    let (parent, index, _) = d.position(id).ok_or("không tìm thấy vị trí object")?;
    let mask_id = d.alloc_id();
    let mut mask = Node::new(mask_id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: true, guide: false });
    mask.appearance = Appearance::basic(Paint::None, Paint::None, 0.0).into();
    mask.name = Some("Khung clip".into());
    mask.bbox_angle = angle;
    // Mask nằm ngay trên object, rồi bọc cả hai: clipping path là con đầu tiên của clip group.
    d.insert(parent, index + 1, mask).map_err(|e| e.to_string())?;
    let gid = d.alloc_id();
    let gid = d
        .wrap(&[id, mask_id], |mut children| {
            children.rotate_right(1);
            let mut g = Node::new(gid, NodeKind::Group { children, clip: true });
            // Khung transform của clip group đứng cùng góc với mask.
            g.bbox_angle = angle;
            g
        })
        .map_err(|e| e.to_string())?;
    layout::expand(d, gid);
    Ok(gid)
}

fn clip_to_size(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.clipToSize";
    let (unit, k) = unit_param(p, C)?;
    let (wp, hp) = size_param(p, C, k)?;
    let (w, h) = (wp / k, hp / k);
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Hãy chọn ít nhất 1 object để clip.".into()));
    }
    let follow_angle = bool_or(p, "followAngle", true);
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

// ---------------------------------------------------------------- resize

/// Sai lệch kích thước chấp nhận sau khi resize (pt).
const RESIZE_TOL: f64 = 0.01;

/// Rộng × cao của khung `f` theo phần nghiêng lẻ của nó ([`tilt_of`]): ngang × dọc như đang thấy
/// trên trang (khung đã xoay 90° thì đổi chỗ), cùng góc nghiêng đó.
fn page_dims(f: &Frame) -> Option<(f64, f64, f64)> {
    if !f.angle.is_finite() {
        return None;
    }
    let tilt = tilt_of(f.angle);
    let quarters = ((f.angle - tilt) / 90.0).round();
    let odd = quarters.is_finite() && (quarters as i64).rem_euclid(2) == 1;
    Some(if odd { (f.height, f.width, tilt) } else { (f.width, f.height, tilt) })
}

/// Co giãn `id` thành `w` × `h` (pt) theo khung transform của nó, giữ tâm: `w` × `h` là ngang × dọc
/// như đang thấy trên trang (DanCard đo khung của trang), dọc theo trục khung nghiêng. Đo lại sau
/// mỗi lần và bù tối đa hai lần (viền không co giãn, ảnh…), như DanCard.
fn resize_one(d: &mut Document, id: NodeId, w: f64, h: f64, visual: bool, sc: vectorcraft_doc::Scaling) -> std::result::Result<(), String> {
    for _ in 0..3 {
        let f = frame_of(d, &[id], visual).ok_or("không đo được khung của object")?;
        let (fw, fh, tilt) = page_dims(&f).ok_or("không đo được khung của object")?;
        if !(fw > 0.0 && fh > 0.0) {
            return Err("kích thước object không hợp lệ".into());
        }
        if (fw - w).abs() <= RESIZE_TOL && (fh - h).abs() <= RESIZE_TOL {
            return Ok(());
        }
        let (sx, sy) = (w / fw, h / fh);
        if !(sx.is_finite() && sy.is_finite()) {
            return Err("không tính được tỉ lệ resize".into());
        }
        let a = tilt.to_radians();
        let c = f.center.to_vec2();
        let m = Affine::translate(c) * Affine::rotate(-a) * Affine::scale_non_uniform(sx, sy) * Affine::rotate(a) * Affine::translate(-c);
        let n = d.node_mut(id).ok_or("không còn trong tài liệu")?;
        n.transform(m, sc);
        // Co giãn dọc theo trục khung giữ nguyên góc, cả góc dò được của file .ai/PDF.
        n.bbox_angle = f.angle;
    }
    let f = frame_of(d, &[id], visual).ok_or("không đo được khung của object")?;
    match page_dims(&f) {
        Some((fw, fh, _)) if (fw - w).abs() <= 1.0 && (fh - h).abs() <= 1.0 => Ok(()),
        _ => Err("không thể đặt đúng kích thước cho object này".into()),
    }
}

fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.resize";
    let (unit, k) = unit_param(p, C)?;
    let (wp, hp) = size_param(p, C, k)?;
    let (w, h) = (wp / k, hp / k);
    let ids = roots_in_paint_order(s)?;
    if ids.is_empty() {
        return Err(EngineError::Other("Hãy chọn ít nhất một object để resize.".into()));
    }
    let visual = s.prefs.use_preview_bounds;
    let sc = super::object::scaling(s, p);
    let total = ids.len();
    let (done, errors) = s.edit("Resize theo kích thước (DanCard)", |d, sel| {
        let mut done = Vec::new();
        let mut errors = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            match resize_one(d, *id, wp, hp, visual, sc) {
                Ok(()) => done.push(*id),
                Err(e) => errors.push(format!("Object {}: {e}", i + 1)),
            }
        }
        if done.is_empty() {
            return Err(EngineError::Other(format!("Không resize được object nào.\n{}", errors.join("\n"))));
        }
        sel.set(done.iter().copied());
        Ok((done, errors))
    })?;
    let fmt = |v: f64| {
        let t = format!("{v:.3}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut message = format!("Đã resize {}/{total} object thành {} x {} {unit}.", done.len(), fmt(w), fmt(h));
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
#[path = "dancard_tests.rs"]
mod session_tests;

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
