//! Panel "Công cụ bình" (DanCard, IN Vĩnh Phát), bố cục và màu như panel DanCard trên Illustrator
//! (`dancard_style.rs`).
//!
//! - Đầu panel: dấu 4 màu, tên panel, nút Raster (CMYK, 500 ppi, nền trong suốt, đúng khung nhìn
//!   thấy), Resize và Clip theo kích thước nhập (giữ tâm, bảng nhập mở ngay dưới), tác giả và người
//!   thử như DanCard.
//! - Các tab theo thứ tự của DanCard: Dàn file, Dàn tối ưu, Dấu cắt, Dàn theo mẫu, Catalogue, CTL
//!   Offset, Đổi tên, Variable, Auto Save; thêm tab Đo khung của bản VectorCraft ở cuối. Mỗi tab có
//!   ô kết quả như DanCard (xanh: xong, đỏ: lỗi, "n con / tờ").
//! - Tab Đo khung: khung transform của thứ đang chọn — góc xoay, rộng × cao, tâm — đọc bằng lệnh
//!   `dancard.transform`, khớp số với bảng Transform. Chọn clip group, group lồng nhiều tầng hay
//!   object con đều đọc thẳng. Riêng hình chữ nhật đặt nghiêng trong file .ai/PDF (không lưu góc)
//!   thì panel cho khung dò được, kèm số của bảng Transform để đối chiếu. Chỉ tính lại khi tài
//!   liệu, vùng chọn hoặc đơn vị đổi.
//!
//! Giá trị đã gõ vào các ô, tab đang mở và mục gập đang mở được giữ lại giữa các lần mở app
//! (`UiState::dancard`).

use egui::Ui;
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;

pub const ID: &str = "dancard";

// Các tab chuyển từ DanCard, mỗi tab một file.
#[path = "dancard_be.rs"]
mod be;
#[path = "dancard_ctl.rs"]
mod ctl;
#[path = "dancard_kts.rs"]
mod kts;
#[path = "dancard_mau.rs"]
mod mau;
#[path = "dancard_style.rs"]
mod style;
#[path = "dancard_tutro.rs"]
mod tutro;
#[path = "dancard_variable.rs"]
mod variable;

/// Số bản của lần build này (workflow của fork đặt, app báo lại lúc mở; bản build tay không có).
static BUILD: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
/// Bản mới hơn đã tải về, dùng từ lần mở app sau (0: chưa có).
static UPDATE_READY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Số bản của lần build này, hiện ở chân panel.
pub fn set_build(build: u64) {
    // Đặt một lần lúc mở app; lần đặt sau (không có) cũng là cùng số.
    let _ = BUILD.set(build);
}

/// Bản `build` đã tải về (`dancard_update` của app): panel nhắc mở lại app để dùng.
pub fn set_update_ready(build: u64) {
    UPDATE_READY.store(build, std::sync::atomic::Ordering::Relaxed);
}

/// Chân panel: phiên bản, số bản (nếu có).
fn foot_text() -> String {
    let build = BUILD.get().map(|b| format!(" · bản {b}")).unwrap_or_default();
    format!("DanCard trên VectorCraft {}{build}", env!("CARGO_PKG_VERSION"))
}

/// Bề rộng panel khi mở từ cột biểu tượng hay nổi (dock::panel_width): rộng hơn panel thường cho
/// đầu panel và các ô cạnh nhau như DanCard.
pub const WIDTH: f32 = 320.0;
/// Lề trong của panel (dock::panel_body), nền DanCard phủ cả lề.
const MARGIN: f32 = 10.0;

/// Chữ của fork DanCard: tiếng Việt (có chữ ă, đ, ơ, ư hay dấu thanh) hoặc bắt đầu bằng
/// "DanCard". Không nằm trong bộ dịch của VectorCraft, nên các bài kiểm tra "mọi chuỗi đều có bản
/// dịch" bỏ qua chúng.
pub fn is_fork_text(s: &str) -> bool {
    s.starts_with("DanCard") || s.chars().any(|c| matches!(c, 'ă' | 'Ă' | 'đ' | 'Đ' | 'ơ' | 'Ơ' | 'ư' | 'Ư' | '\u{1EA0}'..='\u{1EF9}'))
}

/// Kết quả lần đọc trước và khóa của nó (tài liệu, lần sửa, vùng chọn, đơn vị, Use Preview Bounds).
#[derive(Clone, Default)]
struct Cache {
    key: (usize, u64, Vec<u64>, String, bool),
    value: Value,
}

/// Số kiểu Việt Nam, như bảng Transform: tối đa 4 chữ số lẻ, bỏ số 0 thừa, dấu phẩy thập phân.
fn num(v: &Value) -> String {
    let Some(x) = v.as_f64() else { return "–".into() };
    let s = format!("{x:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.replace('.', ",") }
}

fn source_label(s: &Value) -> &'static str {
    match s.as_str() {
        Some("transform") => "khung xoay",
        Some("detected") => "tự dò góc từ hình dạng",
        _ => "khung thẳng",
    }
}

/// Số đo của khung `f` thành từng cặp ô (nhãn trên, số dưới) như các ô của panel: góc và cách đọc
/// khung, rộng × cao, tâm.
fn frame_rows(ui: &mut Ui, f: &Value, unit: &str) {
    let angle = format!("{}°", num(&f["angle"]));
    let (width, height) = (format!("{} {unit}", num(&f["width"])), format!("{} {unit}", num(&f["height"])));
    // Tâm theo thước: bằng X/Y của bảng Transform khi chọn điểm tâm.
    let (cx, cy) = (num(&f["rulerCenter"][0]), num(&f["rulerCenter"][1]));
    let rows: [[(&str, &str); 2]; 3] =
        [[("Góc", angle.as_str()), ("Khung", source_label(&f["source"]))], [("Rộng", &width), ("Cao", &height)], [("Tâm X", &cx), ("Tâm Y", &cy)]];
    for row in rows {
        style::columns(ui, 2, |ui, i| {
            if let Some((label, value)) = row.get(i) {
                style::label(ui, label);
                style::readout(ui, value);
            }
        });
    }
    if let Some(tb) = f.get("transformBox") {
        style::hint(ui, &format!("Bảng Transform (khung thẳng): {} × {} {unit}", num(&tb["width"]), num(&tb["height"])));
    }
}

