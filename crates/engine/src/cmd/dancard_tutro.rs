//! DanCard — Dàn Offset tự trở (tab Dàn tối ưu), cùng quy tắc `dcDanTuTro` / `dcDanTuTroNhieuKho`.
//!
//! Chọn từng cặp: trái = mặt trước, phải = mặt sau (thứ tự đọc: hàng trên trước, trong hàng trái →
//! phải; cặp là hai object liền nhau theo thứ tự đó). Tự trở: hai mặt chung một tờ, nửa trái là mặt
//! trước, nửa phải là ảnh gương của từng chỗ qua tim dọc tờ giấy (chỉ vị trí đối xứng, bài không lật;
//! mặt trước xoay 90° thì mặt sau xoay −90°). In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm
//! đó, cắt ra là mỗi con đủ 2 mặt. Không khe giữa các con, không khe ở tim giấy.
//!
//! - Mọi cặp cùng khổ (lệch không quá 0,5 mm so với cặp đầu): bộ tính guillotine một khổ trên nửa tờ,
//!   tự thử xoay 90°, tự bóp tối đa "Bóp tối đa" mm mỗi chiều (không quá 20 % cạnh ngắn) khi nhờ vậy
//!   dàn thêm được con. Các chỗ dùng tới được dồn sát tim giấy, cụm được canh giữa tờ.
//! - Khổ lẫn lộn: bộ xếp guillotine nhiều khổ, mỗi cặp một chỗ, ưu tiên dàn được nhiều cặp nhất rồi
//!   bóp ít nhất; cụm sát lề trên và sát tim giấy.
//!
//! Tờ mới là một artboard đúng khổ giấy, đặt ở chỗ trống bên phải mọi bài và artboard có sẵn; dựng
//! xong thì các artboard cũ bị xoá khung (bài giữ nguyên), như DanCard. Mặt trước, mặt sau và PON
//! giấy (4 góc artboard) nằm trên ba layer mới, layer PON khóa lại. Cặp đã dàn bị xoá, cặp chưa vừa
//! giữ nguyên chỗ cũ để dàn tiếp vào tờ sau. Cả lần dàn là một bước hoàn tác.
//!
//! Bộ tính viết theo toạ độ của DanCard (y hướng lên, tờ giấy là [0, cao, rộng, 0], mm của DanCard)
//! để ra đúng các vector đối chiếu; chỉ lúc dựng mới đổi sang trang của VectorCraft (y hướng xuống).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Artboard, Document, Node, NodeId, Scaling};
use vectorcraft_geom::{Affine, Point, Rect, Vec2, shapes};

use super::roots_in_paint_order;
use crate::cmd::*;
use crate::{EngineError, Result, Session};

/// 1 mm (pt), đúng hằng số DanCard dùng (các vector đối chiếu tính bằng nó).
const MM: f64 = 2.834645669;
/// Sai số hình học của DanCard (pt; bộ đọc số dùng nó theo đơn vị của ô nhập).
const EPS: f64 = 0.01;
/// Hai mặt của một cặp, hay hai cặp, coi là cùng khổ khi lệch không quá 0,5 mm.
const SIZE_TOLERANCE: f64 = 0.5 * MM;
/// Khoảng cách từ bài và artboard có sẵn tới tờ mới.
const PAGE_GAP: f64 = 10.0 * MM;
/// Nhiều chỗ nhất trên một nửa tờ (mỗi chỗ là 2 bản: trước + sau).
const MAX_SLOTS: f64 = 20_000.0;
/// Nhiều dải nhất bộ tính một khổ thử theo mỗi chiều.
const MAX_LINES: f64 = 1_000_000.0;
/// Nhiều cặp nhất bộ xếp nhiều khổ nhận một lần.
const MAX_MIXED_PAIRS: usize = 200;
const FRONT_LAYER: &str = "Dàn tự trở - Mặt trước";
const BACK_LAYER: &str = "Dàn tự trở - Mặt sau";
const PON_LAYER: &str = "PON giấy Dàn Offset";
/// PON giấy: nét dài 3 mm, dày 1,995 pt, K 100.
const PON_LENGTH: f64 = 3.0 * MM;
const PON_STROKE: f64 = 1.995;
const NOT_FIT_SINGLE: &str = "Bài không nằm vừa nửa tờ giấy. Tự trở cần mỗi mặt lọt vào một nửa khổ, hãy tăng khổ giấy hoặc giảm lề an toàn.";
const NOT_FIT_MIXED: &str = "Không có cặp nào nằm vừa nửa tờ giấy. Tự trở cần mỗi mặt lọt vào một nửa khổ; hãy tăng khổ giấy hoặc giảm lề an toàn.";
const NO_COPY_BOUNDS: &str = "Không đo được bản sao để đặt vào tờ giấy.";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.tuTro",
        "DanCard: Dàn Offset tự trở",
        [],
        None,
        "{width, height: cm (khổ giấy: số, hoặc chuỗi như \"86\", \"8,6\"), margin?: mm (lề an toàn mỗi mép, mặc định 0, tối đa 50), maxSqueeze?: mm (Bóp tối đa mỗi chiều, mặc định 0, tối đa 50, không quá 20 % cạnh ngắn), repeatFill?: bool (lặp lại các cặp cho đầy tờ; khổ lẫn lộn thì bỏ qua)} dàn tự trở các cặp đang chọn (theo thứ tự đọc, trái = mặt trước, phải = mặt sau) lên một tờ mới (artboard đúng khổ giấy, bên phải mọi bài; các artboard cũ bị xoá khung): nửa trái mặt trước, nửa phải mặt sau đối xứng qua tim giấy, tự xoay 90°, tự bóp khi dàn thêm được con, khổ lẫn lộn thì xếp guillotine nhiều khổ; PON giấy 4 góc; cặp đã dàn bị xoá, cặp chưa vừa giữ nguyên; một bước hoàn tác → {message (\"OK:[[COUNT:n]] …\"), count (số chỗ mỗi nửa; nhiều khổ: số cặp đã dàn), placed, leftover, mixed, squeezeMm: [ngang, dọc], artboard, ids}",
        has_doc,
        tu_tro
    )]
}

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

// ---------------------------------------------------------------- số kiểu JavaScript

/// `Math.round` của JavaScript (nửa làm tròn lên).
fn js_round(v: f64) -> f64 {
    let f = v.floor();
    if v - f >= 0.5 { f + 1.0 } else { f }
}

/// `String(Math.round(v * 100) / 100)`, như DanCard in số.
fn round2(v: f64) -> String {
    (js_round(v * 100.0) / 100.0).to_string()
}

fn cm(pt: f64) -> String {
    round2(pt / (10.0 * MM))
}

fn mm(pt: f64) -> String {
    round2(pt / MM)
}

/// `Array.prototype.sort` ổn định với hàm so sánh trả số (dương: `b` đứng trước `a`). Sắp kiểu
/// trộn, không bao giờ panic kể cả khi hàm so sánh không bắc cầu (các hàm so sánh có dung sai của
/// DanCard).
fn js_sorted<T: Clone>(v: Vec<T>, cmp: impl Fn(&T, &T) -> f64) -> Vec<T> {
    let n = v.len();
    let mut src = v;
    let mut dst: Vec<T> = Vec::with_capacity(n);
    let mut width = 1usize;
    while width < n {
        dst.clear();
        let mut start = 0;
        while start < n {
            let mid = start.saturating_add(width).min(n);
            let end = mid.saturating_add(width).min(n);
            let (mut i, mut j) = (start, mid);
            while i < mid && j < end {
                let (Some(a), Some(b)) = (src.get(i), src.get(j)) else { break };
                if cmp(a, b) > 0.0 {
                    dst.push(b.clone());
                    j += 1;
                } else {
                    dst.push(a.clone());
                    i += 1;
                }
            }
            dst.extend(src.get(i..mid).into_iter().flatten().cloned());
            dst.extend(src.get(j..end).into_iter().flatten().cloned());
            start = end;
        }
        std::mem::swap(&mut src, &mut dst);
        width = width.saturating_mul(2);
    }
    src
}

fn digits_end(b: &[u8], from: usize) -> usize {
    let mut i = from;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    i
}

