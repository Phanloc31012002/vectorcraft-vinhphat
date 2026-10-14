//! DanCard — Dàn tối ưu › Dàn KTS (`dcDanToiUu` v22 của DanCard): dàn tối ưu không PON.
//!
//! Chọn các bài, nhập khổ giấy: mỗi tờ là một artboard mới (2 mặt: cặp artboard trước / sau đặt
//! cạnh nhau), lề 3 mm mỗi cạnh, khe 0 mm, không PON, không vẽ đường cắt; các con nằm sát nhau.
//! - Cùng khổ (mọi mẫu lệch không quá 0,2 pt so với mặt trước của mẫu đầu): dải hàng / cột con
//!   thường rồi con xoay 90°, tối đa hai tầng chẻ guillotine; cụm con canh giữa tờ. Mặc định mỗi
//!   mẫu một tờ đầy; tick "nhiều mẫu" thì chia lần lượt các vị trí, con cùng mẫu đứng gần nhau.
//!   Bài nguồn giữ nguyên.
//! - Nhiều khổ (phải tick "nhiều mẫu"): đúng một tờ, tối đa con lớn trước rồi lấp con nhỏ; bố cục
//!   nào cũng phải chứng minh được trình tự dao cắt thẳng. Mẫu đã dàn được xóa khỏi nguồn.
//! - 2 mặt: chọn theo hàng, trái = trước, phải = sau; mặt sau đặt đối xứng ngang (lật ngang tờ),
//!   con mặt trước xoay +90° thì mặt sau xoay −90°.
//!
//! Khác DanCard: tờ mới đặt vào chỗ trống bên phải mọi thứ đang có (thay cho gốc canvas ±7200 pt
//! của Illustrator), vẫn xếp trái → phải rồi xuống hàng trong bề ngang 14400 pt như canvas đó; hộp
//! thoại "Dàn nhiều loại" thành tham số `continueUneven`; cả lần dàn là một bước hoàn tác (lỗi giữa
//! chừng không để lại gì). Số con được chặn (`MAX_PER_SHEET`, `MAX_PER_AXIS`, `MAX_COPIES`).
//!
//! Hệ toạ độ: phần dựng tờ tính như DanCard (y hướng LÊN, khung `[trái, trên, phải, dưới]`, trên >
//! dưới) rồi đổi sang trang của VectorCraft (y hướng xuống) bằng cách đổi dấu y. Bộ tính bố cục
//! nhiều khổ dùng hệ y hướng xuống, gốc ở góc trên trái vùng in.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Artboard, Document, Node, NodeId, Scaling};
use vectorcraft_geom::{Affine, Rect, Vec2};

use super::layout::{union_of, vis};
use super::roots_in_paint_order;
use crate::cmd::*;
use crate::{EngineError, Result, Session};

/// 1 mm (pt), đúng hằng số của DanCard.
const MM: f64 = 2.834645669;
/// Lề an toàn mỗi cạnh tờ.
const MARGIN: f64 = 3.0 * MM;
/// Khoảng cách giữa các tờ / artboard trên canvas.
const PAGE_GAP: f64 = 10.0 * MM;
/// Sai số của DanCard (pt).
const EPS: f64 = 0.01;
/// Sai số kích thước giữa hai mặt / giữa các mẫu (pt).
const SIZE_TOL: f64 = 0.2;
/// Bề ngang canvas Illustrator mà DanCard xếp tờ trong đó (pt).
const CANVAS_WIDTH: f64 = 14400.0;
/// Giới hạn số con / tờ khi cùng khổ (DanCard không chặn; chặn để không treo).
const MAX_PER_SHEET: usize = 5000;
/// Giới hạn số con mỗi chiều tờ (theo cạnh ngắn của bài) khi cùng khổ: số phương án chẻ tăng theo
/// lập phương của nó (160: dưới nửa giây).
const MAX_PER_AXIS: usize = 160;
/// Giới hạn tổng số bản sao một lần dàn (mọi tờ, cả hai mặt), và số object chọn.
const MAX_COPIES: usize = 20_000;

const FRONT_SIDE: &str = "Dan toi uu - Mat truoc";
const BACK_SIDE: &str = "Dan toi uu - Mat sau";
const ONE_SIDE: &str = "Dan toi uu";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.ktsLayout",
        "DanCard: Dàn tối ưu (Dàn KTS)",
        [],
        None,
        "{width?: Ngang giấy cm (số hoặc chuỗi như \"33\" hay \"35,4\"; mặc định 33), height?: Dọc giấy cm (mặc định 35.4), twoSided?: bool (Dàn 2 mặt đối xứng; chọn theo hàng: trái = trước và phải = sau), multiPerArtboard?: bool (Dàn nhiều mẫu vào một tờ; bắt buộc khi các mẫu khác khổ), continueUneven?: bool (khi số vị trí không chia đều cho số mẫu: true tiếp tục và false dừng; bỏ trống thì lệnh dừng và trả câu hỏi)} dàn các bài đang chọn lên các tờ giấy mới (lề 3 mm, khe 0 mm, không PON) đặt bên phải mọi thứ đang có, mỗi tờ một artboard (2 mặt: một cặp artboard trước / sau), xóa các artboard cũ; khác khổ thì ghép một tờ và xóa các mẫu nguồn đã dàn; một bước hoàn tác → {message: \"OK:[[COUNT:n]] …\", count: số con / tờ, sheets, artboards, mixed, counts: số con từng mẫu, copies, removedSources, ids}",
        has_selection,
        kts_layout
    )]
}

fn fail(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

// ---------------------------------------------------------------- tham số

/// `parseFloat` của JavaScript: khoảng trắng đầu, dấu, chữ số, phần lẻ, mũ; phần sau bỏ qua
/// (`"33cm"` → 33).
fn js_parse_float(s: &str) -> Option<f64> {
    let t = s.trim_start();
    let b = t.as_bytes();
    let digits = |from: usize| b.iter().skip(from).take_while(|c| c.is_ascii_digit()).count();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let int = digits(i);
    i += int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        if int + frac > 0 {
            i += 1 + frac;
        }
    }
    if int + frac == 0 {
        return None;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let e = digits(j);
        if e > 0 {
            i = j + e;
        }
    }
    t.get(..i)?.parse::<f64>().ok()
}

/// Khổ giấy (cm → pt) như `parseCentimeters` của DanCard: dấu phẩy đầu tiên thành dấu chấm, rồi
/// `parseFloat`. Không có tham số thì lấy mặc định của panel.
fn paper_param(p: &Value, key: &str, default: f64, label: &str) -> Result<f64> {
    let n = match p.get(key) {
        None => Some(default),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => js_parse_float(&s.replacen(',', ".", 1)),
        Some(_) => None,
    };
    match n.map(|v| (v, v * 10.0 * MM)) {
        Some((v, pt)) if v > 0.0 && pt.is_finite() => Ok(pt),
        _ => Err(fail(format!("{label} phải là số cm lớn hơn 0."))),
    }
}

// ---------------------------------------------------------------- dựng tờ (hệ DanCard, y hướng lên)

/// Khung `[trái, trên, phải, dưới]` của DanCard (trên > dưới).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Box4 {
    l: f64,
    t: f64,
    r: f64,
    b: f64,
}

impl Box4 {
    /// Khung trên trang của VectorCraft (y hướng xuống).
    fn to_page(self) -> Rect {
        Rect::new(self.l, -self.t, self.r, -self.b)
    }
    fn overlaps(&self, o: &Box4) -> bool {
        self.l < o.r - EPS && self.r > o.l + EPS && self.b < o.t - EPS && self.t > o.b + EPS
    }
}

const CANVAS_FULL: &str = "Không còn chỗ trong canvas để tạo tờ mới. Hãy xóa/bớt artboard cũ hoặc mở tài liệu Large Canvas.";

/// Tìm chỗ cho từng cụm tờ (`reserveOutputPosition`): trái → phải, hết bề ngang thì xuống hàng.
struct Placer {
    left: f64,
    right: f64,
    bottom: f64,
    cursor_left: f64,
    cursor_top: f64,
    paper_w: f64,
    paper_h: f64,
    two_sided: bool,
    occupied: Vec<Box4>,
}

impl Placer {
    /// Khung tờ trước và (2 mặt) tờ sau của cụm kế tiếp.
    fn reserve(&mut self) -> Result<(Box4, Option<Box4>)> {
        let bundle_w = if self.two_sided { self.paper_w * 2.0 + PAGE_GAP } else { self.paper_w };
        let bundle_h = self.paper_h;
        // Cụm rộng hơn cả bề ngang: DanCard xuống hàng mãi tới đáy canvas rồi báo hết chỗ.
        if bundle_w > self.right - self.left + EPS {
            return Err(fail(CANVAS_FULL));
        }
        let (mut cl, mut ct) = (self.cursor_left, self.cursor_top);
        for _ in 0..5000 {
            if cl + bundle_w > self.right + EPS {
                cl = self.left;
                ct -= bundle_h + PAGE_GAP;
                continue;
            }
            if ct - bundle_h < self.bottom - EPS {
                return Err(fail(CANVAS_FULL));
            }
            let bundle = Box4 { l: cl, t: ct, r: cl + bundle_w, b: ct - bundle_h };
            if let Some(blocker) = self.occupied.iter().find(|o| bundle.overlaps(o)) {
                cl = blocker.r + PAGE_GAP;
                continue;
            }
            self.occupied.push(bundle);
            self.cursor_left = cl + bundle_w + PAGE_GAP;
            self.cursor_top = ct;
            let front = Box4 { l: cl, t: ct, r: cl + self.paper_w, b: ct - self.paper_h };
            let back = self.two_sided.then_some(Box4 { l: front.r + PAGE_GAP, t: ct, r: front.r + PAGE_GAP + self.paper_w, b: ct - self.paper_h });
            return Ok((front, back));
        }
        Err(fail("Không tìm được vị trí trong canvas để đặt artboard mới."))
    }
}

/// Layer mới trên cùng, tên `base` (đã có thì `base 2`, `base 3`… như `uniqueLayerName`).
fn add_layer(d: &mut Document, base: &str) -> Result<NodeId> {
    let mut name = base.to_string();
    let mut suffix = 2;
    while suffix < 100_000 && d.layers.iter().any(|l| l.name.as_deref() == Some(name.as_str())) {
        name = format!("{base} {suffix}");
        suffix += 1;
    }
    let mut n = crate::cmd::layer::make_layer(d, Some(&name));
    n.locked = false;
    n.visible = true;
    Ok(d.insert(None, usize::MAX, n)?)
}

/// Artboard mới `base N`, N = số artboard đang có + 1 (kể cả artboard cũ chưa xóa, như DanCard).
fn add_artboard(d: &mut Document, rect: Box4, base: &str) {
    let id = d.next_artboard_id();
    let name = format!("{base} {}", d.artboards.len() + 1);
    d.artboards.push(Artboard { id, name, rect: rect.to_page(), show_center_mark: false, show_cross_hairs: false });
}

fn scaling() -> Scaling {
    Scaling { strokes: true, patterns: true, ..Scaling::default() }
}

/// Bài nguồn thứ `i` (bản chụp lúc bắt đầu dựng).
fn source_of(sources: &[Option<Node>], i: usize) -> Result<&Node> {
    sources.get(i).and_then(Option::as_ref).ok_or_else(|| fail("Không tìm thấy bài nguồn."))
}

