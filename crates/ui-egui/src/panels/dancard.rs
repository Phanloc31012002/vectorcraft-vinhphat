//! Panel "Công cụ bình" (DanCard, IN Vĩnh Phát).
//!
//! - Đầu panel: Raster (CMYK, 500 ppi, nền trong suốt, đúng khung nhìn thấy), Resize và Clip theo
//!   kích thước nhập (giữ tâm).
//! - Tab Đo khung: khung transform của thứ đang chọn — góc xoay, rộng × cao, tâm — đọc bằng lệnh
//!   `dancard.transform`, khớp số với bảng Transform. Chọn clip group, group lồng nhiều tầng hay
//!   object con đều đọc thẳng. Riêng hình chữ nhật đặt nghiêng trong file .ai/PDF (không lưu góc)
//!   thì panel cho khung dò được, kèm số của bảng Transform để đối chiếu. Chỉ tính lại khi tài
//!   liệu, vùng chọn hoặc đơn vị đổi.
//! - Tab Đổi tên: `dancard.rename`.
//!
//! Giá trị đã gõ vào các ô được giữ lại giữa các lần mở app (`UiState::dancard`).

use egui::{Color32, RichText, Ui};
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

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
#[path = "dancard_tutro.rs"]
mod tutro;
#[path = "dancard_variable.rs"]
mod variable;

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

fn frame_rows(ui: &mut Ui, f: &Value, unit: &str, t: &Tokens, id: &str) {
    let mono = |ui: &mut Ui, s: String| {
        ui.label(RichText::new(s).monospace().color(t.text));
    };
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    egui::Grid::new(("dancard-frame", id)).num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        dim(ui, "Góc:");
        mono(ui, format!("{}°", num(&f["angle"])));
        dim(ui, "");
        dim(ui, source_label(&f["source"]));
        ui.end_row();
        dim(ui, "Rộng:");
        mono(ui, format!("{} {unit}", num(&f["width"])));
        dim(ui, "Cao:");
        mono(ui, format!("{} {unit}", num(&f["height"])));
        ui.end_row();
        // Tâm theo thước: bằng X/Y của bảng Transform khi chọn điểm tâm.
        dim(ui, "Tâm X:");
        mono(ui, num(&f["rulerCenter"][0]));
        dim(ui, "Tâm Y:");
        mono(ui, num(&f["rulerCenter"][1]));
        ui.end_row();
    });
    if let Some(tb) = f.get("transformBox") {
        ui.label(RichText::new(format!("Bảng Transform (khung thẳng): {} × {} {unit}", num(&tb["width"]), num(&tb["height"]))).color(t.text_dim));
    }
}

/// Các tab của panel: (khóa, nhãn).
const TABS: &[(&str, &str)] = &[
    ("measure", "Đo khung"),
    ("danfile", "Dàn card"),
    ("toiuu", "Dàn tối ưu"),
    ("cutmarks", "Dấu cắt"),
    ("danmau", "Dàn theo mẫu"),
    ("catalogue", "Catalogue"),
    ("offset", "CTL Offset"),
    ("rename", "Đổi tên"),
    ("variable", "Variable"),
    ("autosave", "Auto Save"),
];

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    header(app, ui);
    widgets::divider(ui);
    let tab = saved(app, "tab", "measure");
    ui.horizontal_wrapped(|ui| {
        for (key, label) in TABS {
            if ui.selectable_label(tab == *key, *label).clicked() {
                save(app, "tab", key);
            }
        }
    });
    widgets::divider(ui);
    match tab.as_str() {
        "rename" => rename_tab(app, ui),
        "cutmarks" => cut_marks_tab(app, ui),
        "danfile" => layout_tab(app, ui),
        "autosave" => auto_save_tab(app, ui),
        "catalogue" => catalogue_tab(app, ui),
        "toiuu" => toi_uu_tab(app, ui),
        "danmau" => mau::section(app, ui),
        "offset" => ctl::section(app, ui),
        "variable" => variable::section(app, ui),
        _ => transform_tab(app, ui),
    }
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

