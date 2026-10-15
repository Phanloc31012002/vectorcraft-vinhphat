//! Dàn bế: vector tham chiếu của lõi dàn (đo trên `dan_be_nester.js` gốc của DanCard bằng Node,
//! spec §7) và các lần dàn đầy đủ qua `Session` như panel gọi.

use serde_json::{Value, json};
use vectorcraft_doc::{Appearance, Document, Node, NodeId};
use vectorcraft_geom::{Anchor, PathData, Point, SubPath};

use super::nester::{self, Clock, DotIn, Groups, NestInput, NestOut, Sheet, Slot, TypeIn};
use super::{FLATNESS, MM, is_generated, read_die, sub_contour};
use crate::Session;

const KAPPA: f64 = 0.5522847498;

fn rect_mm(w: f64, h: f64) -> Groups {
    vec![vec![vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]]]
}

fn triangle() -> Groups {
    vec![vec![vec![[0.0, 0.0], [60.0, 0.0], [30.0, 45.0]]]]
}

fn ell_shape() -> Groups {
    vec![vec![vec![[0.0, 0.0], [60.0, 0.0], [60.0, 20.0], [20.0, 20.0], [20.0, 60.0], [0.0, 60.0]]]]
}

/// Đường tròn bốn điểm neo kiểu Illustrator (κ = 0,5522847498), bán kính `r_mm`, tâm (200, 200) pt.
fn circle(r_mm: f64) -> SubPath {
    let (c, r) = (Point::new(200.0, 200.0), r_mm * MM);
    let k = KAPPA * r;
    let anchors = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)].map(|(ux, uy): (f64, f64)| {
        let p = Point::new(c.x + r * ux, c.y + r * uy);
        let (tx, ty) = (-uy, ux);
        Anchor::with_handles(p, Point::new(p.x - k * tx, p.y - k * ty), Point::new(p.x + k * tx, p.y + k * ty))
    });
    SubPath::new(anchors.to_vec(), true)
}

fn straight(groups: Groups) -> TypeIn {
    TypeIn { groups, curve_error: 0.0 }
}

/// Payload như Prepare gửi: khe 2, lề 4, né PON 7,5 mm; `pon` = trên, dưới, trái, phải.
fn input(w: f64, h: f64, types: Vec<TypeIn>, pon: [f64; 4]) -> NestInput {
    let [top, bottom, left, right] = pon;
    let dot = |x: f64, y: f64| DotIn { x, y, r: 2.5 };
    NestInput {
        width: w,
        height: h,
        dots: vec![dot(left, h - top), dot(w - right, h - top), dot(left, bottom), dot(w - right, bottom)],
        types,
        gap: 2.0,
        margin: 4.0,
        pon_clear: 7.5,
        step: 0.25,
        budget_ms: 3000.0,
    }
}

fn nest(inp: &NestInput) -> NestOut {
    nester::nest(inp, &Clock::default()).unwrap_or_else(|e| panic!("{e}"))
}

/// Các con (hướng, x, y mm) bất kể thứ tự, sai lệch 0,002 mm.
fn assert_slots(out: &NestOut, want: &[(usize, f64, f64)]) {
    let mut got: Vec<(usize, f64, f64)> = out.slots.iter().map(|s| (s.vi, s.x, s.y)).collect();
    assert_eq!(got.len(), want.len(), "{got:?}");
    let mut want = want.to_vec();
    let key = |a: &(usize, f64, f64)| ((a.2 * 100.0).round() as i64, (a.1 * 100.0).round() as i64);
    got.sort_by_key(key);
    want.sort_by_key(key);
    for (g, w) in got.iter().zip(&want) {
        assert!(g.0 == w.0 && (g.1 - w.1).abs() < 2e-3 && (g.2 - w.2).abs() < 2e-3, "{got:?} ≠ {want:?}");
    }
}

fn edges(contours: &[Vec<[f64; 2]>]) -> Vec<([f64; 2], [f64; 2])> {
    let mut out = Vec::new();
    for c in contours {
        for (i, p) in c.iter().enumerate() {
            out.push((*p, c[(i + 1) % c.len()]));
        }
    }
    out
}

fn seg_dist(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    if o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0 {
        return 0.0;
    }
    let ps = nester::point_segment_distance_sq;
    ps(a, c, d).min(ps(b, c, d)).min(ps(c, a, b)).min(ps(d, a, b)).sqrt()
}