/// Bản sao của `src` xoay `angle` độ (ngược chiều kim đồng hồ), góc trên trái khung nhìn thấy đặt
/// tại (`left`, `top`) của DanCard; `slot`: canh giữa trong ô rộng × cao đó (nhiều khổ).
fn copy_at(d: &mut Document, src: &Node, left: f64, top: f64, angle: i32, slot: Option<(f64, f64)>) -> Result<Node> {
    let mut c = d.reid(src);
    if angle != 0
        && let Some(ctr) = vis(&c).map(|b| b.center())
    {
        let m = Affine::translate(ctr.to_vec2()) * Affine::rotate(-f64::from(angle).to_radians()) * Affine::translate(-ctr.to_vec2());
        c.transform(m, scaling());
    }
    let b = vis(&c).filter(|b| b.x0.is_finite() && b.y0.is_finite()).ok_or_else(|| fail("Không đo được bản sao để đặt vào tờ giấy."))?;
    let (mut left, mut top) = (left, top);
    if let Some((sw, sh)) = slot {
        left += (sw - b.width()) / 2.0;
        top -= (sh - b.height()) / 2.0;
    }
    // Trên trang: x giữ nguyên, y = −top.
    c.transform(Affine::translate(Vec2::new(left - b.x0, -top - b.y0)), scaling());
    Ok(c)
}

/// Dời cả cụm cho tâm khung nhìn thấy của cụm trùng tâm tờ (`centerItems`).
fn center_nodes(nodes: &mut [Node], sheet: Box4) {
    let Some(u) = union_of(nodes.iter().filter_map(vis)) else { return };
    let by = sheet.to_page().center() - u.center();
    for n in nodes.iter_mut() {
        n.transform(Affine::translate(by), scaling());
    }
}

/// Đưa các bản sao vào layer: bản sao sau nằm dưới bản sao trước (`PLACEATEND`).
fn place_in_layer(d: &mut Document, layer: NodeId, nodes: Vec<Node>) -> Result<Vec<NodeId>> {
    let ids: Vec<NodeId> = nodes.iter().map(|n| n.id).collect();
    let ch = d.node_mut(layer).and_then(Node::children_mut).ok_or(EngineError::NoNode(layer))?;
    let old = std::mem::take(ch);
    ch.extend(nodes.into_iter().rev().map(Arc::new));
    ch.extend(old);
    Ok(ids)
}

/// Một vị trí trên tờ (hệ DanCard): góc trên trái, rộng, cao, góc xoay, mẫu.
#[derive(Clone, Copy, Debug)]
struct Placed {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    angle: i32,
    model: usize,
}

/// Bố cục đã chọn: cùng khổ hay nhiều khổ.
enum Layout {
    Uniform(Rc<Plan>),
    Mixed(MPlan),
}

/// Kết quả dựng trong bước hoàn tác.
struct Built {
    ids: Vec<NodeId>,
    front_layer: NodeId,
    artboards: usize,
    copies_per_model: Vec<usize>,
    removed: usize,
    kept: usize,
    warnings: usize,
}

fn kts_layout(s: &mut Session, p: &Value) -> Result<Value> {
    let two_sided = bool_or(p, "twoSided", false);
    let multi = bool_or(p, "multiPerArtboard", false);
    let uneven = p.get("continueUneven").and_then(Value::as_bool);
    let doc = s.doc().map_err(|_| fail("Chưa mở tài liệu nào."))?.doc.clone();
    let roots = roots_in_paint_order(s)?;
    if roots.is_empty() {
        return Err(fail("Chọn ít nhất một artwork trước khi dàn."));
    }
    let old_count = doc.artboards.len();

    // Bài nguồn đo được (khung nhìn thấy, hệ DanCard).
    let mut raw: Vec<Rec> = Vec::new();
    let mut source_ids: Vec<NodeId> = Vec::new();
    for id in roots {
        let Some(b) = doc.node(id).and_then(vis) else { continue };
        if ![b.x0, b.y0, b.x1, b.y1].iter().all(|v| v.is_finite()) || b.width() <= EPS || b.height() <= EPS {
            continue;
        }
        raw.push(Rec::new(source_ids.len(), b.x0, -b.y0, b.x1, -b.y1));
        source_ids.push(id);
    }
    if raw.is_empty() {
        return Err(fail("Selection hiện tại không có artwork hợp lệ để dàn."));
    }
    if raw.len() > MAX_COPIES {
        return Err(fail(format!("Chọn quá nhiều object ({}); giới hạn {MAX_COPIES} object một lần.", raw.len())));
    }
    if two_sided && (raw.len() < 2 || !raw.len().is_multiple_of(2)) {
        return Err(fail("Dàn 2 mặt cần chọn số object chẵn: từng cặp trái = trước, phải = sau."));
    }

    // Mẫu: (mặt trước, mặt sau) theo chỉ số trong `raw`.
    let models: Vec<(usize, Option<usize>)> = if two_sided {
        pair_records(&raw).map_err(fail)?.into_iter().map(|(f, b)| (f, Some(b))).collect()
    } else {
        let order = msort(raw.clone(), &|a: &Rec, b: &Rec| if (a.cy - b.cy).abs() > EPS { b.cy.total_cmp(&a.cy) } else { a.cx.total_cmp(&b.cx) });
        order.into_iter().map(|r| (r.idx, None)).collect()
    };
    let size = |i: usize| raw.get(i).map(|r| (r.r - r.l, r.t - r.b));
    let (source_w, source_h) = models.first().and_then(|m| size(m.0)).ok_or_else(|| fail("Không đo được kích thước bài mẫu."))?;
    let mut model_specs: Vec<(f64, f64)> = Vec::with_capacity(models.len());
    let mut mixed = false;
    for (mi, (front, back)) in models.iter().enumerate() {
        let (mut w, mut h) = size(*front).ok_or_else(|| fail("Không đo được mặt trước của mẫu."))?;
        if let Some(back) = back {
            let (bw, bh) = size(*back).ok_or_else(|| fail("Không đo được mặt sau của mẫu."))?;
            if (w - bw).abs() > SIZE_TOL || (h - bh).abs() > SIZE_TOL {
                return Err(fail(format!("Hai mặt của mẫu {} phải cùng kích thước và hướng khung.", mi + 1)));
            }
            w = w.max(bw);
            h = h.max(bh);
        }
        model_specs.push((w, h));
        if (w - source_w).abs() > SIZE_TOL || (h - source_h).abs() > SIZE_TOL {
            mixed = true;
        }
    }
    if mixed && !multi {
        return Err(fail("Tick Dàn nhiều mẫu vào một tờ để ghép các kích thước khác nhau."));
    }

    let paper_w = paper_param(p, "width", 33.0, "Ngang giấy")?;
    let paper_h = paper_param(p, "height", 35.4, "Dọc giấy")?;
    let usable_w = paper_w - 2.0 * MARGIN;
    let usable_h = paper_h - 2.0 * MARGIN;
    if usable_w <= EPS || usable_h <= EPS {
        return Err(fail("Khổ giấy phải lớn hơn lề an toàn 3 mm mỗi cạnh."));
    }

    let plan = if mixed {
        Layout::Mixed(mixed_plan(&model_specs, usable_w, usable_h).map_err(fail)?)
    } else {
        let per_axis = floor_fit(usable_w.max(usable_h), source_w.min(source_h));
        let estimate = usable_w * usable_h / (source_w * source_h);
        if per_axis > MAX_PER_AXIS || !estimate.is_finite() || estimate > MAX_PER_SHEET as f64 {
            return Err(fail(format!(
                "Bài mẫu quá nhỏ so với khổ giấy: vượt giới hạn {MAX_PER_SHEET} con / tờ ({MAX_PER_AXIS} con mỗi chiều). Hãy tăng kích thước bài hoặc giảm khổ giấy."
            )));
        }
        Layout::Uniform(Uniform::new(source_w, source_h).solve(usable_w, usable_h, 2))
    };
    let count = match &plan {
        Layout::Uniform(u) => u.count,
        Layout::Mixed(m) => m.count,
    };
    if count == 0 {
        return Err(fail("Bài mẫu không nằm vừa trong vùng in sau khi trừ lề 3 mm."));
    }
    if count > MAX_PER_SHEET {
        return Err(fail(format!("Quá nhiều con trên một tờ ({count}); giới hạn {MAX_PER_SHEET} con / tờ.")));
    }

    // Lô mẫu của từng tờ.
    let n_models = models.len();
    // Đủ tờ để mẫu nào cũng có mặt; vị trí dư lặp các mẫu đầu (DanCard hỏi trước: `continueUneven`).
    let (total_slots, extra_slots) = if multi && !mixed {
        let sheet_count = n_models.div_ceil(count);
        let total = sheet_count.saturating_mul(count);
        let extra = total.checked_rem(n_models).unwrap_or(0);
        if n_models > 1 && extra != 0 {
            match uneven {
                Some(true) => {}
                Some(false) => return Err(fail("Đã dừng dàn vì số mẫu không chia đều cho số vị trí trên các tờ.")),
                None => {
                    return Err(fail(format!(
                        "{total} vị trí trên {sheet_count} tờ không chia đều cho {n_models} mẫu. Nếu tiếp tục, tool sẽ giữ các con cùng mẫu gần nhau; một vài mẫu sẽ được lặp thêm 1 con."
                    )));
                }
            }
        }
        (total, extra)
    } else {
        (count, 0)
    };
    let batches: Vec<Vec<usize>> = if mixed {
        vec![(0..n_models).collect()]
    } else if multi {
        let per_model = total_slots.checked_div(n_models).unwrap_or(0);
        let remainder = total_slots.checked_rem(n_models).unwrap_or(0);
        let mut grouped = Vec::with_capacity(total_slots.min(MAX_COPIES + 1));
        for mi in 0..n_models {
            grouped.extend(std::iter::repeat_n(mi, per_model + usize::from(mi < remainder)));
            if grouped.len() > MAX_COPIES {
                break;
            }
        }
        grouped.chunks(count).map(<[usize]>::to_vec).collect()
    } else {
        (0..n_models).map(|m| vec![m]).collect()
    };
    let copies = batches.len().saturating_mul(count).saturating_mul(if two_sided { 2 } else { 1 });
    if copies > MAX_COPIES {
        return Err(fail(format!("Quá nhiều con để dàn ({copies}); giới hạn {MAX_COPIES} con một lần. Hãy chia thành nhóm nhỏ hơn.")));
    }

    // Tờ mới nằm bên phải mọi thứ đang có, mép trên ngang mép trên của chúng.
    let extent = doc.artboards.iter().fold(doc.art_bounds(), |acc, a| Some(acc.map_or(a.rect, |e| e.union(a.rect))));
    let (layout_left, layout_top) = match extent.filter(|e| e.x1.is_finite() && e.y0.is_finite()) {
        Some(e) => (e.x1 + PAGE_GAP, -e.y0),
        None => (0.0, 0.0),
    };
    let mut placer = Placer {
        left: layout_left,
        right: (layout_left + CANVAS_WIDTH - 2.0 * PAGE_GAP).min(crate::MAX_COORD - PAGE_GAP),
        bottom: -crate::MAX_COORD + PAGE_GAP,
        cursor_left: layout_left,
        cursor_top: layout_top,
        paper_w,
        paper_h,
        two_sided,
        occupied: Vec::new(),
    };
    let front_base = if two_sided { FRONT_SIDE } else { ONE_SIDE };

    let built = s.edit("Dàn tối ưu (DanCard)", |d, sel| {
        let sources: Vec<Option<Node>> = source_ids.iter().map(|id| d.node(*id).cloned()).collect();
        let front_layer = add_layer(d, front_base)?;
        let back_layer = if two_sided { Some(add_layer(d, BACK_SIDE)?) } else { None };
        let mut ids = Vec::new();
        let mut copies_per_model = vec![0usize; n_models];
        let mut artboards = 0;
        for batch in &batches {
            let (front_rect, back_rect) = placer.reserve()?;
            add_artboard(d, front_rect, front_base);
            artboards += 1;
            if let Some(br) = back_rect {
                add_artboard(d, br, BACK_SIDE);
                artboards += 1;
            }
            let safe_left = front_rect.l + MARGIN;
            let safe_top = front_rect.t - MARGIN;
            let slots: Vec<Placed> = match &plan {
                Layout::Mixed(mp) => {
                    // Canh giữa khung chung của bố cục một lần, dùng cho cả hai mặt.
                    let right = mp.cells.iter().fold(0.0f64, |m, c| m.max(c.x + c.w));
                    let bottom = mp.cells.iter().fold(0.0f64, |m, c| m.max(c.y + c.h));
                    let (dx, dy) = ((usable_w - right) / 2.0, (usable_h - bottom) / 2.0);
                    mp.cells
                        .iter()
                        .map(|c| Placed { x: safe_left + dx + c.x, y: safe_top - dy - c.y, w: c.w, h: c.h, angle: c.angle, model: c.model })
                        .collect()
                }
                Layout::Uniform(up) => {
                    let mut out = Vec::new();
                    collect_slots(up, safe_left, safe_top, usable_w, usable_h, &mut out);
                    if out.len() != count {
                        return Err(fail("Lỗi tạo vị trí dàn. Hãy thử lại."));
                    }
                    order_slots(out, batch, multi)
                        .into_iter()
                        .enumerate()
                        .map(|(i, u)| Placed {
                            x: u.x,
                            y: u.y,
                            w: u.w,
                            h: u.h,
                            angle: u.angle,
                            model: i.checked_rem(batch.len()).and_then(|k| batch.get(k)).copied().unwrap_or(0),
                        })
                        .collect()
                }
            };
            if slots.len() != count {
                return Err(fail("Lỗi tạo vị trí dàn. Hãy thử lại."));
            }
            let mut fronts = Vec::with_capacity(slots.len());
            for sl in &slots {
                let (front, _) = models.get(sl.model).copied().ok_or_else(|| fail("Lỗi tạo vị trí dàn. Hãy thử lại."))?;
                fronts.push(copy_at(d, source_of(&sources, front)?, sl.x, sl.y, sl.angle, mixed.then_some((sl.w, sl.h)))?);
                if let Some(c) = copies_per_model.get_mut(sl.model) {
                    *c += 1;
                }
            }
            if !mixed {
                center_nodes(&mut fronts, front_rect);
            }
            ids.extend(place_in_layer(d, front_layer, fronts)?);
            if let (Some(br), Some(bl)) = (back_rect, back_layer) {
                let back_left = br.l + MARGIN;
                let back_top = br.t - MARGIN;
                let mut backs = Vec::with_capacity(slots.len());
                for sl in &slots {
                    let Some((_, Some(back))) = models.get(sl.model).copied() else { continue };
                    // Đối xứng ngang trong vùng in, cùng độ cao; con +90° ở mặt trước thì −90° ở sau.
                    let rel_x = sl.x - safe_left;
                    let rel_y = safe_top - sl.y;
                    let left = back_left + usable_w - rel_x - sl.w;
                    let top = back_top - rel_y;
                    let angle = if sl.angle == 90 { -90 } else { 0 };
                    backs.push(copy_at(d, source_of(&sources, back)?, left, top, angle, mixed.then_some((sl.w, sl.h)))?);
                }
                if !mixed {
                    center_nodes(&mut backs, br);
                }
                ids.extend(place_in_layer(d, bl, backs)?);
            }
        }
        // Chỉ xóa khung artboard cũ (bài trên đó giữ nguyên), cùng guide của artboard đó.
        if d.artboards.len() > old_count {
            let gone: Vec<u32> = d.artboards.iter().take(old_count).map(|a| a.id).collect();
            if !gone.is_empty() {
                d.artboards.retain(|a| !gone.contains(&a.id));
                d.retain_guides(sel, |_, g| g.artboard.is_none_or(|id| !gone.contains(&id)));
            }
        }
        // Nhiều khổ: xóa nguồn của mẫu đã dàn được, giữ mẫu chưa dàn.
        let (mut removed, mut kept, mut warnings) = (0, 0, 0);
        if let Layout::Mixed(mp) = &plan {
            for (mi, (front, back)) in models.iter().enumerate() {
                if mp.counts.get(mi).copied().unwrap_or(0) == 0 {
                    kept += 1;
                    continue;
                }
                let mut ok = source_ids.get(*front).is_some_and(|id| d.remove_many(&[*id]).is_ok());
                if let Some(back) = back {
                    ok &= source_ids.get(*back).is_some_and(|id| d.remove_many(&[*id]).is_ok());
                }
                if ok {
                    removed += 1;
                } else {
                    warnings += 1;
                }
            }
        }
        sel.set(ids.iter().copied());
        Ok(Built { ids, front_layer, artboards, copies_per_model, removed, kept, warnings })
    })?;
    if let Some(st) = s.active_mut() {
        st.active_layer = Some(built.front_layer);
    }

    let sheets = batches.len();
    let mut detail = if two_sided {
        format!("Đã tạo {sheets} cặp artboard trước/sau đối xứng và canh giữa artwork; các cặp đã tự xuống hàng khi hết bề ngang.")
    } else {
        format!("Đã tạo {sheets} artboard và canh giữa artwork; các tờ đã tự xuống hàng khi hết bề ngang.")
    };
    let counts = match &plan {
        Layout::Mixed(mp) => mp.counts.clone(),
        Layout::Uniform(_) => built.copies_per_model.clone(),
    };
    if let Layout::Mixed(mp) = &plan {
        let list: Vec<String> = counts.iter().map(usize::to_string).collect();
        detail += &format!(" Ghép nhiều kích thước; số con từng mẫu: {}.", list.join(", "));
        detail += " Tự nhân bản tối đa con lớn trước, rồi lấp con nhỏ.";
        detail += &format!(" Đã xóa {} mẫu nguồn đã dàn; còn {} mẫu chưa dàn được giữ nguyên.", built.removed, built.kept);
        if built.warnings > 0 {
            detail += &format!(" Không xóa được hết nguồn của {} mẫu; hãy kiểm tra layer khóa.", built.warnings);
        }
        if mp.capacity_limited {
            detail += " Đã đạt giới hạn 128 con/tờ của tìm kiếm ghép; chưa lấp hết phần dư.";
        }
    } else if multi && n_models > 1 {
        if extra_slots == 0 {
            detail += &format!(" Đã dàn đều {n_models} mẫu.");
        } else {
            detail += &format!(" Đã dàn đủ {n_models} mẫu; {extra_slots} slot dư lặp mẫu đầu theo từng cụm.");
        }
    } else if sheets > 1 {
        detail += " Mỗi mẫu được dàn đầy trên một tờ riêng.";
    }
    Ok(json!({
        "message": format!("OK:[[COUNT:{count}]] {detail}"),
        "count": count,
        "sheets": sheets,
        "artboards": built.artboards,
        "mixed": mixed,
        "counts": counts,
        "copies": built.ids.len(),
        "removedSources": built.removed,
        "ids": built.ids.iter().map(|id| id.0).collect::<Vec<_>>(),
    }))
}