/// Ô chữ một dòng gắn với giá trị đã lưu `key`; trả giá trị hiện có.
fn text_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, default: &str, width: f32) -> String {
    let cur = saved(app, key, default);
    match widgets::exact_text_field(ui, ("dancard-field", key), &cur, width) {
        Some(v) => {
            save(app, key, v.trim());
            v.trim().to_string()
        }
        None => cur,
    }
}

/// Ô đánh dấu gắn với giá trị đã lưu `key`.
fn check_box(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, label: &str, default: bool) -> bool {
    let on = saved_bool(app, key, default);
    if widgets::check(ui, label, on, true) {
        app.ui.dancard.insert(key.into(), Value::Bool(!on));
        return !on;
    }
    on
}

/// Dãy nút chọn một trong `options` (giá trị, nhãn) gắn với `key`.
fn choice_row(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, options: &[(&str, &str)], default: &str) -> String {
    let cur = saved(app, key, default);
    let mut out = cur.clone();
    for (v, label) in options {
        if widgets::radio(ui, label, cur == *v, true) {
            save(app, key, v);
            out = v.to_string();
        }
    }
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
/// ghi chú; lỗi và thông báo khác như `show_note`.
fn show_count_note(ui: &mut Ui, area: &str, t: &Tokens, hint: &str) {
    let Note(text, err) = pstate::<Note>(ui.ctx(), &note_id(area));
    if !err && let Some((n, rest)) = count_prefix(&text) {
        ui.label(RichText::new(format!("{n} con / tờ")).strong().color(t.text));
        if !rest.is_empty() {
            ui.label(RichText::new(rest).color(t.text));
        }
        return;
    }
    show_note(ui, area, t, hint);
}

fn show_note(ui: &mut Ui, area: &str, t: &Tokens, hint: &str) {
    let Note(text, err) = pstate::<Note>(ui.ctx(), &note_id(area));
    if text.is_empty() {
        if !hint.is_empty() {
            ui.label(RichText::new(hint).color(t.text_dim));
        }
        return;
    }
    let color = if err { Color32::from_rgb(0xe5, 0x4b, 0x4b) } else { t.text };
    ui.label(RichText::new(text).color(color));
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

fn header(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    let mut open = pstate::<Popover>(ui.ctx(), "dancard-popover");
    ui.horizontal(|ui| {
        ui.label(RichText::new("Công cụ bình").strong().color(t.text));
        ui.add_space(6.0);
        let raster =
            ui.add_enabled(has_selection, egui::Button::new("Raster")).on_hover_text("Raster CMYK · 500 ppi · nền trong suốt, đúng khung nhìn thấy");
        if raster.clicked() {
            run_and_note(app, ui, "header", "dancard.rasterize", json!({}));
            open = Popover::None;
        }
        let resize = ui.add_enabled(has_selection, egui::Button::new("Resize").selected(open == Popover::Resize));
        if resize.on_hover_text("Resize theo kích thước nhập, giữ tâm").clicked() {
            open = if open == Popover::Resize { Popover::None } else { Popover::Resize };
        }
        let clip = ui.add_enabled(has_selection, egui::Button::new("Clip").selected(open == Popover::Clip));
        if clip.on_hover_text("Clip canh giữa theo kích thước nhập").clicked() {
            open = if open == Popover::Clip { Popover::None } else { Popover::Clip };
        }
    });
    if open != Popover::None {
        let (title, prefix, command, ok) = match open {
            Popover::Resize => ("Resize kích thước (giữ tâm)", "resize", "dancard.resize", "Resize"),
            _ => ("Clip kích thước (canh giữa)", "clip", "dancard.clipToSize", "OK"),
        };
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(title).color(t.text));
            ui.horizontal(|ui| {
                let w = text_box(app, ui, &format!("{prefix}W"), "5", 56.0);
                ui.label("×");
                let h = text_box(app, ui, &format!("{prefix}H"), "5", 56.0);
                let unit = saved(app, &format!("{prefix}Unit"), "cm");
                let units = ["cm", "mm", "in"];
                if let Some(i) = widgets::dropdown(ui, ("dancard-unit", prefix), &unit, &units, 56.0) {
                    save(app, &format!("{prefix}Unit"), units.get(i).copied().unwrap_or("cm"));
                }
                let unit = saved(app, &format!("{prefix}Unit"), "cm");
                ui.horizontal(|ui| {
                    if ui.button("Huỷ").clicked() {
                        open = Popover::None;
                    }
                    if ui.add_enabled(has_selection, egui::Button::new(ok)).clicked() {
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
            if open == Popover::Clip {
                check_box(app, ui, "clipFollowAngle", "Khung clip theo khung transform (card xoay, đặt nghiêng)", true);
            }
        });
    }
    set_pstate(ui.ctx(), "dancard-popover", open);
    show_note(ui, "header", &t, "");
}

// ---------------------------------------------------------------- tab Dàn card

/// Các loại card của tab Dàn card: (khóa, nhãn, rộng mm, cao mm, voucher).
const CARD_LAYOUTS: &[(&str, &str, &str, &str, bool)] = &[
    ("9.2x5.6", "9.2 x 5.6  (20 chỗ)", "92", "56", false),
    ("9x5.5", "9 x 5.5  (20 chỗ)", "90", "55", false),
    ("9.2x5.2", "9.2 x 5.2  (21 chỗ)", "92", "52", false),
    ("8.7x5.5", "8.7 x 5.5  (22 chỗ)", "87", "55", false),
    ("10.2x15.2", "10.2 x 15.2  (6 chỗ)", "152", "102", false),
    ("15.2x7.2", "15.2 x 7.2  (voucher, 8 chỗ)", "152", "72", true),
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
    let t = Tokens::get(ui.ctx());
    let label = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    label(ui, "Loại card");
    let cur = saved(app, "cardLayout", "9.2x5.6");
    for (key, name, w, h, _) in CARD_LAYOUTS {
        if widgets::radio(ui, name, cur == *key, true) && cur != *key {
            save(app, "cardLayout", key);
            // Đổi loại card: khổ thẻ về khổ của loại đó.
            save(app, "cardW", w);
            save(app, "cardH", h);
        }
    }
    let key = saved(app, "cardLayout", "9.2x5.6");
    let (_, _, dw, dh, voucher) = CARD_LAYOUTS.iter().find(|l| l.0 == key).copied().unwrap_or(CARD_LAYOUTS[0]);
    let with_card = voucher && check_box(app, ui, "voucherCard", "Ghép thêm card 9.2×5.6", false);
    ui.add_space(4.0);
    let (mut w, mut h, mut gap) = (String::new(), String::new(), String::new());
    egui::Grid::new("dancard-layout").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        for s in ["Rộng thẻ (mm)", "Cao thẻ (mm)", "Khe khối (mm)"] {
            label(ui, s);
        }
        ui.end_row();
        w = text_box(app, ui, "cardW", dw, 56.0);
        h = text_box(app, ui, "cardH", dh, 56.0);
        gap = text_box(app, ui, "cardGap", "2", 56.0);
        ui.end_row();
    });
    label(ui, "Chiều xoay thẻ dọc");
    let rot = ui.horizontal(|ui| choice_row(app, ui, "cardRot", &[("90", "90°"), ("-90", "-90°")], "90")).inner;
    label(ui, "Loại in");
    let kind = ui.horizontal(|ui| choice_row(app, ui, "cardKind", &[("fast", "In nhanh"), ("offset", "Cardoffset")], "fast")).inner;
    let note = if kind == "offset" {
        label(ui, "Ghi chú (cardoffset)");
        text_box(app, ui, "cardNote", "cardoffset - cán mờ - bỏ hộp nhựa", 220.0)
    } else {
        String::new()
    };
    ui.add_space(4.0);
    // File pon chung: đã chọn, hoặc lấy từ config của DanCard trên Illustrator.
    let mut pon = saved(app, "ponPath", "");
    if pon.is_empty() && !saved_bool(app, "ponLegacyChecked", false) {
        app.ui.dancard.insert("ponLegacyChecked".into(), Value::Bool(true));
        if let Some(p) = legacy_pon_path() {
            save(app, "ponPath", &p);
            pon = p;
        }
    }
    label(ui, "File pon chung (artboard + pon)");
    ui.horizontal(|ui| {
        let shown = if pon.is_empty() {
            "(chưa chọn: dùng artboard sẵn có)".to_string()
        } else {
            pon.rsplit(['/', '\\']).next().unwrap_or(&pon).to_string()
        };
        ui.label(RichText::new(shown).color(t.text)).on_hover_text(&pon);
        if ui.button("Chọn…").clicked() {
            crate::picks::button(app, |app| {
                let pick = crate::FilePick { filters: vec![("File pon", &["ai", "pdf", "eps", "svg"])], ..Default::default() };
                if let Some(p) = crate::picks::open(app, &pick) {
                    save(app, "ponPath", &p);
                }
            });
        }
        if !pon.is_empty() && ui.button("Bỏ").clicked() {
            save(app, "ponPath", "");
        }
    });
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Dàn card")).clicked() {
        let params = json!({
            "layout": key, "voucherWithCard": with_card, "width": w, "height": h, "gap": gap,
            "rotation": rot, "offset": kind == "offset", "note": note,
            "pon": saved(app, "ponPath", ""),
        });
        run_and_note(app, ui, "danfile", "dancard.layout", params);
    }
    show_note(ui, "danfile", &t, "Chọn loại card rồi bấm Dàn.");
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

fn auto_save_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let label = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    label(ui, "Xếp hàng: trái = trước, phải = sau. Một mặt lưu PDF 1 trang; hai mặt lưu 2 trang. Không sửa file gốc.");
    ui.add_space(4.0);
    label(ui, "Mã bắt buộc");
    let prefix = text_box(app, ui, "asPrefix", "1088", 120.0);
    let (mut start, mut pad) = (String::new(), String::new());
    egui::Grid::new("dancard-autosave").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        label(ui, "STT bắt đầu");
        label(ui, "Đệm số 0");
        ui.end_row();
        start = text_box(app, ui, "asStart", "1", 64.0);
        pad = text_box(app, ui, "asPad", "2", 64.0);
        ui.end_row();
    });
    label(ui, "Số hộp (nhiều số ngăn bằng dấu phẩy)");
    let boxes = text_box(app, ui, "asHop", "5", 160.0);
    let in_order = check_box(app, ui, "asHopTheoThuTu", "Ghép số hộp theo thứ tự từng con", false);
    label(ui, "Ghi chú (tuỳ chọn)");
    let note = text_box(app, ui, "asNote", "", 200.0);
    label(ui, "Cán màng");
    let lamination = ui.horizontal(|ui| choice_row(app, ui, "asCanMang", &[("cm", "cm — cán màng"), ("km", "km — không màng")], "cm")).inner;
    label(ui, "Ngày đặt in");
    // Ngày mặc định là hôm nay mỗi lần mở app; sửa tay thì giữ trong phiên.
    if pstate::<String>(ui.ctx(), "dancard-asNgay-set").is_empty() {
        save(app, "asNgay", &today_day());
        set_pstate(ui.ctx(), "dancard-asNgay-set", "1".to_string());
    }
    let date = text_box(app, ui, "asNgay", "", 64.0);
    label(ui, "Thư mục lưu");
    let folder = saved(app, "asFolder", "");
    ui.horizontal(|ui| {
        let shown = if folder.is_empty() { "(chưa chọn)".to_string() } else { folder.clone() };
        ui.label(RichText::new(shown).color(t.text));
        if ui.button("Chọn…").clicked() {
            crate::picks::button(app, |app| {
                if let Some(f) = crate::picks::folder(app) {
                    save(app, "asFolder", &f);
                }
            });
        }
    });
    label(ui, "Xem trước tên file");
    let pad_n = pad.parse::<usize>().unwrap_or(0).min(8);
    let start_n = start.parse::<u64>().unwrap_or(1);
    let list: Vec<&str> = boxes.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let list = if list.is_empty() { vec![""] } else { list };
    for (k, b) in list.iter().take(4).enumerate() {
        ui.label(RichText::new(auto_save_name(&prefix, start_n + k as u64, pad_n, b, &note, &lamination, &date)).monospace().color(t.text));
    }
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Clip 9.2 × 5.6 (tự xoay)")).clicked() {
        run_and_note(app, ui, "autosave", "dancard.clipCard", json!({}));
    }
    if ui.add_enabled(has_selection, egui::Button::new("OK — Auto Save PDF")).clicked() {
        let params = json!({
            "prefix": prefix, "start": start, "pad": pad, "boxes": boxes, "boxesInOrder": in_order,
            "note": note, "lamination": lamination, "date": date, "folder": saved(app, "asFolder", ""),
        });
        run_and_note(app, ui, "autosave", "dancard.autoSavePdf", params);
    }
    show_note(ui, "autosave", &t, "Chọn object rồi bấm OK.");
}