/// Kiểm tra độc lập (spec §3.18): khe giữa mọi cặp, lề, né PON, không lọt vào nhau.
fn check_contract(inp: &NestInput, out: &NestOut) {
    let placed: Vec<Vec<Vec<[f64; 2]>>> = out
        .slots
        .iter()
        .map(|s| {
            let g = nester::rotate_groups(&inp.types[s.mi].groups, nester::ANGLES[s.vi]).unwrap();
            g.into_iter().flatten().map(|c| c.into_iter().map(|p| [p[0] + s.x, p[1] + s.y]).collect()).collect()
        })
        .collect();
    let curve = inp.types.iter().map(|t| t.curve_error).fold(0.0, f64::max);
    for (i, a) in placed.iter().enumerate() {
        for p in a.iter().flatten() {
            assert!(p[0] >= inp.margin - 0.0011 && p[0] <= inp.width - inp.margin + 0.0011, "lề trái/phải: {p:?}");
            assert!(p[1] >= inp.margin - 0.0011 && p[1] <= inp.height - inp.margin + 0.0011, "lề dưới/trên: {p:?}");
        }
        let ea = edges(a);
        for d in &inp.dots {
            let c = [d.x, d.y];
            let near = ea.iter().map(|(p, q)| nester::point_segment_distance_sq(c, *p, *q).sqrt()).fold(f64::INFINITY, f64::min);
            assert!(near >= d.r + inp.pon_clear - 0.0011, "né PON: {near}");
            assert!(!nester::inside_groups(c, std::slice::from_ref(a)), "chấm PON nằm trong khuôn");
        }
        for b in placed.iter().skip(i + 1) {
            let eb = edges(b);
            let min = ea.iter().flat_map(|(p, q)| eb.iter().map(move |(r, s)| seg_dist(*p, *q, *r, *s))).fold(f64::INFINITY, f64::min);
            assert!(min >= inp.gap - 0.0011 - 2.0 * curve, "khe {min} < {}", inp.gap);
            assert!(
                !nester::inside_groups(a[0][0], std::slice::from_ref(b)) && !nester::inside_groups(b[0][0], std::slice::from_ref(a)),
                "khuôn lọt vào nhau"
            );
        }
    }
}

// ---------------------------------------------------------------- vector đơn vị (§7.1)

#[test]
fn lam_phang_nhu_dancard() {
    for (r, n) in [(20.0, 1024), (15.0, 1024), (4.0, 512)] {
        let mut curve = false;
        let pts = sub_contour(&circle(r), &mut curve).unwrap().unwrap();
        assert_eq!(pts.len(), n, "R {r} mm");
        assert!(curve);
    }
    let mut curve = false;
    let square = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(50.0, 0.0), Point::new(50.0, 30.0), Point::new(0.0, 30.0)], true);
    assert_eq!(sub_contour(&square, &mut curve).unwrap().unwrap().len(), 4);
    assert!(!curve);
    let open = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(50.0, 0.0), Point::new(50.0, 30.0)], false);
    assert_eq!(sub_contour(&open, &mut curve).unwrap_err(), "Khuôn bế phải là path khép kín; không dùng khung bao thay thế.");
    let helper = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(50.0, 0.0)], false);
    assert!(sub_contour(&helper, &mut curve).unwrap().is_none(), "đường hở ngắn bị bỏ qua");
}

#[test]
fn raster_chu_nhat_nhu_ban_goc() {
    let ck = Clock::default();
    let rect = rect_mm(50.0, 30.0);
    let v = nester::build_variant(&rect, 0, 0, 0, 2.0, 0.25, &ck).unwrap();
    assert_eq!((v.w_cells, v.h_cells, v.pad, v.clear_rows.len()), (200, 120, 5, 130));
    assert_eq!(v.raw_rows[0], [(0, 200)]);
    let head: [&[(i64, i64)]; 6] = [&[], &[(-3, 203)], &[(-4, 204)], &[(-5, 205)], &[(-5, 205)], &[(-5, 205)]];
    for (k, want) in head.iter().enumerate() {
        assert_eq!(v.clear_rows[k].as_slice(), *want, "clearRows[{k}]");
    }
    let v90 = nester::build_variant(&rect, 0, 1, 90, 2.0, 0.25, &ck).unwrap();
    assert_eq!((v90.w, v90.h), (30.0, 50.0));
    assert_eq!((v90.w_cells, v90.h_cells, v90.pad, v90.clear_rows.len()), (120, 200, 5, 210));
    assert_eq!(v90.clear_rows[1], [(-3, 123)]);
    let g0 = nester::build_variant(&rect, 0, 0, 0, 0.0, 0.25, &ck).unwrap();
    assert_eq!((g0.pad, g0.clear_rows.len()), (1, 122));
    assert!(g0.clear_rows[0].is_empty());
    assert_eq!(g0.clear_rows[1], [(-1, 201)]);
    assert_eq!(g0.clear_rows[120], [(-1, 201)]);
    assert_eq!(nester::right_profile_contact(&v, &v, 0, &ck), 210);
}

