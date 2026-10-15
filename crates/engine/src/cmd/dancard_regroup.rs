//! DanCard: giữ group khi mở file .ai / PDF của Illustrator mà bản chỉnh sửa (editing data) không
//! dùng được vì vẽ ra khác trang PDF (vd. đường clip có màu nền mà VectorCraft vẽ đè lên ảnh, logo).
//! VectorCraft khi đó mở trang PDF: hình đúng như in nhưng mọi object rời nhau, mất hết group.
//!
//! Ở đây hình vẫn lấy từ trang PDF, chỉ gom lại thành group theo cấu trúc của bản chỉnh sửa: các
//! object của trang theo thứ tự vẽ được chia thành từng đoạn liền nhau, mỗi đoạn thuộc một object
//! cấp trên cùng của bản chỉnh sửa (group, clip group, path…) có khung chứa chúng; đoạn của một
//! group được gom thành group (cùng tên), rồi chia tiếp theo các object con của nó (tầng lồng nhau),
//! clip group của trang thì chia phần bên trong theo clip group tương ứng. Chỉ thêm group thường
//! quanh những đoạn liền nhau, không đổi thứ tự vẽ, không đổi object nào, nên hình giữ nguyên.

use std::sync::Arc;

use vectorcraft_doc::{Document, Node, NodeKind};
use vectorcraft_geom::{Rect, Vec2};

/// Sai số khung (pt) khi xét một object của trang có nằm trong khung object của bản chỉnh sửa.
const SLACK: f64 = 1.0;
/// Sai số khổ artboard (pt) giữa trang và bản chỉnh sửa, như khi VectorCraft đặt các trang về chỗ
/// artboard của chúng.
const BOARD_SLACK: f64 = 0.5;
/// Số tầng group lồng nhau dựng lại tối đa.
const MAX_DEPTH: usize = 6;
/// Giới hạn số ô của bảng chia đoạn (object × đơn vị) cho một lần chia: file lớn hơn thì gom
/// những tầng ngoài, bỏ qua tầng quá lớn.
const MAX_CELLS: usize = 4_000_000;

/// Một object của bản chỉnh sửa: khung (theo toạ độ của trang), tên, các object con (group: con;
/// clip group: phần bên trong, không kể đường clip; object đơn: không có).
struct Unit {
    bounds: Rect,
    name: Option<String>,
    group: bool,
    clip: bool,
    children: Vec<Unit>,
}

/// Gom các object trên trang `page` (file đọc theo trang PDF) theo group của bản chỉnh sửa `editing`
/// (cùng file, đọc theo dữ liệu Illustrator dù vẽ khác trang) → số group dựng lại (0: không đổi gì).
pub(crate) fn regroup(page: &mut Document, editing: &Document) -> usize {
    let Some(offsets) = artboard_offsets(page, editing) else { return 0 };
    // Các object cấp trên cùng của bản chỉnh sửa, mọi layer và layer con đang hiện (layer ẩn
    // không có trên trang), theo thứ tự vẽ.
    let mut units = vec![];
    layer_units(&editing.layers, &offsets, &mut units, 0);
    if units.is_empty() {
        return 0;
    }
    let mut made = 0;
    let mut layers = std::mem::take(&mut page.layers);
    for layer in &mut layers {
        let layer = Arc::make_mut(layer);
        if let Some(children) = layer.children_mut() {
            let objs = std::mem::take(children);
            *children = rebuild(page, objs, &units, 0, &mut made);
        }
    }
    page.layers = layers;
    page.drop_lookup_cache();
    made
}

/// Dịch chuyển từ bản chỉnh sửa sang trang theo từng artboard (khung artboard của bản chỉnh sửa,
/// dịch chuyển): `None` khi hai bên không cùng số artboard cùng khổ.
fn artboard_offsets(page: &Document, editing: &Document) -> Option<Vec<(Rect, Vec2)>> {
    if page.artboards.is_empty() || page.artboards.len() != editing.artboards.len() {
        return None;
    }
    page.artboards
        .iter()
        .zip(&editing.artboards)
        .map(|(p, e)| {
            let same = (p.rect.width() - e.rect.width()).abs() <= BOARD_SLACK && (p.rect.height() - e.rect.height()).abs() <= BOARD_SLACK;
            same.then(|| (e.rect, p.rect.origin() - e.rect.origin()))
        })
        .collect()
}