// ---------------------------------------------------------------- tab Catalogue

fn catalogue_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let label = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    ui.label(RichText::new("Dàn trang catalogue đóng gáy giữa").strong().color(t.text));
    label(ui, "Chọn trang theo thứ tự đọc; số trang chia hết cho 4.");
    label(ui, "Kích thước 1 trang (cm)");
    let (w, h) = ui
        .horizontal(|ui| {
            let w = text_box(app, ui, "catW", "21.1", 56.0);
            ui.label("×");
            let h = text_box(app, ui, "catH", "29.9", 56.0);
            (w, h)
        })
        .inner;
    label(ui, "Tự chọn 2 cuốn/tờ; không vừa thì 1 cuốn/tờ.");
    label(ui, "Ghi chú BÌA (artboard 1)");
    let cover = text_box(app, ui, "catCoverNote", "", 220.0);
    label(ui, "Ghi chú RUỘT (tự thêm RUỘT 1, RUỘT 2…)");
    let inside = text_box(app, ui, "catInsideNote", "", 220.0);
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Dàn Catalogue")).clicked() {
        run_and_note(app, ui, "catalogue", "dancard.catalogue", json!({"width": w, "height": h, "coverNote": cover, "insideNote": inside}));
    }
    show_note(ui, "catalogue", &t, "Chọn trang rồi bấm Dàn Catalogue.");
}

