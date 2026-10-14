//! DanCard — Dàn bế (tab Dàn tối ưu): dàn decal / tag theo khuôn bế, tự vẽ bốn chấm PON tròn
//! 5 mm theo khổ giấy nhập. Cùng quy tắc `dcDanBePrepare` → `DanBeNester.nest` → `dcDanBeRender`
//! của DanCard, gộp thành một lệnh và một bước hoàn tác.
//!
//! - Mẫu: chọn từng cặp Khuôn (trái) → Bài (phải) cùng hàng; dàn 2 mặt chọn từng bộ ba Khuôn →
//!   mặt trước → mặt sau. Không chọn gì thì lấy theo layer (Khuôn bế / Bài, hai mặt: Khuôn bế /
//!   Mặt trước / Mặt sau), ghép mỗi khuôn với bài gần nhất. Vai trò chỉ theo vị trí trên trang,
//!   không theo thứ tự chọn.
//! - Đường bế (path khép kín, compound path, group; clipping path cũng tính) được làm phẳng với
//!   dung sai 0,00025 mm rồi xếp bằng lõi silhouette (`dancard_be_nester.rs`): xoay 0/90/180/270°,
//!   khe giữa đường bế, lề, né PON tính từ mép chấm. Bài chỉ đi theo khuôn, không được đo khi xếp.
//! - Mặc định mỗi mẫu một tờ riêng; `multiPerArtboard` gộp mọi mẫu vào một tờ (số con mỗi mẫu
//!   lệch nhau tối đa 1).
//! - Vẽ: mỗi tờ một artboard "Dan be…", ba layer "Dàn bế - PON / Khuôn / Bài…" (Khuôn dưới cùng,
//!   PON trên cùng). Hai mặt: tờ mặt sau bên phải, lật ngang (khuôn lật gương, vị trí và PON đối
//!   xứng; bài mặt sau chỉ xoay ngược, không lật). Tờ mới đặt vào chỗ trống bên phải mọi artboard
//!   và hình đang có (DanCard đặt từ góc canvas). Như DanCard, mọi artboard cũ bị xoá; khuôn và bài
//!   nguồn giữ nguyên.

use std::cmp::Ordering;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Artboard, Document, Node, NodeId, NodeKind, Scaling};
use vectorcraft_geom::{Affine, PathData, Point, Rect, SubPath, Vec2, shapes};

use super::roots_in_paint_order;
use crate::cmd::*;
use crate::{EngineError, Result, Session};

#[path = "dancard_be_nester.rs"]
mod nester;

use nester::{ANGLES, Clock, DotIn, Groups, NestInput, NestOut, TypeIn};

const MM: f64 = 72.0 / 25.4;
/// Sai số so sánh kích thước (pt), như DanCard.
const EPS: f64 = 0.01;
/// Bán kính chấm PON (mm).
const PON_RADIUS: f64 = 2.5;
/// Dung sai làm phẳng đường cong (mm) và độ sâu chia đôi tối đa.
const FLATNESS: f64 = 0.00025;
const MAX_DEPTH: u32 = 18;
/// Số điểm tối đa của một khuôn sau khi làm phẳng.
const MAX_DIE_POINTS: usize = 300_000;
/// Khoảng cách giữa các tờ, giữa mặt trước và mặt sau (mm).
const PAGE_GAP: f64 = 10.0;
/// Bề ngang một hàng tờ (pt): như canvas 14400 pt của DanCard trừ lề hai bên.
const ROW_WIDTH: f64 = 14_400.0 - 2.0 * PAGE_GAP * MM;
/// Dấu nhận biết layer kết quả (Layer.note của DanCard).
const OUTPUT_NOTE: &str = "DANCARD_DANBE_OUTPUT";
const LABEL: &str = "Dàn bế (DanCard)";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.dieLayout",
        "DanCard: Dàn bế",
        [],
        None,
        "{paperW?, paperH?: cm (khổ tờ, mặc định 33 × 35,4), ponTop?, ponBottom?, ponLeft?, ponRight?: mm (từ mép tờ đến tâm chấm PON tròn 5 mm, mặc định 10), gap?: mm (khe giữa 2 đường bế, mặc định 2), margin?: mm (khuôn cách mép tờ, mặc định 4), ponClear?: mm (né PON tính từ mép chấm, mặc định 7,5), twoSided?: bool (dàn 2 mặt: bộ ba Khuôn → mặt trước → mặt sau; mặt sau lật ngang), multiPerArtboard?: bool (gộp mọi mẫu vào một tờ; mặc định mỗi mẫu một tờ)}; số nhận số JSON hoặc chuỗi kiểu \"7,5\", chuỗi trống lấy mặc định. Mẫu: chọn từng cặp Khuôn (trái) → Bài (phải) cùng hàng, hoặc không chọn gì để lấy theo layer Khuôn bế / Bài (2 mặt: Khuôn bế / Mặt trước / Mặt sau). Xếp đường bế thật (xoay 0/90/180/270°), vẽ 4 chấm PON, artboard, layer PON / Khuôn / Bài; tờ mới đặt bên phải những gì đang có, artboard cũ bị xoá như DanCard; một bước hoàn tác → {message, count, sheets, counts, twoSided, separate, plans: [{model, count, counts, mode, centred, offsetMm, detail}]}",
        has_doc,
        die_layout
    )]
}

