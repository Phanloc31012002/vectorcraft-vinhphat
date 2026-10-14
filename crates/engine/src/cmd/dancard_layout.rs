//! DanCard — Dàn Card (tab Dàn file), cùng quy tắc `dcDan` của DanCard trên Illustrator.
//!
//! Chọn các mẫu card (1 cột: in 1 mặt; 2 cột: mỗi hàng [trước][sau], in 2 mặt), chọn loại card
//! rồi dàn: mỗi mẫu được clip đúng khổ (clip group có sẵn thì giữ), Expand Appearance, rồi nhân
//! bản vào các ô của loại card đã chọn; mặt trước lên artboard 1, mặt sau lên artboard 2 (đối xứng
//! để lật giấy in mặt sau). Artboard lấy từ file pon chung (nếu có), nội dung cũ trong artboard
//! được dọn trước (giữ các mẫu đang chọn). Sai khổ thì thử tự sửa: tìm nền đúng khổ rồi raster
//! theo khung đó, không có thì raster cả object rồi clip đúng khổ ở tâm artboard. Cả lần dàn là
//! một bước hoàn tác.
//!
//! Toạ độ của các ô được viết như DanCard (y hướng lên, gốc góc trên trái cụm) rồi đổi sang trang
//! của VectorCraft (y hướng xuống).

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::{Appearance, Artboard, Document, Node, NodeId, NodeKind, RasterEffectsSettings, Scaling};
use vectorcraft_geom::{Affine, FillRule, Point, Rect, shapes};
use vectorcraft_render::encode::{RasterExportOptions, RasterFormat};

use super::raster::{Fit, image_node, render_raster};
use super::{int_param, number_param, roots_in_paint_order, seen_bounds};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

const MM: f64 = 72.0 / 25.4;
/// Dung sai kích thước khi kiểm tra (mm).
const TOL: f64 = 0.5;
/// Card đôi 18,4 × 5,6 (cạnh dài, mm) và dung sai nhận diện.
const DBL_LONG: f64 = 184.0;
const DBL_SHORT: f64 = 56.0;
const DBL_TOL: f64 = 6.0;
/// Ghi chú mặc định cho Cardoffset.
pub(crate) const DEFAULT_NOTE: &str = "cardoffset - cán mờ - bỏ hộp nhựa";

/// Một loại card: khổ (mm, card nằm ngang), số chỗ, số mẫu hợp lệ.
pub(crate) struct Layout {
    pub key: &'static str,
    pub name: &'static str,
    pub w: f64,
    pub h: f64,
    pub total: usize,
    pub divisors: &'static [usize],
    /// Khe giữa các khối cố định (mm), bỏ qua ô nhập khe.
    block_gap: Option<f64>,
    /// Đưa cụm đã dàn xuống dưới cùng.
    send_to_back: bool,
    /// Voucher: có thể ghép thêm card 9,2 × 5,6.
    pub voucher: bool,
}

pub(crate) const LAYOUTS: &[Layout] = &[
    Layout {
        key: "9.2x5.6",
        name: "9.2 x 5.6  (20 chỗ)",
        w: 92.0,
        h: 56.0,
        total: 20,
        divisors: &[1, 2, 4, 5, 10, 20],
        block_gap: None,
        send_to_back: false,
        voucher: false,
    },
    Layout {
        key: "9x5.5",
        name: "9 x 5.5  (20 chỗ)",
        w: 90.0,
        h: 55.0,
        total: 20,
        divisors: &[1, 2, 4, 5, 10, 20],
        block_gap: None,
        send_to_back: false,
        voucher: false,
    },
    Layout {
        key: "9.2x5.2",
        name: "9.2 x 5.2  (21 chỗ)",
        w: 92.0,
        h: 52.0,
        total: 21,
        divisors: &[1, 3, 7, 21],
        block_gap: Some(5.0),
        send_to_back: false,
        voucher: false,
    },
    Layout {
        key: "8.7x5.5",
        name: "8.7 x 5.5  (22 chỗ)",
        w: 87.0,
        h: 55.0,
        total: 22,
        divisors: &[1, 2, 11, 22],
        block_gap: Some(0.0),
        send_to_back: true,
        voucher: false,
    },
    Layout {
        key: "10.2x15.2",
        name: "10.2 x 15.2  (6 chỗ)",
        w: 152.0,
        h: 102.0,
        total: 6,
        divisors: &[1, 2, 3, 6],
        block_gap: None,
        send_to_back: false,
        voucher: false,
    },
    Layout {
        key: "15.2x7.2",
        name: "15.2 x 7.2  (voucher, 8 chỗ)",
        w: 152.0,
        h: 72.0,
        total: 8,
        divisors: &[1, 2, 4, 8],
        block_gap: None,
        send_to_back: false,
        voucher: true,
    },
];

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.layout",
        "DanCard: Dàn card",
        [],
        None,
        "{layout: 9.2x5.6|9x5.5|9.2x5.2|8.7x5.5|10.2x15.2|15.2x7.2, voucherWithCard?: bool (15.2x7.2: ghép thêm card 9,2 × 5,6), width?, height?: mm (khổ thẻ, mặc định của loại card), gap?: mm (khe giữa khối, mặc định 2; 9.2x5.2 luôn 5, 8.7x5.5 luôn 0), rotation?: 90|-90 (chiều xoay thẻ dọc, mặc định 90), offset?: bool (Cardoffset: thêm dòng ghi chú), note?: string, pon?: đường dẫn file pon chung (.ai; artboard và pon lấy từ đó, trống: dùng artboard sẵn có)} dàn các mẫu đang chọn (1 cột: 1 mặt; 2 cột: mỗi hàng [trước][sau]) vào các ô của loại card, mặt trước artboard 1, mặt sau artboard 2, một bước hoàn tác → {message, count, models, twoSides, groups, warnings}",
        has_selection,
        layout_cmd
    )]
}

