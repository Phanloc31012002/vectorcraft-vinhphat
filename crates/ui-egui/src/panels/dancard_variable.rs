//! Panel Công cụ bình — tab Variable: nhập file dữ liệu CSV/TXT thành Variables của tài liệu
//! (`dancard.importVariables`). Mỗi cột một biến, mỗi dòng một bản ghi; object đặt tên (hoặc ghi
//! chú) trùng tên cột được gắn tự động và bản ghi 1 hiện ngay trên mẫu. Các bản ghi được duyệt
//! trong bảng Variables của VectorCraft.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{Note, check_box, choice_row, note_id, save, saved, saved_bool, show_note, text_box};
use crate::VectorcraftApp;
use crate::panels::{pstate, set_pstate};
use crate::theme::Tokens;

/// Vùng thông báo của tab.
const AREA: &str = "variable";
const COMMAND: &str = "dancard.importVariables";
/// Kết quả lần nhập gần nhất (biến, bản ghi, cảnh báo).
const RESULT: &str = "dancard-variable-result";
/// Số dòng biến / cảnh báo hiện tối đa.
const SHOWN: usize = 24;

/// Nhập (hoặc kiểm tra trước) file `path` với các tùy chọn đã lưu; ghi thông báo và kết quả.
/// Không cần `Ui`: chạy được cả khi hộp chọn file trả lời sau (picks).
fn import(app: &mut VectorcraftApp, ctx: &egui::Context, path: &str, dry_run: bool) {
    let params = json!({
        "path": path,
        "delimiter": saved(app, "varDelimiter", "auto"),
        "headers": saved_bool(app, "varHeaders", true),
        "transpose": saved_bool(app, "varTranspose", false),
        "bindBy": saved(app, "varBindBy", "name"),
        "datasetNames": saved(app, "varDatasetNames", ""),
        "backslashBreak": saved_bool(app, "varBackslash", false),
        "dryRun": dry_run,
    });
    let (text, err) = match app.run(COMMAND, params) {
        Ok(v) => {
            let text = v["message"].as_str().unwrap_or("Xong.").to_string();
            set_pstate(ctx, RESULT, v);
            (text, false)
        }
        Err(e) => {
            set_pstate(ctx, RESULT, Value::Null);
            (e, true)
        }
    };
    let text = text.trim_start_matches("OK:").trim_start_matches("ERR:").trim().to_string();
    app.status(text.lines().next().unwrap_or_default().to_string());
    set_pstate(ctx, &note_id(AREA), Note(text, err));
}

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    dim(ui, "Tạo nhiều bản từ mẫu và file dữ liệu.");
    dim(
        ui,
        "File CSV/TXT (giữ dấu tiếng Việt): mỗi cột một biến, mỗi dòng một bản ghi. Cột có tên bắt đầu bằng # là biến ẩn/hiện (true/false, on/off).",
    );
    dim(ui, "Đặt tên object trùng tên cột (tab Đổi tên hoặc bảng Layers) để gắn tự động.");
    ui.add_space(4.0);

    dim(ui, "Gắn object theo");
    ui.horizontal_wrapped(|ui| {
        choice_row(app, ui, "varBindBy", &[("name", "Tên object"), ("note", "Ghi chú (Attributes)"), ("none", "Không gắn")], "name");
    });
    ui.horizontal_wrapped(|ui| {
        check_box(app, ui, "varHeaders", "Dòng đầu là tên biến", true);
        check_box(app, ui, "varTranspose", "Đảo hàng ↔ cột", false);
    });
    check_box(app, ui, "varBackslash", "Hai dấu \\\\ trong ô là xuống dòng", false);
    dim(ui, "Dấu phân cách");
    ui.horizontal_wrapped(|ui| {
        choice_row(app, ui, "varDelimiter", &[("auto", "Tự nhận"), ("comma", "Dấu phẩy"), ("semicolon", "Chấm phẩy"), ("tab", "Tab")], "auto");
    });
    dim(ui, "Tên bản ghi theo cột (để trống: Record-1, Record-2…)");
    text_box(app, ui, "varDatasetNames", "", 160.0);

    let existing = app.session.active().map_or(0, |st| st.doc.variables.variables.len());
    if existing > 0 {
        dim(ui, &format!("Tài liệu đang có {existing} biến: lần nhập sẽ thay toàn bộ biến và bản ghi cũ (hoàn tác được)."));
    }

    ui.add_space(6.0);
    let has_doc = app.session.active().is_some();
    let path = saved(app, "varPath", "");
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(has_doc, egui::Button::new("Chọn file dữ liệu…")).clicked() {
            let ctx = ui.ctx().clone();
            let folder = path.rsplit_once(['/', '\\']).map(|(f, _)| f.to_string());
            crate::picks::button(app, move |app| {
                let pick = crate::FilePick { filters: vec![("CSV / TXT", &["csv", "txt", "tsv"])], folder: folder.clone(), ..Default::default() };
                if let Some(p) = crate::picks::open(app, &pick) {
                    save(app, "varPath", &p);
                    import(app, &ctx, &p, false);
                }
            });
        }
        let again = has_doc && !path.is_empty();
        if ui.add_enabled(again, egui::Button::new("Nhập lại")).on_hover_text("Đọc lại file đã chọn (sau khi sửa dữ liệu)").clicked()
        {
            import(app, ui.ctx(), &path, false);
        }
        let check = ui
            .add_enabled(again, egui::Button::new("Kiểm tra trước"))
            .on_hover_text("Chỉ báo sẽ tạo bao nhiêu biến, bản ghi và gắn được bao nhiêu object; không đổi tài liệu");
        if check.clicked() {
            import(app, ui.ctx(), &path, true);
        }
    });
    if !path.is_empty() {
        let file = path.rsplit(['/', '\\']).next().unwrap_or(&path);
        ui.label(RichText::new(format!("File: {file}")).color(t.text)).on_hover_text(&path);
    }
    show_note(ui, AREA, &t, "Bấm Chọn file dữ liệu… để nhập.");

    // Kết quả lần nhập gần nhất: biến, số object gắn, bản ghi, cảnh báo.
    let result = pstate::<Value>(ui.ctx(), RESULT);
    if let Some(vars) = result["variables"].as_array()
        && !vars.is_empty()
    {
        ui.add_space(4.0);
        egui::Grid::new("dancard-variable-vars").num_columns(3).spacing([8.0, 2.0]).striped(true).show(ui, |ui| {
            dim(ui, "Biến");
            dim(ui, "Loại");
            dim(ui, "Object gắn");
            ui.end_row();
            for v in vars.iter().take(SHOWN) {
                ui.label(RichText::new(v["name"].as_str().unwrap_or_default()).color(t.text));
                dim(ui, if v["kind"] == "visibility" { "ẩn/hiện" } else { "chữ" });
                match v["bound"].as_u64().unwrap_or(0) {
                    0 => ui.label(RichText::new("chưa gắn").color(t.accent_strong)),
                    n => ui.label(RichText::new(n.to_string()).color(t.text)),
                };
                ui.end_row();
            }
        });
        if vars.len() > SHOWN {
            dim(ui, &format!("… và {} biến nữa", vars.len() - SHOWN));
        }
    }
    if let Some(sets) = result["datasets"].as_array() {
        let first: Vec<&str> = sets.iter().take(3).filter_map(Value::as_str).collect();
        let more = if sets.len() > first.len() { ", …" } else { "" };
        dim(ui, &format!("{} bản ghi: {}{more}", sets.len(), first.join(", ")));
    }
    if let Some(warnings) = result["warnings"].as_array() {
        for w in warnings.iter().filter_map(Value::as_str).take(SHOWN) {
            ui.label(RichText::new(format!("• {w}")).color(t.text));
        }
    }

    ui.add_space(6.0);
    dim(ui, "Duyệt các bản ghi trong bảng Variables của VectorCraft: chọn bản ghi hoặc bấm mũi tên trước/sau để mẫu đổi theo.");
    if ui.button("Mở bảng Variables").clicked()
        && let Err(e) = app.run("window.panel", json!({"panel": "variables"}))
    {
        app.status(e);
    }
}