// ================================================================ bộ tính bố cục
//
// Thuần tính toán, chỉ dùng std: không đọc, không ghi tài liệu. Chuyển nguyên văn từ DanCard
// (`dcDanToiUu` phần trong, `dcKtsPairSourceRecords`, `dcKtsKnifePlan`, `dcKtsKnifeBandVariants`,
// `dcKtsSlicingProfilePlan`, `dcKtsMixedSizePlan`), giữ đúng thứ tự duyệt và ngân sách để ra cùng
// kết quả.

/// `Math.round` của JavaScript (nửa làm tròn lên), ra số nguyên.
fn js_round(v: f64) -> i64 {
    let f = v.floor();
    let r = if v - f >= 0.5 { f + 1.0 } else { f };
    if r.is_finite() { r.clamp(-9.0e15, 9.0e15) as i64 } else { 0 }
}

/// Sắp xếp ổn định (merge sort) theo `cmp`. Không bao giờ panic, kể cả khi `cmp` dùng sai số nên
/// không bắc cầu (như các hàm so sánh của DanCard); `sort_by` của std có thể panic khi đó.
fn msort<T>(v: Vec<T>, cmp: &dyn Fn(&T, &T) -> Ordering) -> Vec<T> {
    if v.len() <= 1 {
        return v;
    }
    let mut left = v;
    let right = left.split_off(left.len() / 2);
    let (left, right) = (msort(left, cmp), msort(right, cmp));
    let mut out = Vec::with_capacity(left.len() + right.len());
    let mut li = left.into_iter().peekable();
    let mut ri = right.into_iter().peekable();
    loop {
        let take_right = match (li.peek(), ri.peek()) {
            (Some(a), Some(b)) => cmp(b, a) == Ordering::Less,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (None, None) => break,
        };
        if let Some(x) = if take_right { ri.next() } else { li.next() } {
            out.push(x);
        }
    }
    out
}

/// `floorFit`: số con cỡ `size` nằm vừa `space`.
fn floor_fit(space: f64, size: f64) -> usize {
    if size <= EPS {
        return 0;
    }
    let v = ((space + EPS) / size).floor();
    if v.is_finite() && v > 0.0 { v.min(1.0e9) as usize } else { 0 }
}

// ---------------------------------------------------------------- cùng khổ

/// Một khối dải lặp: `repeat` hàng (hoặc cột), mỗi dải `cells` con `cell_w` × `cell_h`, xoay `angle`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    angle: i32,
    repeat: usize,
    cells: usize,
    cell_w: f64,
    cell_h: f64,
}

#[derive(Debug)]
enum Shape {
    Empty,
    /// `row`: các dải là hàng ngang; không thì cột dọc. Khối dải thường trước, khối dải xoay sau.
    Strips {
        row: bool,
        bands: [Option<Band>; 2],
    },
    /// Chẻ dọc (`vertical`, `cut` tính từ mép trái vùng) hoặc ngang (`cut` tính từ mép trên xuống).
    Split {
        vertical: bool,
        cut: f64,
        first: Rc<Plan>,
        second: Rc<Plan>,
    },
}

/// Bố cục cùng khổ của một vùng: số con, số con thường / xoay, hình dạng.
#[derive(Debug)]
struct Plan {
    count: usize,
    normal: usize,
    rotated: usize,
    shape: Shape,
}

