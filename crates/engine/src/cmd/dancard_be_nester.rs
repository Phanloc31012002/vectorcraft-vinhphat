//! DanCard — lõi xếp khuôn bế theo silhouette thật (bản Rust của `dan_be_nester.js` v9).
//!
//! Mỗi khuôn được raster thành các đoạn ngang trên từng hàng ô (0,25 mm): một hàng giữ nhiều đoạn,
//! nên khuôn có eo / lõm vẫn để chỗ cho khuôn xoay khác lồng vào. Đoạn được nở thêm nửa khe; hai
//! khuôn đã nở không chồng nhau thì cách nhau ít nhất một khe. Phép tìm thử các mô-típ (dải, hàng
//! xen kẽ, băng, lưới so le, ghép hàng), rồi mở rộng tham lam theo tiếp xúc thật của đường bao, gỡ
//! và lấp lại, canh tâm. Cuối cùng kiểm tra trên đường bao liên tục (khe, lề, né PON) và khép khe
//! thừa của lưới bằng cách co vị trí (không bao giờ co hình).
//!
//! Toạ độ như bản gốc: milimet, y hướng lên, gốc góc dưới trái; vị trí trong phép tìm tính bằng ô
//! của vùng dàn (tờ trừ lề).
//!
//! Khác bản gốc:
//! - đồng hồ 3 giây được thay bằng ngân sách phép tính ([`Clock`]): kết quả tất định, không phụ
//!   thuộc tốc độ máy (thời gian thật chỉ là chốt an toàn khi máy chậm hơn hẳn), không dùng giờ hệ
//!   thống (wasm);
//! - kiểm tra liên tục né PON theo sai số đường cong của chính khuôn đó, cùng quy tắc với lúc canh
//!   tâm (bản gốc lấy sai số lớn nhất của mọi khuôn nên tờ trộn khuôn thẳng + khuôn cong có thể bị
//!   từ chối oan);
//! - số con mỗi tờ tối đa [`MAX_PIECES`] (chốt an toàn, xa hơn mọi mẫu thử của bản gốc), vùng dàn
//!   tối đa [`MAX_CELLS`] ô mỗi chiều.

use std::cell::{Cell, OnceCell, RefCell};
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(super) type Pt = [f64; 2];
pub(super) type Contour = Vec<Pt>;
/// Một nhóm even-odd (một compound path, gồm cả lỗ).
pub(super) type Group = Vec<Contour>;
/// Các nhóm của một khuôn, hợp lại.
pub(super) type Groups = Vec<Group>;
pub(super) type Span = (i64, i64);
type Row = Vec<Span>;
type Plan = Vec<Slot>;
type PPlan = Vec<PSlot>;
type Offsets = Rc<Vec<(i64, i64)>>;

/// Số phép thử coi như 1 ms của bản gốc (đo trên bản release: một hàng ô, một vị trí ứng
/// viên, một cặp cạnh… đã cân theo chi phí thật; các tờ thử lệch thời gian thật khoảng 0,7–1,7
/// lần). Ngân sách 3000 ms ≈ 300 triệu phép thử.
const WORK_PER_MS: f64 = 100_000.0;
/// Thời gian thật được phép gấp bao nhiêu lần ngân sách trước khi chốt an toàn dừng phép tìm.
#[cfg(all(not(test), not(target_arch = "wasm32")))]
const REAL_SLACK: f64 = 2.0;
/// Số con tối đa trên một tờ: chỉ là chốt an toàn bộ nhớ / thời gian. Bản gốc không giới hạn và
/// bộ test của nó đòi hàng chục nghìn khuôn tí hon (13 440 hình tròn 2 mm), nên chốt đặt xa hơn.
const MAX_PIECES: usize = 50_000;
/// Số ô tối đa mỗi chiều của vùng dàn (0,25 mm: 5 m).
pub(super) const MAX_CELLS: i64 = 20_000;
/// Giới hạn số vị trí thử của một mô-típ lặp (như bản gốc).
const VISITS: usize = 20_000;
/// Góc xoay của bốn biến thể (ngược chiều kim đồng hồ).
pub(super) const ANGLES: [i64; 4] = [0, 90, 180, 270];

// ---------------------------------------------------------------- ngân sách

/// Đồng hồ của ngân sách: đếm phép thử (tất định: mọi máy cho đúng kết quả đã kiểm bằng bộ test
/// gốc của DanCard). Bản desktop chỉ dùng thời gian thật làm chốt an toàn khi máy chậm hơn hẳn
/// ([`REAL_SLACK`] lần ngân sách), không để nó cắt phép tìm sớm trên máy bình thường. Bản web
/// (không có đồng hồ thật) và kiểm thử chỉ đếm.
#[derive(Default)]
pub(super) struct Clock {
    work: Cell<u64>,
    #[cfg(all(not(test), not(target_arch = "wasm32")))]
    started: OnceCell<std::time::Instant>,
}

impl Clock {
    /// "Mili giây" đã dùng.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(super) fn now(&self) -> f64 {
        self.work.get() as f64 / WORK_PER_MS
    }
    /// "Mili giây" đã dùng: theo phép đếm; thời gian thật (chia [`REAL_SLACK`]) chỉ thắng khi máy
    /// chậm hơn hẳn.
    #[cfg(all(not(test), not(target_arch = "wasm32")))]
    pub(super) fn now(&self) -> f64 {
        let real = self.started.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0;
        (self.work.get() as f64 / WORK_PER_MS).max(real / REAL_SLACK)
    }
    fn tick(&self, n: usize) {
        self.work.set(self.work.get().saturating_add(n as u64));
    }
    /// Số phép thử đã đếm.
    pub(super) fn work(&self) -> u64 {
        self.work.get()
    }
}

// ---------------------------------------------------------------- tiện ích số

/// `Math.round` của JavaScript (nửa làm tròn lên, kể cả số âm).
fn jround(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// `Math.round(a / 2)`.
fn half(a: i64) -> i64 {
    jround(a as f64 / 2.0) as i64
}

fn cmp_f(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn sq(v: f64) -> f64 {
    v * v
}

// ---------------------------------------------------------------- bản ghi

/// Chấm PON trong ô của vùng dàn; `radius` = bán kính chấm + né PON.
#[derive(Clone, Copy, Debug)]
pub(super) struct Dot {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) radius: f64,
}

/// Một con đặt trong phép tìm: biến thể `v` (chỉ số vào danh sách đang dùng), góc dưới trái khung
/// (ô, vùng dàn).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Slot {
    pub(super) mi: usize,
    pub(super) v: usize,
    pub(super) x: i64,
    pub(super) y: i64,
}

/// Một con sau bước vật lý: vị trí lẻ ô.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PSlot {
    pub(super) mi: usize,
    pub(super) v: usize,
    pub(super) x: f64,
    pub(super) y: f64,
}

/// Vùng dàn (ô) và các chấm PON.
pub(super) struct Sheet {
    pub(super) rw: i64,
    pub(super) rh: i64,
    pub(super) dots: Vec<Dot>,
}

fn to_physical(plan: &[Slot]) -> PPlan {
    plan.iter().map(|s| PSlot { mi: s.mi, v: s.v, x: s.x as f64, y: s.y as f64 }).collect()
}

// ---------------------------------------------------------------- hình học

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub(super) x0: f64,
    pub(super) y0: f64,
    pub(super) x1: f64,
    pub(super) y1: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

pub(super) fn bounds_of_groups(groups: &[Group]) -> Option<Bounds> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in groups.iter().flatten().flatten() {
        if p[0] < x0 {
            x0 = p[0];
        }
        if p[0] > x1 {
            x1 = p[0];
        }
        if p[1] < y0 {
            y0 = p[1];
        }
        if p[1] > y1 {
            y1 = p[1];
        }
    }
    let ok = x1 > x0 && y1 > y0;
    ok.then_some(Bounds { x0, y0, x1, y1, w: x1 - x0, h: y1 - y0 })
}

/// Bản sao bỏ đường bao dưới 3 điểm và nhóm rỗng.
fn copy_groups(groups: &[Group]) -> Groups {
    groups.iter().map(|g| g.iter().filter(|c| c.len() >= 3).cloned().collect::<Group>()).filter(|g| !g.is_empty()).collect()
}

/// Dời về gốc khung (0, 0).
pub(super) fn normalize_groups(groups: &[Group]) -> Option<Groups> {
    let b = bounds_of_groups(groups)?;
    let mut out = copy_groups(groups);
    for p in out.iter_mut().flatten().flatten() {
        p[0] -= b.x0;
        p[1] -= b.y0;
    }
    Some(out)
}

/// Cùng một đường bao: bất kể điểm bắt đầu và chiều đi (sai lệch 1e-6 mm).
fn same_contour(a: &[Pt], b: &[Pt]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let Some(a0) = a.first() else { return true };
    let n = b.len() as i64;
    let near = |p: &Pt, q: &Pt| (p[0] - q[0]).abs() <= 0.000001 && (p[1] - q[1]).abs() <= 0.000001;
    for (start, bs) in b.iter().enumerate() {
        if !near(a0, bs) {
            continue;
        }
        for dir in [-1i64, 1] {
            let equal = a.iter().enumerate().skip(1).all(|(i, ap)| {
                let j = (start as i64 + dir * i as i64 + n).checked_rem_euclid(n).unwrap_or(0);
                b.get(j as usize).is_some_and(|bp| near(ap, bp))
            });
            if equal {
                return true;
            }
        }
    }
    false
}

fn same_group(a: &[Contour], b: &[Contour]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used = vec![false; b.len()];
    for ca in a {
        let mut found = false;
        for (j, cb) in b.iter().enumerate() {
            if used.get(j).copied().unwrap_or(true) || !same_contour(ca, cb) {
                continue;
            }
            if let Some(u) = used.get_mut(j) {
                *u = true;
            }
            found = true;
            break;
        }
        if !found {
            return false;
        }
    }
    true
}

/// Cùng một khuôn: bất kể thứ tự nhóm / đường bao.
pub(super) fn same_groups(a: &[Group], b: &[Group]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut used = vec![false; b.len()];
    for ga in a {
        let mut found = false;
        for (j, gb) in b.iter().enumerate() {
            if used.get(j).copied().unwrap_or(true) || !same_group(ga, gb) {
                continue;
            }
            if let Some(u) = used.get_mut(j) {
                *u = true;
            }
            found = true;
            break;
        }
        if !found {
            return false;
        }
    }
    true
}

/// Xoay 0/90/180/270° (ngược chiều kim đồng hồ, y hướng lên) rồi dời về gốc.
pub(super) fn rotate_groups(groups: &[Group], deg: i64) -> Option<Groups> {
    let turn = |p: &Pt| -> Pt {
        match deg {
            90 => [-p[1], p[0]],
            180 => [-p[0], -p[1]],
            270 => [p[1], -p[0]],
            _ => *p,
        }
    };
    let out: Groups = groups.iter().map(|g| g.iter().map(|c| c.iter().map(turn).collect()).collect()).collect();
    normalize_groups(&out)
}

// ---------------------------------------------------------------- đoạn trên hàng

/// Sắp xếp rồi gộp các đoạn chồng hoặc chạm nhau.
fn merge_spans(mut spans: Vec<Span>) -> Row {
    spans.sort_unstable();
    let mut out: Row = Vec::with_capacity(spans.len());
    for s in spans {
        match out.last_mut() {
            Some(cur) if s.0 <= cur.1 => {
                if s.1 > cur.1 {
                    cur.1 = s.1;
                }
            }
            _ => out.push(s),
        }
    }
    out
}

fn merge_f(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| cmp_f(a.0, b.0).then(cmp_f(a.1, b.1)));
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(spans.len());
    for s in spans {
        match out.last_mut() {
            Some(cur) if s.0 <= cur.1 => {
                if s.1 > cur.1 {
                    cur.1 = s.1;
                }
            }
            _ => out.push(s),
        }
    }
    out
}

/// `a` ∪ (`b` dời `dx`), cả hai đã gộp: một lượt, cùng kết quả với `merge_spans` của cả hai.
fn merge_rows(a: &[Span], b: &[Span], dx: i64) -> Row {
    let mut out: Row = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    loop {
        let next = match (a.get(i), b.get(j)) {
            (Some(&p), Some(&(b0, b1))) => {
                let q = (b0 + dx, b1 + dx);
                if p <= q {
                    i += 1;
                    p
                } else {
                    j += 1;
                    q
                }
            }
            (Some(&p), None) => {
                i += 1;
                p
            }
            (None, Some(&(b0, b1))) => {
                j += 1;
                (b0 + dx, b1 + dx)
            }
            (None, None) => break,
        };
        match out.last_mut() {
            Some(cur) if next.0 <= cur.1 => {
                if next.1 > cur.1 {
                    cur.1 = next.1;
                }
            }
            _ => out.push(next),
        }
    }
    out
}

/// Hai hàng (`b` dời `dx`) chồng nhau; chạm mép không tính.
fn row_hits(a: &[Span], b: &[Span], dx: i64) -> bool {
    let (mut i, mut j) = (0, 0);
    while let (Some(&(a0, a1)), Some(&(b0, b1))) = (a.get(i), b.get(j)) {
        let (b0, b1) = (b0 + dx, b1 + dx);
        if a0 < b1 && a1 > b0 {
            return true;
        }
        if a1 <= b1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    false
}

fn to_cell_spans(spans: Vec<(f64, f64)>, step: f64) -> Row {
    let mut out = Vec::with_capacity(spans.len());
    for (x0, x1) in spans {
        // Bỏ sai số vụn đổi point → mm (50.000000002 mm không thêm một ô).
        let a = (x0 / step + 1e-7).floor() as i64;
        let b = (x1 / step - 1e-7).ceil() as i64;
        if b > a {
            out.push((a, b));
        }
    }
    merge_spans(out)
}

/// Các hàng ô `0..rows` (tâm hàng y = (r + 0.5)·step) cắt khuôn: luật even-odd trong từng nhóm, hợp
/// các nhóm. Mỗi cạnh chỉ thăm các hàng nó đi qua.
fn scan_rows(groups: &[Group], rows: i64, step: f64, ck: &Clock) -> Vec<Row> {
    let n = usize::try_from(rows).unwrap_or(0);
    let mut spans: Vec<Vec<(f64, f64)>> = vec![Vec::new(); n];
    let mut xs: Vec<Vec<f64>> = vec![Vec::new(); n];
    let mut work = 0usize;
    for group in groups {
        let mut touched: Vec<usize> = Vec::new();
        for c in group {
            let len = c.len();
            for (i, p) in c.iter().enumerate() {
                let Some(q) = c.get((i + 1) % len) else { continue };
                work += 1;
                let (lo, hi) = if p[1] < q[1] { (p[1], q[1]) } else { (q[1], p[1]) };
                if hi <= lo {
                    continue;
                }
                let first = ((lo / step - 0.5).ceil() as i64 - 1).max(0);
                let last = ((hi / step - 0.5).ceil() as i64 + 1).min(rows);
                for r in first..last {
                    let y = (r as f64 + 0.5) * step;
                    // Nửa mở: một đỉnh không được đếm hai lần
                    // ((p.y ≤ y < q.y) hoặc (q.y ≤ y < p.y)).
                    let crosses = if p[1] <= y { q[1] > y } else { q[1] <= y };
                    if crosses {
                        let x = p[0] + (y - p[1]) * (q[0] - p[0]) / (q[1] - p[1]);
                        if let Some(row) = xs.get_mut(r as usize) {
                            if row.is_empty() {
                                touched.push(r as usize);
                            }
                            row.push(x);
                        }
                    }
                }
            }
        }
        for r in touched {
            let (Some(row), Some(out)) = (xs.get_mut(r), spans.get_mut(r)) else { continue };
            row.sort_by(|a, b| cmp_f(*a, *b));
            for pair in row.chunks_exact(2) {
                if let [a, b] = pair
                    && *b > *a + 1e-7
                {
                    out.push((*a, *b));
                }
            }
            work += row.len();
            row.clear();
        }
    }
    ck.tick(work);
    spans.into_iter().map(|row| to_cell_spans(merge_f(row), step)).collect()
}

// ---------------------------------------------------------------- biến thể xoay

/// Một hướng xoay của một khuôn, đã raster.
pub(super) struct Variant {
    pub(super) mi: usize,
    pub(super) vi: usize,
    pub(super) angle: i64,
    /// Đường bao đã xoay, mm, khung bắt đầu ở (0, 0).
    pub(super) groups: Groups,
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) w_cells: i64,
    pub(super) h_cells: i64,
    /// Các đoạn ô của khuôn trên hàng r.
    pub(super) raw_rows: Vec<Row>,
    /// Các đoạn đã nở nửa khe; `clear_rows[r + pad]` ứng với hàng r.
    pub(super) clear_rows: Vec<Row>,
    pub(super) pad: i64,
    pub(super) step: f64,
    pub(super) curve_error: f64,
    id: usize,
    features: OnceCell<Vec<i64>>,
    contacts: RefCell<HashMap<usize, Offsets>>,
}