// ---------------------------------------------------------------- các ô

/// Một ô: tâm (toạ độ DanCard: y hướng lên, gốc góc trên trái cụm) và góc xoay (độ, ngược chiều
/// kim đồng hồ).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slot {
    pub cx: f64,
    pub cy: f64,
    pub rot: f64,
}

fn slot(cx: f64, cy: f64, rot: f64) -> Slot {
    Slot { cx, cy, rot }
}

/// Kích thước dàn (pt) và chiều xoay thẻ dọc.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    pub w: f64,
    pub h: f64,
    pub g: f64,
    pub rot: f64,
}

/// Các ô mặt trước (`front`) hoặc mặt sau của loại `key`, đúng thứ tự lấp của DanCard.
pub(crate) fn layout_slots(key: &str, front: bool, c: Ctx) -> Vec<Slot> {
    let (w, h, g, rot) = (c.w, c.h, c.g, c.rot);
    let mut s = vec![];
    match key {
        "9.2x5.6" | "9x5.5" => {
            let (vw, vh) = (h, w);
            let (left_w, left_h) = (4.0 * vw, 3.0 * vh);
            let bot_t = -(left_h + g);
            if front {
                // Cột phải 6 ngang.
                let rl = left_w + g;
                for row in 0..6 {
                    s.push(slot(rl + w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                }
                // Khối 12 dọc, mỗi hàng phải → trái, xoay ROT + 180.
                for row in 0..3 {
                    for col in (0..4).rev() {
                        s.push(slot(col as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot + 180.0));
                    }
                }
                // Dải 2 dưới.
                for col in 0..2 {
                    s.push(slot(col as f64 * w + w / 2.0, bot_t - h / 2.0, 0.0));
                }
            } else {
                // Cột 6 ngang trái.
                for row in 0..6 {
                    s.push(slot(w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                }
                // Khối 12 dọc phải, xoay ROT, lấp trái → phải.
                let b12 = w + g;
                for row in 0..3 {
                    for col in 0..4 {
                        s.push(slot(b12 + col as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot + 360.0));
                    }
                }
                // Dải 2 dưới phải.
                let right_edge = b12 + left_w;
                for col in 0..2 {
                    s.push(slot(right_edge - (col as f64 + 1.0) * w + w / 2.0, bot_t - h / 2.0, 0.0));
                }
            }
        }
        "9.2x5.2" => {
            let (vw, vh) = (h, w);
            let bot_t = -(3.0 * vh + g);
            if front {
                // Khối 18 dọc: 6 cột × 3 hàng, theo cột trái → phải, mỗi cột trên → dưới.
                for col in 0..6 {
                    for row in 0..3 {
                        s.push(slot(col as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot));
                    }
                }
                // Dải 3 ngang dưới, căn mép phải khối 18.
                let right_edge = 6.0 * vw;
                for col in 0..3 {
                    s.push(slot(right_edge - (3.0 - col as f64) * w + w / 2.0, bot_t - h / 2.0, 0.0));
                }
            } else {
                for col in 0..6 {
                    for row in 0..3 {
                        let mcol = 5 - col;
                        s.push(slot(mcol as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot + 180.0));
                    }
                }
                for col in 0..3 {
                    let mc = 2 - col;
                    s.push(slot(mc as f64 * w + w / 2.0, bot_t - h / 2.0, 0.0));
                }
            }
        }
        "8.7x5.5" => {
            if front {
                // Khối 18 ngang: 3 cột × 6 hàng, theo cột trên → dưới.
                for col in 0..3 {
                    for row in 0..6 {
                        s.push(slot(col as f64 * w + w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                    }
                }
                // Cột phải 4 dọc.
                let rl = 3.0 * w + g;
                for row in 0..4 {
                    s.push(slot(rl + h / 2.0, -(row as f64) * w - w / 2.0, rot));
                }
            } else {
                let block_l = h + g;
                for col in 0..3 {
                    let mcol = 2 - col;
                    for row in 0..6 {
                        s.push(slot(block_l + mcol as f64 * w + w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                    }
                }
                for row in 0..4 {
                    s.push(slot(h / 2.0, -(row as f64) * w - w / 2.0, rot + 180.0));
                }
            }
        }
        "10.2x15.2" => {
            for row in 0..3 {
                for col in 0..2 {
                    let c = if front { col } else { 1 - col };
                    s.push(slot(c as f64 * w + w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                }
            }
        }
        "15.2x7.2" => {
            // Mặt trước: cột phải 1–4 (trên → dưới), rồi cột trái 5–8; mặt sau đối xứng.
            let cols = if front { [1.0, 0.0] } else { [0.0, 1.0] };
            for col in cols {
                for row in 0..4 {
                    s.push(slot(col * w + w / 2.0, -(row as f64) * h - h / 2.0, 0.0));
                }
            }
        }
        _ => {}
    }
    s
}

/// Ô cho card đôi (layout 9.2x5.6): ô đôi ưu tiên dải dưới rồi khối 12; ô đơn còn trống. Trả (ô
/// đôi đã dùng, ô đơn còn lại).
fn mixed_slots(front: bool, c: Ctx, need_double: usize) -> (Vec<Slot>, Vec<Slot>) {
    let (w, h, g, rot) = (c.w, c.h, c.g, c.rot);
    let (vw, vh) = (h, w);
    let (left_w, left_h) = (4.0 * vw, 3.0 * vh);
    let bot_t = -(left_h + g);
    // (ô, loại: None = dải dưới, Some(cột) = khối 12)
    let mut doubles: Vec<(Slot, Option<usize>)> = vec![];
    let mut singles: Vec<(Slot, String)> = vec![];
    if front {
        doubles.push((slot(w, bot_t - h / 2.0, 0.0), None));
        for col in (0..4).rev() {
            doubles.push((slot(col as f64 * vw + vw / 2.0, -2.0 * vh, rot + 180.0), Some(col)));
        }
        let rl = left_w + g;
        for r in 0..6 {
            singles.push((slot(rl + w / 2.0, -(r as f64) * h - h / 2.0, 0.0), format!("phai{r}")));
        }
        for row in 0..3 {
            for cc in (0..4).rev() {
                singles.push((slot(cc as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot + 180.0), format!("k12_{row}_{cc}")));
            }
        }
        for d in 0..2 {
            singles.push((slot(d as f64 * w + w / 2.0, bot_t - h / 2.0, 0.0), format!("duoi{d}")));
        }
    } else {
        let b12 = w + g;
        let right_edge = b12 + left_w;
        doubles.push((slot(right_edge - w, bot_t - h / 2.0, 0.0), None));
        for col in 0..4 {
            doubles.push((slot(b12 + col as f64 * vw + vw / 2.0, -2.0 * vh, rot + 360.0), Some(col)));
        }
        for r in 0..6 {
            singles.push((slot(w / 2.0, -(r as f64) * h - h / 2.0, 0.0), format!("phai{r}")));
        }
        for row in 0..3 {
            for cc in 0..4 {
                singles.push((slot(b12 + cc as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, rot + 360.0), format!("k12_{row}_{cc}")));
            }
        }
        for d in 0..2 {
            singles.push((slot(right_edge - (d as f64 + 1.0) * w + w / 2.0, bot_t - h / 2.0, 0.0), format!("duoi{d}")));
        }
    }
    let used: Vec<(Slot, Option<usize>)> = doubles.into_iter().take(need_double).collect();
    let mut blocked: Vec<String> = vec![];
    for (_, kind) in &used {
        match kind {
            None => blocked.extend(["duoi0".to_string(), "duoi1".to_string()]),
            Some(col) => blocked.extend([format!("k12_1_{col}"), format!("k12_2_{col}")]),
        }
    }
    let free = singles.into_iter().filter(|(_, tag)| !blocked.contains(tag)).map(|(s, _)| s).collect();
    (used.into_iter().map(|(s, _)| s).collect(), free)
}

// ---------------------------------------------------------------- đo

/// Vùng nhìn thấy thật (visibleUnion của DanCard), không đo được thì khung hình học.
pub(super) fn vis(n: &Node) -> Option<Rect> {
    seen_bounds(n).or_else(|| n.visual_bounds()).or_else(|| n.geometric_bounds())
}

/// Kích thước nhìn thấy (mm).
fn vis_mm(n: &Node) -> (f64, f64) {
    vis(n).map_or((0.0, 0.0), |b| (b.width() / MM, b.height() / MM))
}

/// Card đang clip đo theo mask; card thường theo vùng nhìn thấy (mm).
fn card_mm(n: &Node) -> (f64, f64) {
    if let NodeKind::Group { children, clip: true } = &n.kind
        && let Some(b) = children.first().and_then(|m| m.geometric_bounds())
    {
        return (b.width() / MM, b.height() / MM);
    }
    vis_mm(n)
}

fn matches_size((w, h): (f64, f64), (tw, th): (f64, f64), tol: f64) -> bool {
    (w.max(h) - tw.max(th)).abs() <= tol && (w.min(h) - tw.min(th)).abs() <= tol
}

fn is_double(n: &Node) -> bool {
    let (w, h) = vis_mm(n);
    (w.max(h) - DBL_LONG).abs() <= DBL_TOL
}

/// Số cột của các object (gom tâm X cách nhau dưới `threshold`).
fn columns(xs: &mut [f64], threshold: f64) -> usize {
    xs.sort_by(f64::total_cmp);
    1 + xs.windows(2).filter(|w| w[1] - w[0] > threshold).count()
}

// ---------------------------------------------------------------- sửa trên tài liệu

/// Xoay `id` `deg` độ (ngược chiều kim đồng hồ) quanh tâm vùng nhìn thấy.
pub(super) fn rotate_node(d: &mut Document, id: NodeId, deg: f64) {
    if deg.rem_euclid(360.0) == 0.0 {
        return;
    }
    let Some(c) = d.node(id).and_then(vis).map(|b| b.center()) else { return };
    let m = Affine::translate(c.to_vec2()) * Affine::rotate(-deg.to_radians()) * Affine::translate(-c.to_vec2());
    if let Some(n) = d.node_mut(id) {
        n.transform(m, Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
}

pub(super) fn translate_node(d: &mut Document, id: NodeId, by: vectorcraft_geom::Vec2) {
    if by.hypot() < 1e-9 {
        return;
    }
    if let Some(n) = d.node_mut(id) {
        n.transform(Affine::translate(by), Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
}

/// Bọc `id` trong clip group: mask `w` × `h` (pt) đặt tâm tại `c`.
pub(super) fn wrap_clip(d: &mut Document, id: NodeId, c: Point, w: f64, h: f64) -> Result<NodeId> {
    let (parent, index, _) = d.position(id).ok_or(EngineError::NoNode(id))?;
    let mask_id = d.alloc_id();
    let path = shapes::rectangle(Rect::new(c.x - w / 2.0, c.y - h / 2.0, c.x + w / 2.0, c.y + h / 2.0));
    let mut mask = Node::new(mask_id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: true, guide: false });
    mask.appearance = Appearance::basic(Paint::None, Paint::None, 0.0).into();
    mask.name = Some("Khung clip".into());
    d.insert(parent, index + 1, mask)?;
    let gid = d.alloc_id();
    Ok(d.wrap(&[id, mask_id], |mut children| {
        children.rotate_right(1);
        Node::new(gid, NodeKind::Group { children, clip: true })
    })?)
}

/// Object › Expand Appearance trên `id` (giữ id).
pub(super) fn expand(d: &mut Document, id: NodeId) {
    let brushes = vectorcraft_brush::library(d);
    let mut out = vec![];
    crate::cmd::effectcmd::expand_node(d, id, &brushes, &mut out);
}

/// Clip `id` về khung `w` × `h` mm quanh tâm nội dung thật (clip group có sẵn thì giữ), rồi
/// Expand Appearance. Trả id của card.
fn auto_clip(d: &mut Document, id: NodeId, w: f64, h: f64) -> Result<NodeId> {
    let n = d.node(id).ok_or(EngineError::NoNode(id))?;
    let out = if n.clips() {
        id
    } else {
        let c = vis(n).ok_or_else(|| EngineError::Other("Không đo được vùng nhìn thấy của mẫu.".into()))?.center();
        wrap_clip(d, id, c, w * MM, h * MM)?
    };
    expand(d, out);
    Ok(out)
}

/// Raster `id` trong vùng `region` (500 ppi, CMYK, nền trong suốt) và thay nó bằng ảnh.
pub(super) fn raster_in(d: &mut Document, id: NodeId, region: Rect) -> Result<NodeId> {
    let n = d.node(id).cloned().ok_or(EngineError::NoNode(id))?;
    let opts = RasterExportOptions { ppi: 500.0, background: None, anti_alias: vectorcraft_render::AntiAlias::Art, ..Default::default() };
    let inks = d.color_mode == vectorcraft_doc::ColorMode::Cmyk;
    let mut shown = n.clone();
    shown.visible = true;
    let r = render_raster(
        d,
        vec![shown],
        region,
        Affine::IDENTITY,
        500.0,
        Fit::Exact,
        &opts.render_options(RasterFormat::Png),
        &RasterEffectsSettings::default(),
        inks,
    )?;
    let img = image_node(d, r, n.name.clone(), 0.0);
    Ok(d.wrap(&[id], |_| img)?)
}

/// Object con (nền) có khung hình học khớp `w` × `h` mm nhất trong `n`, trong dung sai `tol` mm.
fn find_frame(n: &Node, w: f64, h: f64, tol: f64) -> Option<Rect> {
    fn scan(n: &Node, w: f64, h: f64, best: &mut Option<(f64, Rect)>) {
        if let Some(b) = n.geometric_bounds() {
            let (bw, bh) = (b.width() / MM, b.height() / MM);
            let err = (bw.max(bh) - w.max(h)).abs() + (bw.min(bh) - w.min(h)).abs();
            if best.is_none_or(|(e, _)| err < e) {
                *best = Some((err, b));
            }
        }
        if matches!(n.kind, NodeKind::Group { .. }) {
            for c in n.children().into_iter().flatten() {
                scan(c, w, h, best);
            }
        }
    }
    let mut best = None;
    scan(n, w, h, &mut best);
    best.filter(|(e, _)| *e <= tol).map(|(_, b)| b)
}

/// Tự sửa object sai khổ: (A) có nền đúng khổ thì raster theo khung nền; (B) không thì raster cả
/// object, đưa ảnh về tâm artboard `board` rồi clip đúng khổ.
fn auto_fix(d: &mut Document, id: NodeId, w: f64, h: f64, board: Rect) -> Option<NodeId> {
    let n = d.node(id)?.clone();
    if let Some(frame) = find_frame(&n, w, h, 2.0)
        && let Ok(img) = raster_in(d, id, frame)
    {
        return Some(img);
    }
    let all = vis(&n)?;
    let img = raster_in(d, id, all).ok()?;
    let c = board.center();
    let at = d.node(img).and_then(|n| n.geometric_bounds())?.center();
    translate_node(d, img, c - at);
    wrap_clip(d, img, c, w * MM, h * MM).ok()
}

/// Các object cấp đầu (con trực tiếp của layer không khóa, đang hiện; không khóa, không ẩn) có tâm
/// nằm trong `area`, trừ những object chứa thứ trong `keep`.
pub(super) fn top_level_in(d: &Document, area: Rect, keep: &[NodeId]) -> Vec<NodeId> {
    let mut out = vec![];
    for layer in d.layers.iter().filter(|l| !l.locked && l.visible) {
        for c in layer.children().into_iter().flatten() {
            if c.is_layer() || c.locked || !c.visible {
                continue;
            }
            let Some(b) = c.geometric_bounds() else { continue };
            let p = b.center();
            if p.x < area.x0 || p.x > area.x1 || p.y < area.y0 || p.y > area.y1 {
                continue;
            }
            let protects = keep.iter().any(|k| *k == c.id || inside(d, *k, c.id));
            if !protects {
                out.push(c.id);
            }
        }
    }
    out
}

/// `id` nằm trong `ancestor` (ở mọi tầng).
pub(super) fn inside(d: &Document, id: NodeId, ancestor: NodeId) -> bool {
    let mut up = d.parent_of(id);
    while let Some(p) = up {
        if p == ancestor {
            return true;
        }
        up = d.parent_of(p);
    }
    false
}

pub(super) fn union_of(rects: impl Iterator<Item = Rect>) -> Option<Rect> {
    rects.fold(None, |acc: Option<Rect>, r| Some(acc.map_or(r, |a| a.union(r))))
}

/// File pon: artboard của nó thay artboard của tài liệu, pon (nếu có) chép vào layer `layer`.
fn import_pon(d: &mut Document, pon: &Document, layer: Option<NodeId>) -> Result<usize> {
    let mut art: Vec<Node> = pon.layers.iter().flat_map(|l| l.children().into_iter().flatten().map(|c| (**c).clone())).collect();
    crate::cmd::place::adopt::adopt(d, pon, &mut art);
    let count = art.len();
    for n in art {
        let n = d.reid(&n);
        d.insert(layer, usize::MAX, n)?;
    }
    if !pon.artboards.is_empty() {
        d.artboards = pon.artboards.clone();
    }
    Ok(count)
}

/// Đủ 2 artboard: thiếu thì thêm artboard 2 cùng khổ ở bên phải artboard 1, cách 10 mm.
fn ensure_two_artboards(d: &mut Document) {
    if d.artboards.len() >= 2 {
        return;
    }
    let Some(first) = d.artboards.first().cloned() else { return };
    let r = first.rect;
    let mut ab: Artboard = first;
    ab.rect = Rect::new(r.x1 + 10.0 * MM, r.y0, r.x1 + 10.0 * MM + r.width(), r.y1);
    ab.name = format!("Artboard {}", d.artboards.len() + 1);
    ab.id = d.next_artboard_id();
    d.artboards.push(ab);
}

/// In 1 mặt: xoá artboard 2 và các object cấp đầu có tâm trong nó.
fn remove_second_artboard(d: &mut Document, keep: &[NodeId]) -> Result<()> {
    let Some(r) = d.artboards.get(1).map(|a| a.rect) else { return Ok(()) };
    let gone = top_level_in(d, r, keep);
    if !gone.is_empty() {
        d.remove_many(&gone)?;
    }
    d.artboards.remove(1);
    Ok(())
}

/// Nhân bản `src` vào ô `s` (toạ độ DanCard, cộng vào gốc `origin` trên trang).
fn copy_at(d: &mut Document, src: NodeId, s: Slot, origin: Point) -> Option<Node> {
    let n = d.node(src)?.clone();
    let mut c = d.reid(&n);
    if s.rot.rem_euclid(360.0) != 0.0
        && let Some(ctr) = vis(&c).map(|b| b.center())
    {
        let m = Affine::translate(ctr.to_vec2()) * Affine::rotate(-s.rot.to_radians()) * Affine::translate(-ctr.to_vec2());
        c.transform(m, Scaling { strokes: true, patterns: true, ..Scaling::default() });
    }
    let at = vis(&c)?.center();
    let want = Point::new(origin.x + s.cx, origin.y - s.cy);
    c.transform(Affine::translate(want - at), Scaling { strokes: true, patterns: true, ..Scaling::default() });
    Some(c)
}

/// Gom `copies` thành một group trên layer `layer`, đặt tâm cụm vào tâm artboard `board`.
fn group_and_center(d: &mut Document, copies: Vec<Node>, board: usize, layer: Option<NodeId>, to_back: bool) -> Result<Option<NodeId>> {
    if copies.is_empty() {
        return Ok(None);
    }
    let Some(ab) = d.artboards.get(board).map(|a| a.rect) else { return Ok(None) };
    let Some(b) = union_of(copies.iter().filter_map(vis)) else { return Ok(None) };
    let by = ab.center() - b.center();
    let gid = d.alloc_id();
    let mut g = Node::group(gid, copies.into_iter().map(Arc::new).collect());
    g.transform(Affine::translate(by), Scaling { strokes: true, patterns: true, ..Scaling::default() });
    let id = d.insert(layer, if to_back { 0 } else { usize::MAX }, g)?;
    Ok(Some(id))
}

// ---------------------------------------------------------------- lệnh

/// Tham số chung của một lần dàn.
struct Opts {
    w: f64,
    h: f64,
    gap: f64,
    rot: f64,
    offset: bool,
    note: String,
    pon: Option<Document>,
}

fn pon_doc(p: &Value) -> Result<Option<Document>> {
    let Some(path) = str_param(p, "pon").map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None) };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = crate::cmd::fileio::read_file(path).map_err(|e| EngineError::Other(format!("Không đọc được file pon: {e}")))?;
        let name = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("pon.ai");
        let loaded = crate::cmd::fileio::load(name, &bytes).map_err(|e| EngineError::Other(format!("Không mở được file pon {path}: {e}")))?;
        Ok(Some(loaded.doc))
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(EngineError::Other(format!("Bản web không đọc được file pon {path}.")))
    }
}

/// Lớp đang vẽ của tài liệu.
pub(super) fn active_layer(s: &Session) -> Option<NodeId> {
    s.active().and_then(|st| st.current_layer())
}

fn layout_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dancard.layout";
    let key = str_param(p, "layout").unwrap_or("");
    let layout = LAYOUTS.iter().find(|l| l.key == key).ok_or_else(|| bad(C, format!("Không tìm thấy loại card '{key}'.")))?;
    let roots = roots_in_paint_order(s)?;
    let rot = match number_param(p, "rotation") {
        Some(r) if r < 0.0 => -90.0,
        _ => 90.0,
    };
    let opts = Opts {
        w: number_param(p, "width").filter(|v| *v > 0.0).unwrap_or(layout.w),
        h: number_param(p, "height").filter(|v| *v > 0.0).unwrap_or(layout.h),
        gap: layout.block_gap.unwrap_or_else(|| number_param(p, "gap").filter(|v| *v >= 0.0).unwrap_or(2.0)),
        rot,
        offset: bool_or(p, "offset", false),
        note: str_param(p, "note").filter(|n| !n.trim().is_empty()).unwrap_or(DEFAULT_NOTE).to_string(),
        pon: pon_doc(p)?,
    };
    if layout.voucher && bool_or(p, "voucherWithCard", false) {
        return voucher_with_card(s, roots, opts);
    }
    if roots.is_empty() {
        return Err(EngineError::Other("Chưa chọn object nào để dàn.".into()));
    }
    let st = s.doc()?;
    let d = &st.doc;
    // 1 cột: in 1 mặt; 2 cột trở lên: in 2 mặt.
    let centers: Vec<(NodeId, Point)> = roots.iter().filter_map(|id| Some((*id, vis(d.node(*id)?)?.center()))).collect();
    let mut xs: Vec<f64> = centers.iter().map(|(_, c)| c.x).collect();
    let two_sides = columns(&mut xs, 0.5 * layout.w * MM) >= 2;
    let n_models = if two_sides {
        if roots.len() % 2 != 0 {
            return Err(EngineError::Other(format!("In 2 mặt cần số object chẵn (mỗi mẫu 2: trước+sau). Đang chọn {} object lẻ.", roots.len())));
        }
        roots.len() / 2
    } else {
        roots.len()
    };
    // Card đôi (layout 9.2x5.6): chiếm 2 chỗ; đếm theo mẫu (2 mặt: đo mặt trước).
    // Hàng: trên → dưới; 2 mặt: mỗi hàng 2 object, trái = trước.
    let mut sorted = centers.clone();
    sorted.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    let rows: Vec<Vec<NodeId>> = if two_sides {
        sorted
            .chunks(2)
            .map(|pair| {
                let mut pair = pair.to_vec();
                pair.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
                pair.into_iter().map(|(id, _)| id).collect()
            })
            .collect()
    } else {
        sorted.into_iter().map(|(id, _)| vec![id]).collect()
    };
    let doubles: Vec<bool> = rows.iter().map(|r| layout.key == "9.2x5.6" && r.first().and_then(|id| d.node(*id)).is_some_and(is_double)).collect();
    let n_double = doubles.iter().filter(|x| **x).count();
    let mut per_model = layout.total / n_models.max(1);
    if n_double > 0 {
        let cho_bo = (n_models - n_double) + n_double * 2;
        if cho_bo == 0 || layout.total % cho_bo != 0 {
            return Err(EngineError::Other(format!(
                "Số mẫu không đều. Chỗ quy đổi (nhỏ + đôi×2) = {cho_bo} phải là ước của {} (1,2,4,5,10,20). Chọn lại số con (đôi=2 chỗ, nhỏ=1 chỗ).",
                layout.total
            )));
        }
        let k = layout.total / cho_bo;
        if n_double * k > 5 {
            return Err(EngineError::Other(format!(
                "Card đôi chiếm tối đa 5 chỗ đôi. Đang cần {} chỗ đôi (nĐôi {n_double} × nhân {k}).",
                n_double * k
            )));
        }
        per_model = 1;
    } else if !layout.divisors.contains(&n_models) {
        return Err(EngineError::Other(format!(
            "Số mẫu {n_models} không hợp lệ cho khổ này. Cần chọn số object sao cho số mẫu là: {}.",
            layout.divisors.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")
        )));
    }
    let layer = active_layer(s);
    let board_index = int_param(p, "artboard").and_then(|i| usize::try_from(i).ok()).unwrap_or(0);
    let board_now = st.doc.artboards.get(board_index).or(st.doc.artboards.first()).map(|a| a.rect).unwrap_or_default();
    let ctx = Ctx { w: opts.w * MM, h: opts.h * MM, g: opts.gap * MM, rot: opts.rot };
    let (cur_w, cur_h) = (opts.w, opts.h);
    let label = format!("Dàn card {} (DanCard)", layout.key);
    let pon = opts.pon.as_ref();
    let result = s.edit(&label, |d, sel| {
        let layer = layer.or_else(|| d.default_layer());
        // Kiểm tra khổ (dung sai 0,5 mm); sai thì thử tự sửa.
        let mut rows = rows.clone();
        let mut bad_sizes = vec![];
        for (ri, row) in rows.iter_mut().enumerate() {
            let target = if doubles.get(ri).copied().unwrap_or(false) { (DBL_LONG, DBL_SHORT) } else { (cur_w, cur_h) };
            for id in row.iter_mut() {
                let Some(n) = d.node(*id) else { continue };
                if n.clips() || matches_size(vis_mm(n), target, TOL) {
                    continue;
                }
                let before = vis_mm(n);
                match auto_fix(d, *id, target.0, target.1, board_now) {
                    Some(fixed) if d.node(fixed).is_some_and(|n| matches_size(vis_mm(n), target, TOL)) => *id = fixed,
                    _ => bad_sizes.push(before),
                }
            }
        }
        if let Some((w, h)) = bad_sizes.first() {
            return Err(EngineError::Other(format!(
                "Sai kích thước. {} object không khớp khổ {:.1}x{:.1}cm (vd object đo được {:.1}x{:.1}). Đã thử tự sửa (tìm nền + rasterize) nhưng không được. Cách sửa: chỉnh object về đúng khổ, hoặc bỏ nền thừa/object lạ.",
                bad_sizes.len(),
                cur_w / 10.0,
                cur_h / 10.0,
                w / 10.0,
                h / 10.0
            )));
        }
        // Dọn nội dung cũ trong các artboard (giữ các mẫu đang chọn).
        let keep: Vec<NodeId> = rows.iter().flatten().copied().collect();
        if let Some(area) = union_of(d.artboards.iter().map(|a| a.rect)) {
            let gone = top_level_in(d, area, &keep);
            if !gone.is_empty() {
                d.remove_many(&gone)?;
            }
        }
        // Clip đúng khổ, Expand Appearance, xoay về hướng khổ đích.
        let mut models: Vec<(NodeId, Option<NodeId>, bool)> = vec![];
        for (ri, row) in rows.iter().enumerate() {
            let dbl = doubles.get(ri).copied().unwrap_or(false);
            let (tw, th) = if dbl { (DBL_LONG, DBL_SHORT) } else { (cur_w, cur_h) };
            let mut faces = vec![];
            for (fi, id) in row.iter().enumerate() {
                let Some(n) = d.node(*id) else { continue };
                let (w, h) = card_mm(n);
                let turn = w > 0.0 && h > 0.0 && (w >= h) != (tw >= th);
                let (cw, ch, deg) = if turn { (th, tw, if fi == 1 { -90.0 } else { 90.0 }) } else { (tw, th, 0.0) };
                let card = auto_clip(d, *id, cw, ch)?;
                rotate_node(d, card, deg);
                faces.push(card);
            }
            let Some(front) = faces.first().copied() else { continue };
            models.push((front, faces.get(1).copied(), dbl));
        }
        if models.len() != n_models {
            return Err(EngineError::Other(format!("Số hàng ({}) != số mẫu.", models.len())));
        }
        // Artboard chung từ file pon.
        if let Some(pon) = pon {
            import_pon(d, pon, layer)?;
        }
        let keep: Vec<NodeId> = models.iter().flat_map(|(f, b, _)| [Some(*f), *b]).flatten().collect();
        if two_sides {
            ensure_two_artboards(d);
        } else {
            remove_second_artboard(d, &keep)?;
        }
        // Dàn từng mặt.
        let mut groups = vec![];
        for (face, board) in [(true, 0usize), (false, 1usize)] {
            if !face && !two_sides {
                continue;
            }
            let src = |m: &(NodeId, Option<NodeId>, bool)| if face { Some(m.0) } else { m.1 };
            let mut copies = vec![];
            if n_double > 0 {
                let cho_bo = (n_models - n_double) + n_double * 2;
                let k = (layout.total / cho_bo.max(1)).max(1);
                let (dbl_slots, singles) = mixed_slots(face, ctx, n_double * k);
                let dbl_models: Vec<_> = models.iter().filter(|m| m.2).collect();
                let norm_models: Vec<_> = models.iter().filter(|m| !m.2).collect();
                for (i, sl) in dbl_slots.iter().enumerate() {
                    let Some(m) = dbl_models.get((i / k).min(dbl_models.len().saturating_sub(1))) else { continue };
                    if let Some(c) = src(m).and_then(|id| copy_at(d, id, *sl, Point::ZERO)) {
                        copies.push(c);
                    }
                }
                if !norm_models.is_empty() {
                    for (i, sl) in singles.iter().enumerate() {
                        let Some(m) = norm_models.get((i / k).min(norm_models.len() - 1)) else { continue };
                        if let Some(c) = src(m).and_then(|id| copy_at(d, id, *sl, Point::ZERO)) {
                            copies.push(c);
                        }
                    }
                }
            } else {
                let slots = layout_slots(layout.key, face, ctx);
                for (m, model) in models.iter().enumerate() {
                    for k in 0..per_model {
                        let Some(sl) = slots.get(m * per_model + k) else { continue };
                        if let Some(c) = src(model).and_then(|id| copy_at(d, id, *sl, Point::ZERO)) {
                            copies.push(c);
                        }
                    }
                }
            }
            if let Some(g) = group_and_center(d, copies, board, layer, layout.send_to_back)? {
                groups.push(g);
            }
        }
        sel.set(groups.iter().copied());
        Ok(groups)
    })?;
    let count = layout.total;
    let mut warnings = vec![];
    if opts.offset
        && let Err(e) = offset_note(s, &opts.note)
    {
        warnings.push(format!("Không thêm được ghi chú cardoffset: {e}"));
    }
    if opts.pon.is_none() {
        warnings.push("Không có file pon → dùng artboard sẵn có của file hiện tại.".to_string());
    }
    let mut message = format!(
        "OK: Đã dàn xong {n_models} mẫu ({}). Mỗi mẫu {per_model} chỗ. {}",
        layout.name.split("  ").next().unwrap_or(layout.name),
        if two_sides { "In 2 mặt: mặt trước → AB1, mặt sau → AB2." } else { "In 1 mặt: chỉ dàn AB1." }
    );
    if layout.key == "9.2x5.6" {
        message.push_str(&if n_double > 0 {
            format!(" Nhận diện {n_double} con đôi (18.4×5.6) + {} con thường.", n_models - n_double)
        } else {
            format!(" Đã dàn {n_models} con thường (không có con đôi 18.4).")
        });
    }
    if opts.offset && warnings.iter().all(|w| !w.contains("cardoffset")) {
        message.push_str(" + Ghi chú cardoffset.");
    }
    Ok(json!({
        "message": message,
        "count": count,
        "models": n_models,
        "twoSides": two_sides,
        "groups": result.iter().map(|g| g.0).collect::<Vec<_>>(),
        "warnings": warnings,
    }))
}

/// Cardoffset: dòng ghi chú cỡ 14 pt dưới artboard 1, lệch trái 30 mm khỏi tâm (như DanCard).
fn offset_note(s: &mut Session, note: &str) -> Result<()> {
    let st = s.doc()?;
    let Some(ab) = st.doc.artboards.first().map(|a| a.rect) else { return Ok(()) };
    let x = ab.center().x - 30.0 * MM;
    let y = ab.y1 - 5.5 * MM;
    s.execute("text.create", &json!({"x": x, "y": y, "text": note, "size": 14, "justify": "center"}))?;
    Ok(())
}

/// Voucher 15,2 × 7,2 ghép card 9,2 × 5,6: 8 voucher ngang (2 cột × 4 hàng) và dải 3 card ngay dưới
/// khối voucher; mặt sau đối xứng.
fn voucher_with_card(s: &mut Session, roots: Vec<NodeId>, opts: Opts) -> Result<Value> {
    const VW: f64 = 152.0;
    const VH: f64 = 72.0;
    const CW: f64 = 92.0;
    const CH: f64 = 56.0;
    if roots.len() < 2 {
        return Err(EngineError::Other("Chưa chọn object voucher/card.".into()));
    }
    let st = s.doc()?;
    let d = &st.doc;
    let mut vouchers = vec![];
    let mut cards = vec![];
    let mut off = 0;
    for id in &roots {
        let Some(n) = d.node(*id) else { continue };
        let size = vis_mm(n);
        let Some(c) = vis(n).map(|b| b.center()) else { continue };
        let (hi, lo) = (size.0.max(size.1), size.0.min(size.1));
        let dv = (hi - VW).abs() + (lo - VH).abs();
        let dc = (hi - CW).abs() + (lo - CH).abs();
        if dv <= dc {
            if matches_size(size, (VW, VH), TOL) { vouchers.push((*id, c)) } else { off += 1 }
        } else if matches_size(size, (CW, CH), TOL) {
            cards.push((*id, c));
        } else {
            off += 1;
        }
    }
    if off > 0 {
        return Err(EngineError::Other(format!(
            "Sai kích thước. {off}/{} object không khớp (voucher cần 15.2x7.2cm, card 9.2x5.6cm). Cách sửa: chỉnh object về đúng khổ rồi chạy lại.",
            roots.len()
        )));
    }
    if vouchers.is_empty() {
        return Err(EngineError::Other("Không thấy voucher (cần ~152x72mm).".into()));
    }
    if vouchers.len() % 2 != 0 {
        return Err(EngineError::Other(format!("Số voucher lẻ ({}). In 2 mặt cần chẵn (mỗi mẫu 2: trước+sau).", vouchers.len())));
    }
    let n_v = vouchers.len() / 2;
    if 8 % n_v != 0 {
        return Err(EngineError::Other(format!("Có {n_v} mẫu voucher, không chia đều 8 chỗ. Cần 1, 2, 4 hoặc 8 mẫu.")));
    }
    let n_c = cards.len() / 2;
    if !cards.is_empty() {
        if cards.len() % 2 != 0 {
            return Err(EngineError::Other(format!("Số card lẻ ({}). In 2 mặt cần chẵn (mỗi mẫu 2: trước+sau).", cards.len())));
        }
        if 3 % n_c != 0 {
            return Err(EngineError::Other(format!("Có {n_c} mẫu card, không chia đều 3 chỗ. Cần 1 hoặc 3 mẫu.")));
        }
    }
    let (per_v, per_c) = (8 / n_v, 3usize.checked_div(n_c).unwrap_or(0));
    let pair_up = |mut v: Vec<(NodeId, Point)>| -> Vec<(NodeId, NodeId)> {
        v.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        v.chunks(2)
            .filter_map(|p| {
                let mut p = p.to_vec();
                p.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
                Some((p.first()?.0, p.get(1)?.0))
            })
            .collect()
    };
    let (v_rows, c_rows) = (pair_up(vouchers), pair_up(cards));
    let layer = active_layer(s);
    let pon = opts.pon.as_ref();
    let groups = s.edit("Dàn voucher + card (DanCard)", |d, sel| {
        let layer = layer.or_else(|| d.default_layer());
        let keep: Vec<NodeId> = v_rows.iter().chain(c_rows.iter()).flat_map(|(a, b)| [*a, *b]).collect();
        if let Some(area) = union_of(d.artboards.iter().map(|a| a.rect)) {
            let gone = top_level_in(d, area, &keep);
            if !gone.is_empty() {
                d.remove_many(&gone)?;
            }
        }
        let mut clip_pairs = |rows: &[(NodeId, NodeId)], w: f64, h: f64| -> Result<Vec<(NodeId, NodeId)>> {
            rows.iter().map(|(f, b)| Ok((auto_clip(d, *f, w, h)?, auto_clip(d, *b, w, h)?))).collect()
        };
        let v_models = clip_pairs(&v_rows, VW, VH)?;
        let c_models = clip_pairs(&c_rows, CW, CH)?;
        if let Some(pon) = pon {
            import_pon(d, pon, layer)?;
        }
        ensure_two_artboards(d);
        let (vw, vh, cw, ch) = (VW * MM, VH * MM, CW * MM, CH * MM);
        let mut groups = vec![];
        for (front, board) in [(true, 0usize), (false, 1usize)] {
            let mut copies = vec![];
            // Voucher: mặt trước cột trái 1–4, cột phải 5–8; mặt sau đối xứng.
            let mut vslots = vec![];
            for col in 0..2 {
                let c = if front { col } else { 1 - col };
                for row in 0..4 {
                    vslots.push(slot(c as f64 * vw + vw / 2.0, -(row as f64) * vh - vh / 2.0, 0.0));
                }
            }
            for (m, (f, b)) in v_models.iter().enumerate() {
                for k in 0..per_v {
                    let (Some(sl), src) = (vslots.get(m * per_v + k), if front { *f } else { *b }) else { continue };
                    copies.extend(copy_at(d, src, *sl, Point::ZERO));
                }
            }
            // Card: dải 3 ngay dưới khối voucher; mặt trước căn trái, mặt sau căn phải và đảo.
            if !c_models.is_empty() {
                let bot_t = -4.0 * vh;
                let left = if front { 0.0 } else { 2.0 * vw - 3.0 * cw };
                for (m, (f, b)) in c_models.iter().enumerate() {
                    for k in 0..per_c {
                        let i = m * per_c + k;
                        let col = if front { i } else { 2 - i.min(2) };
                        let sl = slot(left + col as f64 * cw + cw / 2.0, bot_t - ch / 2.0, 0.0);
                        copies.extend(copy_at(d, if front { *f } else { *b }, sl, Point::ZERO));
                    }
                }
            }
            if let Some(g) = group_and_center(d, copies, board, layer, false)? {
                groups.push(g);
            }
        }
        sel.set(groups.iter().copied());
        Ok(groups)
    })?;
    let mut warnings = vec![];
    if opts.offset
        && let Err(e) = offset_note(s, &opts.note)
    {
        warnings.push(format!("Không thêm được ghi chú cardoffset: {e}"));
    }
    if opts.pon.is_none() {
        warnings.push("Không có file pon → dùng artboard sẵn có của file hiện tại.".to_string());
    }
    let mut message = format!("OK: Đã dàn xong voucher 15.2x7.2 ({n_v} mẫu, {per_v} chỗ/mẫu)");
    if n_c > 0 {
        message.push_str(&format!(" + card 9.2x5.6 ({n_c} mẫu, {per_c} chỗ/mẫu)"));
    }
    message.push_str(". Mặt trước → AB1, mặt sau → AB2.");
    if opts.offset {
        message.push_str(" + Ghi chú cardoffset.");
    }
    Ok(json!({
        "message": message,
        "count": 8 + 3 * usize::from(n_c > 0),
        "models": n_v + n_c,
        "twoSides": true,
        "groups": groups.iter().map(|g| g.0).collect::<Vec<_>>(),
        "warnings": warnings,
    }))
}
