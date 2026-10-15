//! Dàn bế: bộ test gốc của DanCard (`tests/dan_be_*.test.js`) chuyển sang Rust.
//!
//! Cùng dữ liệu mẫu (dựng lại đúng như các file fixture JS), cùng mốc số con, cùng các phép kiểm tra
//! hình học độc lập: đường bao được dựng lại từ đầu vào và kết quả công khai rồi đo khe, lề, né PON,
//! chồng lấn bằng hình học liên tục, không dùng chốt chặn của chính lõi dàn. Mốc số con là mốc của
//! bản gốc; chỗ nào bản JS đo được nhiều hơn mốc trên máy (log khi chạy bộ test gốc) thì giữ cả số
//! đó làm mốc dưới, để bản Rust không bao giờ kém DanCard hiện tại.

use std::collections::HashMap;

use super::nester::{self, ANGLES, Clock, Dot, DotIn, Groups, NestInput, NestOut, OutSlot, PSlot, Sheet, Slot, TypeIn, Variant};

type P = [f64; 2];

// ---------------------------------------------------------------- dữ liệu mẫu (fixtures JS)

/// Hằng số MM của các file fixture JS (khác 72/25,4 ở chữ số cuối, giữ đúng như bản gốc).
const MM_JS: f64 = 2.834645669;

fn bez(a: P, b: P, c: P, d: P, t: f64) -> P {
    let u = 1.0 - t;
    [
        u * u * u * a[0] + 3.0 * u * u * t * b[0] + 3.0 * u * t * t * c[0] + t * t * t * d[0],
        u * u * u * a[1] + 3.0 * u * u * t * b[1] + 3.0 * u * t * t * c[1] + t * t * t * d[1],
    ]
}

/// `dan_be_fixtures.js` › `spoon()`: đường viền cái muỗng đo từ file của người dùng.
fn spoon() -> Groups {
    let mut pts: Vec<P> = Vec::new();
    let line = |pts: &mut Vec<P>, p: P| pts.push([p[0] / MM_JS, p[1] / MM_JS]);
    let cubic = |pts: &mut Vec<P>, a: P, b: P, c: P, d: P| {
        for i in 1..=32 {
            let p = bez(a, b, c, d, f64::from(i) / 32.0);
            pts.push([p[0] / MM_JS, p[1] / MM_JS]);
        }
    };
    line(&mut pts, [514.9791259765625, 344.3480224609375]);
    cubic(
        &mut pts,
        [514.9791259765625, 344.3480224609375],
        [514.9791259765625, 380.5780334472656],
        [494.1991271972656, 411.9580078125],
        [463.90911865234375, 427.218017578125],
    );
    line(&mut pts, [463.90911865234375, 742.6110229492188]);
    line(&mut pts, [380.5691223144531, 742.6110229492188]);
    line(&mut pts, [380.5691223144531, 427.218017578125]);
    cubic(
        &mut pts,
        [380.5691223144531, 427.218017578125],
        [350.27911376953125, 411.9580078125],
        [329.4991455078125, 380.5780334472656],
        [329.4991455078125, 344.3480224609375],
    );
    cubic(
        &mut pts,
        [329.4991455078125, 344.3480224609375],
        [329.4991455078125, 293.1280212402344],
        [371.01910400390625, 251.6080322265625],
        [422.2391357421875, 251.6080322265625],
    );
    cubic(
        &mut pts,
        [422.2391357421875, 251.6080322265625],
        [473.4591369628906, 251.6080322265625],
        [514.9791259765625, 293.1280212402344],
        [514.9791259765625, 344.3480224609375],
    );
    vec![vec![pts]]
}

/// `dan_be_fixtures.js` › `circle(radius, count = 96)`.
fn circle_poly(radius: f64, count: usize) -> Groups {
    let pts = (0..count)
        .map(|i| {
            let a = (i as f64 * std::f64::consts::PI * 2.0) / count as f64;
            [radius + radius * a.cos(), radius + radius * a.sin()]
        })
        .collect();
    vec![vec![pts]]
}

fn circle(radius: f64) -> Groups {
    circle_poly(radius, 96)
}

/// `dan_be_fixtures.js` › `leaf()`.
fn leaf() -> Groups {
    enum C {
        L(P),
        B(P, P, P, P),
    }
    let commands = [
        C::B(
            [106.73069763183594, 173.57888793945312],
            [94.67369842529297, 173.57888793945312],
            [83.46269989013672, 168.16189575195312],
            [75.97270202636719, 158.71688842773438],
        ),
        C::L([32.48069763183594, 103.8648910522461]),
        C::B(
            [32.48069763183594, 103.8648910522461],
            [31.264694213867188, 102.33088684082031],
            [31.25969696044922, 100.16288757324219],
            [32.469696044921875, 98.62288665771484],
        ),
        C::L([74.6427001953125, 44.9578857421875]),
        C::B(
            [74.6427001953125, 44.9578857421875],
            [82.439697265625, 35.035888671875],
            [94.31069946289062, 28.921890258789062],
            [106.93270111083984, 29.012893676757812],
        ),
        C::B(
            [106.93270111083984, 29.012893676757812],
            [119.21170043945312, 29.101882934570312],
            [130.57969665527344, 34.68388366699219],
            [138.15769958496094, 44.35888671875],
        ),
        C::L([180.90170288085938, 98.92688751220703]),
        C::B(
            [180.90170288085938, 98.92688751220703],
            [182.10870361328125, 100.46788787841797],
            [182.1007080078125, 102.6358871459961],
            [180.88169860839844, 104.16888427734375],
        ),
        C::L([137.44869995117188, 158.7668914794922]),
        C::B(
            [137.44869995117188, 158.7668914794922],
            [129.95970153808594, 168.1798858642578],
            [118.7636947631836, 173.57888793945312],
            [106.73069763183594, 173.57888793945312],
        ),
    ];
    let mut pts: Vec<P> = vec![[106.73069763183594 / MM_JS, 173.57888793945312 / MM_JS]];
    for c in &commands {
        match c {
            C::L(p) => pts.push([p[0] / MM_JS, p[1] / MM_JS]),
            C::B(a, b, cc, d) => {
                for i in 1..=32 {
                    let p = bez(*a, *b, *cc, *d, f64::from(i) / 32.0);
                    pts.push([p[0] / MM_JS, p[1] / MM_JS]);
                }
            }
        }
    }
    vec![vec![pts]]
}

/// Một điểm neo kiểu Illustrator: điểm, tay trái, tay phải.
#[derive(Clone, Copy)]
struct Node3 {
    a: P,
    l: P,
    r: P,
}

fn n3(a: P, l: P, r: P) -> Node3 {
    Node3 { a, l, r }
}

fn mid(a: P, b: P) -> P {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

/// Khoảng cách tới dây cung hữu hạn (như `chordDistance` / `finiteChordDistance` của fixture).
fn chord(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let den = dx * dx + dy * dy;
    let t = if den != 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / den).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Làm phẳng các đoạn bậc ba của `nodes` (khép kín) bằng chia đôi; `dist` đo độ phẳng, dừng khi
/// ≤ `flat` hoặc tới `max_depth` (`stop_at_depth`: dừng và nhận điểm như fixture cap / biến đổi).
fn flatten_nodes(nodes: &[Node3], flat: f64, max_depth: u32, dist: fn(P, P, P) -> f64) -> Vec<P> {
    #[allow(clippy::too_many_arguments)]
    fn rec(a: P, b: P, c: P, d: P, depth: u32, flat: f64, max_depth: u32, dist: fn(P, P, P) -> f64, out: &mut Vec<P>) {
        if depth >= max_depth || dist(b, a, d).max(dist(c, a, d)) <= flat {
            out.push(d);
            return;
        }
        let (ab, bc, cd) = (mid(a, b), mid(b, c), mid(c, d));
        let (abc, bcd) = (mid(ab, bc), mid(bc, cd));
        let m = mid(abc, bcd);
        rec(a, ab, abc, m, depth + 1, flat, max_depth, dist, out);
        rec(m, bcd, cd, d, depth + 1, flat, max_depth, dist, out);
    }
    let mut out = vec![nodes[0].a];
    for (i, n) in nodes.iter().enumerate() {
        let next = nodes[(i + 1) % nodes.len()];
        rec(n.a, n.r, next.l, next.a, 0, flat, max_depth, dist, &mut out);
    }
    out
}

fn normalized(pts: Vec<P>) -> Groups {
    let x0 = pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let y0 = pts.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    vec![vec![pts.into_iter().map(|p| [p[0] - x0, p[1] - y0]).collect()]]
}

/// `dan_be_cap_fixture.js`: khuôn bế bảy nút cong 53 × 26,51 mm.
fn cap() -> Groups {
    let nodes = [
        n3([52.677234043278226, 26.51160434369366], [53.10761259692489, 25.971844008351724], [52.677234043278226, 26.51160434369366]),
        n3([0.3250032029364609, 26.405775316718717], [0.3250032029364609, 26.405775316718717], [-0.105375350710203, 25.862483758503156]),
        n3([0.3179407571892073, 24.557223206068656], [-0.10890657358382982, 25.10049323243752], [0.3179407571892073, 24.557223206068656]),
        n3([15.198126373410581, 5.623624367859679], [15.198126373410581, 5.623624367859679], [17.946257511116812, 2.124074840861531]),
        n3([26.58931306756068, 0.00034572823006419296], [22.13372801122185, -0.03139221393896128], [30.921412982556806, 0.03210520224593271]),
        n3([37.603024955628435, 5.411966313909974], [34.92898490272789, 2.0006112313653697], [37.603024955628435, 5.411966313909974]),
        n3([52.68427495717845, 24.6630522330436], [52.68427495717845, 24.6630522330436], [53.10761259692489, 25.20632225941245]),
    ];
    // Khoảng cách tới đường thẳng vô hạn (trùng điểm thì tới điểm), như `distance` của fixture.
    fn line_dist(p: P, a: P, b: P) -> f64 {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        if len != 0.0 { (dy * (p[0] - a[0]) - dx * (p[1] - a[1])).abs() / len } else { (p[0] - a[0]).hypot(p[1] - a[1]) }
    }
    normalized(flatten_nodes(&nodes, 0.005, 14, line_dist))
}

fn quarter(p: P, angle: i64) -> P {
    match angle {
        90 => [-p[1], p[0]],
        180 => [-p[0], -p[1]],
        270 => [p[1], -p[0]],
        _ => p,
    }
}

/// `dan_be_arch_fixture.js` › `curveFor(width, height, flatness, angle)`: vòm bo sáu nút.
fn arch(width: u32, height: u32, flat: f64, angle: i64) -> Groups {
    let nodes: Vec<Node3> = match (width, height) {
        (42, 72) => vec![
            n3([42.000000004473236, 0.0], [42.000000004473236, 0.0], [42.000000004473236, 0.0]),
            n3([0.0, 0.0], [0.0, 0.0], [0.0, 0.0]),
            n3([5.715132071884461e-10, 53.625550295586585], [5.715132071884461e-10, 53.625550295586585], [6.773133363507704e-10, 63.77347865798609]),
            n3([18.374449712578855, 72.00000000739924], [8.226521350182226, 72.00000000739993], [18.374449712578855, 72.00000000739924]),
            n3([23.625550291979007, 72.00000000739924], [23.625550291979007, 72.00000000739924], [33.77347865495429, 72.00000000739888]),
            n3([42.00000000480836, 53.625550294452765], [42.00000000487189, 63.77347865742658], [42.00000000480836, 53.625550294452765]),
        ],
        (40, 70) => vec![
            n3([40.00000000425803, 0.0], [40.00000000425803, 0.0], [40.00000000425803, 0.0]),
            n3([0.0, 0.0], [0.0, 0.0], [0.0, 0.0]),
            n3([5.36219760007686e-10, 51.62555029537815], [5.36219760007686e-10, 51.62555029537815], [6.420198891700102e-10, 61.77347865777941]),
            n3([18.37444971254709, 70.00000000719537], [8.226521350146932, 70.00000000719608], [18.37444971254709, 70.00000000719537]),
            n3([21.625550291746155, 70.00000000719503], [21.625550291746155, 70.00000000719503], [31.773478654721433, 70.00000000719396]),
            n3([40.00000000457904, 51.62555029424961], [40.00000000463904, 61.77347865722307], [40.00000000457904, 51.62555029424961]),
        ],
        _ => panic!("vòm không có trong fixture"),
    };
    let mut pts = flatten_nodes(&nodes, flat, 64, chord);
    pts.pop();
    normalized(pts.into_iter().map(|p| quarter(p, angle)).collect())
}

/// `dan_be_circle_fixture.js`: bốn cung bậc ba kiểu Illustrator (κ chuẩn).
const KAPPA: f64 = 0.5522847498307936;

fn cubic_circle_nodes(diameter: f64) -> Vec<Node3> {
    let r = diameter / 2.0;
    let k = KAPPA * r;
    vec![
        n3([diameter, r], [diameter, r - k], [diameter, r + k]),
        n3([r, diameter], [r + k, diameter], [r - k, diameter]),
        n3([0.0, r], [0.0, r + k], [0.0, r - k]),
        n3([r, 0.0], [r - k, 0.0], [r + k, 0.0]),
    ]
}

fn cubic_circle(diameter: f64, flat: f64) -> Groups {
    let mut pts = flatten_nodes(&cubic_circle_nodes(diameter), flat, 64, chord);
    pts.pop();
    normalized(pts)
}

/// `dan_be_variable_fixture.js`: đường cong kín bảy nút 46,086 × 122 mm.
fn curve_nodes() -> [Node3; 7] {
    [
        n3([46.08638720757836, 98.9568064087484], [46.08638720757836, 111.68348125861304], [46.08638720757836, 89.95469352801146]),
        n3([33.396989066601755, 78.36601988176393], [40.92316872410234, 82.15767386071136], [33.396989066601755, 78.36601988176393]),
        n3([33.396989066601755, 0.0], [33.396989066601755, 0.0], [33.396989066601755, 0.0]),
        n3([12.689413306436784, 0.0], [12.689413306436784, 0.0], [12.689413306436784, 0.0]),
        n3([12.689413306436784, 78.36601988176393], [12.689413306436784, 78.36601988176393], [5.163233648938289, 82.15767386071136]),
        n3([0.0, 98.9568064087484], [0.0, 89.95469352801146], [0.0, 111.68348125861304]),
        n3([23.04319360378934, 122.0000000125377], [10.316518753918722, 122.0000000125377], [35.76988361912367, 122.0000000125377]),
    ]
}

fn curve(scale: f64, flat: f64) -> Groups {
    fn point_line(p: P, a: P, b: P) -> f64 {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let distance = dx.hypot(dy);
        let t = if distance > 1e-12 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (distance * distance)).clamp(0.0, 1.0) } else { 0.0 };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    }
    let s = |p: P| [p[0] * scale, p[1] * scale];
    let nodes: Vec<Node3> = curve_nodes().iter().map(|n| n3(s(n.a), s(n.l), s(n.r))).collect();
    let mut pts = flatten_nodes(&nodes, flat, 18, point_line);
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    if (last[0] - first[0]).hypot(last[1] - first[1]) < 0.00001 / 2.834645669 {
        pts.pop();
    }
    normalized(pts)
}

/// `dan_be_native_circle.test.js` › `illustratorCircle`: bốn cung, mỗi cung 32 điểm (t = i/32).
fn illustrator_circle(diameter: f64) -> Groups {
    let (r, k) = (diameter / 2.0, KAPPA);
    let anchors = [[0.0, r], [r, 2.0 * r], [2.0 * r, r], [r, 0.0]];
    let controls = [
        [[0.0, r + k * r], [r - k * r, 2.0 * r]],
        [[r + k * r, 2.0 * r], [2.0 * r, r + k * r]],
        [[2.0 * r, r - k * r], [r + k * r, 0.0]],
        [[r - k * r, 0.0], [0.0, r - k * r]],
    ];
    let mut pts = Vec::new();
    for q in 0..4 {
        for i in 0..32 {
            pts.push(bez(anchors[q], controls[q][0], controls[q][1], anchors[(q + 1) % 4], f64::from(i) / 32.0));
        }
    }
    vec![vec![pts]]
}

fn rect(w: f64, h: f64) -> Groups {
    vec![vec![vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]]]
}

fn poly(pts: &[P]) -> Groups {
    vec![vec![pts.to_vec()]]
}

fn rotated(groups: &Groups, angle: i64) -> Groups {
    groups.iter().map(|g| g.iter().map(|c| c.iter().map(|p| quarter(*p, angle)).collect()).collect()).collect()
}

// ---------------------------------------------------------------- đầu vào, gọi lõi dàn

/// Chấm PON 5 mm cách mép 10 mm trên tờ 330 × 354 (payload Prepare mặc định).
const STD_DOTS: [(f64, f64, f64); 4] = [(10.0, 10.0, 2.5), (320.0, 10.0, 2.5), (10.0, 344.0, 2.5), (320.0, 344.0, 2.5)];
/// `f.input` của `dan_be_fixtures.js`: chấm bán kính 1 mm cách mép 5 mm.
const FIX_DOTS: [(f64, f64, f64); 4] = [(5.0, 5.0, 1.0), (325.0, 5.0, 1.0), (5.0, 349.0, 1.0), (325.0, 349.0, 1.0)];

fn dots_of(list: &[(f64, f64, f64)]) -> Vec<DotIn> {
    list.iter().map(|&(x, y, r)| DotIn { x, y, r }).collect()
}

fn types_of(list: &[Groups], curve_error: f64) -> Vec<TypeIn> {
    list.iter().map(|g| TypeIn { groups: g.clone(), curve_error }).collect()
}

/// Lề 4 mm, né PON 7,5 mm, lưới 0,25 mm như payload của Prepare.
fn input(w: f64, h: f64, dots: &[(f64, f64, f64)], types: Vec<TypeIn>, gap: f64, budget: f64) -> NestInput {
    NestInput { width: w, height: h, dots: dots_of(dots), types, gap, margin: 4.0, pon_clear: 7.5, step: 0.25, budget_ms: budget }
}

fn nest(inp: &NestInput) -> NestOut {
    nester::nest(inp, &Clock::default()).unwrap_or_else(|e| panic!("{e}"))
}

/// Bốn biến thể xoay của mỗi loại, ở khe của đầu vào (như `_test.buildVariant` trong test gốc).
fn variants_for(inp: &NestInput, with_curve: bool) -> Vec<Variant> {
    let ck = Clock::default();
    let mut vs = Vec::new();
    for (mi, t) in inp.types.iter().enumerate() {
        for (vi, &a) in ANGLES.iter().enumerate() {
            let mut v = nester::build_variant(&t.groups, mi, vi, a, inp.gap, inp.step, &ck).expect("biến thể xoay");
            if with_curve {
                v.curve_error = t.curve_error;
            }
            vs.push(v);
        }
    }
    vs
}

fn cell_dots(inp: &NestInput) -> Vec<Dot> {
    inp.dots
        .iter()
        .map(|d| Dot { x: (d.x - inp.margin) / inp.step, y: (d.y - inp.margin) / inp.step, radius: (d.r + inp.pon_clear) / inp.step })
        .collect()
}