/// Các tab của panel: (khóa, nhãn), theo thứ tự của DanCard; Đo khung (của bản VectorCraft) ở cuối.
const TABS: &[(&str, &str)] = &[
    ("danfile", "Dàn file"),
    ("toiuu", "Dàn tối ưu"),
    ("cutmarks", "Dấu cắt"),
    ("danmau", "Dàn theo mẫu"),
    ("catalogue", "Catalogue"),
    ("offset", "CTL Offset"),
    ("rename", "Đổi tên"),
    ("variable", "Variable"),
    ("autosave", "Auto Save"),
    ("measure", "Đo khung"),
];

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::scope(ui, MARGIN, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        let tab = current_tab(app);
        header(app, ui, &tab);
        if let Some(key) = style::tab_bar(ui, TABS, &tab) {
            save(app, "tab", key);
        }
        let tab = current_tab(app);
        // Phần thân cuộn được khi cao hơn cửa sổ (`.body` của DanCard); mỗi tab nhớ chỗ cuộn của nó
        // như mỗi tab của DanCard.
        let max_h = (ui.ctx().content_rect().bottom() - ui.cursor().top() - 48.0).max(160.0);
        egui::ScrollArea::vertical().id_salt(("dancard-body", tab.as_str())).max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.add_space(4.0);
            match tab.as_str() {
                "rename" => rename_tab(app, ui),
                "cutmarks" => cut_marks_tab(app, ui),
                "autosave" => auto_save_tab(app, ui),
                "catalogue" => catalogue_tab(app, ui),
                "toiuu" => toi_uu_tab(app, ui),
                "danmau" => mau::section(app, ui),
                "offset" => ctl::section(app, ui),
                "variable" => variable::section(app, ui),
                "measure" => transform_tab(app, ui),
                _ => layout_tab(app, ui),
            }
            ui.add_space(4.0);
        });
        let ready = UPDATE_READY.load(std::sync::atomic::Ordering::Relaxed);
        if ready > BUILD.get().copied().unwrap_or(0) {
            style::hint_strong(ui, &format!("Đã tải bản {ready}: đóng rồi mở lại app là dùng bản mới."));
        }
        style::footer(ui, &foot_text());
    });
}

/// Tab đang mở (một khóa của [`TABS`]; mặc định Dàn file như DanCard).
fn current_tab(app: &VectorcraftApp) -> String {
    let tab = saved(app, "tab", "danfile");
    if TABS.iter().any(|(k, _)| *k == tab) { tab } else { "danfile".into() }
}

// ---------------------------------------------------------------- giá trị đã gõ

/// Giá trị đã lưu của ô `key` (chuỗi), hoặc `default`.
fn saved(app: &VectorcraftApp, key: &str, default: &str) -> String {
    app.ui.dancard.get(key).and_then(Value::as_str).map_or_else(|| default.to_string(), str::to_string)
}

fn save(app: &mut VectorcraftApp, key: &str, v: &str) {
    app.ui.dancard.insert(key.into(), Value::String(v.into()));
}

fn saved_bool(app: &VectorcraftApp, key: &str, default: bool) -> bool {
    app.ui.dancard.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// Ô nhập gắn với giá trị đã lưu `key`, rộng hết chỗ (`center`: ô số căn giữa) → giá trị, đã bỏ
/// khoảng trắng hai đầu. Gõ đến đâu lưu đến đó.
fn edit_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str, center: bool, placeholder: &str) -> String {
    let w = ui.available_width();
    edit_box_width(app, ui, key, default, center, placeholder, w)
}

/// [`edit_box`] rộng `width`.
fn edit_box_width(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str, center: bool, placeholder: &str, width: f32) -> String {
    let mut cur = saved(app, key, default);
    if style::input(ui, egui::Id::new(("dancard-field", key)), &mut cur, width, center, placeholder).changed() {
        save(app, key, &cur);
    }
    cur.trim().to_string()
}

/// Ô số (căn giữa) gắn với `key`.
fn num_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str) -> String {
    edit_box(app, ui, key, default, true, "")
}

/// Ô chữ (căn trái) gắn với `key`; trống thì hiện `placeholder`.
fn text_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str, placeholder: &str) -> String {
    edit_box(app, ui, key, default, false, placeholder)
}

/// Ô chữ gắn với `key` có chữ đơn vị `unit` bên phải (`.size-row` của DanCard: "5 hộp").
fn text_box_unit(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str, placeholder: &str, unit: &str) -> String {
    let mut out = String::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let unit_w = ui.painter().layout_no_wrap(unit.to_string(), egui::FontId::proportional(13.0), style::INK_DIM).size().x;
        let w = (ui.available_width() - unit_w - 8.0).max(60.0);
        out = edit_box_width(app, ui, key, default, false, placeholder, w);
        ui.label(egui::RichText::new(unit).size(13.0).color(style::INK_DIM));
    });
    out
}