// ---------------------------------------------------------------- tab Dàn tối ưu

/// Tab Dàn tối ưu: ba mục gập Dàn KTS, Dàn bế, Dàn Offset (như DanCard).
fn toi_uu_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    egui::CollapsingHeader::new("Dàn KTS").id_salt("dancard-acc-kts").default_open(true).show(ui, |ui| kts::section(app, ui));
    egui::CollapsingHeader::new("Dàn bế").id_salt("dancard-acc-be").show(ui, |ui| be::section(app, ui));
    egui::CollapsingHeader::new("Dàn Offset").id_salt("dancard-acc-offset").show(ui, |ui| tutro::section(app, ui));
}

// ---------------------------------------------------------------- tab Dấu cắt

fn cut_marks_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new("Dấu cắt tự động").strong().color(t.text));
    ui.label(RichText::new("Chọn bài hoặc group đã dàn.").color(t.text_dim));
    let (mut length, mut gap, mut edge) = (String::new(), String::new(), String::new());
    egui::Grid::new("dancard-cutmarks").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
        for s in ["Dài nét (mm)", "Cách bài (mm)", "Ngưỡng mép (mm)"] {
            ui.label(RichText::new(s).color(t.text_dim));
        }
        ui.end_row();
        length = text_box(app, ui, "cutMarkLength", "1.4", 64.0);
        gap = text_box(app, ui, "cutMarkGap", "0", 64.0);
        edge = text_box(app, ui, "cutMarkEdge", "4", 64.0);
        ui.end_row();
    });
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Đánh dấu cắt")).clicked() {
        run_and_note(app, ui, "cutmarks", "dancard.cutMarks", json!({"length": length, "gap": gap, "edge": edge}));
    }
    show_note(ui, "cutmarks", &t, "Chọn bài rồi đánh dấu cắt.");
}