fn area(inp: &NestInput) -> (f64, f64) {
    ((inp.width - 2.0 * inp.margin) / inp.step, (inp.height - 2.0 * inp.margin) / inp.step)
}

fn grid_sheet(inp: &NestInput) -> Sheet {
    let cells = |v: f64| ((v - 2.0 * inp.margin) / inp.step + 1e-7).floor() as i64;
    Sheet { rw: cells(inp.width), rh: cells(inp.height), dots: cell_dots(inp) }
}

/// Kết quả công khai → phương án vật lý (ô, lẻ ô) để đưa vào `verify_geometry`.
fn pplan(inp: &NestInput, slots: &[OutSlot]) -> Vec<PSlot> {
    slots.iter().map(|s| PSlot { mi: s.mi, v: s.mi * 4 + s.vi, x: (s.x - inp.margin) / inp.step, y: (s.y - inp.margin) / inp.step }).collect()
}

/// `_test.verifyGeometry` của test gốc trên kết quả công khai.
fn verify_out(inp: &NestInput, out: &NestOut) -> bool {
    let vs = variants_for(inp, false);
    nester::verify_geometry(&pplan(inp, &out.slots), &vs, area(inp), &cell_dots(inp), inp.gap, inp.step, &Clock::default())
}

fn out_slots(inp: &NestInput, plan: &[Slot], vs: &[Variant]) -> Vec<OutSlot> {
    plan.iter().map(|s| OutSlot { mi: s.mi, vi: vs[s.v].vi, x: inp.margin + s.x as f64 * inp.step, y: inp.margin + s.y as f64 * inp.step }).collect()
}

fn pout_slots(inp: &NestInput, plan: &[PSlot], vs: &[Variant]) -> Vec<OutSlot> {
    plan.iter().map(|s| OutSlot { mi: s.mi, vi: vs[s.v].vi, x: inp.margin + s.x * inp.step, y: inp.margin + s.y * inp.step }).collect()
}

/// Số con từng loại khớp các con thật; chỉ số và hướng hợp lệ.
fn assert_counts(inp: &NestInput, out: &NestOut, label: &str) {
    let mut c = vec![0usize; inp.types.len()];
    for s in &out.slots {
        assert!(s.mi < c.len() && s.vi < 4, "{label}: chỉ số con");
        c[s.mi] += 1;
    }
    assert_eq!(out.counts, c, "{label}: số con từng mẫu khớp các con");
}

fn spread(counts: &[usize]) -> usize {
    counts.iter().max().unwrap_or(&0) - counts.iter().min().unwrap_or(&0)
}

// ---------------------------------------------------------------- kiểm tra hình học độc lập

const EPS: f64 = 1e-7;
const TOL: f64 = 0.001;
const BIN: f64 = 4.0;

/// Một khuôn đặt trên tờ, dựng lại từ đường bao gốc: xoay một phần tư, dời góc dưới trái khung
/// về (x, y). Cạnh được chia theo dải y (tia chẵn/lẻ) và theo ô (tìm cạnh gần), mỗi ô 4 mm.
struct Shape {
    vertices: Vec<P>,
    edges: Vec<(P, P)>,
    rows: Vec<HashMap<i64, Vec<usize>>>,
    bins: HashMap<(i64, i64), Vec<usize>>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn cell(v: f64) -> i64 {
    (v / BIN).floor() as i64
}

fn shape(groups: &Groups, angle: i64, x: f64, y: f64) -> Shape {
    let turned = rotated(groups, angle);
    let (mut minx, mut miny) = (f64::INFINITY, f64::INFINITY);
    for p in turned.iter().flatten().flatten() {
        minx = minx.min(p[0]);
        miny = miny.min(p[1]);
    }
    let placed: Groups = turned.iter().map(|g| g.iter().map(|c| c.iter().map(|p| [p[0] - minx + x, p[1] - miny + y]).collect()).collect()).collect();
    let (mut vertices, mut edges, mut rows_all) = (Vec::new(), Vec::new(), Vec::new());
    let mut bins: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for g in &placed {
        let mut rows: HashMap<i64, Vec<usize>> = HashMap::new();
        for c in g {
            for (i, p) in c.iter().enumerate() {
                let q = c[(i + 1) % c.len()];
                let ei = edges.len();
                edges.push((*p, q));
                vertices.push(*p);
                for r in cell(p[1].min(q[1]))..=cell(p[1].max(q[1])) {
                    rows.entry(r).or_default().push(ei);
                    for cx in cell(p[0].min(q[0]))..=cell(p[0].max(q[0])) {
                        bins.entry((cx, r)).or_default().push(ei);
                    }
                }
            }
        }
        rows_all.push(rows);
    }
    let x0 = vertices.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let y0 = vertices.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let x1 = vertices.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
    let y1 = vertices.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    Shape { vertices, edges, rows: rows_all, bins, x0, y0, x1, y1 }
}

fn slot_shape(groups: &Groups, s: &OutSlot) -> Shape {
    shape(groups, ANGLES[s.vi], s.x, s.y)
}

fn point_segment(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx * dx + dy * dy;
    let t = if len != 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn cross(a: P, b: P, c: P) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn proper_cross(a: P, b: P, c: P, d: P) -> bool {
    let (ac, ad, ca, cb) = (cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b));
    ((ac > EPS && ad < -EPS) || (ac < -EPS && ad > EPS)) && ((ca > EPS && cb < -EPS) || (ca < -EPS && cb > EPS))
}

fn seg_distance(a: P, b: P, c: P, d: P) -> f64 {
    if proper_cross(a, b, c, d) {
        return 0.0;
    }
    point_segment(a, c, d).min(point_segment(b, c, d)).min(point_segment(c, a, b)).min(point_segment(d, a, b))
}

/// Điểm nằm hẳn trong vùng tô (chẵn/lẻ trong một nhóm, hợp giữa các nhóm); nằm trên biên thì không.
fn inside(p: P, s: &Shape) -> bool {
    if p[0] < s.x0 - EPS || p[0] > s.x1 + EPS || p[1] < s.y0 - EPS || p[1] > s.y1 + EPS {
        return false;
    }
    for rows in &s.rows {
        let Some(es) = rows.get(&cell(p[1])) else { continue };
        let (mut parity, mut boundary) = (false, false);
        for &e in es {
            let (a, b) = s.edges[e];
            if p[0] >= a[0].min(b[0]) - EPS && p[0] <= a[0].max(b[0]) + EPS && point_segment(p, a, b) <= EPS {
                boundary = true;
                break;
            }
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) {
                parity = !parity;
            }
        }
        if parity && !boundary {
            return true;
        }
    }
    false
}

type BoxF = (f64, f64, f64, f64);

fn box_gap(a: BoxF, b: BoxF) -> f64 {
    (a.0 - b.2).max(b.0 - a.2).max(0.0).hypot((a.1 - b.3).max(b.1 - a.3).max(0.0))
}

fn sbox(s: &Shape) -> BoxF {
    (s.x0, s.y0, s.x1, s.y1)
}

fn ebox(a: P, b: P) -> BoxF {
    (a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1]))
}

fn near_edges(s: &Shape, b: BoxF, r: f64, out: &mut Vec<usize>) {
    out.clear();
    for y in cell(b.1 - r)..=cell(b.3 + r) {
        for x in cell(b.0 - r)..=cell(b.2 + r) {
            if let Some(es) = s.bins.get(&(x, y)) {
                for &e in es {
                    let (p, q) = s.edges[e];
                    if box_gap(b, ebox(p, q)) <= r && !out.contains(&e) {
                        out.push(e);
                    }
                }
            }
        }
    }
}

/// Khoảng cách thật giữa hai đường bao: cận trên từ vài đỉnh, rồi mọi cặp cạnh có thể gần hơn.
fn pair_distance(a: &Shape, b: &Shape) -> f64 {
    let mut best = f64::INFINITY;
    for p in a.vertices.iter().step_by((a.vertices.len() / 8).max(1)) {
        for &(c, d) in &b.edges {
            best = best.min(point_segment(*p, c, d));
        }
    }
    let mut near = Vec::new();
    for &(p, q) in &a.edges {
        near_edges(b, ebox(p, q), best, &mut near);
        for &e in &near {
            let (c, d) = b.edges[e];
            best = best.min(seg_distance(p, q, c, d));
        }
    }
    best
}

/// Dò phía trong cạnh: hai đa giác trùng khít mọi cạnh không có đỉnh nào nằm hẳn bên trong nhau.
fn interior_side_overlap(a: &Shape, b: &Shape) -> bool {
    for &(p, q) in &a.edges {
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let len = dx.hypot(dy);
        if len <= EPS {
            continue;
        }
        let m = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
        if inside(m, b) {
            return true;
        }
        let delta = 1e-5f64.min(len / 1000.0);
        for sign in [-1.0, 1.0] {
            let probe = [m[0] + sign * delta * dy / len, m[1] - sign * delta * dx / len];
            if inside(probe, a) && inside(probe, b) {
                return true;
            }
        }
    }
    false
}

/// Hai vùng tô chồng nhau (chạm biên thì không).
fn filled_overlap(a: &Shape, b: &Shape) -> bool {
    if a.x1 <= b.x0 + EPS || b.x1 <= a.x0 + EPS || a.y1 <= b.y0 + EPS || b.y1 <= a.y0 + EPS {
        return false;
    }
    let mut near = Vec::new();
    for &(p, q) in &a.edges {
        near_edges(b, ebox(p, q), EPS, &mut near);
        if near.iter().any(|&e| proper_cross(p, q, b.edges[e].0, b.edges[e].1)) {
            return true;
        }
    }
    if a.vertices.iter().any(|p| inside(*p, b)) || b.vertices.iter().any(|p| inside(*p, a)) {
        return true;
    }
    interior_side_overlap(a, b) || interior_side_overlap(b, a)
}

struct Audit {
    shapes: Vec<Shape>,
    nearest: f64,
    diagonal: f64,
    axial: f64,
    diagonal_pairs: usize,
    axial_pairs: usize,
    /// Khung cả khối: x0, y0, x1, y1 (mm).
    block: BoxF,
}

impl std::fmt::Debug for Audit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Audit {{ {} con, khe gần nhất {}, khối {:?} }}", self.shapes.len(), self.nearest, self.block)
    }
}

impl Audit {
    fn offset(&self, inp: &NestInput) -> (f64, f64) {
        ((self.block.0 + self.block.2 - inp.width) / 2.0, (self.block.1 + self.block.3 - inp.height) / 2.0)
    }
}

/// Kiểm tra độc lập như `physicalGuard` / `independentAudit` của bộ test gốc: lề tờ, tâm PON không
/// nằm trong khuôn, khoảng cách tới mép chấm, không chồng vùng tô (kể cả trùng khít ở khe 0), khe
/// thật giữa các đường bao. `dense`: đường bao mịn hơn đường bao lúc dàn (nếu có).
fn guard(inp: &NestInput, slots: &[OutSlot], dense: Option<&[Groups]>) -> Result<Audit, String> {
    let shapes = slots.iter().map(|s| slot_shape(dense.map_or(&inp.types[s.mi].groups, |d| &d[s.mi]), s)).collect();
    guard_shapes(inp, shapes)
}

fn guard_shapes(inp: &NestInput, shapes: Vec<Shape>) -> Result<Audit, String> {
    let mut block = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (i, a) in shapes.iter().enumerate() {
        if a.x0 < inp.margin - TOL || a.y0 < inp.margin - TOL || a.x1 > inp.width - inp.margin + TOL || a.y1 > inp.height - inp.margin + TOL {
            return Err(format!("con {i}: physical sheet margin ({}, {}, {}, {})", a.x0, a.y0, a.x1, a.y1));
        }
        block = (block.0.min(a.x0), block.1.min(a.y0), block.2.max(a.x1), block.3.max(a.y1));
        for d in &inp.dots {
            let c = [d.x, d.y];
            if inside(c, a) {
                return Err(format!("con {i}: PON not inside a cutter"));
            }
            let dist = a.edges.iter().map(|&(p, q)| point_segment(c, p, q)).fold(f64::INFINITY, f64::min);
            if dist + TOL < d.r + inp.pon_clear {
                return Err(format!("con {i}: physical PON edge clearance {dist} < {}", d.r + inp.pon_clear));
            }
        }
    }
    let reach = inp.gap.max(0.0) + 1.0;
    let size = shapes.iter().map(|s| (s.x1 - s.x0).max(s.y1 - s.y0)).fold(0.0, f64::max) + reach;
    let key = |s: &Shape| ((s.x0 / size).floor() as i64, (s.y0 / size).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in shapes.iter().enumerate() {
        grid.entry(key(s)).or_default().push(i);
    }
    let (mut nearest, mut diagonal, mut axial) = (f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let (mut diagonal_pairs, mut axial_pairs) = (0, 0);
    for (i, a) in shapes.iter().enumerate() {
        let (gx, gy) = key(a);
        for nx in gx - 1..=gx + 1 {
            for ny in gy - 1..=gy + 1 {
                let Some(list) = grid.get(&(nx, ny)) else { continue };
                for &j in list {
                    if j <= i {
                        continue;
                    }
                    let b = &shapes[j];
                    let bg = box_gap(sbox(a), sbox(b));
                    if bg >= reach {
                        continue;
                    }
                    if bg <= EPS && filled_overlap(a, b) {
                        return Err(format!("con {i}/{j}: filled overlap"));
                    }
                    let d = pair_distance(a, b);
                    if d < inp.gap - TOL {
                        return Err(format!("con {i}/{j}: physical gap {d} mm < {} mm", inp.gap));
                    }
                    nearest = nearest.min(d);
                    let cx = ((a.x0 + a.x1) - (b.x0 + b.x1)).abs() / 2.0;
                    let cy = ((a.y0 + a.y1) - (b.y0 + b.y1)).abs() / 2.0;
                    if cx > 0.1 && cy > 0.1 {
                        diagonal_pairs += 1;
                        diagonal = diagonal.min(d);
                    } else {
                        axial_pairs += 1;
                        axial = axial.min(d);
                    }
                }
            }
        }
    }
    Ok(Audit { shapes, nearest, diagonal, axial, diagonal_pairs, axial_pairs, block })
}

/// Thông tin canh tâm công khai khớp khung thật dựng lại; không đúng tâm thì phải nói vì PON.
fn assert_centering(inp: &NestInput, out: &NestOut, au: &Audit, tol: f64, label: &str) {
    let (ox, oy) = au.offset(inp);
    assert!((ox - out.offset.0).abs() < tol && (oy - out.offset.1).abs() < tol, "{label}: lệch tâm báo {:?} ≠ đo được ({ox}, {oy})", out.offset);
    if out.exact {
        assert!(ox.abs() < 0.001 && oy.abs() < 0.001, "{label}: báo đúng tâm nhưng lệch ({ox}, {oy})");
        assert!(!out.detail.contains("Canh tâm bị giới hạn bởi vùng né PON"), "{label}");
    } else {
        assert!(out.detail.contains("Canh tâm bị giới hạn bởi vùng né PON."), "{label}: {}", out.detail);
    }
}

/// Dàn thật theo silhouette: có cặp khung chồng nhau; `same_row`: có cặp đối hướng cùng hàng.
fn assert_interlocked(slots: &[OutSlot], shapes: &[Shape], same_row: bool, label: &str) {
    let mut overlap = false;
    let mut opposed = false;
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            let (a, b) = (&shapes[i], &shapes[j]);
            if a.x1.min(b.x1) - a.x0.max(b.x0) > 0.001 && a.y1.min(b.y1) - a.y0.max(b.y0) > 0.001 {
                overlap = true;
            }
            let turn = (ANGLES[slots[i].vi] - ANGLES[slots[j].vi] + 360) % 360;
            if (slots[i].y - slots[j].y).abs() < 0.001 && turn == 180 {
                opposed = true;
            }
        }
    }
    assert!(overlap, "{label}: phải lồng theo silhouette thật, không xếp kệ theo khung");
    if same_row {
        assert!(opposed, "{label}: hai hướng đối đầu phải cùng một hàng được");
    }
}

// ---------------------------------------------------------------- dan_be_nester.test.js

/// `check(name, groups, minimum, overrides)`: tối thiểu `minimum` con, `verifyGeometry` đúng.
fn check_min(inp: &NestInput, minimum: usize, label: &str) -> NestOut {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: chỉ {} con < {minimum}", out.slots.len());
    assert_counts(inp, &out, label);
    assert!(verify_out(inp, &out), "{label}: khe / PON vật lý");
    out
}

fn fix_input(groups: &Groups) -> NestInput {
    input(330.0, 354.0, &FIX_DOTS, types_of(std::slice::from_ref(groups), 0.0), 1.0, 3000.0)
}

/// Cùng một khuôn vật lý, khác toạ độ nguồn, điểm bắt đầu và chiều đi của đường bao.
fn same_cutter_designs(groups: &Groups, count: usize) -> Vec<TypeIn> {
    (0..count)
        .map(|mi| TypeIn {
            groups: groups
                .iter()
                .map(|g| {
                    g.iter()
                        .map(|c| {
                            let mut pts: Vec<P> = c.iter().map(|p| [p[0] + mi as f64 * 17.0, p[1] - mi as f64 * 9.0]).collect();
                            if mi % 2 == 1 {
                                pts.reverse();
                            }
                            let start = (mi * 7) % pts.len();
                            pts.rotate_left(start);
                            pts
                        })
                        .collect()
                })
                .collect(),
            curve_error: 0.0,
        })
        .collect()
}

#[test]
fn goc_nester_khuon_that_va_mau() {
    // Mốc của bản gốc (log Node: 42, 10, 10, 42, 42, 50, 50, 42).
    check_min(&fix_input(&circle(25.0)), 42, "Tròn 5 cm, tờ 33 × 35,4, khe 1");
    check_min(&fix_input(&spoon()), 10, "Muỗng thật, khe 1");
    let actual = |g: &Groups| NestInput { dots: dots_of(&STD_DOTS), ..fix_input(g) };
    check_min(&actual(&spoon()), 10, "Muỗng, PON 5 mm cách 10 mm");
    check_min(&actual(&circle(25.0)), 42, "Tròn 5 cm, PON thật, so le theo cột");
    check_min(&actual(&circle(25.000000001)), 42, "Tròn bỏ qua bụi số khi đổi đơn vị");
    check_min(&fix_input(&leaf()), 50, "Lá thật, khe 1");
    check_min(&NestInput { budget_ms: 250.0, ..fix_input(&leaf()) }, 50, "Lá giữ mọi hướng xoay trong lát tìm 250 ms");
    check_min(&NestInput { gap: 1.1, ..fix_input(&circle(25.0)) }, 40, "Khe lẻ 1,1 mm làm tròn ra ngoài");
}

