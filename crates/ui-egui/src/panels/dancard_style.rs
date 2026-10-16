//! Giao diện panel "Công cụ bình" theo DanCard trên Illustrator (`css/style.css`, `index.html` của
//! DanCard 2.16): mỗi loại thành phần một kiểu riêng, có quy luật, nhìn là biết.
//!
//! - Nền panel xám xanh. Thanh tab: tab đang mở chữ trắng trên nền ửng hồng, gạch cam bên dưới;
//!   tab khác chữ xám; panel hẹp thì tab tự xuống hàng, mỗi hàng chia đều.
//! - Mục gập (▶ / ▼): thanh nền sáng hơn, chữ đậm; bấm cả thanh để mở / thu.
//! - Nhãn của ô: CHỮ HOA nhỏ màu xám, giãn chữ. Gợi ý: chữ xám nhỏ. Tiêu đề: chữ trắng đậm.
//! - Ô nhập: nền tối, viền xám, đang gõ thì viền xanh; ô số căn giữa; bấm vào ô là chọn hết chữ để
//!   gõ đè (`attachSelectFirst` của DanCard).
//! - Nút chính (chạy lệnh): viên thuốc trải hết bề ngang, viền cầu vồng, chữ đậm nhiều màu; chưa bấm
//!   được thì viền và chữ xám. Nút phụ (Chọn…, Huỷ…): nền sáng hơn, viền xám. Nút đầu panel
//!   (Raster, Resize, Clip): viền xanh.
//! - Ô kết quả: chữ đơn cách trong khung tối; xong: chữ và viền xanh lá; lỗi: chữ đỏ, viền nâu;
//!   "n con / tờ": chữ hồng cánh sen đậm.
//!
//! Panel luôn dùng bộ màu này như DanCard, dù giao diện VectorCraft đang sáng hay tối: ô đánh dấu,
//! danh sách chọn và các thành phần dùng chung khác vẽ trong panel cũng theo nó ([`scope`]). Đây là
//! chỗ duy nhất đặt màu của panel.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use egui::epaint::{Mesh, Shadow};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Margin, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::theme::{self, Tokens};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// Bộ màu của DanCard (`:root` trong style.css).
/// `--bg`: nền panel.
pub(super) const BG: Color32 = rgb(0x20242b);
/// `--bg-raise`: thanh mục gập, nút.
const RAISE: Color32 = rgb(0x2b3039);
/// Thanh mục gập, nút khi rê chuột; ô đang chọn của lựa chọn ngắn.
const RAISE_HOVER: Color32 = rgb(0x39414d);
/// Nút đang bấm.
const RAISE_DOWN: Color32 = rgb(0x333b46);
/// `--bg-input`: ô nhập, ô kết quả.
const INPUT: Color32 = rgb(0x1d2229);
/// `--line`: đường kẻ.
const LINE: Color32 = rgb(0x171b21);
/// `--line-soft`: viền ô.
const LINE_SOFT: Color32 = rgb(0x3b4350);
/// `--ink`: chữ.
pub(super) const INK: Color32 = rgb(0xecf2f8);
/// `--ink-dim`: chữ phụ (nhãn, gợi ý).
pub(super) const INK_DIM: Color32 = rgb(0x9daab9);
/// Chữ của thứ chưa bấm được, gợi ý trong ô trống.
const INK_OFF: Color32 = rgb(0x5d6673);
/// `--accent`: viền ô đang gõ, nút đầu panel, ô đánh dấu.
const ACCENT: Color32 = rgb(0x4da1ff);
/// `--ok`: chữ ô kết quả khi xong (`.out.ok`).
pub(super) const OK: Color32 = rgb(0x73d3a6);
/// Viền ô kết quả khi xong.
const OK_LINE: Color32 = rgb(0x2f5c46);
/// `--warn`: chữ ô kết quả khi lỗi (`.out.warn`).
pub(super) const WARN: Color32 = rgb(0xff9292);
/// Viền ô kết quả khi lỗi.
const WARN_LINE: Color32 = rgb(0x5c4630);
/// `.optimal-capacity`: "n con / tờ".
const COUNT: Color32 = rgb(0xec008c);
/// Gạch dưới tab đang mở (`.tab.active`).
const TAB_LINE: Color32 = rgb(0xf8863d);
/// Viền nút chính (`--btn-pink`, `--btn-peach`, `--btn-yellow`, `--btn-blue`), quanh một vòng.
const RING: [Color32; 5] = [rgb(0x14d5a1), rgb(0x13abe8), rgb(0xffb550), rgb(0x287ed8), rgb(0x14d5a1)];
/// Chữ nút chính (`.btn-primary .btn-label`).
const LABEL: [Color32; 7] = [rgb(0xff2d95), rgb(0xff8a00), rgb(0xffee00), rgb(0x37ff5e), rgb(0x00e5ff), rgb(0x8a5cff), rgb(0xff2d95)];
/// Tên Lộc, Tân, Duẫn (`.rainbow-text`).
const RAINBOW: [Color32; 7] = [rgb(0xe5484d), rgb(0xf8863d), rgb(0xf5c518), rgb(0x4caf7d), rgb(0x2d8ceb), rgb(0x6a5acd), rgb(0xe5484d)];
/// "Code dạo" (`#codeDaoGradient`).
const CODE_DAO: [Color32; 5] = [rgb(0xffd428), rgb(0xff1a0a), rgb(0x13abe8), rgb(0x61ff2f), rgb(0xffd428)];
/// "1 cú" (`.tim-vang`).
const TIM_VANG: [Color32; 5] = [rgb(0x8c237b), rgb(0xd629d9), rgb(0xf5c518), rgb(0xd629d9), rgb(0x8c237b)];