#[test]
fn tam_giac_hang_dac_trung_va_tiep_xuc() {
    let ck = Clock::default();
    let t0 = nester::build_variant(&triangle(), 0, 0, 0, 2.0, 0.25, &ck).unwrap();
    let t180 = nester::build_variant(&triangle(), 0, 2, 180, 2.0, 0.25, &ck).unwrap();
    assert_eq!((t0.w_cells, t0.h_cells, t0.pad), (240, 180, 5));
    assert_eq!(t0.raw_rows[0], [(0, 240)]);
    assert_eq!(t0.raw_rows[90], [(60, 180)]);
    assert_eq!(t0.raw_rows[179], [(119, 121)]);
    assert_eq!(t180.raw_rows[0], [(119, 121)]);
    assert_eq!(t180.raw_rows[179], [(0, 240)]);
    assert_eq!(nester::right_profile_contact(&t0, &t180, 0, &ck), 134);
    assert_eq!(nester::right_profile_contact(&t180, &t0, 0, &ck), 134);
    assert_eq!(nester::right_profile_contact(&t0, &t0, 0, &ck), 250);
    // Không có hàng 179: hàng cuối bị bỏ khi rút gọn (quirk của bản gốc, giữ nguyên).
    assert_eq!(nester::feature_rows(&t0), [0, 10, 13, 26, 30, 38, 51, 60, 64, 70, 77, 90, 100, 102, 115, 120, 128, 141, 150, 153, 160, 166]);
    assert_eq!(nester::feature_rows(&t180), [0, 13, 20, 26, 30, 38, 51, 60, 64, 77, 80, 90, 102, 110, 115, 120, 128, 141, 150, 153, 166, 170]);
    let vs = vec![t0, t180];
    let sheet = Sheet { rw: 1288, rh: 1384, dots: Vec::new() };
    let cs = nester::candidates_for(&[Slot { mi: 0, v: 0, x: 0, y: 0 }], &vs[1], &vs, &sheet, &ck);
    assert_eq!(cs.len(), 255);
    assert_eq!(cs[..10], [(0, 0), (1048, 0), (0, 1204), (1048, 1204), (524, 0), (524, 1204), (0, 602), (1048, 602), (134, 0), (133, 1)]);
    assert_eq!(nester::contact_offsets(&vs[0], &vs[1], &ck).len(), 750);
}

#[test]
fn nhan_dien_khuon_tron() {
    let poly: Vec<[f64; 2]> = (0..1024)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / 1024.0;
            [20.0 + 20.0 * a.cos(), 20.0 + 20.0 * a.sin()]
        })
        .collect();
    let c = nester::circle_like(&[vec![poly]]).unwrap();
    assert!((c.radius - 20.0).abs() < 1e-9 && (c.cx - 20.0).abs() < 1e-9 && (c.cy - 20.0).abs() < 1e-9, "{c:?}");
    assert!(nester::circle_like(&rect_mm(40.0, 40.0)).is_none(), "hình vuông không phải hình tròn");
}

// ---------------------------------------------------------------- nest() đầy đủ (§7.2)

#[test]
fn nest_chu_nhat_nho() {
    let inp = input(120.0, 100.0, vec![straight(rect_mm(50.0, 30.0))], [10.0; 4]);
    let out = nest(&inp);
    assert_slots(&out, &[(1, 13.0, 25.0), (1, 45.0, 25.0), (1, 77.0, 25.0)]);
    assert!(out.exact, "{out:?}");
    assert_eq!(out.mode, "silhouette nhiều khoảng + tiếp xúc thật");
    assert_eq!(out.counts, [3]);
    assert!(out.detail.starts_with("Dàn silhouette thật: 3 con (loại 1: 3). Lưới kiểm tra 0.25 mm, khe 2.00 mm;"), "{}", out.detail);
    check_contract(&inp, &out);
    // Tất định: chạy lại cho đúng kết quả cũ.
    assert_eq!(format!("{:?}", nest(&inp).slots), format!("{:?}", out.slots));
}