/// Nhãn rồi hai ô rộng × cao trên một hàng (`.size-row` của DanCard); `w`, `h`: (khóa, mặc định)
/// → (rộng, cao).
fn size_pair(app: &mut VectorcraftApp, ui: &mut Ui, label: &str, w: (&str, &str), h: (&str, &str)) -> (String, String) {
    const GAP: f32 = 8.0;
    style::label(ui, label);
    let times_w = ui.painter().layout_no_wrap("×".into(), egui::FontId::proportional(13.0), style::INK_DIM).size().x;
    let field = ((ui.available_width() - times_w - 2.0 * GAP) / 2.0).max(40.0);
    let mut out = (String::new(), String::new());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        out.0 = edit_box_width(app, ui, w.0, w.1, true, "rộng", field);
        ui.label(egui::RichText::new("×").size(13.0).color(style::INK_DIM));
        out.1 = edit_box_width(app, ui, h.0, h.1, true, "cao", field);
    });
    out
}

/// Nhãn rồi ô số (`.field` của DanCard).
fn num_field(app: &mut VectorcraftApp, ui: &mut Ui, label: &str, key: &str, default: &str) -> String {
    style::label(ui, label);
    num_box(app, ui, key, default)
}

/// Các ô số cạnh nhau chia đều bề ngang (`.field-row`): (nhãn, khóa, mặc định) → giá trị từng ô.
fn num_fields<const N: usize>(app: &mut VectorcraftApp, ui: &mut Ui, fields: [(&str, &str, &str); N]) -> [String; N] {
    let mut out: [String; N] = std::array::from_fn(|_| String::new());
    // Nhãn dài xuống dòng thì nhãn cùng hàng chừa đủ chỗ: các ô thẳng hàng.
    let w = style::column_width(ui, N);
    let label_h = fields.iter().map(|(label, _, _)| style::label_height(ui, label, w)).fold(0.0, f32::max);
    style::columns(ui, N, |ui, i| {
        if let (Some((label, key, default)), Some(v)) = (fields.get(i), out.get_mut(i)) {
            style::label_block(ui, label, label_h);
            *v = num_box(app, ui, key, default);
        }
    });
    out
}

/// Ô đánh dấu gắn với giá trị đã lưu `key`.
fn check_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, label: &str, default: bool) -> bool {
    let on = saved_bool(app, key, default);
    if style::check(ui, label, on, true).clicked() {
        app.ui.dancard.insert(key.into(), Value::Bool(!on));
        return !on;
    }
    on
}

/// Chọn một trong `options` (giá trị, nhãn) gắn với `key`: danh sách có khung (`list`, như
/// `.radio-list` của DanCard) hay các ô trên một hàng → giá trị đang chọn.
fn choice(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, options: &[(&str, &str)], default: &str, list: bool) -> String {
    let cur = saved(app, key, default);
    let labels: Vec<&str> = options.iter().map(|o| o.1).collect();
    let at = options.iter().position(|o| o.0 == cur);
    let clicked = if list { style::radio_list(ui, key, &labels, at) } else { style::segmented(ui, key, &labels, at) };
    if let Some((v, _)) = clicked.and_then(|i| options.get(i)) {
        save(app, key, v);
        return (*v).to_string();
    }
    cur
}

/// Mục gập (`.acc`) có trạng thái mở / thu giữ lại theo `key`.
fn accordion(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, title: &str, default_open: bool, add: impl FnOnce(&mut VectorcraftApp, &mut Ui)) {
    let state = format!("acc.{key}");
    let open = saved_bool(app, &state, default_open);
    if style::accordion_head(ui, title, open) {
        app.ui.dancard.insert(state, Value::Bool(!open));
    }
    if open {
        style::accordion_body(ui, |ui| add(app, ui));
    }
}

/// Đã chọn object chưa (các lệnh cần vùng chọn).
fn has_selection(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| !st.selection.objects.is_empty())
}

/// Tên file của đường dẫn `path`.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Dòng file đã chọn (`key`): tên file trong khung (rê chuột: cả đường dẫn), nút Chọn… và Bỏ.
fn file_row(app: &mut VectorcraftApp, ui: &mut Ui, key: &'static str, empty: &str, filter: (&'static str, &'static [&'static str])) {
    let path = saved(app, key, "");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let pick = style::button(ui, "Chọn…", true, false);
        let clear = !path.is_empty() && style::button(ui, "Bỏ", true, false).clicked();
        let shown = if path.is_empty() { empty.to_string() } else { file_name(&path).to_string() };
        style::text(ui, &shown).on_hover_text(&path);
        if pick.clicked() {
            crate::picks::button(app, move |app| {
                let pick = crate::FilePick { filters: vec![filter], ..Default::default() };
                if let Some(p) = crate::picks::open(app, &pick) {
                    save(app, key, &p);
                }
            });
        }
        if clear {
            save(app, key, "");
        }
    });
}

/// Ô thư mục (`key`, gõ được) và nút Chọn… bên phải.
fn folder_row(app: &mut VectorcraftApp, ui: &mut Ui, key: &'static str, placeholder: &str) -> String {
    let mut out = String::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let w = (ui.available_width() - style::button_width(ui, "Chọn…") - 6.0).max(60.0);
        out = edit_box_width(app, ui, key, "", false, placeholder, w);
        if style::button(ui, "Chọn…", true, false).clicked() {
            crate::picks::button(app, move |app| {
                if let Some(f) = crate::picks::folder(app) {
                    save(app, key, &f);
                }
            });
        }
    });
    out
}

// ---------------------------------------------------------------- thông báo

/// Thông báo của lần chạy gần nhất: (chữ, có phải lỗi).
#[derive(Clone, Default)]
struct Note(String, bool);

fn note_id(area: &str) -> String {
    format!("dancard-note-{area}")
}