fn fail(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

fn by(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn scaling() -> Scaling {
    Scaling { strokes: true, patterns: true, ..Scaling::default() }
}

// ---------------------------------------------------------------- tham số

/// Số thường có dấu tuỳ chọn: `^[+-]?(\d+(\.\d*)?|\.\d+)$`.
fn plain_number(t: &str) -> bool {
    let s = t.strip_prefix(['+', '-']).unwrap_or(t);
    let digits = |x: &str| x.chars().all(|c| c.is_ascii_digit());
    match s.split_once('.') {
        None => !s.is_empty() && digits(s),
        Some((int, frac)) => digits(int) && digits(frac) && !(int.is_empty() && frac.is_empty()),
    }
}

/// `numberValue` của Dàn bế: chuỗi bỏ khoảng trắng, dấu phẩy đầu tiên thành dấu chấm; trống thì
/// `dflt`; phải là số và nằm trong [`min`, `max`].
fn number_value(p: &Value, key: &str, label: &str, min: f64, max: f64, dflt: f64) -> std::result::Result<f64, String> {
    let n = match p.get(key) {
        None | Some(Value::Null) => return Ok(dflt),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let t = s.trim().replacen(',', ".", 1);
            if t.is_empty() {
                return Ok(dflt);
            }
            if !plain_number(&t) {
                return Err(format!("{label} phải là số."));
            }
            t.parse::<f64>().ok()
        }
        Some(_) => None,
    };
    let n = n.filter(|v| v.is_finite()).ok_or_else(|| format!("{label} phải là số."))?;
    if !(min..=max).contains(&n) {
        return Err(format!("{label} nằm ngoài phạm vi cho phép."));
    }
    Ok(n)
}

/// Thông số đã kiểm tra (mm).
struct Opts {
    gap: f64,
    margin: f64,
    pon_clear: f64,
    paper_w: f64,
    paper_h: f64,
    /// Từ mép tờ đến tâm chấm: trên, dưới, trái, phải.
    pon: [f64; 4],
    two_sided: bool,
    multi: bool,
}

fn opts(p: &Value) -> std::result::Result<Opts, String> {
    let gap = number_value(p, "gap", "Khe giữa 2 khuôn", 0.0, 50.0, 2.0)?;
    let margin = number_value(p, "margin", "Khuôn cách mép tờ", 0.0, 100.0, 4.0)?;
    let pon_clear = number_value(p, "ponClear", "Né PON", 0.0, 100.0, 7.5)?;
    let paper_w = number_value(p, "paperW", "Rộng tờ (cm)", 0.5, 5000.0, 33.0)? * 10.0;
    let paper_h = number_value(p, "paperH", "Cao tờ (cm)", 0.5, 5000.0, 35.4)? * 10.0;
    let top = number_value(p, "ponTop", "PON cách trên (mm)", 2.5, 50000.0, 10.0)?;
    let bottom = number_value(p, "ponBottom", "PON cách dưới (mm)", 2.5, 50000.0, 10.0)?;
    let left = number_value(p, "ponLeft", "PON cách trái (mm)", 2.5, 50000.0, 10.0)?;
    let right = number_value(p, "ponRight", "PON cách phải (mm)", 2.5, 50000.0, 10.0)?;
    // Như DanCard, so sánh theo point.
    let (w, h, r) = (paper_w * MM, paper_h * MM, PON_RADIUS * MM);
    if w - left * MM - right * MM < r * 2.0 - 0.0000001 || h - top * MM - bottom * MM < r * 2.0 - 0.0000001 {
        return Err("Khổ tờ và khoảng PON không hợp lệ: bốn chấm phải nằm trong tờ, không chồng nhau.".into());
    }
    if margin * MM * 2.0 >= w.min(h) - EPS {
        return Err("Lề khuôn quá lớn, không còn vùng dàn trong tờ.".into());
    }
    Ok(Opts {
        gap,
        margin,
        pon_clear,
        paper_w,
        paper_h,
        pon: [top, bottom, left, right],
        two_sided: bool_or(p, "twoSided", false),
        multi: bool_or(p, "multiPerArtboard", false),
    })
}

// ---------------------------------------------------------------- mẫu nguồn

/// Một object nguồn: tâm và chiều cao khung (pt, y hướng xuống).
#[derive(Clone, Copy, Debug)]
struct Rec {
    id: NodeId,
    cx: f64,
    cy: f64,
    h: f64,
}

#[derive(Clone, Copy, Debug)]
struct Model {
    khuon: NodeId,
    bai: NodeId,
    sau: Option<NodeId>,
}

fn geometric_first(n: &Node) -> Option<Rect> {
    n.geometric_bounds().or_else(|| n.visual_bounds())
}

fn visible_first(n: &Node) -> Option<Rect> {
    n.visual_bounds().or_else(|| n.geometric_bounds())
}

fn measurable(b: Rect) -> bool {
    b.width() > EPS && b.height() > EPS
}

fn rec_of(id: NodeId, b: Rect) -> Rec {
    Rec { id, cx: (b.x0 + b.x1) / 2.0, cy: (b.y0 + b.y1) / 2.0, h: b.height() }
}

/// Một mặt: từng cặp theo hàng (trên xuống), trong hàng trái → phải: Khuôn rồi Bài. Như DanCard,
/// ngưỡng hàng là nửa chiều cao của object trên cùng và hàng lẻ ghép sang hàng sau.
fn pair_models(d: &Document, roots: &[NodeId]) -> std::result::Result<Vec<Model>, String> {
    const ODD: &str = "Hãy chọn số object chẵn: từng cặp Khuôn bên trái, Bài bên phải.";
    let mut recs: Vec<Rec> = roots
        .iter()
        .filter_map(|id| {
            let b = geometric_first(d.node(*id)?)?;
            measurable(b).then(|| rec_of(*id, b))
        })
        .collect();
    if recs.len() < 2 || !recs.len().is_multiple_of(2) {
        return Err(ODD.into());
    }
    recs.sort_by(|a, b| by(a.cy, b.cy));
    let Some(&first) = recs.first() else { return Err(ODD.into()) };
    let threshold = first.h * 0.5;
    let mut flat: Vec<Rec> = Vec::with_capacity(recs.len());
    let flush = |row: &mut Vec<Rec>, flat: &mut Vec<Rec>| {
        row.sort_by(|a, b| by(a.cx, b.cx));
        flat.append(row);
    };
    let mut row = vec![first];
    let mut anchor = first.cy;
    for r in recs.iter().skip(1) {
        if r.cy - anchor <= threshold {
            row.push(*r);
        } else {
            flush(&mut row, &mut flat);
            row.push(*r);
            anchor = r.cy;
        }
    }
    flush(&mut row, &mut flat);
    let mut models = Vec::with_capacity(flat.len() / 2);
    for p in flat.chunks_exact(2) {
        if let [k, b] = p {
            models.push(Model { khuon: k.id, bai: b.id, sau: None });
        }
    }
    Ok(models)
}

/// Hai mặt: bộ ba Khuôn → mặt trước → mặt sau trong mỗi hàng, trái sang phải.
fn triple_models(d: &Document, roots: &[NodeId]) -> std::result::Result<Vec<Model>, String> {
    if roots.is_empty() || !roots.len().is_multiple_of(3) {
        return Err("Dàn bế 2 mặt cần chọn đủ bộ 3: Khuôn bên trái → mặt trước → mặt sau, cùng hàng.".into());
    }
    let mut pending: Vec<Rec> = Vec::with_capacity(roots.len());
    for (i, id) in roots.iter().enumerate() {
        let Some(b) = d.node(*id).and_then(geometric_first).filter(|b| measurable(*b)) else {
            return Err(format!("Không đo được object {} trong bộ Khuôn/trước/sau.", i + 1));
        };
        pending.push(rec_of(*id, b));
    }
    pending.sort_by(|a, b| by(a.cy, b.cy).then(by(a.cx, b.cx)));
    let mut models = Vec::new();
    while !pending.is_empty() {
        let anchor = pending.remove(0);
        let mut row = vec![anchor];
        let (mut high, mut low, mut min_h) = (anchor.cy, anchor.cy, anchor.h);
        let mut i = 0;
        while let Some(c) = pending.get(i).copied() {
            let height = min_h.min(c.h);
            let (next_high, next_low) = (high.max(c.cy), low.min(c.cy));
            if next_high - next_low <= height * 0.25 + EPS {
                row.push(c);
                pending.remove(i);
                high = next_high;
                low = next_low;
                min_h = height;
            } else {
                i += 1;
            }
        }
        if !row.len().is_multiple_of(3) {
            return Err("Hàng nguồn chưa đủ Khuôn → mặt trước → mặt sau. Hãy canh hàng và chọn đủ 3 object cho mỗi mẫu.".into());
        }
        row.sort_by(|a, b| by(a.cx, b.cx));
        for t in row.chunks_exact(3) {
            let [k, f, s] = t else { continue };
            if f.cx - k.cx <= EPS || s.cx - f.cx <= EPS {
                return Err("Không phân biệt được thứ tự trái/phải. Đặt Khuôn → mặt trước → mặt sau cạnh nhau.".into());
            }
            models.push(Model { khuon: k.id, bai: f.id, sau: Some(s.id) });
        }
    }
    Ok(models)
}

#[derive(Clone, Copy)]
enum Role {
    Cut,
    Art,
    Front,
    Back,
}

fn base_letter(c: char) -> char {
    match c {
        'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ' | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' => 'a',
        'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' => 'e',
        'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' => 'i',
        'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ' | 'ớ' | 'ợ' | 'ở' | 'ỡ' => 'o',
        'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' => 'u',
        'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' => 'y',
        'đ' => 'd',
        _ => c,
    }
}

/// Chữ thường, bỏ dấu tiếng Việt (chữ dựng sẵn).
fn normalise(s: &str) -> String {
    s.to_lowercase().chars().map(base_letter).collect()
}

/// Layer kết quả của một lần Dàn bế trước.
fn is_generated(layer: &Node) -> bool {
    normalise(layer.name.as_deref().unwrap_or("")).starts_with("dan be -") || layer.extras().attrs.as_deref().is_some_and(|a| a.note == OUTPUT_NOTE)
}

/// Layer cấp đầu đầu tiên (trên xuống) có tên chứa từ khoá của vai trò.
fn find_layer(d: &Document, role: Role) -> Option<&Arc<Node>> {
    let keys: &[&str] = match role {
        Role::Cut => &["khuon", "die", "cut"],
        Role::Art => &["bai", "object", "artwork", "hinh"],
        Role::Front => &["mat truoc", "front"],
        Role::Back => &["mat sau", "back"],
    };
    d.layers.iter().rev().filter(|l| !is_generated(l)).find(|l| {
        let n = normalise(l.name.as_deref().unwrap_or(""));
        keys.iter().any(|k| n.contains(k))
    })
}

/// Các object cấp đầu đang hiện của layer, đo được.
fn layer_items(layer: &Node, geometric: bool) -> Vec<Rec> {
    if !layer.visible {
        return Vec::new();
    }
    layer
        .children()
        .into_iter()
        .flatten()
        .filter(|c| !c.is_layer() && c.visible)
        .filter_map(|c| {
            let b = if geometric { geometric_first(c) } else { visible_first(c) }?;
            measurable(b).then(|| rec_of(c.id, b))
        })
        .collect()
}

/// Object chưa dùng gần `cut` nhất (bằng nhau thì cái trước).
fn nearest_unused(cut: &Rec, items: &[Rec], used: &mut [bool]) -> Option<NodeId> {
    let mut best: Option<usize> = None;
    let mut best_d = 1e99;
    for (i, it) in items.iter().enumerate() {
        if used.get(i).copied().unwrap_or(true) {
            continue;
        }
        let (dx, dy) = (it.cx - cut.cx, it.cy - cut.cy);
        let dist = dx * dx + dy * dy;
        if dist < best_d {
            best_d = dist;
            best = Some(i);
        }
    }
    let i = best?;
    if let Some(u) = used.get_mut(i) {
        *u = true;
    }
    items.get(i).map(|r| r.id)
}

fn source_models(d: &Document, roots: &[NodeId], two: bool) -> std::result::Result<Vec<Model>, String> {
    if two {
        if !roots.is_empty() {
            return triple_models(d, roots);
        }
        const MISSING: &str = "Dàn bế 2 mặt: chọn từng bộ Khuôn → mặt trước → mặt sau, hoặc dùng 3 layer Khuôn bế, Mặt trước, Mặt sau.";
        const UNEVEN: &str = "Ba layer Khuôn bế, Mặt trước, Mặt sau cần cùng số mẫu, không được thiếu mặt.";
        let (Some(cut), Some(front), Some(back)) = (find_layer(d, Role::Cut), find_layer(d, Role::Front), find_layer(d, Role::Back)) else {
            return Err(MISSING.into());
        };
        if cut.id == front.id || cut.id == back.id || front.id == back.id {
            return Err(MISSING.into());
        }
        let (cuts, fronts, backs) = (layer_items(cut, true), layer_items(front, false), layer_items(back, false));
        if cuts.is_empty() || fronts.len() != cuts.len() || backs.len() != cuts.len() {
            return Err(UNEVEN.into());
        }
        let (mut used_f, mut used_b) = (vec![false; fronts.len()], vec![false; backs.len()]);
        let mut models = Vec::with_capacity(cuts.len());
        for c in &cuts {
            let (Some(bai), Some(sau)) = (nearest_unused(c, &fronts, &mut used_f), nearest_unused(c, &backs, &mut used_b)) else {
                return Err(UNEVEN.into());
            };
            models.push(Model { khuon: c.id, bai, sau: Some(sau) });
        }
        return Ok(models);
    }
    if roots.len() >= 2 {
        return pair_models(d, roots);
    }
    const NO_LAYERS: &str = "Chọn từng cặp Khuôn/Bài, hoặc tạo hai layer Khuôn bế và Bài.";
    let (Some(cut), Some(art)) = (find_layer(d, Role::Cut), find_layer(d, Role::Art)) else {
        return Err(NO_LAYERS.into());
    };
    if cut.id == art.id {
        return Err(NO_LAYERS.into());
    }
    let (cuts, arts) = (layer_items(cut, true), layer_items(art, false));
    if cuts.is_empty() || arts.is_empty() {
        return Err("Layer Khuôn bế hoặc Bài đang trống.".into());
    }
    let mut used = vec![false; arts.len()];
    let mut models = Vec::with_capacity(cuts.len());
    for c in &cuts {
        let bai = nearest_unused(c, &arts, &mut used).ok_or_else(|| "Số Bài ít hơn số Khuôn bế.".to_string())?;
        models.push(Model { khuon: c.id, bai, sau: None });
    }
    Ok(models)
}

// ---------------------------------------------------------------- đọc đường bế

/// Đường bế của một mẫu: đường bao mm (y hướng lên, khung từ 0), có đoạn cong không, khung các
/// điểm đã làm phẳng trên trang (pt).
struct Die {
    groups: Groups,
    curve: bool,
    bbox: Rect,
}

type PtGroups = Vec<Vec<Vec<Point>>>;

/// Khoảng cách từ `p` tới dây cung hữu hạn `a`–`b`.
fn chord_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let d2 = dx * dx + dy * dy;
    if d2 < 0.000001 {
        let (ex, ey) = (p.x - a.x, p.y - a.y);
        return (ex * ex + ey * ey).sqrt();
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / d2).clamp(0.0, 1.0);
    let (ex, ey) = (p.x - a.x - t * dx, p.y - a.y - t * dy);
    (ex * ex + ey * ey).sqrt()
}