/// Khung `b` của bản chỉnh sửa theo toạ độ trang: dịch theo artboard chứa tâm của nó (ngoài mọi
/// artboard thì artboard gần nhất).
fn to_page(b: Rect, offsets: &[(Rect, Vec2)]) -> Rect {
    let c = b.center();
    let v = offsets
        .iter()
        .find(|(r, _)| r.contains(c))
        .or_else(|| offsets.iter().min_by(|a, b| (a.0.center() - c).hypot().total_cmp(&(b.0.center() - c).hypot())))
        .map_or(Vec2::ZERO, |(_, v)| *v);
    b + v
}

/// Đơn vị của các object trong `layers` (layer con: các object của nó, như ở layer cha), theo
/// thứ tự vẽ. Layer ẩn, không in hay template không có trên trang PDF: bỏ qua.
fn layer_units(layers: &[Arc<Node>], offsets: &[(Rect, Vec2)], out: &mut Vec<Unit>, depth: usize) {
    let on_page = |l: &&Arc<Node>| l.visible && matches!(l.kind, NodeKind::Layer { printable: true, template: false, .. });
    for l in layers.iter().filter(on_page) {
        for n in l.children().map(Vec::as_slice).unwrap_or_default() {
            if matches!(n.kind, NodeKind::Layer { .. }) {
                if depth < MAX_DEPTH {
                    layer_units(std::slice::from_ref(n), offsets, out, depth + 1);
                }
            } else if let Some(u) = unit(n, offsets, 0) {
                out.push(u);
            }
        }
    }
}

/// Đơn vị của bản chỉnh sửa cho object `n` (bỏ object ẩn, guide, object không có khung).
fn unit(n: &Node, offsets: &[(Rect, Vec2)], depth: usize) -> Option<Unit> {
    if !n.visible || matches!(n.kind, NodeKind::Path { guide: true, .. }) {
        return None;
    }
    let bounds = to_page(n.visual_bounds().or_else(|| n.geometric_bounds())?, offsets);
    let (group, clip) = match &n.kind {
        NodeKind::Group { clip, .. } => (true, *clip),
        _ => (false, false),
    };
    let children = match n.children() {
        Some(c) if group && depth < MAX_DEPTH => {
            let inner = if clip { c.get(1..).unwrap_or_default() } else { c.as_slice() };
            inner.iter().filter_map(|c| unit(c, offsets, depth + 1)).collect()
        }
        _ => vec![],
    };
    Some(Unit { bounds, name: n.name.clone(), group, clip, children })
}

/// Chi phí xếp object có khung `b` vào đơn vị có khung `u`: nằm trong (cho sai số) thì gần 0, ưu
/// tiên đơn vị nhỏ hơn (logo trong card thuộc logo, không thuộc card); không nằm trong thì từ 1,
/// thêm phần nằm ngoài.
fn cost(b: Option<Rect>, u: Rect, total: f64) -> f64 {
    let Some(b) = b else { return 0.5 };
    let grown = u.inflate(SLACK, SLACK);
    if grown.contains_rect(b) {
        return 0.01 * (u.area() / total).clamp(0.0, 1.0);
    }
    let area = b.area().max(1e-9);
    let inside = b.intersect(grown);
    let shared = if inside.width() > 0.0 && inside.height() > 0.0 { inside.area() } else { 0.0 };
    1.0 + (1.0 - shared / area).clamp(0.0, 1.0)
}