/// Cao của ô nhập.
const INPUT_H: f32 = 30.0;
/// Cao của nút phụ.
const BUTTON_H: f32 = 30.0;
/// Cao của nút chính.
const PRIMARY_H: f32 = 38.0;
/// Cao một dòng của danh sách chọn, của lựa chọn ngắn.
const ROW_H: f32 = 32.0;
/// Cao một hàng tab.
const TAB_H: f32 = 32.0;
/// Khoảng cách giữa các ô của một hàng (`.field-row`).
const COLUMN_GAP: f32 = 10.0;

// ---------------------------------------------------------------- màu

fn mix(a: u8, b: u8, f: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * f).round().clamp(0.0, 255.0) as u8
}

/// Màu ở vị trí `t` (0…1, vòng lại) của dải `stops` chia đều.
fn gradient(stops: &[Color32], t: f32) -> Color32 {
    let n = stops.len();
    if n < 2 {
        return stops.first().copied().unwrap_or(INK);
    }
    let t = (if t.is_finite() { t.rem_euclid(1.0) } else { 0.0 }) * (n - 1) as f32;
    let i = (t.floor() as usize).min(n - 2);
    let f = t - i as f32;
    match (stops.get(i), stops.get(i + 1)) {
        (Some(a), Some(b)) => Color32::from_rgb(mix(a.r(), b.r(), f), mix(a.g(), b.g(), f), mix(a.b(), b.b(), f)),
        _ => INK,
    }
}

/// Chữ `text` tô dải màu `stops` từ trái sang phải, trên đoạn `span` của dải (bắt đầu ở `shift`).
fn colored_text(text: &str, font: FontId, stops: &[Color32], span: f32, shift: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let n = text.chars().count().max(1) as f32;
    let mut buf = [0u8; 4];
    for (k, ch) in text.chars().enumerate() {
        let color = gradient(stops, shift + span * (k as f32 + 0.5) / n);
        job.append(ch.encode_utf8(&mut buf), 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
    }
    job
}

// ---------------------------------------------------------------- phạm vi panel

/// Màu dùng chung của VectorCraft (`Tokens`) theo bộ màu DanCard, cho các thành phần dùng chung vẽ
/// trong panel (ô đánh dấu, danh sách thả xuống, gợi ý khi rê chuột).
fn tokens(base: Tokens) -> Tokens {
    Tokens {
        dark: true,
        panel: BG,
        panel_darker: INPUT,
        input: INPUT,
        input_border: LINE_SOFT,
        border: LINE,
        divider: LINE,
        text: INK,
        text_dim: INK_DIM,
        text_disabled: INK_OFF,
        icon: INK_DIM,
        hover: RAISE_HOVER,
        tool_active: RAISE_DOWN,
        accent: ACCENT,
        accent_strong: ACCENT,
        row_selected: RAISE_HOVER,
        button: RAISE,
        text_strong: Color32::WHITE,
        button_border: LINE_SOFT,
        ..base
    }
}

/// Đặt `Tokens` của DanCard khi tạo, trả lại bộ màu cũ khi xong (kể cả khi panel dừng giữa chừng).
struct TokensGuard {
    ctx: egui::Context,
    saved: Tokens,
}

impl TokensGuard {
    fn new(ctx: &egui::Context) -> Self {
        let saved = Tokens::get(ctx);
        ctx.data_mut(|d| d.insert_temp(Id::NULL, tokens(saved)));
        Self { ctx: ctx.clone(), saved }
    }
}

impl Drop for TokensGuard {
    fn drop(&mut self) {
        let saved = self.saved;
        self.ctx.data_mut(|d| d.insert_temp(Id::NULL, saved));
    }
}

/// Màu của các thành phần egui (nút, danh sách thả xuống) trong panel.
fn visuals(ui: &mut Ui) {
    let v = ui.visuals_mut();
    v.dark_mode = true;
    v.extreme_bg_color = INPUT;
    v.faint_bg_color = RAISE;
    v.panel_fill = BG;
    v.window_fill = INPUT;
    v.window_stroke = Stroke::new(1.0, ACCENT);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.text_cursor.stroke.color = ACCENT;
    v.hyperlink_color = ACCENT;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    for (w, fill) in [
        (&mut v.widgets.inactive, RAISE),
        (&mut v.widgets.hovered, RAISE_HOVER),
        (&mut v.widgets.active, RAISE_DOWN),
        (&mut v.widgets.open, RAISE_HOVER),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, LINE_SOFT);
        w.fg_stroke = Stroke::new(1.0, INK);
        w.corner_radius = CornerRadius::same(5);
    }
}