/// `parseFloat` của JavaScript: khoảng trắng đầu, dấu, rồi phần đầu dài nhất đọc được thành số
/// ("86cm" → 86, "1.5,2" → 1.5); không đọc được thì NaN.
fn js_parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", t.strip_prefix('+').unwrap_or(t)),
    };
    if rest.starts_with("Infinity") {
        return if sign.is_empty() { f64::INFINITY } else { f64::NEG_INFINITY };
    }
    let b = rest.as_bytes();
    let int_end = digits_end(b, 0);
    let mut end = int_end;
    let mut frac = "";
    if b.get(int_end) == Some(&b'.') {
        let frac_end = digits_end(b, int_end + 1);
        if int_end > 0 || frac_end > int_end + 1 {
            frac = rest.get(int_end + 1..frac_end).unwrap_or("");
            end = frac_end;
        }
    }
    if end == 0 {
        return f64::NAN;
    }
    let mut exp = "";
    if matches!(b.get(end), Some(b'e' | b'E')) {
        let mut j = end + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let k = digits_end(b, j);
        if k > j {
            exp = rest.get(end..k).unwrap_or("");
        }
    }
    let int = rest.get(..int_end).unwrap_or("");
    let int = if int.is_empty() { "0" } else { int };
    let frac = if frac.is_empty() { "0" } else { frac };
    format!("{sign}{int}.{frac}{exp}").parse().unwrap_or(f64::NAN)
}

/// `parseNumberText` của DanCard: dấu phẩy đầu tiên thành dấu chấm, `parseFloat`, rồi kiểm tra
/// khoảng (dung sai `EPS` theo đơn vị của ô). `blank_zero`: ô trống (hay thiếu) là 0.
fn number_text(v: Option<&Value>, label: &str, min: f64, max: Option<f64>, blank_zero: bool) -> std::result::Result<f64, String> {
    let n = match v {
        None | Some(Value::Null) if blank_zero => 0.0,
        Some(Value::String(s)) if blank_zero && s.is_empty() => 0.0,
        Some(Value::Number(x)) => x.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => js_parse_float(&s.replacen(',', ".", 1)),
        _ => f64::NAN,
    };
    if !n.is_finite() {
        return Err(format!("{label} phải là số."));
    }
    if n < min - EPS {
        return Err(format!("{label} không được nhỏ hơn {min}."));
    }
    if let Some(max) = max
        && n > max + EPS
    {
        return Err(format!("{label} không được lớn hơn {max}."));
    }
    Ok(n)
}

/// Tờ giấy đã đọc từ tham số.
#[derive(Clone, Copy, Debug)]
struct Paper {
    /// Khổ giấy (pt).
    w: f64,
    h: f64,
    margin_mm: f64,
    max_bop_mm: f64,
    repeat: bool,
}

fn parse_paper(p: &Value) -> std::result::Result<Paper, String> {
    let w = number_text(p.get("width"), "Ngang giấy", 0.1, None, false)? * 10.0 * MM;
    let h = number_text(p.get("height"), "Dọc giấy", 0.1, None, false)? * 10.0 * MM;
    let margin_mm = number_text(p.get("margin"), "Lề an toàn", 0.0, Some(50.0), true)?;
    let max_bop_mm = number_text(p.get("maxSqueeze"), "Bóp tối đa", 0.0, Some(50.0), true)?;
    let repeat = match p.get("repeatFill") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    };
    Ok(Paper { w, h, margin_mm, max_bop_mm, repeat })
}

// ---------------------------------------------------------------- chỗ đặt

/// Một chỗ trên tờ: góc trên trái (`x`, `y`) theo toạ độ DanCard (y hướng lên), khung `w` × `h` đã
/// xoay theo `angle` (0, 90, hay −90 cho mặt sau), khổ bài khi chưa xoay `base_w` × `base_h`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    angle: i32,
    base_w: f64,
    base_h: f64,
}

/// Chỗ mặt sau của chỗ mặt trước `s`: đối xứng qua tim dọc vùng in, cùng y; mặt trước xoay 90° thì
/// mặt sau xoay −90°.
fn mirror(s: &Slot, safe_left: f64, usable_w: f64) -> Slot {
    Slot { x: safe_left + usable_w - (s.x - safe_left) - s.w, angle: if s.angle == 90 { -90 } else { 0 }, ..*s }
}

// ---------------------------------------------------------------- bộ tính một khổ (`solveLayout`)

/// Một dải: `repeat` hàng (hay cột) mỗi dải `cells` con, khung con `cell_w` × `cell_h` đã xoay.
#[derive(Debug)]
struct Band {
    angle: i32,
    repeat: u64,
    cells: u64,
    cell_w: f64,
    cell_h: f64,
}

impl Band {
    fn slot(&self, x: f64, y: f64) -> Slot {
        let (base_w, base_h) = if self.angle == 90 { (self.cell_h, self.cell_w) } else { (self.cell_w, self.cell_h) };
        Slot { x, y, w: self.cell_w, h: self.cell_h, angle: self.angle, base_w, base_h }
    }
}

#[derive(Debug)]
enum Shape {
    Empty,
    /// `row`: các hàng xếp từ trên xuống; không: các cột xếp từ trái qua.
    Strips {
        row: bool,
        bands: Vec<Band>,
    },
    /// Một nhát dao: dọc cách mép trái `cut` (`vertical`), hay ngang cách mép trên `cut`.
    Split {
        vertical: bool,
        cut: f64,
        first: Rc<Plan>,
        second: Rc<Plan>,
    },
}

/// Một cách dàn một vùng: số con, số con không xoay / xoay, số nhát dao guillotine, độ phức tạp.
#[derive(Debug)]
struct Plan {
    shape: Shape,
    count: u64,
    normal: u64,
    rotated: u64,
    cuts: u64,
    complexity: u64,
}

impl Plan {
    fn empty() -> Plan {
        Plan { shape: Shape::Empty, count: 0, normal: 0, rotated: 0, cuts: 0, complexity: 0 }
    }

    fn strips(row: bool, bands: Vec<Band>, normal: u64, rotated: u64) -> Plan {
        let count = normal.saturating_add(rotated);
        let cuts = if count <= 1 {
            0
        } else {
            let inner = bands.iter().fold(0u64, |a, b| a.saturating_add(b.cells.saturating_sub(1)).saturating_add(b.repeat.saturating_sub(1)));
            inner.saturating_add((bands.len() as u64).saturating_sub(1))
        };
        Plan { complexity: bands.len() as u64, shape: Shape::Strips { row, bands }, count, normal, rotated, cuts }
    }

    fn split(vertical: bool, cut: f64, first: Rc<Plan>, second: Rc<Plan>) -> Plan {
        let count = first.count.saturating_add(second.count);
        let cuts = if count <= 1 {
            0
        } else if first.count == 0 {
            second.cuts
        } else if second.count == 0 {
            first.cuts
        } else {
            first.cuts.saturating_add(second.cuts).saturating_add(1)
        };
        let normal = first.normal.saturating_add(second.normal);
        let rotated = first.rotated.saturating_add(second.rotated);
        let complexity = first.complexity.saturating_add(second.complexity).saturating_add(1);
        Plan { shape: Shape::Split { vertical, cut, first, second }, count, normal, rotated, cuts, complexity }
    }
}

/// `isBetter`: nhiều con hơn, rồi ít nhát dao hơn, rồi đơn giản hơn, rồi nhiều con không xoay hơn.
fn is_better(c: &Plan, b: Option<&Plan>) -> bool {
    let Some(b) = b else { return true };
    if c.count != b.count {
        return c.count > b.count;
    }
    if c.cuts != b.cuts {
        return c.cuts < b.cuts;
    }
    if c.complexity != b.complexity {
        return c.complexity < b.complexity;
    }
    c.normal > b.normal
}

/// `floorFit`: số con khổ `size` lọt trong `space`.
fn floor_fit(space: f64, size: f64) -> u64 {
    if size <= EPS {
        return 0;
    }
    let n = ((space + EPS) / size).floor();
    if n > 0.0 { n as u64 } else { 0 }
}

struct Solver {
    cw: f64,
    ch: f64,
    memo: HashMap<(i64, i64, u32), Rc<Plan>>,
}

impl Solver {
    /// `makeStripPlan`: `nb` dải không xoay rồi `rb` dải xoay 90°.
    fn strip(&self, row: bool, nb: u64, rb: u64, w: f64, h: f64) -> Option<Plan> {
        let (nc, rc) = if row { (floor_fit(w, self.cw), floor_fit(w, self.ch)) } else { (floor_fit(h, self.ch), floor_fit(h, self.cw)) };
        if (nb > 0 && nc == 0) || (rb > 0 && rc == 0) {
            return None;
        }
        let (normal, rotated) = (nb.saturating_mul(nc), rb.saturating_mul(rc));
        if normal.saturating_add(rotated) == 0 {
            return None;
        }
        let mut bands = vec![];
        if nb > 0 {
            bands.push(Band { angle: 0, repeat: nb, cells: nc, cell_w: self.cw, cell_h: self.ch });
        }
        if rb > 0 {
            bands.push(Band { angle: 90, repeat: rb, cells: rc, cell_w: self.ch, cell_h: self.cw });
        }
        Some(Plan::strips(row, bands, normal, rotated))
    }