/// Đơn vị cho từng object của `bounds` (theo thứ tự vẽ), chỉ số đơn vị không giảm (mỗi đơn vị một
/// đoạn liền nhau), tổng chi phí nhỏ nhất. `None` khi bảng quá lớn.
fn segment(bounds: &[Option<Rect>], units: &[Unit]) -> Option<Vec<usize>> {
    let (n, k) = (bounds.len(), units.len());
    if n == 0 || k == 0 || n.checked_mul(k)? > MAX_CELLS {
        return None;
    }
    let total = units.iter().map(|u| u.bounds.area()).fold(1.0, f64::max);
    // from[i * k + j]: đơn vị của object trước khi object i ở đơn vị j.
    let mut from = vec![0u32; n * k];
    let mut best: Vec<f64> = units.iter().map(|u| cost(bounds.first().copied().flatten(), u.bounds, total)).collect();
    for i in 1..n {
        let b = bounds.get(i).copied().flatten();
        // Nhỏ nhất của best[0..=j] (bằng nhau thì lấy đơn vị sau: object mới nối tiếp đoạn gần nhất).
        let mut run = (f64::INFINITY, 0usize);
        let mut next = vec![0.0; k];
        for (j, u) in units.iter().enumerate() {
            let v = best.get(j).copied().unwrap_or(f64::INFINITY);
            if v <= run.0 {
                run = (v, j);
            }
            if let Some(slot) = next.get_mut(j) {
                *slot = run.0 + cost(b, u.bounds, total);
            }
            if let Some(f) = from.get_mut(i * k + j) {
                *f = u32::try_from(run.1).ok()?;
            }
        }
        best = next;
    }
    let mut j = best.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1)).map(|(j, _)| j)?;
    let mut out = vec![0usize; n];
    for i in (0..n).rev() {
        if let Some(slot) = out.get_mut(i) {
            *slot = j;
        }
        if i > 0 {
            j = *from.get(i * k + j)? as usize;
        }
    }
    Some(out)
}

/// Các object `objs` (một tầng của trang, theo thứ tự vẽ) gom theo các đơn vị `units` của tầng
/// tương ứng trong bản chỉnh sửa. Thứ tự vẽ giữ nguyên. Group / clip group của trang bao trùm từ 2
/// đơn vị trở lên (clip theo khổ trang, nhóm trong suốt cả trang…) không thuộc đơn vị nào: mở ra,
/// gom phần bên trong theo cùng các đơn vị đó.
fn rebuild(d: &mut Document, objs: Vec<Arc<Node>>, units: &[Unit], depth: usize, made: &mut usize) -> Vec<Arc<Node>> {
    let mut out = Vec::with_capacity(objs.len());
    let mut pending = vec![];
    for o in objs {
        // Layer con (từ nhóm nội dung tùy chọn của PDF) không vào group nào: gom phần bên trong nó.
        let sublayer = matches!(o.kind, NodeKind::Layer { .. });
        if sublayer || (depth + 1 < MAX_DEPTH && spans_units(&o, units)) {
            out.extend(rebuild_segment(d, std::mem::take(&mut pending), units, depth, made));
            let mut node = Node::clone(&o);
            let clips = node.clips();
            if depth + 1 < MAX_DEPTH
                && let Some(children) = node.children_mut()
            {
                let at = usize::from(clips).min(children.len());
                let content = children.split_off(at);
                let content = rebuild(d, content, units, depth + 1, made);
                children.extend(content);
            }
            out.push(Arc::new(node));
        } else {
            pending.push(o);
        }
    }
    out.extend(rebuild_segment(d, pending, units, depth, made));
    out
}

/// `o` là group / clip group của trang mà khung chứa trọn từ 2 đơn vị trở lên.
fn spans_units(o: &Node, units: &[Unit]) -> bool {
    if !matches!(o.kind, NodeKind::Group { .. }) {
        return false;
    }
    let Some(b) = o.visual_bounds().or_else(|| o.geometric_bounds()) else { return false };
    let grown = b.inflate(SLACK, SLACK);
    units.iter().filter(|u| grown.contains_rect(u.bounds)).nth(1).is_some()
}

/// [`rebuild`] cho một dãy không có object bao trùm.
fn rebuild_segment(d: &mut Document, objs: Vec<Arc<Node>>, units: &[Unit], depth: usize, made: &mut usize) -> Vec<Arc<Node>> {
    let bounds: Vec<Option<Rect>> = objs.iter().map(|o| o.visual_bounds().or_else(|| o.geometric_bounds())).collect();
    let Some(owner) = segment(&bounds, units) else { return objs };
    let mut out = Vec::with_capacity(objs.len());
    let mut objs = objs.into_iter().zip(bounds).zip(owner).peekable();
    while let Some(((first, b), u)) = objs.next() {
        let mut run = vec![(first, b)];
        while let Some(((o, b), _)) = objs.next_if(|(_, next)| *next == u) {
            run.push((o, b));
        }
        let Some(unit) = units.get(u) else {
            out.extend(run.into_iter().map(|(o, _)| o));
            continue;
        };
        // Object ở hai đầu đoạn không nằm trong khung đơn vị (dấu in của trang…) để ngoài group:
        // ngay trước / sau group nên thứ tự vẽ không đổi.
        let inside = |b: &Option<Rect>| b.is_some_and(|b| unit.bounds.inflate(SLACK, SLACK).contains_rect(b));
        let lead = run.iter().take_while(|(_, b)| !inside(b)).count();
        let tail = run.iter().rev().take_while(|(_, b)| !inside(b)).count().min(run.len() - lead);
        let mut middle: Vec<(Arc<Node>, Option<Rect>)> = run.split_off(lead);
        let after = middle.split_off(middle.len() - tail);
        out.extend(run.into_iter().map(|(o, _)| o));
        out.extend(regroup_run(d, middle, unit, depth, made));
        out.extend(after.into_iter().map(|(o, _)| o));
    }
    out
}