/// Chạy lệnh `id`, ghi thông báo (bỏ tiền tố OK:/ERR: của DanCard) cho vùng `area` và thanh trạng thái.
fn run_and_note(app: &mut VectorcraftApp, ui: &Ui, area: &str, id: &str, params: Value) -> Option<Value> {
    let (text, err, out) = match app.run(id, params) {
        Ok(v) => (v["message"].as_str().unwrap_or("Xong.").to_string(), false, Some(v)),
        Err(e) => (e, true, None),
    };
    let text = text.trim_start_matches("OK:").trim_start_matches("ERR:").trim().to_string();
    app.status(text.lines().next().unwrap_or_default().to_string());
    set_pstate(ui.ctx(), &note_id(area), Note(text, err));
    out
}

/// `[[COUNT:n]]` ở đầu thông báo (`^\[\[COUNT:(\d+)\]\]\s*` của DanCard) → (n, phần còn lại).
fn count_prefix(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[[COUNT:")?;
    let end = rest.find("]]")?;
    let n = rest.get(..end)?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((n, rest.get(end + 2..)?.trim_start()))
}

/// Ô kết quả như `showCountResult` của DanCard: "[[COUNT:n]] …" thành dòng đậm "n con / tờ" rồi
/// ghi chú; lỗi và thông báo khác như [`show_note`].
fn show_count_note(ui: &mut Ui, area: &str, hint: &str) {
    let Note(text, err) = pstate::<Note>(ui.ctx(), &note_id(area));
    if !err && let Some((n, rest)) = count_prefix(&text) {
        style::output(ui, style::Out::Count(n, rest));
        return;
    }
    show_note(ui, area, hint);
}

/// Ô kết quả của vùng `area` (`.out`): chưa chạy thì lời nhắc `hint` (trống: không có ô).
fn show_note(ui: &mut Ui, area: &str, hint: &str) {
    let Note(text, err) = pstate::<Note>(ui.ctx(), &note_id(area));
    if text.is_empty() {
        if !hint.is_empty() {
            style::output(ui, style::Out::Hint(hint));
        }
        return;
    }
    style::output(ui, if err { style::Out::Error(&text) } else { style::Out::Done(&text) });
}

// ---------------------------------------------------------------- đầu panel: Raster, Resize, Clip

/// Bảng nhập đang mở dưới đầu panel.
#[derive(Clone, Copy, Default, PartialEq)]
enum Popover {
    #[default]
    None,
    Resize,
    Clip,
}

/// Hàng nhập khổ của bảng Clip / Resize (`.clip-pop-row` của DanCard): rộng × cao rồi đơn vị, các
/// giá trị lưu theo `prefix` → (rộng, cao, đơn vị).
fn size_row(app: &mut VectorcraftApp, ui: &mut Ui, prefix: &str) -> (String, String, String) {
    const UNIT_W: f32 = 96.0;
    const GAP: f32 = 6.0;
    const FIELD_MIN: f32 = 40.0;
    let units = [("cm", "cm"), ("mm", "mm"), ("in", "in")];
    let times_w = ui.painter().layout_no_wrap("×".into(), egui::FontId::proportional(13.0), style::INK_DIM).size().x;
    // Panel hẹp (cột dock kéo nhỏ): đơn vị xuống hàng dưới, các ô không tràn ra ngoài.
    let one_row = ui.available_width() >= 2.0 * FIELD_MIN + times_w + UNIT_W + 3.0 * GAP;
    let unit_room = if one_row { UNIT_W + GAP } else { 0.0 };
    let field = ((ui.available_width() - unit_room - times_w - 2.0 * GAP) / 2.0).max(24.0);
    let mut out = (String::new(), String::new(), String::new());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        out.0 = edit_box_width(app, ui, &format!("{prefix}W"), "5", true, "", field);
        ui.label(egui::RichText::new("×").size(13.0).color(style::INK_DIM));
        out.1 = edit_box_width(app, ui, &format!("{prefix}H"), "5", true, "", field);
        if one_row {
            out.2 = ui.allocate_ui(egui::vec2(UNIT_W, 32.0), |ui| choice(app, ui, &format!("{prefix}Unit"), &units, "cm", false)).inner;
        }
    });
    if !one_row {
        out.2 = choice(app, ui, &format!("{prefix}Unit"), &units, "cm", false);
    }
    out
}