#[test]
fn nest_pon_lech_gioi_han_canh_tam() {
    let inp = input(120.0, 100.0, vec![straight(rect_mm(50.0, 30.0))], [25.0, 5.0, 30.0, 10.0]);
    let out = nest(&inp);
    assert_slots(&out, &[(1, 13.0, 15.001), (1, 45.0, 15.001), (1, 77.0, 15.001)]);
    assert!(!out.exact && (out.offset.1 + 9.999008).abs() < 1e-3, "{:?}", out.offset);
    assert!(out.detail.ends_with(" Canh tâm bị giới hạn bởi vùng né PON."), "{}", out.detail);
    check_contract(&inp, &out);
}

#[test]
fn nest_tam_giac_va_chu_l() {
    // Bản gốc chạm hạn 3 giây ở tam giác: chỉ so số con (cận dưới) và hợp đồng.
    let tri = input(150.0, 120.0, vec![straight(triangle())], [10.0; 4]);
    let out = nest(&tri);
    assert!(out.slots.len() >= 6, "{out:?}");
    check_contract(&tri, &out);
    let again = nest(&tri);
    assert_eq!(format!("{:?}", again.slots), format!("{:?}", out.slots), "tất định");
    let ell = input(150.0, 150.0, vec![straight(ell_shape())], [10.0; 4]);
    let out = nest(&ell);
    assert_slots(&out, &[(2, 8.001, 7.8471), (1, 70.001, 7.8471), (2, 30.321, 29.8472), (3, 8.001, 69.8472), (2, 70.001, 69.8472)]);
    check_contract(&ell, &out);
}

#[test]
fn nest_khuon_tron_luoi_luc_giac() {
    let die = read_die(&Node::path(NodeId(1), PathData::single(circle(20.0)), Appearance::default()), 0).unwrap();
    assert!(die.curve);
    let inp = input(100.0, 100.0, vec![TypeIn { groups: die.groups, curve_error: FLATNESS }], [10.0; 4]);
    let out = nest(&inp);
    assert_eq!(out.mode, "lục giác động 2 hướng + silhouette kiểm tra thật");
    assert_slots(&out, &[(0, 8.9997, 30.0), (0, 51.0003, 30.0)]);
    assert!(out.exact, "{out:?}");
    check_contract(&inp, &out);
}

#[test]
fn nest_nhieu_mau_can_bang() {
    let inp = input(150.0, 130.0, vec![straight(rect_mm(50.0, 30.0)), straight(triangle())], [10.0; 4]);
    let out = nest(&inp);
    assert!(out.slots.len() >= 6, "{out:?}");
    assert!(out.counts[0].abs_diff(out.counts[1]) <= 1, "{:?}", out.counts);
    check_contract(&inp, &out);
    // Hai mẫu chung một khuôn: tìm một lần rồi chia xoay vòng.
    let same = input(120.0, 100.0, vec![straight(rect_mm(50.0, 30.0)), straight(rect_mm(50.0, 30.0))], [10.0; 4]);
    let out = nest(&same);
    assert!(out.mode.ends_with(" + luân phiên các bài cùng khuôn"), "{}", out.mode);
    assert_eq!(out.counts, [2, 1]);
}

#[test]
fn nest_bao_loi_nhu_dancard() {
    let too_big = input(330.0, 354.0, vec![straight(rect_mm(400.0, 30.0))], [10.0; 4]);
    assert_eq!(nester::nest(&too_big, &Clock::default()).unwrap_err(), "Khuôn loại 1 không vừa khổ tờ sau khi trừ lề, ở cả 4 hướng xoay.");
    let tight = NestInput { margin: 60.0, ..input(120.0, 100.0, vec![straight(rect_mm(50.0, 30.0))], [10.0; 4]) };
    assert_eq!(nester::nest(&tight, &Clock::default()).unwrap_err(), "Khổ tờ nhỏ hơn phần lề đã nhập.");
}