/// Chia đôi (de Casteljau t = ½) tới khi tay nắm cách dây cung không quá dung sai.
fn flatten_bezier(p0: Point, c1: Point, c2: Point, p3: Point, out: &mut Vec<Point>, depth: u32) {
    let flat = chord_distance(c1, p0, p3).max(chord_distance(c2, p0, p3));
    if flat <= FLATNESS * MM || depth >= MAX_DEPTH || out.len() > MAX_DIE_POINTS {
        out.push(p3);
        return;
    }
    let mid = |a: Point, b: Point| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let (p01, p12, p23) = (mid(p0, c1), mid(c1, c2), mid(c2, p3));
    let (p012, p123) = (mid(p01, p12), mid(p12, p23));
    let p0123 = mid(p012, p123);
    flatten_bezier(p0, p01, p012, p0123, out, depth + 1);
    flatten_bezier(p0123, p123, p23, p3, out, depth + 1);
}

/// Một đường bao kín đã làm phẳng (pt). Đường hở ngắn (dưới 3 điểm) bị bỏ qua; đường hở dài là
/// lỗi như DanCard. `curve`: có đoạn nào có tay nắm.
fn sub_contour(sp: &SubPath, curve: &mut bool) -> std::result::Result<Option<Vec<Point>>, String> {
    let pts = &sp.anchors;
    if pts.is_empty() || (!sp.closed && pts.len() < 3) {
        return Ok(None);
    }
    if !sp.closed {
        return Err("Khuôn bế phải là path khép kín; không dùng khung bao thay thế.".into());
    }
    let n = pts.len();
    let mut out: Vec<Point> = Vec::new();
    for (i, a) in pts.iter().enumerate() {
        let Some(b) = pts.get((i + 1) % n) else { continue };
        if a.h_out != a.p || b.h_in != b.p {
            *curve = true;
        }
        if i == 0 {
            out.push(a.p);
        }
        flatten_bezier(a.p, a.h_out, b.h_in, b.p, &mut out, 0);
        if out.len() > MAX_DIE_POINTS {
            return Err("Khuôn bế có quá nhiều điểm sau khi làm phẳng; hãy đơn giản hoá đường bế.".into());
        }
    }
    if out.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err("Khuôn bế có toạ độ không hợp lệ.".into());
    }
    if out.len() > 1
        && let (Some(f), Some(l)) = (out.first(), out.last())
        && (l.x - f.x).abs() < 0.00001
        && (l.y - f.y).abs() < 0.00001
    {
        out.pop();
    }
    if out.len() < 3 {
        return Err("Đường kín của khuôn không tạo được biên dạng; hãy kiểm tra đường bao ngoài.".into());
    }
    Ok(Some(out))
}