fn header(app: &mut VectorcraftApp, ui: &mut Ui, tab: &str) {
    let selected = has_selection(app);
    let mut open = pstate::<Popover>(ui.ctx(), "dancard-popover");
    let buttons = |app: &mut VectorcraftApp, ui: &mut Ui, open: &mut Popover| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // Phải sang trái: Clip, Resize, Raster.
        let clip = style::header_button(ui, "Clip", selected, *open == Popover::Clip).on_hover_text("Clip canh giữa theo kích thước nhập");
        if clip.clicked() {
            *open = if *open == Popover::Clip { Popover::None } else { Popover::Clip };
        }
        let resize = style::header_button(ui, "Resize", selected, *open == Popover::Resize).on_hover_text("Resize theo kích thước nhập, giữ tâm");
        if resize.clicked() {
            *open = if *open == Popover::Resize { Popover::None } else { Popover::Resize };
        }
        let raster =
            style::header_button(ui, "Raster", selected, false).on_hover_text("Raster CMYK · 500 ppi · nền trong suốt, đúng khung nhìn thấy");
        if raster.clicked() {
            run_and_note(app, ui, "header", "dancard.rasterize", json!({}));
            *open = Popover::None;
        }
    };
    // Tên và các nút trên một hàng khi đủ chỗ, không thì các nút xuống hàng dưới.
    let title_w = ui.painter().layout_no_wrap("Công cụ bình".into(), crate::theme::semibold(14.0), style::INK).size().x;
    let one_row = ui.available_width() >= 18.0 + 8.0 + title_w + 8.0 + 3.0 * 54.0 + 12.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        style::logo(ui, 18.0);
        style::brand(ui, "Công cụ bình");
        if one_row {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| buttons(app, ui, &mut open));
        }
    });
    if !one_row {
        // Một hàng cao đúng cỡ nút (`with_layout` ngang trong cột dọc chiếm hết chiều cao còn lại
        // và đặt nút ở giữa nó).
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 26.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            buttons(app, ui, &mut open)
        });
    }
    if open != Popover::None {
        let (title, prefix, command, ok) = match open {
            Popover::Resize => ("Resize kích thước (giữ tâm)", "resize", "dancard.resize", "Resize"),
            _ => ("Clip kích thước (canh giữa)", "clip", "dancard.clipToSize", "OK"),
        };
        style::popover(ui, title, |ui| {
            let (w, h, unit) = size_row(app, ui, prefix);
            if open == Popover::Clip {
                check_box(app, ui, "clipFollowAngle", "Khung clip theo khung transform (card xoay, đặt nghiêng)", true);
            }
            style::columns(ui, 2, |ui, i| {
                if i == 0 {
                    if style::button(ui, "Huỷ", true, true).clicked() {
                        open = Popover::None;
                    }
                } else if style::primary(ui, ok, selected).clicked() {
                    let mut params = json!({"width": w, "height": h, "unit": unit});
                    if open == Popover::Clip {
                        params["followAngle"] = json!(saved_bool(app, "clipFollowAngle", true));
                    }
                    if run_and_note(app, ui, "header", command, params).is_some() {
                        open = Popover::None;
                    }
                }
            });
        });
    }
    set_pstate(ui.ctx(), "dancard-popover", open);
    // Tác giả, người thử (Duẫn ở tab CTL Offset, như DanCard).
    style::credits(ui, tab == "offset");
    show_note(ui, "header", "");
}

// ---------------------------------------------------------------- tab Dàn file

/// Các loại card của tab Dàn file: (khóa, nhãn, rộng mm, cao mm, voucher).
const CARD_LAYOUTS: &[(&str, &str, &str, &str, bool)] = &[
    ("9.2x5.6", "9.2 × 5.6  (20 chỗ)", "92", "56", false),
    ("9x5.5", "9 × 5.5  (20 chỗ)", "90", "55", false),
    ("9.2x5.2", "9.2 × 5.2  (21 chỗ)", "92", "52", false),
    ("8.7x5.5", "8.7 × 5.5  (22 chỗ)", "87", "55", false),
    ("10.2x15.2", "10.2 × 15.2  (6 chỗ)", "152", "102", false),
    ("15.2x7.2", "15.2 × 7.2  (voucher, 8 chỗ)", "152", "72", true),
];

/// File pon chung của DanCard trên Illustrator: `dan_card_config_path.txt` trong thư mục dữ liệu
/// người dùng (Windows: %APPDATA%) trỏ tới file config, dòng `9.2x5.6 = đường dẫn`.
#[cfg(not(target_arch = "wasm32"))]
fn legacy_pon_path() -> Option<String> {
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))?;
    let config = std::fs::read_to_string(base.join("dan_card_config_path.txt")).ok()?;
    let text = std::fs::read_to_string(config.trim()).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == "9.2x5.6").map(|(_, v)| v.trim().to_string()))
        .filter(|p| std::path::Path::new(p).is_file())
}

#[cfg(target_arch = "wasm32")]
fn legacy_pon_path() -> Option<String> {
    None
}

fn layout_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    accordion(app, ui, "card", "Dàn Card", true, |app, ui| {
        style::label(ui, "Loại card");
        let cur = saved(app, "cardLayout", "9.2x5.6");
        let names: Vec<&str> = CARD_LAYOUTS.iter().map(|l| l.1).collect();
        let at = CARD_LAYOUTS.iter().position(|l| l.0 == cur);
        if let Some((key, _, w, h, _)) = style::radio_list(ui, "cardLayout", &names, at).and_then(|i| CARD_LAYOUTS.get(i))
            && cur != *key
        {
            save(app, "cardLayout", key);
            // Đổi loại card: khổ thẻ về khổ của loại đó.
            save(app, "cardW", w);
            save(app, "cardH", h);
        }
        let key = saved(app, "cardLayout", "9.2x5.6");
        let (_, _, dw, dh, voucher) =
            CARD_LAYOUTS.iter().find(|l| l.0 == key).or(CARD_LAYOUTS.first()).copied().unwrap_or(("", "", "92", "56", false));
        let with_card = voucher && check_box(app, ui, "voucherCard", "Ghép thêm card 9.2×5.6", false);
        let [w, h, gap] = num_fields(app, ui, [("Rộng thẻ (mm)", "cardW", dw), ("Cao thẻ (mm)", "cardH", dh), ("Khe khối (mm)", "cardGap", "2")]);
        style::label(ui, "Chiều xoay thẻ dọc");
        let rot = choice(app, ui, "cardRot", &[("90", "90°"), ("-90", "-90°")], "90", false);
        style::label(ui, "Loại in");
        let kind = choice(app, ui, "cardKind", &[("fast", "In nhanh"), ("offset", "Cardoffset")], "fast", false);
        let note = if kind == "offset" {
            style::label(ui, "Ghi chú (cardoffset)");
            text_box(app, ui, "cardNote", "cardoffset - cán mờ - bỏ hộp nhựa", "")
        } else {
            String::new()
        };
        // File pon chung: đã chọn, hoặc lấy từ config của DanCard trên Illustrator.
        if saved(app, "ponPath", "").is_empty() && !saved_bool(app, "ponLegacyChecked", false) {
            app.ui.dancard.insert("ponLegacyChecked".into(), Value::Bool(true));
            if let Some(p) = legacy_pon_path() {
                save(app, "ponPath", &p);
            }
        }
        style::label(ui, "File pon chung (artboard + pon)");
        file_row(app, ui, "ponPath", "Chưa chọn: dùng artboard sẵn có", ("File pon", &["ai", "pdf", "eps", "svg"]));
        if style::primary(ui, "Dàn card", has_selection(app)).clicked() {
            let params = json!({
                "layout": key, "voucherWithCard": with_card, "width": w, "height": h, "gap": gap,
                "rotation": rot, "offset": kind == "offset", "note": note,
                "pon": saved(app, "ponPath", ""),
            });
            run_and_note(app, ui, "danfile", "dancard.layout", params);
        }
        show_note(ui, "danfile", "Chọn loại card rồi bấm Dàn.");
    });
}