// ---------------------------------------------------------------- qua Session

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn layer_named<'a>(d: &'a Document, name: &str) -> &'a Node {
    d.layers.iter().find(|l| l.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("thiếu layer {name}"))
}

fn kids(l: &Node) -> Vec<&Node> {
    l.children().unwrap().iter().map(|c| &**c).collect()
}

/// Hình chữ nhật `w` × `h` mm có tâm (`cx`, `cy`) pt.
fn rect_at(s: &mut Session, cx: f64, cy: f64, w: f64, h: f64) -> NodeId {
    id_of(&run(s, "shape.rectangle", json!({"x": cx - w * MM / 2.0, "y": cy - h * MM / 2.0, "width": w * MM, "height": h * MM})))
}

#[test]
fn dan_be_mot_mat_qua_session() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let die = rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    let art = rect_at(&mut s, 360.0, 98.0, 40.0, 20.0);
    // Thứ tự chọn không quan trọng: Khuôn là cái bên trái.
    run(&mut s, "select.set", json!({"ids": [art.0, die.0]}));
    let layers_before = s.doc().unwrap().doc.layers.len();
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10", "ponClear": "7,5", "gap": ""}));
    assert_eq!(r["count"], 3, "{r}");
    assert!(r["message"].as_str().unwrap().starts_with("OK:[[COUNT:3]] Dàn silhouette thật: 3 con"), "{r}");
    let d = &s.doc().unwrap().doc;
    // Artboard cũ bị xoá; tờ mới ở góc trên trái canvas, cách mép 10 mm (như DanCard).
    assert_eq!(d.artboards.len(), 1);
    assert_eq!(d.artboards[0].name, "Dan be");
    let ab = d.artboards[0].rect;
    assert!((ab.width() - 120.0 * MM).abs() < 1e-9 && (ab.height() - 100.0 * MM).abs() < 1e-9, "{ab:?}");
    assert!((ab.x0 - (-7200.0 + 10.0 * MM)).abs() < 1e-9 && (ab.y0 - (-7200.0 + 10.0 * MM)).abs() < 1e-9, "{ab:?}");
    // Khuôn dưới cùng, PON trên cùng, mỗi layer có dấu nhận biết.
    assert_eq!(d.layers.len(), layers_before + 3);
    assert_eq!(d.layers.first().unwrap().name.as_deref(), Some("Dàn bế - Khuôn"));
    assert_eq!(d.layers.last().unwrap().name.as_deref(), Some("Dàn bế - PON"));
    let pon = layer_named(d, "Dàn bế - PON");
    assert!(is_generated(pon) && is_generated(layer_named(d, "Dàn bế - Bài")));
    let mut dots: Vec<(f64, f64)> = kids(pon)
        .iter()
        .map(|n| {
            let b = n.geometric_bounds().unwrap();
            assert!((b.width() - 5.0 * MM).abs() < 1e-6, "PON tròn 5 mm");
            ((b.center().x - ab.x0) / MM, (b.center().y - ab.y0) / MM)
        })
        .collect();
    dots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (g, w) in dots.iter().zip([(10.0, 10.0), (10.0, 90.0), (110.0, 10.0), (110.0, 90.0)]) {
        assert!((g.0 - w.0).abs() < 1e-6 && (g.1 - w.1).abs() < 1e-6, "{dots:?}");
    }
    // Ba khuôn xoay 90° ở (13, 25), (45, 25), (77, 25) mm từ góc dưới trái tờ; bài nằm giữa khuôn.
    let cuts = kids(layer_named(d, "Dàn bế - Khuôn"));
    let arts = kids(layer_named(d, "Dàn bế - Bài"));
    assert_eq!((cuts.len(), arts.len()), (3, 3));
    let mut lefts: Vec<f64> = Vec::new();
    for c in &cuts {
        let b = c.geometric_bounds().unwrap();
        assert!((b.width() - 30.0 * MM).abs() < 1e-6 && (b.height() - 50.0 * MM).abs() < 1e-6, "{b:?}");
        assert!(((ab.y1 - b.y1) / MM - 25.0).abs() < 2e-3, "{b:?}");
        lefts.push((b.x0 - ab.x0) / MM);
        assert!(arts.iter().any(|a| (a.visual_bounds().unwrap().center() - b.center()).hypot() < 1e-6), "bài không nằm giữa khuôn");
    }
    lefts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (g, w) in lefts.iter().zip([13.0, 45.0, 77.0]) {
        assert!((g - w).abs() < 2e-3, "{lefts:?}");
    }
    // Bài xoay theo khuôn: 40 × 20 thành 20 × 40.
    let a0 = arts[0].geometric_bounds().unwrap();
    assert!((a0.width() - 20.0 * MM).abs() < 1e-6 && (a0.height() - 40.0 * MM).abs() < 1e-6, "{a0:?}");
    // Nguồn giữ nguyên, vùng chọn bỏ trống.
    assert!(d.node(die).is_some() && d.node(art).is_some());
    assert!(s.doc().unwrap().selection.objects.is_empty());
    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.layers.len(), layers_before);
    assert_eq!(d.artboards.len(), 1);
    assert!((d.artboards[0].rect.width() - 800.0).abs() < 1e-9);
}