fn path_contours(path: &PathData, curve: &mut bool) -> std::result::Result<Vec<Vec<Point>>, String> {
    let mut out = Vec::new();
    for sp in &path.subpaths {
        if let Some(c) = sub_contour(sp, curve)? {
            out.push(c);
        }
    }
    Ok(out)
}

/// Các nhóm đường bao của khuôn: một path (mọi subpath của nó) hay một compound path là một nhóm
/// even-odd; group đi vào từng con (clipping path cũng là đường bế). Chữ, ảnh, guide, object ẩn
/// bị bỏ qua.
fn collect_groups(n: &Node, out: &mut PtGroups, curve: &mut bool) -> std::result::Result<(), String> {
    if !n.visible {
        return Ok(());
    }
    match &n.kind {
        NodeKind::Path { guide: true, .. } => {}
        NodeKind::Path { path, .. } => {
            let contours = path_contours(path, curve)?;
            if !contours.is_empty() {
                out.push(contours);
            }
        }
        NodeKind::Compound { children, .. } => {
            let mut group = Vec::new();
            for c in children {
                if let NodeKind::Path { path, .. } = &c.kind {
                    group.extend(path_contours(path, curve)?);
                }
            }
            if !group.is_empty() {
                out.push(group);
            }
        }
        NodeKind::Group { children, .. } => {
            for c in children {
                collect_groups(c, out, curve)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn read_die(n: &Node, mi: usize) -> std::result::Result<Die, String> {
    let mut groups: PtGroups = Vec::new();
    let mut curve = false;
    collect_groups(n, &mut groups, &mut curve)?;
    if groups.iter().flatten().map(Vec::len).sum::<usize>() > MAX_DIE_POINTS {
        return Err("Khuôn bế có quá nhiều điểm sau khi làm phẳng; hãy đơn giản hoá đường bế.".into());
    }
    let none = || format!("Không đọc được path khép kín của khuôn {}.", mi + 1);
    let mut pts = groups.iter().flatten().flatten();
    let first = *pts.next().ok_or_else(none)?;
    let bbox = pts.fold(Rect::from_points(first, first), |r, p| r.union_pt(*p));
    // mm, y hướng lên, khung bắt đầu ở (0, 0).
    let mm: Groups = groups
        .iter()
        .map(|g| {
            g.iter()
                .map(|c| c.iter().map(|p| [(p.x - bbox.x0) / MM, (bbox.y1 - p.y) / MM]).collect::<Vec<_>>())
                .filter(|c| c.len() >= 3)
                .collect::<Vec<_>>()
        })
        .filter(|g| !g.is_empty())
        .collect();
    if mm.is_empty() {
        return Err(none());
    }
    Ok(Die { groups: mm, curve, bbox })
}

// ---------------------------------------------------------------- tìm cách dàn

/// Kết quả dàn của một tờ; `model`: mẫu của tờ riêng.
struct SheetPlan {
    model: Option<usize>,
    out: NestOut,
}

fn nest_input(o: &Opts, prepared: &[(Model, Die)]) -> NestInput {
    let (w, h) = (o.paper_w, o.paper_h);
    let [top, bottom, left, right] = o.pon;
    // Trên trái, trên phải, dưới trái, dưới phải; y tính lên từ mép dưới tờ.
    let dots = vec![
        DotIn { x: left, y: h - top, r: PON_RADIUS },
        DotIn { x: w - right, y: h - top, r: PON_RADIUS },
        DotIn { x: left, y: bottom, r: PON_RADIUS },
        DotIn { x: w - right, y: bottom, r: PON_RADIUS },
    ];
    NestInput {
        width: w,
        height: h,
        dots,
        types: prepared.iter().map(|(_, d)| TypeIn { groups: d.groups.clone(), curve_error: if d.curve { FLATNESS } else { 0.0 } }).collect(),
        gap: o.gap,
        margin: o.margin,
        pon_clear: o.pon_clear,
        step: 0.25,
        budget_ms: 3000.0,
    }
}

/// Như panel DanCard: tờ riêng thì mỗi mẫu một lần dàn (mẫu cùng khuôn dùng lại kết quả), gộp thì
/// một lần cho mọi mẫu.
fn plan_sheets(input: &NestInput, separate: bool) -> Result<Vec<SheetPlan>> {
    let clock = Clock::default();
    if !separate {
        let out = nester::nest(input, &clock).map_err(fail)?;
        if out.slots.is_empty() {
            return Err(fail("Mẫu 1 không có vị trí dàn hợp lệ."));
        }
        return Ok(vec![SheetPlan { model: None, out }]);
    }
    let mut cache: Vec<(&Groups, NestOut)> = Vec::new();
    let mut plans = Vec::with_capacity(input.types.len());
    for (i, t) in input.types.iter().enumerate() {
        let out = match cache.iter().find(|(g, _)| **g == t.groups) {
            Some((_, o)) => o.clone(),
            None => {
                let one = NestInput {
                    width: input.width,
                    height: input.height,
                    dots: input.dots.clone(),
                    types: vec![t.clone()],
                    gap: input.gap,
                    margin: input.margin,
                    pon_clear: input.pon_clear,
                    step: input.step,
                    budget_ms: input.budget_ms,
                };
                let o = nester::nest(&one, &clock).map_err(fail)?;
                cache.push((&t.groups, o.clone()));
                o
            }
        };
        if out.slots.is_empty() {
            return Err(fail(format!("Mẫu {} không có vị trí dàn hợp lệ.", i + 1)));
        }
        let slots = out.slots.iter().map(|s| nester::OutSlot { mi: i, ..*s }).collect();
        plans.push(SheetPlan { model: Some(i), out: NestOut { slots, ..out } });
    }
    Ok(plans)
}

// ---------------------------------------------------------------- vẽ

/// Xoay ngược chiều kim đồng hồ (như nhìn trên màn hình) `deg` (bội của 90°) quanh `c`, ma trận đúng.
fn turn(deg: i64, c: Point) -> Affine {
    let (cos, sin) = match deg.rem_euclid(360) {
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => (1.0, 0.0),
    };
    // y hướng xuống: (x, y) → (x cosθ + y sinθ, −x sinθ + y cosθ).
    Affine::translate(c.to_vec2()) * Affine::new([cos, -sin, sin, cos, 0.0, 0.0]) * Affine::translate(-c.to_vec2())
}

/// Các vị trí tờ (mặt trước): bên phải mọi artboard và hình đang có, trái sang phải, hết hàng thì
/// xuống dưới; hai mặt thì mặt sau ngay bên phải mặt trước.
fn sheet_positions(d: &Document, w: f64, h: f64, two: bool, count: usize) -> Result<Vec<Rect>> {
    let gap = PAGE_GAP * MM;
    let bundle_w = if two { 2.0 * w + gap } else { w };
    let used = d.artboards.iter().fold(d.art_bounds(), |acc, a| vectorcraft_geom::union_opt(acc, Some(a.rect)));
    let (left, top) = used.map_or((0.0, 0.0), |u| (u.x1 + gap, u.y0));
    let (mut x, mut y) = (left, top);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if x > left && x + bundle_w > left + ROW_WIDTH {
            x = left;
            y += h + gap;
        }
        out.push(Rect::new(x, y, x + w, y + h));
        x += bundle_w + gap;
    }
    let inside = |v: f64| v.is_finite() && v.abs() <= crate::MAX_COORD;
    if !out.iter().all(|r| inside(r.x0) && inside(r.y0) && inside(r.x0 + bundle_w) && inside(r.y1)) {
        return Err(fail("Không còn chỗ trong canvas để tạo đủ tờ. Hãy bớt artboard cũ."));
    }
    Ok(out)
}

fn unique_layer_name(d: &Document, base: &str) -> String {
    let taken = |s: &str| d.layers.iter().any(|l| l.name.as_deref() == Some(s));
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|k| format!("{base} {k}")).find(|c| !taken(c)).unwrap_or_else(|| base.to_string())
}

/// Layer kết quả mới trên cùng (tên không trùng, mở khoá, đang hiện, có dấu nhận biết).
fn output_layer(d: &mut Document, base: &str) -> Result<NodeId> {
    let name = unique_layer_name(d, base);
    let mut n = crate::cmd::layer::make_layer(d, Some(name.as_str()));
    n.locked = false;
    n.visible = true;
    n.edit_attrs(|a| a.note = OUTPUT_NOTE.into());
    Ok(d.insert(None, usize::MAX, n)?)
}

fn fill_layer(d: &mut Document, layer: NodeId, nodes: Vec<Node>) {
    if let Some(ch) = d.node_mut(layer).and_then(Node::children_mut) {
        ch.extend(nodes.into_iter().map(Arc::new));
    }
}

struct Face {
    pon: NodeId,
    cut: NodeId,
    art: NodeId,
}

/// Artboard "Dan be…" và ba layer PON / Khuôn / Bài, có sẵn bốn chấm PON (mặt sau: lật ngang).
fn add_face(d: &mut Document, rect: Rect, suffix: &str, back: bool, o: &Opts) -> Result<Face> {
    let id = d.next_artboard_id();
    d.artboards.push(Artboard { id, name: format!("Dan be{suffix}"), rect, show_center_mark: false, show_cross_hairs: false });
    let pon = output_layer(d, &format!("Dàn bế - PON{suffix}"))?;
    let cut = output_layer(d, &format!("Dàn bế - Khuôn{suffix}"))?;
    let art = output_layer(d, &format!("Dàn bế - Bài{suffix}"))?;
    let (w, h, r) = (rect.width(), rect.height(), PON_RADIUS * MM);
    let [top, bottom, left, right] = o.pon.map(|v| v * MM);
    let mut dots = Vec::with_capacity(4);
    for (dx, dy) in [(left, top), (w - right, top), (left, h - bottom), (w - right, h - bottom)] {
        let dx = if back { w - dx } else { dx };
        let c = Point::new(rect.x0 + dx, rect.y0 + dy);
        let ink = Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0));
        dots.push(Node::path(d.alloc_id(), shapes::ellipse(Rect::new(c.x - r, c.y - r, c.x + r, c.y + r)), Appearance::basic(ink, Paint::None, 0.0)));
    }
    fill_layer(d, pon, dots);
    Ok(Face { pon, cut, art })
}