/// Vẽ nội dung panel trong bộ màu DanCard, trên nền của nó (phủ cả lề của panel, `margin`).
pub(super) fn scope<R>(ui: &mut Ui, margin: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let background = ui.painter().add(Shape::Noop);
    let guard = TokensGuard::new(ui.ctx());
    let out = ui.scope(|ui| {
        visuals(ui);
        add(ui)
    });
    drop(guard);
    ui.painter().set(background, Shape::rect_filled(out.response.rect.expand(margin), 0.0, BG));
    out.inner
}

// ---------------------------------------------------------------- đầu panel

/// Dấu 4 màu của DanCard (`logo-4-mau.png`), vẽ bằng code: các vòng tròn C, M, Y, K lồng nhau, tiếp
/// xúc ở phía dưới bên trái, ngăn nhau bằng vòng trắng.
pub(super) fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let r0 = size / 2.0;
    let dir = vec2(1.0, -0.85).normalized();
    let touch = rect.center() - dir * r0;
    let rings = [
        (1.0, rgb(0x00aeef)),
        (0.86, Color32::WHITE),
        (0.81, rgb(0xec008c)),
        (0.65, Color32::WHITE),
        (0.60, rgb(0xfff200)),
        (0.37, Color32::WHITE),
        (0.32, rgb(0x231f20)),
    ];
    for (f, color) in rings {
        let r = r0 * f;
        ui.painter().circle_filled(touch + dir * r, r, color);
    }
}

/// Tiêu đề panel (`.title`).
pub(super) fn brand(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(theme::semibold(14.0)).color(INK));
}

/// Nút đầu panel (`.header-raster`): viền xanh, chữ đậm nhỏ; `selected`: bảng nhập của nó đang mở.
pub(super) fn header_button(ui: &mut Ui, text: &str, enabled: bool, selected: bool) -> Response {
    let font = theme::semibold(11.5);
    let color = if enabled { INK } else { INK_OFF };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let size = vec2((galley.size().x + 16.0).max(52.0), 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let fill = if !enabled {
        RAISE
    } else if resp.is_pointer_button_down_on() {
        RAISE_DOWN
    } else if selected || resp.hovered() {
        RAISE_HOVER
    } else {
        RAISE
    };
    let border = if enabled { ACCENT } else { LINE_SOFT };
    ui.painter().rect(rect, CornerRadius::same(4), fill, Stroke::new(if selected { 2.0 } else { 1.0 }, border), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Tác giả và người thử (`.credits`): Lộc · Code dạo; Tân 1 cú, hay Duẫn ở tab CTL Offset.
pub(super) fn credits(ui: &mut Ui, duan: bool) {
    const GAP: f32 = 28.0;
    let role = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).size(9.5).color(INK_DIM).extra_letter_spacing(1.0));
    };
    let author = |ui: &mut Ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            role(ui, "AUTHOR");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(colored_text("Lộc", theme::semibold(20.0), &RAINBOW, 0.45, 0.0));
                ui.label(colored_text("Code dạo", theme::semibold(11.5), &CODE_DAO, 1.0, 0.0));
            });
        });
    };
    let tester = |ui: &mut Ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            role(ui, "TESTER");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if duan {
                    ui.label(colored_text("Duẫn", theme::semibold(20.0), &RAINBOW, 0.6, 0.4));
                } else {
                    ui.label(colored_text("Tân", theme::semibold(20.0), &RAINBOW, 0.45, 0.45));
                    ui.label(colored_text("1 cú", theme::semibold(12.5), &TIM_VANG, 1.0, 0.0));
                }
            });
        });
    };
    // Đường kẻ trên (`border-top` của `.credits`).
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, LINE_SOFT);
    // Hai người cạnh nhau khi đủ chỗ; panel hẹp thì người thử xuống dưới (`flex-wrap` của `.credits`).
    let width = |text: &str, size: f32| ui.painter().layout_no_wrap(text.to_string(), theme::semibold(size), INK).size().x;
    let author_w = width("Lộc", 20.0) + 6.0 + width("Code dạo", 11.5);
    let tester_w = if duan { width("Duẫn", 20.0) } else { width("Tân", 20.0) + 6.0 + width("1 cú", 12.5) };
    if author_w + GAP + tester_w <= ui.available_width() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            author(ui);
            tester(ui);
        });
    } else {
        author(ui);
        tester(ui);
    }
}

/// Bảng nhập mở dưới đầu panel (`.clip-pop`): khung tối viền xanh, tiêu đề.
pub(super) fn popover<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(INPUT)
        .stroke(Stroke::new(1.0, ACCENT))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .shadow(Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(title).font(theme::semibold(12.5)).color(INK));
            add(ui)
        })
        .inner
}

/// Chân panel (`.foot`): đường kẻ, chữ phiên bản nhỏ.
pub(super) fn footer(ui: &mut Ui, text: &str) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, LINE));
    ui.painter().text(pos2(rect.left(), rect.center().y + 2.0), Align2::LEFT_CENTER, text, FontId::proportional(10.0), INK_DIM.gamma_multiply(0.7));
}

// ---------------------------------------------------------------- thanh tab