    /// `bestStripPlan`: thử mọi số hàng không xoay (phần còn lại xoay), rồi mọi số cột.
    fn best_strip(&self, w: f64, h: f64) -> Option<Plan> {
        let mut best: Option<Plan> = None;
        for nr in 0..=floor_fit(h, self.ch) {
            let rr = floor_fit(h - nr as f64 * self.ch, self.cw);
            if let Some(p) = self.strip(true, nr, rr, w, h)
                && is_better(&p, best.as_ref())
            {
                best = Some(p);
            }
        }
        for nc in 0..=floor_fit(w, self.cw) {
            let rc = floor_fit(w - nc as f64 * self.cw, self.ch);
            if let Some(p) = self.strip(false, nc, rc, w, h)
                && is_better(&p, best.as_ref())
            {
                best = Some(p);
            }
        }
        best
    }

    /// `bestSingleDirectionPlan`: lưới một chiều (toàn hàng không xoay, hay toàn hàng xoay).
    fn single_direction(&self, w: f64, h: f64) -> Option<Plan> {
        let normal = self.strip(true, floor_fit(h, self.ch), 0, w, h);
        let rotated = self.strip(true, 0, floor_fit(h, self.cw), w, h);
        match (normal, rotated) {
            (None, r) => r,
            (Some(n), Some(r)) if r.count > n.count => Some(r),
            (n, _) => n,
        }
    }

    /// `cutValues`: các vị trí dao theo bội số khổ con tính từ hai mép, không trùng (theo 0,001 pt).
    fn cut_values(&self, total: f64) -> Vec<f64> {
        let mut values = vec![];
        let mut seen = HashSet::new();
        let mut add = |v: f64| {
            if v <= EPS || total - v <= EPS {
                return;
            }
            if seen.insert(js_round(v * 1000.0) as i64) {
                values.push(v);
            }
        };
        for size in [self.cw, self.ch] {
            for i in 1..=floor_fit(total, size) {
                add(i as f64 * size);
                add(total - i as f64 * size);
            }
        }
        values
    }

    /// `solveRegion`: dải tốt nhất, hoặc một nhát dao chia hai vùng giải tiếp (`depth` tầng).
    fn solve(&mut self, w: f64, h: f64, depth: u32) -> Rc<Plan> {
        let key = (js_round(w * 1000.0) as i64, js_round(h * 1000.0) as i64, depth);
        if let Some(p) = self.memo.get(&key) {
            return Rc::clone(p);
        }
        let mut best = match self.best_strip(w, h) {
            Some(p) if depth > 0 => Rc::new(p),
            other => {
                let p = Rc::new(other.unwrap_or_else(Plan::empty));
                self.memo.insert(key, Rc::clone(&p));
                return p;
            }
        };
        // Bài rất nhỏ: chỉ lưới thẳng (như DanCard, để tính nhanh).
        if ((w * h + EPS) / (self.cw * self.ch)).floor() > 160.0 {
            self.memo.insert(key, Rc::clone(&best));
            return best;
        }
        for c in self.cut_values(w) {
            let first = self.solve(c, h, depth - 1);
            let second = self.solve(w - c, h, depth - 1);
            let p = Plan::split(true, c, first, second);
            if is_better(&p, Some(best.as_ref())) {
                best = Rc::new(p);
            }
        }
        for c in self.cut_values(h) {
            let first = self.solve(w, c, depth - 1);
            let second = self.solve(w, h - c, depth - 1);
            let p = Plan::split(false, c, first, second);
            if is_better(&p, Some(best.as_ref())) {
                best = Rc::new(p);
            }
        }
        self.memo.insert(key, Rc::clone(&best));
        best
    }
}

/// `solveLayout`: cách dàn con `cw` × `ch` trên vùng `w0` × `h0`; cùng số chỗ thì giữ lưới một chiều.
fn solve_layout(cw: f64, ch: f64, w0: f64, h0: f64, max_depth: u32) -> Rc<Plan> {
    let mut s = Solver { cw, ch, memo: HashMap::new() };
    let single = s.single_direction(w0, h0);
    let mixed = s.solve(w0, h0, max_depth);
    match single {
        Some(p) if mixed.count <= p.count => Rc::new(p),
        _ => mixed,
    }
}

/// `collectSlots`: các chỗ của `plan` trên vùng có góc trên trái (`left`, `top`) (y hướng lên).
fn collect_slots(plan: &Plan, left: f64, top: f64, out: &mut Vec<Slot>) {
    if plan.count == 0 {
        return;
    }
    match &plan.shape {
        Shape::Empty => {}
        Shape::Split { vertical: true, cut, first, second } => {
            collect_slots(first, left, top, out);
            collect_slots(second, left + *cut, top, out);
        }
        Shape::Split { vertical: false, cut, first, second } => {
            collect_slots(first, left, top, out);
            collect_slots(second, left, top - *cut, out);
        }
        Shape::Strips { row, bands } => {
            let (mut x, mut y) = (left, top);
            for band in bands {
                for _ in 0..band.repeat {
                    if *row {
                        x = left;
                    } else {
                        y = top;
                    }
                    for _ in 0..band.cells {
                        out.push(band.slot(x, y));
                        if *row {
                            x += band.cell_w;
                        } else {
                            y -= band.cell_h;
                        }
                    }
                    if *row {
                        y -= band.cell_h;
                    } else {
                        x += band.cell_w;
                    }
                }
            }
        }
    }
}

/// Chặn đầu vào làm bộ tính chạy quá lâu: con nhỏ nhất `cw` × `ch` trên nửa tờ `w` × `h`.
fn check_size(cw: f64, ch: f64, w: f64, h: f64) -> std::result::Result<(), String> {
    let (cw, ch) = (cw.max(EPS), ch.max(EPS));
    let area = ((w * h + EPS) / (cw * ch)).floor();
    let lines = (w + h + 2.0 * EPS) / cw.min(ch);
    if !area.is_finite() || !lines.is_finite() || area > MAX_SLOTS || lines > MAX_LINES {
        return Err(format!(
            "Bài quá nhỏ so với tờ giấy: một nửa tờ chứa hơn {MAX_SLOTS} con. Hãy dàn lên tờ nhỏ hơn hoặc gom bài thành cụm lớn hơn trước khi dàn."
        ));
    }
    Ok(())
}

/// Kết quả bộ tính một khổ (toạ độ DanCard, tờ [0, cao, rộng, 0]).
struct SinglePlan {
    margin: f64,
    usable_w: f64,
    pick_dw: f64,
    pick_dh: f64,
    cell_w: f64,
    cell_h: f64,
    plan: Rc<Plan>,
    /// Mọi chỗ của nửa trái theo thứ tự đọc; `used` chỗ đầu đã dồn sát tim giấy.
    slots: Vec<Slot>,
    used: usize,
    leftover: usize,
    empty: usize,
    /// (cặp, các chỗ mặt trước của nó), theo thứ tự cặp xuất hiện.
    assignments: Vec<(usize, Vec<Slot>)>,
    /// Dời cả cụm (trước + sau) để canh giữa tờ: (dx, dy), y hướng lên.
    center: (f64, f64),
}