/// Khuôn xuống dưới cùng của cả chồng layer, PON lên trên cùng.
fn finish_face(d: &mut Document, f: &Face) {
    if let Some(i) = d.layers.iter().position(|l| l.id == f.cut) {
        let l = d.layers.remove(i);
        d.layers.insert(0, l);
    }
    if let Some(i) = d.layers.iter().position(|l| l.id == f.pon) {
        let l = d.layers.remove(i);
        d.layers.push(l);
    }
}

/// Bản sao của bài `id` xoay `deg`, tâm khung nhìn thấy đặt tại `at`.
fn art_copy(d: &mut Document, id: NodeId, deg: i64, at: Point) -> Result<Node> {
    let src = d.node(id).cloned().ok_or(EngineError::NoNode(id))?;
    let mut n = d.reid(&src);
    if deg.rem_euclid(360) != 0
        && let Some(b) = visible_first(&n)
    {
        n.transform(turn(deg, b.center()), scaling());
    }
    let b = visible_first(&n).ok_or_else(|| fail("Không đo được Bài để canh tâm theo khuôn."))?;
    n.transform(Affine::translate(at - b.center()), scaling());
    Ok(n)
}

struct Rendered {
    total: usize,
    counts: Vec<String>,
}

/// Vẽ các tờ đã dàn (như `dcDanBeRender`), rồi xoá các artboard cũ.
fn render(d: &mut Document, models: &[(Model, Die)], o: &Opts, separate: bool, sheets: &[SheetPlan]) -> Result<Rendered> {
    let (w, h) = (o.paper_w * MM, o.paper_h * MM);
    let two = o.two_sided;
    if sheets.is_empty() {
        return Err(fail("Bộ tối ưu không tìm được vị trí hợp lệ."));
    }
    if separate && sheets.len() != models.len() {
        return Err(fail("Thiếu tờ riêng cho mẫu nguồn. Hãy dàn lại."));
    }
    // Kiểm tra cả lô trước khi tạo gì.
    let mut seen = vec![false; models.len()];
    let mut total = 0;
    let mut counts = Vec::with_capacity(sheets.len());
    for (k, sheet) in sheets.iter().enumerate() {
        if sheet.out.slots.is_empty() {
            return Err(fail(format!("Mẫu {} không có vị trí dàn hợp lệ.", k + 1)));
        }
        if separate {
            let flag = match sheet.model {
                Some(m) => seen.get_mut(m),
                None => None,
            };
            let Some(flag) = flag else {
                return Err(fail("Danh sách mẫu/tờ riêng không hợp lệ."));
            };
            if *flag {
                return Err(fail("Danh sách mẫu/tờ riêng không hợp lệ."));
            }
            *flag = true;
        }
        let bad_slot = sheet.out.slots.iter().any(|s| {
            s.mi >= models.len()
                || (separate && sheet.model != Some(s.mi))
                || s.vi > 3
                || !(s.x.is_finite() && s.y.is_finite())
                || s.x < 0.0
                || s.y < 0.0
                || s.x * MM >= w
                || s.y * MM >= h
        });
        if bad_slot {
            return Err(fail(format!("Dữ liệu vị trí dàn không hợp lệ ở tờ {}.", k + 1)));
        }
        total += sheet.out.slots.len();
        counts.push(format!("Mẫu {}: {} con", sheet.model.unwrap_or(0) + 1, sheet.out.slots.len()));
    }
    if two && let Some(i) = models.iter().position(|(m, _)| m.sau.is_none()) {
        return Err(fail(format!("Thiếu mặt sau của mẫu {}. Hãy chọn đủ Khuôn → trước → sau.", i + 1)));
    }
    let positions = sheet_positions(d, w, h, two, sheets.len())?;
    let first_new = d.artboards.len();
    let gap = PAGE_GAP * MM;
    for (sheet, &rect) in sheets.iter().zip(&positions) {
        let suffix = match (separate, sheet.model) {
            (true, Some(m)) => format!(" - mẫu {}", m + 1),
            _ => String::new(),
        };
        let front = add_face(d, rect, &format!("{suffix}{}", if two { " - mặt trước" } else { "" }), false, o)?;
        let (mut cuts, mut arts, mut placed) = (Vec::new(), Vec::new(), Vec::new());
        for s in &sheet.out.slots {
            let (model, die) = models.get(s.mi).ok_or_else(|| fail("Dữ liệu vị trí dàn không hợp lệ."))?;
            let deg = ANGLES.get(s.vi).copied().unwrap_or(0);
            // Đặt theo khung các điểm đường bế (đúng thứ lõi dàn đã xếp), không theo khung cả object.
            let rot = turn(deg, die.bbox.center());
            let rb = rot.transform_rect_bbox(die.bbox);
            let shift = Vec2::new(rect.x0 + s.x * MM - rb.x0, rect.y1 - s.y * MM - rb.y1);
            let src = d.node(model.khuon).cloned().ok_or(EngineError::NoNode(model.khuon))?;
            let mut cut = d.reid(&src);
            cut.transform(Affine::translate(shift) * rot, scaling());
            let at = (rb + shift).center();
            arts.push(art_copy(d, model.bai, deg, at)?);
            if two {
                placed.push((cut.clone(), *model, deg, at));
            }
            cuts.push(cut);
        }
        // Như DanCard (PLACEATEND): mỗi bản sau nằm dưới bản trước.
        fill_layer(d, front.cut, cuts.into_iter().rev().collect());
        fill_layer(d, front.art, arts.into_iter().rev().collect());
        finish_face(d, &front);
        if two {
            let back_rect = Rect::new(rect.x1 + gap, rect.y0, rect.x1 + gap + w, rect.y1);
            let back = add_face(d, back_rect, &format!("{suffix} - mặt sau"), true, o)?;
            let (mut cuts, mut arts) = (Vec::new(), Vec::new());
            for (cut, model, deg, c) in placed {
                // Lật tờ theo chiều ngang: tâm đối xứng qua trục dọc giữa tờ, cùng độ cao.
                let back_x = back_rect.x0 + w - (c.x - rect.x0);
                let mut k = d.reid(&cut);
                // Lật gương khuôn mặt trước quanh tâm của nó, rồi đưa tâm tới (back_x, c.y).
                k.transform(Affine::new([-1.0, 0.0, 0.0, 1.0, c.x + back_x, 0.0]), scaling());
                cuts.push(k);
                let sau = model.sau.ok_or_else(|| fail("Thiếu mặt sau của mẫu. Hãy chọn đủ Khuôn → trước → sau."))?;
                // Bài mặt sau xoay ngược lại, không bao giờ lật gương.
                arts.push(art_copy(d, sau, -deg, Point::new(back_x, c.y))?);
            }
            fill_layer(d, back.cut, cuts.into_iter().rev().collect());
            fill_layer(d, back.art, arts.into_iter().rev().collect());
            finish_face(d, &back);
        }
    }
    // Vẽ xong cả lô mới xoá các artboard cũ (khuôn, bài nguồn giữ nguyên).
    let kept = d.artboards.split_off(first_new.min(d.artboards.len()));
    d.artboards = kept;
    Ok(Rendered { total, counts })
}