#[test]
fn dan_be_hai_mat_lat_ngang() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    // Cùng hàng, trái → phải: Khuôn, mặt trước, mặt sau.
    let die = rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    let front = rect_at(&mut s, 300.0, 100.0, 40.0, 20.0);
    let back = rect_at(&mut s, 480.0, 100.0, 30.0, 10.0);
    run(&mut s, "select.set", json!({"ids": [back.0, die.0, front.0]}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": 12, "paperH": 10, "ponLeft": 15, "twoSided": true}));
    assert!(r["count"].as_u64().unwrap() >= 1, "{r}");
    assert!(r["message"].as_str().unwrap().contains("Hai mặt dùng chung bố cục khuôn"), "{r}");
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 2);
    assert_eq!((d.artboards[0].name.as_str(), d.artboards[1].name.as_str()), ("Dan be - mặt trước", "Dan be - mặt sau"));
    let (fa, ba) = (d.artboards[0].rect, d.artboards[1].rect);
    assert!((ba.x0 - fa.x1 - 10.0 * MM).abs() < 1e-9 && (ba.y0 - fa.y0).abs() < 1e-9);
    // Khuôn mặt sau đối xứng qua trục dọc giữa tờ, cùng độ cao.
    let fronts = kids(layer_named(d, "Dàn bế - Khuôn - mặt trước"));
    let backs = kids(layer_named(d, "Dàn bế - Khuôn - mặt sau"));
    assert_eq!(fronts.len(), backs.len());
    for f in &fronts {
        let c = f.geometric_bounds().unwrap().center();
        let want = Point::new(ba.x0 + fa.width() - (c.x - fa.x0), c.y);
        assert!(backs.iter().any(|b| (b.geometric_bounds().unwrap().center() - want).hypot() < 1e-6), "mặt sau không đối xứng");
    }
    // PON mặt sau lật ngang: trái 15 mm thành phải 15 mm.
    let mut xs: Vec<f64> =
        kids(layer_named(d, "Dàn bế - PON - mặt sau")).iter().map(|n| (n.geometric_bounds().unwrap().center().x - ba.x0) / MM).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (g, w) in xs.iter().zip([10.0, 10.0, 105.0, 105.0]) {
        assert!((g - w).abs() < 1e-6, "{xs:?}");
    }
    // Bài mặt sau là mặt sau nguồn (30 × 10 mm), xoay ngược nhưng không lật.
    let back_arts = kids(layer_named(d, "Dàn bế - Bài - mặt sau"));
    assert_eq!(back_arts.len(), fronts.len());
    let src = d.node(back).unwrap().geometric_bounds().unwrap();
    for b in &back_arts {
        let g = b.geometric_bounds().unwrap();
        let same = (g.width() - src.width()).abs() < 1e-6 && (g.height() - src.height()).abs() < 1e-6;
        let turned = (g.width() - src.height()).abs() < 1e-6 && (g.height() - src.width()).abs() < 1e-6;
        assert!(same || turned, "{g:?}");
    }
}