/// Dàn một khổ (`dcDanTuTro`): con `source_w` × `source_h` (khổ lớn nhất các mặt), `n_pairs` cặp.
fn plan_single(source_w: f64, source_h: f64, paper: &Paper, n_pairs: usize) -> std::result::Result<SinglePlan, String> {
    let margin = paper.margin_mm * MM;
    // Không bóp quá 20 % cạnh ngắn.
    let max_bop = (paper.max_bop_mm * MM).min(source_w.min(source_h) * 0.2);
    let usable_w = paper.w - 2.0 * margin;
    let usable_h = paper.h - 2.0 * margin;
    if usable_w <= EPS || usable_h <= EPS {
        return Err("Khổ giấy phải lớn hơn lề an toàn hai bên.".into());
    }
    let half_w = usable_w / 2.0;
    check_size(source_w - max_bop, source_h - max_bop, half_w, usable_h)?;
    // Bóp: 11 mức mỗi chiều; chọn mức rẻ nhất (bóp tương đối ít nhất) mà đủ số con của mức bóp tối
    // đa, chỉ khi bóp tối đa dàn được nhiều hơn không bóp (đo ở độ sâu 1).
    let (step, steps): (f64, u32) = if max_bop > EPS { (max_bop / 10.0, 10) } else { (0.0, 0) };
    let mut cands = vec![];
    for ai in 0..=steps {
        for bi in 0..=steps {
            let (dw, dh) = (f64::from(ai) * step, f64::from(bi) * step);
            cands.push((dw, dh, dw / source_w + dh / source_h));
        }
    }
    let cands = js_sorted(cands, |a, b| a.2 - b.2);
    let (mut pick_dw, mut pick_dh) = (0.0, 0.0);
    if steps > 0 {
        let full = solve_layout(source_w, source_h, half_w, usable_h, 1).count;
        let tight = solve_layout(source_w - max_bop, source_h - max_bop, half_w, usable_h, 1).count;
        if tight > full
            && let Some(&(dw, dh, _)) = cands.iter().find(|c| solve_layout(source_w - c.0, source_h - c.1, half_w, usable_h, 1).count >= tight)
        {
            (pick_dw, pick_dh) = (dw, dh);
        }
    }
    let (cell_w, cell_h) = (source_w - pick_dw, source_h - pick_dh);
    let plan = solve_layout(cell_w, cell_h, half_w, usable_h, 2);
    if plan.count == 0 {
        return Err(NOT_FIT_SINGLE.into());
    }
    let (safe_left, safe_top) = (margin, paper.h - margin);
    let mut slots = vec![];
    collect_slots(&plan, safe_left, safe_top, &mut slots);
    if slots.len() as u64 != plan.count {
        return Err("Lỗi tạo vị trí dàn. Hãy thử lại.".into());
    }
    // Thứ tự đọc: hàng trên trước, trong hàng trái qua.
    let mut slots = js_sorted(slots, |a, b| if (a.y - b.y).abs() > 0.5 { b.y - a.y } else { a.x - b.x });
    let used = if paper.repeat { slots.len() } else { n_pairs.min(slots.len()) };
    let leftover = if paper.repeat { 0 } else { n_pairs.saturating_sub(used) };
    let empty = slots.len().saturating_sub(used);
    // Không khe ở tim giấy: dồn các chỗ dùng tới sát tim của nửa trái.
    let right = slots.iter().take(used).fold(safe_left, |r, s| r.max(s.x + s.w));
    let shift = safe_left + half_w - right;
    if shift > EPS {
        for s in slots.iter_mut().take(used) {
            s.x += shift;
        }
    }
    // Lặp lại: các cặp lần lượt vào các chỗ (0, 1, 2, 0, 1, 2…).
    let mut assignments: Vec<(usize, Vec<Slot>)> = vec![];
    let mut index: HashMap<usize, usize> = HashMap::new();
    for (k, s) in slots.iter().take(used).enumerate() {
        let pair = if paper.repeat { k.checked_rem(n_pairs).unwrap_or(0) } else { k };
        let at = *index.entry(pair).or_insert_with(|| {
            assignments.push((pair, vec![]));
            assignments.len() - 1
        });
        if let Some((_, v)) = assignments.get_mut(at) {
            v.push(*s);
        }
    }
    // Canh giữa tờ theo các chỗ (trước + sau) đã dùng: cụm hai mặt đối xứng qua tim giấy nên chỉ dời
    // dọc, trước / sau không lệch đăng.
    let (mut top, mut bottom, mut left, mut far) = (f64::NEG_INFINITY, f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for s in slots.iter().take(used) {
        let m = mirror(s, safe_left, usable_w);
        top = top.max(s.y);
        bottom = bottom.min(s.y - s.h);
        left = left.min(s.x);
        far = far.max(m.x + m.w);
    }
    let center = if used > 0 { (paper.w / 2.0 - (left + far) / 2.0, paper.h / 2.0 - (top + bottom) / 2.0) } else { (0.0, 0.0) };
    Ok(SinglePlan { margin, usable_w, pick_dw, pick_dh, cell_w, cell_h, plan, slots, used, leftover, empty, assignments, center })
}

// ---------------------------------------------------------------- bộ xếp nhiều khổ (`dcDanTuTroNhieuKho`)

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fit {
    /// Ít cạnh thừa ngắn nhất (hoà: cạnh thừa dài).
    Short,
    /// Ít diện tích thừa nhất (hoà: cạnh thừa ngắn).
    Area,
}

/// Hai nhát dao chẻ vùng trống sau khi đặt: dọc trước, hay ngang trước.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    Vertical,
    Horizontal,
}

/// Vùng trống (y hướng xuống, gốc góc trên trái vùng in).
#[derive(Clone, Copy, Debug)]
struct Free {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Một cặp đã đặt (y hướng xuống).
#[derive(Clone, Copy, Debug)]
struct Placed {
    pair: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    base_w: f64,
    base_h: f64,
    angle: i32,
    free_index: usize,
    cut: Cut,
    split_score: f64,
    short_side: f64,
    long_side: f64,
    area_left: f64,
}

#[derive(Clone, Debug, Default)]
struct Packing {
    placed: Vec<Placed>,
    area: f64,
    normal: usize,
}

/// `splitScore`: cạnh ngắn của mảnh thừa dùng được nhất sau khi chẻ.
fn split_score(free: &Free, w: f64, h: f64, cut: Cut) -> f64 {
    let (rw, rh) = (free.w - w, free.h - h);
    let (first, second) = match cut {
        Cut::Vertical => ((rw, free.h), (w, rh)),
        Cut::Horizontal => ((free.w, rh), (rw, h)),
    };
    let side = |(a, b): (f64, f64)| if a > EPS && b > EPS { a.min(b) } else { 0.0 };
    side(first).max(side(second))
}

/// `splitGuillotineRect`: bỏ vùng trống đã dùng, thêm hai mảnh thừa (hai nhát dao thẳng mép tới mép).
fn split_free(frees: &[Free], p: &Placed) -> Vec<Free> {
    let Some(free) = frees.get(p.free_index).copied() else { return frees.to_vec() };
    let mut next: Vec<Free> = frees.iter().enumerate().filter(|(i, _)| *i != p.free_index).map(|(_, f)| *f).collect();
    let (rw, rh) = (free.w - p.w, free.h - p.h);
    match p.cut {
        Cut::Vertical => {
            if rw > EPS {
                next.push(Free { x: free.x + p.w, y: free.y, w: rw, h: free.h });
            }
            if rh > EPS {
                next.push(Free { x: free.x, y: free.y + p.h, w: p.w, h: rh });
            }
        }
        Cut::Horizontal => {
            if rh > EPS {
                next.push(Free { x: free.x, y: free.y + p.h, w: free.w, h: rh });
            }
            if rw > EPS {
                next.push(Free { x: free.x + p.w, y: free.y, w: rw, h: p.h });
            }
        }
    }
    next
}

/// Chỗ `c` tốt hơn chỗ `b` (cùng quy tắc so sánh của `bestSlotFor`).
fn slot_better(c: &Placed, b: &Placed, fit: Fit) -> bool {
    let (pn, po, sn, so) = match fit {
        Fit::Area => (c.area_left, b.area_left, c.short_side, b.short_side),
        Fit::Short => (c.short_side, b.short_side, c.long_side, b.long_side),
    };
    if pn < po - EPS {
        return true;
    }
    if (pn - po).abs() > EPS {
        return false;
    }
    if sn < so - EPS {
        return true;
    }
    if (sn - so).abs() > EPS {
        return false;
    }
    if c.split_score > b.split_score + EPS {
        return true;
    }
    if (c.split_score - b.split_score).abs() > EPS {
        return false;
    }
    if c.angle != b.angle {
        return c.angle < b.angle;
    }
    if c.y < b.y - EPS {
        return true;
    }
    (c.y - b.y).abs() <= EPS && c.x < b.x - EPS
}

/// `betterPacking`: nhiều cặp hơn, rồi nhiều diện tích hơn, rồi nhiều cặp không xoay hơn.
fn better_packing(c: &Packing, b: Option<&Packing>) -> bool {
    let Some(b) = b else { return true };
    if c.placed.len() != b.placed.len() {
        return c.placed.len() > b.placed.len();
    }
    if (c.area - b.area).abs() > EPS {
        return c.area > b.area;
    }
    c.normal > b.normal
}

/// `buildOrders`: các thứ tự xếp theo diện tích, cạnh dài, cạnh ngắn, rộng, cao (và vài vòng xoay
/// của chúng), bỏ thứ tự trùng. Không dùng thứ tự chọn.
fn build_orders(specs: &[(f64, f64)]) -> Vec<Vec<usize>> {
    let n = specs.len();
    let original: Vec<usize> = (0..n).collect();
    let key = |i: usize, f: fn(f64, f64) -> f64| specs.get(i).map_or(0.0, |&(w, h)| f(w, h));
    let by = |desc: bool, f: fn(f64, f64) -> f64| {
        js_sorted(original.clone(), |&a, &b| {
            let (ka, kb) = (key(a, f), key(b, f));
            if (kb - ka).abs() > EPS { if desc { kb - ka } else { ka - kb } } else { a as f64 - b as f64 }
        })
    };
    let area_desc = by(true, |w, h| w * h);
    let mut area_asc = area_desc.clone();
    area_asc.reverse();
    let bases = [area_desc, area_asc, by(true, f64::max), by(false, f64::min), by(true, |w, _| w), by(true, |_, h| h)];
    let mut shifts = vec![0];
    if n > 2 {
        shifts.push(1);
    }
    if n > 5 {
        shifts.push(n / 3);
    }
    if n > 9 {
        shifts.push(n / 2);
    }
    if n > 40 {
        shifts = vec![0, n / 2];
    }
    let mut orders = vec![];
    let mut seen = HashSet::new();
    for base in &bases {
        for &s in &shifts {
            let rotated: Vec<usize> = (0..n).filter_map(|i| (i + s).checked_rem(n).and_then(|j| base.get(j)).copied()).collect();
            let k = rotated.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
            if seen.insert(k) {
                orders.push(rotated);
            }
        }
    }
    orders
}

struct Packer<'a> {
    /// Khổ mỗi cặp (rộng, cao), pt.
    specs: &'a [(f64, f64)],
    half_w: f64,
    usable_h: f64,
    orders: Vec<Vec<usize>>,
}