impl Plan {
    fn empty() -> Self {
        Plan { count: 0, normal: 0, rotated: 0, shape: Shape::Empty }
    }
    /// `planComplexity`.
    fn complexity(&self) -> usize {
        match &self.shape {
            Shape::Empty => 0,
            Shape::Strips { bands, .. } => bands.iter().flatten().count(),
            Shape::Split { first, second, .. } => 1 + first.complexity() + second.complexity(),
        }
    }
    fn split(vertical: bool, cut: f64, first: Rc<Plan>, second: Rc<Plan>) -> Self {
        Plan {
            count: first.count.saturating_add(second.count),
            normal: first.normal.saturating_add(second.normal),
            rotated: first.rotated.saturating_add(second.rotated),
            shape: Shape::Split { vertical, cut, first, second },
        }
    }
}

/// `isBetter`: nhiều con hơn, rồi nhiều con thường hơn, rồi đơn giản hơn (hoà thì giữ cái trước).
fn is_better(c: &Plan, best: Option<&Plan>) -> bool {
    let Some(b) = best else { return true };
    if c.count != b.count {
        return c.count > b.count;
    }
    if c.normal != b.normal {
        return c.normal > b.normal;
    }
    c.complexity() < b.complexity()
}

/// Bộ tìm bố cục cùng khổ cho bài `sw` × `sh` (`solveRegion`, ghi nhớ theo vùng).
struct Uniform {
    sw: f64,
    sh: f64,
    memo: HashMap<(i64, i64, u32), Rc<Plan>>,
}

impl Uniform {
    fn new(sw: f64, sh: f64) -> Self {
        Uniform { sw, sh, memo: HashMap::new() }
    }

    /// `makeStripPlan`.
    fn strip(&self, row: bool, normal_bands: usize, rotated_bands: usize, w: f64, h: f64) -> Option<Plan> {
        let (normal_cells, rotated_cells) =
            if row { (floor_fit(w, self.sw), floor_fit(w, self.sh)) } else { (floor_fit(h, self.sh), floor_fit(h, self.sw)) };
        if (normal_bands > 0 && normal_cells == 0) || (rotated_bands > 0 && rotated_cells == 0) {
            return None;
        }
        let normal = normal_bands.saturating_mul(normal_cells);
        let rotated = rotated_bands.saturating_mul(rotated_cells);
        let count = normal.saturating_add(rotated);
        if count == 0 {
            return None;
        }
        let normal_band =
            (normal_bands > 0).then_some(Band { angle: 0, repeat: normal_bands, cells: normal_cells, cell_w: self.sw, cell_h: self.sh });
        let rotated_band =
            (rotated_bands > 0).then_some(Band { angle: 90, repeat: rotated_bands, cells: rotated_cells, cell_w: self.sh, cell_h: self.sw });
        Some(Plan { count, normal, rotated, shape: Shape::Strips { row, bands: [normal_band, rotated_band] } })
    }

    /// `bestStripPlan`: mỗi số dải thường, phần còn lại xếp tối đa dải xoay.
    fn best_strip(&self, w: f64, h: f64) -> Option<Plan> {
        let mut best: Option<Plan> = None;
        for normal_rows in 0..=floor_fit(h, self.sh) {
            let rotated_rows = floor_fit(h - normal_rows as f64 * self.sh, self.sw);
            if let Some(p) = self.strip(true, normal_rows, rotated_rows, w, h)
                && is_better(&p, best.as_ref())
            {
                best = Some(p);
            }
        }
        for normal_cols in 0..=floor_fit(w, self.sw) {
            let rotated_cols = floor_fit(w - normal_cols as f64 * self.sw, self.sh);
            if let Some(p) = self.strip(false, normal_cols, rotated_cols, w, h)
                && is_better(&p, best.as_ref())
            {
                best = Some(p);
            }
        }
        best
    }

    /// `cutValues`: các vị trí chẻ (bội số cạnh bài tính từ hai mép), không trùng.
    fn cut_values(&self, total: f64) -> Vec<f64> {
        let mut values = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |v: f64| {
            if v <= EPS || total - v <= EPS {
                return;
            }
            if seen.insert(js_round(v * 1000.0)) {
                values.push(v);
            }
        };
        for size in [self.sw, self.sh] {
            for i in 1..=floor_fit(total, size) {
                add(i as f64 * size);
                add(total - i as f64 * size);
            }
        }
        values
    }

    /// `solveRegion`: dải tốt nhất, rồi thử chẻ dọc / ngang tới `depth` tầng.
    fn solve(&mut self, w: f64, h: f64, depth: u32) -> Rc<Plan> {
        let key = (js_round(w * 1000.0), js_round(h * 1000.0), depth);
        if let Some(p) = self.memo.get(&key) {
            return p.clone();
        }
        let best = match self.best_strip(w, h) {
            None => Rc::new(Plan::empty()),
            Some(strip) if depth == 0 => Rc::new(strip),
            Some(strip) => {
                let mut best = Rc::new(strip);
                for cut in self.cut_values(w) {
                    let a = self.solve(cut, h, depth - 1);
                    let b = self.solve(w - cut, h, depth - 1);
                    let p = Plan::split(true, cut, a, b);
                    if is_better(&p, Some(best.as_ref())) {
                        best = Rc::new(p);
                    }
                }
                for cut in self.cut_values(h) {
                    let a = self.solve(w, cut, depth - 1);
                    let b = self.solve(w, h - cut, depth - 1);
                    let p = Plan::split(false, cut, a, b);
                    if is_better(&p, Some(best.as_ref())) {
                        best = Rc::new(p);
                    }
                }
                best
            }
        };
        self.memo.insert(key, best.clone());
        best
    }
}

/// Một vị trí cùng khổ (hệ DanCard, y hướng lên): góc trên trái (`x`, `y`), chiếm
/// `[x, x + w] × [y − h, y]`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct USlot {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    angle: i32,
}

/// `collectSlots`: các vị trí của `plan` trong vùng góc trên trái (`left`, `top`), `w` × `h`.
fn collect_slots(plan: &Plan, left: f64, top: f64, w: f64, h: f64, out: &mut Vec<USlot>) {
    if plan.count == 0 {
        return;
    }
    match &plan.shape {
        Shape::Empty => {}
        Shape::Split { vertical: true, cut, first, second } => {
            collect_slots(first, left, top, *cut, h, out);
            collect_slots(second, left + cut, top, w - cut, h, out);
        }
        Shape::Split { vertical: false, cut, first, second } => {
            collect_slots(first, left, top, w, *cut, out);
            collect_slots(second, left, top - cut, w, h - cut, out);
        }
        Shape::Strips { row: true, bands } => {
            let mut y = top;
            for b in bands.iter().flatten() {
                for _ in 0..b.repeat {
                    let mut x = left;
                    for _ in 0..b.cells {
                        out.push(USlot { x, y, w: b.cell_w, h: b.cell_h, angle: b.angle });
                        x += b.cell_w;
                    }
                    y -= b.cell_h;
                }
            }
        }
        Shape::Strips { row: false, bands } => {
            let mut x = left;
            for b in bands.iter().flatten() {
                for _ in 0..b.repeat {
                    let mut y = top;
                    for _ in 0..b.cells {
                        out.push(USlot { x, y, w: b.cell_w, h: b.cell_h, angle: b.angle });
                        y -= b.cell_h;
                    }
                    x += b.cell_w;
                }
            }
        }
    }
}

/// Cao hơn trước (y hướng lên), cùng hàng thì trái trước.
fn before_on_sheet(a: &USlot, b: &USlot) -> bool {
    if (a.y - b.y).abs() > EPS { a.y > b.y } else { a.x < b.x }
}

/// Bình phương khoảng hở giữa hai vị trí (0 khi chạm hoặc chồng).
fn edge_distance(a: &USlot, b: &USlot) -> f64 {
    let (a_right, a_bottom, b_right, b_bottom) = (a.x + a.w, a.y - a.h, b.x + b.w, b.y - b.h);
    let dx = if a_right < b.x {
        b.x - a_right
    } else if b_right < a.x {
        a.x - b_right
    } else {
        0.0
    };
    let dy = if a_bottom > b.y {
        a_bottom - b.y
    } else if b_bottom > a.y {
        b_bottom - a.y
    } else {
        0.0
    };
    dx * dx + dy * dy
}

/// `orderSlotsForBatch`: mỗi cụm con cùng mẫu (các phần tử liền nhau của `batch`) lấy các vị trí
/// kề nhau, cùng chiều xoay trước; chỉ khi chia nhiều mẫu và lô đủ một tờ.
fn order_slots(slots: Vec<USlot>, batch: &[usize], multi: bool) -> Vec<USlot> {
    if !multi || batch.len() != slots.len() {
        return slots;
    }
    let mut remaining = slots;
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut bi = 0;
    while bi < batch.len() && !remaining.is_empty() {
        let Some(&model) = batch.get(bi) else { break };
        let mut run = 1;
        while batch.get(bi + run) == Some(&model) {
            run += 1;
        }
        let mut cluster: Vec<USlot> = Vec::with_capacity(run);
        let mut preferred_angle = 0;
        for _ in 0..run {
            let mut picked: Option<usize> = None;
            if cluster.is_empty() {
                for (i, c) in remaining.iter().enumerate() {
                    if picked.and_then(|p| remaining.get(p)).is_none_or(|p| before_on_sheet(c, p)) {
                        picked = Some(i);
                    }
                }
                if let Some(p) = picked.and_then(|p| remaining.get(p)) {
                    preferred_angle = p.angle;
                }
            } else {
                let has_preferred = remaining.iter().any(|c| c.angle == preferred_angle);
                let mut best_distance = -1.0;
                for (i, c) in remaining.iter().enumerate() {
                    if has_preferred && c.angle != preferred_angle {
                        continue;
                    }
                    let mut nearest = -1.0;
                    for k in &cluster {
                        let d = edge_distance(c, k);
                        if nearest < 0.0 || d < nearest {
                            nearest = d;
                        }
                    }
                    let better = match picked.and_then(|p| remaining.get(p)) {
                        None => true,
                        Some(p) => nearest < best_distance - EPS || ((nearest - best_distance).abs() <= EPS && before_on_sheet(c, p)),
                    };
                    if better {
                        picked = Some(i);
                        best_distance = nearest;
                    }
                }
            }
            let Some(p) = picked.filter(|p| *p < remaining.len()) else { break };
            cluster.push(remaining.remove(p));
        }
        ordered.extend(cluster);
        bi += run;
    }
    ordered
}

// ---------------------------------------------------------------- ghép cặp trước / sau

/// Một bài nguồn (hệ DanCard, y hướng lên): khung và tâm; `idx`: thứ tự trong danh sách nguồn.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rec {
    idx: usize,
    l: f64,
    t: f64,
    r: f64,
    b: f64,
    cx: f64,
    cy: f64,
}

impl Rec {
    fn new(idx: usize, l: f64, t: f64, r: f64, b: f64) -> Self {
        Rec { idx, l, t, r, b, cx: (l + r) / 2.0, cy: (t + b) / 2.0 }
    }
    fn height(&self) -> f64 {
        self.t - self.b
    }
}