// ---------------------------------------------------------------- tab Đổi tên

fn rename_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let label = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    label(ui, "Tiền tố");
    let prefix = text_box(app, ui, "rnPrefix", "s", 160.0);
    ui.add_space(4.0);
    label(ui, "Kiểu đánh");
    let mode = choice_row(app, ui, "rnMode", &[("num", "Số: 1, 2, 3…"), ("low", "Chữ thường: a, b, c…"), ("up", "Chữ HOA: A, B, C…")], "num");
    ui.add_space(4.0);
    let (mut start, mut pad) = (String::new(), String::new());
    egui::Grid::new("dancard-rename").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        label(ui, "Bắt đầu");
        label(ui, "Đệm số 0");
        ui.end_row();
        start = text_box(app, ui, "rnStart", "1", 72.0);
        pad = text_box(app, ui, "rnPad", "0", 72.0);
        ui.end_row();
    });
    label(ui, "Số layer lặp mỗi nhãn");
    let repeat = text_box(app, ui, "rnRepeat", "1", 72.0);
    ui.label(RichText::new("Lặp 2: s1, s1, s2, s2…").color(t.text_dim));
    ui.add_space(4.0);
    label(ui, "Hướng đếm");
    let dir = choice_row(app, ui, "rnDir", &[("bottom", "Dưới cùng = đầu tiên"), ("top", "Trên cùng = đầu tiên")], "bottom");
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Đổi tên")).clicked() {
        let params = json!({"prefix": prefix, "mode": mode, "start": start, "pad": pad, "repeat": repeat, "bottomUp": dir == "bottom"});
        run_and_note(app, ui, "rename", "dancard.rename", params);
    }
    show_note(ui, "rename", &t, "Chọn object rồi bấm Đổi tên.");
}