impl Packer<'_> {
    /// `bestSlotFor`: chỗ tốt nhất cho cặp `pair` (bóp `dw`, `dh`) trong các vùng trống.
    fn best_slot(&self, pair: usize, frees: &[Free], dw: f64, dh: f64, fit: Fit, cuts: &[Cut]) -> Option<Placed> {
        let &(sw, sh) = self.specs.get(pair)?;
        let (base_w, base_h) = (sw - dw, sh - dh);
        if base_w <= EPS || base_h <= EPS {
            return None;
        }
        let mut best: Option<Placed> = None;
        for (fi, free) in frees.iter().enumerate() {
            for angle in [0, 90] {
                let (w, h) = if angle == 90 { (base_h, base_w) } else { (base_w, base_h) };
                if w > free.w + EPS || h > free.h + EPS {
                    continue;
                }
                for &cut in cuts {
                    let c = Placed {
                        pair,
                        x: free.x,
                        y: free.y,
                        w,
                        h,
                        base_w,
                        base_h,
                        angle,
                        free_index: fi,
                        cut,
                        split_score: split_score(free, w, h, cut),
                        short_side: (free.w - w).min(free.h - h),
                        long_side: (free.w - w).max(free.h - h),
                        area_left: free.w * free.h - w * h,
                    };
                    if best.as_ref().is_none_or(|b| slot_better(&c, b, fit)) {
                        best = Some(c);
                    }
                }
            }
        }
        best
    }

    /// `packInOrder`: đặt lần lượt theo `order`; cặp không vừa thì bỏ qua, thử tiếp các cặp sau.
    fn pack(&self, order: &[usize], dw: f64, dh: f64, fit: Fit, cuts: &[Cut]) -> Packing {
        let mut frees = vec![Free { x: 0.0, y: 0.0, w: self.half_w, h: self.usable_h }];
        let mut out = Packing::default();
        for &pair in order {
            let Some(slot) = self.best_slot(pair, &frees, dw, dh, fit, cuts) else { continue };
            out.area += slot.w * slot.h;
            if slot.angle == 0 {
                out.normal += 1;
            }
            frees = split_free(&frees, &slot);
            out.placed.push(slot);
        }
        out
    }

    /// `findBestPacking`: ba cách chấm điểm × các thứ tự; xếp được hết các cặp thì dừng ngay.
    fn best_packing(&self, dw: f64, dh: f64) -> Option<Packing> {
        const MODES: [(Fit, &[Cut]); 3] =
            [(Fit::Short, &[Cut::Vertical]), (Fit::Short, &[Cut::Horizontal]), (Fit::Area, &[Cut::Vertical, Cut::Horizontal])];
        let mut best: Option<Packing> = None;
        for (fit, cuts) in MODES {
            for order in &self.orders {
                let p = self.pack(order, dw, dh, fit, cuts);
                if better_packing(&p, best.as_ref()) {
                    best = Some(p);
                }
                if best.as_ref().is_some_and(|b| b.placed.len() == self.specs.len()) {
                    return best;
                }
            }
        }
        best
    }
}

/// Kết quả bộ xếp nhiều khổ (toạ độ DanCard, tờ [0, cao, rộng, 0]).
struct MixedPlan {
    margin: f64,
    usable_w: f64,
    pick_dw: f64,
    pick_dh: f64,
    /// Cách xếp đã chọn (các chỗ theo thứ tự đặt, y hướng xuống, chưa dồn sát tim giấy).
    best: Packing,
    /// (cặp, chỗ mặt trước), theo thứ tự đọc của các chỗ.
    assignments: Vec<(usize, Vec<Slot>)>,
}

/// Dàn nhiều khổ (`dcDanTuTroNhieuKho`, không có chế độ A/B): khổ mỗi cặp `specs` (pt).
fn plan_mixed(specs: &[(f64, f64)], paper: &Paper) -> std::result::Result<MixedPlan, String> {
    let margin = paper.margin_mm * MM;
    let smallest = specs.iter().map(|&(w, h)| w.min(h)).reduce(f64::min).unwrap_or(0.0);
    let max_bop = (paper.max_bop_mm * MM).min(smallest * 0.2);
    let usable_w = paper.w - 2.0 * margin;
    let usable_h = paper.h - 2.0 * margin;
    if usable_w <= EPS || usable_h <= EPS {
        return Err("Khổ giấy phải lớn hơn lề an toàn hai bên.".into());
    }
    let half_w = usable_w / 2.0;
    let packer = Packer { specs, half_w, usable_h, orders: build_orders(specs) };
    // Bóp: 9 mức mỗi chiều, cùng một lượng cho mọi cặp; tổng bóp (pt) ít trước.
    let (step, steps): (f64, u32) = if max_bop > EPS { (max_bop / 8.0, 8) } else { (0.0, 0) };
    let mut cands = vec![(0.0, 0.0, 0.0)];
    for bx in 0..=steps {
        for by in 0..=steps {
            if bx == 0 && by == 0 {
                continue;
            }
            cands.push((f64::from(bx) * step, f64::from(by) * step, f64::from(bx + by) * step));
        }
    }
    let cands = js_sorted(cands, |a, b| a.2 - b.2);
    let mut best: Option<Packing> = None;
    let (mut pick_dw, mut pick_dh, mut best_cost) = (0.0, 0.0, 0.0);
    for &(dw, dh, cost) in &cands {
        if best.as_ref().is_some_and(|b| b.placed.len() == specs.len()) {
            break;
        }
        let Some(p) = packer.best_packing(dw, dh) else { continue };
        if p.placed.is_empty() {
            continue;
        }
        let take = match &best {
            None => true,
            Some(b) => {
                p.placed.len() > b.placed.len()
                    || (p.placed.len() == b.placed.len()
                        && (cost < best_cost - EPS || ((cost - best_cost).abs() <= EPS && better_packing(&p, Some(b)))))
            }
        };
        if take {
            best = Some(p);
            (pick_dw, pick_dh, best_cost) = (dw, dh, cost);
        }
    }
    let Some(best) = best.filter(|b| !b.placed.is_empty()) else { return Err(NOT_FIT_MIXED.into()) };
    // Thứ tự đọc (y hướng xuống: trên trước), rồi dồn sát tim giấy. Không canh giữa theo chiều dọc.
    let mut slots = js_sorted(best.placed.clone(), |a, b| if (a.y - b.y).abs() > EPS { a.y - b.y } else { a.x - b.x });
    let right = slots.iter().fold(0.0_f64, |r, s| r.max(s.x + s.w));
    let shift = half_w - right;
    if shift > EPS {
        for s in &mut slots {
            s.x += shift;
        }
    }
    let (safe_left, safe_top) = (margin, paper.h - margin);
    let mut assignments: Vec<(usize, Vec<Slot>)> = vec![];
    for s in &slots {
        let slot = Slot { x: safe_left + s.x, y: safe_top - s.y, w: s.w, h: s.h, angle: s.angle, base_w: s.base_w, base_h: s.base_h };
        match assignments.iter().position(|(p, _)| *p == s.pair) {
            Some(i) => {
                if let Some((_, v)) = assignments.get_mut(i) {
                    v.push(slot);
                }
            }
            None => assignments.push((s.pair, vec![slot])),
        }
    }
    Ok(MixedPlan { margin, usable_w, pick_dw, pick_dh, best, assignments })
}