/// `dcKtsPairSourceRecords`: gom hàng (tâm lệch không quá ¼ bài thấp nhất của hàng), mỗi hàng
/// chẵn, trái = trước, phải = sau → các cặp (`idx` trước, `idx` sau), hàng trên trước.
fn pair_records(records: &[Rec]) -> std::result::Result<Vec<(usize, usize)>, String> {
    let mut pending = msort(records.to_vec(), &|a: &Rec, b: &Rec| if a.cy != b.cy { b.cy.total_cmp(&a.cy) } else { a.cx.total_cmp(&b.cx) });
    let mut pairs = Vec::new();
    while !pending.is_empty() {
        let anchor = pending.remove(0);
        let mut row = vec![anchor];
        let (mut high, mut low, mut min_height) = (anchor.cy, anchor.cy, anchor.height());
        let mut i = 0;
        while let Some(c) = pending.get(i).copied() {
            let next_height = min_height.min(c.height());
            let (next_high, next_low) = (high.max(c.cy), low.min(c.cy));
            if next_high - next_low <= next_height * 0.25 + EPS {
                row.push(c);
                pending.remove(i);
                (high, low, min_height) = (next_high, next_low, next_height);
            } else {
                i += 1;
            }
        }
        if !row.len().is_multiple_of(2) {
            return Err("Không ghép rõ cặp trước/sau: mỗi hàng cần đủ object trái và phải. Hãy canh lại hàng nguồn.".into());
        }
        let row = msort(row, &|a: &Rec, b: &Rec| a.cx.total_cmp(&b.cx));
        for pair in row.chunks(2) {
            let [front, back] = pair else { continue };
            if front.r > back.l + EPS || back.cx - front.cx <= EPS {
                return Err("Hai mặt nguồn đang chồng ngang hoặc không phân biệt được trái/phải. Hãy đặt mỗi cặp cạnh nhau.".into());
            }
            pairs.push((front.idx, back.idx));
        }
    }
    Ok(pairs)
}

// ---------------------------------------------------------------- nhiều khổ: dao cắt thẳng

/// Một con trong bố cục nhiều khổ (y hướng xuống, gốc góc trên trái vùng in).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    angle: i32,
    model: usize,
}

impl Cell {
    fn area(&self) -> Area {
        Area { x: self.x, y: self.y, w: self.w, h: self.h }
    }
}

/// Hình chữ nhật trong hệ bộ tính (y hướng xuống).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Area {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Một nhát dao: ngang (`horizontal`, đường y = `at`) hoặc dọc (x = `at`), trên vùng nó chẻ.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cut {
    horizontal: bool,
    at: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Cut {
    fn on(horizontal: bool, at: f64, r: Area) -> Self {
        Cut { horizontal, at, x: r.x, y: r.y, w: r.w, h: r.h }
    }
    fn length(&self) -> f64 {
        if self.horizontal { self.w } else { self.h }
    }
}

fn length_of(cuts: &[Cut]) -> f64 {
    cuts.iter().map(Cut::length).sum()
}

/// Trình tự dao đã chứng minh: các nhát, số nhát, tổng chiều dài, số lượt dao (chồng tờ cắt chung).
#[derive(Clone, Debug)]
struct Knife {
    cuts: Vec<Cut>,
    cut_count: usize,
    cut_length: f64,
    work: usize,
}

/// Lời giải của một nhóm con.
struct Part {
    bounds: Area,
    cuts: Vec<Cut>,
    cut_length: f64,
    work: usize,
}

/// Xén vùng `region` về khung `b` (tối đa 4 nhát), mỗi nhát ghi vùng trước khi xén.
fn trim(b: Area, region: Area) -> Vec<Cut> {
    let mut r = region;
    let mut cuts = Vec::new();
    if b.x > r.x + EPS {
        cuts.push(Cut::on(false, b.x, r));
        r.w -= b.x - r.x;
        r.x = b.x;
    }
    if b.x + b.w < r.x + r.w - EPS {
        cuts.push(Cut::on(false, b.x + b.w, r));
        r.w = b.w;
    }
    if b.y > r.y + EPS {
        cuts.push(Cut::on(true, b.y, r));
        r.h -= b.y - r.y;
        r.y = b.y;
    }
    if b.y + b.h < r.y + r.h - EPS {
        cuts.push(Cut::on(true, b.y + b.h, r));
    }
    cuts
}

/// Mẫu cắt tương đối với `region` (`cutPattern`): hai mảnh cùng mẫu thì chồng lên cắt chung.
fn cut_pattern(cuts: &[Cut], region: Area) -> Vec<i64> {
    let q = |n: f64| js_round(n * 100.0);
    let mut v = vec![q(region.w), q(region.h)];
    for c in cuts {
        let origin = if c.horizontal { region.y } else { region.x };
        v.extend([i64::from(c.horizontal), q(c.at - origin), q(c.x - region.x), q(c.y - region.y), q(c.w), q(c.h)]);
    }
    v
}

/// `dcKtsKnifePlan`: tìm có giới hạn (30000 bước) một trình tự chẻ guillotine ít lượt dao nhất.
struct KnifeSolver<'a> {
    cells: &'a [Cell],
    width: f64,
    height: f64,
    budget: i64,
    memo: HashMap<Vec<usize>, Option<Rc<Part>>>,
}

impl KnifeSolver<'_> {
    fn solve(&mut self, ids: &[usize]) -> Option<Rc<Part>> {
        if let Some(m) = self.memo.get(ids) {
            return m.clone();
        }
        let cells = self.cells;
        let (mut x, mut y, mut right, mut bottom) = (self.width, self.height, 0.0f64, 0.0f64);
        for s in ids.iter().filter_map(|i| cells.get(*i)) {
            x = x.min(s.x);
            y = y.min(s.y);
            right = right.max(s.x + s.w);
            bottom = bottom.max(s.y + s.h);
        }
        let bounds = Area { x, y, w: right - x, h: bottom - y };
        if ids.len() == 1 {
            let one = Some(Rc::new(Part { bounds, cuts: Vec::new(), cut_length: 0.0, work: 0 }));
            self.memo.insert(ids.to_vec(), one.clone());
            return one;
        }
        let mut best: Option<Part> = None;
        for horizontal in [false, true] {
            if self.budget <= 0 {
                break;
            }
            let span = |s: &Cell| if horizontal { (s.y, s.y + s.h) } else { (s.x, s.x + s.w) };
            let mut edges: Vec<f64> = ids.iter().filter_map(|i| cells.get(*i)).flat_map(|s| <[f64; 2]>::from(span(s))).collect();
            edges.sort_by(f64::total_cmp);
            let (low, high) = if horizontal { (y, bottom) } else { (x, right) };
            for (ei, &at) in edges.iter().enumerate() {
                if self.budget <= 0 {
                    break;
                }
                let repeated = ei > 0 && edges.get(ei - 1).is_some_and(|p| (at - p).abs() <= EPS);
                if at <= low + EPS || at >= high - EPS || repeated {
                    continue;
                }
                let (mut first, mut second, mut crossed) = (Vec::new(), Vec::new(), false);
                for &id in ids {
                    self.budget -= 1;
                    if self.budget <= 0 {
                        crossed = true;
                        break;
                    }
                    let Some(s) = cells.get(id) else { continue };
                    let (start, end) = span(s);
                    if start < at - EPS && end > at + EPS {
                        crossed = true;
                        break;
                    }
                    if end <= at + EPS {
                        first.push(id);
                    } else {
                        second.push(id);
                    }
                }
                if crossed || first.is_empty() || second.is_empty() {
                    continue;
                }
                let a = self.solve(&first);
                let b = self.solve(&second);
                let (Some(a), Some(b)) = (a, b) else { continue };
                let (ar, br) = if horizontal {
                    (Area { x, y, w: bounds.w, h: at - y }, Area { x, y: at, w: bounds.w, h: bottom - at })
                } else {
                    (Area { x, y, w: at - x, h: bounds.h }, Area { x: at, y, w: right - at, h: bounds.h })
                };
                let cut = Cut { horizontal, at, x, y, w: bounds.w, h: bounds.h };
                let (trim_a, trim_b) = (trim(a.bounds, ar), trim(b.bounds, br));
                let (work_a, work_b) = (trim_a.len() + a.work, trim_b.len() + b.work);
                let mut ac = trim_a;
                ac.extend(a.cuts.iter().copied());
                let mut bc = trim_b;
                bc.extend(b.cuts.iter().copied());
                // Hai mảnh rời giống hệt nhau thì chồng lên cắt chung một lượt.
                let work = if cut_pattern(&ac, ar) == cut_pattern(&bc, br) { 1 + work_a.max(work_b) } else { 1 + work_a + work_b };
                let mut cuts = vec![cut];
                cuts.extend(ac);
                cuts.extend(bc);
                let cut_length = length_of(&cuts);
                let improve = match &best {
                    None => true,
                    Some(p) => {
                        work < p.work
                            || (work == p.work && (cuts.len() < p.cuts.len() || (cuts.len() == p.cuts.len() && cut_length < p.cut_length - EPS)))
                    }
                };
                if improve {
                    best = Some(Part { bounds, cuts, cut_length, work });
                }
            }
        }
        let best = best.map(Rc::new);
        self.memo.insert(ids.to_vec(), best.clone());
        best
    }
}

/// `dcKtsKnifePlan`: None khi không có con, hơn 128 con, hoặc chưa chứng minh được trong ngân sách.
fn knife_plan(cells: &[Cell], width: f64, height: f64) -> Option<Knife> {
    if cells.is_empty() || cells.len() > 128 {
        return None;
    }
    let mut solver = KnifeSolver { cells, width, height, budget: 30_000, memo: HashMap::new() };
    let ids: Vec<usize> = (0..cells.len()).collect();
    let result = solver.solve(&ids)?;
    let mut cuts = trim(result.bounds, Area { x: 0.0, y: 0.0, w: width, h: height });
    let edge = cuts.len();
    cuts.extend(result.cuts.iter().copied());
    Some(Knife { cut_count: cuts.len(), cut_length: length_of(&cuts), work: edge + result.work, cuts })
}

/// `dcKtsKnifeBandVariants`: xếp lại các dải nguyên vẹn (hàng, rồi cột) để các dải giống nhau
/// nằm kề nhau; không đổi số lượng hay kích thước con.
fn band_variants(cells: &[Cell]) -> Vec<Vec<Cell>> {
    struct BandInfo {
        from: f64,
        size: f64,
        cells: Vec<Cell>,
        area: f64,
        key: String,
        order: usize,
    }
    let mut variants = Vec::new();
    for columns in [false, true] {
        let span = |s: &Cell| if columns { (s.x, s.x + s.w) } else { (s.y, s.y + s.h) };
        let mut borders = vec![0.0];
        for s in cells {
            let (a, b) = span(s);
            borders.push(a);
            borders.push(b);
        }
        borders.sort_by(f64::total_cmp);
        let mut edges: Vec<f64> = Vec::new();
        for &at in &borders {
            if edges.last().is_some_and(|l| at <= l + EPS) {
                continue;
            }
            let crossed = cells.iter().any(|s| {
                let (a, b) = span(s);
                a < at - EPS && b > at + EPS
            });
            if !crossed {
                edges.push(at);
            }
        }
        if edges.len() < 3 {
            continue;
        }
        let mut bands = Vec::new();
        for (ei, pair) in edges.windows(2).enumerate() {
            let [from, to] = pair else { continue };
            let (from, to) = (*from, *to);
            let mut band = BandInfo { from, size: to - from, cells: Vec::new(), area: 0.0, key: String::new(), order: ei + 1 };
            let mut signatures = Vec::new();
            for s in cells {
                let (start, end) = span(s);
                let mid = (start + end) / 2.0;
                if start < from - EPS || end > to + EPS || mid < from || mid >= to {
                    continue;
                }
                band.cells.push(*s);
                band.area = band.area.max(s.w * s.h);
                let other = if columns { s.y } else { s.x };
                signatures.push(format!(
                    "{},{},{},{}",
                    js_round(other * 100.0),
                    js_round((start - from) * 100.0),
                    js_round(s.w * 100.0),
                    js_round(s.h * 100.0)
                ));
            }
            signatures.sort();
            band.key = signatures.join("|");
            bands.push(band);
        }
        let bands = msort(bands, &|a: &BandInfo, b: &BandInfo| {
            if (a.area - b.area).abs() > EPS {
                return b.area.total_cmp(&a.area);
            }
            a.key.cmp(&b.key).then(a.order.cmp(&b.order))
        });
        let mut reordered = Vec::with_capacity(cells.len());
        let mut offset = 0.0;
        for b in &bands {
            let shift = offset - b.from;
            for s in &b.cells {
                reordered.push(if columns { Cell { x: s.x + shift, ..*s } } else { Cell { y: s.y + shift, ..*s } });
            }
            offset += b.size;
        }
        if reordered.len() == cells.len() {
            variants.push(reordered);
        }
    }
    variants
}