/// Một đoạn của trang thuộc đơn vị `unit`: một object thì giữ nguyên (clip group thì gom tiếp phần
/// bên trong), nhiều object và đơn vị là group thì gom thành group rồi gom tiếp theo con của nó.
fn regroup_run(d: &mut Document, run: Vec<(Arc<Node>, Option<Rect>)>, unit: &Unit, depth: usize, made: &mut usize) -> Vec<Arc<Node>> {
    let objs: Vec<Arc<Node>> = run.into_iter().map(|(o, _)| o).collect();
    if objs.is_empty() {
        return objs;
    }
    let deeper = depth + 1 < MAX_DEPTH && !unit.children.is_empty();
    if let [one] = objs.as_slice() {
        // Group / clip group của trang ứng với group / clip group của bản chỉnh sửa: gom phần bên
        // trong (sau đường clip) theo các con của đơn vị (clip group của trang trong group bọc
        // clip group của bản chỉnh sửa: theo phần bên trong của clip group đó).
        if deeper && unit.group && matches!(one.kind, NodeKind::Group { .. }) {
            let mut node = Node::clone(one);
            let clips = node.clips();
            let units = if clips && !unit.clip { inner_units(unit) } else { unit.children.as_slice() };
            if let Some(children) = node.children_mut() {
                let at = usize::from(clips).min(children.len());
                if children.len() > at + 1 {
                    let content = children.split_off(at);
                    let content = rebuild(d, content, units, depth + 1, made);
                    children.extend(content);
                }
            }
            return vec![Arc::new(node)];
        }
        return objs;
    }
    if !unit.group {
        return objs;
    }
    let children = if deeper { rebuild(d, objs, &unit.children, depth + 1, made) } else { objs };
    let mut group = Node::group(d.alloc_id(), children);
    group.name = unit.name.clone();
    *made += 1;
    vec![Arc::new(group)]
}