// ---------------------------------------------------------------- thông báo

fn squeeze_parts(dw: f64, dh: f64) -> String {
    let mut parts = vec![];
    if dw > EPS {
        parts.push(format!("ngang {} mm", mm(dw)));
    }
    if dh > EPS {
        parts.push(format!("dọc {} mm", mm(dh)));
    }
    parts.join(", ")
}

/// Thông báo OK của `dcDanTuTro`.
fn single_message(paper: &Paper, r: &SinglePlan, n_pairs: usize) -> String {
    let (count, used) = (r.plan.count, r.used);
    let mut t = format!("Sức chứa tờ {} × {} cm: {count} chỗ mỗi nửa. Đã dàn {used} con mặt trước + {used} con mặt sau", cm(paper.w), cm(paper.h));
    if paper.repeat && n_pairs < r.slots.len() {
        t.push_str(&format!(" (lặp lại {n_pairs} mẫu cho đầy tờ)"));
    }
    t.push_str(&format!(". KT con dàn: {} × {} cm", cm(r.cell_w), cm(r.cell_h)));
    if r.pick_dw > EPS || r.pick_dh > EPS {
        t.push_str(&format!(" (đã bóp {} để dàn thêm được con)", squeeze_parts(r.pick_dw, r.pick_dh)));
    } else {
        t.push_str(" (giữ nguyên, không cần bóp)");
    }
    t.push_str(&format!(". Lề an toàn {} mm.", paper.margin_mm));
    if r.leftover > 0 {
        t.push_str(&format!(" Còn {} cặp chưa dàn, đã để nguyên chỗ cũ để dàn tiếp vào tờ sau.", r.leftover));
    } else if r.empty > 0 {
        t.push_str(&format!(" Hết mẫu, tờ còn trống {} chỗ mỗi nửa.", r.empty));
    } else {
        t.push_str(" Đã dàn kín tờ.");
    }
    t.push_str(" In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó.");
    format!("OK:[[COUNT:{count}]] {t}")
}

/// Thông báo OK của `dcDanTuTroNhieuKho`.
fn mixed_message(paper: &Paper, r: &MixedPlan, n_pairs: usize) -> String {
    let placed = r.best.placed.len();
    let mut t =
        format!("Tờ {} × {} cm: đã dàn {placed} / {n_pairs} cặp. Mỗi cặp có 1 mặt trước + 1 mặt sau đối xứng tự trở.", cm(paper.w), cm(paper.h));
    if r.pick_dw > EPS || r.pick_dh > EPS {
        t.push_str(&format!(" Đã bóp {} để dàn thêm được cặp.", squeeze_parts(r.pick_dw, r.pick_dh)));
    } else {
        t.push_str(" Giữ nguyên kích thước, không cần bóp.");
    }
    t.push_str(&format!(" Lề an toàn {} mm.", paper.margin_mm));
    if paper.repeat && n_pairs > 1 {
        t.push_str(" Đã bỏ qua Lặp lại mẫu vì đang chọn nhiều cặp: mỗi cặp chỉ dàn một lần.");
    }
    let left = n_pairs.saturating_sub(r.assignments.len());
    if left > 0 {
        t.push_str(&format!(" Còn {left} cặp chưa dàn, giữ nguyên ở vị trí cũ để chạy tờ tiếp theo."));
    } else {
        t.push_str(" Đã dàn hết các cặp nguồn chọn vào tờ này.");
    }
    t.push_str(" In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó.");
    format!("OK:[[COUNT:{placed}]] {t}")
}

// ---------------------------------------------------------------- đo và dựng trên tài liệu

fn usable(r: &Rect) -> bool {
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) && r.width() > EPS && r.height() > EPS
}

/// `boundsOf` của DanCard: khung nhìn thấy (có viền; clip group theo clipping path), không dùng được
/// thì khung hình học.
fn bounds_of(n: &Node, d: &Document) -> Option<Rect> {
    let symbols = d.symbol_boxes();
    n.visual_bounds_with(&symbols).filter(usable).or_else(|| n.geometric_bounds_with(&symbols).filter(usable))
}

/// Xoay, dời, bóp: nét giữ nguyên độ dày, hoa văn đi theo bài.
fn moves() -> Scaling {
    Scaling { patterns: true, ..Scaling::default() }
}

/// Một object đang chọn và khung của nó theo kiểu DanCard: [trái, trên, phải, dưới], y hướng lên.
#[derive(Clone, Copy, Debug)]
struct Item {
    id: NodeId,
    b: [f64; 4],
}

impl Item {
    fn size(&self) -> (f64, f64) {
        (self.b[2] - self.b[0], self.b[1] - self.b[3])
    }
}

/// Thứ tự đọc của các khung `bounds` ([trái, trên, phải, dưới], y hướng lên): gom hàng từ trên
/// xuống (cùng hàng khi tâm thấp hơn tâm con đầu hàng không quá nửa chiều cao con cao nhất tờ), trong
/// hàng trái qua.
fn reading_order(bounds: &[[f64; 4]]) -> Vec<usize> {
    let raw: Vec<(usize, f64, f64, f64)> =
        bounds.iter().enumerate().map(|(i, b)| (i, (b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0, b[1] - b[3])).collect();
    let raw = js_sorted(raw, |a, b| b.2 - a.2);
    let Some(&(_, _, first_cy, first_h)) = raw.first() else { return vec![] };
    let tolerance = first_h * 0.5;
    let mut ordered = vec![];
    let mut row: Vec<(usize, f64, f64, f64)> = vec![];
    let mut anchor = first_cy;
    let flush = |row: Vec<(usize, f64, f64, f64)>, ordered: &mut Vec<usize>| {
        ordered.extend(js_sorted(row, |a, b| a.1 - b.1).into_iter().map(|r| r.0));
    };
    for (k, r) in raw.iter().enumerate() {
        if k > 0 && anchor - r.2 > tolerance {
            flush(std::mem::take(&mut row), &mut ordered);
            anchor = r.2;
        }
        row.push(*r);
    }
    flush(row, &mut ordered);
    ordered
}

/// Đổi toạ độ DanCard trên tờ ([0, cao, rộng, 0], y hướng lên) sang trang.
struct Frame {
    /// Góc trên trái tờ trên trang.
    origin: Point,
    paper_h: f64,
    /// Dời thêm để canh giữa tờ (trên trang).
    shift: Vec2,
}

impl Frame {
    fn at(&self, x: f64, y_up: f64) -> Point {
        Point::new(self.origin.x + x, self.origin.y + (self.paper_h - y_up))
    }
}

/// Bản xoay `angle` độ ngược chiều kim đồng hồ (0, 90 hay −90) của `master` quanh tâm khung, kèm
/// khung đo được.
fn turned(master: &Node, angle: i32, d: &Document) -> Option<(i32, Node, Rect)> {
    let mut n = master.clone();
    if angle != 0 {
        let c = bounds_of(&n, d)?.center().to_vec2();
        // Đúng 90° (không sai số sin / cos). Trang y hướng xuống: ngược chiều kim đồng hồ 90° là
        // (x, y) → (y, −x).
        let r = if angle > 0 { Affine::new([0.0, -1.0, 1.0, 0.0, 0.0, 0.0]) } else { Affine::new([0.0, 1.0, -1.0, 0.0, 0.0, 0.0]) };
        n.transform(Affine::translate(c) * r * Affine::translate(-c), moves());
    }
    let b = bounds_of(&n, d)?;
    Some((angle, n, b))
}

/// `copyAt`: bản sao của `master` vào chỗ `slot`, xoay theo chỗ, canh giữa trong chỗ (bài lớn hơn chỗ
/// thì canh trên trái). `turns`: các bản đã xoay của `master`, đo một lần cho mỗi góc.
fn place(d: &mut Document, turns: &mut Vec<(i32, Node, Rect)>, master: &Node, slot: &Slot, frame: &Frame) -> Result<Node> {
    if !turns.iter().any(|t| t.0 == slot.angle) {
        let t = turned(master, slot.angle, d).ok_or_else(|| other(NO_COPY_BOUNDS))?;
        turns.push(t);
    }
    let (proto, b) = turns.iter().find(|t| t.0 == slot.angle).map(|t| (&t.1, t.2)).ok_or_else(|| other(NO_COPY_BOUNDS))?;
    let left = slot.x + ((slot.w - b.width()) / 2.0).max(0.0);
    let top = slot.y - ((slot.h - b.height()) / 2.0).max(0.0);
    let to = frame.at(left, top) + frame.shift;
    let mut copy = d.reid(proto);
    copy.transform(Affine::translate(to - b.origin()), moves());
    Ok(copy)
}

/// `resizeAtCenter`: co giãn `sx` × `sy` theo trục trang, giữ tâm khung nhìn thấy, nét không đổi.
fn resize_at_center(n: &mut Node, d: &Document, sx: f64, sy: f64) -> Result<Option<Rect>> {
    let before = bounds_of(n, d).ok_or_else(|| other("Không đo được bài trước khi bóp."))?;
    let c = before.center().to_vec2();
    n.transform(Affine::translate(c) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c), moves());
    let after = bounds_of(n, d).ok_or_else(|| other("Không đo được bài sau khi bóp."))?;
    n.transform(Affine::translate(c - after.center().to_vec2()), moves());
    Ok(bounds_of(n, d))
}