// ---------------------------------------------------------------- tab Auto Save

/// Ngày trong tháng hôm nay theo giờ Việt Nam (UTC+7, không đổi giờ theo mùa): ô Ngày đặt in.
#[cfg(target_arch = "wasm32")]
fn today_day() -> String {
    String::new()
}

/// Ngày trong tháng hôm nay theo giờ Việt Nam (UTC+7, không đổi giờ theo mùa): ô Ngày đặt in.
#[cfg(not(target_arch = "wasm32"))]
fn today_day() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64 + 7 * 3600;
    // Ngày dương lịch từ số ngày kể từ 1970-01-01 (thuật toán civil_from_days).
    let z = secs.div_euclid(86_400) + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (doy - (153 * mp + 2) / 5 + 1).to_string()
}

/// Tên file như Auto Save sẽ đặt (xem trước).
fn auto_save_name(prefix: &str, stt: u64, pad: usize, boxes: &str, note: &str, lamination: &str, date: &str) -> String {
    let note = if note.trim().is_empty() { String::new() } else { format!(" - {}", note.trim()) };
    format!("{prefix} - {stt:0pad$} - {boxes} hop{note} - {lamination} - {date}.pdf")
}

/// Mã bắt buộc của Auto Save (ô khóa của DanCard).
const AUTO_SAVE_PREFIX: &str = "1088";

fn auto_save_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::hint(ui, "Xếp hàng: trái = trước, phải = sau. Một mặt lưu PDF 1 trang; hai mặt lưu 2 trang. Không sửa file gốc.");
    style::label(ui, "Mã bắt buộc");
    style::locked(ui, AUTO_SAVE_PREFIX);
    let [start, pad] = num_fields(app, ui, [("STT bắt đầu", "asStart", "1"), ("Đệm số 0", "asPad", "2")]);
    style::hint(ui, "Đệm 2: 01, 02…; đệm 0: 1, 2…");
    style::label(ui, "Số hộp");
    let boxes = text_box_unit(app, ui, "asHop", "5", "5", "hộp");
    style::hint(ui, "Ngăn nhiều số bằng dấu phẩy. Mặc định: mỗi con × mỗi số.");
    let in_order = check_box(app, ui, "asHopTheoThuTu", "Ghép số hộp theo thứ tự từng con (thay vì mỗi con × mỗi số)", false);
    style::hint(ui, "Tick: một số cho một con theo thứ tự; số lượng phải bằng nhau.");
    style::label(ui, "Ghi chú (tuỳ chọn)");
    let note = text_box(app, ui, "asNote", "", "để trống nếu không cần");
    style::hint(ui, "Thêm vào tên file nếu có nhập.");
    style::label(ui, "Cán màng");
    let lamination = choice(app, ui, "asCanMang", &[("cm", "cm — cán màng"), ("km", "km — không màng")], "cm", false);
    style::label(ui, "Ngày đặt in");
    // Ngày mặc định là hôm nay mỗi lần mở app; sửa tay thì giữ trong phiên.
    if pstate::<String>(ui.ctx(), "dancard-asNgay-set").is_empty() {
        save(app, "asNgay", &today_day());
        set_pstate(ui.ctx(), "dancard-asNgay-set", "1".to_string());
    }
    let date = num_box(app, ui, "asNgay", "");
    style::label(ui, "Thư mục lưu (bỏ trống = hỏi khi chạy)");
    let folder = folder_row(app, ui, "asFolder", "vd: D:\\PDF\\1088");
    style::label(ui, "Xem trước tên file");
    let pad_n = pad.parse::<usize>().unwrap_or(0).min(8);
    let start_n = start.parse::<u64>().unwrap_or(1);
    let list: Vec<&str> = boxes.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let list = if list.is_empty() { vec![""] } else { list };
    let names: Vec<String> = list
        .iter()
        .take(4)
        .enumerate()
        .map(|(k, b)| auto_save_name(AUTO_SAVE_PREFIX, start_n + k as u64, pad_n, b, &note, &lamination, &date))
        .collect();
    style::output(ui, style::Out::Hint(&names.join("\n")));
    let selected = has_selection(app);
    if style::button(ui, "Clip 9.2 × 5.6 (tự xoay)", selected, true).clicked() {
        run_and_note(app, ui, "clipcard", "dancard.clipCard", json!({}));
    }
    show_note(ui, "clipcard", "");
    if style::primary(ui, "OK — Auto Save PDF", selected).clicked() {
        let params = json!({
            "prefix": AUTO_SAVE_PREFIX, "start": start, "pad": pad, "boxes": boxes, "boxesInOrder": in_order,
            "note": note, "lamination": lamination, "date": date, "folder": folder,
        });
        run_and_note(app, ui, "autosave", "dancard.autoSavePdf", params);
    }
    show_note(ui, "autosave", "Chọn object rồi bấm OK.");
}