/// Thanh tab (`.tabbar`): `tabs` là (khóa, nhãn), `current` là tab đang mở → khóa của tab vừa bấm.
/// Tab xếp từng hàng theo bề rộng chữ (ít nhất 62), hàng nào cũng chia đều phần còn lại.
pub(super) fn tab_bar(ui: &mut Ui, tabs: &[(&'static str, &'static str)], current: &str) -> Option<&'static str> {
    // Tab Auto Save chữ to hơn, như DanCard.
    let font = |key: &str| theme::semibold(if key == "autosave" { 14.0 } else { 12.0 });
    let avail = ui.available_width().max(62.0);
    let widths: Vec<f32> =
        tabs.iter().map(|(k, l)| (ui.painter().layout_no_wrap((*l).to_string(), font(k), INK).size().x + 16.0).clamp(62.0, avail)).collect();
    let mut rows: Vec<Vec<usize>> = vec![];
    let mut row: Vec<usize> = vec![];
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        if !row.is_empty() && used + w > avail {
            rows.push(std::mem::take(&mut row));
            used = 0.0;
        }
        row.push(i);
        used += w;
    }
    if !row.is_empty() {
        rows.push(row);
    }
    let (rect, _) = ui.allocate_exact_size(vec2(avail, TAB_H * rows.len() as f32), Sense::hover());
    let painter = ui.painter();
    let mut clicked = None;
    for (r, row) in rows.iter().enumerate() {
        let natural: f32 = row.iter().filter_map(|i| widths.get(*i)).sum();
        let extra = ((avail - natural) / row.len().max(1) as f32).max(0.0);
        let mut x = rect.left();
        for &i in row {
            let (Some((key, label)), Some(w)) = (tabs.get(i), widths.get(i)) else { continue };
            let tr = Rect::from_min_size(pos2(x, rect.top() + r as f32 * TAB_H), vec2(w + extra, TAB_H));
            x += w + extra;
            let resp = ui.interact(tr, ui.id().with(("dancard-tab", *key)), Sense::click());
            let active = *key == current;
            if active {
                vertical_gradient(painter, tr, Color32::from_rgba_unmultiplied(245, 95, 141, 38), Color32::from_rgba_unmultiplied(248, 174, 86, 13));
                painter.rect_filled(Rect::from_min_max(pos2(tr.left(), tr.bottom() - 2.0), tr.max), 0.0, TAB_LINE);
            } else if resp.hovered() {
                painter.rect_filled(tr, 0.0, Color32::from_rgba_unmultiplied(245, 95, 141, 26));
            }
            let color = if active || resp.hovered() { Color32::WHITE } else { INK_DIM };
            painter.text(tr.center() - vec2(0.0, 1.0), Align2::CENTER_CENTER, *label, font(key), color);
            resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, active, *label));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                clicked = Some(*key);
            }
        }
    }
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, LINE));
    clicked
}

/// `rect` tô màu từ `top` xuống `bottom`.
fn vertical_gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

// ---------------------------------------------------------------- mục gập

/// Tam giác ▶ (thu) hay ▼ (mở) quanh `c`.
fn arrow(painter: &Painter, c: Pos2, open: bool, color: Color32) {
    let s = 4.0;
    let points = if open {
        vec![c + vec2(-s, -s * 0.55), c + vec2(s, -s * 0.55), c + vec2(0.0, s * 0.75)]
    } else {
        vec![c + vec2(-s * 0.55, -s), c + vec2(-s * 0.55, s), c + vec2(s * 0.75, 0.0)]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Thanh mục gập (`.acc-head`) trải hết bề ngang: ▶ / ▼ rồi tên mục → có bấm (mở / thu) không.
pub(super) fn accordion_head(ui: &mut Ui, title: &str, open: bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), if resp.hovered() { RAISE_HOVER } else { RAISE });
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, LINE));
    arrow(painter, pos2(rect.left() + 15.0, rect.center().y), open, INK_DIM);
    painter.text(pos2(rect.left() + 28.0, rect.center().y), Align2::LEFT_CENTER, title, theme::semibold(13.0), INK);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, title));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Thân mục gập (`.acc-body`): lề quanh, các dòng cách đều.
pub(super) fn accordion_body<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::NONE
        .inner_margin(Margin { left: 10, right: 10, top: 10, bottom: 14 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            add(ui)
        })
        .inner
}

// ---------------------------------------------------------------- chữ

/// Nhãn của ô (`.label`): CHỮ HOA nhỏ màu xám, giãn chữ.
pub(super) fn label(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(label_text(text)).wrap());
}

fn label_text(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).size(10.5).color(INK_DIM).extra_letter_spacing(0.7)
}

/// Cao của nhãn `text` khi xuống dòng trong bề rộng `width`.
pub(super) fn label_height(ui: &Ui, text: &str, width: f32) -> f32 {
    egui::WidgetText::from(label_text(text)).into_galley(ui, Some(egui::TextWrapMode::Wrap), width, egui::FontSelection::Default).size().y
}

/// Nhãn chiếm đủ cao `height` (nhãn khác cùng hàng xuống dòng): các ô bên dưới thẳng hàng.
pub(super) fn label_block(ui: &mut Ui, text: &str, height: f32) {
    let r = ui.add(egui::Label::new(label_text(text)).wrap());
    let extra = height - r.rect.height();
    if extra > 0.5 {
        ui.add_space(extra);
    }
}

/// Gợi ý (`.hint`): chữ xám nhỏ, tự xuống dòng.
pub(super) fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(11.5).color(INK_DIM)).wrap());
}