#[test]
fn goc_nester_nhieu_bai_chung_khuon() {
    for (label, groups, minimum, n) in
        [("3 bài tròn chung khuôn", circle(25.0), 42, 3), ("2 bài muỗng chung khuôn", spoon(), 10, 2), ("4 bài lá chung khuôn", leaf(), 50, 4)]
    {
        let inp = NestInput { types: same_cutter_designs(&groups, n), ..fix_input(&groups) };
        let out = check_min(&inp, minimum, label);
        assert!(spread(&out.counts) <= 1, "{label}: xen kẽ đều {:?}", out.counts);
        for (i, s) in out.slots.iter().enumerate() {
            assert_eq!(s.mi, i % n, "{label}: chia bài xoay vòng theo thứ tự con");
        }
    }
}

#[test]
fn goc_nester_vuong_khong_vua_va_l_long() {
    let square = rect(20.0, 20.0);
    assert!(nester::circle_like(&square).is_none(), "hình vuông không được đi đường tắt của hình tròn");
    let inp = input(55.0, 55.0, &[], types_of(std::slice::from_ref(&square), 0.0), 1.0, 1200.0);
    check_min(&inp, 4, "Hình vuông dùng cạnh vật lý");
    let impossible = rect(400.0, 400.0);
    let unfit = NestInput { types: types_of(&[square.clone(), impossible], 0.0), ..fix_input(&square) };
    let e = nester::nest(&unfit, &Clock::default()).unwrap_err();
    assert!(e.contains("loại 2"), "mẫu không vừa phải báo đúng loại: {e}");

    // Hai chữ L lồng vào góc trống của nhau dù khung chồng 18 × 18 mm.
    let ck = Clock::default();
    let elbow = poly(&[[0.0, 0.0], [30.0, 0.0], [30.0, 10.0], [10.0, 10.0], [10.0, 30.0], [0.0, 30.0]]);
    let vs: Vec<Variant> = ANGLES.iter().enumerate().map(|(i, &a)| nester::build_variant(&elbow, 0, i, a, 2.0, 0.25, &ck).unwrap()).collect();
    let nested = [Slot { mi: 0, v: 0, x: 0, y: 0 }, Slot { mi: 0, v: 2, x: 48, y: 48 }];
    assert!(nester::verify_plan(&nested, &vs, &Sheet { rw: 168, rh: 168, dots: Vec::new() }, &ck));
    let p = nested.map(|s| PSlot { mi: s.mi, v: s.v, x: s.x as f64, y: s.y as f64 });
    assert!(nester::verify_geometry(&p, &vs, (168.0, 168.0), &[], 2.0, 0.25, &ck));
    // Lọc thô theo không gian không được bỏ sót khe 0,75 mm thiếu an toàn.
    let sv = vec![nester::build_variant(&square, 0, 0, 0, 1.0, 0.25, &ck).unwrap()];
    let close = [PSlot { mi: 0, v: 0, x: 0.0, y: 0.0 }, PSlot { mi: 0, v: 0, x: 83.0, y: 0.0 }];
    assert!(!nester::verify_geometry(&close, &sv, (220.0, 220.0), &[], 1.0, 0.25, &ck));
}

#[test]
fn goc_nester_hang_nghin_khuon_ti_hon() {
    let began = std::time::Instant::now();
    let inp = NestInput { budget_ms: 250.0, ..fix_input(&circle(1.0)) };
    let out = nest(&inp);
    assert!(out.slots.len() >= 13_440, "phải dàn được hàng chục nghìn khuôn tí hon như bản gốc: {}", out.slots.len());
    assert_counts(&inp, &out, "tí hon");
    // Bản gốc: dưới 8 giây trên máy của DanCard; ở đây chỉ chặn trường hợp bậc hai.
    assert!(began.elapsed().as_secs_f64() < 60.0, "kiểm tra an toàn cho khuôn tí hon phải có giới hạn");
    // Kiểm tra độc lập một mẫu các con kề nhau (đầu, giữa, cuối).
    let n = out.slots.len();
    let mut idx: Vec<usize> = (0..12).flat_map(|i| [i, n - 1 - i, n / 2 + i]).collect();
    idx.sort_unstable();
    idx.dedup();
    let pick: Vec<OutSlot> = idx.iter().map(|&i| out.slots[i]).collect();
    guard(&inp, &pick, None).unwrap();
}

// ---------------------------------------------------------------- dan_be_mixed.test.js

#[test]
fn goc_mixed_can_bang_nhieu_mau() {
    let s = |mi: &[usize]| mi.iter().map(|&mi| Slot { mi, v: 0, x: 0, y: 0 }).collect::<Vec<_>>();
    assert!(!nester::balanced(&s(&[0, 0, 0, 1]), 2), "seed đồng nhất lớn không được bỏ qua xen kẽ đều");
    assert!(nester::balanced(&s(&[0, 1, 0]), 2), "chu kỳ cuối chưa đủ được hơn một con");
    let elbow = poly(&[[0.0, 0.0], [30.0, 0.0], [30.0, 10.0], [10.0, 10.0], [10.0, 30.0], [0.0, 30.0]]);
    let corner = [(5.0, 5.0, 1.0), (105.0, 5.0, 1.0), (5.0, 105.0, 1.0), (105.0, 105.0, 1.0)];
    // Mốc gốc 2 và 10; bản JS đo được 5 và 12.
    for (label, groups, dots, minimum) in [
        ("Tròn khác đường kính", vec![circle(10.0), circle(25.0)], &[][..], 5),
        ("Khuôn lõm và chữ nhật khác nhau", vec![elbow, rect(18.0, 12.0)], &corner[..], 12),
    ] {
        let inp = input(110.0, 110.0, dots, types_of(&groups, 0.0), 2.0, 1000.0);
        let out = check_min(&inp, minimum, label);
        assert!(spread(&out.counts) <= 1, "{label}: không lệch vì seed đồng nhất {:?}", out.counts);
    }
}

// ---------------------------------------------------------------- dan_be_native_circle.test.js

#[test]
fn goc_native_circle_42_con() {
    let groups = illustrator_circle(50.0);
    let c = nester::circle_like(&groups).expect("nhận ra hình tròn");
    assert!((c.radius - 25.00679985159256).abs() < 1e-8, "giữ đúng độ phình của đường tròn bậc ba: {}", c.radius);
    let sheet_dots = [
        (10.0000019136746, 343.999997783619, 2.50000015390447),
        (320.000000138853, 343.999997783619, 2.49999914459916),
        (10.0000019136746, 10.0000002341426, 2.49999914459916),
        (320.000000138853, 10.0000002341426, 2.49999813529384),
    ];
    let inp = input(330.000000033917, 354.000000036371, &sheet_dots, types_of(std::slice::from_ref(&groups), 0.0), 1.0, 3000.0);
    let out = nest(&inp);
    assert!(verify_out(&inp, &out));
    assert_eq!(out.slots.len(), 42, "ellipse 50 mm phải giữ đủ 42 chỗ");
    let shared = NestInput { types: types_of(&[groups.clone(), groups.clone()], 0.0), ..inp.clone() };
    let out = nest(&shared);
    assert!(verify_out(&shared, &out));
    assert_eq!(out.slots.len(), 42);
    assert_eq!(out.counts, [21, 21]);
    // Gần tròn không được coi là tròn lý tưởng.
    let oval: Groups = groups.iter().map(|g| g.iter().map(|c| c.iter().map(|p| [p[0], p[1] * 1.001]).collect()).collect()).collect();
    let oval_in = NestInput { types: types_of(&[oval], 0.0), ..inp };
    assert!(verify_out(&oval_in, &nest(&oval_in)));
}

// ---------------------------------------------------------------- dan_be_polish.test.js

#[test]
fn goc_polish_cap_muong_xoay_180() {
    let ck = Clock::default();
    let (step, margin, rw, rh) = (0.25, 4.0, 1288, 1384);
    let groups = nester::build_variant(&spoon(), 0, 2, 180, 1.0, step, &ck).unwrap().groups;
    let vs: Vec<Variant> = ANGLES.iter().enumerate().map(|(vi, &a)| nester::build_variant(&groups, 0, vi, a, 1.0, step, &ck).unwrap()).collect();
    let dots: Vec<Dot> = STD_DOTS.iter().map(|&(x, y, r)| Dot { x: (x - margin) / step, y: (y - margin) / step, radius: (r + 7.5) / step }).collect();
    let cells = |v: f64| ((v - margin) / step) as i64;
    let mut original = Vec::new();
    for i in 0..4 {
        original.push(Slot { mi: 0, v: 0, x: cells(16.25 + f64::from(i) * 67.0), y: cells(176.75) });
    }
    for i in 0..4 {
        original.push(Slot { mi: 0, v: 2, x: cells(47.5 + f64::from(i) * 67.0), y: cells(114.25) });
    }
    original.push(Slot { mi: 0, v: 1, x: cells(77.0), y: 0 });
    original.push(Slot { mi: 0, v: 1, x: cells(21.75), y: cells(52.5) });
    let phys = |p: &[Slot]| p.iter().map(|s| PSlot { mi: s.mi, v: s.v, x: s.x as f64, y: s.y as f64 }).collect::<Vec<_>>();
    assert!(nester::verify_geometry(&phys(&original), &vs, (f64::from(rw), f64::from(rh)), &dots, 1.0, step, &ck));
    let before = margin + step * nester::edge_margin(&original, &vs, rw.into(), rh.into()) as f64;
    let sheet = Sheet { rw: rw.into(), rh: rh.into(), dots: dots.clone() };
    let polished = nester::polish_edges(&original, &vs, &sheet, 1.0, step, ck.now() + 550.0, &ck);
    let after = margin + step * nester::edge_margin(&polished, &vs, rw.into(), rh.into()) as f64;
    assert_eq!(polished.len(), original.len(), "đánh bóng không được làm mất khuôn");
    assert!(nester::verify_plan(&polished, &vs, &sheet, &ck));
    assert!(nester::verify_geometry(&phys(&polished), &vs, (f64::from(rw), f64::from(rh)), &dots, 1.0, step, &ck));
    assert!(after >= before + 5.0, "cặp xoay 180° phải nới lề mép rõ rệt: {before} → {after}");
}

// ---------------------------------------------------------------- dan_be_irregular.test.js

fn ring(radius: f64, count: usize) -> Vec<P> {
    (0..count)
        .map(|i| {
            let a = (i as f64 * 2.0 * std::f64::consts::PI) / count as f64;
            [radius * a.cos(), radius * a.sin()]
        })
        .collect()
}