// ---------------------------------------------------------------- tab Catalogue

fn catalogue_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Dàn trang catalogue đóng gáy giữa");
    style::hint(ui, "Chọn trang theo thứ tự đọc; số trang chia hết cho 4.");
    let (w, h) = size_pair(app, ui, "Kích thước 1 trang (cm)", ("catW", "21.1"), ("catH", "29.9"));
    style::hint(ui, "Tự chọn 2 cuốn/tờ; không vừa thì 1 cuốn/tờ.");
    style::label(ui, "Ghi chú BÌA (artboard 1)");
    let cover = text_box(app, ui, "catCoverNote", "", "");
    style::label(ui, "Ghi chú RUỘT (tự thêm RUỘT 1, RUỘT 2…)");
    let inside = text_box(app, ui, "catInsideNote", "", "");
    if style::primary(ui, "Dàn Catalogue", has_selection(app)).clicked() {
        run_and_note(app, ui, "catalogue", "dancard.catalogue", json!({"width": w, "height": h, "coverNote": cover, "insideNote": inside}));
    }
    show_note(ui, "catalogue", "Chọn trang rồi bấm Dàn Catalogue.");
}

// ---------------------------------------------------------------- tab Dàn tối ưu

/// Tab Dàn tối ưu: ba mục gập Dàn KTS, Dàn bế, Dàn Offset (như DanCard).
fn toi_uu_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    accordion(app, ui, "kts", "Dàn KTS", true, kts::section);
    accordion(app, ui, "be", "Dàn bế", false, be::section);
    accordion(app, ui, "offset", "Dàn Offset", false, tutro::section);
}

// ---------------------------------------------------------------- tab Dấu cắt

fn cut_marks_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Dấu cắt tự động");
    style::hint(
        ui,
        "Chọn bài (group, clip group) hoặc cả tờ đã dàn: mỗi khối một bộ dấu cắt, không đánh quanh chi tiết bên trong. Chọn một chi tiết (giữ Ctrl) cũng tính cả bài chứa nó.",
    );
    let [length, gap, edge] = num_fields(
        app,
        ui,
        [("Dài nét (mm)", "cutMarkLength", "1.4"), ("Cách bài (mm)", "cutMarkGap", "0"), ("Ngưỡng mép (mm)", "cutMarkEdge", "4")],
    );
    if style::primary(ui, "Đánh dấu cắt", has_selection(app)).clicked() {
        run_and_note(app, ui, "cutmarks", "dancard.cutMarks", json!({"length": length, "gap": gap, "edge": edge}));
    }
    show_note(ui, "cutmarks", "Chọn bài rồi đánh dấu cắt.");
}

// ---------------------------------------------------------------- tab Đổi tên

fn rename_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Tiền tố");
    let prefix = text_box(app, ui, "rnPrefix", "s", "");
    style::label(ui, "Kiểu đánh");
    let mode = choice(app, ui, "rnMode", &[("num", "Số: 1, 2, 3…"), ("low", "Chữ thường: a, b, c…"), ("up", "Chữ HOA: A, B, C…")], "num", true);
    let [start, pad] = num_fields(app, ui, [("Bắt đầu", "rnStart", "1"), ("Đệm số 0", "rnPad", "0")]);
    let repeat = num_field(app, ui, "Số layer lặp mỗi nhãn", "rnRepeat", "1");
    style::hint(ui, "Lặp 2: s1, s1, s2, s2…");
    style::label(ui, "Hướng đếm");
    let dir = choice(app, ui, "rnDir", &[("bottom", "Dưới cùng = đầu tiên"), ("top", "Trên cùng = đầu tiên")], "bottom", true);
    if style::primary(ui, "Đổi tên", has_selection(app)).clicked() {
        let params = json!({"prefix": prefix, "mode": mode, "start": start, "pad": pad, "repeat": repeat, "bottomUp": dir == "bottom"});
        run_and_note(app, ui, "rename", "dancard.rename", params);
    }
    show_note(ui, "rename", "Chọn object rồi bấm Đổi tên.");
}

// ---------------------------------------------------------------- tab Đo khung