#[test]
fn dan_be_theo_layer_khi_khong_chon() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    run(&mut s, "layer.new", json!({"name": "Khuôn bế"}));
    rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    run(&mut s, "layer.new", json!({"name": "Bài"}));
    rect_at(&mut s, 125.0, 100.0, 40.0, 20.0);
    run(&mut s, "select.none", json!({}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10"}));
    assert_eq!(r["count"], 3, "{r}");
    // Lần hai: layer kết quả cũ không bị nhận nhầm là layer nguồn; tên mới thêm số.
    run(&mut s, "select.none", json!({}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10"}));
    assert_eq!(r["count"], 3, "{r}");
    let d = &s.doc().unwrap().doc;
    assert!(d.layers.iter().any(|l| l.name.as_deref() == Some("Dàn bế - PON 2")));
    assert_eq!(d.artboards.len(), 1, "artboard của lần trước cũng bị xoá");
}

#[test]
fn dan_be_bao_loi_nhu_dancard() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let a = rect_at(&mut s, 100.0, 100.0, 35.0, 20.0);
    let b = rect_at(&mut s, 250.0, 100.0, 30.0, 15.0);
    let c = rect_at(&mut s, 400.0, 100.0, 30.0, 15.0);
    let err = |s: &mut Session, p: Value| s.execute("dancard.dieLayout", &p).unwrap_err().to_string();
    run(&mut s, "select.set", json!({"ids": [a.0, b.0, c.0]}));
    assert_eq!(err(&mut s, json!({})), "Hãy chọn số object chẵn: từng cặp Khuôn bên trái, Bài bên phải.");
    assert_eq!(err(&mut s, json!({"gap": "abc"})), "Khe giữa 2 khuôn phải là số.");
    assert_eq!(err(&mut s, json!({"margin": 200})), "Khuôn cách mép tờ nằm ngoài phạm vi cho phép.");
    assert_eq!(
        err(&mut s, json!({"paperW": "1", "paperH": "1"})),
        "Khổ tờ và khoảng PON không hợp lệ: bốn chấm phải nằm trong tờ, không chồng nhau."
    );
    run(&mut s, "select.set", json!({"ids": [a.0, b.0]}));
    assert_eq!(err(&mut s, json!({"twoSided": true})), "Dàn bế 2 mặt cần chọn đủ bộ 3: Khuôn bên trái → mặt trước → mặt sau, cùng hàng.");
    assert_eq!(
        err(&mut s, json!({"paperW": "3", "paperH": "3", "ponTop": 2.5, "ponBottom": 2.5, "ponLeft": 2.5, "ponRight": 2.5, "margin": 0})),
        "Khuôn loại 1 không vừa khổ tờ sau khi trừ lề, ở cả 4 hướng xoay."
    );
    // Không chọn gì và không có layer Khuôn bế / Bài.
    run(&mut s, "select.none", json!({}));
    assert_eq!(err(&mut s, json!({})), "Chọn từng cặp Khuôn/Bài, hoặc tạo hai layer Khuôn bế và Bài.");
    // Khuôn (bên trái, cùng hàng) là đường hở.
    let open =
        id_of(&run(&mut s, "path.create", json!({"anchors": [{"x": 50, "y": 90}, {"x": 150, "y": 90}, {"x": 150, "y": 110}], "closed": false})));
    run(&mut s, "select.set", json!({"ids": [open.0, b.0]}));
    assert_eq!(err(&mut s, json!({})), "Khuôn bế phải là path khép kín; không dùng khung bao thay thế.");
}

/// Khuôn và bài đặt nghiêng 12° (xoay trong app): dựng thẳng theo khung transform rồi mới xếp, nên
/// ra đúng bố cục của khuôn thẳng (3 khuôn xoay 90°, 30 × 50 mm, bài 20 × 40 nằm giữa khuôn).
#[test]
fn dan_be_khuon_nghieng_dung_thang_theo_transform() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let die = rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    let art = rect_at(&mut s, 360.0, 98.0, 40.0, 20.0);
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    run(&mut s, "object.transformEach", json!({"rotate": 12}));
    for id in [die, art] {
        assert!((s.doc().unwrap().doc.node(id).unwrap().bbox_angle - 12.0).abs() < 1e-9);
    }
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10", "ponClear": "7,5"}));
    assert_eq!(r["count"], 3, "{r}");
    let d = &s.doc().unwrap().doc;
    let cuts = kids(layer_named(d, "Dàn bế - Khuôn"));
    let arts = kids(layer_named(d, "Dàn bế - Bài"));
    assert_eq!((cuts.len(), arts.len()), (3, 3));
    for c in &cuts {
        let b = c.geometric_bounds().unwrap();
        assert!((b.width() - 30.0 * MM).abs() < 1e-6 && (b.height() - 50.0 * MM).abs() < 1e-6, "khuôn phải thẳng: {b:?}");
    }
    for a in &arts {
        let b = a.geometric_bounds().unwrap();
        assert!((b.width() - 20.0 * MM).abs() < 1e-6 && (b.height() - 40.0 * MM).abs() < 1e-6, "bài phải thẳng theo khuôn: {b:?}");
        assert!(cuts.iter().any(|c| (c.geometric_bounds().unwrap().center() - b.center()).hypot() < 1e-6), "bài không nằm giữa khuôn");
    }
}

/// Khuôn và bài nghiêng 12° trong file .ai (không lưu góc, góc dò từ hình dạng): dựng thẳng như khuôn
/// xoay trong app, và khung transform của các bản sao đúng hướng dàn (0/90/180/270°), không còn
/// lệch −12°.
#[test]
fn dan_be_khuon_nghieng_do_tu_file_ai_khung_dung_huong() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let die = rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    let art = rect_at(&mut s, 360.0, 98.0, 40.0, 20.0);
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    run(&mut s, "object.transformEach", json!({"rotate": 12}));
    run(&mut s, "object.resetBoundingBox", json!({}));
    for id in [die, art] {
        assert_eq!(s.doc().unwrap().doc.node(id).unwrap().bbox_angle, 0.0, "như file .ai: không lưu góc");
    }
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10", "ponClear": "7,5"}));
    assert_eq!(r["count"], 3, "{r}");
    let d = &s.doc().unwrap().doc;
    let quarter = |a: f64| ((a / 90.0).round() * 90.0 - a).abs() < 1e-6;
    for c in kids(layer_named(d, "Dàn bế - Khuôn")) {
        let b = c.geometric_bounds().unwrap();
        assert!((b.width() - 30.0 * MM).abs() < 1e-6 && (b.height() - 50.0 * MM).abs() < 1e-6, "khuôn phải thẳng: {b:?}");
        assert!(quarter(c.bbox_angle), "khung transform khuôn đúng hướng dàn: {}", c.bbox_angle);
    }
    for a in kids(layer_named(d, "Dàn bế - Bài")) {
        let b = a.geometric_bounds().unwrap();
        assert!((b.width() - 20.0 * MM).abs() < 1e-6 && (b.height() - 40.0 * MM).abs() < 1e-6, "bài phải thẳng theo khuôn: {b:?}");
        assert!(quarter(a.bbox_angle), "khung transform bài đúng hướng dàn: {}", a.bbox_angle);
    }
}