/// `makeSqueezedMaster`: bản mẫu của `src` đã bóp cho khung nhìn thấy thành `tw` × `th` (một lần bù
/// khi lệch quá 0,5 pt, vì nét không co giãn). Bản mẫu không nằm trong tài liệu.
fn squeezed(src: &Node, d: &Document, (tw, th): (f64, f64), mixed: bool) -> Result<Node> {
    let mut m = src.clone();
    let before = bounds_of(&m, d).ok_or_else(|| other("Kích thước bài không hợp lệ."))?;
    if (before.width() - tw).abs() <= 0.05 && (before.height() - th).abs() <= 0.05 {
        return Ok(m);
    }
    let (sx, sy) = (tw / before.width(), th / before.height());
    if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
        return Err(other(if mixed { "Không tính được tỷ lệ bóp." } else { "Không tính được tỉ lệ bóp." }));
    }
    if let Some(done) = resize_at_center(&mut m, d, sx, sy)?
        && ((done.width() - tw).abs() > 0.5 || (done.height() - th).abs() > 0.5)
    {
        resize_at_center(&mut m, d, tw / done.width(), th / done.height())?;
    }
    Ok(m)
}

/// `uniqueLayerName`: `base`, `base 2`, `base 3`… chưa layer nào dùng.
fn unique_layer_name(d: &Document, base: &str) -> String {
    let taken = |name: &str| d.layers.iter().any(|l| l.name.as_deref() == Some(name));
    if !taken(base) {
        return base.to_string();
    }
    (2u64..).map(|i| format!("{base} {i}")).find(|name| !taken(name)).unwrap_or_else(|| base.to_string())
}

/// Layer mới trên cùng tên `base` (không trùng), chứa `children` (dưới cùng trước).
fn add_layer(d: &mut Document, base: &str, children: Vec<Node>, locked: bool) -> Result<NodeId> {
    let name = unique_layer_name(d, base);
    let mut layer = crate::cmd::layer::make_layer(d, Some(&name));
    layer.visible = true;
    layer.locked = locked;
    if let Some(ch) = layer.children_mut() {
        ch.extend(children.into_iter().map(Arc::new));
    }
    Ok(d.insert(None, usize::MAX, layer)?)
}

/// PON giấy ở 4 góc `r` (artboard): 8 nét K 100, tâm nét nằm trên mép artboard.
fn paper_pon(d: &mut Document, r: Rect) -> Vec<Node> {
    let l = PON_LENGTH;
    let segs = [
        ((r.x0, r.y0), (r.x0 + l, r.y0)),
        ((r.x0, r.y0), (r.x0, r.y0 + l)),
        ((r.x1 - l, r.y0), (r.x1, r.y0)),
        ((r.x1, r.y0), (r.x1, r.y0 + l)),
        ((r.x0, r.y1), (r.x0 + l, r.y1)),
        ((r.x0, r.y1), (r.x0, r.y1 - l)),
        ((r.x1 - l, r.y1), (r.x1, r.y1)),
        ((r.x1, r.y1), (r.x1, r.y1 - l)),
    ];
    let ink = Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0));
    segs.iter()
        .map(|&((ax, ay), (bx, by))| {
            Node::path(d.alloc_id(), shapes::line(Point::new(ax, ay), Point::new(bx, by)), Appearance::basic(Paint::None, ink.clone(), PON_STROKE))
        })
        .collect()
}

/// Tờ mới (trang): khổ `w` × `h`, cách 10 mm bên phải mọi bài và artboard, ngang mép trên của chúng.
fn sheet_place(d: &Document, w: f64, h: f64, mixed: bool) -> Result<Rect> {
    let mut area = d.artboards.iter().map(|a| a.rect).reduce(|a, b| a.union(b));
    if let Some(art) = d.art_bounds() {
        area = Some(area.map_or(art, |a| a.union(art)));
    }
    let (left, top) = area.map_or((0.0, 0.0), |a| (a.x1 + PAGE_GAP, a.y0));
    let r = Rect::new(left, top, left + w, top + h);
    if [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite() && v.abs() <= crate::MAX_COORD) {
        Ok(r)
    } else if mixed {
        Err(other("Không còn chỗ trong canvas để tạo tờ mới. Hãy bớt artboard cũ."))
    } else {
        Err(other("Không còn chỗ trống trong canvas để tạo tờ mới. Hãy xoá bớt artboard cũ."))
    }
}

/// Những gì cần dựng, chung cho hai nhánh.
struct Job {
    mixed: bool,
    paper: Paper,
    /// (cặp, các chỗ mặt trước), toạ độ DanCard trên tờ.
    assignments: Vec<(usize, Vec<Slot>)>,
    /// Khi bóp: khổ đích [mặt trước, mặt sau] cho từng phần tử của `assignments`.
    targets: Option<Vec<[(f64, f64); 2]>>,
    margin: f64,
    usable_w: f64,
    /// Dời cả cụm để canh giữa tờ (y hướng lên).
    center: (f64, f64),
    message: String,
    count: usize,
    placed: usize,
    leftover: usize,
    squeeze: (f64, f64),
}

/// Nhánh một khổ: mọi mặt phải lệch không quá 0,5 mm so với mặt trước của cặp đầu.
fn single_job(p: &Value, pairs: &[(Item, Item)], source_w: f64, source_h: f64) -> Result<Job> {
    let (mut big_w, mut big_h) = (source_w, source_h);
    for (pi, (f, b)) in pairs.iter().enumerate() {
        for (face, side) in [(f, "mặt trước"), (b, "mặt sau")] {
            let (w, h) = face.size();
            if (w - source_w).abs() > SIZE_TOLERANCE || (h - source_h).abs() > SIZE_TOLERANCE {
                return Err(other(format!(
                    "Cặp thứ {} ({side}) lệch quá 0.5 mm so với cặp đầu. Mọi mẫu phải cùng khổ thành phẩm; hãy group/clip từng mặt về cùng một khung trước khi dàn.",
                    pi + 1
                )));
            }
            big_w = big_w.max(w);
            big_h = big_h.max(h);
        }
    }
    let paper = parse_paper(p).map_err(other)?;
    let r = plan_single(big_w, big_h, &paper, pairs.len()).map_err(other)?;
    // Mỗi mặt bóp theo đúng tỉ lệ của chính nó.
    let target = |it: &Item| {
        let (w, h) = it.size();
        (w * r.cell_w / big_w, h * r.cell_h / big_h)
    };
    let targets = (r.pick_dw > EPS || r.pick_dh > EPS)
        .then(|| r.assignments.iter().map(|(pi, _)| pairs.get(*pi).map_or([(0.0, 0.0); 2], |(f, b)| [target(f), target(b)])).collect());
    Ok(Job {
        mixed: false,
        message: single_message(&paper, &r, pairs.len()),
        count: usize::try_from(r.plan.count).unwrap_or(usize::MAX),
        placed: r.used,
        leftover: r.leftover,
        squeeze: (r.pick_dw, r.pick_dh),
        paper,
        targets,
        margin: r.margin,
        usable_w: r.usable_w,
        center: r.center,
        assignments: r.assignments,
    })
}