/// Gợi ý nhấn mạnh (`<b>` / `<strong>` trong gợi ý của DanCard).
pub(super) fn hint_strong(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(theme::semibold(11.5)).color(INK_DIM)).wrap());
}

/// Tiêu đề một vùng: chữ trắng đậm.
pub(super) fn title(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(theme::semibold(13.0)).color(INK)).wrap());
}

/// Chữ thường của panel (giá trị, tên file đã chọn).
pub(super) fn text(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Label::new(RichText::new(text).size(12.5).color(INK)).wrap())
}

/// Chữ số đo (đơn cách).
pub(super) fn mono(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Label::new(RichText::new(text).font(theme::mono(12.0)).color(INK)).wrap())
}

// ---------------------------------------------------------------- ô nhập

/// Chọn hết chữ của ô `id`.
fn select_all(ui: &Ui, id: Id, text: &str) {
    let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
    st.cursor.set_char_range(Some(all));
    st.store(ui.ctx(), id);
}

/// Ô nhập (`.num` căn giữa, `.inp` căn trái) rộng `width`: nền tối, viền xám, đang gõ viền xanh; ô
/// trống hiện `placeholder`. Bấm vào ô chưa được chọn thì chọn hết chữ để gõ đè.
pub(super) fn input(ui: &mut Ui, id: Id, text: &mut String, width: f32, center: bool, placeholder: &str) -> Response {
    // Cả khung bấm được: bấm vào lề trong của khung cũng là bấm vào ô.
    let (rect, frame) = ui.allocate_exact_size(vec2(width.max(24.0), INPUT_H), Sense::click());
    let focused = ui.memory(|m| m.has_focus(id));
    let border = if focused { ACCENT } else { LINE_SOFT };
    ui.painter().rect(rect, CornerRadius::same(5), INPUT, Stroke::new(1.0, border), StrokeKind::Inside);
    let inner = rect.shrink2(vec2(9.0, 3.0));
    let edit = egui::TextEdit::singleline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .font(FontId::proportional(12.5))
        .text_color(INK)
        .desired_width(inner.width())
        .horizontal_align(if center { Align::Center } else { Align::Min })
        .vertical_align(Align::Center)
        .hint_text(RichText::new(placeholder).color(INK_OFF));
    // Ô chữ trong khung đã giữ chỗ ở trên: Ui con không dời con trỏ của hàng (`put` thì dời về
    // mép trong của khung, cái đặt sau dính sát vào ô).
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)));
    let resp = child.add(edit);
    if frame.on_hover_cursor(egui::CursorIcon::Text).clicked() {
        ui.memory_mut(|m| m.request_focus(id));
    }
    if resp.gained_focus() {
        select_all(ui, id, text);
    }
    resp
}

/// Ô khóa (`.inp-locked`): giá trị cố định, chữ cam đậm, không sửa được.
pub(super) fn locked(ui: &mut Ui, value: &str) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, INPUT_H), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(5), INPUT, Stroke::new(1.0, LINE_SOFT), StrokeKind::Inside);
    ui.painter().text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, value, theme::semibold(13.0), rgb(0xffb550));
}

/// Ô số đo (chỉ đọc): khung như ô nhập, chữ đơn cách; chữ dài hơn ô thì nhỏ lại rồi bị cắt.
pub(super) fn readout(ui: &mut Ui, value: &str) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, INPUT_H), Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(5), INPUT, Stroke::new(1.0, LINE_SOFT), StrokeKind::Inside);
    let mut galley = painter.layout_no_wrap(value.to_string(), theme::mono(12.5), INK);
    if galley.size().x > rect.width() - 16.0 {
        galley = painter.layout_no_wrap(value.to_string(), theme::mono(10.5), INK);
    }
    painter.with_clip_rect(rect.shrink(2.0)).galley(pos2(rect.left() + 9.0, rect.center().y - galley.size().y / 2.0), galley, INK);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, value));
    resp
}

/// Thanh tiến độ: khung như ô kết quả, phần đã xong (`done` trên 0…1) tô màu nhấn, chữ `text`
/// (vd. "12/500 · 012.pdf") ở giữa, dài quá thì bị cắt.
pub(super) fn progress(ui: &mut Ui, done: f32, text: &str) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, INPUT_H), Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(5), INPUT, Stroke::new(1.0, LINE_SOFT), StrokeKind::Inside);
    let done = if done.is_finite() { done.clamp(0.0, 1.0) } else { 0.0 };
    if done > 0.0 {
        let fill = Rect::from_min_size(rect.min, vec2(rect.width() * done, rect.height())).shrink(2.0);
        painter.rect_filled(fill, CornerRadius::same(4), ACCENT.gamma_multiply(0.55));
    }
    painter.with_clip_rect(rect.shrink(2.0)).text(rect.center(), Align2::CENTER_CENTER, text, theme::semibold(12.0), INK);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ProgressIndicator, true, text));
    resp
}

/// Bề rộng một cột của [`columns`] khi chia `n` cột.
pub(super) fn column_width(ui: &Ui, n: usize) -> f32 {
    let n = n.max(1);
    ((ui.available_width() - COLUMN_GAP * (n - 1) as f32) / n as f32).max(24.0)
}