/// Khuôn xoay 12° ngay trong app (góc đã lưu) còn Bài lấy từ file .ai (nghiêng 12° nhưng không lưu
/// góc): Bài bỏ cùng góc với khuôn, khung transform của từng bản Bài theo đúng hướng dàn.
#[test]
fn dan_be_khuon_luu_goc_bai_tu_file_ai() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let die = rect_at(&mut s, 120.0, 100.0, 50.0, 30.0);
    let art = rect_at(&mut s, 360.0, 98.0, 40.0, 20.0);
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    run(&mut s, "object.transformEach", json!({"rotate": 12}));
    run(&mut s, "select.set", json!({"ids": [art.0]}));
    run(&mut s, "object.resetBoundingBox", json!({}));
    let d = &s.doc().unwrap().doc;
    assert!((d.node(die).unwrap().bbox_angle - 12.0).abs() < 1e-9 && d.node(art).unwrap().bbox_angle == 0.0);
    run(&mut s, "select.set", json!({"ids": [die.0, art.0]}));
    let r = run(&mut s, "dancard.dieLayout", json!({"paperW": "12", "paperH": "10", "ponClear": "7,5"}));
    assert_eq!(r["count"], 3, "{r}");
    let d = &s.doc().unwrap().doc;
    let quarter = |a: f64| ((a / 90.0).round() * 90.0 - a).abs() < 1e-6;
    for (layer, (w, h)) in [("Dàn bế - Khuôn", (30.0, 50.0)), ("Dàn bế - Bài", (20.0, 40.0))] {
        for c in kids(layer_named(d, layer)) {
            let b = c.geometric_bounds().unwrap();
            assert!((b.width() - w * MM).abs() < 1e-6 && (b.height() - h * MM).abs() < 1e-6, "{layer}: {b:?}");
            assert!(quarter(c.bbox_angle), "{layer}: khung transform đúng hướng dàn, không lệch 12°: {}", c.bbox_angle);
        }
    }
}