fn transform_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let unit = Some(pstate::<String>(ui.ctx(), "dancard-unit")).filter(|u| !u.is_empty()).unwrap_or_else(|| "cm".into());
    style::label(ui, "Đơn vị");
    let units = ["cm", "mm", "pt"];
    if let Some(u) = style::segmented(ui, "dancard-unit", &units, units.iter().position(|u| *u == unit)).and_then(|i| units.get(i)) {
        set_pstate(ui.ctx(), "dancard-unit", (*u).to_string());
    }
    let unit = Some(pstate::<String>(ui.ctx(), "dancard-unit")).filter(|u| !u.is_empty()).unwrap_or_else(|| "cm".into());
    let preview = app.session.prefs.use_preview_bounds;
    let Some(st) = app.session.active() else {
        style::output(ui, style::Out::Hint("Chưa mở tài liệu."));
        return;
    };
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();
    let key = (std::sync::Arc::as_ptr(&st.doc) as usize, st.revision, selected, unit.clone(), preview);
    if key.2.is_empty() {
        style::output(ui, style::Out::Hint("Chọn card, clip group, group… để đọc khung transform."));
        return;
    }
    let id = egui::Id::new("dancard-transform-cache");
    let cached: Option<Cache> = ui.data(|d| d.get_temp(id));
    let value = match cached {
        Some(c) if c.key == key => c.value,
        _ => {
            let v = app.session.execute("dancard.transform", &json!({"unit": unit})).unwrap_or(Value::Null);
            ui.data_mut(|d| d.insert_temp(id, Cache { key, value: v.clone() }));
            v
        }
    };
    let Some(sel) = value.get("selection").filter(|s| s.is_object()) else {
        style::output(ui, style::Out::Error("Không đo được vùng chọn."));
        return;
    };
    let count = value["count"].as_u64().unwrap_or(0);
    style::title(ui, &if count > 1 { format!("Cả vùng chọn ({count} object)") } else { "Object đang chọn".into() });
    frame_rows(ui, sel, &unit);
    if count > 1
        && let Some(objects) = value["objects"].as_array()
    {
        accordion(app, ui, "measureEach", &format!("Từng object ({count})"), true, |_, ui| {
            egui::ScrollArea::vertical().id_salt("dancard-measure-each").max_height(220.0).show(ui, |ui| {
                for (i, o) in objects.iter().enumerate() {
                    style::mono(
                        ui,
                        &format!(
                            "{}. {}  {}°  {} × {}",
                            i + 1,
                            o["kind"].as_str().unwrap_or(""),
                            num(&o["angle"]),
                            num(&o["width"]),
                            num(&o["height"])
                        ),
                    );
                }
            });
        });
    }
    if style::button(ui, "Sao chép số đo (JSON)", true, true).clicked() {
        ui.ctx().copy_text(serde_json::to_string_pretty(&value).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;
    use crate::tests_labels::painted_text;

    #[test]
    fn hien_khung_xoay_cua_card_dang_chon() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        app.ui.dancard.insert("tab".into(), json!("measure"));
        // Card 9,2 × 5,6 cm xoay 20°.
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 260.787, "height": 158.740})).unwrap();
        app.run("object.rotate", json!({"angle": 20})).unwrap();
        let text = painted_text(&mut app, super::show);
        assert!(text.contains("20°"), "{text}");
        assert!(text.contains("9,2 cm") && text.contains("5,6 cm"), "{text}");
        // Không chọn gì: nhắc chọn object.
        app.run("select.none", json!({})).unwrap();
        assert!(painted_text(&mut app, super::show).contains("Chọn card"));
    }

    /// Đầu panel có Raster, Resize, Clip, tác giả và người thử; tab theo thứ tự của DanCard; tab Đổi
    /// tên hiện các ô của nó, giá trị gõ vào được giữ.
    #[test]
    fn dau_panel_va_tab_doi_ten() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let text = painted_text(&mut app, super::show);
        for s in ["Công cụ bình", "Raster", "Resize", "Clip", "AUTHOR", "TESTER", "Đo khung", "Đổi tên", "Dàn Card", "LOẠI CARD"] {
            assert!(text.contains(s), "{s}: {text}");
        }
        let order: Vec<usize> = super::TABS.iter().map(|(_, l)| text.find(l).unwrap_or_else(|| panic!("{l}: {text}"))).collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "tab theo thứ tự của DanCard: {order:?}");
        app.ui.dancard.insert("tab".into(), json!("rename"));
        app.ui.dancard.insert("rnPrefix".into(), json!("card"));
        let text = painted_text(&mut app, super::show);
        assert!(text.contains("TIỀN TỐ") && text.contains("HƯỚNG ĐẾM"), "{text}");
        assert!(text.contains("card"), "giá trị đã lưu hiện trong ô: {text}");
        assert!(super::is_fork_text("Đổi tên") && super::is_fork_text("DanCard: Raster") && !super::is_fork_text("Rasterize"));
        // Tab CTL Offset: người thử là Duẫn.
        app.ui.dancard.insert("tab".into(), json!("offset"));
        assert!(painted_text(&mut app, super::show).contains("Duẫn"));
        // Tab đã lưu không còn: về Dàn file.
        app.ui.dancard.insert("tab".into(), json!("khong-co"));
        assert_eq!(super::current_tab(&app), "danfile");
    }

    /// Mục gập thu lại thì không vẽ phần thân; trạng thái giữ lại theo khóa của mục.
    #[test]
    fn muc_gap_giu_trang_thai() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        app.ui.dancard.insert("tab".into(), json!("toiuu"));
        let text = painted_text(&mut app, super::show);
        assert!(text.contains("Dàn KTS") && text.contains("Dàn bế") && text.contains("Dàn Offset"), "{text}");
        assert!(text.contains("NGANG GIẤY (CM)"), "Dàn KTS mở sẵn: {text}");
        assert!(!text.contains("KHE GIỮA 2 KHUÔN"), "Dàn bế thu lại như DanCard: {text}");
        app.ui.dancard.insert("acc.be".into(), json!(true));
        assert!(painted_text(&mut app, super::show).contains("KHE GIỮA 2 KHUÔN (MM)"));
    }

    /// Clip group: đúng khung mask như bảng Transform, không phải phần giao với nội dung.
    #[test]
    fn clip_group_khop_bang_transform() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        // Nội dung chỉ phủ nửa trái của mask 200 × 100 pt.
        let content = app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].clone();
        let mask = app.run("shape.rectangle", json!({"x": 100, "y": 50, "width": 200, "height": 100})).unwrap()["id"].clone();
        app.run("select.set", json!({"ids": [content, mask]})).unwrap();
        app.run("object.clippingMask.make", json!({})).unwrap();
        let b = app.selection_box().unwrap();
        let v = app.session.execute("dancard.transform", &json!({"unit": "pt"})).unwrap();
        assert!((v["selection"]["width"].as_f64().unwrap() - b.rect.width()).abs() < 1e-3, "{v}");
        assert!((v["selection"]["height"].as_f64().unwrap() - b.rect.height()).abs() < 1e-3, "{v}");
        assert_eq!(v["selection"]["width"], json!(200.0));
    }
}