/// Raster khuôn đã xoay `angle`; nở mỗi hàng theo đĩa bán kính khe/2 + nửa ô.
pub(super) fn build_variant(groups: &[Group], mi: usize, vi: usize, angle: i64, gap: f64, step: f64, ck: &Clock) -> Option<Variant> {
    let rotated = rotate_groups(groups, angle)?;
    let b = bounds_of_groups(&rotated)?;
    let (wc, hc) = ((b.w / step - 1e-7).ceil(), (b.h / step - 1e-7).ceil());
    let cap = (4 * MAX_CELLS) as f64;
    let fits = wc.is_finite() && hc.is_finite() && wc <= cap && hc <= cap;
    if !fits {
        return None;
    }
    let (w_cells, h_cells) = (wc as i64, hc as i64);
    let raw_rows = scan_rows(&rotated, h_cells, step, ck);
    let radius = gap / 2.0 + step / 2.0;
    let pad = (radius / step).ceil() as i64;
    let mut clear_rows = Vec::with_capacity(usize::try_from(h_cells + 2 * pad).unwrap_or(0));
    for cr in -pad..h_cells + pad {
        let mut expanded = Vec::new();
        for rr in (cr - pad).max(0)..h_cells.min(cr + pad + 1) {
            let delta = (cr - rr).abs() as f64 * step;
            if delta > radius + 1e-7 {
                continue;
            }
            let ext = ((radius * radius - delta * delta).max(0.0).sqrt() / step).ceil() as i64;
            if let Some(raw) = raw_rows.get(rr as usize) {
                expanded.extend(raw.iter().map(|&(a, b)| (a - ext, b + ext)));
            }
        }
        clear_rows.push(merge_spans(expanded));
    }
    ck.tick(clear_rows.len());
    Some(Variant {
        mi,
        vi,
        angle,
        groups: rotated,
        w: b.w,
        h: b.h,
        w_cells,
        h_cells,
        raw_rows,
        clear_rows,
        pad,
        step,
        curve_error: 0.0,
        id: mi * 4 + vi,
        features: OnceCell::new(),
        contacts: RefCell::new(HashMap::new()),
    })
}

type Sig = Option<(Span, Option<(Span, usize)>)>;

fn sig(spans: &[Span]) -> Sig {
    let first = *spans.first()?;
    let tail = if spans.len() > 1 { spans.last().map(|l| (*l, spans.len())) } else { None };
    Some((first, tail))
}

fn mark(flags: &mut [bool], r: i64) {
    if let Ok(i) = usize::try_from(r)
        && let Some(f) = flags.get_mut(i)
    {
        *f = true;
    }
}

/// Các hàng đặc trưng (mốc đều, chỗ đổi số đoạn, chỗ mép nhảy) dùng để canh tiếp xúc.
pub(super) fn feature_rows(v: &Variant) -> &[i64] {
    v.features.get_or_init(|| features_of(v))
}

fn features_of(v: &Variant) -> Vec<i64> {
    let n = v.h_cells;
    let Ok(nu) = usize::try_from(n) else { return Vec::new() };
    if nu == 0 {
        return Vec::new();
    }
    let mut keep = vec![false; nu];
    let mut strong = vec![false; nu];
    mark(&mut keep, 0);
    mark(&mut keep, n - 1);
    for k in 0..=14i64 {
        mark(&mut keep, jround(((n - 1) * k) as f64 / 14.0) as i64);
    }
    let period = (jround(n as f64 / 18.0) as i64).max(1);
    for r in 1..n {
        let (Some(prev), Some(cur)) = (v.raw_rows.get((r - 1) as usize), v.raw_rows.get(r as usize)) else { continue };
        if sig(prev) != sig(cur) && r % period == 0 {
            mark(&mut keep, r);
        }
        if cur.len() != prev.len() {
            mark(&mut keep, r);
            mark(&mut strong, r);
        }
        if let (Some(a0), Some(a1), Some(z0), Some(z1)) = (prev.first(), cur.first(), prev.last(), cur.last())
            && ((a0.0 - a1.0).abs() > 5 || (a0.1 - a1.1).abs() > 5 || (z0.0 - z1.0).abs() > 5 || (z0.1 - z1.1).abs() > 5)
        {
            mark(&mut keep, r);
            mark(&mut strong, r);
        }
    }
    let list: Vec<i64> = (0..n).filter(|r| keep.get(*r as usize).copied().unwrap_or(false)).collect();
    if list.len() <= 24 {
        return list;
    }
    let (Some(&head), Some(&tail)) = (list.first(), list.last()) else { return list };
    let mut compact = vec![head];
    let mut used: HashSet<i64> = HashSet::from([head, tail]);
    for &r in &list {
        if strong.get(r as usize).copied().unwrap_or(false) && used.insert(r) {
            compact.push(r);
        }
    }
    for ix in 1..22usize {
        let at = jround(((list.len() - 1) * ix) as f64 / 22.0) as usize;
        if let Some(&pick) = list.get(at)
            && used.insert(pick)
        {
            compact.push(pick);
        }
    }
    // Như bản gốc: hàng cuối đã được đánh dấu từ đầu nên không bao giờ được thêm lại.
    compact.sort_unstable();
    compact
}

// ---------------------------------------------------------------- chiếm chỗ, PON

struct Occ {
    rows: Vec<Row>,
    pad: i64,
    height: i64,
}

fn make_occ(height: i64, pad: i64) -> Occ {
    let n = usize::try_from(height + 2 * pad + 3).unwrap_or(0);
    Occ { rows: vec![Vec::new(); n], pad, height }
}

fn occ_can_place(occ: &Occ, v: &Variant, x: i64, y: i64, ck: &Clock) -> bool {
    let mut work = 1usize;
    let mut ok = true;
    for (ri, clear) in v.clear_rows.iter().enumerate() {
        if clear.is_empty() {
            continue;
        }
        let gy = y + ri as i64 - v.pad;
        if !(-occ.pad..occ.height + occ.pad).contains(&gy) {
            continue;
        }
        let Some(row) = occ.rows.get((gy + occ.pad) as usize) else { continue };
        // Một hàng tốn theo số khoảng đã có (mọi con cùng hàng), không phải 1.
        work += 1 + (row.len() + clear.len()) / 4;
        if row_hits(row, clear, x) {
            ok = false;
            break;
        }
    }
    ck.tick(work);
    ok
}

fn occ_add(occ: &mut Occ, v: &Variant, x: i64, y: i64, ck: &Clock) {
    let (pad, height) = (occ.pad, occ.height);
    let mut work = v.clear_rows.len();
    for (ri, clear) in v.clear_rows.iter().enumerate() {
        if clear.is_empty() {
            continue;
        }
        let gy = y + ri as i64 - v.pad;
        if !(-pad..height + pad).contains(&gy) {
            continue;
        }
        if let Some(row) = occ.rows.get_mut((gy + pad) as usize) {
            work += (row.len() + clear.len()) / 2;
            *row = merge_rows(row, clear, x);
        }
    }
    ck.tick(work);
}

/// Khuôn (chưa nở) chạm đĩa né PON.
fn raw_hits_pon(v: &Variant, x: i64, y: i64, dots: &[Dot], ck: &Clock) -> bool {
    let (xf, yf) = (x as f64, y as f64);
    let mut work = 1usize;
    let mut hit = false;
    'dots: for dot in dots {
        // Chỉ các hàng có |dy| ≤ bán kính (dy = y + r + 0.5 − tâm chấm).
        let lo = ((dot.y - dot.radius - yf - 0.5).ceil() as i64 - 1).max(0);
        let hi = ((dot.y + dot.radius - yf - 0.5).floor() as i64 + 2).min(v.h_cells);
        for r in lo..hi {
            let Some(spans) = v.raw_rows.get(r as usize) else { continue };
            work += 1;
            if spans.is_empty() {
                continue;
            }
            let dy = yf + r as f64 + 0.5 - dot.y;
            if dy.abs() > dot.radius {
                continue;
            }
            let reach = (dot.radius * dot.radius - dy * dy).max(0.0).sqrt();
            for &(a, b) in spans {
                if dot.x > xf + a as f64 - reach && dot.x < xf + b as f64 + reach {
                    hit = true;
                    break 'dots;
                }
            }
        }
    }
    ck.tick(work);
    hit
}

fn inside_and_clear(v: &Variant, x: i64, y: i64, sh: &Sheet, occ: &Occ, ck: &Clock) -> bool {
    if x < 0 || y < 0 || x + v.w_cells > sh.rw || y + v.h_cells > sh.rh {
        return false;
    }
    if raw_hits_pon(v, x, y, &sh.dots, ck) {
        return false;
    }
    occ_can_place(occ, v, x, y, ck)
}

fn build_occ(plan: &[Slot], vs: &[Variant], rh: i64, ck: &Clock) -> Occ {
    let pad = vs.iter().map(|v| v.pad).max().unwrap_or(0).max(0);
    let mut occ = make_occ(rh, pad + 2);
    for s in plan {
        if let Some(v) = vs.get(s.v) {
            occ_add(&mut occ, v, s.x, s.y, ck);
        }
    }
    occ
}

pub(super) fn verify_plan(plan: &[Slot], vs: &[Variant], sh: &Sheet, ck: &Clock) -> bool {
    let mut occ = build_occ(&[], vs, sh.rh, ck);
    for s in plan {
        let Some(v) = vs.get(s.v) else { return false };
        if !inside_and_clear(v, s.x, s.y, sh, &occ, ck) {
            return false;
        }
        occ_add(&mut occ, v, s.x, s.y, ck);
    }
    true
}

// ---------------------------------------------------------------- chấm điểm