// ---------------------------------------------------------------- nhiều khổ: DP normal pattern

/// Sai số của `dcKtsSlicingProfilePlan`.
const PROFILE_EPS: f64 = 0.0001;
const MAX_TRANSITIONS: u64 = 5_000_000;

/// Cây chẻ của DP. Vùng của nút: `rw` × `rh`.
#[derive(Debug)]
enum Tree {
    /// Một con `w` × `h` của nhóm `group` ở góc trên trái vùng.
    Leaf { group: usize, w: f64, h: f64, rw: f64, rh: f64 },
    /// Xén một dải trống tại `at` (từ mép trái / trên) cho cả bố cục `inner`.
    Trim { horizontal: bool, at: f64, inner: Rc<Tree>, rw: f64, rh: f64 },
    /// Chẻ tại `at`; phần sau dùng lời giải của vùng `cap` (xén thêm khi `avail` > `cap`).
    Split { horizontal: bool, at: f64, a: Rc<Tree>, b: Rc<Tree>, cap: f64, avail: f64, rw: f64, rh: f64 },
}

/// Một bản ghi DP: số con nhỏ, tổng số con, số nhát, tổng chiều dài, số lượt dao, cây.
struct DpRec {
    small: usize,
    total: usize,
    cut_count: usize,
    cut_length: f64,
    work: usize,
    tree: Option<Rc<Tree>>,
}

type Records = Vec<Option<Rc<DpRec>>>;

/// `preferred`: nhiều con nhỏ hơn, rồi ít lượt dao, ít nhát, ngắn hơn.
fn preferred(small: usize, work: usize, cut_count: usize, cut_length: f64, prev: Option<&DpRec>) -> bool {
    let Some(p) = prev else { return true };
    if small != p.small {
        return small > p.small;
    }
    if work != p.work {
        return work < p.work;
    }
    if cut_count != p.cut_count {
        return cut_count < p.cut_count;
    }
    cut_length < p.cut_length - PROFILE_EPS
}

/// `adoptTrimmed`; false khi hết ngân sách.
fn adopt_trim(
    records: &mut [Option<Rc<DpRec>>],
    src: &[Option<Rc<DpRec>>],
    horizontal: bool,
    at: f64,
    region: (f64, f64),
    transitions: &mut u64,
) -> bool {
    let (rw, rh) = region;
    let trim_length = if horizontal { rw } else { rh };
    for (k, slot) in records.iter_mut().enumerate() {
        let Some(Some(inner)) = src.get(k) else { continue };
        if inner.total == 0 {
            continue;
        }
        *transitions += 1;
        if *transitions > MAX_TRANSITIONS {
            return false;
        }
        let (cut_count, cut_length, work) = (inner.cut_count + 1, inner.cut_length + trim_length, inner.work + 1);
        if !preferred(inner.small, work, cut_count, cut_length, slot.as_deref()) {
            continue;
        }
        let Some(t) = inner.tree.clone() else { continue };
        let tree = Tree::Trim { horizontal, at, inner: t, rw, rh };
        *slot = Some(Rc::new(DpRec { small: inner.small, total: inner.total, cut_count, cut_length, work, tree: Some(Rc::new(tree)) }));
    }
    true
}

/// Một phép chẻ của DP: hướng, vị trí, vùng phần sau (`cap`) và chỗ thật có (`avail`); `same`: hai
/// phần dùng cùng một ô DP.
struct Join {
    horizontal: bool,
    at: f64,
    cap: f64,
    avail: f64,
    same: bool,
}

/// `combine`; false khi hết ngân sách.
///
/// DanCard so chữ ký hình học của hai phần (`left.signature === right.signature`) để biết chúng
/// chồng lên cắt chung được. Chữ ký mở đầu bằng khổ vùng của ô DP (các vị trí normal pattern cách
/// nhau hơn sai số nên khác ô thì khác chữ ký), và trong một ô thì khác số con lớn là khác cây: hai
/// chữ ký bằng nhau đúng khi cùng ô và cùng số con lớn, tức cùng một bản ghi.
fn combine(
    records: &mut [Option<Rc<DpRec>>],
    first: &[Option<Rc<DpRec>>],
    second: &[Option<Rc<DpRec>>],
    j: &Join,
    region: (f64, f64),
    transitions: &mut u64,
) -> bool {
    let (rw, rh) = region;
    let residual = j.avail > j.cap + PROFILE_EPS;
    let across = if j.horizontal { rw } else { rh };
    let residual_length = if residual { across } else { 0.0 };
    let max_a = records.len().saturating_sub(1);
    for (qa, left) in first.iter().enumerate().take(max_a + 1) {
        let Some(left) = left else { continue };
        if left.total == 0 {
            continue;
        }
        for qb in 0..=(max_a - qa) {
            let Some(Some(right)) = second.get(qb) else { continue };
            if right.total == 0 {
                continue;
            }
            *transitions += 1;
            if *transitions > MAX_TRANSITIONS {
                return false;
            }
            let total = left.total + right.total;
            if total > 128 {
                continue;
            }
            let out = qa + qb;
            let small = left.small + right.small;
            let prev = records.get(out).and_then(Option::as_deref);
            if prev.is_some_and(|p| small < p.small) {
                continue;
            }
            let extra = usize::from(residual);
            let cut_count = 1 + left.cut_count + right.cut_count + extra;
            let work = if !residual && j.same && qa == qb { 1 + left.work.max(right.work) } else { 1 + left.work + right.work + extra };
            let cut_length = across + left.cut_length + right.cut_length + residual_length;
            if !preferred(small, work, cut_count, cut_length, prev) {
                continue;
            }
            let (Some(ta), Some(tb)) = (left.tree.clone(), right.tree.clone()) else { continue };
            let tree = Tree::Split { horizontal: j.horizontal, at: j.at, a: ta, b: tb, cap: j.cap, avail: j.avail, rw, rh };
            if let Some(slot) = records.get_mut(out) {
                *slot = Some(Rc::new(DpRec { small, total, cut_count, cut_length, work, tree: Some(Rc::new(tree)) }));
            }
        }
    }
    true
}

/// Trải cây thành các con (góc trên trái, `model` = nhóm) và các nhát dao.
fn expand(t: &Tree, x: f64, y: f64, slots: &mut Vec<Cell>, cuts: &mut Vec<Cut>) {
    match t {
        Tree::Trim { horizontal, at, inner, rw, rh } => {
            let absolute = if *horizontal { y + at } else { x + at };
            cuts.push(Cut { horizontal: *horizontal, at: absolute, x, y, w: *rw, h: *rh });
            expand(inner, x, y, slots, cuts);
        }
        Tree::Leaf { group, w, h, rw, rh } => {
            slots.push(Cell { x, y, w: *w, h: *h, angle: 0, model: *group });
            let mut region_w = *rw;
            if region_w > w + PROFILE_EPS {
                cuts.push(Cut { horizontal: false, at: x + w, x, y, w: region_w, h: *rh });
                region_w = *w;
            }
            if *rh > h + PROFILE_EPS {
                cuts.push(Cut { horizontal: true, at: y + h, x, y, w: region_w, h: *rh });
            }
        }
        Tree::Split { horizontal: false, at, a, b, cap, avail, rw, rh } => {
            cuts.push(Cut { horizontal: false, at: x + at, x, y, w: *rw, h: *rh });
            expand(a, x, y, slots, cuts);
            let (bx, bw) = (x + at, rw - at);
            if *avail > cap + PROFILE_EPS {
                cuts.push(Cut { horizontal: false, at: bx + cap, x: bx, y, w: bw, h: *rh });
            }
            expand(b, bx, y, slots, cuts);
        }
        Tree::Split { horizontal: true, at, a, b, cap, avail, rw, rh } => {
            cuts.push(Cut { horizontal: true, at: y + at, x, y, w: *rw, h: *rh });
            expand(a, x, y, slots, cuts);
            let (by, bh) = (y + at, rh - at);
            if *avail > cap + PROFILE_EPS {
                cuts.push(Cut { horizontal: true, at: by + cap, x, y: by, w: *rw, h: bh });
            }
            expand(b, x, by, slots, cuts);
        }
    }
}