/// Nhánh nhiều khổ.
fn mixed_job(p: &Value, pairs: &[(Item, Item)], specs: &[(f64, f64)]) -> Result<Job> {
    let paper = parse_paper(p).map_err(other)?;
    if specs.len() > MAX_MIXED_PAIRS {
        return Err(other(format!("Dàn nhiều khổ nhận tối đa {MAX_MIXED_PAIRS} cặp mỗi lần; đang chọn {} cặp.", specs.len())));
    }
    let r = plan_mixed(specs, &paper).map_err(other)?;
    let targets = (r.pick_dw > EPS || r.pick_dh > EPS).then(|| {
        r.assignments
            .iter()
            .map(|(pi, slots)| {
                let (Some((f, b)), Some(spec), Some(first)) = (pairs.get(*pi), specs.get(*pi), slots.first()) else { return [(0.0, 0.0); 2] };
                let target = |it: &Item| {
                    let (w, h) = it.size();
                    (w * first.base_w / spec.0, h * first.base_h / spec.1)
                };
                [target(f), target(b)]
            })
            .collect()
    });
    let placed = r.best.placed.len();
    Ok(Job {
        mixed: true,
        message: mixed_message(&paper, &r, pairs.len()),
        count: placed,
        placed,
        leftover: pairs.len().saturating_sub(r.assignments.len()),
        squeeze: (r.pick_dw, r.pick_dh),
        paper,
        targets,
        margin: r.margin,
        usable_w: r.usable_w,
        center: (0.0, 0.0),
        assignments: r.assignments,
    })
}

fn tu_tro(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc().map_err(|_| other("Chưa mở tài liệu nào."))?;
    let d = &st.doc;
    // Như selection của Illustrator: object trên cùng trước.
    let mut roots = roots_in_paint_order(s)?;
    roots.reverse();
    if roots.is_empty() {
        return Err(other("Hãy chọn cặp mặt trước + mặt sau trước khi dàn."));
    }
    let mut found = vec![];
    let mut skipped = vec![];
    for (si, id) in roots.iter().enumerate() {
        let Some(n) = d.node(*id) else {
            skipped.push(format!("{} (không rõ loại)", si + 1));
            continue;
        };
        match bounds_of(n, d) {
            Some(r) => found.push(Item { id: *id, b: [r.x0, -r.y0, r.x1, -r.y1] }),
            None => skipped.push(format!("{} ({})", si + 1, n.kind_label())),
        }
    }
    if found.is_empty() {
        return Err(other("Selection hiện tại không có artwork hợp lệ để dàn."));
    }
    let bounds: Vec<[f64; 4]> = found.iter().map(|i| i.b).collect();
    let items: Vec<Item> = reading_order(&bounds).into_iter().filter_map(|i| found.get(i).copied()).collect();
    if items.len() < 2 || !items.len().is_multiple_of(2) {
        let mut info = format!(" Tool nhận {} object, đọc hợp lệ {}.", roots.len(), items.len());
        if !skipped.is_empty() {
            info.push_str(&format!(" Không đọc được khung: {}.", skipped.join(", ")));
        }
        return Err(other(format!("Dàn tự trở cần chọn số object chẵn: từng cặp trái = mặt trước, phải = mặt sau.{info}")));
    }
    let pairs: Vec<(Item, Item)> = items.chunks(2).filter_map(|c| Some((*c.first()?, *c.get(1)?))).collect();
    let Some((first, _)) = pairs.first() else { return Err(other("Không đo được kích thước bài mẫu.")) };
    let (source_w, source_h) = first.size();
    // Hai mặt của cùng một cặp phải cùng khung; cặp khác khổ cặp đầu thì sang bộ xếp nhiều khổ.
    let mut specs = Vec::with_capacity(pairs.len());
    let mut mixed = false;
    for (pi, (f, b)) in pairs.iter().enumerate() {
        let ((fw, fh), (bw, bh)) = (f.size(), b.size());
        if (fw - bw).abs() > SIZE_TOLERANCE || (fh - bh).abs() > SIZE_TOLERANCE {
            return Err(other(format!(
                "Cặp thứ {} có mặt trước và mặt sau lệch khung quá 0.5 mm. Hãy đưa riêng 2 mặt của cặp này về cùng khung trước khi dàn.",
                pi + 1
            )));
        }
        let spec = (fw.max(bw), fh.max(bh));
        if (spec.0 - source_w).abs() > SIZE_TOLERANCE || (spec.1 - source_h).abs() > SIZE_TOLERANCE {
            mixed = true;
        }
        specs.push(spec);
    }
    let job = if mixed { mixed_job(p, &pairs, &specs)? } else { single_job(p, &pairs, source_w, source_h)? };
    let sheet = sheet_place(d, job.paper.w, job.paper.h, job.mixed)?;
    let ids_of: Vec<(NodeId, NodeId)> = pairs.iter().map(|(f, b)| (f.id, b.id)).collect();
    let frame = Frame { origin: Point::new(sheet.x0, sheet.y0), paper_h: job.paper.h, shift: Vec2::new(job.center.0, -job.center.1) };
    let (ids, front_layer, board) = s.edit("Dàn Offset tự trở (DanCard)", |d, sel| {
        let (mut fronts, mut backs, mut gone) = (vec![], vec![], vec![]);
        for (k, (pi, slots)) in job.assignments.iter().enumerate() {
            let &(fid, bid) = ids_of.get(*pi).ok_or_else(|| other("Lỗi tạo vị trí dàn. Hãy thử lại."))?;
            let front = d.node(fid).cloned().ok_or(EngineError::NoNode(fid))?;
            let back = d.node(bid).cloned().ok_or(EngineError::NoNode(bid))?;
            // Bóp một lần cho mỗi mặt rồi nhân bản bản mẫu đó.
            let (front, back) = match job.targets.as_ref().and_then(|t| t.get(k)) {
                Some([ft, bt]) => (squeezed(&front, d, *ft, job.mixed)?, squeezed(&back, d, *bt, job.mixed)?),
                None => (front, back),
            };
            let (mut front_turns, mut back_turns) = (vec![], vec![]);
            for slot in slots {
                fronts.push(place(d, &mut front_turns, &front, slot, &frame)?);
            }
            for slot in slots {
                backs.push(place(d, &mut back_turns, &back, &mirror(slot, job.margin, job.usable_w), &frame)?);
            }
            gone.extend([fid, bid]);
        }
        // Cặp nào đã dàn thì bỏ nguồn; cặp chưa vừa giữ nguyên chỗ cũ.
        if !gone.is_empty() {
            d.remove_many(&gone)?;
        }
        let ids: Vec<NodeId> = fronts.iter().chain(backs.iter()).map(|n| n.id).collect();
        // Bản sao mới nằm dưới cùng layer (như PLACEATEND): bản đặt trước nằm trên.
        let front_layer = add_layer(d, FRONT_LAYER, fronts.into_iter().rev().collect(), false)?;
        add_layer(d, BACK_LAYER, backs.into_iter().rev().collect(), false)?;
        let marks = paper_pon(d, sheet);
        add_layer(d, PON_LAYER, marks, true)?;
        // Artboard đúng khổ giấy; số trong tên đếm các artboard có trước.
        let id = d.next_artboard_id();
        let name = if job.mixed { "Tự trở nhiều khổ".to_string() } else { format!("Tự trở {}", d.artboards.len() + 1) };
        let old: Vec<u32> = d.artboards.iter().map(|a| a.id).collect();
        d.artboards.push(Artboard { id, name: name.clone(), rect: sheet, show_center_mark: false, show_cross_hairs: false });
        // Như DanCard: dựng xong mới xoá khung các artboard cũ (bài bên trong giữ nguyên), kèm guide
        // của chúng.
        d.artboards.retain(|a| a.id == id);
        d.retain_guides(sel, |_, g| g.artboard.is_none_or(|a| !old.contains(&a)));
        sel.set(ids.iter().copied());
        Ok((ids, front_layer, name))
    })?;
    if let Some(st) = s.active_mut() {
        st.active_layer = Some(front_layer);
    }
    Ok(json!({
        "message": job.message,
        "count": job.count,
        "placed": job.placed,
        "leftover": job.leftover,
        "mixed": job.mixed,
        "squeezeMm": [js_round(job.squeeze.0 / MM * 100.0) / 100.0, js_round(job.squeeze.1 / MM * 100.0) / 100.0],
        "artboard": board,
        "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
#[path = "dancard_tutro_tests.rs"]
mod tests;