/// Các ô đặt cạnh nhau chia đều bề ngang (`.field-row`): `add(ui, i)` vẽ ô thứ `i` trong cột của nó.
pub(super) fn columns(ui: &mut Ui, n: usize, mut add: impl FnMut(&mut Ui, usize)) {
    let n = n.max(1);
    let w = column_width(ui, n);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = COLUMN_GAP;
        for i in 0..n {
            ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing.y = 6.0;
                add(ui, i);
            });
        }
    });
}

// ---------------------------------------------------------------- lựa chọn

/// Danh sách chọn một (`.radio-list`): khung viền, mỗi lựa chọn một dòng có nút tròn → chỉ số dòng
/// vừa bấm.
pub(super) fn radio_list(ui: &mut Ui, id: &str, options: &[&str], selected: Option<usize>) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H * options.len() as f32), Sense::hover());
    let painter = ui.painter();
    let mut clicked = None;
    for (i, text) in options.iter().enumerate() {
        let row = Rect::from_min_size(pos2(rect.left(), rect.top() + ROW_H * i as f32), vec2(w, ROW_H));
        let resp = ui.interact(row, ui.id().with((id, i)), Sense::click());
        if resp.hovered() {
            painter.rect_filled(row.shrink(1.0), CornerRadius::same(4), RAISE);
        }
        if i > 0 {
            painter.hline(row.x_range(), row.top(), Stroke::new(1.0, LINE));
        }
        let on = selected == Some(i);
        radio_dot(painter, pos2(row.left() + 17.0, row.center().y), on);
        // Chữ dài so với dòng (panel hẹp): chữ nhỏ hơn, phần thừa bị cắt trong khung.
        let mut galley = painter.layout_no_wrap((*text).to_string(), FontId::proportional(12.5), INK);
        if galley.size().x > w - 40.0 {
            galley = painter.layout_no_wrap((*text).to_string(), FontId::proportional(11.0), INK);
        }
        painter.with_clip_rect(row.shrink(1.0)).galley(pos2(row.left() + 32.0, row.center().y - galley.size().y / 2.0), galley, INK);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, on, *text));
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
    }
    painter.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, LINE), StrokeKind::Inside);
    clicked
}

/// Ô đánh dấu (`.check`): ô vuông, bật thì nền xanh có dấu ✓; chữ tự xuống dòng khi dài hơn panel.
/// Bấm cả dòng để đổi. Trả về `Response` của dòng (`clicked()`: vừa đổi).
pub(super) fn check(ui: &mut Ui, text: &str, on: bool, enabled: bool) -> Response {
    const BOX: f32 = 14.0;
    const GAP: f32 = 9.0;
    let w = ui.available_width();
    let color = if enabled { INK } else { INK_OFF };
    let galley = ui.painter().layout(text.to_string(), FontId::proportional(12.5), color, (w - BOX - GAP).max(40.0));
    let h = (galley.size().y + 8.0).max(24.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if enabled { Sense::click() } else { Sense::hover() });
    let painter = ui.painter();
    let bx = Rect::from_center_size(pos2(rect.left() + BOX / 2.0, rect.center().y), Vec2::splat(BOX));
    let border = if on && enabled {
        ACCENT
    } else if enabled && resp.hovered() {
        INK
    } else {
        LINE_SOFT
    };
    painter.rect(bx, CornerRadius::same(3), if on && enabled { ACCENT } else { INPUT }, Stroke::new(1.0, border), StrokeKind::Inside);
    if on {
        let c = if enabled { Color32::WHITE } else { INK_OFF };
        let mid = bx.center_bottom() + vec2(-1.0, -3.5);
        painter.line_segment([bx.left_center() + vec2(3.0, 0.0), mid], Stroke::new(1.7, c));
        painter.line_segment([mid, bx.right_top() + vec2(-3.0, 3.5)], Stroke::new(1.7, c));
    }
    painter.galley(pos2(bx.right() + GAP, rect.center().y - galley.size().y / 2.0), galley, color);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

fn radio_dot(painter: &Painter, c: Pos2, on: bool) {
    painter.circle(c, 6.5, if on { ACCENT } else { INPUT }, Stroke::new(1.0, if on { ACCENT } else { LINE_SOFT }));
    if on {
        painter.circle_filled(c, 2.6, Color32::WHITE);
    }
}

/// Lựa chọn ngắn trên một hàng (90° / -90°, In nhanh / Cardoffset): các ô chia đều, ô đang chọn
/// có nút tròn xanh → chỉ số ô vừa bấm.
pub(super) fn segmented(ui: &mut Ui, id: &str, options: &[&str], selected: Option<usize>) -> Option<usize> {
    let w = ui.available_width();
    let n = options.len().max(1);
    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(5), INPUT, Stroke::new(1.0, LINE_SOFT), StrokeKind::Inside);
    let cell = w / n as f32;
    // Mọi ô cùng cỡ chữ, cùng có hay không có nút tròn: chữ dài so với ô (panel hẹp) thì chữ nhỏ
    // hơn, rồi bỏ nút tròn (ô đang chọn vẫn sáng hơn).
    let widest =
        |size: f32| options.iter().map(|t| painter.layout_no_wrap((*t).to_string(), FontId::proportional(size), INK).size().x).fold(0.0, f32::max);
    let (size, dot) = if widest(12.5) + 18.0 <= cell - 8.0 { (12.5, true) } else { (11.0, widest(11.0) + 18.0 <= cell - 6.0) };
    let mut clicked = None;
    for (i, text) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + cell * i as f32, rect.top()), vec2(cell, ROW_H));
        let resp = ui.interact(r, ui.id().with((id, i)), Sense::click());
        let on = selected == Some(i);
        if on {
            painter.rect_filled(r.shrink(2.0), CornerRadius::same(4), RAISE_HOVER);
        } else if resp.hovered() {
            painter.rect_filled(r.shrink(2.0), CornerRadius::same(4), RAISE);
        }
        if i > 0 {
            painter.vline(r.left(), r.y_range().shrink(6.0), Stroke::new(1.0, LINE_SOFT));
        }
        let color = if on { Color32::WHITE } else { INK_DIM };
        let galley = painter.layout_no_wrap((*text).to_string(), FontId::proportional(size), color);
        let block = galley.size().x + if dot { 18.0 } else { 0.0 };
        let start = r.center().x - block / 2.0;
        if dot {
            radio_dot(painter, pos2(start + 6.5, r.center().y), on);
        }
        let text_x = if dot { start + 18.0 } else { start };
        painter.with_clip_rect(r.shrink(2.0)).galley(pos2(text_x, r.center().y - galley.size().y / 2.0), galley, INK);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, on, *text));
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