/// `dcKtsSlicingProfilePlan` với `{allowPartialCoverage: true, repeatLargest: true}` (lời gọi duy
/// nhất của DanCard): DP chính xác cho đúng hai khổ, tối đa con lớn rồi con nhỏ. None: chưa hỗ trợ
/// hoặc vượt ngân sách (không có nghĩa là không xếp được).
fn slicing_profile(specs: &[(f64, f64)], width: f64, height: f64) -> Option<MPlan> {
    const E: f64 = PROFILE_EPS;
    struct Group {
        w: f64,
        h: f64,
        area: f64,
        models: Vec<usize>,
    }
    if specs.is_empty() || !width.is_finite() || !height.is_finite() || width <= E || height <= E {
        return None;
    }
    let mut groups: Vec<Group> = Vec::new();
    for (i, &(w, h)) in specs.iter().enumerate() {
        if !w.is_finite() || !h.is_finite() || w <= E || h <= E {
            return None;
        }
        let (long, short) = (w.max(h), w.min(h));
        let gi = match groups.iter().position(|g| (g.w - long).abs() <= E && (g.h - short).abs() <= E) {
            Some(g) => g,
            None => {
                if groups.len() >= 2 {
                    return None;
                }
                groups.push(Group { w: long, h: short, area: long * short, models: Vec::new() });
                groups.len() - 1
            }
        };
        groups.get_mut(gi)?.models.push(i);
    }
    let [g0, g1]: [Group; 2] = groups.try_into().ok()?;
    // Nhóm A (chỉ số 0) là khổ có diện tích lớn hơn.
    let groups = if g0.area < g1.area { [g1, g0] } else { [g0, g1] };
    let [ga, gb] = &groups;
    let full = width * height;
    let max_a_f = ((full + E) / ga.area).floor();
    let copy_bound = (full / ga.area.min(gb.area) + E).floor();
    if !(0.0..=40.0).contains(&max_a_f) || copy_bound > 128.0 {
        return None;
    }
    let max_a = max_a_f as usize;

    let mut edges: Vec<f64> = Vec::new();
    for v in [ga.w, ga.h, gb.w, gb.h] {
        if !edges.iter().any(|e| (e - v).abs() <= E) {
            edges.push(v);
        }
    }
    edges.sort_by(f64::total_cmp);
    // Toạ độ normal pattern: tổng các cạnh con, tối đa 56 vị trí mỗi chiều.
    let positions = |max: f64| -> Option<Vec<f64>> {
        let mut p = vec![0.0];
        let mut pi = 0;
        while let Some(&base) = p.get(pi) {
            for &e in &edges {
                let mut at = base + e;
                if at > max + E {
                    continue;
                }
                if (at - max).abs() <= E {
                    at = max;
                }
                let place = p.iter().position(|v| *v >= at - E).unwrap_or(p.len());
                if p.get(place).is_some_and(|v| (v - at).abs() <= E) {
                    continue;
                }
                if p.len() >= 56 {
                    return None;
                }
                p.insert(place, at);
            }
            pi += 1;
        }
        if p.last().is_some_and(|l| max > l + E) {
            if p.len() >= 56 {
                return None;
            }
            p.push(max);
        }
        Some(p)
    };
    let xs = positions(width)?;
    let ys = positions(height)?;
    let (nx, ny) = (xs.len(), ys.len());
    if nx.saturating_mul(ny) > 2800 {
        return None;
    }
    let floor_position = |p: &[f64], available: f64| -> usize {
        let (mut low, mut high) = (0, p.len().saturating_sub(1));
        while low < high {
            let mid = (low + high).div_ceil(2);
            if p.get(mid).is_some_and(|v| *v <= available + E) {
                low = mid;
            } else {
                high = mid.saturating_sub(1);
            }
        }
        low
    };

    let mut dp: Vec<Records> = Vec::with_capacity(nx * ny);
    let mut transitions: u64 = 0;
    for (i, &rw) in xs.iter().enumerate() {
        for (j, &rh) in ys.iter().enumerate() {
            let mut records: Records = vec![None; max_a + 1];
            if let Some(r0) = records.first_mut() {
                *r0 = Some(Rc::new(DpRec { small: 0, total: 0, cut_count: 0, cut_length: 0.0, work: 0, tree: None }));
            }
            // Lá: một con ở góc trên trái vùng (A rồi B; nằm rồi đứng).
            for (gi, g) in groups.iter().enumerate() {
                for (lw, lh) in [(g.w, g.h), (g.h, g.w)] {
                    if lw > rw + E || lh > rh + E {
                        continue;
                    }
                    let (aq, bq) = if gi == 0 { (1, 0) } else { (0, 1) };
                    if aq > max_a {
                        continue;
                    }
                    let (mut cuts, mut length) = (0, 0.0);
                    if rw > lw + E {
                        cuts += 1;
                        length += rh;
                    }
                    if rh > lh + E {
                        cuts += 1;
                        length += lw;
                    }
                    if preferred(bq, cuts, cuts, length, records.get(aq).and_then(Option::as_deref))
                        && let Some(slot) = records.get_mut(aq)
                    {
                        let tree = Tree::Leaf { group: gi, w: lw, h: lh, rw, rh };
                        *slot =
                            Some(Rc::new(DpRec { small: bq, total: 1, cut_count: cuts, cut_length: length, work: cuts, tree: Some(Rc::new(tree)) }));
                    }
                }
            }
            let region = (rw, rh);
            for tx in 1..i {
                if let Some(src) = dp.get(tx * ny + j)
                    && !adopt_trim(&mut records, src, false, xs.get(tx).copied().unwrap_or(0.0), region, &mut transitions)
                {
                    return None;
                }
            }
            for ty in 1..j {
                if let Some(src) = dp.get(i * ny + ty)
                    && !adopt_trim(&mut records, src, true, ys.get(ty).copied().unwrap_or(0.0), region, &mut transitions)
                {
                    return None;
                }
            }
            for xi in 1..i {
                let at = xs.get(xi).copied().unwrap_or(0.0);
                let ri = floor_position(&xs, rw - at);
                if ri == 0 {
                    continue;
                }
                let (Some(first), Some(second)) = (dp.get(xi * ny + j), dp.get(ri * ny + j)) else { continue };
                let join = Join { horizontal: false, at, cap: xs.get(ri).copied().unwrap_or(0.0), avail: rw - at, same: xi == ri };
                if !combine(&mut records, first, second, &join, region, &mut transitions) {
                    return None;
                }
            }
            for yi in 1..j {
                let at = ys.get(yi).copied().unwrap_or(0.0);
                let bi = floor_position(&ys, rh - at);
                if bi == 0 {
                    continue;
                }
                let (Some(first), Some(second)) = (dp.get(i * ny + yi), dp.get(i * ny + bi)) else { continue };
                let join = Join { horizontal: true, at, cap: ys.get(bi).copied().unwrap_or(0.0), avail: rh - at, same: yi == bi };
                if !combine(&mut records, first, second, &join, region, &mut transitions) {
                    return None;
                }
            }
            dp.push(records);
        }
    }

    // Không đổi một con lớn lấy vài con nhỏ: chỉ lấy số con lớn khả thi cao nhất.
    let result = dp.last()?;
    let k = (0..=max_a).rev().find(|k| result.get(*k).and_then(Option::as_ref).is_some_and(|r| r.total > 0))?;
    let record = result.get(k)?.as_ref()?;
    let tree = record.tree.clone()?;
    let mut raw = Vec::new();
    let mut cuts = Vec::new();
    expand(&tree, 0.0, 0.0, &mut raw, &mut cuts);
    let mut assigned = [0usize; 2];
    let mut counts = vec![0usize; specs.len()];
    let mut area = 0.0;
    let mut cells = Vec::with_capacity(raw.len());
    for c in raw {
        let g = c.model;
        let models = &groups.get(g)?.models;
        let n = assigned.get(g).copied().unwrap_or(0);
        let mi = *models.get(n.checked_rem(models.len()).unwrap_or(0))?;
        if let Some(v) = assigned.get_mut(g) {
            *v += 1;
        }
        if let Some(v) = counts.get_mut(mi) {
            *v += 1;
        }
        let (sw, sh) = specs.get(mi).copied()?;
        let angle = if (c.w - sw).abs() > E || (c.h - sh).abs() > E { 90 } else { 0 };
        area += c.w * c.h;
        cells.push(Cell { angle, model: mi, ..c });
    }
    Some(MPlan {
        count: cells.len(),
        cells,
        counts,
        area,
        cut_count: cuts.len(),
        cut_length: length_of(&cuts),
        work: record.work,
        tiers: Vec::new(),
        capacity_limited: false,
    })
}

// ---------------------------------------------------------------- nhiều khổ: bộ ghép

/// Bố cục nhiều khổ một tờ (`dcKtsMixedSizePlan`).
#[derive(Clone, Debug)]
struct MPlan {
    cells: Vec<Cell>,
    /// Số con từng mẫu (thứ tự mẫu).
    counts: Vec<usize>,
    count: usize,
    area: f64,
    cut_count: usize,
    cut_length: f64,
    work: usize,
    /// Số con từng bậc kích thước (bậc 0 = lớn nhất).
    tiers: Vec<usize>,
    /// Đã chạm giới hạn 128 con / tờ của tìm kiếm.
    capacity_limited: bool,
}

/// `less`: so từng phần tử với sai số; `b` None thì `a` nhỏ hơn.
fn less(a: &[f64], b: Option<&[f64]>) -> bool {
    let Some(b) = b else { return true };
    for (x, y) in a.iter().zip(b) {
        if *x < y - EPS {
            return true;
        }
        if *x > y + EPS {
            return false;
        }
    }
    false
}

/// `preferPlan`: bậc lớn trước (bậc lớn hơn thắng mọi số con nhỏ), rồi nhiều con, ít lượt dao,
/// ít nhát, ngắn hơn, diện tích lớn hơn.
fn prefer_plan(c: &MPlan, prev: Option<&MPlan>) -> bool {
    let Some(p) = prev else { return true };
    for (a, b) in c.tiers.iter().zip(&p.tiers) {
        if a != b {
            return a > b;
        }
    }
    if c.count != p.count {
        return c.count > p.count;
    }
    if c.work != p.work {
        return c.work < p.work;
    }
    if c.cut_count != p.cut_count {
        return c.cut_count < p.cut_count;
    }
    if (c.cut_length - p.cut_length).abs() > EPS {
        return c.cut_length < p.cut_length;
    }
    c.area > p.area + EPS
}

/// `knifeFor`: `knife_plan` ghi nhớ theo hình học các con.
struct KnifeCache {
    width: f64,
    height: f64,
    cache: HashMap<Vec<[i64; 4]>, Option<Rc<Knife>>>,
}

impl KnifeCache {
    fn get(&mut self, cells: &[Cell]) -> Option<Rc<Knife>> {
        let key: Vec<[i64; 4]> = cells.iter().map(|c| [js_round(c.x * 1e4), js_round(c.y * 1e4), js_round(c.w * 1e4), js_round(c.h * 1e4)]).collect();
        if let Some(k) = self.cache.get(&key) {
            return k.clone();
        }
        let k = knife_plan(cells, self.width, self.height).map(Rc::new);
        self.cache.insert(key, k.clone());
        k
    }
}

/// Chẻ guillotine ô trống `free[index]` sau khi đặt `cell` ở góc trên trái của nó (`splitFree`).
fn split_free(free: &mut Vec<Area>, index: usize, cell: Area, rule: usize, cuts: &mut Vec<Cut>) {
    let Some(r) = free.get(index).copied() else { return };
    let (dw, dh) = (r.w - cell.w, r.h - cell.h);
    let (x2, y2) = (cell.x + cell.w, cell.y + cell.h);
    free.remove(index);
    let mut add = |x: f64, y: f64, w: f64, h: f64| {
        if w > EPS && h > EPS {
            free.push(Area { x, y, w, h });
        }
    };
    let horizontal = match rule {
        0 => dw <= dh,
        1 => dw > dh,
        2 => true,
        _ => false,
    };
    if horizontal {
        if dh > EPS {
            cuts.push(Cut::on(true, y2, r));
        }
        if dw > EPS {
            cuts.push(Cut::on(false, x2, Area { h: cell.h, ..r }));
        }
        add(x2, r.y, dw, cell.h);
        add(r.x, y2, r.w, dh);
    } else {
        if dw > EPS {
            cuts.push(Cut::on(false, x2, r));
        }
        if dh > EPS {
            cuts.push(Cut::on(true, y2, Area { w: cell.w, ..r }));
        }
        add(r.x, y2, cell.w, dh);
        add(x2, r.y, dw, r.h);
    }
}