/// Con của một group chỉ bọc một clip group (card: group › clip group › phần bên trong): phần bên
/// trong của clip group đó; không thì con của chính nó.
fn inner_units(unit: &Unit) -> &[Unit] {
    match unit.children.as_slice() {
        [only] if only.clip => &only.children,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, NodeId};
    use vectorcraft_geom::shapes;

    use super::*;

    fn rect(d: &mut Document, x: f64, y: f64, w: f64, h: f64, c: Color) -> Arc<Node> {
        Arc::new(Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, y, x + w, y + h)), Appearance::basic(Paint::solid(c), Paint::None, 0.0)))
    }

    fn named_group(d: &mut Document, name: &str, children: Vec<Arc<Node>>) -> Arc<Node> {
        let mut g = Node::group(d.alloc_id(), children);
        g.name = Some(name.into());
        Arc::new(g)
    }

    fn clip_group(d: &mut Document, mask: Rect, content: Vec<Arc<Node>>) -> Arc<Node> {
        let mut m = Node::path(d.alloc_id(), shapes::rectangle(mask), Appearance::basic(Paint::None, Paint::None, 0.0));
        if let NodeKind::Path { clipping, .. } = &mut m.kind {
            *clipping = true;
        }
        let mut children = vec![Arc::new(m)];
        children.extend(content);
        Arc::new(Node::new(d.alloc_id(), NodeKind::Group { children, clip: true }))
    }

    fn top(d: &Document) -> Vec<Arc<Node>> {
        d.layers.first().and_then(|l| l.children()).cloned().unwrap_or_default()
    }

    fn put(d: &mut Document, objs: Vec<Arc<Node>>) {
        let layer = d.layers.first().map(|l| l.id).unwrap_or(NodeId(0));
        for o in objs {
            d.insert(Some(layer), usize::MAX, Node::clone(&o)).unwrap();
        }
    }

    /// Hai card (group: nền, logo hai mảnh; group: clip group nền + chữ) trong bản chỉnh sửa; trang
    /// có cùng các mảnh nhưng rời nhau (thêm một dấu in ở đầu): gom lại đúng hai group cùng tên, đúng
    /// thứ tự vẽ, dấu in để ngoài; logo thành group con.
    #[test]
    fn gom_lai_hai_card_theo_ban_chinh_sua() {
        let (red, blue, white) = (Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 0.0, 1.0), Color::rgb(1.0, 1.0, 1.0));
        let mut e = Document::new(600.0, 400.0);
        let bg1 = rect(&mut e, 10.0, 10.0, 100.0, 60.0, white);
        let l1 = rect(&mut e, 20.0, 20.0, 10.0, 10.0, red);
        let l2 = rect(&mut e, 32.0, 20.0, 10.0, 10.0, blue);
        let logo = named_group(&mut e, "logo", vec![l1, l2]);
        let card1 = named_group(&mut e, "card 1", vec![bg1, logo]);
        let bg2 = rect(&mut e, 120.0, 10.0, 100.0, 60.0, white);
        let clip = clip_group(&mut e, Rect::new(120.0, 10.0, 220.0, 70.0), vec![bg2]);
        let text = rect(&mut e, 130.0, 30.0, 50.0, 8.0, blue);
        let card2 = named_group(&mut e, "card 2", vec![clip, text]);
        put(&mut e, vec![card1, card2]);

        let mut p = Document::new(600.0, 400.0);
        let mark = rect(&mut p, 0.0, 0.0, 2.0, 300.0, red);
        let pieces = vec![
            mark,
            rect(&mut p, 10.0, 10.0, 100.0, 60.0, white),
            rect(&mut p, 20.0, 20.0, 10.0, 10.0, red),
            rect(&mut p, 32.0, 20.0, 10.0, 10.0, blue),
        ];
        let pbg2 = rect(&mut p, 115.0, 5.0, 110.0, 70.0, white);
        let pclip = clip_group(&mut p, Rect::new(120.0, 10.0, 220.0, 70.0), vec![pbg2]);
        let ptext = rect(&mut p, 130.0, 30.0, 50.0, 8.0, blue);
        put(&mut p, pieces.into_iter().chain([pclip, ptext]).collect());
        let ids_before: Vec<NodeId> = top(&p).iter().map(|n| n.id).collect();

        let made = regroup(&mut p, &e);
        let t = top(&p);
        assert_eq!(made, 3, "hai card và logo");
        assert_eq!(t.len(), 3, "dấu in, card 1, card 2");
        assert_eq!(t[0].id, ids_before[0], "dấu in để ngoài, vẽ trước");
        assert_eq!(t[1].name.as_deref(), Some("card 1"));
        assert_eq!(t[2].name.as_deref(), Some("card 2"));
        let c1 = t[1].children().unwrap();
        assert_eq!(c1.len(), 2, "nền và logo");
        assert_eq!(c1[1].name.as_deref(), Some("logo"));
        assert_eq!(c1[1].children().unwrap().len(), 2);
        let c2 = t[2].children().unwrap();
        assert!(c2[0].clips() && c2[1].id == ids_before[5], "clip group của trang giữ nguyên rồi tới chữ");
        // Thứ tự vẽ các object lá không đổi.
        fn leaves(n: &Node, out: &mut Vec<NodeId>) {
            match n.children() {
                Some(c) if !matches!(n.kind, NodeKind::Compound { .. }) => c.iter().for_each(|c| leaves(c, out)),
                _ => out.push(n.id),
            }
        }
        let mut after = vec![];
        t.iter().for_each(|n| leaves(n, &mut after));
        let mut before = vec![];
        ids_before.iter().filter_map(|id| p.node(*id)).for_each(|n| leaves(n, &mut before));
        assert_eq!(after.len(), 7);
        assert_eq!(after, before, "thứ tự vẽ giữ nguyên");
    }

    /// Trang bọc mọi thứ trong một clip group theo khổ trang: mở ra, gom phần bên trong.
    #[test]
    fn mo_clip_group_bao_trum_ca_trang() {
        let c = Color::rgb(0.2, 0.4, 0.6);
        let mut e = Document::new(600.0, 400.0);
        let a = vec![rect(&mut e, 10.0, 10.0, 50.0, 50.0, c), rect(&mut e, 20.0, 20.0, 10.0, 10.0, c)];
        let card_a = named_group(&mut e, "A", a);
        let b = vec![rect(&mut e, 100.0, 10.0, 50.0, 50.0, c), rect(&mut e, 110.0, 20.0, 10.0, 10.0, c)];
        let card_b = named_group(&mut e, "B", b);
        put(&mut e, vec![card_a, card_b]);
        let mut p = Document::new(600.0, 400.0);
        let pieces = vec![
            rect(&mut p, 10.0, 10.0, 50.0, 50.0, c),
            rect(&mut p, 20.0, 20.0, 10.0, 10.0, c),
            rect(&mut p, 100.0, 10.0, 50.0, 50.0, c),
            rect(&mut p, 110.0, 20.0, 10.0, 10.0, c),
        ];
        let page_clip = clip_group(&mut p, Rect::new(0.0, 0.0, 600.0, 400.0), pieces);
        put(&mut p, vec![page_clip]);
        assert_eq!(regroup(&mut p, &e), 2);
        let t = top(&p);
        assert_eq!(t.len(), 1);
        let inner = t[0].children().unwrap();
        assert!(t[0].clips() && inner.len() == 3, "đường clip rồi hai card");
        assert_eq!(inner[1].name.as_deref(), Some("A"));
        assert_eq!(inner[2].name.as_deref(), Some("B"));
    }

    /// Layer con của trang (nội dung tùy chọn của PDF) không vào group nào: các mảnh bên trong nó
    /// được gom; object trong layer con của bản chỉnh sửa vẫn là đơn vị.
    #[test]
    fn layer_con_khong_vao_group() {
        let c = Color::rgb(0.2, 0.4, 0.6);
        let mut e = Document::new(600.0, 400.0);
        let a = vec![rect(&mut e, 10.0, 10.0, 50.0, 50.0, c), rect(&mut e, 20.0, 20.0, 10.0, 10.0, c)];
        let card = named_group(&mut e, "A", a);
        let mut sub = crate::cmd::layer::make_layer(&mut e, Some("sub"));
        if let Some(ch) = sub.children_mut() {
            ch.push(card);
        }
        put(&mut e, vec![Arc::new(sub)]);
        let mut p = Document::new(600.0, 400.0);
        let pieces = vec![rect(&mut p, 10.0, 10.0, 50.0, 50.0, c), rect(&mut p, 20.0, 20.0, 10.0, 10.0, c)];
        let mut psub = crate::cmd::layer::make_layer(&mut p, Some("sub"));
        if let Some(ch) = psub.children_mut() {
            ch.extend(pieces);
        }
        put(&mut p, vec![Arc::new(psub)]);
        assert_eq!(regroup(&mut p, &e), 1);
        let t = top(&p);
        assert_eq!(t.len(), 1);
        assert!(matches!(t[0].kind, NodeKind::Layer { .. }), "layer con giữ nguyên chỗ");
        let inner = t[0].children().unwrap();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].name.as_deref(), Some("A"));
        assert_eq!(inner[0].children().unwrap().len(), 2);
    }

    /// Không cùng số artboard, hay bản chỉnh sửa trống: không đổi gì.
    #[test]
    fn khong_doi_khi_khong_khop() {
        let mut e = Document::new(600.0, 400.0);
        let mut p = Document::new(600.0, 400.0);
        let r = rect(&mut p, 0.0, 0.0, 10.0, 10.0, Color::rgb(1.0, 0.0, 0.0));
        put(&mut p, vec![r]);
        assert_eq!(regroup(&mut p, &e), 0, "bản chỉnh sửa không có object");
        let r = rect(&mut e, 0.0, 0.0, 10.0, 10.0, Color::rgb(1.0, 0.0, 0.0));
        put(&mut e, vec![r]);
        let mut other = Document::new(500.0, 400.0);
        assert_eq!(regroup(&mut other, &e), 0, "khổ khác");
        assert_eq!(segment(&[], &[]), None);
    }
}