// ---------------------------------------------------------------- nút

/// Điểm quanh viên thuốc `r` (bán kính nửa chiều cao), theo chiều kim đồng hồ từ đỉnh phải.
fn pill_outline(r: Rect) -> Vec<Pos2> {
    let rad = (r.height() / 2.0).min(r.width() / 2.0).max(0.0);
    let (left, right) = (pos2(r.left() + rad, r.center().y), pos2(r.right() - rad, r.center().y));
    let (cap, edge) = (24, 24);
    let mut pts = Vec::with_capacity(2 * (cap + edge + 2));
    for k in 0..=cap {
        let t = -FRAC_PI_2 + PI * k as f32 / cap as f32;
        pts.push(right + rad * vec2(t.cos(), t.sin()));
    }
    for k in 1..edge {
        pts.push(pos2(right.x + (left.x - right.x) * k as f32 / edge as f32, r.bottom()));
    }
    for k in 0..=cap {
        let t = FRAC_PI_2 + PI * k as f32 / cap as f32;
        pts.push(left + rad * vec2(t.cos(), t.sin()));
    }
    for k in 1..edge {
        pts.push(pos2(left.x + (right.x - left.x) * k as f32 / edge as f32, r.top()));
    }
    pts
}

/// Viền cầu vồng dày `width` quanh viên thuốc `rect` (`conic-gradient` của `.btn-primary`), quay
/// đi `turn` vòng.
fn ring(painter: &Painter, rect: Rect, width: f32, turn: f32) {
    let outer = pill_outline(rect);
    let inner = pill_outline(rect.shrink(width));
    let c = rect.center();
    let mut mesh = Mesh::default();
    for (o, i) in outer.iter().zip(&inner) {
        let angle = (o.y - c.y).atan2(o.x - c.x) / TAU + 0.25 + turn;
        let color = gradient(&RING, angle);
        mesh.colored_vertex(*o, color);
        mesh.colored_vertex(*i, color);
    }
    let n = outer.len().min(inner.len()) as u32;
    for k in 0..n {
        let (a, b) = (2 * k, 2 * k + 1);
        let (c2, d) = (2 * ((k + 1) % n), 2 * ((k + 1) % n) + 1);
        mesh.add_triangle(a, b, c2);
        mesh.add_triangle(b, d, c2);
    }
    painter.add(Shape::mesh(mesh));
}

/// Nút chính (`.btn-primary`): viên thuốc trải hết bề ngang, viền cầu vồng, chữ đậm nhiều màu; rê
/// chuột vào thì viền quay. `enabled` sai (chưa chọn gì): viền và chữ xám, không bấm được.
pub(super) fn primary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, PRIMARY_H), if enabled { Sense::click() } else { Sense::hover() });
    let rect = if enabled && resp.is_pointer_button_down_on() { rect.translate(vec2(0.0, 1.0)) } else { rect };
    let painter = ui.painter();
    let radius = rect.height() / 2.0;
    let hovered = enabled && resp.hovered();
    let cr = CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8);
    if enabled {
        let glow = if hovered { Color32::from_rgba_unmultiplied(255, 112, 28, 120) } else { Color32::from_rgba_unmultiplied(97, 255, 47, 64) };
        painter.add(Shadow { offset: [0, 0], blur: if hovered { 11 } else { 7 }, spread: 0, color: glow }.as_shape(rect, cr));
        let turn = if hovered { (ui.input(|i| i.time) / 1.5).fract() as f32 } else { 0.0 };
        ring(painter, rect, 3.0, turn);
        if hovered {
            ui.ctx().request_repaint();
        }
    } else {
        painter.rect_stroke(rect, cr, Stroke::new(1.5, LINE_SOFT), StrokeKind::Inside);
    }
    let fill = if hovered { RAISE_HOVER } else { RAISE };
    painter.rect_filled(rect.shrink(3.0), CornerRadius::same((radius - 3.0).round().clamp(0.0, 255.0) as u8), fill);
    let label = |size: f32| {
        let font = theme::semibold(size);
        let job = if enabled { colored_text(text, font, &LABEL, 0.5, 0.0) } else { LayoutJob::simple_singleline(text.to_string(), font, INK_OFF) };
        painter.layout_job(job)
    };
    let mut galley = label(13.0);
    // Chữ dài hơn nút (panel hẹp): chữ nhỏ hơn.
    if galley.size().x > rect.width() - 2.0 * radius {
        galley = label(11.0);
    }
    painter.galley(rect.center() - galley.size() / 2.0, galley, INK);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Bề rộng của nút phụ không trải ngang có chữ `text`.