/// Ô trống MaxRects (chồng lấn) sau khi đặt `cell` (`splitMaxRects`).
fn split_max_rects(free: &[Area], cell: Area) -> Vec<Area> {
    let mut next = Vec::new();
    let (x2, y2) = (cell.x + cell.w, cell.y + cell.h);
    let add = |next: &mut Vec<Area>, x: f64, y: f64, w: f64, h: f64| {
        if w > EPS && h > EPS {
            next.push(Area { x, y, w, h });
        }
    };
    for r in free {
        let (rx2, ry2) = (r.x + r.w, r.y + r.h);
        if cell.x >= rx2 - EPS || x2 <= r.x + EPS || cell.y >= ry2 - EPS || y2 <= r.y + EPS {
            next.push(*r);
            continue;
        }
        if cell.x > r.x + EPS {
            add(&mut next, r.x, r.y, cell.x - r.x, r.h);
        }
        if x2 < rx2 - EPS {
            add(&mut next, x2, r.y, rx2 - x2, r.h);
        }
        if cell.y > r.y + EPS {
            add(&mut next, r.x, r.y, r.w, cell.y - r.y);
        }
        if y2 < ry2 - EPS {
            add(&mut next, r.x, y2, r.w, ry2 - y2);
        }
    }
    let contains = |b: &Area, a: &Area| a.x >= b.x - EPS && a.y >= b.y - EPS && a.x + a.w <= b.x + b.w + EPS && a.y + a.h <= b.y + b.h + EPS;
    let mut i = next.len();
    while i > 0 {
        i -= 1;
        let Some(a) = next.get(i).copied() else { continue };
        if next.iter().enumerate().any(|(j, b)| j != i && contains(b, &a)) {
            next.remove(i);
        }
    }
    next
}

/// Con thêm vào lỗ trống đang tốt nhất: con, dao, điểm, bậc và số con của mẫu nó.
struct Extra {
    cell: Cell,
    knife: Rc<Knife>,
    score: Vec<f64>,
    tier: usize,
    count: usize,
}

/// `dcKtsMixedSizePlan`: ghép các khổ `specs` (rộng, cao) vào vùng in `width` × `height` trên một
/// tờ. Lỗi: chữ của DanCard.
fn mixed_plan(specs: &[(f64, f64)], width: f64, height: f64) -> std::result::Result<MPlan, String> {
    if !width.is_finite() || !height.is_finite() || width <= EPS || height <= EPS || specs.is_empty() {
        return Err("Không có vùng in/mẫu hợp lệ.".into());
    }
    let n = specs.len();
    let mut smallest = f64::INFINITY;
    let mut largest_area = 0.0;
    let mut largest = (0.0, 0.0);
    let mut fitting = 0;
    for (si, &(w, h)) in specs.iter().enumerate() {
        if !w.is_finite() || !h.is_finite() || w <= EPS || h <= EPS {
            return Err(format!("Không đo được kích thước mẫu {}.", si + 1));
        }
        if (w <= width + EPS && h <= height + EPS) || (h <= width + EPS && w <= height + EPS) {
            fitting += 1;
        }
        smallest = smallest.min(w * h);
        if w * h > largest_area {
            largest_area = w * h;
            largest = (w, h);
        }
    }
    if fitting == 0 {
        return Err("Mẫu không vừa vùng in; không tự thu nhỏ bài.".into());
    }
    // Bậc kích thước: lớn → nhỏ; cùng bậc khi diện tích kém đầu bậc không quá `tolerance`.
    let tolerance = 0.2 * (largest.0 + largest.1) + 0.04;
    let area_of = |i: usize| specs.get(i).map_or(0.0, |(w, h)| w * h);
    let size_order = msort((0..n).collect(), &|a: &usize, b: &usize| {
        let difference = area_of(*b) - area_of(*a);
        if difference.abs() > EPS { difference.total_cmp(&0.0) } else { a.cmp(b) }
    });
    let mut tier_heads: Vec<f64> = Vec::new();
    let mut tier = vec![0usize; n];
    for si in size_order {
        let a = area_of(si);
        if tier_heads.last().is_none_or(|head| head - a > tolerance) {
            tier_heads.push(a);
        }
        if let Some(t) = tier.get_mut(si) {
            *t = tier_heads.len() - 1;
        }
    }
    let tier_counts = |counts: &[usize]| -> Vec<usize> {
        let mut totals = vec![0usize; tier_heads.len()];
        for (mi, c) in counts.iter().enumerate() {
            if let Some(t) = tier.get(mi).and_then(|t| totals.get_mut(*t)) {
                *t += c;
            }
        }
        totals
    };
    let theoretical = (width * height / smallest).ceil();
    if !theoretical.is_finite() || theoretical > 10_000.0 || n > 100 {
        return Err("Quá nhiều vị trí/mẫu để ghép; hãy chia thành nhóm nhỏ hơn.".into());
    }
    let limit = theoretical.min(128.0) as usize;
    let mut knives = KnifeCache { width, height, cache: HashMap::new() };
    let mut best: Option<MPlan> = None;

    // Hạt giống: DP chính xác khi đúng hai khổ.
    if let Some(mut profile) = slicing_profile(specs, width, height) {
        profile.tiers = tier_counts(&profile.counts);
        if let Some(k) = knives.get(&profile.cells)
            && k.work <= profile.work
        {
            profile.cut_count = k.cut_count;
            profile.cut_length = k.cut_length;
            profile.work = k.work;
        }
        if prefer_plan(&profile, best.as_ref()) {
            best = Some(profile);
        }
    }

    // Tham lam: 4 cách chấm điểm × 5 cách chẻ × tối đa 4 hạt giống, chung ngân sách 400000 bước.
    let mut operations: u64 = 0;
    for strategy in 0..4 {
        for split_rule in 0..5 {
            for seed in 0..n.min(4) {
                let mut free = vec![Area { x: 0.0, y: 0.0, w: width, h: height }];
                let mut cells: Vec<Cell> = Vec::new();
                let mut counts = vec![0usize; n];
                let mut area = 0.0;
                let mut cuts: Vec<Cut> = Vec::new();
                while cells.len() < limit && operations < 400_000 {
                    let mut chosen: Option<(Cell, usize)> = None;
                    let mut chosen_score: Option<Vec<f64>> = None;
                    for (si, &(sw, sh)) in specs.iter().enumerate() {
                        // Bậc lớn còn vừa thì lấy trước; trong bậc, mẫu ít con nhất đi trước.
                        let head = [tier.get(si).copied().unwrap_or(0) as f64, counts.get(si).copied().unwrap_or(0) as f64];
                        let order = (si + n - seed).checked_rem(n).unwrap_or(0) as f64;
                        for (fi, r) in free.iter().enumerate() {
                            for rotation in 0..2 {
                                operations += 1;
                                if operations >= 400_000 {
                                    break;
                                }
                                let (w, h) = if rotation == 1 { (sh, sw) } else { (sw, sh) };
                                if w > r.w + EPS || h > r.h + EPS {
                                    continue;
                                }
                                let short = (r.w - w).min(r.h - h);
                                let long = (r.w - w).max(r.h - h);
                                let waste = r.w * r.h - w * h;
                                let rot = f64::from(rotation);
                                let tail = match strategy {
                                    0 => vec![short, long, waste, order, r.y, r.x, rot],
                                    1 => vec![waste, short, long, order, r.y, r.x, rot],
                                    2 => vec![r.y, r.x, -w * h, order, rot],
                                    _ => vec![-w * h, short, long, order, r.y, r.x, rot],
                                };
                                let mut score = head.to_vec();
                                score.extend(tail);
                                if less(&score, chosen_score.as_deref()) {
                                    chosen_score = Some(score);
                                    chosen = Some((Cell { x: r.x, y: r.y, w, h, angle: if rotation == 1 { 90 } else { 0 }, model: si }, fi));
                                }
                            }
                        }
                    }
                    let Some((cell, fi)) = chosen else { break };
                    cells.push(cell);
                    if let Some(c) = counts.get_mut(cell.model) {
                        *c += 1;
                    }
                    area += cell.w * cell.h;
                    if split_rule == 4 {
                        free = split_max_rects(&free, cell.area());
                    } else {
                        split_free(&mut free, fi, cell.area(), split_rule, &mut cuts);
                    }
                }
                if cells.is_empty() {
                    continue;
                }
                let knife = knives.get(&cells);
                match &knife {
                    Some(k) => cuts = k.cuts.clone(),
                    // MaxRects mà chưa chứng minh được đường dao thì không dùng.
                    None if split_rule == 4 => continue,
                    None => {}
                }
                let mut work = knife.as_ref().map_or(cuts.len(), |k| k.work);
                for variant in band_variants(&cells) {
                    if let Some(bk) = knives.get(&variant)
                        && (bk.work < work || (bk.work == work && bk.cut_count < cuts.len()))
                    {
                        cells = variant;
                        cuts = bk.cuts.clone();
                        work = bk.work;
                    }
                }
                let trial = MPlan {
                    count: cells.len(),
                    cells,
                    tiers: tier_counts(&counts),
                    counts,
                    area,
                    cut_count: cuts.len(),
                    cut_length: length_of(&cuts),
                    work,
                    capacity_limited: false,
                };
                if prefer_plan(&trial, best.as_ref()) {
                    best = Some(trial);
                }
            }
        }
    }
    let Some(mut best) = best else {
        return Err("Không ghép được mẫu trên một tờ; hãy tăng khổ giấy hoặc bớt mẫu.".into());
    };

    // Lấp lỗ: chỉ thêm con khi chứng minh được đường dao.
    let mut hole_checks = 0;
    while best.count < limit.min(128) && hole_checks < 128 {
        let mut actual = vec![Area { x: 0.0, y: 0.0, w: width, h: height }];
        for s in &best.cells {
            actual = split_max_rects(&actual, s.area());
        }
        let mut extra: Option<Extra> = None;
        for (si, &(sw, sh)) in specs.iter().enumerate() {
            let (t, c) = (tier.get(si).copied().unwrap_or(0), best.counts.get(si).copied().unwrap_or(0));
            for ri in 0..2 {
                let (ew, eh) = if ri == 1 { (sh, sw) } else { (sw, sh) };
                if let Some(e) = &extra
                    && (t > e.tier || (t == e.tier && c > e.count))
                {
                    continue;
                }
                for ar in &actual {
                    if hole_checks >= 128 {
                        break;
                    }
                    if ew > ar.w + EPS || eh > ar.h + EPS {
                        continue;
                    }
                    let candidate = Cell { x: ar.x, y: ar.y, w: ew, h: eh, angle: if ri == 1 { 90 } else { 0 }, model: si };
                    hole_checks += 1;
                    let mut with = best.cells.clone();
                    with.push(candidate);
                    let Some(k) = knives.get(&with) else { continue };
                    let score = vec![t as f64, c as f64, k.work as f64, k.cut_count as f64, k.cut_length, ar.y, ar.x, f64::from(ri)];
                    if less(&score, extra.as_ref().map(|e| e.score.as_slice())) {
                        extra = Some(Extra { cell: candidate, knife: k, score, tier: t, count: c });
                    }
                }
            }
        }
        let Some(Extra { cell, knife: k, .. }) = extra else { break };
        best.cells.push(cell);
        if let Some(c) = best.counts.get_mut(cell.model) {
            *c += 1;
        }
        best.count += 1;
        if let Some(t) = tier.get(cell.model).and_then(|t| best.tiers.get_mut(*t)) {
            *t += 1;
        }
        best.area += cell.w * cell.h;
        best.cut_count = k.cut_count;
        best.cut_length = k.cut_length;
        best.work = k.work;
    }
    for variant in band_variants(&best.cells) {
        if let Some(k) = knives.get(&variant)
            && (k.work < best.work || (k.work == best.work && k.cut_count < best.cut_count))
        {
            best.cells = variant;
            best.work = k.work;
            best.cut_count = k.cut_count;
            best.cut_length = k.cut_length;
        }
    }
    best.capacity_limited = best.count >= limit && theoretical > limit as f64;
    Ok(best)
}

#[cfg(test)]
#[path = "dancard_kts_tests.rs"]
mod tests;