#[derive(Clone, Copy, Default, Debug)]
struct Pb {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Pb {
    fn union(self, o: Pb) -> Pb {
        Pb { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }
    /// Càng nhỏ càng gọn: diện tích khung × 10000 + 8 × cao + rộng.
    fn score(self) -> i64 {
        let (w, h) = (self.x1 - self.x0, self.y1 - self.y0);
        w.saturating_mul(h).saturating_mul(10_000).saturating_add(h * 8 + w)
    }
}

fn slot_box(s: &Slot, vs: &[Variant]) -> Pb {
    let (w, h) = vs.get(s.v).map_or((0, 0), |v| (v.w_cells, v.h_cells));
    Pb { x0: s.x, y0: s.y, x1: s.x + w, y1: s.y + h }
}

fn plan_bounds(plan: &[Slot], vs: &[Variant]) -> Pb {
    plan.iter().map(|s| slot_box(s, vs)).reduce(Pb::union).unwrap_or_default()
}

fn plan_score(plan: &[Slot], vs: &[Variant]) -> i64 {
    plan_bounds(plan, vs).score()
}

/// +1: `a` tốt hơn `b` (nhiều con hơn, rồi gọn hơn); `b` không có thì `a` tốt hơn.
fn compare_plans(a: &[Slot], b: Option<&[Slot]>, vs: &[Variant]) -> i32 {
    let Some(b) = b else { return 1 };
    if a.len() != b.len() {
        return if a.len() > b.len() { 1 } else { -1 };
    }
    match plan_score(a, vs).cmp(&plan_score(b, vs)) {
        Ordering::Less => 1,
        Ordering::Greater => -1,
        Ordering::Equal => 0,
    }
}

/// Thứ tự "tốt nhất trước".
fn best_first(a: &[Slot], b: &[Slot], vs: &[Variant]) -> Ordering {
    match compare_plans(a, Some(b), vs) {
        1 => Ordering::Less,
        -1 => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

fn count_types(plan: &[Slot], n: usize) -> Vec<usize> {
    let mut out = vec![0; n];
    for s in plan {
        if let Some(c) = out.get_mut(s.mi) {
            *c += 1;
        }
    }
    out
}

/// Nhiều mẫu trên một tờ: số con mỗi mẫu lệch nhau tối đa 1.
pub(super) fn balanced(plan: &[Slot], n: usize) -> bool {
    if n <= 1 {
        return true;
    }
    let c = count_types(plan, n);
    let (lo, hi) = (c.iter().min().copied().unwrap_or(0), c.iter().max().copied().unwrap_or(0));
    hi - lo <= 1
}

fn allowed(mi: usize, counts: &[usize], n: usize) -> bool {
    if n <= 1 {
        return true;
    }
    let min = counts.iter().copied().min().unwrap_or(0);
    counts.get(mi).is_some_and(|c| *c <= min)
}

// ---------------------------------------------------------------- vị trí ứng viên

struct Cands {
    list: Vec<(i64, i64)>,
    seen: HashSet<(i64, i64)>,
    w: i64,
    h: i64,
    rw: i64,
    rh: i64,
    work: usize,
}

impl Cands {
    fn push(&mut self, x: i64, y: i64) {
        // Một vị trí: kiểm tra biên + tra bảng băm, khoảng bốn phép thử.
        self.work += 4;
        if x < 0 || y < 0 || x + self.w > self.rw || y + self.h > self.rh {
            return;
        }
        if self.seen.insert((x, y)) {
            self.list.push((x, y));
        }
    }
}

/// Biên no-fit dạng raster của `b` quanh `a` (cả hai đã nở): ở mỗi độ lệch hàng, hai đầu mỗi
/// khoảng cấm là vị trí tiếp xúc, kể cả trong hốc lõm. Lưu trong `b` theo `a`.
pub(super) fn contact_offsets(a: &Variant, b: &Variant, ck: &Clock) -> Offsets {
    if let Ok(map) = b.contacts.try_borrow()
        && let Some(c) = map.get(&a.id)
    {
        return Rc::clone(c);
    }
    let (na, nb) = (a.clear_rows.len() as i64, b.clear_rows.len() as i64);
    let first = -a.pad - b.h_cells - b.pad;
    let last = a.h_cells + a.pad + b.pad;
    let mut out = Vec::new();
    let mut blocks: Vec<Span> = Vec::new();
    let mut work = 0usize;
    for dy in first..=last {
        let mut ib = (b.pad - a.pad - dy).max(0);
        while ib < nb {
            let ia = ib + dy + a.pad - b.pad;
            if ia >= na {
                break;
            }
            if let (Some(ra), Some(rb)) = (a.clear_rows.get(ia as usize), b.clear_rows.get(ib as usize)) {
                for &(a0, a1) in ra {
                    for &(b0, b1) in rb {
                        blocks.push((a0 - b1, a1 - b0));
                    }
                }
            }
            ib += 1;
        }
        work += blocks.len() + 1;
        for (s, e) in merge_spans(std::mem::take(&mut blocks)) {
            out.push((s, dy));
            out.push((e, dy));
        }
    }
    ck.tick(work);
    let out = Rc::new(out);
    if let Ok(mut map) = b.contacts.try_borrow_mut() {
        map.insert(a.id, Rc::clone(&out));
    }
    out
}

/// Các vị trí đáng thử cho `v`: góc / giữa cạnh tờ, tiếp xúc no-fit, tiếp xúc khung và tiếp xúc
/// theo hàng đặc trưng với từng con đã đặt.
pub(super) fn candidates_for(plan: &[Slot], v: &Variant, vs: &[Variant], sh: &Sheet, ck: &Clock) -> Vec<(i64, i64)> {
    let (rw, rh, w, h) = (sh.rw, sh.rh, v.w_cells, v.h_cells);
    let mut c = Cands { list: Vec::new(), seen: HashSet::new(), w, h, rw, rh, work: 0 };
    c.push(0, 0);
    c.push(rw - w, 0);
    c.push(0, rh - h);
    c.push(rw - w, rh - h);
    c.push(half(rw - w), 0);
    c.push(half(rw - w), rh - h);
    c.push(0, half(rh - h));
    c.push(rw - w, half(rh - h));
    let fb = feature_rows(v);
    for s in plan {
        let Some(a) = vs.get(s.v) else { continue };
        let fa = feature_rows(a);
        let contacts = contact_offsets(a, v, ck);
        for &(dx, dy) in contacts.iter() {
            c.push(s.x + dx, s.y + dy);
        }
        c.push(s.x - w, s.y);
        c.push(s.x + a.w_cells, s.y);
        c.push(s.x, s.y - h);
        c.push(s.x, s.y + a.h_cells);
        c.push(s.x + half(a.w_cells - w), s.y);
        c.push(s.x, s.y + half(a.h_cells - h));
        c.push(s.x + a.w_cells - w, s.y + a.h_cells - h);
        for &ia in fa {
            let Some(row_a) = a.clear_rows.get((ia + a.pad) as usize).filter(|r| !r.is_empty()) else { continue };
            for &ib in fb {
                let Some(row_b) = v.clear_rows.get((ib + v.pad) as usize).filter(|r| !r.is_empty()) else { continue };
                let yy = s.y + ia - ib;
                for &(a0, a1) in row_a {
                    for &(b0, b1) in row_b {
                        c.push(s.x + a1 - b0, yy);
                        c.push(s.x + a0 - b1, yy);
                    }
                }
            }
        }
    }
    ck.tick(c.work);
    c.list
}

/// Phép thử của một lần sắp xếp `n` vị trí (n log n, khoảng bốn so sánh một phép thử).
fn sort_work(n: usize) -> usize {
    n.saturating_mul(usize::BITS.saturating_sub(n.leading_zeros()) as usize) / 4
}

/// Tối đa `cap` vị trí: giữ hai đầu danh sách (theo y, x), lấy đều phần giữa.
fn sample_candidates(mut cs: Vec<(i64, i64)>, cap: usize) -> Vec<(i64, i64)> {
    cs.sort_by_key(|&(x, y)| (y, x));
    if cs.len() <= cap {
        return cs;
    }
    let len = cs.len();
    let edge = ((cap as f64 * 0.36).floor() as usize).min(len / 2);
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(cap);
    let mut used: HashSet<(i64, i64)> = HashSet::new();
    for &c in cs.iter().take(edge) {
        if used.insert(c) {
            out.push(c);
        }
    }
    for &c in cs.iter().skip(edge.max(len - edge)) {
        if used.insert(c) {
            out.push(c);
        }
    }
    let stride = ((len - 2 * edge) / cap.saturating_sub(out.len()).max(1)).max(1);
    let mut i = edge;
    while i < len - edge && out.len() < cap {
        if let Some(&c) = cs.get(i)
            && used.insert(c)
        {
            out.push(c);
        }
        i += stride;
    }
    out
}

// ---------------------------------------------------------------- mở rộng tham lam

fn grow_plan(seed: &[Slot], vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Plan {
    let mut plan = seed.to_vec();
    let mut occ = build_occ(&plan, vs, sh.rh, ck);
    let mut guard = 0;
    loop {
        if guard >= 220 || plan.len() >= MAX_PIECES {
            break;
        }
        guard += 1;
        if ck.now() >= deadline {
            break;
        }
        let counts = count_types(&plan, tc);
        let base = (!plan.is_empty()).then(|| plan_bounds(&plan, vs));
        let mut best: Option<(Slot, f64)> = None;
        for (vi, v) in vs.iter().enumerate() {
            if ck.now() >= deadline {
                break;
            }
            if !allowed(v.mi, &counts, tc) {
                continue;
            }
            let mut cs = candidates_for(&plan, v, vs, sh, ck);
            cs.sort_by_key(|&(x, y)| (y, x));
            ck.tick(sort_work(cs.len()));
            // Mỗi hướng xoay một hạn mức: giữ cả hai đầu danh sách, lấy thưa phần giữa.
            let len = cs.len();
            let edge = 280.min(len / 2);
            let stride = ((len - 2 * edge).max(1) / 180).max(1);
            let mut checked = 0;
            for (ci, &(x, y)) in cs.iter().enumerate() {
                if checked >= 760 {
                    break;
                }
                if (edge..len - edge).contains(&ci) && !(ci - edge).is_multiple_of(stride) {
                    continue;
                }
                checked += 1;
                if !inside_and_clear(v, x, y, sh, &occ, ck) {
                    continue;
                }
                let me = Pb { x0: x, y0: y, x1: x + v.w_cells, y1: y + v.h_cells };
                // Ưu tiên nhẹ góc dưới trái; số con vẫn là mục tiêu chính.
                let score = base.map_or(me, |b| b.union(me)).score() as f64 + ((y * 4) as f64 + x as f64 * 0.05);
                if best.is_none_or(|(_, s)| score < s) {
                    best = Some((Slot { mi: v.mi, v: vi, x, y }, score));
                }
            }
        }
        let Some((slot, _)) = best else { break };
        if let Some(v) = vs.get(slot.v) {
            occ_add(&mut occ, v, slot.x, slot.y, ck);
        }
        plan.push(slot);
    }
    plan
}

/// Thêm một cặp như một nước đi: con đầu không chiếm mất chỗ con thứ hai cần.
fn augment_with_pair(plan: &[Slot], vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Plan {
    if plan.is_empty() {
        return plan.to_vec();
    }
    let counts = count_types(plan, tc);
    let base_occ = build_occ(plan, vs, sh.rh, ck);
    let bb = plan_bounds(plan, vs);
    let free_y = bb.y0.max(sh.rh - bb.y1);
    let free_x = bb.x0.max(sh.rw - bb.x1);
    let fits_edge =
        |v: &Variant| -> u8 { if (v.h_cells <= free_y && v.w_cells <= sh.rw) || (v.w_cells <= free_x && v.h_cells <= sh.rh) { 0 } else { 1 } };
    let mut order: Vec<usize> = (0..vs.len()).collect();
    order.sort_by_key(|&i| (vs.get(i).map_or(1, fits_edge), i));
    let valid = |seed: &[Slot], vv: &Variant, occ: &Occ| -> Vec<(i64, i64)> {
        let good = candidates_for(seed, vv, vs, sh, ck).into_iter().filter(|&(x, y)| inside_and_clear(vv, x, y, sh, occ, ck)).collect();
        sample_candidates(good, 24)
    };
    for &av in &order {
        if ck.now() >= deadline {
            break;
        }
        let Some(a) = vs.get(av) else { continue };
        if !allowed(a.mi, &counts, tc) {
            continue;
        }
        let ca = valid(plan, a, &base_occ);
        for &(ax, ay) in &ca {
            if ck.now() >= deadline {
                break;
            }
            let mut occ_a = build_occ(plan, vs, sh.rh, ck);
            let mut once = plan.to_vec();
            once.push(Slot { mi: a.mi, v: av, x: ax, y: ay });
            occ_add(&mut occ_a, a, ax, ay, ck);
            let counts_once = count_types(&once, tc);
            for &bv in &order {
                if ck.now() >= deadline {
                    break;
                }
                let Some(b) = vs.get(bv) else { continue };
                if !allowed(b.mi, &counts_once, tc) {
                    continue;
                }
                let cb = valid(&once, b, &occ_a);
                for &(bx, by) in &cb {
                    if ck.now() >= deadline {
                        break;
                    }
                    if !inside_and_clear(b, bx, by, sh, &occ_a, ck) {
                        continue;
                    }
                    once.push(Slot { mi: b.mi, v: bv, x: bx, y: by });
                    return once;
                }
            }
        }
    }
    plan.to_vec()
}

// ---------------------------------------------------------------- seed: dải, con lẻ, cặp

fn opposite_of(vs: &[Variant], ix: usize) -> Option<usize> {
    let v = vs.get(ix)?;
    let want = (v.angle + 180) % 360;
    vs.iter().position(|q| q.mi == v.mi && q.angle == want)
}

fn seed_plans(vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Vec<Plan> {
    let (rw, rh) = (sh.rw, sh.rh);
    let mut seeds: Vec<Plan> = Vec::new();
    let mut seen: HashSet<Vec<(usize, i64, i64)>> = HashSet::new();
    let mut push_seed = |plan: Plan, seeds: &mut Vec<Plan>| {
        if plan.is_empty() || !balanced(&plan, tc) {
            return;
        }
        let key: Vec<(usize, i64, i64)> = plan.iter().map(|s| (s.v, s.x, s.y)).collect();
        if seen.insert(key) {
            seeds.push(plan);
        }
    };
    let make_strip = |vi: usize, x0: i64, y: i64, n: i64| -> Option<Plan> {
        let vv = vs.get(vi)?;
        let pitch = vv.w_cells + (2 * vv.pad).max(1);
        let mut occ = build_occ(&[], vs, rh, ck);
        let mut plan = Vec::new();
        for si in 0..n {
            let xx = x0 + si * pitch;
            if !inside_and_clear(vv, xx, y, sh, &occ, ck) {
                return None;
            }
            occ_add(&mut occ, vv, xx, y, ck);
            plan.push(Slot { mi: vv.mi, v: vi, x: xx, y });
        }
        Some(plan)
    };

    // Hai dải đối hướng (khuôn dài): một hàng đầy rồi hàng ngược chiều lồng vào chỗ lõm.
    if tc == 1 {
        let mut twins: Vec<(Plan, i64)> = Vec::new();
        for a in 0..vs.len() {
            if ck.now() >= deadline {
                break;
            }
            let Some(b) = opposite_of(vs, a) else { continue };
            if a > b {
                continue;
            }
            let (Some(vv), Some(oo)) = (vs.get(a), vs.get(b)) else { continue };
            if (vv.h_cells as f64) < vv.w_cells as f64 * 1.25 {
                continue;
            }
            let sp = (2 * vv.pad).max(1);
            let n = 40.min((rw + sp).checked_div_euclid(vv.w_cells + sp).unwrap_or(0));
            if n < 3 {
                continue;
            }
            let base_ys = [rh - vv.h_cells, 0];
            let spare_x = rw - (vv.w_cells + (n - 1) * (vv.w_cells + sp));
            let base_xs = [spare_x.div_euclid(4), (3 * spare_x).div_euclid(4), spare_x.div_euclid(2), 0, spare_x];
            'anchors: for &bx in &base_xs {
                for &by in &base_ys {
                    if ck.now() >= deadline {
                        break 'anchors;
                    }
                    let Some(base) = make_strip(a, bx, by, n) else { continue };
                    let mut cs = candidates_for(&base, oo, vs, sh, ck);
                    let bb = plan_bounds(&base, vs);
                    let tail = (n - 1) * (oo.w_cells + 2 * oo.pad);
                    cs.sort_by_key(|&(x, y)| (bb.x1.max(x + oo.w_cells + tail) - bb.x0.min(x)) * (bb.y1.max(y + oo.h_cells) - bb.y0.min(y)));
                    let base_occ = build_occ(&base, vs, rh, ck);
                    let pitch2 = oo.w_cells + (2 * oo.pad).max(1);
                    for &(cx, cy) in &cs {
                        if ck.now() >= deadline {
                            break;
                        }
                        let second: Option<Plan> = (0..n)
                            .map(|k| {
                                let sx = cx + k * pitch2;
                                inside_and_clear(oo, sx, cy, sh, &base_occ, ck).then_some(Slot { mi: oo.mi, v: b, x: sx, y: cy })
                            })
                            .collect();
                        let Some(second) = second else { continue };
                        let mut joined = base.clone();
                        joined.extend(second);
                        let score = plan_score(&joined, vs);
                        twins.push((joined, score));
                        twins.sort_by_key(|t| t.1);
                        twins.truncate(12);
                        break;
                    }
                }
            }
        }
        for (plan, _) in twins {
            push_seed(plan, &mut seeds);
        }
    }

    // Dải canh giữa ở đáy và đỉnh tờ, mỗi hướng.
    let strip = |vi: usize, y: i64| -> Option<Plan> {
        if tc != 1 {
            return None;
        }
        let vv = vs.get(vi)?;
        let sp = (2 * vv.pad).max(1);
        let pitch = vv.w_cells + sp;
        let max_n = 40.min((rw + sp).checked_div_euclid(pitch).unwrap_or(0));
        for n in (2..=max_n).rev() {
            let left = (rw - (vv.w_cells + (n - 1) * pitch)).div_euclid(2);
            let mut occ = build_occ(&[], vs, rh, ck);
            let mut plan = Vec::new();
            let mut ok = true;
            for si in 0..n {
                let xx = left + si * pitch;
                if !inside_and_clear(vv, xx, y, sh, &occ, ck) {
                    ok = false;
                    break;
                }
                occ_add(&mut occ, vv, xx, y, ck);
                plan.push(Slot { mi: vv.mi, v: vi, x: xx, y });
            }
            if ok {
                return Some(plan);
            }
        }
        None
    };
    for (vi, v) in vs.iter().enumerate() {
        if let Some(p) = strip(vi, 0) {
            push_seed(p, &mut seeds);
        }
        if let Some(p) = strip(vi, rh - v.h_cells) {
            push_seed(p, &mut seeds);
        }
    }

    // Một con ở góc hoặc giữa tờ (PON góc có thể chặn cả bốn góc).
    let empty = build_occ(&[], vs, rh, ck);
    for (vi, v) in vs.iter().enumerate() {
        let (w, h) = (v.w_cells, v.h_cells);
        let anchors = [(0, 0), (rw - w, 0), (0, rh - h), (rw - w, rh - h), ((rw - w).div_euclid(2), (rh - h).div_euclid(2))];
        for (x, y) in anchors {
            if inside_and_clear(v, x, y, sh, &empty, ck) {
                push_seed(vec![Slot { mi: v.mi, v: vi, x, y }], &mut seeds);
            }
        }
    }

    // Cặp tiếp xúc nhỏ nhất của mỗi cặp hướng.
    let mut total = 0;
    'pairs: for (a, va) in vs.iter().enumerate() {
        for (b, vb) in vs.iter().enumerate() {
            if ck.now() >= deadline || total >= 32 {
                break 'pairs;
            }
            if tc > 1 && va.mi == vb.mi {
                continue;
            }
            let base = Slot { mi: va.mi, v: a, x: 0, y: 0 };
            if !inside_and_clear(va, 0, 0, sh, &empty, ck) {
                continue;
            }
            let pair_occ = build_occ(&[base], vs, rh, ck);
            let mut cands = candidates_for(&[base], vb, vs, sh, ck);
            cands.sort_by_key(|&(x, y)| (y, x));
            for &(x, y) in cands.iter().take(120) {
                if !inside_and_clear(vb, x, y, sh, &pair_occ, ck) {
                    continue;
                }
                push_seed(vec![base, Slot { mi: vb.mi, v: b, x, y }], &mut seeds);
                total += 1;
                break;
            }
        }
    }
    seeds
}

// ---------------------------------------------------------------- tiếp xúc theo biên dạng

/// `b` đặt ở (`dx`, `dy`) so với `a` không chồng biên dạng đã nở của `a`.
fn pair_profiles_clear(a: &Variant, b: &Variant, dx: i64, dy: i64, ck: &Clock) -> bool {
    let (na, nb) = (a.clear_rows.len() as i64, b.clear_rows.len() as i64);
    let mut ib = (b.pad - a.pad - dy).max(0);
    let mut work = 1usize;
    let mut clear = true;
    while ib < nb {
        let ia = ib + dy + a.pad - b.pad;
        if ia >= na {
            break;
        }
        work += 1;
        if let (Some(ra), Some(rb)) = (a.clear_rows.get(ia as usize), b.clear_rows.get(ib as usize))
            && row_hits(ra, rb, dx)
        {
            clear = false;
            break;
        }
        ib += 1;
    }
    ck.tick(work);
    clear
}

/// Khoảng `dx` nhỏ nhất để `b` (lệch `dy` hàng) nằm bên phải biên ngoài của `a`.
pub(super) fn right_profile_contact(a: &Variant, b: &Variant, dy: i64, ck: &Clock) -> i64 {
    let (na, nb) = (a.clear_rows.len() as i64, b.clear_rows.len() as i64);
    let mut ib = (b.pad - a.pad - dy).max(0);
    let mut right = 0;
    let mut work = 1usize;
    while ib < nb {
        let ia = ib + dy + a.pad - b.pad;
        if ia >= na {
            break;
        }
        work += 1;
        if let (Some(ra), Some(rb)) = (a.clear_rows.get(ia as usize), b.clear_rows.get(ib as usize))
            && let (Some(la), Some(fb)) = (ra.last(), rb.first())
        {
            right = right.max(la.1 - fb.0);
        }
        ib += 1;
    }
    ck.tick(work);
    right.max(1)
}

// ---------------------------------------------------------------- seed: hàng xen kẽ

pub(super) fn alternating_row_plans(vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Vec<Plan> {
    if tc != 1 {
        return Vec::new();
    }
    let (rw, rh) = (sh.rw, sh.rh);
    let mut out: Vec<Plan> = Vec::new();
    let retain = |plan: Plan, out: &mut Vec<Plan>| {
        if plan.is_empty() {
            return;
        }
        out.push(plan);
        out.sort_by(|u, w| best_first(u, w, vs));
        out.truncate(8);
    };
    // A → B → A trong một hàng: hai hướng đối nhau dùng chung cạnh xiên / chỗ lõm.
    let motif = |first: usize, second: usize, dy: i64, extra: bool, out: &mut Vec<Plan>| {
        let (Some(a), Some(b)) = (vs.get(first), vs.get(second)) else { return };
        let (ay, by) = ((-dy).max(0), dy.max(0));
        let ab = right_profile_contact(a, b, dy, ck);
        let ba = right_profile_contact(b, a, -dy, ck);
        let cycle = (ab + ba).max(right_profile_contact(a, a, 0, ck)).max(right_profile_contact(b, b, 0, ck));
        let height = (ay + a.h_cells).max(by + b.h_cells);
        let row_pitch = height + (2 * a.pad.max(b.pad)).max(1);
        if height > rh || a.w_cells.min(b.w_cells) > rw {
            return;
        }
        let rows = (rh - height).checked_div_euclid(row_pitch).unwrap_or(0) + 1;
        let spare_y = rh - height - (rows - 1) * row_pitch;
        let modulo = |x: i64| x.checked_rem_euclid(cycle).unwrap_or(0);
        let mut phases_x = vec![0, modulo(rw - a.w_cells), modulo(rw - b.w_cells - ab)];
        if extra {
            phases_x.extend([jround(cycle as f64 / 4.0) as i64, jround(cycle as f64 / 2.0) as i64, jround((3 * cycle) as f64 / 4.0) as i64]);
        }
        let phases_y = [0, spare_y.div_euclid(2), spare_y];
        let cols_end = (rw as f64 / cycle as f64).ceil() as i64;
        for &px in &phases_x {
            for &py in &phases_y {
                if extra && ck.now() >= deadline {
                    return;
                }
                let mut plan = Vec::new();
                let mut visits = 0usize;
                'rows: for row in 0..rows {
                    for col in -1..=cols_end {
                        if visits >= VISITS || plan.len() >= MAX_PIECES {
                            break 'rows;
                        }
                        let parts =
                            [(first, px + col * cycle, py + row * row_pitch + ay), (second, px + col * cycle + ab, py + row * row_pitch + by)];
                        for (vi, x, y) in parts {
                            visits += 1;
                            let Some(v) = vs.get(vi) else { continue };
                            // Tiếp xúc AB/BA và chu kỳ đã tách các con; chốt kiểm tra sau vẫn chạy.
                            if x < 0 || y < 0 || x + v.w_cells > rw || y + v.h_cells > rh || raw_hits_pon(v, x, y, &sh.dots, ck) {
                                continue;
                            }
                            plan.push(Slot { mi: v.mi, v: vi, x, y });
                        }
                    }
                }
                retain(plan, out);
            }
        }
    };
    for (ai, va) in vs.iter().enumerate() {
        if let Some(bi) = vs.iter().position(|o| o.mi == va.mi && o.angle == (va.angle + 180) % 360) {
            motif(ai, bi, 0, false, &mut out);
        }
    }
    'outer: for (ai, va) in vs.iter().enumerate() {
        for (bi, vb) in vs.iter().enumerate() {
            if ck.now() >= deadline {
                break 'outer;
            }
            if ai == bi {
                continue;
            }
            // Lệch một phần tư chiều cao tìm các hốc bất đối xứng.
            let quarter = jround(va.h_cells.min(vb.h_cells) as f64 / 4.0) as i64;
            motif(ai, bi, 0, true, &mut out);
            if quarter != 0 {
                motif(ai, bi, quarter, true, &mut out);
                motif(ai, bi, -quarter, true, &mut out);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- seed: ghép hàng đầy

struct Tpl {
    id: usize,
    family: (usize, usize),
    /// (biến thể, x) của các con trong hàng.
    parts: Vec<(usize, i64)>,
    height: i64,
}

#[derive(Clone, Default)]
struct Stack {
    rows: Vec<(Rc<Tpl>, i64)>,
    height: i64,
    total: usize,
}

#[derive(Clone)]
struct BeamState {
    rows: Stack,
    count: usize,
    height: i64,
    last: usize,
}

/// Hàng thấp nhất / cao nhất (+1) của từng cột ô (lỗ được lấp).
struct Cols {
    top: Vec<Option<i64>>,
    bottom: Vec<Option<i64>>,
    offset: i64,
}

fn column_envelope(v: &Variant) -> Cols {
    let mut c = Cols { top: Vec::new(), bottom: Vec::new(), offset: v.pad };
    for (r, spans) in v.clear_rows.iter().enumerate() {
        let r = r as i64;
        for &(s0, s1) in spans {
            for x in s0..s1 {
                let Ok(ix) = usize::try_from(x + c.offset) else { continue };
                if ix >= c.top.len() {
                    c.top.resize(ix + 1, None);
                    c.bottom.resize(ix + 1, None);
                }
                if let Some(t) = c.top.get_mut(ix)
                    && t.is_none()
                {
                    *t = Some(r - v.pad);
                }
                if let Some(b) = c.bottom.get_mut(ix) {
                    *b = Some(r - v.pad + 1);
                }
            }
        }
    }
    c
}

type DownCache = HashMap<(usize, usize, i64), i64>;

/// Số hàng tối thiểu để `b` (lệch `dx`) xếp chồng lên trên `a` theo biên ngoài từng cột.
fn down_contact(cols: &[Cols], cache: &mut DownCache, a: usize, b: usize, dx: i64) -> i64 {
    if let Some(&y) = cache.get(&(a, b, dx)) {
        return y;
    }
    let (Some(ca), Some(cb)) = (cols.get(a), cols.get(b)) else { return 1 };
    let mut y = 0;
    for (x, bottom) in ca.bottom.iter().enumerate() {
        let Some(bottom) = bottom else { continue };
        let bx = x as i64 - ca.offset - dx + cb.offset;
        if let Some(Some(top)) = usize::try_from(bx).ok().and_then(|i| cb.top.get(i)) {
            y = y.max(bottom - top);
        }
    }
    let y = y.max(1);
    cache.insert((a, b, dx), y);
    y
}

fn row_contact(cols: &[Cols], dc: &mut DownCache, rc: &mut HashMap<(usize, usize), i64>, a: &Tpl, b: &Tpl) -> i64 {
    if let Some(&y) = rc.get(&(a.id, b.id)) {
        return y;
    }
    let mut y = 1;
    for &(pav, pax) in &a.parts {
        for &(pbv, pbx) in &b.parts {
            y = y.max(down_contact(cols, dc, pav, pbv, pbx - pax));
        }
    }
    rc.insert((a.id, b.id), y);
    y
}

/// Chồng các hàng đầy của một hoặc hai hướng; trả tối đa 2 phương án đã kiểm tra.
pub(super) fn compositional_band_plans(vs: &[Variant], tc: usize, sh: &Sheet, gap: f64, step: f64, deadline: f64, ck: &Clock) -> Vec<Plan> {
    if tc != 1 || vs.is_empty() {
        return Vec::new();
    }
    let (rw, rh) = (sh.rw, sh.rh);
    let min_side = vs.iter().map(|v| v.w_cells.min(v.h_cells)).min().unwrap_or(0);
    let area_work: i64 = vs.iter().map(|v| v.w_cells.saturating_mul(v.h_cells)).sum();
    if min_side <= 0 {
        return Vec::new();
    }
    // Tìm tổ hợp cho số con vừa phải; khuôn rất nhỏ để các seed lưới lo.
    let skip = (rw * rh) as f64 / (min_side * min_side) as f64 > 400.0
        || (rw as f64 / min_side as f64).ceil() > 40.0
        || (rh as f64 / min_side as f64).ceil() > 16.0
        || area_work > 2_400_000;
    if skip {
        return Vec::new();
    }
    let max_depth = 12.min((rh as f64 / min_side as f64).ceil() as i64);
    let mut local = Vec::with_capacity(vs.len());
    for o in vs {
        // Một ô dự phòng chỉ để tìm seed; kiểm tra vật lý dùng khuôn và khe gốc.
        let pad_gap = gap + step.max(2.0 * o.curve_error);
        let Some(mut v) = build_variant(&o.groups, o.mi, o.vi, 0, pad_gap, step, ck) else { return Vec::new() };
        v.angle = o.angle;
        local.push(v);
    }
    let columns: Vec<Cols> = local.iter().map(column_envelope).collect();
    let mut contacts: DownCache = HashMap::new();
    let mut row_contacts: HashMap<(usize, usize), i64> = HashMap::new();
    let next_id = Cell::new(0usize);
    let template = |first: usize, second: usize, shorten: bool, phase: u8| -> Option<Rc<Tpl>> {
        let (a, b) = (local.get(first)?, local.get(second)?);
        let ab = right_profile_contact(a, b, 0, ck);
        let cycle = if first == second {
            ab
        } else {
            (ab + right_profile_contact(b, a, 0, ck)).max(right_profile_contact(a, a, 0, ck)).max(right_profile_contact(b, b, 0, ck))
        };
        let mut parts: Vec<(usize, i64)> = Vec::new();
        for col in 0..40 {
            let x = col * cycle;
            if x + a.w_cells > rw {
                break;
            }
            parts.push((first, x));
            if first != second && x + ab + b.w_cells <= rw {
                parts.push((second, x + ab));
            }
        }
        if shorten && parts.len() > 1 {
            parts.pop();
        }
        if parts.is_empty() {
            return None;
        }
        let (mut width, mut height) = (0, 0);
        for &(v, x) in &parts {
            if let Some(vv) = local.get(v) {
                width = width.max(x + vv.w_cells);
                height = height.max(vv.h_cells);
            }
        }
        let offset = match phase {
            0 => 0,
            2 => rw - width,
            _ => (rw - width).div_euclid(2),
        };
        for p in &mut parts {
            p.1 += offset;
        }
        let id = next_id.get();
        next_id.set(id + 1);
        Some(Rc::new(Tpl { id, family: (first, second), parts, height }))
    };
    let opposite = |i: usize| local.get(i).and_then(|li| local.iter().position(|lj| lj.angle == (li.angle + 180) % 360));
    let mut base: Vec<Rc<Tpl>> = Vec::new();
    for i in 0..local.len() {
        base.extend(template(i, i, false, 1));
        if let Some(j) = opposite(i) {
            base.extend(template(i, j, false, 1));
        }
    }
    let mut append = |rows: &Stack, t: &Rc<Tpl>| -> Option<Stack> {
        let mut y = 0;
        for (rt, ry) in &rows.rows {
            y = y.max(ry + row_contact(&columns, &mut contacts, &mut row_contacts, rt, t));
        }
        if y + t.height > rh {
            return None;
        }
        let mut added = rows.clone();
        added.rows.push((Rc::clone(t), y));
        added.height = rows.height.max(y + t.height);
        added.total = rows.total + t.parts.len();
        Some(added)
    };
    let hits_pon = |v: &Variant, x: i64, y: i64| -> bool {
        // Khung rời đĩa né PON thì đường bao bên trong cũng rời.
        for d in &sh.dots {
            let dx = (x as f64 - d.x).max(d.x - x as f64 - v.w_cells as f64).max(0.0);
            let dy = (y as f64 - d.y).max(d.y - y as f64 - v.h_cells as f64).max(0.0);
            if dx * dx + dy * dy < d.radius * d.radius {
                return raw_hits_pon(v, x, y, &sh.dots, ck);
            }
        }
        false
    };
    let materialize = |rows: &Stack, offset: i64| -> Plan {
        let mut plan = Vec::new();
        for (t, ry) in &rows.rows {
            for &(pv, x) in &t.parts {
                let Some(vv) = vs.get(pv) else { continue };
                let y = ry + offset;
                if x < 0 || y < 0 || x + vv.w_cells > rw || y + vv.h_cells > rh || hits_pon(vv, x, y) {
                    continue;
                }
                plan.push(Slot { mi: vv.mi, v: pv, x, y });
            }
        }
        plan
    };
    let retain = |plan: Plan, pending: &mut Vec<(Plan, i64)>| {
        if plan.is_empty() {
            return;
        }
        if pending.len() == 12 && pending.last().is_some_and(|l| plan.len() < l.0.len()) {
            return;
        }
        if pending.iter().any(|(p, _)| *p == plan) {
            return;
        }
        let score = plan_score(&plan, vs);
        let mut at = 0;
        while let Some((p, s)) = pending.get(at) {
            if p.len() > plan.len() || (p.len() == plan.len() && *s <= score) {
                at += 1;
            } else {
                break;
            }
        }
        pending.insert(at, (plan, score));
        pending.truncate(12);
    };
    let retain_rows = |rows: &Stack, pending: &mut Vec<(Plan, i64)>| {
        if rows.rows.is_empty() {
            return;
        }
        if pending.len() == 12 && pending.last().is_some_and(|l| rows.total < l.0.len()) {
            return;
        }
        let spare = rh - rows.height;
        for p in [0, spare.div_euclid(2), spare] {
            retain(materialize(rows, p), pending);
        }
    };
    let mut pending: Vec<(Plan, i64)> = Vec::new();
    // Cơ sở tất định: một họ hàng đầy trước, rồi một họ khác lấp phần còn lại.
    for t in &base {
        let mut prefix = Stack::default();
        for depth in 0..max_depth {
            let Some(next) = append(&prefix, t) else { break };
            prefix = next;
            retain_rows(&prefix, &mut pending);
            for u in &base {
                let mut tail = prefix.clone();
                for _ in depth + 1..max_depth {
                    let Some(c) = append(&tail, u) else { break };
                    tail = c;
                    retain_rows(&tail, &mut pending);
                }
            }
        }
    }
    // Beam: hàng ngắn / so le, đổi hướng nhiều lần.
    if ck.now() < deadline {
        let mut extra: Vec<Rc<Tpl>> = Vec::new();
        for i in 0..local.len() {
            for phase in 0..3u8 {
                extra.extend(template(i, i, false, phase));
                extra.extend(template(i, i, true, phase));
                if let Some(j) = opposite(i) {
                    extra.extend(template(i, j, false, phase));
                    extra.extend(template(i, j, true, phase));
                }
            }
        }
        let mut beam = vec![BeamState { rows: Stack::default(), count: 0, height: 0, last: usize::MAX }];
        for _ in 0..max_depth {
            if ck.now() >= deadline {
                break;
            }
            let mut next: Vec<BeamState> = Vec::new();
            'beam: for state in &beam {
                for (ti, t) in extra.iter().enumerate() {
                    if ck.now() >= deadline {
                        break 'beam;
                    }
                    let Some(rows) = append(&state.rows, t) else { continue };
                    let count = materialize(&rows, 0).len();
                    let height = rows.height;
                    retain_rows(&rows, &mut pending);
                    next.push(BeamState { rows, count, height, last: ti });
                }
            }
            if next.is_empty() {
                break;
            }
            next.sort_by(|a, b| b.count.cmp(&a.count).then(a.height.cmp(&b.height)));
            // Mỗi họ hai nhánh: nhiều con nhất và thấp nhất.
            let mut best: HashMap<(usize, usize), (usize, usize, bool)> = HashMap::new();
            for (si, st) in next.iter().enumerate() {
                let Some(fam) = extra.get(st.last).map(|t| t.family) else { continue };
                match best.entry(fam) {
                    Entry::Vacant(e) => {
                        e.insert((si, si, false));
                    }
                    Entry::Occupied(mut e) => {
                        let low = e.get().1;
                        if let Some(l) = next.get(low)
                            && (st.height < l.height || (st.height == l.height && st.count > l.count))
                        {
                            e.get_mut().1 = si;
                        }
                    }
                }
            }
            beam = Vec::new();
            for t in &extra {
                let Some(e) = best.get_mut(&t.family) else { continue };
                if e.2 {
                    continue;
                }
                e.2 = true;
                if let Some(s) = next.get(e.0) {
                    beam.push(s.clone());
                }
                if e.1 != e.0
                    && let Some(s) = next.get(e.1)
                {
                    beam.push(s.clone());
                }
            }
            beam.truncate(16);
        }
    }
    let mut out = Vec::new();
    for (plan, _) in &pending {
        if !out.is_empty() && ck.now() >= deadline {
            break;
        }
        if !verify_plan(plan, vs, sh, ck) || !verify_geometry(&to_physical(plan), vs, (rw as f64, rh as f64), &sh.dots, gap, step, ck) {
            continue;
        }
        out.push(plan.clone());
        if out.len() >= 2 {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- seed: băng đối hướng

fn band_plans(vs: &[Variant], tc: usize, sh: &Sheet, gap: f64, step: f64, deadline: f64, ck: &Clock) -> Vec<Plan> {
    if tc != 1 {
        return Vec::new();
    }
    let (rw, rh) = (sh.rw, sh.rh);
    let mut pending: Vec<Plan> = Vec::new();
    let retain = |plan: Plan, pending: &mut Vec<Plan>| {
        if plan.is_empty() {
            return;
        }
        pending.push(plan);
        pending.sort_by(|u, w| best_first(u, w, vs));
        pending.truncate(8);
    };
    let ppc = |a: &Variant, b: &Variant, dx: i64, dy: i64| pair_profiles_clear(a, b, dx, dy, ck);
    let motif = |first: usize, second: usize, shift: i64, timed: bool, pending: &mut Vec<Plan>| {
        let (Some(a), Some(b)) = (vs.get(first), vs.get(second)) else { return };
        let pad2 = 2 * a.pad.max(b.pad);
        let pitch = a.w_cells.max(b.w_cells) + pad2;
        let limit = a.h_cells.max(b.h_cells) + pad2;
        // Chỉ hai cột kề nhau mới có thể va nhau.
        let mut found = None;
        for dy in 0..=limit {
            if timed && ck.now() >= deadline {
                return;
            }
            if ppc(a, b, shift, dy) && ppc(a, b, shift - pitch, dy) {
                found = Some(dy);
                break;
            }
        }
        let Some(dy) = found else { return };
        let height = a.h_cells.max(dy + b.h_cells);
        if height > rh || a.w_cells.min(b.w_cells) > rw {
            return;
        }
        let start = ((height + pad2) as f64 / 2.0).ceil() as i64;
        let mut found = None;
        for period in start..=height + pad2 {
            if timed && ck.now() >= deadline {
                return;
            }
            if ppc(a, a, 0, period)
                && ppc(b, b, 0, period)
                && ppc(a, b, shift, period + dy)
                && ppc(a, b, shift - pitch, period + dy)
                && ppc(b, a, -shift, period - dy)
                && ppc(b, a, pitch - shift, period - dy)
            {
                found = Some(period);
                break;
            }
        }
        let Some(period) = found else { return };
        let unit_width = a.w_cells.max(shift + b.w_cells);
        let columns = ((rw - unit_width).checked_div_euclid(pitch).unwrap_or(0) + 1).max(1);
        let rows = (rh - height).checked_div_euclid(period).unwrap_or(0) + 1;
        let spare_x = rw - unit_width - (columns - 1) * pitch;
        let spare_y = rh - height - (rows - 1) * period;
        let cols_end = (rw as f64 / pitch as f64).ceil() as i64;
        for px in [0, spare_x.div_euclid(2), spare_x] {
            for py in [0, spare_y.div_euclid(2), spare_y] {
                if timed && ck.now() >= deadline {
                    return;
                }
                let mut plan = Vec::new();
                let mut visits = 0usize;
                'rows: for row in 0..rows {
                    for col in -1..=cols_end {
                        if visits >= VISITS || plan.len() >= MAX_PIECES {
                            break 'rows;
                        }
                        let parts = [(first, px + col * pitch, py + row * period), (second, px + col * pitch + shift, py + row * period + dy)];
                        for (vi, x, y) in parts {
                            visits += 1;
                            let Some(v) = vs.get(vi) else { continue };
                            if x < 0 || y < 0 || x + v.w_cells > rw || y + v.h_cells > rh || raw_hits_pon(v, x, y, &sh.dots, ck) {
                                continue;
                            }
                            plan.push(Slot { mi: v.mi, v: vi, x, y });
                        }
                    }
                }
                retain(plan, pending);
            }
        }
    };
    // Cặp đối hướng (khuôn không đối xứng 180°), lệch nửa cột; rồi các độ lệch khác nếu còn ngân sách.
    let pair = |ai: usize| -> Option<(usize, i64)> {
        let bi = opposite_of(vs, ai)?;
        let (va, vb) = (vs.get(ai)?, vs.get(bi)?);
        if same_groups(&va.groups, &vb.groups) {
            return None;
        }
        Some((bi, va.w_cells.max(vb.w_cells) + 2 * va.pad.max(vb.pad)))
    };
    for ai in 0..vs.len() {
        if let Some((bi, pitch)) = pair(ai) {
            motif(ai, bi, half(pitch), false, &mut pending);
        }
    }
    'outer: for ai in 0..vs.len() {
        if ck.now() >= deadline {
            break;
        }
        let Some((bi, pitch)) = pair(ai) else { continue };
        for s in [0, jround(pitch as f64 / 4.0) as i64, jround((3 * pitch) as f64 / 4.0) as i64] {
            if ck.now() >= deadline {
                break 'outer;
            }
            motif(ai, bi, s, true, &mut pending);
        }
    }
    let mut out = Vec::new();
    for plan in &pending {
        if !out.is_empty() && ck.now() >= deadline {
            break;
        }
        if !verify_plan(plan, vs, sh, ck) || !verify_geometry(&to_physical(plan), vs, (rw as f64, rh as f64), &sh.dots, gap, step, ck) {
            continue;
        }
        out.push(plan.clone());
        if out.len() >= 2 {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- seed: lưới so le

fn lattice_plans(vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Vec<Plan> {
    if tc != 1 {
        return Vec::new();
    }
    let (rw, rh) = (sh.rw, sh.rh);
    let ppc = |a: &Variant, b: &Variant, dx: i64, dy: i64| pair_profiles_clear(a, b, dx, dy, ck);
    let mut out: Vec<Plan> = Vec::new();
    let motif = |first: usize, second: usize, shift: i64, out: &mut Vec<Plan>| {
        let (Some(a), Some(b)) = (vs.get(first), vs.get(second)) else { return };
        let pmax = a.pad.max(b.pad);
        let pitch = a.w_cells.max(b.w_cells) + (2 * pmax).max(1);
        let hmax = a.h_cells.max(b.h_cells);
        let max_y = hmax + 2 * pmax;
        let mut found = None;
        for py in 1..=max_y {
            if ppc(a, b, shift, py)
                && ppc(a, b, shift - pitch, py)
                && ppc(b, a, -shift, py)
                && ppc(b, a, pitch - shift, py)
                && ppc(a, a, 0, 2 * py)
                && ppc(b, b, 0, 2 * py)
            {
                found = Some(py);
                break;
            }
        }
        let Some(py) = found else { return };
        let columns = (rw - a.w_cells).checked_div_euclid(pitch).unwrap_or(0) + 1;
        let spare_x = rw - (a.w_cells + (columns - 1).max(0) * pitch);
        let rows = (rh - hmax).checked_div_euclid(py).unwrap_or(0) + 1;
        let spare_y = rh - (hmax + (rows - 1).max(0) * py);
        // Hàng cách hai bước có khung nở rời nhau: cả lưới tách nhau ngay từ cách dựng.
        let proven = (0..=pitch).contains(&shift) && 2 * py >= hmax + 2 * pmax;
        for px in [0, spare_x.div_euclid(2), spare_x] {
            for py0 in [0, spare_y.div_euclid(2), spare_y] {
                let mut plan = Vec::new();
                let mut occ = (!proven).then(|| build_occ(&[], vs, rh, ck));
                'rows: for row in 0..rows {
                    let odd = row % 2 != 0;
                    let vx = if odd { second } else { first };
                    let Some(vv) = vs.get(vx) else { continue };
                    let yy = py0 + row * py;
                    for col in -1..=columns {
                        if plan.len() >= MAX_PIECES {
                            break 'rows;
                        }
                        let xx = px + if odd { shift } else { 0 } + col * pitch;
                        match occ.as_mut() {
                            None => {
                                if xx < 0 || yy < 0 || xx + vv.w_cells > rw || yy + vv.h_cells > rh || raw_hits_pon(vv, xx, yy, &sh.dots, ck) {
                                    continue;
                                }
                            }
                            Some(o) => {
                                if !inside_and_clear(vv, xx, yy, sh, o, ck) {
                                    continue;
                                }
                                occ_add(o, vv, xx, yy, ck);
                            }
                        }
                        plan.push(Slot { mi: vv.mi, v: vx, x: xx, y: yy });
                    }
                }
                if !plan.is_empty() {
                    out.push(plan);
                }
            }
        }
    };
    for (ai, va) in vs.iter().enumerate() {
        motif(ai, ai, half(va.w_cells + (2 * va.pad).max(1)), &mut out);
    }
    for (ai, va) in vs.iter().enumerate() {
        for (bi, vb) in vs.iter().enumerate() {
            if vb.angle == (va.angle + 180) % 360 {
                motif(ai, bi, 0, &mut out);
            }
        }
    }
    'outer: for (ai, va) in vs.iter().enumerate() {
        for (bi, vb) in vs.iter().enumerate() {
            if ck.now() >= deadline {
                break 'outer;
            }
            if (va.w_cells - vb.w_cells).abs() > 2 || (va.h_cells - vb.h_cells).abs() > 2 {
                continue;
            }
            let pitch_x = va.w_cells.max(vb.w_cells) + (2 * va.pad.max(vb.pad)).max(1);
            let shifts = [half(pitch_x), 0, jround(pitch_x as f64 / 4.0) as i64, jround((3 * pitch_x) as f64 / 4.0) as i64];
            for (si, &s) in shifts.iter().enumerate() {
                if ck.now() >= deadline {
                    break 'outer;
                }
                if ai == bi && si == 0 {
                    continue;
                }
                motif(ai, bi, s, &mut out);
            }
        }
    }
    out.sort_by(|u, w| best_first(u, w, vs));
    out.truncate(8);
    out
}

// ---------------------------------------------------------------- seed: nhiều mẫu xếp kệ

fn mixed_row_plans(vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Vec<Plan> {
    if tc <= 1 {
        return Vec::new();
    }
    let mut lookup: Vec<[Option<usize>; 4]> = vec![[None; 4]; tc];
    let mut spacing = 1;
    for (ix, v) in vs.iter().enumerate() {
        if let Some(slot) = lookup.get_mut(v.mi).and_then(|l| l.get_mut(v.vi)) {
            *slot = Some(ix);
        }
        spacing = spacing.max(2 * v.pad);
    }
    let mut out = Vec::new();
    'all: for phase in 0..4usize {
        for alternate in [false, true] {
            for start in 0..tc.min(2) {
                if ck.now() >= deadline {
                    break 'all;
                }
                let mut plan: Plan = Vec::new();
                let mut occ = build_occ(&[], vs, sh.rh, ck);
                let (mut x, mut y, mut row_height, mut row) = (0i64, 0i64, 0i64, 0usize);
                let mut guard = 0;
                loop {
                    if plan.len() >= 220 || guard >= 2000 {
                        break;
                    }
                    guard += 1;
                    if ck.now() >= deadline {
                        break;
                    }
                    let mi = (start + plan.len()) % tc;
                    let want = (phase + if alternate && row % 2 == 1 { 2 } else { 0 }) % 4;
                    let mut chosen = None;
                    for t in 0..4 {
                        let Some(ix) = lookup.get(mi).and_then(|l| l.get((want + t) % 4)).copied().flatten() else { continue };
                        if let Some(v) = vs.get(ix)
                            && v.w_cells <= sh.rw
                            && y + v.h_cells <= sh.rh
                        {
                            chosen = Some(ix);
                            break;
                        }
                    }
                    let Some(ix) = chosen else { break };
                    let Some(v) = vs.get(ix) else { break };
                    if x + v.w_cells > sh.rw {
                        x = 0;
                        y += row_height + spacing;
                        row_height = 0;
                        row += 1;
                        continue;
                    }
                    row_height = row_height.max(v.h_cells);
                    if !inside_and_clear(v, x, y, sh, &occ, ck) {
                        // Nhích qua PON góc từng bước nhỏ.
                        x += spacing;
                        continue;
                    }
                    plan.push(Slot { mi, v: ix, x, y });
                    occ_add(&mut occ, v, x, y, ck);
                    x += v.w_cells + spacing;
                }
                if !plan.is_empty() && balanced(&plan, tc) {
                    out.push(plan);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------- canh tâm, gỡ / lấp, mép

/// Dời nguyên khối (ô nguyên) về giữa vùng dàn, tránh PON.
fn centre_plan(plan: &[Slot], vs: &[Variant], sh: &Sheet, ck: &Clock) -> Plan {
    if plan.is_empty() {
        return Vec::new();
    }
    let b = plan_bounds(plan, vs);
    let want_x = jround((sh.rw - (b.x1 - b.x0)) as f64 / 2.0 - b.x0 as f64) as i64;
    let want_y = jround((sh.rh - (b.y1 - b.y0)) as f64 / 2.0 - b.y0 as f64) as i64;
    let mut tries = vec![(want_x, want_y)];
    for r in 1..=14 {
        tries.extend([(want_x - r, want_y), (want_x + r, want_y), (want_x, want_y - r), (want_x, want_y + r)]);
    }
    for (tx, ty) in tries {
        let fits = |s: &Slot| -> bool {
            let Some(v) = vs.get(s.v) else { return false };
            let (x, y) = (s.x + tx, s.y + ty);
            if x < 0 || y < 0 || x + v.w_cells > sh.rw || y + v.h_cells > sh.rh {
                return false;
            }
            !raw_hits_pon(v, x, y, &sh.dots, ck)
        };
        if plan.iter().all(fits) {
            return plan.iter().map(|s| Slot { x: s.x + tx, y: s.y + ty, ..*s }).collect();
        }
    }
    plan.to_vec()
}

pub(super) fn edge_margin(plan: &[Slot], vs: &[Variant], rw: i64, rh: i64) -> i64 {
    if plan.is_empty() {
        return 0;
    }
    let b = plan_bounds(plan, vs);
    b.x0.min(b.y0).min(rw - b.x1).min(rh - b.y1)
}

/// Gỡ một trong 12 con thêm sau cùng rồi để mọi hướng lấp lại.
fn repair_plans(best: &[Slot], vs: &[Variant], tc: usize, sh: &Sheet, deadline: f64, ck: &Clock) -> Plan {
    let mut current = best.to_vec();
    let mut passes = 0;
    while passes < 12 && ck.now() < deadline {
        passes += 1;
        let mut improved = false;
        let start = current.len().saturating_sub(12);
        for i in start..current.len() {
            if ck.now() >= deadline {
                break;
            }
            let seed: Plan = current.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, s)| *s).collect();
            let trial = grow_plan(&seed, vs, tc, sh, deadline, ck);
            if balanced(&trial, tc) && compare_plans(&trial, Some(&current), vs) > 0 {
                current = trial;
                improved = true;
                break;
            }
        }
        if !improved {
            break;
        }
    }
    current
}

/// Giữ số con, đẩy cặp khuôn dài sát mép nhất lồng sâu hơn để khối cách mép đều hơn.
pub(super) fn polish_edges(plan: &[Slot], vs: &[Variant], sh: &Sheet, gap: f64, step: f64, deadline: f64, ck: &Clock) -> Plan {
    if plan.len() < 2 || plan.len() > 80 {
        return plan.to_vec();
    }
    let (rw, rh) = (sh.rw, sh.rh);
    let mut picked: Option<(usize, i64, bool, bool)> = None;
    for (i, s) in plan.iter().enumerate() {
        let Some(v) = vs.get(s.v) else { continue };
        let (w, h) = (v.w_cells, v.h_cells);
        if (w.max(h) as f64) < 1.3 * w.min(h) as f64 {
            continue;
        }
        let axis_y = w >= h;
        let (low, high) = if axis_y { (s.y, rh - s.y - h) } else { (s.x, rw - s.x - w) };
        let dist = low.min(high);
        if picked.is_none_or(|p| dist < p.1) {
            picked = Some((i, dist, axis_y, low <= high));
        }
    }
    let Some((pi, _, use_y, toward_low)) = picked else { return plan.to_vec() };
    let Some(first) = plan.get(pi).copied() else { return plan.to_vec() };
    let Some(fv) = vs.get(first.v) else { return plan.to_vec() };
    let mut mate: Option<(usize, f64)> = None;
    for (i, s) in plan.iter().enumerate() {
        if i == pi || s.mi != first.mi {
            continue;
        }
        let Some(v) = vs.get(s.v) else { continue };
        if (v.w_cells - fv.w_cells).abs() > 1 || (v.h_cells - fv.h_cells).abs() > 1 {
            continue;
        }
        let dist = sq(s.x as f64 + v.w_cells as f64 / 2.0 - first.x as f64 - fv.w_cells as f64 / 2.0)
            + sq(s.y as f64 + v.h_cells as f64 / 2.0 - first.y as f64 - fv.h_cells as f64 / 2.0);
        if mate.is_none_or(|m| dist < m.1) {
            mate = Some((i, dist));
        }
    }
    let Some((mi, _)) = mate else { return plan.to_vec() };
    let Some(second) = plan.get(mi).copied() else { return plan.to_vec() };
    let base: Plan = plan.iter().enumerate().filter(|(i, _)| *i != pi && *i != mi).map(|(_, s)| *s).collect();
    // Hướng gốc và hướng xoay 180° thật của nó; không bao giờ lật.
    let headings = |slot: &Slot| -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        if let Some(cur) = vs.get(slot.v) {
            let opp = (cur.angle + 180) % 360;
            out.extend(vs.iter().enumerate().filter(|(_, v)| v.mi == slot.mi && v.angle == opp).map(|(j, _)| j));
        }
        out.push(slot.v);
        out
    };
    let (a_variants, b_variants) = (headings(&first), headings(&second));
    let cands = |seed: &[Slot], vi: usize, occ: &Occ| -> Vec<(i64, i64)> {
        let Some(vv) = vs.get(vi) else { return Vec::new() };
        let mut good: Vec<(i64, i64)> =
            candidates_for(seed, vv, vs, sh, ck).into_iter().filter(|&(x, y)| inside_and_clear(vv, x, y, sh, occ, ck)).collect();
        good.sort_by(|a, b| {
            let primary = if use_y { a.1.cmp(&b.1) } else { a.0.cmp(&b.0) };
            let primary = if toward_low { primary.reverse() } else { primary };
            primary.then(if use_y { a.0.cmp(&b.0) } else { a.1.cmp(&b.1) })
        });
        let sep = 4.max(jround(vv.w_cells.min(vv.h_cells) as f64 / 8.0) as i64);
        let mut sampled: Vec<(i64, i64)> = Vec::new();
        for p in good {
            if sampled.len() >= 4 {
                break;
            }
            if sampled.iter().all(|q| (p.0 - q.0).abs() >= sep || (p.1 - q.1).abs() >= sep) {
                sampled.push(p);
            }
        }
        sampled
    };
    let mut best = plan.to_vec();
    let mut best_edge = edge_margin(plan, vs, rw, rh);
    let base_occ = build_occ(&base, vs, rh, ck);
    for (av, &aa) in a_variants.iter().enumerate() {
        if av != 0 && ck.now() >= deadline {
            break;
        }
        let ca = cands(&base, aa, &base_occ);
        for (ai, &(ax, ay)) in ca.iter().enumerate() {
            let go = (av == 0 && ai < 2) || ck.now() < deadline;
            if !go {
                break;
            }
            let mut once = base.clone();
            once.push(Slot { mi: first.mi, v: aa, x: ax, y: ay });
            let occ = build_occ(&once, vs, rh, ck);
            for &bb in &b_variants {
                for (bx, by) in cands(&once, bb, &occ) {
                    let mut trial = once.clone();
                    trial.push(Slot { mi: second.mi, v: bb, x: bx, y: by });
                    let trial = centre_plan(&trial, vs, sh, ck);
                    let e = edge_margin(&trial, vs, rw, rh);
                    if e <= best_edge {
                        continue;
                    }
                    if !verify_geometry(&to_physical(&trial), vs, (rw as f64, rh as f64), &sh.dots, gap, step, ck) {
                        continue;
                    }
                    best = trial;
                    best_edge = e;
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------- khuôn tròn

#[derive(Clone, Copy, Debug)]
pub(super) struct Circle {
    /// Bán kính bao của đường bao đã làm phẳng (mm).
    pub(super) radius: f64,
    pub(super) cx: f64,
    pub(super) cy: f64,
}

/// Khuôn một đường bao gần tròn (xét cả đỉnh lẫn trung điểm cạnh, nên vuông / thoi không qua).
pub(super) fn circle_like(groups: &[Group]) -> Option<Circle> {
    let [g] = groups else { return None };
    let [c] = g.as_slice() else { return None };
    let b = bounds_of_groups(groups)?;
    if (b.w - b.h).abs() > 0.12 {
        return None;
    }
    let (cx, cy) = (b.x0 + b.w / 2.0, b.y0 + b.h / 2.0);
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    let n = c.len();
    for (i, p) in c.iter().enumerate() {
        let d = (sq(p[0] - cx) + sq(p[1] - cy)).sqrt();
        lo = lo.min(d);
        hi = hi.max(d);
        let Some(next) = c.get((i + 1) % n) else { continue };
        let d = (sq((p[0] + next[0]) / 2.0 - cx) + sq((p[1] + next[1]) / 2.0 - cy)).sqrt();
        lo = lo.min(d);
        hi = hi.max(d);
    }
    let r = (lo + hi) / 2.0;
    let ok = r != 0.0 && !r.is_nan() && hi - lo <= 0.12f64.max(r * 0.025);
    ok.then_some(Circle { radius: hi, cx, cy })
}

/// Lưới lục giác động: thử độ so le và pha theo khe, lề, PON hiện tại.
fn circle_plan(v: &Variant, sh: &Sheet, gap: f64, deadline: f64, ck: &Clock) -> Plan {
    let (rw, rh) = (sh.rw, sh.rh);
    let dia = v.w_cells.max(v.h_cells);
    let prof = circle_like(&v.groups);
    let radius = prof.map_or(dia as f64 / 2.0, |p| p.radius / v.step) + v.curve_error / v.step;
    let cx = prof.map_or(dia as f64 / 2.0, |p| p.cx / v.step);
    let cy = prof.map_or(v.h_cells as f64 / 2.0, |p| p.cy / v.step);
    // Làm tròn khoảng cách ra ngoài, không bao giờ vào trong.
    let pitch_x = (if gap == 0.0 { dia } else { dia + 1 }).max((2.0 * radius + gap / v.step - 1e-7).ceil() as i64);
    if pitch_x <= 0 {
        return Vec::new();
    }
    let cols = (rw - dia).div_euclid(pitch_x) + 1;
    let spare_x = rw - (dia + (cols - 1) * pitch_x);
    let max_shift = pitch_x.div_euclid(2).min(spare_x).max(0);
    let shift_step = max_shift.max(1).div_euclid(12).max(1);
    let one = std::slice::from_ref(v);
    let mut best: Option<Plan> = None;
    let mut shift = 0;
    while shift <= max_shift && ck.now() < deadline {
        let pitch_y = ((sq(pitch_x as f64) - sq(shift as f64)).max(1.0).sqrt().ceil() as i64).max(1);
        let rows = (rh - dia).div_euclid(pitch_y) + 1;
        let spare_y = rh - (dia + (rows - 1) * pitch_y);
        'phases: for px in [0, (spare_x - shift).div_euclid(2), spare_x - shift] {
            if ck.now() >= deadline {
                break;
            }
            if px < 0 {
                continue;
            }
            for py in [0, spare_y.div_euclid(2), spare_y] {
                if ck.now() >= deadline {
                    break 'phases;
                }
                let mut plan = Vec::new();
                'fill: for row in 0..rows {
                    let y = py + row * pitch_y;
                    let row_shift = if row % 2 != 0 { shift } else { 0 };
                    for col in 0..cols {
                        if plan.len() >= MAX_PIECES {
                            break 'fill;
                        }
                        let x = px + row_shift + col * pitch_x;
                        if x < 0 || x + dia > rw || y < 0 || y + dia > rh {
                            continue;
                        }
                        // Khoảng cách các con do lưới bảo đảm; với PON thử như hai hình tròn.
                        let safe = sh.dots.iter().all(|d| {
                            let (dx, dy) = (x as f64 + cx - d.x, y as f64 + cy - d.y);
                            let min_d = radius + d.radius;
                            dx * dx + dy * dy >= min_d * min_d - 1e-6
                        });
                        if safe {
                            plan.push(Slot { mi: 0, v: 0, x, y });
                        }
                    }
                }
                ck.tick(1 + usize::try_from(rows.max(0) * cols.max(0)).unwrap_or(0));
                if compare_plans(&plan, best.as_deref(), one) > 0 {
                    best = Some(plan);
                }
            }
        }
        shift = if shift < max_shift { max_shift.min(shift + shift_step) } else { max_shift + 1 };
    }
    best.unwrap_or_default()
}

fn verify_circle_plan(plan: &[Slot], v: &Variant, sh: &Sheet, gap_cells: f64) -> bool {
    let prof = circle_like(&v.groups);
    let radius = prof.map_or(v.w_cells as f64 / 2.0, |p| p.radius / v.step);
    let cx = prof.map_or(v.w_cells as f64 / 2.0, |p| p.cx / v.step);
    let cy = prof.map_or(v.h_cells as f64 / 2.0, |p| p.cy / v.step);
    let spacing = 2.0 * radius + gap_cells;
    let mut index = SpatialIndex::new(spacing.max(1.0));
    for (i, a) in plan.iter().enumerate() {
        if a.x < 0 || a.y < 0 || a.x + v.w_cells > sh.rw || a.y + v.h_cells > sh.rh {
            return false;
        }
        for d in &sh.dots {
            let (dx, dy) = (a.x as f64 + cx - d.x, a.y as f64 + cy - d.y);
            let min_d = radius + d.radius;
            if dx * dx + dy * dy < min_d * min_d - 1e-6 {
                return false;
            }
        }
        let (ax, ay) = (a.x as f64, a.y as f64);
        for j in index.near(ax - spacing, ay - spacing, ax + spacing, ay + spacing) {
            let Some(b) = plan.get(j) else { continue };
            let (dx, dy) = ((a.x - b.x) as f64, (a.y - b.y) as f64);
            let min_d = (spacing - 1e-7).max(0.0);
            if dx * dx + dy * dy < min_d * min_d - 1e-6 {
                return false;
            }
        }
        index.insert(i, ax, ay, ax, ay);
    }
    true
}

// ---------------------------------------------------------------- bước vật lý (vị trí lẻ ô)

/// Khung thật (không làm tròn ô) của các con: (x0, y0, x1, y1) theo ô.
fn physical_bounds(plan: &[PSlot], vs: &[Variant], step: f64) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for s in plan {
        let Some(v) = vs.get(s.v) else { continue };
        b.0 = b.0.min(s.x);
        b.1 = b.1.min(s.y);
        b.2 = b.2.max(s.x + v.w / step);
        b.3 = b.3.max(s.y + v.h / step);
    }
    b
}

/// Dời nguyên khối về giữa vùng dàn thật; PON chặn thì tới gần giữa nhất có thể.
pub(super) fn centre_physical_plan(plan: &[PSlot], vs: &[Variant], area: (f64, f64), dots: &[Dot], step: f64, ck: &Clock) -> PPlan {
    if plan.is_empty() {
        return Vec::new();
    }
    let (rw, rh) = area;
    let (x0, y0, x1, y1) = physical_bounds(plan, vs, step);
    let want_x = (rw - x0 - x1) / 2.0;
    let want_y = (rh - y0 - y1) / 2.0;
    let safe_move = |tx: f64, ty: f64| -> bool {
        if x0 + tx < -1e-8 || y0 + ty < -1e-8 || x1 + tx > rw + 1e-8 || y1 + ty > rh + 1e-8 {
            return false;
        }
        ck.tick(1 + plan.len() * dots.len());
        for s in plan {
            let Some(v) = vs.get(s.v) else { return false };
            let (x, y) = ((s.x + tx) * step, (s.y + ty) * step);
            for dot in dots {
                let p = [dot.x * step - x, dot.y * step - y];
                let r = (dot.radius * step + v.curve_error - 0.001).max(0.0);
                let dx = (-p[0]).max(p[0] - v.w).max(0.0);
                let dy = (-p[1]).max(p[1] - v.h).max(0.0);
                if dx * dx + dy * dy >= r * r {
                    continue;
                }
                if inside_groups(p, &v.groups) {
                    return false;
                }
                for c in v.groups.iter().flatten() {
                    let n = c.len();
                    ck.tick(n);
                    for (i, a) in c.iter().enumerate() {
                        let Some(b) = c.get((i + 1) % n) else { continue };
                        if point_segment_distance_sq(p, *a, *b) < r * r {
                            return false;
                        }
                    }
                }
            }
        }
        true
    };
    let moved = |tx: f64, ty: f64| -> PPlan { plan.iter().map(|s| PSlot { x: s.x + tx, y: s.y + ty, ..*s }).collect() };
    if safe_move(want_x, want_y) {
        return moved(want_x, want_y);
    }
    // PON đặt lệch: giữ phép dời an toàn gần giữa nhất, không bỏ bớt khuôn.
    let mut tries = vec![(0.0, 0.0), (want_x, 0.0), (0.0, want_y)];
    for r in 1..=14 {
        let r = f64::from(r);
        tries.extend([
            (want_x - r, want_y),
            (want_x + r, want_y),
            (want_x, want_y - r),
            (want_x, want_y + r),
            (want_x - r, want_y - r),
            (want_x + r, want_y - r),
            (want_x - r, want_y + r),
            (want_x + r, want_y + r),
        ]);
    }
    let (mut bx, mut by) = (0.0, 0.0);
    let mut found = safe_move(0.0, 0.0);
    let mut best_score = if found { sq(want_x) + sq(want_y) } else { f64::INFINITY };
    for (tx, ty) in tries {
        let score = sq(tx - want_x) + sq(ty - want_y);
        if score >= best_score - 1e-10 || !safe_move(tx, ty) {
            continue;
        }
        bx = tx;
        by = ty;
        best_score = score;
        found = true;
    }
    if !found {
        return plan.to_vec();
    }
    // Tiến dần về vị trí lý tưởng, không nhận vị trí trung gian nào không an toàn.
    let (base_x, base_y) = (bx, by);
    let (mut low, mut high) = (0.0f64, 1.0f64);
    for _ in 0..20 {
        let mid = (low + high) / 2.0;
        let (tx, ty) = (base_x + (want_x - base_x) * mid, base_y + (want_y - base_y) * mid);
        if safe_move(tx, ty) {
            low = mid;
            bx = tx;
            by = ty;
        } else {
            high = mid;
        }
    }
    moved(bx, by)
}

/// Nới vị trí (không đổi hình) khi lưới chưa đạt khe liên tục vì sai số đường cong.
pub(super) fn open_physical_gap_plan(plan: &[PSlot], vs: &[Variant], area: (f64, f64), dots: &[Dot], gap: f64, step: f64, ck: &Clock) -> PPlan {
    if plan.len() < 2 {
        return plan.to_vec();
    }
    let (rw, rh) = area;
    let (mut x_lo, mut x_hi, mut y_lo, mut y_hi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for s in plan {
        x_lo = x_lo.min(s.x);
        x_hi = x_hi.max(s.x);
        y_lo = y_lo.min(s.y);
        y_hi = y_hi.max(s.y);
    }
    let (cx, cy) = ((x_lo + x_hi) / 2.0, (y_lo + y_hi) / 2.0);
    let max_shift = step.max(0.001);
    let mut shift = 0.001;
    while shift <= max_shift + 1e-9 {
        for axis in 0..3 {
            let fx = if axis != 2 && x_hi > x_lo { 1.0 + 2.0 * shift / ((x_hi - x_lo) * step) } else { 1.0 };
            let fy = if axis != 1 && y_hi > y_lo { 1.0 + 2.0 * shift / ((y_hi - y_lo) * step) } else { 1.0 };
            let mut trial: PPlan = plan.iter().map(|s| PSlot { x: cx + (s.x - cx) * fx, y: cy + (s.y - cy) * fy, ..*s }).collect();
            let (bx0, by0, bx1, by1) = physical_bounds(&trial, vs, step);
            if bx1 - bx0 > rw + 1e-8 || by1 - by0 > rh + 1e-8 {
                continue;
            }
            let tx = (-bx0).max((rw - bx1).min(0.0));
            let ty = (-by0).max((rh - by1).min(0.0));
            for s in &mut trial {
                s.x += tx;
                s.y += ty;
            }
            if verify_geometry(&trial, vs, area, dots, gap, step, ck) {
                return trial;
            }
            let trial = centre_physical_plan(&trial, vs, area, dots, step, ck);
            if verify_geometry(&trial, vs, area, dots, gap, step, ck) {
                return trial;
            }
        }
        shift = if shift < max_shift { max_shift.min(shift * 2.0) } else { max_shift + 1.0 };
    }
    plan.to_vec()
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    X,
    Y,
    Both,
}

/// Co vị trí (không bao giờ co hình) quanh tâm các vị trí tới khi khe liên tục vừa đúng khe nhập.
#[allow(clippy::too_many_arguments)]
fn close_physical_gap_plan(plan: &[PSlot], vs: &[Variant], area: (f64, f64), dots: &[Dot], gap: f64, step: f64, deadline: f64, ck: &Clock) -> PPlan {
    if plan.len() < 2 {
        return plan.to_vec();
    }
    let curve = vs.iter().map(|v| v.curve_error).fold(0.0, f64::max);
    // Dung sai kiểm tra 0,001 mm không được ăn vào khe dương đã nhập.
    let guard_gap = if gap > 0.0 && curve == 0.0 { gap + 0.001 } else { gap };
    let verify = |p: &[PSlot], dots: &[Dot]| verify_geometry(p, vs, area, dots, guard_gap, step, ck);
    let contracted = |base: &[PSlot], y_axis: bool, f: f64| -> PPlan {
        let get = |s: &PSlot| if y_axis { s.y } else { s.x };
        let (lo, hi) = base.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| (lo.min(get(s)), hi.max(get(s))));
        let centre = (lo + hi) / 2.0;
        base.iter()
            .map(|s| {
                let c = centre + (get(s) - centre) * f;
                if y_axis { PSlot { y: c, ..*s } } else { PSlot { x: c, ..*s } }
            })
            .collect()
    };
    let closed = |base: PPlan, axis: Axis, required: bool| -> PPlan {
        let at = |f: f64| -> PPlan {
            let out = contracted(&base, axis == Axis::Y, f);
            if axis == Axis::Both { contracted(&out, true, f) } else { out }
        };
        let (mut x_lo, mut x_hi, mut y_lo, mut y_hi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for s in &base {
            x_lo = x_lo.min(s.x);
            x_hi = x_hi.max(s.x);
            y_lo = y_lo.min(s.y);
            y_hi = y_hi.max(s.y);
        }
        let span = match axis {
            Axis::Both => (x_hi - x_lo).max(y_hi - y_lo),
            Axis::X => x_hi - x_lo,
            Axis::Y => y_hi - y_lo,
        } * step;
        if span <= 0.00005 {
            return base;
        }
        // Trục đã chạm khe thật thì không thử thêm.
        let trial = at((1.0 - 0.00005 / span).max(0.0));
        if !verify(&trial, &[]) {
            return base;
        }
        let mut kept = if verify(&trial, dots) { trial } else { base.clone() };
        let trial = at(0.0);
        if verify(&trial, dots) {
            return trial;
        }
        let (mut low, mut high) = (0.0f64, 1.0f64);
        for _ in 0..24 {
            if (high - low) * span <= 0.00005 || (!required && ck.now() >= deadline) {
                break;
            }
            let mid = (low + high) / 2.0;
            let trial = at(mid);
            if verify(&trial, dots) {
                high = mid;
                kept = trial;
            } else {
                low = mid;
            }
        }
        kept
    };
    let mut best = plan.to_vec();
    if !verify(&best, dots) {
        if gap > 0.0 {
            best = open_physical_gap_plan(&best, vs, area, dots, guard_gap, step, ck);
        }
        if !verify(&best, dots) {
            return plan.to_vec();
        }
    }
    // Khép đều trước để khe cùng cột / hàng cũng đúng; tờ thường phải khép xong các bước chính.
    let required = gap > 0.0 && plan.len() <= 400;
    if gap > 0.0 && (required || ck.now() < deadline) {
        best = closed(best, Axis::Both, required);
    }
    for pass in 0..2 {
        for axis in [Axis::X, Axis::Y] {
            if (!required || pass != 0) && ck.now() >= deadline {
                return best;
            }
            best = closed(best, axis, required && pass == 0);
        }
    }
    best
}

// ---------------------------------------------------------------- kiểm tra liên tục

/// Lưới ô vuông để tìm nhanh các phần tử gần một khung.
struct SpatialIndex {
    cell: f64,
    bins: HashMap<(i64, i64), Vec<usize>>,
    /// Phần tử phủ quá nhiều ô: luôn được trả.
    wide: Vec<usize>,
}

const MAX_BINS: i64 = 65_536;

impl SpatialIndex {
    fn new(cell: f64) -> Self {
        let cell = if cell.is_finite() && cell > 0.0 { cell } else { 1.0 };
        SpatialIndex { cell, bins: HashMap::new(), wide: Vec::new() }
    }
    fn range(&self, lo: f64, hi: f64) -> (i64, i64) {
        ((lo / self.cell).floor() as i64, (hi / self.cell).floor() as i64)
    }
    fn insert(&mut self, item: usize, x0: f64, y0: f64, x1: f64, y1: f64) {
        let ((bx0, bx1), (by0, by1)) = (self.range(x0, x1), self.range(y0, y1));
        if (bx1 - bx0 + 1).saturating_mul(by1 - by0 + 1) > MAX_BINS {
            self.wide.push(item);
            return;
        }
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                self.bins.entry((x, y)).or_default().push(item);
            }
        }
    }
    /// Các phần tử có thể gần khung (mỗi phần tử một lần, theo thứ tự tăng).
    fn near(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<usize> {
        let ((bx0, bx1), (by0, by1)) = (self.range(x0, x1), self.range(y0, y1));
        let mut out: Vec<usize> = self.wide.clone();
        if (bx1 - bx0 + 1).saturating_mul(by1 - by0 + 1) > MAX_BINS {
            out.extend(self.bins.values().flatten().copied());
        } else {
            for y in by0..=by1 {
                for x in bx0..=bx1 {
                    if let Some(items) = self.bins.get(&(x, y)) {
                        out.extend(items.iter().copied());
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

pub(super) fn point_segment_distance_sq(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let den = dx * dx + dy * dy;
    let t = if den != 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / den).clamp(0.0, 1.0) } else { 0.0 };
    sq(p[0] - a[0] - t * dx) + sq(p[1] - a[1] - t * dy)
}

/// Hai đoạn cắt nhau thật sự (không tính chạm đầu mút).
fn segments_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let cross = |p: Pt, q: Pt, r: Pt| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (x, y, u, w) = (cross(a, b, c), cross(a, b, d), cross(c, d, a), cross(c, d, b));
    ((x > 1e-9 && y < -1e-9) || (x < -1e-9 && y > 1e-9)) && ((u > 1e-9 && w < -1e-9) || (u < -1e-9 && w > 1e-9))
}

fn ray_crosses(p: Pt, a: &Pt, b: &Pt) -> bool {
    (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
}

/// Trong vùng tô (even-odd từng nhóm, hợp các nhóm).
pub(super) fn inside_groups(p: Pt, groups: &[Group]) -> bool {
    for g in groups {
        let mut inside = false;
        for c in g {
            let n = c.len();
            for (i, a) in c.iter().enumerate() {
                if let Some(b) = c.get((i + 1) % n)
                    && ray_crosses(p, a, b)
                {
                    inside = !inside;
                }
            }
        }
        if inside {
            return true;
        }
    }
    false
}

/// Hẳn bên trong (nằm trên biên của một nhóm thì nhóm đó không tính).
fn strict_inside_groups(p: Pt, groups: &[Group]) -> bool {
    for g in groups {
        let (mut inside, mut boundary) = (false, false);
        for c in g {
            let n = c.len();
            for (i, a) in c.iter().enumerate() {
                let Some(b) = c.get((i + 1) % n) else { continue };
                if point_segment_distance_sq(p, *a, *b) <= 1e-18 {
                    boundary = true;
                }
                if ray_crosses(p, a, b) {
                    inside = !inside;
                }
            }
        }
        if inside && !boundary {
            return true;
        }
    }
    false
}

struct Edge {
    a: Pt,
    b: Pt,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

struct Shape {
    groups: Groups,
    edges: Vec<Edge>,
    bounds: Bounds,
    outer: f64,
    cx: f64,
    cy: f64,
    edge_index: Option<SpatialIndex>,
}

fn edges_clash(e: &Edge, f: &Edge, gap: f64, gap_sq: f64) -> bool {
    if e.x0 >= f.x1 + gap || f.x0 >= e.x1 + gap || e.y0 >= f.y1 + gap || f.y0 >= e.y1 + gap {
        return false;
    }
    segments_cross(e.a, e.b, f.a, f.b)
        || point_segment_distance_sq(e.a, f.a, f.b) < gap_sq
        || point_segment_distance_sq(e.b, f.a, f.b) < gap_sq
        || point_segment_distance_sq(f.a, e.a, e.b) < gap_sq
        || point_segment_distance_sq(f.b, e.a, e.b) < gap_sq
}

/// Khe 0: chạm biên được, chồng vùng tô thì không.
fn filled_shapes_overlap(a: &Shape, b: &Shape) -> bool {
    let overlap = (a.bounds.x1.min(b.bounds.x1) - a.bounds.x0.max(b.bounds.x0)).min(a.bounds.y1.min(b.bounds.y1) - a.bounds.y0.max(b.bounds.y0));
    let strict_in = |p: Pt, s: &Shape| -> bool {
        let bb = s.bounds;
        if p[0] <= bb.x0 || p[0] >= bb.x1 || p[1] <= bb.y0 || p[1] >= bb.y1 {
            return false;
        }
        strict_inside_groups(p, &s.groups)
    };
    let shared = |p: Pt| strict_in(p, a) && strict_in(p, b);
    let dirs: [(f64, f64); 8] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];
    let probe = |shape: &Shape, other: &Shape| -> bool {
        let scale = shape.bounds.w.min(shape.bounds.h);
        for e in &shape.edges {
            let (dx, dy) = (e.b[0] - e.a[0], e.b[1] - e.a[1]);
            let length = (dx * dx + dy * dy).sqrt();
            if length <= 1e-12 {
                continue;
            }
            let eps = 1e-6f64.min(length * 1e-4).min(scale * 1e-4).min(overlap / 4.0);
            let mid = [(e.a[0] + e.b[0]) / 2.0, (e.a[1] + e.b[1]) / 2.0];
            for p in [e.a, mid] {
                if !strict_in(p, other) {
                    continue;
                }
                if strict_in(p, shape) {
                    return true;
                }
                if dirs.iter().any(|&(ux, uy)| shared([p[0] + ux * eps, p[1] + uy * eps])) {
                    return true;
                }
            }
            // Cạnh chung có hai hình ở hai phía thì không có điểm nào trong cả hai vùng tô.
            for side in [-1.0, 1.0] {
                let q = [mid[0] - side * dy / length * eps, mid[1] + side * dx / length * eps];
                if shared(q) {
                    return true;
                }
            }
        }
        false
    };
    probe(a, b) || probe(b, a)
}

/// Kiểm tra độc lập trên đường bao đã làm phẳng: mọi cặp cách nhau ít nhất `gap` (dung sai
/// 0,001 mm; khuôn cong thêm 2 × sai số + 0,001), không cắt / không lọt nhau, trong vùng dàn, né
/// PON tính từ mép chấm (sai số đường cong của chính khuôn đó). `area` = vùng dàn (ô).
pub(super) fn verify_geometry(plan: &[PSlot], vs: &[Variant], area: (f64, f64), dots: &[Dot], gap: f64, step: f64, ck: &Clock) -> bool {
    let (rw, rh) = area;
    let tol = 0.001;
    let curve = vs.iter().map(|v| v.curve_error).fold(0.0, f64::max);
    let gap = if curve != 0.0 { gap + 2.0 * curve + tol } else { gap };
    let physical_gap = (gap - tol).max(0.0);
    let gap_sq = physical_gap * physical_gap;
    let outer: Vec<f64> = vs
        .iter()
        .map(|v| v.groups.iter().flatten().flatten().map(|p| sq(p[0] - v.w / 2.0) + sq(p[1] - v.h / 2.0)).fold(0.0, f64::max).sqrt())
        .collect();
    let mut index = SpatialIndex::new(8f64.max(gap * 2.0));
    let mut mapped: Vec<Shape> = Vec::with_capacity(plan.len());
    for (i, s) in plan.iter().enumerate() {
        let Some(v) = vs.get(s.v) else { return false };
        let (ox, oy) = (s.x * step, s.y * step);
        let mut groups = copy_groups(&v.groups);
        let mut edges = Vec::new();
        for c in groups.iter_mut().flatten() {
            for p in c.iter_mut() {
                p[0] += ox;
                p[1] += oy;
            }
            let n = c.len();
            for (pi, p) in c.iter().enumerate() {
                let Some(q) = c.get((pi + 1) % n) else { continue };
                if p[0] < -tol || p[1] < -tol || p[0] > rw * step + tol || p[1] > rh * step + tol {
                    return false;
                }
                edges.push(Edge { a: *p, b: *q, x0: p[0].min(q[0]), x1: p[0].max(q[0]), y0: p[1].min(q[1]), y1: p[1].max(q[1]) });
            }
        }
        ck.tick(edges.len());
        let Some(bb) = bounds_of_groups(&groups) else { return false };
        let shape = Shape {
            groups,
            edges,
            bounds: bb,
            outer: outer.get(s.v).copied().unwrap_or(0.0),
            cx: ox + v.w / 2.0,
            cy: oy + v.h / 2.0,
            edge_index: None,
        };
        for d in dots {
            let centre = [d.x * step, d.y * step];
            // Sai số đường cong của chính khuôn này, như lúc canh tâm (bản gốc lấy lớn nhất của mọi khuôn).
            let rr = (d.radius * step + v.curve_error - tol).max(0.0);
            let r_sq = rr * rr;
            if inside_groups(centre, &shape.groups) {
                return false;
            }
            if shape.edges.iter().any(|e| point_segment_distance_sq(centre, e.a, e.b) < r_sq) {
                return false;
            }
            ck.tick(shape.edges.len());
        }
        for j in index.near(bb.x0 - gap, bb.y0 - gap, bb.x1 + gap, bb.y1 + gap) {
            let Some(other) = mapped.get_mut(j) else { continue };
            let ob = other.bounds;
            if bb.x0 >= ob.x1 + gap || ob.x0 >= bb.x1 + gap || bb.y0 >= ob.y1 + gap || ob.y0 >= bb.y1 + gap {
                continue;
            }
            if sq(shape.cx - other.cx) + sq(shape.cy - other.cy) >= sq(shape.outer + other.outer + physical_gap) + 1e-10 {
                continue;
            }
            if other.edge_index.is_none() && other.edges.len() > 160 {
                let mut ix = SpatialIndex::new(1f64.max(gap * 2.0));
                for (k, e) in other.edges.iter().enumerate() {
                    ix.insert(k, e.x0, e.y0, e.x1, e.y1);
                }
                other.edge_index = Some(ix);
            }
            let other: &Shape = other;
            for e in &shape.edges {
                let clash = match &other.edge_index {
                    Some(ix) => {
                        let near = ix.near(e.x0 - gap, e.y0 - gap, e.x1 + gap, e.y1 + gap);
                        ck.tick(near.len());
                        near.into_iter().any(|k| other.edges.get(k).is_some_and(|f| edges_clash(e, f, gap, gap_sq)))
                    }
                    None => {
                        ck.tick(other.edges.len());
                        other.edges.iter().any(|f| edges_clash(e, f, gap, gap_sq))
                    }
                };
                if clash {
                    return false;
                }
            }
            if gap > tol {
                // Khe dương đã loại biên chung; chỉ còn trường hợp lọt hẳn vào trong.
                for c in shape.groups.iter().flatten() {
                    if let Some(p) = c.first()
                        && inside_groups(*p, &other.groups)
                    {
                        return false;
                    }
                }
                for c in other.groups.iter().flatten() {
                    if let Some(p) = c.first()
                        && inside_groups(*p, &shape.groups)
                    {
                        return false;
                    }
                }
            } else if filled_shapes_overlap(&shape, other) {
                return false;
            }
        }
        index.insert(i, bb.x0, bb.y0, bb.x1, bb.y1);
        mapped.push(shape);
    }
    true
}

// ---------------------------------------------------------------- nest

/// Một mẫu khuôn: đường bao mm (y hướng lên, khung bắt đầu ở 0) và sai số làm phẳng (mm).
#[derive(Clone, Debug)]
pub(super) struct TypeIn {
    pub(super) groups: Groups,
    pub(super) curve_error: f64,
}

/// Chấm PON: tâm (mm, từ mép trái / mép dưới tờ) và bán kính chấm.
#[derive(Clone, Copy, Debug)]
pub(super) struct DotIn {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) r: f64,
}

/// Đầu vào của một lần dàn (như payload `OKJSON` của DanCard).
#[derive(Clone, Debug)]
pub(super) struct NestInput {
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) dots: Vec<DotIn>,
    pub(super) types: Vec<TypeIn>,
    pub(super) gap: f64,
    pub(super) margin: f64,
    pub(super) pon_clear: f64,
    pub(super) step: f64,
    pub(super) budget_ms: f64,
}

/// Một con: góc dưới trái khung khuôn đã xoay (mm, từ mép trái / mép dưới tờ), hướng `vi`
/// (góc [`ANGLES`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct OutSlot {
    pub(super) mi: usize,
    pub(super) vi: usize,
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Clone, Debug)]
pub(super) struct NestOut {
    pub(super) slots: Vec<OutSlot>,
    pub(super) mode: String,
    pub(super) counts: Vec<usize>,
    /// Khối nằm đúng giữa vùng dàn.
    pub(super) exact: bool,
    /// Tâm khối lệch tâm vùng dàn (mm).
    pub(super) offset: (f64, f64),
    pub(super) detail: String,
}

enum Attempt {
    Done(NestOut),
    Retry(f64),
}

/// Dàn tối đa số con lên một tờ. Lỗi là thông báo tiếng Việt như DanCard.
pub(super) fn nest(input: &NestInput, ck: &Clock) -> Result<NestOut, String> {
    let mut pad = 0.0;
    for _ in 0..3 {
        match nest_once(input, pad, ck)? {
            Attempt::Done(out) => return Ok(out),
            Attempt::Retry(next) => pad = next,
        }
    }
    Err("Bộ kiểm tra an toàn từ chối phương án canh tâm.".into())
}

fn nest_once(input: &NestInput, planning_pad: f64, ck: &Clock) -> Result<Attempt, String> {
    let began = ck.now();
    let work0 = ck.work();
    if input.types.is_empty() {
        return Err("Thiếu dữ liệu khuôn bế.".into());
    }
    let step = if input.step > 0.0 && input.step <= 1.0 { input.step } else { 0.25 };
    let planning_pad = if (0.0..=2.0 * step).contains(&planning_pad) { planning_pad } else { 0.0 };
    let (gap, margin, pon_clear) = (input.gap, input.margin, input.pon_clear);
    let valid = gap >= 0.0 && margin >= 0.0 && pon_clear >= 0.0 && gap.is_finite() && margin.is_finite() && pon_clear.is_finite();
    if !valid {
        return Err("Thông số khe/lề/PON không hợp lệ.".into());
    }
    let rw_f = ((input.width - 2.0 * margin) / step + 1e-7).floor();
    let rh_f = ((input.height - 2.0 * margin) / step + 1e-7).floor();
    let positive = rw_f > 0.0 && rh_f > 0.0;
    if !positive {
        return Err("Khổ tờ nhỏ hơn phần lề đã nhập.".into());
    }
    if rw_f > MAX_CELLS as f64 || rh_f > MAX_CELLS as f64 {
        return Err(format!("Khổ tờ quá lớn cho lưới kiểm tra {step:.2} mm: vùng dàn tối đa {:.0} cm mỗi chiều.", MAX_CELLS as f64 * step / 10.0));
    }
    let (rw, rh) = (rw_f as i64, rh_f as i64);
    let n_types = input.types.len();
    let mut variants: Vec<Variant> = Vec::with_capacity(4 * n_types);
    let mut by_type: Vec<Groups> = Vec::with_capacity(n_types);
    for (ti, t) in input.types.iter().enumerate() {
        let no_fit = || format!("Khuôn loại {} không vừa khổ tờ sau khi trừ lề, ở cả 4 hướng xoay.", ti + 1);
        let g = normalize_groups(&t.groups).ok_or_else(|| format!("Không đọc được đường bế kín của loại {}.", ti + 1))?;
        // Khuôn lớn hơn cả vùng dàn ở mọi hướng: khỏi raster.
        if let Some(b) = bounds_of_groups(&g)
            && b.w.max(b.h) / step > (rw.max(rh) + 1) as f64
        {
            return Err(no_fit());
        }
        let ce = if t.curve_error.is_finite() && t.curve_error >= 0.0 { t.curve_error.min(0.025) } else { 0.0 };
        let mut fits = false;
        for (ri, &angle) in ANGLES.iter().enumerate() {
            if let Some(mut v) = build_variant(&g, ti, ri, angle, gap + planning_pad, step, ck) {
                v.curve_error = ce;
                fits |= v.w_cells <= rw && v.h_cells <= rh;
                variants.push(v);
            }
        }
        if !fits {
            return Err(no_fit());
        }
        by_type.push(g);
    }
    if variants.len() != 4 * n_types {
        return Err("Không tạo được biến thể xoay của khuôn.".into());
    }
    // Nhiều bài dùng chung một khuôn: tìm một lần rồi chia xoay vòng.
    let equivalent = n_types > 1 && by_type.first().is_some_and(|f| by_type.iter().skip(1).all(|g| same_groups(f, g)));
    let planning: &[Variant] = if equivalent { variants.get(..4).unwrap_or(&variants) } else { &variants };
    let tc = if equivalent { 1 } else { n_types };
    let dots: Vec<Dot> =
        input.dots.iter().map(|d| Dot { x: (d.x - margin) / step, y: (d.y - margin) / step, radius: (d.r + pon_clear) / step }).collect();
    let sheet = Sheet { rw, rh, dots };
    let budget = (if input.budget_ms.is_finite() && input.budget_ms != 0.0 { input.budget_ms } else { 1450.0 }).clamp(250.0, 4000.0);
    let deadline = began + budget;
    let mut best: Option<Plan> = None;
    let mut mode = String::from("silhouette nhiều khoảng + tiếp xúc thật");

    // Khe 0: lát lấy mẫu có thể che một chồng lấn nhỏ hơn ô; chỉ nhận bố cục qua kiểm tra liên tục.
    let memo: RefCell<HashMap<Plan, bool>> = RefCell::new(HashMap::new());
    let safe = |plan: &[Slot]| -> bool {
        if gap != 0.0 {
            return true;
        }
        if let Ok(m) = memo.try_borrow()
            && let Some(&v) = m.get(plan)
        {
            return v;
        }
        let ok = verify_geometry(&to_physical(plan), planning, (rw as f64, rh as f64), &sheet.dots, 0.0, step, ck);
        if let Ok(mut m) = memo.try_borrow_mut() {
            m.insert(plan.to_vec(), ok);
        }
        ok
    };
    let safe_growth = |trial: Plan, seed: &[Slot]| -> Plan {
        if gap != 0.0 || safe(&trial) {
            return trial;
        }
        // Giữ từng con thêm vào mà an toàn; thứ tự bị xáo thì giữ seed.
        if !seed.iter().enumerate().all(|(k, b)| trial.get(k) == Some(b)) {
            return seed.to_vec();
        }
        let mut recovered = seed.to_vec();
        for s in trial.iter().skip(seed.len()) {
            if ck.now() >= deadline {
                break;
            }
            if !allowed(s.mi, &count_types(&recovered, tc), tc) {
                continue;
            }
            let mut appended = recovered.clone();
            appended.push(*s);
            if safe(&appended) {
                recovered = appended;
            }
        }
        recovered
    };

    let round = if tc == 1 { by_type.first().and_then(|g| circle_like(g.as_slice())) } else { None };
    if round.is_some()
        && let Some(v0) = variants.first()
    {
        let c = circle_plan(v0, &sheet, gap, deadline, ck);
        if !c.is_empty() {
            best = Some(c);
        }
        // Tờ chữ nhật có thể chứa nhiều hơn khi so le theo cột: thử cả chiều xoay ngang.
        let turned_sheet = Sheet { rw: rh, rh: rw, dots: sheet.dots.iter().map(|d| Dot { x: d.y, y: d.x, radius: d.radius }).collect() };
        let mut turned = circle_plan(v0, &turned_sheet, gap, deadline, ck);
        for s in &mut turned {
            std::mem::swap(&mut s.x, &mut s.y);
        }
        if !turned.is_empty() && compare_plans(&turned, best.as_deref(), &variants) > 0 {
            best = Some(turned);
        }
        mode = "lục giác động 2 hướng + silhouette kiểm tra thật".into();
    }

    let mut seeds: Vec<Plan> = Vec::new();
    if round.is_none() {
        let sub = |f: f64| deadline.min(ck.now() + budget * f);
        let composed = compositional_band_plans(planning, tc, &sheet, gap, step, sub(0.2), ck);
        let mixed = mixed_row_plans(planning, tc, &sheet, sub(0.08), ck);
        let alternating = alternating_row_plans(planning, tc, &sheet, sub(0.12), ck);
        let bands = band_plans(planning, tc, &sheet, gap, step, sub(0.15), ck);
        let lattice = lattice_plans(planning, tc, &sheet, sub(0.2), ck);
        seeds = seed_plans(planning, tc, &sheet, sub(0.35), ck);
        seeds.extend(lattice);
        seeds.extend(alternating);
        seeds.extend(bands);
        seeds.extend(mixed);
        seeds.extend(composed);
        seeds.sort_by(|u, w| best_first(u, w, planning));
    }
    // Mọi seed hợp lệ là phương án dự phòng, kể cả khi hết ngân sách.
    for s in &seeds {
        if balanced(s, tc) && compare_plans(s, best.as_deref(), planning) > 0 && safe(s) {
            best = Some(s.clone());
        }
    }
    for s in &seeds {
        if ck.now() >= deadline {
            break;
        }
        if !safe(s) {
            continue;
        }
        let slice = deadline.min(ck.now() + 650f64.max(budget * 0.25));
        let mut c = s.clone();
        if c.len() >= 6 && ck.now() < slice {
            c = augment_with_pair(&c, planning, tc, &sheet, slice, ck);
        }
        c = grow_plan(&c, planning, tc, &sheet, slice, ck);
        c = safe_growth(c, s);
        if balanced(&c, tc) && compare_plans(&c, best.as_deref(), planning) > 0 {
            best = Some(c);
        }
    }
    let mut best = best.unwrap_or_default();
    if round.is_none() && ck.now() < deadline && !best.is_empty() {
        let c = repair_plans(&best, planning, tc, &sheet, deadline, ck);
        if balanced(&c, tc) && safe(&c) {
            best = c;
        }
    }
    let c = centre_plan(&best, &variants, &sheet, ck);
    if safe(&c) {
        best = c;
    }
    if round.is_none() {
        let c = polish_edges(&best, planning, &sheet, gap, step, ck.now() + 550f64.min(budget * 0.2), ck);
        if safe(&c) {
            best = c;
        }
    }
    if equivalent {
        for (k, s) in best.iter_mut().enumerate() {
            let vi = variants.get(s.v).map_or(0, |v| v.vi);
            s.mi = k % n_types;
            s.v = s.mi * 4 + vi;
        }
        mode.push_str(" + luân phiên các bài cùng khuôn");
    }
    let valid = match (round.is_some(), best.is_empty(), variants.first()) {
        (true, false, Some(v0)) => verify_circle_plan(&best, v0, &sheet, gap / step),
        _ => verify_plan(&best, &variants, &sheet, ck),
    };
    let accepted = valid && balanced(&best, n_types);
    if !accepted {
        return Err("Bộ kiểm tra an toàn từ chối phương án dàn.".into());
    }

    let p_area = ((input.width - 2.0 * margin) / step, (input.height - 2.0 * margin) / step);
    let close_deadline = ck.now() + if best.len() <= 400 { 5000.0 } else { 2000f64.min(750f64.max(budget * 0.5)) };
    let closed = close_physical_gap_plan(&to_physical(&best), &variants, p_area, &sheet.dots, gap, step, close_deadline, ck);
    let placed = centre_physical_plan(&closed, &variants, p_area, &sheet.dots, step, ck);
    if !verify_geometry(&placed, &variants, p_area, &sheet.dots, gap, step, ck) {
        // Chỉ tìm lại với lưới thận trọng hơn (tối đa hai lần); không nới khe, không bỏ kiểm tra.
        if gap > 0.0 && planning_pad < 2.0 * step - 1e-8 {
            return Ok(Attempt::Retry((2.0 * step).min(planning_pad + step)));
        }
        return Err("Bộ kiểm tra an toàn từ chối phương án canh tâm.".into());
    }
    let (x0, y0, x1, y1) = physical_bounds(&placed, &variants, step);
    let offset = if placed.is_empty() { (0.0, 0.0) } else { ((x0 + x1 - p_area.0) * step / 2.0, (y0 + y1 - p_area.1) * step / 2.0) };
    let exact = offset.0.abs() <= 0.001 && offset.1.abs() <= 0.001;
    let slots: Vec<OutSlot> = placed
        .iter()
        .filter_map(|s| {
            let v = variants.get(s.v)?;
            Some(OutSlot { mi: s.mi, vi: v.vi, x: margin + s.x * step, y: margin + s.y * step })
        })
        .collect();
    let mut counts = vec![0usize; n_types];
    for s in &slots {
        if let Some(c) = counts.get_mut(s.mi) {
            *c += 1;
        }
    }
    let count_text = counts.iter().enumerate().map(|(i, c)| format!("loại {}: {c}", i + 1)).collect::<Vec<_>>().join(", ");
    let thousands = ck.work().saturating_sub(work0) / 1000;
    let detail = format!(
        "Dàn silhouette thật: {} con ({count_text}). Lưới kiểm tra {step:.2} mm, khe {gap:.2} mm; đã thử xoay/lồng biên dạng ({thousands} nghìn phép thử).{}",
        slots.len(),
        if exact { "" } else { " Canh tâm bị giới hạn bởi vùng né PON." }
    );
    Ok(Attempt::Done(NestOut { slots, mode, counts, exact, offset, detail }))
}