// ---------------------------------------------------------------- tab Đo khung

fn transform_tab(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let unit = Some(pstate::<String>(ui.ctx(), "dancard-unit")).filter(|u| !u.is_empty()).unwrap_or_else(|| "cm".into());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Đơn vị:").color(t.text_dim));
        for u in ["cm", "mm", "pt"] {
            if ui.selectable_label(unit == u, u).clicked() {
                set_pstate(ui.ctx(), "dancard-unit", u.to_string());
            }
        }
    });
    widgets::divider(ui);
    let preview = app.session.prefs.use_preview_bounds;
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, "Chưa mở tài liệu.");
        return;
    };
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();
    let key = (std::sync::Arc::as_ptr(&st.doc) as usize, st.revision, selected, unit.clone(), preview);
    if key.2.is_empty() {
        widgets::dim_label(ui, "Chọn card, clip group, group… để đọc khung transform.");
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
        widgets::dim_label(ui, "Không đo được vùng chọn.");
        return;
    };
    let count = value["count"].as_u64().unwrap_or(0);
    ui.label(RichText::new(if count > 1 { format!("Cả vùng chọn ({count} object)") } else { "Object đang chọn".into() }).strong().color(t.text));
    frame_rows(ui, sel, &unit, &t, "selection");
    if count > 1
        && let Some(objects) = value["objects"].as_array()
    {
        widgets::divider(ui);
        egui::CollapsingHeader::new(format!("Từng object ({count})")).default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for (i, o) in objects.iter().enumerate() {
                    ui.label(
                        RichText::new(format!(
                            "{}. {}  {}°  {} × {}",
                            i + 1,
                            o["kind"].as_str().unwrap_or(""),
                            num(&o["angle"]),
                            num(&o["width"]),
                            num(&o["height"])
                        ))
                        .monospace()
                        .color(t.text),
                    );
                }
            });
        });
    }
    widgets::divider(ui);
    if ui.button("Sao chép số đo (JSON)").clicked() {
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

    /// Đầu panel có Raster, Resize, Clip; tab Đổi tên hiện các ô của nó, giá trị gõ vào được giữ.
    #[test]
    fn dau_panel_va_tab_doi_ten() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let text = painted_text(&mut app, super::show);
        for s in ["Raster", "Resize", "Clip", "Đo khung", "Đổi tên"] {
            assert!(text.contains(s), "{s}: {text}");
        }
        app.ui.dancard.insert("tab".into(), json!("rename"));
        app.ui.dancard.insert("rnPrefix".into(), json!("card"));
        let text = painted_text(&mut app, super::show);
        assert!(text.contains("Tiền tố") && text.contains("Hướng đếm"), "{text}");
        assert!(text.contains("card"), "giá trị đã lưu hiện trong ô: {text}");
        assert!(super::is_fork_text("Đổi tên") && super::is_fork_text("DanCard: Raster") && !super::is_fork_text("Rasterize"));
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