#[test]
fn goc_irregular_hinh_bat_ky() {
    let cat = poly(&[
        [0.0, 18.0],
        [5.0, 0.0],
        [16.0, 9.0],
        [29.0, 9.0],
        [41.0, 0.0],
        [46.0, 18.0],
        [40.0, 28.0],
        [36.0, 32.0],
        [39.0, 43.0],
        [32.0, 55.0],
        [26.0, 55.0],
        [25.0, 42.0],
        [20.0, 42.0],
        [19.0, 55.0],
        [12.0, 55.0],
        [7.0, 43.0],
        [10.0, 32.0],
        [6.0, 28.0],
    ]);
    let rabbit = poly(&[
        [10.0, 30.0],
        [4.0, 14.0],
        [2.0, 0.0],
        [7.0, 0.0],
        [14.0, 23.0],
        [19.0, 0.0],
        [24.0, 0.0],
        [22.0, 15.0],
        [21.0, 28.0],
        [31.0, 34.0],
        [36.0, 47.0],
        [32.0, 60.0],
        [24.0, 65.0],
        [10.0, 65.0],
        [1.0, 59.0],
        [0.0, 45.0],
        [5.0, 35.0],
    ]);
    let elbow = poly(&[[0.0, 0.0], [30.0, 0.0], [30.0, 10.0], [10.0, 10.0], [10.0, 30.0], [0.0, 30.0]]);
    let star = poly(
        &(0..10)
            .map(|i| {
                let a = (f64::from(i) * std::f64::consts::PI) / 5.0 - std::f64::consts::PI / 2.0;
                let r = if i % 2 == 1 { 10.0 } else { 23.0 };
                [r * a.cos(), r * a.sin()]
            })
            .collect::<Vec<_>>(),
    );
    let mut hole = ring(10.0, 64);
    hole.reverse();
    let holed: Groups = vec![vec![ring(22.0, 64), hole]];
    let sheet = |w: f64, h: f64| [(5.0, 5.0, 1.0), (w - 5.0, 5.0, 1.0), (5.0, h - 5.0, 1.0), (w - 5.0, h - 5.0, 1.0)];
    // Mốc: số con bản JS đo được (log Node); khuôn chữ L phải hơn mọi lưới khung 0/90/180/270°.
    let cases: [(&str, Vec<Groups>, f64, usize, bool); 6] = [
        ("Mèo: tai và chân lõm", vec![cat.clone()], 180.0, 12, false),
        ("Thỏ: tai và thân", vec![rabbit.clone()], 180.0, 12, false),
        ("Sao năm cánh lõm", vec![star], 180.0, 16, false),
        ("Compound có lỗ thật", vec![holed], 180.0, 12, false),
        ("Hai loại bất kỳ xen kẽ", vec![cat, rabbit], 220.0, 13, false),
        ("Chữ L lồng hơn mọi lưới khung", vec![elbow], 65.0, 2, true),
    ];
    for (label, groups, size, minimum, beat_bbox) in cases {
        let inp = input(size, size, &sheet(size, size), types_of(&groups, 0.0), 2.0, 1200.0);
        let out = nest(&inp);
        assert!(out.slots.len() >= minimum, "{label}: {} < {minimum}", out.slots.len());
        assert_counts(&inp, &out, label);
        guard(&inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
        if groups.len() > 1 {
            assert!(spread(&out.counts) <= 1, "{label}: các loại cân bằng {:?}", out.counts);
        }
        if beat_bbox {
            let mut best = 0;
            for a in ANGLES {
                let s = shape(&groups[0], a, 0.0, 0.0);
                let cols = ((inp.width - 2.0 * inp.margin + inp.gap) / (s.x1 + inp.gap)).floor().max(0.0);
                let rows = ((inp.height - 2.0 * inp.margin + inp.gap) / (s.y1 + inp.gap)).floor().max(0.0);
                best = best.max((cols * rows) as usize);
            }
            assert!(out.slots.len() > best, "{label}: lồng silhouette phải hơn lưới khung tốt nhất ({best})");
        }
    }
}

// ---------------------------------------------------------------- dan_be_alternating.test.js

fn triangle() -> Groups {
    poly(&[[0.0, 27.0], [26.5, 0.0], [53.0, 27.0]])
}

fn trapezoid() -> Groups {
    poly(&[[0.0, 27.0], [18.0, 0.0], [35.0, 0.0], [53.0, 27.0]])
}

fn alt_input(groups: &[Groups], gap: f64, budget: f64) -> NestInput {
    input(330.0, 354.0, &STD_DOTS, types_of(groups, 0.0), gap, budget)
}

/// `checked`: đủ mốc, tổng khớp, kiểm tra vật lý độc lập, có lồng silhouette thật.
fn alt_checked(inp: &NestInput, minimum: usize, label: &str) -> NestOut {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: {} < {minimum}", out.slots.len());
    assert_counts(inp, &out, label);
    let au = guard(inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert_interlocked(&out.slots, &au.shapes, false, label);
    out
}

/// `expiredBaseline`: hàng xen kẽ đã quá hạn vẫn cho phương án tất định; trả số con.
fn expired_baseline(inp: &NestInput, minimum: usize, label: &str) -> usize {
    let ck = Clock::default();
    let vs = variants_for(inp, false);
    let plans = nester::alternating_row_plans(&vs, inp.types.len(), &grid_sheet(inp), -1.0, &ck);
    assert!(!plans.is_empty(), "{label}: quá hạn vẫn phải có seed hữu hạn");
    let best = plans.iter().max_by_key(|p| p.len()).unwrap();
    assert!(best.len() >= minimum, "{label}: {} < {minimum}", best.len());
    let slots = out_slots(inp, best, &vs);
    let au = guard(inp, &slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert_interlocked(&slots, &au.shapes, true, label);
    best.len()
}

#[test]
fn goc_alternating_tam_giac_hinh_thang() {
    // Mốc cũ một hướng / lưới đều là 74, 83 và 73; bản JS hiện tại đo 109, 119, 94, 86.
    alt_checked(&alt_input(&[triangle()], 2.0, 550.0), 109, "Tam giác, khe 2");
    alt_checked(&alt_input(&[triangle()], 1.0, 550.0), 119, "Tam giác, khe 1");
    expired_baseline(&alt_input(&[triangle()], 1.5, 550.0), 95, "Tam giác, khe lẻ 1,5");
    alt_checked(&alt_input(&[trapezoid()], 1.0, 550.0), 94, "Hình thang, khe 1");
    alt_checked(&alt_input(&[trapezoid()], 2.0, 550.0), 86, "Hình thang, khe 2");
    expired_baseline(&alt_input(&[trapezoid()], 1.5, 550.0), 84, "Hình thang, khe lẻ 1,5");
    let triangle_seed = expired_baseline(&alt_input(&[triangle()], 2.0, 550.0), 95, "Tam giác, quá hạn");
    let trap_seed = expired_baseline(&alt_input(&[trapezoid()], 1.0, 550.0), 90, "Hình thang, quá hạn");
    for a in ANGLES {
        let n = expired_baseline(&alt_input(&[rotated(&triangle(), a)], 2.0, 550.0), 95, &format!("Tam giác nguồn xoay {a}, quá hạn"));
        assert_eq!(n, triangle_seed, "số con mốc không đổi theo hướng nguồn");
    }
    let short = alt_checked(&alt_input(&[rotated(&triangle(), 270)], 2.0, 250.0), 109, "Tam giác xoay 270, 250 ms");
    assert!(short.slots.len() >= triangle_seed, "ngân sách tối thiểu vẫn giữ trọn seed hàng dày");
    let short = alt_checked(&alt_input(&[trapezoid()], 1.0, 250.0), 94, "Hình thang, 250 ms");
    assert!(short.slots.len() >= trap_seed);
}

#[test]
fn goc_alternating_ba_bai_chung_khuon_va_cap() {
    let triangle_seed = expired_baseline(&alt_input(&[triangle()], 2.0, 550.0), 95, "Tam giác, quá hạn");
    let shifted: Groups = triangle().iter().map(|g| g.iter().map(|c| c.iter().map(|p| [p[0] + 123.4, p[1] - 67.8]).collect()).collect()).collect();
    let restarted: Groups = triangle()
        .iter()
        .map(|g| {
            g.iter()
                .map(|c| {
                    let mut v: Vec<P> = c[1..].iter().chain(&c[..1]).copied().collect();
                    v.reverse();
                    v
                })
                .collect()
        })
        .collect();
    let inp = alt_input(&[triangle(), shifted, restarted], 2.0, 550.0);
    let out = alt_checked(&inp, 109, "Ba bài chung một khuôn tam giác");
    assert!(spread(&out.counts) <= 1, "các mẫu cùng khuôn cân bằng {:?}", out.counts);
    for (i, s) in out.slots.iter().enumerate() {
        assert_eq!(s.mi, i % 3, "chia bài xoay vòng");
    }
    assert!(out.slots.len() >= triangle_seed, "nhân bài không được tắt phép tìm silhouette dày");
    alt_checked(&alt_input(&[cap()], 1.0, 250.0), 94, "Khuôn cong bảy nút thật, 250 ms");
    alt_checked(&alt_input(&[cap()], 2.0, 250.0), 92, "Khuôn cong thật, khe 2, 250 ms");
}

#[test]
fn goc_alternating_khuon_ti_hon_2mm() {
    let tiny = rect(2.0, 2.0);
    let inp = input(330.0, 354.0, &[], types_of(std::slice::from_ref(&tiny), 0.0), 1.0, 250.0);
    let ck = Clock::default();
    let vs = variants_for(&inp, false);
    let began = std::time::Instant::now();
    let plans = nester::alternating_row_plans(&vs, 1, &Sheet { rw: 1288, rh: 1384, dots: Vec::new() }, -1.0, &ck);
    assert!(began.elapsed().as_secs_f64() < 10.0, "mốc khuôn tí hon không dựng lại lưới chiếm chỗ cho từng pha");
    let plan = plans.iter().max_by_key(|p| p.len()).unwrap();
    assert!(plan.len() >= 9000, "mốc giữ hàng nghìn chỗ hợp lệ: {}", plan.len());
    let n = plan.len();
    let mut pick: Vec<usize> = (0..12).flat_map(|i| [i, n - 1 - i, n / 2 + i]).collect();
    // Ba giá trị y khác nhau đầu tiên theo thứ tự phương án (như `new Set` của test gốc).
    let mut ys: Vec<i64> = Vec::new();
    for s in plan.iter() {
        if ys.len() == 3 {
            break;
        }
        if !ys.contains(&s.y) {
            ys.push(s.y);
        }
    }
    for y in &ys {
        let start = plan.iter().position(|s| s.y == *y).unwrap();
        pick.extend(start..(start + 10).min(n));
    }
    pick.sort_unstable();
    pick.dedup();
    let sample: Vec<Slot> = pick.iter().map(|&i| plan[i]).collect();
    guard(&inp, &out_slots(&inp, &sample, &vs), None).unwrap();

    let began = std::time::Instant::now();
    let out = nest(&inp);
    assert!(began.elapsed().as_secs_f64() < 60.0, "phép tìm đầy đủ khuôn tí hon phải có giới hạn");
    assert!(out.slots.len() >= 10_580, "phép tìm đầy đủ giữ lưới dày đã kiểm chứng: {}", out.slots.len());
    for s in &out.slots {
        assert_eq!(s.mi, 0);
        assert!(s.x >= 4.0 - 0.001 && s.y >= 4.0 - 0.001 && s.x + 2.0 <= 326.0 + 0.001 && s.y + 2.0 <= 350.0 + 0.001, "{s:?}");
    }
    let mut rows: Vec<f64> = out.slots.iter().map(|s| s.y).collect();
    rows.sort_by(f64::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    let mut sample = Vec::new();
    for ri in [0, 1, 2, rows.len() / 2, rows.len() - 1] {
        let mut row: Vec<OutSlot> = out.slots.iter().filter(|s| (s.y - rows[ri]).abs() < 0.001).copied().collect();
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
        for off in [0, (row.len() / 2).saturating_sub(5), row.len().saturating_sub(10)] {
            for s in row.iter().skip(off).take(10) {
                if !sample.iter().any(|t: &OutSlot| t.x == s.x && t.y == s.y) {
                    sample.push(*s);
                }
            }
        }
    }
    guard(&inp, &sample, None).unwrap();
}

// ---------------------------------------------------------------- dan_be_arch_gap.test.js

/// Mốc dựng độc lập: hàng cách nhau đúng khung + khe (thêm 0,001), bốn hướng, ba pha mỗi trục,
/// chỉ đếm các con né PON thật. Phép tìm không bao giờ được thua bố cục đơn giản này.
fn row_baseline(inp: &NestInput, dense: &Groups, gutter_extra: f64, pon_strict: bool) -> usize {
    let mut best = 0;
    for a in ANGLES {
        let sample = shape(dense, a, 0.0, 0.0);
        let (w, h, gap) = (sample.x1, sample.y1, inp.gap + gutter_extra);
        let (rw, rh) = (inp.width - 2.0 * inp.margin, inp.height - 2.0 * inp.margin);
        let cols = ((rw + gap) / (w + gap)).floor();
        let rows = ((rh + gap) / (h + gap)).floor();
        let spare_x = rw - (cols * w + (cols - 1.0).max(0.0) * gap);
        let spare_y = rh - (rows * h + (rows - 1.0).max(0.0) * gap);
        for px in [0.0, spare_x / 2.0, spare_x] {
            for py in [0.0, spare_y / 2.0, spare_y] {
                let mut count = 0;
                for y in 0..rows as usize {
                    for x in 0..cols as usize {
                        let s = shape(dense, a, inp.margin + px + x as f64 * (w + gap), inp.margin + py + y as f64 * (h + gap));
                        let clear = inp.dots.iter().all(|d| {
                            let c = [d.x, d.y];
                            let dist = s.edges.iter().map(|&(p, q)| point_segment(c, p, q)).fold(f64::INFINITY, f64::min);
                            let need = d.r + inp.pon_clear + if pon_strict { 0.001 } else { -0.001 };
                            !inside(c, &s) && dist >= need
                        });
                        count += usize::from(clear);
                    }
                }
                best = best.max(count);
            }
        }
    }
    best
}

#[test]
fn goc_arch_gap_diem_cuoi_noi_khe() {
    // Chỉ điểm cuối được phép (0,25 mm mỗi bên) mới cứu được tờ đúng khổ này: nới vị trí, không bao
    // giờ co hai khuôn 10 mm.
    let ck = Clock::default();
    let (step, guard_gap) = (0.25, 2.001);
    let groups = rect(10.0, 10.0);
    let vs = vec![nester::build_variant(&groups, 0, 0, 0, guard_gap, step, &ck).unwrap()];
    let plan = vec![PSlot { mi: 0, v: 0, x: 0.0, y: 0.0 }, PSlot { mi: 0, v: 0, x: 11.7 / step, y: 0.0 }];
    let verify = |p: &[PSlot], w: f64| nester::verify_geometry(p, &vs, (w / step, 10.0 / step), &[], guard_gap, step, &ck);
    assert!(!verify(&plan, 22.2), "khe 1,7 mm ban đầu dưới chốt chặn");
    let short = vec![plan[0], PSlot { x: (11.7 + 2.0 * 0.128) / step, ..plan[1] }];
    assert!(!verify(&short, 22.2), "nới 0,128 mm mỗi bên vẫn chỉ còn 1,956 mm");
    let recovered = nester::open_physical_gap_plan(&plan, &vs, (22.2 / step, 10.0 / step), &[], guard_gap, step, &ck);
    assert_eq!(recovered.len(), 2, "giữ cả hai khuôn");
    assert!(verify(&recovered, 22.2), "phải thử tới điểm cuối hợp lệ");
    let placed: Vec<Shape> = recovered.iter().map(|s| shape(&groups, 0, s.x * step, s.y * step)).collect();
    for (s, p) in recovered.iter().zip(&placed) {
        assert_eq!((s.mi, s.v), (0, 0));
        assert!((p.x1 - p.x0 - 10.0).abs() < 1e-8 && (p.y1 - p.y0 - 10.0).abs() < 1e-8);
        assert!(p.x0 >= -1e-8 && p.x1 <= 22.2 + 1e-8 && p.y0 >= -1e-8 && p.y1 <= 10.0 + 1e-8);
    }
    assert!(pair_distance(&placed[0], &placed[1]) >= 2.0, "khe vật lý độc lập sau khi nới");
    let unrecovered = nester::open_physical_gap_plan(&plan, &vs, (21.7 / step, 10.0 / step), &[], guard_gap, step, &ck);
    assert_eq!(unrecovered, plan, "tờ không thể: trả nguyên phương án để chốt chặn cuối từ chối");
    assert!(!verify(&unrecovered, 21.7));
}

fn arch_check(width: u32, height: u32, gap: f64, angle: i64, n: usize) {
    let groups = arch(width, height, 0.00025, angle);
    let dense = arch(width, height, 0.0001, angle);
    let types: Vec<TypeIn> = (0..n).map(|_| TypeIn { groups: groups.clone(), curve_error: 0.00025 }).collect();
    let inp = input(330.0, 354.0, &STD_DOTS, types, gap, 250.0);
    let label = format!("Vòm {width}x{height}, khe {gap}, nguồn xoay {angle}, {n} bài");
    let minimum = row_baseline(&inp, &dense, 0.001, true);
    assert!(minimum >= 24, "{label}: mốc dựng độc lập có nghĩa ({minimum})");
    let out = nest(&inp);
    assert!(out.slots.len() >= minimum, "{label}: {} con thua mốc {minimum} hàng an toàn", out.slots.len());
    assert_counts(&inp, &out, &label);
    if n == 2 {
        assert!(spread(&out.counts) <= 1, "{label}: bài tương đương cân bằng");
    }
    let dense_types = vec![dense; n];
    let au = guard(&inp, &out.slots, Some(&dense_types)).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert_centering(&inp, &out, &au, 0.001, &label);
}

#[test]
fn goc_arch_gap_vom_42x72() {
    for (gap, angle) in [(5.0, 0), (5.0, 90), (5.0, 180), (5.0, 270), (1.5, 0)] {
        arch_check(42, 72, gap, angle, 1);
    }
    arch_check(42, 72, 5.0, 0, 2);
}

#[test]
fn goc_arch_gap_vom_40x70() {
    for (gap, angle) in [(5.0, 0), (5.0, 90), (5.0, 180), (5.0, 270), (1.5, 0)] {
        arch_check(40, 70, gap, angle, 1);
    }
}

// ---------------------------------------------------------------- dan_be_centering.test.js

fn elbow18() -> Groups {
    poly(&[[0.0, 0.0], [18.0, 0.0], [18.0, 6.0], [6.0, 6.0], [6.0, 18.0], [0.0, 18.0]])
}

/// `exactPublicCentre`: đủ mốc, kiểm tra vật lý, lệch tâm báo khớp; lệch thì chỉ vì PON thật chặn.
fn exact_public_centre(inp: &NestInput, minimum: usize, label: &str, allow_pon: bool) -> NestOut {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: {} < {minimum}", out.slots.len());
    assert_counts(inp, &out, label);
    let au = guard(inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
    let (ox, oy) = au.offset(inp);
    assert!((out.offset.0 - ox).abs() < 1e-8 && (out.offset.1 - oy).abs() < 1e-8, "{label}: {:?} ≠ ({ox}, {oy})", out.offset);
    if ox.abs() >= 0.001 || oy.abs() >= 0.001 {
        assert!(allow_pon, "{label}: lệch tâm ngoài dự kiến ({ox}, {oy})");
        let trial: Vec<OutSlot> = out.slots.iter().map(|s| OutSlot { x: s.x - ox, y: s.y - oy, ..*s }).collect();
        let e = guard(inp, &trial, None).err().unwrap_or_else(|| panic!("{label}: tâm đúng không va PON thì phải canh đúng tâm"));
        assert!(e.contains("PON"), "{label}: chỉ PON mới được chặn canh tâm: {e}");
        assert!(ox.abs().min(oy.abs()) < 0.001, "{label}: trục không bị chặn vẫn đúng tâm");
        assert!(!out.exact && out.detail.contains("Canh tâm bị giới hạn bởi vùng né PON."), "{label}");
    } else {
        assert!(out.exact && !out.detail.contains("Canh tâm bị giới hạn"), "{label}");
    }
    out
}

fn centre_input(groups: &[Groups], w: f64, h: f64, dots: &[(f64, f64, f64)], gap: f64) -> NestInput {
    input(w, h, dots, types_of(groups, 0.0), gap, 250.0)
}

#[test]
fn goc_centering_cong_khai() {
    exact_public_centre(&centre_input(&[cap()], 330.0, 354.0, &STD_DOTS, 1.0), 94, "Cap cong thật, khe 1", false);
    exact_public_centre(&centre_input(&[cap()], 330.0, 354.0, &STD_DOTS, 2.0), 92, "Cap cong thật, khe 2", true);
    exact_public_centre(&centre_input(&[circle(25.0)], 330.0, 354.0, &STD_DOTS, 1.0), 42, "Tròn 50 mm", true);
    exact_public_centre(&centre_input(&[circle(25.0)], 330.0, 354.0, &[], 1.0), 42, "Tròn 50 mm, không PON", false);
    exact_public_centre(&centre_input(&[rect(17.4, 12.6)], 100.13, 86.37, &[], 1.0), 24, "Khuôn và giấy số lẻ", false);
    let mixed =
        exact_public_centre(&centre_input(&[elbow18(), rect(11.13, 7.39)], 94.17, 91.31, &[], 1.0), 24, "Hai loại lõm / chữ nhật, không PON", false);
    assert_eq!(mixed.counts, [12, 12], "số con cân bằng sau canh tâm");
}

/// `centredHelper`: canh tâm vật lý giữ số con, danh tính, hướng; mọi con dời cùng một vector.
fn centred_helper(inp: &NestInput, vs: &[Variant], plan: &[PSlot]) -> Vec<PSlot> {
    let out = nester::centre_physical_plan(plan, vs, area(inp), &cell_dots(inp), inp.step, &Clock::default());
    assert_eq!(out.len(), plan.len(), "giữ số con");
    let (dx, dy) = (out[0].x - plan[0].x, out[0].y - plan[0].y);
    for (a, b) in out.iter().zip(plan) {
        assert_eq!((a.mi, a.v), (b.mi, b.v), "giữ danh tính và hướng");
        assert!((a.x - b.x - dx).abs() < 1e-8 && (a.y - b.y - dy).abs() < 1e-8, "một phép dời chung, không co, không xếp lại");
    }
    guard(inp, &pout_slots(inp, &out, vs), None).unwrap();
    out
}

fn centre_error(inp: &NestInput, vs: &[Variant], plan: &[PSlot]) -> (f64, f64) {
    let shapes = pout_slots(inp, plan, vs).iter().map(|s| slot_shape(&inp.types[s.mi].groups, s)).collect::<Vec<_>>();
    let x0 = shapes.iter().map(|s| s.x0).fold(f64::INFINITY, f64::min);
    let y0 = shapes.iter().map(|s| s.y0).fold(f64::INFINITY, f64::min);
    let x1 = shapes.iter().map(|s| s.x1).fold(f64::NEG_INFINITY, f64::max);
    let y1 = shapes.iter().map(|s| s.y1).fold(f64::NEG_INFINITY, f64::max);
    ((x0 + x1 - inp.width) / 2.0, (y0 + y1 - inp.height) / 2.0)
}

#[test]
fn goc_centering_ham_phu() {
    // Dời lẻ ô theo kích thước vật lý thật, giữ danh tính / góc xoay.
    let direct_in = centre_input(&[rect(17.4, 12.6), elbow18()], 100.13, 86.37, &[], 1.0);
    let vs = variants_for(&direct_in, false);
    let direct = [PSlot { mi: 0, v: 0, x: 12.0, y: 16.0 }, PSlot { mi: 1, v: 5, x: 124.0, y: 20.0 }, PSlot { mi: 0, v: 2, x: 20.0, y: 140.0 }];
    guard(&direct_in, &pout_slots(&direct_in, &direct, &vs), None).unwrap();
    let centred = centred_helper(&direct_in, &vs, &direct);
    let (ex, ey) = centre_error(&direct_in, &vs, &centred);
    assert!(ex.abs() < 0.001 && ey.abs() < 0.001, "dời lẻ canh đúng tâm khối thật cả hai trục ({ex}, {ey})");

    // Một PON giữa tờ chặn canh tâm hoàn toàn; canh theo một trục thì an toàn.
    let blocked_in = NestInput { margin: 0.0, pon_clear: 2.0, ..centre_input(&[rect(10.0, 10.0)], 100.0, 80.0, &[(50.0, 40.0, 5.0)], 1.0) };
    let vs = variants_for(&blocked_in, false);
    let blocked = [PSlot { mi: 0, v: 0, x: 40.0, y: 40.0 }];
    guard(&blocked_in, &pout_slots(&blocked_in, &blocked, &vs), None).unwrap();
    assert!(guard(&blocked_in, &[OutSlot { mi: 0, vi: 0, x: 45.0, y: 35.0 }], None).is_err(), "tâm đúng va PON");
    let before = centre_error(&blocked_in, &vs, &blocked);
    let constrained = centred_helper(&blocked_in, &vs, &blocked);
    let after = centre_error(&blocked_in, &vs, &constrained);
    assert!(after.0.hypot(after.1) < before.0.hypot(before.1), "phương án dự phòng đã kiểm chứng vẫn canh tốt hơn");
    assert!(after.0.abs().min(after.1.abs()) < 0.001, "trục không bị chặn vẫn đúng tâm");
    assert!(after.0.abs().max(after.1.abs()) > 0.001, "phép thử bị PON chặn không được giả là đúng tâm");

    // Phương án ban đầu đúng tâm nhưng không an toàn: không được xếp trên phương án an toàn gần đó.
    let unsafe_in = NestInput { margin: 0.0, pon_clear: 0.0, ..centre_input(&[rect(10.0, 10.0)], 100.0, 80.0, &[(57.0, 47.0, 3.0)], 1.0) };
    let vs = variants_for(&unsafe_in, false);
    let original = [PSlot { mi: 0, v: 0, x: 180.0, y: 140.0 }];
    assert_eq!(centre_error(&unsafe_in, &vs, &original), (0.0, 0.0));
    let e = guard(&unsafe_in, &pout_slots(&unsafe_in, &original, &vs), None).unwrap_err();
    assert!(e.contains("PON"), "{e}");
    let recovered = centred_helper(&unsafe_in, &vs, &original);
    let r = centre_error(&unsafe_in, &vs, &recovered);
    assert!(r.0.hypot(r.1) > 0.001 && r.0.hypot(r.1) < 0.5, "giữ phương án an toàn gần nhất ({r:?})");
}

// ---------------------------------------------------------------- dan_be_multiband.test.js

fn scaled_turn(groups: &Groups, scale: f64, angle: i64) -> Groups {
    groups.iter().map(|g| g.iter().map(|c| c.iter().map(|p| quarter([p[0] * scale, p[1] * scale], angle)).collect()).collect()).collect()
}

fn std_dots_scaled(scale: f64) -> Vec<(f64, f64, f64)> {
    STD_DOTS.iter().map(|&(x, y, r)| (x * scale, y * scale, r * scale)).collect()
}

fn arch_input(angle: i64, scale: f64, gap: f64, n: usize) -> NestInput {
    let groups = scaled_turn(&arch(40, 70, 0.00025, 0), scale, angle);
    NestInput {
        width: 330.0 * scale,
        height: 354.0 * scale,
        dots: dots_of(&std_dots_scaled(scale)),
        types: (0..n).map(|_| TypeIn { groups: groups.clone(), curve_error: 0.00025 * scale }).collect(),
        gap: gap * scale,
        margin: 4.0 * scale,
        pon_clear: 7.5 * scale,
        step: 0.25 * scale,
        budget_ms: 250.0,
    }
}

fn arch_dense(angle: i64, scale: f64, n: usize) -> Vec<Groups> {
    vec![scaled_turn(&arch(40, 70, 0.0001, 0), scale, angle); n]
}

/// Bố cục 30 vòm 40 × 70 dựng độc lập (không phải bố cục tay trong ảnh chụp của người dùng): rãnh
/// khung 5,001 mm, hai họ hướng ở hai dải riêng.
fn thirty_witness() -> Vec<OutSlot> {
    let dense = &arch_dense(0, 1.0, 1)[0];
    let b = shape(dense, 0, 0.0, 0.0);
    let (w, h, g) = (b.x1, b.y1, 5.001);
    let (standing, total) = (7.0 * w + 6.0 * g, 4.0 * h + 4.0 * g + w);
    let (left, bottom) = ((330.0 - standing) / 2.0, (354.0 - total) / 2.0);
    let mut slots = Vec::new();
    for row in 0..4 {
        for col in 0..7 {
            if row == 0 && (col == 0 || col == 6) {
                continue;
            }
            slots.push(OutSlot { mi: 0, vi: 0, x: left + f64::from(col) * (w + g), y: bottom + f64::from(row) * (h + g) });
        }
    }
    let horizontal_left = (330.0 - (4.0 * h + 3.0 * g)) / 2.0;
    for col in 0..4 {
        // Hai đầu vòm ngoài cùng quay về góc PON của chúng.
        let vi = if col == 3 { 3 } else { 1 };
        slots.push(OutSlot { mi: 0, vi, x: horizontal_left + f64::from(col) * (h + g), y: bottom + 4.0 * (h + g) });
    }
    slots
}

#[test]
fn goc_multiband_bo_cuc_doc_lap_30_vom() {
    let witness = thirty_witness();
    assert_eq!(witness.len(), 30);
    let inp = arch_input(0, 1.0, 5.0, 1);
    let dense = arch_dense(0, 1.0, 1);
    let proof = guard(&inp, &witness, Some(&dense)).unwrap();
    assert!(proof.nearest >= 5.0009, "bố cục độc lập giữ đúng khe 5 mm thật: {}", proof.nearest);
    let families: std::collections::HashSet<i64> = witness.iter().map(|s| ANGLES[s.vi] % 180).collect();
    assert_eq!(families.len(), 2, "hai họ hướng ở hai dải riêng");
    let mut invalid = witness.clone();
    invalid.last_mut().unwrap().x -= 0.01;
    let e = guard(&inp, &invalid, Some(&dense)).unwrap_err();
    assert!(e.contains("physical gap"), "chốt chặn độc lập phải bắt được khe thiếu: {e}");
}

#[test]
fn goc_multiband_mot_dai_qua_han_van_co_30() {
    let inp = arch_input(0, 1.0, 5.0, 1);
    let ck = Clock::default();
    let vs = variants_for(&inp, true);
    let sheet = grid_sheet(&inp);
    let began = std::time::Instant::now();
    let plans = nester::compositional_band_plans(&vs, 1, &sheet, inp.gap, inp.step, -1.0, &ck);
    assert!(!plans.is_empty(), "hết hạn tuỳ chọn không được bỏ seed dựng tất định");
    for plan in &plans {
        assert!(nester::verify_plan(plan, &vs, &sheet, &ck), "mọi seed ghép dải qua ràng buộc raster");
        let p: Vec<PSlot> = plan.iter().map(|s| PSlot { mi: s.mi, v: s.v, x: s.x as f64, y: s.y as f64 }).collect();
        assert!(
            nester::verify_geometry(&p, &vs, (sheet.rw as f64, sheet.rh as f64), &sheet.dots, inp.gap, inp.step, &ck),
            "và hình học liên tục kể cả dự trữ đường cong"
        );
    }
    let best = plans.iter().max_by_key(|p| p.len()).unwrap();
    assert!(best.len() >= 30, "quá hạn vẫn giữ bố cục 30 vòm đã chứng minh: {}", best.len());
    guard(&inp, &out_slots(&inp, best, &vs), Some(&arch_dense(0, 1.0, 1))).unwrap();
    assert!(began.elapsed().as_secs_f64() < 30.0, "mốc dựng khi quá hạn phải có giới hạn");
}

/// `checked` của multiband: mốc, chỉ số, cân bằng, kiểm tra độc lập (không đi tắt theo khung ở khe 0),
/// canh tâm khớp.
fn band_checked(inp: &NestInput, dense: &[Groups], minimum: usize, label: &str) -> NestOut {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: {} con thua bố cục {minimum} con đã chứng minh", out.slots.len());
    assert_counts(inp, &out, label);
    if inp.types.len() > 1 {
        assert!(spread(&out.counts) <= 1, "{label}: bài tương đương cân bằng");
    }
    let au = guard(inp, &out.slots, Some(dense)).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert!(au.nearest >= (inp.gap - 0.001).max(0.0), "{label}");
    assert_centering(inp, &out, &au, 0.001, label);
    out
}

#[test]
fn goc_multiband_vom_moi_huong_va_ti_le() {
    let mut first: Option<NestOut> = None;
    for a in ANGLES {
        let out = band_checked(&arch_input(a, 1.0, 5.0, 1), &arch_dense(a, 1.0, 1), 30, &format!("Vòm bo thật / khe 5 / xoay {a}"));
        match &first {
            None => first = Some(out),
            Some(f) => assert_eq!(out.slots.len(), f.slots.len(), "cách ghi xoay một phần tư của nguồn không được đổi số con"),
        }
    }
    let first = first.unwrap();
    let again = band_checked(&arch_input(0, 1.0, 5.0, 1), &arch_dense(0, 1.0, 1), 30, "Vòm / chạy lại 250 ms");
    assert_eq!((again.slots.len(), &again.counts), (first.slots.len(), &first.counts));
    for scale in [0.8, 1.25] {
        band_checked(&arch_input(0, scale, 5.0, 1), &arch_dense(0, scale, 1), 30, &format!("Vòm / tờ / PON cùng tỉ lệ {scale}"));
    }
    band_checked(&arch_input(0, 1.0, 5.0, 2), &arch_dense(0, 1.0, 2), 30, "Hai bài tương đương / dải trộn");
    band_checked(&arch_input(0, 1.0, 0.0, 1), &arch_dense(0, 1.0, 1), 34, "Vòm bo thật / khe 0");
}

#[test]
fn goc_multiband_da_giac_tong_quat() {
    let pentagon = poly(&[[0.0, 0.0], [33.4, 0.0], [29.1, 42.3], [14.7, 61.2], [4.3, 42.3]]);
    let ell = poly(&[[0.0, 0.0], [50.6, 0.0], [50.6, 17.3], [18.2, 17.3], [18.2, 48.7], [0.0, 48.7]]);
    let frame: Groups =
        vec![vec![vec![[0.0, 0.0], [52.2, 0.0], [52.2, 42.7], [0.0, 42.7]], vec![[10.0, 10.0], [10.0, 32.7], [42.2, 32.7], [42.2, 10.0]]]];
    let corner = [(10.0, 10.0, 2.5), (140.0, 10.0, 2.5), (10.0, 150.0, 2.5), (140.0, 150.0, 2.5)];
    // Bản JS đo được: ngũ giác 9 / 9, chữ L 12 / 10, khung 8 / 8.
    for (label, groups, dots, measured) in
        [("Ngũ giác lệch", pentagon, &corner[..], [9, 9]), ("Chữ L lõm", ell, &[][..], [12, 10]), ("Khung compound", frame, &[][..], [8, 8])]
    {
        for (k, gap) in [0.0, 1.5].into_iter().enumerate() {
            let inp = input(150.0, 160.0, dots, types_of(std::slice::from_ref(&groups), 0.0), gap, 250.0);
            let baseline = row_baseline(&inp, &groups, 0.001, false);
            assert!(baseline >= 6, "{label}: mốc hàng an toàn có nghĩa");
            band_checked(&inp, std::slice::from_ref(&groups), baseline.max(measured[k]), &format!("{label} / khe {gap} / 250 ms"));
        }
    }
}

// ---------------------------------------------------------------- dan_be_positive_gap.test.js

#[derive(Default)]
struct PosOpts {
    exact_count: Option<usize>,
    diagonal: bool,
    exact_centre: bool,
}

fn positive_check(inp: &NestInput, dense: &[Groups], minimum: usize, label: &str, o: PosOpts) {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: giữ ít nhất {minimum}, được {}", out.slots.len());
    if let Some(n) = o.exact_count {
        assert_eq!(out.slots.len(), n, "{label}: giữ mọi chỗ tròn hợp lệ trước đây");
    }
    assert_counts(inp, &out, label);
    if inp.types.len() == 2 {
        assert!(spread(&out.counts) <= 1, "{label}: bài tương đương cân bằng");
    }
    let au = guard(inp, &out.slots, Some(dense)).unwrap_or_else(|e| panic!("{label}: {e}"));
    let g = inp.gap;
    assert!(
        au.nearest >= g - 0.001 && au.nearest <= g + 0.005,
        "{label}: khe yêu cầu {g}, khe thật gần nhất {} (không đệm lưới cố định)",
        au.nearest
    );
    if o.diagonal {
        assert!(au.diagonal_pairs > 0 && au.axial_pairs > 0, "{label}: phải có cặp chéo và cặp cùng hàng / cột");
        assert!(au.diagonal >= g - 0.001 && au.diagonal <= g + 0.005, "{label}: khe chéo {}", au.diagonal);
        assert!(au.axial >= g - 0.001 && au.axial <= g + 0.005, "{label}: khe cùng hàng {}", au.axial);
    }
    if o.exact_centre {
        assert!(out.exact, "{label}: không vướng PON thì đúng tâm");
    }
    assert_centering(inp, &out, &au, 0.001, label);
}

#[test]
fn goc_positive_gap_ellipse_50() {
    for gap in [0.7, 1.1, 1.5, 1.9] {
        let groups = cubic_circle(50.0, 0.00025);
        let dense = cubic_circle(50.0, 0.0001);
        assert!(dense[0][0].len() > groups[0][0].len(), "đường bao kiểm tra mịn hơn, độc lập với đường bao lúc dàn");
        let inp = input(330.0, 354.0, &STD_DOTS, types_of(&[groups], 0.00025), gap, 250.0);
        let full = gap != 1.9;
        let o = PosOpts { exact_count: full.then_some(42), diagonal: true, exact_centre: gap == 0.7 };
        positive_check(&inp, &[dense], if full { 42 } else { 40 }, &format!("Ellipse 50 mm bậc ba / khe {gap}"), o);
    }
}

#[test]
fn goc_positive_gap_duong_kinh_le_va_hinh_khac() {
    for d in [49.13, 50.07] {
        let inp = input(330.0, 354.0, &STD_DOTS, types_of(&[cubic_circle(d, 0.00025)], 0.00025), 1.5, 250.0);
        positive_check(&inp, &[cubic_circle(d, 0.0001)], 42, &format!("Ellipse {d} mm / khe 1,5"), PosOpts { diagonal: true, ..PosOpts::default() });
    }
    let g = cubic_circle(50.0, 0.00025);
    let dense = cubic_circle(50.0, 0.0001);
    let inp = input(330.0, 354.0, &STD_DOTS, types_of(&[g.clone(), g], 0.00025), 1.5, 250.0);
    positive_check(
        &inp,
        &[dense.clone(), dense],
        42,
        "Hai bài tròn tương đương / khe 1,5",
        PosOpts { exact_count: Some(42), diagonal: true, ..PosOpts::default() },
    );
    let r = rect(10.13, 12.37);
    let inp = input(64.1, 70.2, &[], types_of(std::slice::from_ref(&r), 0.0), 1.5, 250.0);
    positive_check(&inp, &[r], 20, "Chữ nhật số lẻ / khe 1,5", PosOpts { exact_centre: true, ..PosOpts::default() });
    let inp = input(330.0, 354.0, &STD_DOTS, types_of(&[curve(1.0, 0.00025)], 0.00025), 1.5, 250.0);
    positive_check(&inp, &[curve(1.0, 0.0001)], 24, "Khuôn cong bảy nút / khe 1,5", PosOpts::default());
}

// ---------------------------------------------------------------- dan_be_rectangle_regression.test.js

const RW_MM: f64 = 92.0000000094555;
const RH_MM: f64 = 56.0000000057576;

fn rect_input(angle: i64, n: usize) -> NestInput {
    let base = rotated(&rect(RW_MM, RH_MM), angle);
    input(330.0, 354.0, &STD_DOTS, (0..n).map(|_| TypeIn { groups: base.clone(), curve_error: 0.0 }).collect(), 0.0, 250.0)
}

#[test]
fn goc_rectangle_regression_18_con_khe_0() {
    // Bố cục 3 × 6 dựng độc lập: chạm biên, không chồng.
    let inp = rect_input(0, 1);
    let (left, bottom) = ((330.0 - 3.0 * RW_MM) / 2.0, (354.0 - 6.0 * RH_MM) / 2.0);
    let witness: Vec<OutSlot> = (0..6)
        .flat_map(|row| (0..3).map(move |col| OutSlot { mi: 0, vi: 0, x: left + f64::from(col) * RW_MM, y: bottom + f64::from(row) * RH_MM }))
        .collect();
    assert_eq!(witness.len(), 18);
    assert!(guard(&inp, &witness, None).unwrap().nearest <= 1e-8);
    let mut unsafe_ = witness.clone();
    unsafe_[1].x -= 0.01;
    assert!(guard(&inp, &unsafe_, None).unwrap_err().contains("filled overlap"), "khe 0 vẫn phải bắt chồng thật");
    let mut cases: Vec<(NestInput, String)> = ANGLES.iter().map(|&a| (rect_input(a, 1), format!("Chữ nhật số lẻ thật / nguồn xoay {a}"))).collect();
    cases.push((rect_input(0, 2), "Cùng chữ nhật / hai bài tương đương".into()));
    for (inp, label) in cases {
        let out = nest(&inp);
        assert!(out.slots.len() >= 18, "{label}: phải giữ bố cục 3 × 6, không phải 16 con cũ");
        assert_counts(&inp, &out, &label);
        if inp.types.len() > 1 {
            assert!(spread(&out.counts) <= 1);
        }
        let au = guard(&inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
        for (s, sh) in out.slots.iter().zip(&au.shapes) {
            let (w, h) = (sh.x1 - sh.x0, sh.y1 - sh.y0);
            let rigid = ((w - RW_MM).abs() < 1e-8 && (h - RH_MM).abs() < 1e-8) || ((w - RH_MM).abs() < 1e-8 && (h - RW_MM).abs() < 1e-8);
            assert!(rigid, "{label}: giữ nguyên hai kích thước, chỉ xoay cứng ({s:?})");
        }
        assert!(au.nearest <= 0.005, "{label}: khe 0 không được thêm lại rãnh lưới 0,25 / 0,5 mm ({})", au.nearest);
        assert_centering(&inp, &out, &au, 1e-8, &label);
        if !out.exact {
            let (ox, oy) = au.offset(&inp);
            let trial: Vec<OutSlot> = out.slots.iter().map(|s| OutSlot { x: s.x - ox, y: s.y - oy, ..*s }).collect();
            assert!(guard(&inp, &trial, None).unwrap_err().contains("PON"), "{label}: chỉ PON thật mới được chặn canh tâm");
        }
    }
}

// ---------------------------------------------------------------- dan_be_variable_sizes.test.js

fn variable_input(groups: &[Groups]) -> NestInput {
    let dots = [(10.0, 344.0, 2.5), (320.0, 344.0, 2.5), (10.0, 10.0, 2.5), (320.0, 10.0, 2.5)];
    input(330.0, 354.0, &dots, types_of(groups, 0.0), 1.0, 250.0)
}

fn variable_check(groups: &[Groups], minimum: usize, label: &str) -> NestOut {
    let inp = variable_input(groups);
    let out = nest(&inp);
    assert!(out.slots.len() >= minimum, "{label}: {} < {minimum}", out.slots.len());
    assert_counts(&inp, &out, label);
    let au = guard(&inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
    for (s, sh) in out.slots.iter().zip(&au.shapes) {
        let src = shape(&groups[s.mi], ANGLES[s.vi], 0.0, 0.0);
        assert!((sh.x1 - sh.x0 - src.x1).abs() < 1e-7 && (sh.y1 - sh.y0 - src.y1).abs() < 1e-7, "{label}: chỉ dời / xoay cứng, không co giãn ngầm");
    }
    let (ox, oy) = au.offset(&inp);
    assert!((out.offset.0 - ox).abs() < 1e-7 && (out.offset.1 - oy).abs() < 1e-7, "{label}: lệch tâm khớp khung thật");
    assert_eq!(out.exact, ox.abs() <= 0.001 && oy.abs() <= 0.001, "{label}");
    out
}

#[test]
fn goc_variable_sizes_duong_cong_bay_nut() {
    let live = curve(1.0, 0.025);
    assert_eq!(live[0][0].len(), 99, "làm phẳng như bridge thật: 99 đỉnh");
    let b = shape(&live, 0, 0.0, 0.0);
    assert!((b.x1 - 46.0863872075781).abs() < 1e-9 && (b.y1 - 122.000000012539).abs() < 1e-9, "giữ kích thước thật kể cả bụi số");
    // Mốc gốc 54 / 18 / 12; bản JS đo được 63 / 24 / 14.
    for (scale, minimum) in [(0.6, 63), (1.0, 24), (1.25, 14)] {
        let angles: &[i64] = if scale == 1.0 { &ANGLES } else { &[0, 90] };
        for &a in angles {
            variable_check(&[rotated(&curve(scale, 0.025), a)], minimum, &format!("Đường cong kín bảy nút tỉ lệ {scale}, nguồn xoay {a}"));
        }
    }
    let mixed = variable_check(&[curve(0.6, 0.025), curve(1.0, 0.025)], 20, "Hai khuôn khác tỉ lệ chung một tờ");
    assert!(spread(&mixed.counts) <= 1, "các loại khuôn khác nhau cân bằng {:?}", mixed.counts);
}

#[test]
fn goc_variable_sizes_bui_so_bien() {
    let ck = Clock::default();
    let exact = nester::build_variant(&rect(50.0, 122.0), 0, 0, 0, 1.0, 0.25, &ck).unwrap();
    for noise in [-0.0000000125, 0.0000000125] {
        let dusty = nester::build_variant(&rect(50.0 + noise, 122.0 + noise), 0, 0, 0, 1.0, 0.25, &ck).unwrap();
        assert_eq!((dusty.w_cells, dusty.h_cells), (exact.w_cells, exact.h_cells));
        assert_eq!(dusty.raw_rows, exact.raw_rows, "bụi số không thêm ô vào biên dạng thô");
        assert_eq!(dusty.clear_rows, exact.clear_rows, "bụi số không thêm ô vào biên dạng khe");
    }
    let larger = nester::build_variant(&rect(50.00001, 122.00001), 0, 0, 0, 1.0, 0.25, &ck).unwrap();
    assert_eq!(larger.w_cells, exact.w_cells + 1, "tăng thật về chiều rộng thì làm tròn ra ngoài");
    assert_eq!(larger.h_cells, exact.h_cells + 1, "tăng thật về chiều cao thì làm tròn ra ngoài");
    assert!(larger.raw_rows.iter().any(|row| row.iter().any(|sp| sp.1 > exact.w_cells)), "phần tăng hình học thật không bị bỏ");
    let live = curve(1.0, 0.025);
    for (vi, &a) in ANGLES.iter().enumerate() {
        let v = nester::build_variant(&live, 0, vi, a, 1.0, 0.25, &ck).unwrap();
        for row in &v.raw_rows {
            for sp in row {
                assert!(sp.0 >= 0 && sp.1 <= v.w_cells, "dòng quét xoay {a} nằm trong khung ô");
            }
        }
    }
}

// ---------------------------------------------------------------- dan_be_zero_contact_guard.test.js

#[allow(clippy::too_many_arguments)]
fn contact(a: &Groups, b: &Groups, x: f64, y: f64, gap: f64, dots: &[Dot], tx: f64, ty: f64) -> bool {
    let (ck, step) = (Clock::default(), 0.25);
    let vs = vec![nester::build_variant(a, 0, 0, 0, gap, step, &ck).unwrap(), nester::build_variant(b, 1, 0, 0, gap, step, &ck).unwrap()];
    let plan = [PSlot { mi: 0, v: 0, x: tx / step, y: ty / step }, PSlot { mi: 1, v: 1, x: (x + tx) / step, y: (y + ty) / step }];
    nester::verify_geometry(&plan, &vs, (100.0 / step, 100.0 / step), dots, gap, step, &ck)
}

fn cyclic(path: &[P], n: usize, reverse: bool) -> Vec<P> {
    let mut v: Vec<P> = path[n..].iter().chain(&path[..n]).copied().collect();
    if reverse {
        v.reverse();
    }
    v
}

#[test]
fn goc_zero_contact_guard() {
    let square = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    let elbow = [[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 4.0], [0.0, 4.0]];
    let pocket = [[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0]];
    let one = |c: Vec<P>| vec![vec![c]];
    for reverse in [false, true] {
        for i in 0..elbow.len() {
            for j in 0..pocket.len() {
                assert!(
                    contact(&one(cyclic(&elbow, i, reverse)), &one(cyclic(&pocket, j, !reverse)), 1.0, 1.0, 0.0, &[], 0.0, 0.0),
                    "cạnh lõm chung hợp lệ: {i}/{j}/{reverse}"
                );
            }
        }
        for i in 0..4 {
            for j in 0..4 {
                assert!(
                    !contact(&one(cyclic(&square, i, reverse)), &one(cyclic(&square, j, !reverse)), 0.0, 0.0, 0.0, &[], 0.0, 0.0),
                    "vuông trùng khít là chồng: {i}/{j}"
                );
            }
        }
    }
    let tri = poly(&[[0.0, 0.0], [6.0, 0.0], [0.0, 6.0]]);
    let touching = poly(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]]);
    assert!(contact(&tri, &touching, 3.0, 3.0, 0.0, &[], 0.0, 0.0), "góc chạm cạnh chéo là tiếp xúc");
    assert!(!contact(&tri, &touching, 2.99, 2.99, 0.0, &[], 0.0, 0.0), "chồng nhỏ thật quanh chỗ chạm phải bị từ chối");
    let sq = poly(&square);
    assert!(contact(&sq, &sq, 4.0, 0.0, 0.0, &[], 0.0, 0.0), "khe 0 cho chung một cạnh thẳng");
    assert!(contact(&sq, &sq, 4.0, 4.0, 0.0, &[], 0.0, 0.0), "khe 0 cho chạm góc");
    assert!(!contact(&sq, &sq, 4.0, 0.0, 1.0, &[], 0.0, 0.0), "khe dương đã nhập vẫn bắt buộc");
    assert!(contact(&sq, &sq, 5.0, 0.0, 1.0, &[], 0.0, 0.0), "cách đúng khe dương vẫn hợp lệ");
    for (tx, ty) in [(0.133, 0.277), (71.49, 65.89), (12.123456789, 8.987654321)] {
        assert!(contact(&poly(&elbow), &poly(&pocket), 1.0, 1.0, 0.0, &[], tx, ty), "dời lẻ không biến tiếp xúc lõm thành chồng");
        assert!(!contact(&sq, &sq, 3.99, 0.0, 0.0, &[], tx, ty), "dời lẻ không che chồng 0,01 mm thật");
    }
    let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let hole = vec![[3.0, 3.0], [7.0, 3.0], [7.0, 7.0], [3.0, 7.0]];
    let ring: Groups = vec![vec![outer.clone(), hole]];
    assert!(contact(&ring, &sq, 3.0, 3.0, 0.0, &[], 0.0, 0.0), "khuôn vừa lỗ compound chẵn/lẻ được chung biên");
    assert!(!contact(&ring, &sq, 3.01, 3.0, 0.0, &[], 0.0, 0.0), "lệch từ lỗ sang vành tô là chồng");
    assert!(!contact(&ring, &ring, 0.0, 0.0, 0.0, &[], 0.0, 0.0), "hai vành compound trùng nhau chồng vùng tô");
    assert!(
        contact(&vec![vec![outer.clone(), outer.clone()]], &vec![vec![outer.clone()]], 0.0, 0.0, 0.0, &[], 0.0, 0.0),
        "hai đường trùng trong một compound chẵn/lẻ triệt tiêu"
    );
    assert!(
        !contact(&vec![vec![outer.clone()], vec![outer.clone()]], &vec![vec![outer]], 0.0, 0.0, 0.0, &[], 0.0, 0.0),
        "hai nhóm hợp trùng nhau vẫn tô"
    );
    let step = 0.25;
    assert!(
        !contact(&sq, &sq, 5.0, 0.0, 0.0, &[Dot { x: 2.0 / step, y: 2.0 / step, radius: 1.0 / step }], 0.0, 0.0),
        "né PON không được lùi khi cho khuôn chạm khuôn"
    );
}

// ---------------------------------------------------------------- dan_be_zero_gap.test.js

fn core_contact(types: &[Groups], slots: &[(usize, f64, f64)], expect: bool, label: &str, gap: f64) {
    let inp = NestInput { margin: 0.0, pon_clear: 0.0, ..input(100.0, 100.0, &[], types_of(types, 0.0), gap, 250.0) };
    let out: Vec<OutSlot> = slots.iter().map(|&(mi, x, y)| OutSlot { mi, vi: 0, x, y }).collect();
    match guard(&inp, &out, None) {
        Ok(_) => assert!(expect, "{label}: chốt chặn độc lập phải bắt chồng"),
        Err(e) => assert!(!expect && e.contains("filled overlap"), "{label}: {e}"),
    }
    let ck = Clock::default();
    let vs: Vec<Variant> = types.iter().enumerate().map(|(mi, g)| nester::build_variant(g, mi, 0, 0, gap, 0.25, &ck).unwrap()).collect();
    let plan: Vec<PSlot> = slots.iter().map(|&(mi, x, y)| PSlot { mi, v: mi, x: x / 0.25, y: y / 0.25 }).collect();
    assert_eq!(nester::verify_geometry(&plan, &vs, (400.0, 400.0), &[], gap, 0.25, &ck), expect, "{label}");
}

#[test]
fn goc_zero_gap_cham_bien() {
    let elbow = poly(&[[10.0, 15.0], [10.0, 30.0], [0.0, 30.0], [0.0, 0.0], [30.0, 0.0], [30.0, 10.0], [10.0, 10.0]]);
    core_contact(&[elbow, rect(20.0, 20.0)], &[(0, 0.0, 0.0), (1, 10.0, 10.0)], true, "miếng chèn chạm cả hai cạnh trong của chữ L", 0.0);
    let holed: Groups = vec![vec![rect(40.0, 40.0)[0][0].clone(), vec![[10.0, 10.0], [10.0, 30.0], [30.0, 30.0], [30.0, 10.0]]]];
    core_contact(&[holed, rect(20.0, 20.0)], &[(0, 0.0, 0.0), (1, 10.0, 10.0)], true, "miếng chèn chung cả bốn biên của lỗ compound thật", 0.0);
    core_contact(&[rect(40.0, 40.0), rect(20.0, 20.0)], &[(0, 0.0, 0.0), (1, 10.0, 10.0)], false, "nằm trọn trong vùng tô là chồng kể cả khe 0", 0.0);
    core_contact(&[rect(10.0, 10.0), rect(10.0, 10.0)], &[(0, 0.0, 0.0), (1, 0.0, 0.0)], false, "hai đường trùng khít không lọt kiểm tra", 0.0);
    let fr = rect(10.13, 12.37);
    core_contact(&[fr.clone(), fr.clone()], &[(0, 0.0, 0.0), (1, 0.0, 12.25)], false, "hàng cuối lấy mẫu trống không che chồng 0,12 mm thật", 0.0);
    core_contact(&[fr.clone(), fr.clone()], &[(0, 0.0, 0.0), (1, 0.0, 12.37 - 0.00001)], false, "khe 0 không cho chồng dương thật", 0.0);
    core_contact(&[fr.clone(), fr], &[(0, 0.0, 0.0), (1, 0.0, 12.37 - 0.00000096)], false, "chồng 0,00000096 mm không phải bụi số", 0.0);
    core_contact(&[rect(10.0, 10.0), rect(10.0, 10.0)], &[(0, 0.0, 0.0), (1, 10.0, 0.0)], true, "khe 0 cho chung một cạnh thẳng", 0.0);
    let ck = Clock::default();
    let pv: Vec<Variant> = (0..2).map(|mi| nester::build_variant(&rect(10.0, 10.0), mi, 0, 0, 0.01, 0.25, &ck).unwrap()).collect();
    let touching = [PSlot { mi: 0, v: 0, x: 0.0, y: 0.0 }, PSlot { mi: 1, v: 1, x: 40.0, y: 0.0 }];
    assert!(!nester::verify_geometry(&touching, &pv, (400.0, 400.0), &[], 0.01, 0.25, &ck), "cho chạm ở khe 0 không làm yếu khe dương thật");
}

fn zero_public(inp: &NestInput, minimum: usize, label: &str) -> NestOut {
    let out = nest(inp);
    assert!(out.slots.len() >= minimum, "{label}: {} < {minimum}", out.slots.len());
    assert_counts(inp, &out, label);
    let au = guard(inp, &out.slots, None).unwrap_or_else(|e| panic!("{label}: {e}"));
    if inp.gap == 0.0 {
        assert!(au.nearest <= 0.01, "{label}: khe 0 không giữ rãnh lưới 0,25 / 0,5 mm cố định ({})", au.nearest);
    } else {
        assert!(au.nearest >= inp.gap - 0.001, "{label}: khe đo độc lập {} dưới khe yêu cầu", au.nearest);
    }
    out
}

#[test]
fn goc_zero_gap_cong_khai() {
    let r = rect(10.13, 12.37);
    zero_public(&input(64.1, 70.2, &[], types_of(&[r], 0.0), 0.0, 750.0), 24, "Chữ nhật số lẻ, khe 0");
    zero_public(&input(330.0, 354.0, &STD_DOTS, types_of(&[circle(25.0)], 0.0), 0.0, 750.0), 42, "Tròn 50 mm, khe 0");
    let fine = input(330.0, 354.0, &STD_DOTS, types_of(&[curve(1.0, 0.00025)], 0.00025), 0.0, 250.0);
    assert!(fine.types[0].groups[0][0].len() > 1000);
    let out = zero_public(&fine, 24, "Đường cong kín 46,086 × 122 mm mịn, khe 0");
    // Đường bao dày hơn hẳn, sinh độc lập: dùng lại đa giác lúc dàn từng che giao cắt thật.
    let dense = input(330.0, 354.0, &STD_DOTS, types_of(&[curve(1.0, 0.00001)], 0.0), 0.0, 250.0);
    assert!(dense.types[0].groups[0][0].len() > 4000);
    let au = guard(&dense, &out.slots, None).unwrap();
    assert!(au.nearest >= 0.0 && au.nearest <= 0.01, "Bézier dày: khe thật gần nhất {} vẫn phải chạm thấy được", au.nearest);
    let positive = NestInput { gap: 1.0, ..input(330.0, 354.0, &STD_DOTS, types_of(&[curve(1.0, 0.025)], 0.0), 0.0, 250.0) };
    zero_public(&positive, 24, "Đường cong kín, khe dương 1 mm thật");
}

// ================================================================ phần bridge (Prepare / Render)

mod bridge {
    use std::sync::Arc;

    use serde_json::{Value, json};
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
    use vectorcraft_geom::{Affine, Anchor, FillRule, PathData, Point, Rect, SubPath};

    use super::super::nester::{NestOut, OutSlot};
    use super::super::{
        CANVAS_HALF, Die, LARGE_CANVAS_HALF, MM, Model, PAGE_GAP, SheetPlan, nest_input, opts, output_positions, plan_positions, plan_sheets,
        read_die, render, report, source_models, triple_models,
    };
    use super::{Groups, Node3, P, cubic_circle_nodes, curve_nodes, n3, rect, shape};
    use crate::Session;

    const GAP: f64 = PAGE_GAP * MM;

    // ------------------------------------------------------------ dựng tài liệu thử

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800, "height": 600, "colorMode": "cmyk"})).unwrap();
        s
    }

    fn ink() -> Appearance {
        Appearance::basic(Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0)), Paint::None, 0.0)
    }

    /// Toạ độ pt kiểu Illustrator (y lên) → trang (y xuống, tờ 600 pt).
    fn up(x: f64, y: f64) -> Point {
        Point::new(x, 600.0 - y)
    }

    fn poly_node(id: NodeId, name: &str, pts: &[Point]) -> Node {
        let mut n = Node::path(id, PathData::single(SubPath::polyline(pts, true)), ink());
        n.name = Some(name.into());
        n
    }

    /// Thêm `n` vào layer trên cùng; trả id.
    fn put(s: &mut Session, make: impl FnOnce(&mut Document) -> Node) -> NodeId {
        s.edit("thử", |d, _| {
            let layer = d.layers.last().map(|l| l.id);
            let n = make(d);
            Ok(d.insert(layer, usize::MAX, n)?)
        })
        .unwrap()
    }

    fn rect_up(s: &mut Session, name: &str, l: f64, t: f64, r: f64, b: f64) -> NodeId {
        let name = name.to_string();
        put(s, move |d| poly_node(d.alloc_id(), &name, &[up(l, b), up(r, b), up(r, t), up(l, t)]))
    }

    /// Khuôn chữ L 20 × 30 pt của test hai mặt (không đối xứng, để thấy lật gương).
    fn ell_up(s: &mut Session, name: &str, x: f64, y: f64) -> NodeId {
        let name = name.to_string();
        let pts = [[x, y], [x + 20.0, y], [x + 20.0, y + 10.0], [x + 5.0, y + 10.0], [x + 5.0, y + 30.0], [x, y + 30.0]].map(|p| up(p[0], p[1]));
        put(s, move |d| poly_node(d.alloc_id(), &name, &pts))
    }

    /// Bài không đối xứng (tam giác vuông) trong khung l..r × b..t: lật gương sẽ đổi chiều diện tích.
    fn tri_up(s: &mut Session, name: &str, l: f64, t: f64, r: f64, b: f64) -> NodeId {
        let name = name.to_string();
        put(s, move |d| poly_node(d.alloc_id(), &name, &[up(l, b), up(r, b), up(l, t)]))
    }

    fn doc(s: &Session) -> &Document {
        &s.doc().unwrap().doc
    }

    fn anchors_of(n: &Node) -> Vec<Point> {
        match &n.kind {
            NodeKind::Path { path, .. } => path.subpaths.iter().flat_map(|sp| sp.anchors.iter().map(|a| a.p)).collect(),
            _ => Vec::new(),
        }
    }

    fn signed_area(pts: &[Point]) -> f64 {
        let n = pts.len();
        (0..n).map(|i| pts[i].x * pts[(i + 1) % n].y - pts[(i + 1) % n].x * pts[i].y).sum::<f64>() / 2.0
    }

    fn layer<'a>(d: &'a Document, name: &str) -> &'a Node {
        d.layers.iter().find(|l| l.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("thiếu layer {name}"))
    }

    /// Các object của layer theo thứ tự đặt (con đầu tiên của phương án trước): lớp vẽ được nạp
    /// ngược, con sau nằm dưới con trước như PLACEATEND của DanCard.
    fn placed<'a>(d: &'a Document, name: &str) -> Vec<&'a Node> {
        let mut v: Vec<&Node> = layer(d, name).children().unwrap().iter().map(|c| &**c).collect();
        v.reverse();
        v
    }

    fn deg(a: f64) -> i64 {
        (a.round() as i64).rem_euclid(360)
    }

    fn prepared(s: &Session, models: &[Model]) -> Vec<(Model, Die)> {
        models.iter().enumerate().map(|(mi, m)| (*m, read_die(doc(s).node(m.khuon).unwrap(), mi).unwrap())).collect()
    }

    fn sheet(model: Option<usize>, slots: &[(usize, usize, f64, f64)]) -> SheetPlan {
        let slots: Vec<OutSlot> = slots.iter().map(|&(mi, vi, x, y)| OutSlot { mi, vi, x, y }).collect();
        SheetPlan {
            model, out: NestOut { counts: Vec::new(), mode: String::new(), exact: true, offset: (0.0, 0.0), detail: "Tờ thử".into(), slots }
        }
    }

    // ------------------------------------------------------------ dan_be_cut_outline.test.js

    const W: f64 = 40.0;
    const H: f64 = 70.0;

    fn pg(p: P) -> Point {
        Point::new((10.0 + p[0]) * MM, (200.0 - p[1]) * MM)
    }

    fn anchors(nodes: &[Node3]) -> Vec<Anchor> {
        nodes.iter().map(|n| Anchor::with_handles(pg(n.a), pg(n.l), pg(n.r))).collect()
    }

    fn rect_nodes() -> Vec<Node3> {
        [[0.0, 0.0], [W, 0.0], [W, H], [0.0, H]].map(|p| n3(p, p, p)).to_vec()
    }

    /// Đường bao kín hai cung bậc ba: mỗi nửa tới cực trị dọc ở t = ½; chỉ hai điểm neo thì khung
    /// của chúng cao 0, nên thay bằng khung hay đòi ≥ 3 điểm neo đều không đúng khuôn thật.
    fn two_anchor_nodes() -> Vec<Node3> {
        let (mid, reach) = (H / 2.0, H * 2.0 / 3.0);
        vec![n3([0.0, mid], [0.0, mid - reach], [0.0, mid + reach]), n3([W, mid], [W, mid + reach], [W, mid - reach])]
    }

    fn die_node(kind: &str, red_first: bool, two_anchors: bool) -> Node {
        let mut next = 100u64;
        let mut id = || {
            next += 1;
            NodeId(next)
        };
        let outer_nodes = if two_anchors { two_anchor_nodes() } else { rect_nodes() };
        let outer = Node::path(id(), PathData::single(SubPath::new(anchors(&outer_nodes), true)), ink());
        let red: Vec<Node3> = cubic_circle_nodes(14.0)
            .iter()
            .map(|n| {
                let o = |p: P| [p[0] + 13.0, p[1] + 28.0];
                n3(o(n.a), o(n.l), o(n.r))
            })
            .collect();
        let hole = Node::path(id(), PathData::single(SubPath::new(anchors(&red), true)), ink());
        let kids: Vec<Arc<Node>> = if red_first { vec![hole, outer] } else { vec![outer, hole] }.into_iter().map(Arc::new).collect();
        match kind {
            "compound" => Node::new(id(), NodeKind::Compound { children: kids, rule: FillRule::EvenOdd }),
            "group" => Node::group(id(), kids),
            _ => {
                let inner = Node::new(id(), NodeKind::Group { children: kids, clip: true });
                Node::new(id(), NodeKind::Group { children: vec![Arc::new(inner)], clip: true })
            }
        }
    }

    fn bounds(c: &[P]) -> (f64, f64, f64, f64) {
        c.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |b, p| {
            (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1]))
        })
    }

    fn near(a: f64, b: f64, what: &str) {
        assert!((a - b).abs() < 1e-7, "{what}: {a} ≠ {b}");
    }

    fn cubic_at(a: P, b: P, c: P, d: P, t: f64) -> P {
        super::bez(a, b, c, d, t)
    }

    fn outline_check(kind: &str, red_first: bool, two_anchors: bool) -> Vec<String> {
        let label =
            format!("{kind} / {} / {}", if red_first { "đỏ trước" } else { "ngoài trước" }, if two_anchors { "hai neo cong" } else { "chữ nhật" });
        let die = read_die(&die_node(kind, red_first, two_anchors), 0).unwrap_or_else(|e| panic!("{label}: {e}"));
        let groups: &Groups = &die.groups;
        let all: Vec<P> = groups.iter().flatten().flatten().copied().collect();
        let b = bounds(&all);
        near(b.0, 0.0, &label);
        near(b.1, 0.0, &label);
        near(b.2, W, &format!("{label}: khung dàn gồm cả bề ngang đường bế đen, không phải đường kính vòng đỏ"));
        near(b.3, H, &format!("{label}: khung dàn gồm cả bề cao đường bế đen"));
        let exterior = groups
            .iter()
            .flatten()
            .find(|c| {
                let cb = bounds(c);
                (cb.2 - cb.0 - W).abs() < 1e-7 && (cb.3 - cb.1 - H).abs() < 1e-7
            })
            .unwrap_or_else(|| panic!("{label}: phải còn đường bao ngoài thật"))
            .clone();
        let sh = shape(groups, 0, 0.0, 0.0);
        assert!(super::inside([20.0, 60.0], &sh), "{label}: vùng tô ngoài vòng đỏ vẫn thuộc khuôn");
        assert!(die.curve, "{label}: đường cong mang theo dự trữ sai số làm phẳng");
        if kind == "compound" {
            assert_eq!(groups.len(), 1, "{label}: CompoundPath thật là một nhóm chẵn/lẻ");
            assert_eq!(groups[0].len(), 2, "{label}: giữ cả đường ngoài và lỗ compound bất kể thứ tự");
            assert!(!super::inside([20.0, 35.0], &sh), "{label}: lỗ tròn compound giữ nghĩa chẵn/lẻ");
        } else {
            assert!(super::inside([20.0, 35.0], &sh), "{label}: group thường là phép hợp, không thành lỗ");
        }
        if two_anchors {
            assert!(exterior.len() > 100, "{label}: đường cong hai neo thành đường bao chính xác, không phải dây cung");
            let nodes = two_anchor_nodes();
            let mut worst = 0.0f64;
            for (i, a) in nodes.iter().enumerate() {
                let n = nodes[(i + 1) % nodes.len()];
                for k in 0..=128 {
                    let p = cubic_at(a.a, a.r, n.l, n.a, f64::from(k) / 128.0);
                    let d = (0..exterior.len())
                        .map(|e| super::point_segment(p, exterior[e], exterior[(e + 1) % exterior.len()]))
                        .fold(f64::INFINITY, f64::min);
                    worst = worst.max(d);
                }
            }
            assert!(worst <= 0.00025 + 1e-7, "{label}: sai số Bézier {worst} mm vượt dự trữ");
            assert!(!super::inside([1.0, 65.0], &sh), "{label}: đường bao hai neo không bị thay bằng khung");
        } else {
            assert_eq!(exterior.len(), 4, "{label}: đường thẳng giữ đúng 4 đỉnh");
        }
        let mut summary: Vec<String> = groups.iter().flatten().map(|c| format!("{:?}/{}", bounds(c), c.len())).collect();
        summary.sort();
        summary
    }

    #[test]
    fn goc_cut_outline_doc_khuon() {
        for kind in ["clipped", "group", "compound"] {
            for two in [false, true] {
                let first = outline_check(kind, false, two);
                assert_eq!(outline_check(kind, true, two), first, "{kind}: thứ tự con không đổi đường bao dàn");
            }
        }
    }

    // ------------------------------------------------------------ dan_be_custom_sheet.test.js

    fn args(paper: [&str; 6], gap: &str) -> Value {
        json!({"gap": gap, "margin": "4", "ponClear": "7.5", "paperW": paper[0], "paperH": paper[1],
            "ponTop": paper[2], "ponBottom": paper[3], "ponLeft": paper[4], "ponRight": paper[5]})
    }

    fn sorted_dots(o: &super::super::Opts) -> Vec<(f64, f64, f64)> {
        let mut v: Vec<(f64, f64, f64)> = nest_input(o, &[]).dots.iter().map(|d| (d.x, d.y, d.r)).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        v
    }

    fn assert_dots(got: &[(f64, f64, f64)], want: &[(f64, f64, f64)]) {
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            assert!((g.0 - w.0).abs() < 1e-5 && (g.1 - w.1).abs() < 1e-5 && (g.2 - w.2).abs() < 1e-5, "{got:?} ≠ {want:?}");
        }
    }

    #[test]
    fn goc_custom_sheet_mac_dinh_va_tuy_chinh() {
        // Thiếu tham số là dùng mặc định đã mô tả, không phải hỏi file PON.
        let o = opts(&json!({})).unwrap();
        assert!((o.paper_w - 330.0).abs() < 1e-9 && (o.paper_h - 354.0).abs() < 1e-9);
        assert_dots(&sorted_dots(&o), &[(10.0, 10.0, 2.5), (10.0, 344.0, 2.5), (320.0, 10.0, 2.5), (320.0, 344.0, 2.5)]);
        // Khổ giấy cm; bốn khoảng mm từ mép giấy tới TÂM chấm; dấu phẩy thập phân.
        let o = opts(&args(["42,3", "29,7", "13,25", "17,5", "21,75", "25,25"], "2")).unwrap();
        assert!((o.paper_w - 423.0).abs() < 1e-9 && (o.paper_h - 297.0).abs() < 1e-9);
        assert_dots(&sorted_dots(&o), &[(21.75, 17.5, 2.5), (21.75, 283.75, 2.5), (397.75, 17.5, 2.5), (397.75, 283.75, 2.5)]);
        // Cả chấm 5 mm vẫn nằm trong giấy khi chạm đúng mép.
        let o = opts(&args(["3", "4", "2.5", "2.5", "2.5", "2.5"], "2")).unwrap();
        for (x, y, r) in sorted_dots(&o) {
            assert!(x >= r && y >= r && x + r <= 30.0 + 1e-9 && y + r <= 40.0 + 1e-9);
        }
        // Hai chấm đối diện được chạm nhau nhưng không chồng.
        opts(&args(["3", "4", "10", "10", "12.5", "12.5"], "2")).unwrap();
    }

    #[test]
    fn goc_custom_sheet_tu_choi_so_sai() {
        let invalid: [[&str; 6]; 19] = [
            ["0", "35.4", "10", "10", "10", "10"],
            ["-1", "35.4", "10", "10", "10", "10"],
            ["5001", "35.4", "10", "10", "10", "10"],
            ["33", "5001", "10", "10", "10", "10"],
            ["33cm", "35.4", "10", "10", "10", "10"],
            ["Infinity", "35.4", "10", "10", "10", "10"],
            ["NaN", "35.4", "10", "10", "10", "10"],
            ["33", "29,7,1", "10", "10", "10", "10"],
            ["33", "35.4", "10junk", "10", "10", "10"],
            ["33", "35.4", "-1", "10", "10", "10"],
            ["33", "35.4", "2.499", "10", "10", "10"],
            ["33", "35.4", "10", "2.499", "10", "10"],
            ["33", "35.4", "10", "10", "2.499", "10"],
            ["33", "35.4", "10", "10", "10", "2.499"],
            ["33", "35.4", "355", "10", "10", "10"],
            ["3", "4", "10", "10", "13", "13"],
            ["3", "4", "10", "10", "12.5005", "12.5005"],
            ["3", "4", "18", "18", "10", "10"],
            ["3", "4", "10", "10", "20", "20"],
        ];
        for a in invalid {
            assert!(opts(&args(a, "2")).is_err(), "phải từ chối {a:?}");
        }
        for field in ["gap", "margin", "ponClear"] {
            let mut p = args(["33", "35.4", "10", "10", "10", "10"], "2");
            p[field] = json!(format!("{}mm", p[field].as_str().unwrap()));
            assert!(opts(&p).is_err(), "khe / lề / né PON không nhận hậu tố đơn vị: {field}");
        }
        // Canvas thường của DanCard quá hẹp cho tờ 510 cm và cặp hai mặt 260 cm; VectorCraft không có
        // tuỳ chọn Large Canvas nên dùng luôn vùng canvas rộng gấp 10 thay cho báo lỗi.
        let o = opts(&args(["510", "35.4", "10", "10", "10", "10"], "2")).unwrap();
        let e = output_positions(o.paper_w * MM, o.paper_h * MM, false, 1, CANVAS_HALF, &[], None).unwrap_err();
        assert!(e.contains("canvas"), "{e}");
        assert!(plan_positions(o.paper_w * MM, o.paper_h * MM, false, 1, &[], None).is_ok());
        let o = opts(&json!({"paperW": "260", "twoSided": true})).unwrap();
        assert!(output_positions(o.paper_w * MM, o.paper_h * MM, true, 1, CANVAS_HALF, &[], None).unwrap_err().contains("canvas"));
        assert!(output_positions(o.paper_w * MM, o.paper_h * MM, true, 1, LARGE_CANVAS_HALF, &[], None).is_ok());
    }

    #[test]
    fn goc_custom_sheet_duong_cong_khe_0_mịn() {
        // Đường cong bảy nút: cùng 1027 đỉnh ở mọi khe, sai số Bézier thật ≤ 0,00025 mm.
        let page = |p: P| Point::new(10.0 + p[0] * MM, 600.0 - (20.0 + p[1] * MM));
        let nodes = curve_nodes();
        let a: Vec<Anchor> = nodes.iter().map(|n| Anchor::with_handles(page(n.a), page(n.l), page(n.r))).collect();
        let n = Node::path(NodeId(1), PathData::single(SubPath::new(a, true)), ink());
        let die = read_die(&n, 0).unwrap();
        assert!(die.curve);
        let c = &die.groups[0][0];
        assert_eq!(c.len(), 1027, "đường cong khe dương và khe 0 dùng cùng biên dạng đường cong thật");
        let x0 = nodes.iter().map(|n| n.a[0]).fold(f64::INFINITY, f64::min);
        let y0 = nodes.iter().map(|n| n.a[1]).fold(f64::INFINITY, f64::min);
        let mut worst = 0.0f64;
        for (i, a) in nodes.iter().enumerate() {
            let b = nodes[(i + 1) % nodes.len()];
            for k in 0..=256 {
                let p = cubic_at(a.a, a.r, b.l, b.a, f64::from(k) / 256.0);
                let p = [p[0] - x0, p[1] - y0];
                let d = (0..c.len()).map(|e| super::point_segment(p, c[e], c[(e + 1) % c.len()])).fold(f64::INFINITY, f64::min);
                worst = worst.max(d);
            }
        }
        assert!(worst <= 0.00025 + 1e-8, "khoảng cách tới Bézier gốc {worst} mm vượt dung sai làm phẳng");
        // Đường thẳng giữ đúng, không dự trữ sai số Bézier không có.
        let straight = Node::path(
            NodeId(2),
            PathData::single(SubPath::polyline(
                &[page([0.0, 0.0]), page([20.0, 0.0]), page([20.0, 10.0]), page([6.0, 10.0]), page([6.0, 30.0]), page([0.0, 30.0])],
                true,
            )),
            ink(),
        );
        let die = read_die(&straight, 0).unwrap();
        assert!(!die.curve);
        assert_eq!(die.groups[0][0].len(), 6);
    }

    // ------------------------------------------------------------ dan_be_sheets.test.js

    struct Fixture {
        s: Session,
        models: Vec<Model>,
    }

    /// Hai mẫu: khuôn 20 × 30 pt và bài 22 × 32 pt (tên cut0/art0, cut1/art1).
    fn sheets_fixture() -> Fixture {
        let mut s = session();
        let mut models = Vec::new();
        for i in 0..2 {
            let khuon = rect_up(&mut s, &format!("cut{i}"), 10.0, 50.0, 30.0, 20.0);
            let bai = rect_up(&mut s, &format!("art{i}"), 40.0, 51.0, 62.0, 19.0);
            models.push(Model { khuon, bai, sau: None });
        }
        Fixture { s, models }
    }

    fn per_model() -> Vec<SheetPlan> {
        vec![sheet(Some(0), &[(0, 0, 10.0, 10.0), (0, 1, 30.0, 50.0)]), sheet(Some(1), &[(1, 2, 10.0, 10.0), (1, 3, 30.0, 50.0)])]
    }

    fn run_render(f: &mut Fixture, o: &super::super::Opts, separate: bool, plans: &[SheetPlan]) -> crate::Result<super::super::Rendered> {
        let prep = prepared(&f.s, &f.models);
        f.s.edit("Dàn bế (thử)", |d, sel| {
            let r = render(d, &prep, o, separate, plans)?;
            sel.clear();
            Ok(r)
        })
    }

    #[test]
    fn goc_sheets_to_rieng_va_tron() {
        let o = opts(&json!({})).unwrap();
        let mut f = sheets_fixture();
        let before: Vec<Option<Rect>> =
            f.models.iter().flat_map(|m| [m.khuon, m.bai]).map(|id| doc(&f.s).node(id).unwrap().geometric_bounds()).collect();
        let layers0 = doc(&f.s).layers.len();
        let r = run_render(&mut f, &o, true, &per_model()).unwrap();
        let msg = report(true, false, 2, r.total, &r.counts, Some("Tờ thử"));
        assert!(msg.starts_with("OK: Đã tạo 2 artboard riêng; tổng 4 con."), "{msg}");
        assert!(!msg.contains("[[COUNT:"), "tổng nhiều tờ không được báo như số con một tờ");
        let d = doc(&f.s);
        assert_eq!(d.artboards.len(), 2, "artboard cũ bị xoá, mỗi mẫu một tờ");
        assert_eq!(d.layers.len(), layers0 + 6);
        let plans = per_model();
        for (si, ab) in d.artboards.iter().enumerate() {
            let rect = ab.rect;
            assert!((rect.width() - 330.0 * MM).abs() < 1e-5 && (rect.height() - 354.0 * MM).abs() < 1e-5);
            let suffix = format!(" - mẫu {}", si + 1);
            let cuts = placed(d, &format!("Dàn bế - Khuôn{suffix}"));
            let arts = placed(d, &format!("Dàn bế - Bài{suffix}"));
            let dots = placed(d, &format!("Dàn bế - PON{suffix}"));
            assert_eq!((cuts.len(), arts.len(), dots.len()), (2, 2, 4));
            for (i, (k, a)) in cuts.iter().zip(&arts).enumerate() {
                assert_eq!(k.name.as_deref(), Some(format!("cut{si}").as_str()), "tờ riêng chỉ có mẫu của nó");
                assert_eq!(a.name.as_deref(), Some(format!("art{si}").as_str()));
                assert_eq!(deg(k.bbox_angle), deg(a.bbox_angle), "bài xoay cùng khuôn");
                let (kb, ab2) = (k.geometric_bounds().unwrap(), a.visual_bounds().unwrap());
                assert!((kb.center() - ab2.center()).hypot() < 1e-5, "bài nằm giữa khuôn");
                let s = plans[si].out.slots[i];
                assert!((kb.x0 - (rect.x0 + s.x * MM)).abs() < 1e-5 && (kb.y1 - (rect.y1 - s.y * MM)).abs() < 1e-5, "{kb:?} ≠ chỗ dàn {s:?}");
                assert_eq!(deg(k.bbox_angle), [0, 90, 180, 270][s.vi]);
            }
            let mut centres: Vec<(f64, f64)> = dots
                .iter()
                .map(|n| {
                    let b = n.geometric_bounds().unwrap();
                    assert!((b.width() - 5.0 * MM).abs() < 1e-6 && (b.height() - 5.0 * MM).abs() < 1e-6, "PON tròn 5 mm");
                    ((b.center().x - rect.x0) / MM, (b.center().y - rect.y0) / MM)
                })
                .collect();
            centres.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for (c, w) in centres.iter().zip([(10.0, 10.0), (10.0, 344.0), (320.0, 10.0), (320.0, 344.0)]) {
                assert!((c.0 - w.0).abs() < 1e-6 && (c.1 - w.1).abs() < 1e-6, "{centres:?}");
            }
        }
        // Như DanCard (dcDanBeOutputPositions): tờ đầu ở góc trên trái canvas, tờ sau bên phải cách 10 mm.
        let (a0, a1) = (d.artboards[0].rect, d.artboards[1].rect);
        assert!((a0.x0 - (-CANVAS_HALF + GAP)).abs() < 1e-6 && (a0.y0 - (-CANVAS_HALF + GAP)).abs() < 1e-6, "{a0:?}");
        assert!((a1.x0 - (a0.x1 + GAP)).abs() < 1e-6 && (a1.y0 - a0.y0).abs() < 1e-6, "{a1:?}");
        let after: Vec<Option<Rect>> = f.models.iter().flat_map(|m| [m.khuon, m.bai]).map(|id| d.node(id).unwrap().geometric_bounds()).collect();
        assert_eq!(before, after, "nguồn giữ nguyên");

        // Tờ chung (trộn mẫu): giao thức một tờ, báo [[COUNT:2]].
        let mut f = sheets_fixture();
        let mixed = vec![sheet(None, &[(0, 0, 10.0, 10.0), (1, 2, 30.0, 50.0)])];
        let r = run_render(&mut f, &o, false, &mixed).unwrap();
        assert!(report(false, false, 1, r.total, &r.counts, Some("Tờ thử")).starts_with("OK:[[COUNT:2]] Tờ thử"));
        let d = doc(&f.s);
        assert_eq!(d.artboards.len(), 1);
        let cuts = placed(d, "Dàn bế - Khuôn");
        assert_eq!(cuts.iter().map(|k| k.name.clone().unwrap()).collect::<Vec<_>>(), ["cut0", "cut1"]);
    }

    #[test]
    fn goc_sheets_tu_choi_ca_lo_khong_tao_gi() {
        let o = opts(&json!({})).unwrap();
        let good = per_model();
        let cases: Vec<(bool, Vec<SheetPlan>)> = vec![
            (true, Vec::new()),
            (true, vec![sheet(Some(0), &[(0, 0, 10.0, 10.0)])]),
            (true, vec![sheet(Some(0), &[(0, 0, 10.0, 10.0)]), sheet(Some(0), &[(0, 0, 10.0, 10.0)])]),
            (true, vec![sheet(Some(0), &[(0, 0, 10.0, 10.0)]), sheet(Some(1), &[(0, 0, 10.0, 10.0)])]),
            (true, vec![sheet(Some(0), &[(0, 0, 10.0, 10.0)]), sheet(Some(1), &[])]),
            (false, vec![sheet(None, &[(2, 0, 1.0, 1.0)])]),
            (false, vec![sheet(None, &[(0, 4, 1.0, 1.0)])]),
            (false, vec![sheet(None, &[(0, 0, -1.0, 1.0)])]),
            (false, vec![sheet(None, &[(0, 0, 330.0, 1.0)])]),
            (false, vec![sheet(None, &[(0, 0, f64::NAN, 1.0)])]),
            (false, vec![sheet(None, &[(0, 0, 1.0, 354.0)])]),
        ];
        for (k, (separate, plans)) in cases.into_iter().enumerate() {
            let mut f = sheets_fixture();
            let layers0 = doc(&f.s).layers.len();
            assert!(run_render(&mut f, &o, separate, &plans).is_err(), "trường hợp {k} phải bị từ chối");
            let d = doc(&f.s);
            assert_eq!((d.artboards.len(), d.layers.len()), (1, layers0), "lô hỏng không tạo tờ nào ({k})");
        }
        let mut f = sheets_fixture();
        assert!(run_render(&mut f, &o, true, &good).is_ok());
    }

    // ------------------------------------------------------------ dan_be_duplex.test.js

    fn duplex_fixture() -> Fixture {
        let mut s = session();
        let mut models = Vec::new();
        for i in 0..2 {
            let dy = 60.0 * f64::from(i);
            let khuon = ell_up(&mut s, &format!("cut{i}"), 10.0, 200.0 - dy);
            let bai = tri_up(&mut s, &format!("front{i}"), 60.0, 231.0 - dy, 82.0, 199.0 - dy);
            let sau = tri_up(&mut s, &format!("back{i}"), 110.0, 231.02 - dy, 132.0, 199.02 - dy);
            models.push(Model { khuon, bai, sau: Some(sau) });
        }
        Fixture { s, models }
    }

    fn duplex_layout() -> Vec<SheetPlan> {
        (0..2).map(|m| sheet(Some(m), &(0..4).map(|vi| (m, vi, 20.0 + vi as f64 * 50.0, 30.0 + vi as f64 * 20.0)).collect::<Vec<_>>())).collect()
    }

    #[test]
    fn goc_duplex_lat_ngang_khong_lat_bai() {
        let o = opts(&json!({"twoSided": true})).unwrap();
        for separate in [false, true] {
            let mut f = duplex_fixture();
            let src_area: Vec<f64> =
                f.models.iter().flat_map(|m| [m.bai, m.sau.unwrap()]).map(|id| signed_area(&anchors_of(doc(&f.s).node(id).unwrap()))).collect();
            let layers0 = doc(&f.s).layers.len();
            let plans = if separate {
                duplex_layout()
            } else {
                vec![sheet(None, &duplex_layout().iter().flat_map(|p| p.out.slots.iter().map(|s| (s.mi, s.vi, s.x, s.y))).collect::<Vec<_>>())]
            };
            let r = run_render(&mut f, &o, separate, &plans).unwrap();
            if !separate {
                let msg = report(false, true, 1, r.total, &r.counts, Some("Tờ thử"));
                assert!(msg.starts_with("OK:[[COUNT:8]]"), "đếm con thật, không cộng hai mặt: {msg}");
            }
            let d = doc(&f.s);
            let n_sheets = plans.len();
            assert_eq!(d.artboards.len(), n_sheets * 2);
            assert_eq!(d.layers.len(), layers0 + n_sheets * 6);
            for (si, plan) in plans.iter().enumerate() {
                let suffix = if separate { format!(" - mẫu {}", si + 1) } else { String::new() };
                let get = |role: &str, side: &str| placed(d, &format!("Dàn bế - {role}{suffix} - mặt {side}"));
                let (front_cuts, back_cuts) = (get("Khuôn", "trước"), get("Khuôn", "sau"));
                let (fronts, backs) = (get("Bài", "trước"), get("Bài", "sau"));
                let (fdots, bdots) = (get("PON", "trước"), get("PON", "sau"));
                let (fr, br) = (d.artboards[si * 2].rect, d.artboards[si * 2 + 1].rect);
                assert!((br.x0 - fr.x1 - GAP).abs() < 1e-6 && (br.y0 - fr.y0).abs() < 1e-6, "mặt sau ngay bên phải, cùng mép trên");
                let w = 330.0 * MM;
                assert_eq!(front_cuts.len(), plan.out.slots.len());
                assert_eq!((back_cuts.len(), fronts.len(), backs.len()), (plan.out.slots.len(), plan.out.slots.len(), plan.out.slots.len()));
                for (i, s) in plan.out.slots.iter().enumerate() {
                    let (fc, bc) = (front_cuts[i].geometric_bounds().unwrap().center(), back_cuts[i].geometric_bounds().unwrap().center());
                    assert_eq!(fronts[i].name.as_deref(), Some(format!("front{}", s.mi).as_str()));
                    assert_eq!(backs[i].name.as_deref(), Some(format!("back{}", s.mi).as_str()));
                    assert_eq!(deg(fronts[i].bbox_angle), [0, 90, 180, 270][s.vi], "bài mặt trước xoay theo khuôn");
                    assert_eq!(deg(backs[i].bbox_angle), [0, 270, 180, 90][s.vi], "bài mặt sau xoay ngược lại");
                    for n in [fronts[i], backs[i], front_cuts[i], back_cuts[i]] {
                        assert!(anchors_of(n).len() >= 3, "bản sao phải là path đủ đỉnh");
                    }
                    assert_eq!(anchors_of(front_cuts[i]).len(), anchors_of(back_cuts[i]).len());
                    let sign = |n: &Node| signed_area(&anchors_of(n)).signum();
                    assert_eq!(sign(fronts[i]), src_area[s.mi * 2].signum(), "không lật gương chữ / hình mặt trước");
                    assert_eq!(sign(backs[i]), src_area[s.mi * 2 + 1].signum(), "không lật gương chữ / hình mặt sau");
                    assert_eq!(sign(back_cuts[i]), -sign(front_cuts[i]), "khuôn mặt sau lật gương");
                    assert!(
                        (fc.x - fr.x0 + bc.x - br.x0 - w).abs() < 1e-5 && ((fc.y - fr.y0) - (bc.y - br.y0)).abs() < 1e-5,
                        "vị trí đối xứng lật ngang"
                    );
                    let (fa, ba) = (fronts[i].visual_bounds().unwrap().center(), backs[i].visual_bounds().unwrap().center());
                    assert!((fa - fc).hypot() < 1e-5 && (ba - bc).hypot() < 1e-5, "bài nằm giữa khuôn ở cả hai mặt");
                    // Từng đỉnh khuôn thật, không chỉ khung.
                    for (p, q) in anchors_of(front_cuts[i]).iter().zip(anchors_of(back_cuts[i])) {
                        assert!(
                            (q.x - br.x0 - (w - (p.x - fr.x0))).abs() < 1e-5 && ((q.y - br.y0) - (p.y - fr.y0)).abs() < 1e-5,
                            "đỉnh khuôn mặt sau đối xứng"
                        );
                    }
                }
                assert_eq!((fdots.len(), bdots.len()), (4, 4));
                for (p, q) in fdots.iter().zip(&bdots) {
                    let (p, q) = (p.geometric_bounds().unwrap().center(), q.geometric_bounds().unwrap().center());
                    assert!((p.x - fr.x0 + q.x - br.x0 - w).abs() < 1e-5 && ((p.y - fr.y0) - (q.y - br.y0)).abs() < 1e-5, "PON mặt sau đối xứng");
                }
            }
            let after: Vec<f64> =
                f.models.iter().flat_map(|m| [m.bai, m.sau.unwrap()]).map(|id| signed_area(&anchors_of(doc(&f.s).node(id).unwrap()))).collect();
            assert_eq!(after, src_area, "không xoay, lật, xoá hay dời nguồn");
        }
        // Thiếu mặt sau: báo lỗi, không tạo gì.
        let mut f = duplex_fixture();
        f.models[1].sau = None;
        let layers0 = doc(&f.s).layers.len();
        let e = run_render(&mut f, &o, true, &duplex_layout()).unwrap_err().to_string();
        assert!(e.contains("Thiếu mặt sau"), "{e}");
        assert_eq!((doc(&f.s).artboards.len(), doc(&f.s).layers.len()), (1, layers0));
    }

    #[test]
    fn goc_duplex_bo_ba_theo_hinh_hoc() {
        let mut f = duplex_fixture();
        // Thứ tự chọn / z-order khác, y lệch rất nhỏ: vẫn đúng bộ ba theo vị trí.
        let mut items: Vec<NodeId> = f.models.iter().flat_map(|m| [m.sau.unwrap(), m.khuon, m.bai]).collect();
        items.reverse();
        let got = triple_models(doc(&f.s), &items).unwrap();
        assert_eq!(
            got.iter().map(|m| (m.khuon, m.bai, m.sau.unwrap())).collect::<Vec<_>>(),
            f.models.iter().map(|m| (m.khuon, m.bai, m.sau.unwrap())).collect::<Vec<_>>()
        );
        assert!(triple_models(doc(&f.s), &items[1..]).unwrap_err().contains("bộ 3"));
        let moved = items[0];
        f.s.edit("dời", |d, _| {
            if let Some(n) = d.node_mut(moved) {
                n.transform(Affine::translate((0.0, 100.0)), false);
            }
            Ok(())
        })
        .unwrap();
        assert!(triple_models(doc(&f.s), &items).unwrap_err().contains("Hàng nguồn"));
    }

    #[test]
    fn goc_duplex_ba_layer() {
        let mut f = duplex_fixture();
        // Chuyển nguồn sang ba layer Khuôn bế / Mặt trước / Mặt sau, không chọn gì.
        let models = f.models.clone();
        f.s.edit("layer", |d, _| {
            for (name, pick) in [("Khuôn bế", 0usize), ("Mặt trước", 1), ("Mặt sau", 2)] {
                let layer = crate::cmd::layer::make_layer(d, Some(name));
                let lid = d.insert(None, usize::MAX, layer)?;
                for m in &models {
                    let id = [m.khuon, m.bai, m.sau.unwrap()][pick];
                    let n = d.node(id).cloned().unwrap();
                    d.remove(id)?;
                    d.insert(Some(lid), usize::MAX, n)?;
                }
            }
            Ok(())
        })
        .unwrap();
        let got = source_models(doc(&f.s), &[], true).unwrap();
        assert_eq!(got.len(), 2);
        for m in &got {
            let name = |id: NodeId| doc(&f.s).node(id).unwrap().name.clone().unwrap();
            let k = name(m.khuon);
            let i = &k[k.len() - 1..];
            assert_eq!(name(m.bai), format!("front{i}"));
            assert_eq!(name(m.sau.unwrap()), format!("back{i}"));
        }
        // Chọn chưa đủ bộ ba thì báo trước khi dàn.
        let two = [f.models[0].khuon, f.models[0].bai];
        assert!(triple_models(doc(&f.s), &two).unwrap_err().contains("bộ 3"));
    }

    // ------------------------------------------------------------ dan_be_canvas.test.js

    fn safe(half: f64) -> Rect {
        Rect::new(-half + GAP, -half + GAP, half - GAP, half - GAP)
    }

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x0 < b.x1 - 0.01 && a.x1 > b.x0 + 0.01 && a.y0 < b.y1 - 0.01 && a.y1 > b.y0 + 0.01
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-5, "{a} ≠ {b}");
    }

    #[test]
    fn goc_canvas_cap_cho_tu_goc_tren_trai() {
        let (w, h) = (330.0 * MM, 354.0 * MM);
        let sf = safe(CANVAS_HALF);
        let p = output_positions(w, h, false, 2, CANVAS_HALF, &[], None).unwrap();
        close(p[0].x0, sf.x0);
        close(p[0].y0, sf.y0);
        close(p[0].width(), w);
        close(p[0].height(), h);
        close(p[1].x0, p[0].x1 + GAP);
        close(p[1].y0, p[0].y0);
        // Artboard đang có: đặt sau nó, không đụng tới nó.
        let prior = p[0];
        let next = output_positions(w, h, false, 1, CANVAS_HALF, &[prior], None).unwrap()[0];
        close(next.x0, prior.x1 + GAP);
        close(next.y0, prior.y0);
        assert!(!overlaps(&next, &prior));
        // Vùng nguồn nằm ngoài mọi artboard cũng được né.
        let source = Rect::new(prior.x0 + 1.0, prior.y0 + 1.0, prior.x0 + 200.0, prior.y0 + 200.0);
        let protected = output_positions(w, h, false, 1, CANVAS_HALF, &[], Some(source)).unwrap()[0];
        close(protected.x0, source.x1 + GAP);
        assert!(!overlaps(&protected, &source));
        let covered = output_positions(w, h, false, 1, CANVAS_HALF, &[prior], Some(source)).unwrap()[0];
        close(covered.x0, prior.x1 + GAP);
        // Khung làm việc phủ cả canvas thì bỏ qua; dải tờ rộng thì vẫn chắn (ngoại lệ 92 % cần cả hai chiều).
        let canvas = Rect::new(-CANVAS_HALF, -CANVAS_HALF, CANVAS_HALF, CANVAS_HALF);
        let first = output_positions(w, h, false, 1, CANVAS_HALF, &[canvas], None).unwrap()[0];
        close(first.x0, sf.x0);
        close(first.y0, sf.y0);
        let strip = Rect::new(sf.x0, sf.y0, sf.x1, sf.y0 + h);
        let wrapped = output_positions(w, h, false, 1, CANVAS_HALF, &[strip], None).unwrap()[0];
        close(wrapped.x0, sf.x0);
        close(wrapped.y0, sf.y0 + h + GAP);
    }

    #[test]
    fn goc_canvas_cap_hai_mat_xuong_hang_va_qua_kho() {
        for half in [CANVAS_HALF, LARGE_CANVAS_HALF] {
            let sf = safe(half);
            let w = (sf.width() - 3.0 * GAP) / 4.0;
            let h = 354.0 * MM;
            let planned = output_positions(w, h, true, 5, half, &[], None).unwrap();
            for (i, front) in planned.iter().enumerate() {
                let back = Rect::new(front.x1 + GAP, front.y0, front.x1 + GAP + w, front.y1);
                for r in [front, &back] {
                    assert!(r.x0 >= sf.x0 - 0.01 && r.y0 >= sf.y0 - 0.01 && r.x1 <= sf.x1 + 0.01 && r.y1 <= sf.y1 + 0.01, "ra ngoài canvas: {r:?}");
                }
                close(front.x0, sf.x0 + (i % 2) as f64 * (2.0 * w + 2.0 * GAP));
                close(front.y0, sf.y0 + (i / 2) as f64 * (h + GAP));
                for prior in &planned[..i] {
                    let bundle = Rect::new(prior.x0, prior.y0, prior.x1 + GAP + w, prior.y1);
                    assert!(!overlaps(front, &bundle) && !overlaps(&back, &bundle), "giữ trọn cặp trước / sau khi xuống hàng");
                }
            }
        }
        let sf = safe(CANVAS_HALF);
        for (w, h, two, count) in
            [(sf.width() + 1.0, 354.0 * MM, false, 1), (sf.width() / 2.0, 354.0 * MM, true, 1), (330.0 * MM, sf.height() + 1.0, false, 1)]
        {
            let e = output_positions(w, h, two, count, CANVAS_HALF, &[], None).unwrap_err();
            assert!(e.contains("canvas"), "{e}");
        }
        // Tính trước cả lô: tờ thứ hai không còn chỗ thì báo, chưa tạo gì.
        let e = output_positions(sf.width(), sf.height(), false, 2, CANVAS_HALF, &[], None).unwrap_err();
        assert!(e.contains("canvas"), "{e}");
        // VectorCraft: lô đó dùng vùng canvas rộng gấp 10 (như tài liệu Large Canvas).
        assert_eq!(plan_positions(sf.width(), sf.height(), false, 2, &[], None).unwrap().len(), 2);
    }

    // ------------------------------------------------------------ dan_be_panel.test.js (phần lõi)

    #[test]
    fn goc_panel_khuon_trung_dung_lai_phuong_an() {
        // Mặc định mỗi mẫu một tờ: hai mẫu cùng đường bao dùng lại một phương án, đánh lại số mẫu.
        let mk = |g: &Groups| super::super::nester::TypeIn { groups: g.clone(), curve_error: 0.0 };
        let (a, b) = (rect(10.0, 10.0), rect(12.0, 10.0));
        let o = opts(&json!({"gap": "1.25", "margin": "5.5", "ponClear": "8.75"})).unwrap();
        let mut input = nest_input(&o, &[]);
        input.types = vec![mk(&a), mk(&a), mk(&b)];
        let sheets = plan_sheets(&input, true).unwrap();
        assert_eq!(sheets.len(), 3);
        for (i, sh) in sheets.iter().enumerate() {
            assert_eq!(sh.model, Some(i));
            assert!(sh.out.slots.iter().all(|s| s.mi == i), "đưa mẫu cục bộ 0 về đúng mẫu gốc");
        }
        let pos = |k: usize| sheets[k].out.slots.iter().map(|s| (s.vi, s.x.to_bits(), s.y.to_bits())).collect::<Vec<_>>();
        assert_eq!(pos(0), pos(1), "đường bao trùng khớp dùng lại cùng phương án");
        // Tick "Dàn nhiều mẫu vào một tờ": một lần dàn chung cho mọi mẫu.
        let mixed = plan_sheets(&input, false).unwrap();
        assert_eq!(mixed.len(), 1);
        assert_eq!(mixed[0].model, None);
        assert!(mixed[0].out.counts.iter().all(|&c| c > 0));
        // Một mẫu sau không dàn được thì không có tờ nào của các mẫu trước.
        input.types = vec![mk(&a), mk(&rect(400.0, 400.0))];
        assert!(plan_sheets(&input, true).is_err());
    }
}
