//! Panel Công cụ bình — tab Variable: nhập file dữ liệu CSV/TXT thành Variables của tài liệu
//! (`dancard.importVariables`). Mỗi cột một biến, mỗi dòng một bản ghi; object đặt tên (hoặc ghi
//! chú) trùng tên cột được gắn tự động và bản ghi 1 hiện ngay trên mẫu. Các bản ghi được duyệt
//! trong bảng Variables của VectorCraft.

use egui::Ui;
use serde_json::{Value, json};

use super::{Note, check_box, choice, note_id, save, saved, saved_bool, show_note, style, text_box};
use crate::VectorcraftApp;
use crate::panels::{pstate, set_pstate};

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
        "datasetNames": saved(app, "varDatasetNames", "").trim(),
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
    style::hint(ui, "Tạo nhiều bản từ mẫu và file dữ liệu.");
    style::hint(
        ui,
        "File CSV/TXT (giữ dấu tiếng Việt): mỗi cột một biến, mỗi dòng một bản ghi. Cột có tên bắt đầu bằng # là biến ẩn/hiện (true/false, on/off).",
    );
    style::hint(ui, "Đặt tên object trùng tên cột (tab Đổi tên hoặc bảng Layers) để gắn tự động.");

    style::label(ui, "Gắn object theo");
    choice(app, ui, "varBindBy", &[("name", "Tên object"), ("note", "Ghi chú (Attributes)"), ("none", "Không gắn")], "name", true);
    check_box(app, ui, "varHeaders", "Dòng đầu là tên biến", true);
    check_box(app, ui, "varTranspose", "Đảo hàng ↔ cột", false);
    check_box(app, ui, "varBackslash", "Hai dấu \\\\ trong ô là xuống dòng", false);
    style::label(ui, "Dấu phân cách");
    choice(app, ui, "varDelimiter", &[("auto", "Tự nhận"), ("comma", "Phẩy"), ("semicolon", "Chấm phẩy"), ("tab", "Tab")], "auto", false);
    style::label(ui, "Tên bản ghi theo cột (để trống: Record-1, Record-2…)");
    text_box(app, ui, "varDatasetNames", "", "");

    let existing = app.session.active().map_or(0, |st| st.doc.variables.variables.len());
    if existing > 0 {
        style::hint(ui, &format!("Tài liệu đang có {existing} biến: lần nhập sẽ thay toàn bộ biến và bản ghi cũ (hoàn tác được)."));
    }

    let has_doc = app.session.active().is_some();
    let path = saved(app, "varPath", "");
    if style::primary(ui, "Chọn file dữ liệu…", has_doc).clicked() {
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
    let mut action = None;
    style::columns(ui, 2, |ui, i| {
        let (label, tip, dry) = if i == 0 {
            ("Nhập lại", "Đọc lại file đã chọn (sau khi sửa dữ liệu)", false)
        } else {
            ("Kiểm tra trước", "Chỉ báo sẽ tạo bao nhiêu biến, bản ghi và gắn được bao nhiêu object; không đổi tài liệu", true)
        };
        if style::button(ui, label, again, true).on_hover_text(tip).clicked() {
            action = Some(dry);
        }
    });
    if let Some(dry) = action {
        import(app, ui.ctx(), &path, dry);
    }
    if !path.is_empty() {
        style::text(ui, &format!("File: {}", super::file_name(&path))).on_hover_text(&path);
    }
    show_note(ui, AREA, "Bấm Chọn file dữ liệu… để nhập.");

    // Kết quả lần nhập gần nhất: biến, số object gắn, bản ghi, cảnh báo.
    let result = pstate::<Value>(ui.ctx(), RESULT);
    if let Some(vars) = result["variables"].as_array()
        && !vars.is_empty()
    {
        // Bảng ba cột chia đều (tên biến dài thì xuống dòng trong cột của nó).
        style::columns(ui, 3, |ui, i| {
            style::label(ui, ["Biến", "Loại", "Object gắn"].get(i).copied().unwrap_or_default());
        });
        for v in vars.iter().take(SHOWN) {
            style::columns(ui, 3, |ui, i| match i {
                0 => {
                    style::text(ui, v["name"].as_str().unwrap_or_default());
                }
                1 => style::hint(ui, if v["kind"] == "visibility" { "ẩn/hiện" } else { "chữ" }),
                _ => match v["bound"].as_u64().unwrap_or(0) {
                    0 => style::hint_strong(ui, "chưa gắn"),
                    n => {
                        style::text(ui, &n.to_string());
                    }
                },
            });
        }
        if vars.len() > SHOWN {
            style::hint(ui, &format!("… và {} biến nữa", vars.len() - SHOWN));
        }
    }
    if let Some(sets) = result["datasets"].as_array() {
        let first: Vec<&str> = sets.iter().take(3).filter_map(Value::as_str).collect();
        let more = if sets.len() > first.len() { ", …" } else { "" };
        style::hint(ui, &format!("{} bản ghi: {}{more}", sets.len(), first.join(", ")));
    }
    if let Some(warnings) = result["warnings"].as_array() {
        for w in warnings.iter().filter_map(Value::as_str).take(SHOWN) {
            style::text(ui, &format!("• {w}"));
        }
    }

    style::hint(ui, "Duyệt các bản ghi trong bảng Variables của VectorCraft: chọn bản ghi hoặc bấm mũi tên trước/sau để mẫu đổi theo.");
    if style::button(ui, "Mở bảng Variables", true, true).clicked()
        && let Err(e) = app.run("window.panel", json!({"panel": "variables"}))
    {
        app.status(e);
    }
}