// ---------------------------------------------------------------- lệnh

fn die_layout(s: &mut Session, p: &Value) -> Result<Value> {
    if s.active().is_none() {
        return Err(fail("Chưa mở tài liệu nào."));
    }
    let o = opts(p).map_err(fail)?;
    let roots = roots_in_paint_order(s)?;
    let st = s.doc()?;
    let models = source_models(&st.doc, &roots, o.two_sided).map_err(fail)?;
    let mut prepared = Vec::with_capacity(models.len());
    for (mi, m) in models.iter().enumerate() {
        let n = st.doc.node(m.khuon).ok_or(EngineError::NoNode(m.khuon))?;
        prepared.push((*m, read_die(n, mi).map_err(fail)?));
    }
    if prepared.is_empty() {
        return Err(fail("Không có mẫu khuôn hợp lệ để dàn."));
    }
    let input = nest_input(&o, &prepared);
    let separate = !o.multi && input.types.len() > 1;
    let sheets = plan_sheets(&input, separate)?;
    let two = o.two_sided;
    let done = s.edit(LABEL, |d, sel| {
        let r = render(d, &prepared, &o, separate, &sheets)?;
        sel.clear();
        Ok(r)
    })?;
    let n = sheets.len();
    let mut detail = if separate {
        format!("Đã tạo {n}{}; tổng {} con. {}.", if two { " cặp artboard trước/sau" } else { " artboard riêng" }, done.total, done.counts.join("; "))
    } else {
        sheets.first().map_or_else(|| format!("Dàn silhouette thật: {} con.", done.total), |p| p.out.detail.clone())
    };
    detail.push_str(" Khuôn và Bài nằm ở layer riêng; PON giữ ở layer trên cùng.");
    if two {
        if !separate {
            detail.push_str(&format!(" Đã tạo {n} cặp artboard trước/sau."));
        }
        detail.push_str(" Hai mặt dùng chung bố cục khuôn, đối xứng lật ngang; không lật gương chữ/hình.");
    }
    let message = if separate { format!("OK: {detail}") } else { format!("OK: {} con / tờ\n{detail}", done.total) };
    let plans: Vec<Value> = sheets
        .iter()
        .map(|p| {
            json!({
                "model": p.model.map(|m| m + 1),
                "count": p.out.slots.len(),
                "counts": p.out.counts,
                "mode": p.out.mode,
                "centred": p.out.exact,
                "offsetMm": [p.out.offset.0, p.out.offset.1],
                "detail": p.out.detail,
            })
        })
        .collect();
    Ok(json!({
        "message": message,
        "count": done.total,
        "sheets": n,
        "counts": done.counts,
        "twoSided": two,
        "separate": separate,
        "plans": plans,
    }))
}

#[cfg(test)]
#[path = "dancard_be_tests.rs"]
mod tests;