pub(super) fn button_width(ui: &Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), INK).size().x + 24.0
}

/// Nút phụ (`.btn`): nền sáng hơn nền panel, viền xám; `wide`: trải hết bề ngang.
pub(super) fn button(ui: &mut Ui, text: &str, enabled: bool, wide: bool) -> Response {
    let color = if enabled { INK } else { INK_OFF };
    let mut galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), color);
    let width = if wide { ui.available_width() } else { button_width(ui, text) };
    // Chữ dài hơn nút trải ngang (panel hẹp): chữ nhỏ hơn.
    if galley.size().x > width - 12.0 {
        galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(11.0), color);
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(width, BUTTON_H), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if !enabled {
        RAISE
    } else if resp.is_pointer_button_down_on() {
        RAISE_DOWN
    } else if resp.hovered() {
        RAISE_HOVER
    } else {
        RAISE
    };
    ui.painter().rect(rect, CornerRadius::same(5), fill, Stroke::new(1.0, LINE_SOFT), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

// ---------------------------------------------------------------- ô kết quả

/// Nội dung ô kết quả (`.out`).
pub(super) enum Out<'a> {
    /// Chưa chạy: lời nhắc.
    Hint(&'a str),
    /// Chạy xong (`.out.ok`).
    Done(&'a str),
    /// Lỗi (`.out.warn`).
    Error(&'a str),
    /// "n con / tờ" rồi ghi chú (`showCountResult`).
    Count(&'a str, &'a str),
}

/// Ô kết quả trải hết bề ngang, cao theo chữ: chữ đơn cách trong khung tối; xong màu xanh lá, lỗi màu
/// đỏ, "n con / tờ" hồng cánh sen đậm. Chữ chọn và sao chép được (báo lỗi, đường dẫn file).
pub(super) fn output(ui: &mut Ui, out: Out<'_>) {
    let (color, border) = match out {
        Out::Hint(_) => (INK, LINE),
        Out::Done(_) | Out::Count(..) => (OK, OK_LINE),
        Out::Error(_) => (WARN, WARN_LINE),
    };
    let line = |ui: &mut Ui, text: RichText| {
        ui.add(egui::Label::new(text).wrap().selectable(true));
    };
    egui::Frame::NONE
        .fill(INPUT)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin { left: 12, right: 12, top: 11, bottom: 11 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(18.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            match out {
                Out::Count(n, rest) => {
                    line(ui, RichText::new(format!("{n} con / tờ")).font(theme::semibold(13.5)).color(COUNT));
                    if !rest.is_empty() {
                        line(ui, RichText::new(rest).font(theme::mono(12.0)).color(color));
                    }
                }
                Out::Hint(t) | Out::Done(t) | Out::Error(t) => line(ui, RichText::new(t).font(theme::mono(12.0)).color(color)),
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dai_mau_va_chu_nhieu_mau() {
        assert_eq!(gradient(&RING, 0.0), RING[0]);
        assert_eq!(gradient(&RING, 1.0), RING[0], "vòng lại");
        assert_eq!(gradient(&[], 0.3), INK);
        assert_eq!(gradient(&RING, f32::NAN), RING[0]);
        let mid = gradient(&[Color32::BLACK, Color32::WHITE], 0.5);
        assert!((127..=128).contains(&mid.r()));
        let job = colored_text("Lộc", FontId::proportional(12.0), &RAINBOW, 0.5, 0.0);
        assert_eq!(job.text, "Lộc");
        assert_eq!(job.sections.len(), 3, "mỗi chữ một màu");
        let pill = pill_outline(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 40.0)));
        assert!(pill.iter().all(|p| p.x >= -1e-3 && p.x <= 200.001 && p.y >= -1e-3 && p.y <= 40.001));
    }

    /// Màu dùng chung đổi theo DanCard trong panel và trở lại như cũ sau đó.
    #[test]
    fn bo_mau_chi_trong_panel() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx, crate::theme::Brightness::Light);
        let before = Tokens::get(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            scope(ui, 10.0, |ui| {
                assert_eq!(Tokens::get(ui.ctx()).panel, BG);
                assert_eq!(Tokens::get(ui.ctx()).text, INK);
            });
        });
        out.textures_delta.clear();
        assert_eq!(Tokens::get(&ctx).panel, before.panel);
        assert_eq!(Tokens::get(&ctx).text, before.text);
    }
}
