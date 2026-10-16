//! Panel Công cụ bình — tab Variable: nhập file dữ liệu CSV/TXT thành Variables của tài liệu
//! (`dancard.importVariables`). Mỗi cột một biến, mỗi dòng một bản ghi; object đặt tên (hoặc ghi
//! chú) trùng tên cột được gắn tự động và bản ghi 1 hiện ngay trên mẫu. Các bản ghi được duyệt
//! trong bảng Variables của VectorCraft.
//!
//! Xuất file theo bản ghi (`dancard.exportVariables`): chọn thư mục rồi bấm Xuất; mỗi bản ghi
//! lưu một file (PDF hoặc AI) tên = tiền tố + tên bản ghi. Panel chạy lệnh từng bản ghi một, mỗi
//! khung hình một bản ghi ([`tick`]), nên app không đứng, thanh tiến độ chạy và bấm Dừng được.

use egui::Ui;
use serde_json::{Value, json};

use super::{Note, check_box, choice, folder_row, note_id, num_fields, save, saved, saved_bool, show_note, style, text_box};
use crate::VectorcraftApp;
use crate::panels::{pstate, set_pstate};

/// Vùng thông báo của tab.
const AREA: &str = "variable";
const COMMAND: &str = "dancard.importVariables";
/// Kết quả lần nhập gần nhất (biến, bản ghi, cảnh báo).
const RESULT: &str = "dancard-variable-result";
/// Số biến chưa gắn / cảnh báo liệt kê tối đa (tab gọn; danh sách đủ ở bảng Variables).
const SHOWN: usize = 3;
/// Lệnh xuất file theo bản ghi và vùng thông báo của nó.
const EXPORT: &str = "dancard.exportVariables";
const EXPORT_AREA: &str = "varexport";
/// Lần xuất đang chạy ([`Job`]).
const JOB: &str = "dancard-varexport-job";
/// Số lỗi liệt kê khi xuất xong.
const ERRORS_SHOWN: usize = 5;

/// Lần xuất đang chạy: mỗi khung hình xuất bản ghi `next`, tới `last`.
#[derive(Clone, Default)]
struct Job {
    /// Tài liệu (`DocState::uid`) và lần sửa của nó lúc bắt đầu: tài liệu khác hay bị sửa thì dừng.
    doc: u64,
    revision: u64,
    /// Thư mục, định dạng, tiền tố, ghi đè.
    params: Value,
    first: usize,
    last: usize,
    next: usize,
    /// Tên file của từng bản ghi trong đoạn (lần kiểm tra trước), để hiện file đang xuất.
    files: Vec<String>,
    saved: usize,
    errors: Vec<String>,
    /// Đã bấm Dừng: dừng ở khung hình sau.
    stop: bool,
}

impl Job {
    fn total(&self) -> usize {
        self.last + 1 - self.first.min(self.last + 1)
    }

    fn done(&self) -> usize {
        self.next.saturating_sub(self.first).min(self.total())
    }
}

/// Đang có lần xuất chạy chưa.
fn running(ctx: &egui::Context) -> bool {
    pstate::<Option<Job>>(ctx, JOB).is_some()
}

/// Kết thúc lần xuất: thông báo kết quả (`why`: lý do dừng sớm, nếu có).
fn finish(app: &mut VectorcraftApp, ctx: &egui::Context, job: &Job, why: Option<&str>) {
    set_pstate::<Option<Job>>(ctx, JOB, None);
    let folder = job.params["folder"].as_str().unwrap_or_default();
    let format = job.params["format"].as_str().unwrap_or("pdf").to_uppercase();
    let mut text = match why {
        Some(why) => format!("{why} Đã xuất {}/{} file {format}.", job.saved, job.total()),
        None => format!("Đã xuất {}/{} file {format}.", job.saved, job.total()),
    };
    text.push_str(&format!("\nThư mục: {folder}"));
    if !job.errors.is_empty() {
        text.push_str(&format!("\nLỗi ({}):", job.errors.len()));
        for e in job.errors.iter().take(ERRORS_SHOWN) {
            text.push_str(&format!("\n- {e}"));
        }
        if job.errors.len() > ERRORS_SHOWN {
            text.push_str(&format!("\n- … và {} lỗi nữa", job.errors.len() - ERRORS_SHOWN));
        }
    }
    app.status(text.lines().next().unwrap_or_default().to_string());
    let failed = why.is_some() || job.saved < job.total();
    set_pstate(ctx, &note_id(EXPORT_AREA), Note(text, failed));
}

/// Mỗi khung hình (panel đang hiện, ở tab nào cũng vậy): xuất một bản ghi của lần xuất đang chạy.
pub(super) fn tick(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(mut job) = pstate::<Option<Job>>(ctx, JOB) else { return };
    if job.stop {
        finish(app, ctx, &job, Some("Đã dừng."));
        return;
    }
    let same = app.session.active().is_some_and(|st| st.uid == job.doc && st.revision == job.revision);
    if !same {
        finish(app, ctx, &job, Some("Tài liệu đã đổi hoặc bị sửa trong lúc xuất nên đã dừng."));
        return;
    }
    let mut params = job.params.clone();
    params["from"] = json!(job.next);
    params["to"] = json!(job.next);
    match app.run(EXPORT, params) {
        Ok(v) => {
            job.saved += usize::try_from(v["saved"].as_u64().unwrap_or(0)).unwrap_or(0);
            job.errors.extend(v["errors"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
        }
        Err(e) => {
            let name = job.files.get(job.next.saturating_sub(job.first)).cloned().unwrap_or_else(|| format!("bản ghi {}", job.next));
            job.errors.push(format!("{name}: {}", e.lines().next().unwrap_or_default()));
        }
    }
    job.next += 1;
    if job.next > job.last {
        finish(app, ctx, &job, None);
    } else {
        set_pstate(ctx, JOB, Some(job));
        ctx.request_repaint();
    }
}

/// Số thứ tự bản ghi trong ô `text` (trống: `default`).
fn record_number(text: &str, default: usize) -> Option<usize> {
    if text.trim().is_empty() { Some(default) } else { text.trim().parse().ok().filter(|n| *n >= 1) }
}

/// Phần Xuất file theo bản ghi của tab.
fn export_section(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::title(ui, "Xuất file theo bản ghi");
    let records = app.session.active().map_or(0, |st| st.doc.variables.datasets.len());
    if records == 0 {
        style::hint(ui, "Nhập file dữ liệu trước: mỗi bản ghi sẽ lưu thành một file riêng.");
        return;
    }
    let job = pstate::<Option<Job>>(ui.ctx(), JOB);
    let busy = job.is_some();
    style::hint(ui, &format!("{records} bản ghi. Mỗi bản ghi áp lên mẫu rồi lưu cả các artboard thành một file; tài liệu đang mở không đổi."));
    style::label(ui, "Thư mục lưu");
    let folder = folder_row(app, ui, "varOutFolder", "vd: D:\\Xuat\\Ve so");
    style::label(ui, "Định dạng");
    let format = choice(app, ui, "varOutFormat", &[("pdf", "PDF"), ("ai", "AI (sửa tiếp được)")], "pdf", false);
    style::label(ui, "Tiền tố tên file (tuỳ chọn)");
    text_box(app, ui, "varOutPrefix", "", "vd: Ve so - ");
    // Ô tiền tố giữ khoảng trắng ở cuối ("Ve so - 001.pdf").
    let prefix = saved(app, "varOutPrefix", "");
    let [from, to] = num_fields(app, ui, [("Từ bản ghi", "varOutFrom", "1"), ("Đến bản ghi (trống: cuối)", "varOutTo", "")]);
    let overwrite = check_box(app, ui, "varOutOverwrite", "Ghi đè file trùng tên", false);
    let range = record_number(&from, 1).zip(record_number(&to, records)).filter(|(a, b)| a <= b && *b <= records);
    let first_name = app.session.active().and_then(|st| st.doc.variables.datasets.get(range.map_or(0, |r| r.0 - 1)).map(|d| d.name.clone()));
    if let Some(name) = first_name {
        style::hint(ui, &format!("Tên file: {prefix}{name}.{format}…"));
    }
    if range.is_none() {
        style::hint_strong(ui, &format!("Chọn bản ghi từ 1 đến {records} (bản ghi đầu không lớn hơn bản ghi cuối)."));
    }
    let count = range.map_or(0, |(a, b)| b + 1 - a);
    let label = if busy { "Đang xuất…".to_string() } else { format!("Xuất {count} file") };
    if style::primary(ui, &label, !busy && range.is_some()).clicked()
        && let Some((first, last)) = range
    {
        start(app, ui.ctx(), json!({"folder": folder, "format": format, "prefix": prefix, "overwrite": overwrite}), first, last);
    }
    if let Some(job) = &job {
        let current = job.files.get(job.next.saturating_sub(job.first)).map_or("", String::as_str);
        let done = job.done() as f32 / job.total().max(1) as f32;
        style::progress(ui, done, &format!("{}/{} · {current}", job.done(), job.total()));
        if style::button(ui, "Dừng", !job.stop, true).clicked() {
            set_pstate(ui.ctx(), JOB, Some(Job { stop: true, ..job.clone() }));
        }
    }
    if !busy {
        show_note(ui, EXPORT_AREA, "Chọn thư mục rồi bấm Xuất.");
    }
}

/// Kiểm tra trước cả đoạn (tên file, thư mục, file đã có), rồi bắt đầu lần xuất từng bản ghi.
fn start(app: &mut VectorcraftApp, ctx: &egui::Context, params: Value, first: usize, last: usize) {
    if running(ctx) {
        return;
    }
    let mut check = params.clone();
    check["from"] = json!(first);
    check["to"] = json!(last);
    check["dryRun"] = json!(true);
    let plan = match app.run(EXPORT, check) {
        Ok(v) => v,
        Err(e) => {
            app.status(e.lines().next().unwrap_or_default().to_string());
            set_pstate(ctx, &note_id(EXPORT_AREA), Note(e, true));
            return;
        }
    };
    let folder = params["folder"].as_str().unwrap_or_default();
    let existing = plan["existing"].as_array().map_or(0, Vec::len);
    if folder.is_empty() || (existing > 0 && !params["overwrite"].as_bool().unwrap_or(false)) {
        let text = if folder.is_empty() {
            "Chưa chọn thư mục lưu. Hãy bấm \"Chọn…\" để chọn thư mục.".to_string()
        } else {
            plan["message"].as_str().unwrap_or_default().to_string()
        };
        app.status(text.lines().last().unwrap_or_default().to_string());
        set_pstate(ctx, &note_id(EXPORT_AREA), Note(text, true));
        return;
    }
    let Some((doc, revision)) = app.session.active().map(|st| (st.uid, st.revision)) else { return };
    let files = plan["files"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    set_pstate(ctx, &note_id(EXPORT_AREA), Note(String::new(), false));
    set_pstate(ctx, JOB, Some(Job { doc, revision, params, first, last, next: first, files, ..Job::default() }));
    ctx.request_repaint();
}

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

/// Các tùy chọn nhập (mục gập, thu lại sẵn: mặc định đúng cho hầu hết file).
fn import_options(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Gắn object theo");
    choice(app, ui, "varBindBy", &[("name", "Tên object"), ("note", "Ghi chú"), ("none", "Không gắn")], "name", false);
    style::label(ui, "Dấu phân cách");
    choice(app, ui, "varDelimiter", &[("auto", "Tự nhận"), ("comma", "Phẩy"), ("semicolon", "Chấm phẩy"), ("tab", "Tab")], "auto", false);
    check_box(app, ui, "varHeaders", "Dòng đầu là tên biến", true);
    check_box(app, ui, "varTranspose", "Đảo hàng ↔ cột", false);
    check_box(app, ui, "varBackslash", "Hai dấu \\\\ trong ô là xuống dòng", false);
}

/// Kết quả lần nhập gần nhất, gọn: một dòng tóm tắt, biến chưa gắn và cảnh báo (mỗi loại vài
/// dòng). Danh sách đủ ở bảng Variables.
fn import_summary(ui: &mut Ui) {
    let result = pstate::<Value>(ui.ctx(), RESULT);
    let Some(vars) = result["variables"].as_array().filter(|v| !v.is_empty()) else { return };
    let bound = vars.iter().filter(|v| v["bound"].as_u64().unwrap_or(0) > 0).count();
    let sets: Vec<&str> = result["datasets"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let first = sets.iter().take(3).copied().collect::<Vec<_>>().join(", ");
    let more = if sets.len() > 3 { ", …" } else { "" };
    style::text(ui, &format!("{} biến ({bound} đã gắn object) · {} bản ghi: {first}{more}", vars.len(), sets.len()));
    let unbound: Vec<&str> = vars.iter().filter(|v| v["bound"].as_u64().unwrap_or(0) == 0).filter_map(|v| v["name"].as_str()).collect();
    if !unbound.is_empty() {
        let shown = unbound.iter().take(SHOWN).copied().collect::<Vec<_>>().join(", ");
        let rest = if unbound.len() > SHOWN { format!(" và {} biến nữa", unbound.len() - SHOWN) } else { String::new() };
        style::hint_strong(ui, &format!("Chưa gắn: {shown}{rest} (đặt tên object trùng tên cột rồi Nhập lại)."));
    }
    let warnings: Vec<&str> = result["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    for w in warnings.iter().take(SHOWN) {
        style::hint(ui, &format!("• {w}"));
    }
    if warnings.len() > SHOWN {
        style::hint(ui, &format!("… và {} cảnh báo nữa", warnings.len() - SHOWN));
    }
}

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::hint(ui, "File CSV/TXT: mỗi cột một biến, mỗi dòng một bản ghi; object đặt tên trùng tên cột được gắn tự động.");
    style::label(ui, "Tên bản ghi theo cột (trống: Record-1, Record-2…)");
    text_box(app, ui, "varDatasetNames", "", "vd: cột số nhảy");
    super::accordion(app, ui, "varOptions", "Tùy chọn nhập", false, import_options);

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
    if !path.is_empty() {
        let again = has_doc;
        let mut action = None;
        style::columns(ui, 2, |ui, i| {
            let (label, tip, dry) = if i == 0 {
                ("Nhập lại", "Đọc lại file đã chọn (sau khi sửa dữ liệu); thay biến và bản ghi cũ, hoàn tác được", false)
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
        style::text(ui, &format!("File: {}", super::file_name(&path))).on_hover_text(&path);
    }
    show_note(ui, AREA, "Bấm Chọn file dữ liệu… để nhập.");
    import_summary(ui);
    if style::button(ui, "Mở bảng Variables (duyệt bản ghi)", true, true).clicked()
        && let Err(e) = app.run("window.panel", json!({"panel": "variables"}))
    {
        app.status(e);
    }
    ui.add_space(6.0);
    export_section(app, ui);
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::tests_labels::painted_text;

    /// Mẫu có chữ "So" và 3 bản ghi tên 001, 002, 003.
    fn app_with_records() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 200})).unwrap();
        let t = app.run("text.create", json!({"x": 20, "y": 60, "text": "Số"})).unwrap()["id"].clone();
        app.run("object.setProps", json!({"id": t, "name": "So"})).unwrap();
        app.run("dancard.importVariables", json!({"text": "So\n001\n002\n003\n", "datasetNames": "So"})).unwrap();
        app
    }

    fn out_dir(tag: &str) -> std::path::PathBuf {
        let d = vectorcraft_testkit::temp_dir(&format!("dancard-panel-varexport-{tag}"));
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            std::fs::remove_file(e.path()).unwrap();
        }
        d
    }

    fn listing(d: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    fn note(ctx: &egui::Context) -> Note {
        pstate::<Note>(ctx, &note_id(EXPORT_AREA))
    }

    /// Tab Variable có phần xuất khi tài liệu có bản ghi, nút ghi số file của đoạn đã chọn.
    #[test]
    fn tab_hien_phan_xuat_theo_ban_ghi() {
        let mut app = app_with_records();
        app.ui.dancard.insert("tab".into(), json!("variable"));
        app.ui.dancard.insert("varOutPrefix".into(), json!("Ve "));
        app.ui.dancard.insert("varOutFrom".into(), json!("2"));
        let text = painted_text(&mut app, super::super::show);
        for s in ["Xuất file theo bản ghi", "THƯ MỤC LƯU", "Xuất 2 file", "Tên file: Ve 002.pdf", "Ghi đè file trùng tên"] {
            assert!(text.contains(s), "{s}: {text}");
        }
        // Đoạn sai: nút không bấm được, nhắc chọn lại.
        app.ui.dancard.insert("varOutFrom".into(), json!("9"));
        let text = painted_text(&mut app, super::super::show);
        assert!(text.contains("Xuất 0 file") && text.contains("từ 1 đến 3"), "{text}");
    }

    /// Kết quả nhập gọn: một dòng tóm tắt và vài dòng biến chưa gắn / cảnh báo, không còn bảng dài;
    /// tùy chọn nhập thu lại sẵn.
    #[test]
    fn ket_qua_nhap_gon() {
        let mut app = app_with_records();
        app.ui.dancard.insert("tab".into(), json!("variable"));
        let vars: Vec<Value> = (1..=8).map(|i| json!({"name": format!("Bien{i}"), "kind": "text", "bound": u64::from(i == 1)})).collect();
        let warnings: Vec<String> = (1..=6).map(|i| format!("Cảnh báo {i}")).collect();
        let result = json!({"variables": vars, "datasets": ["001", "002", "003", "004"], "warnings": warnings});
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        set_pstate(&ctx, RESULT, result);
        let frame = |app: &mut VectorcraftApp| ctx.run_ui(egui::RawInput::default(), |ui| super::super::show(app, ui));
        frame(&mut app).textures_delta.clear();
        let text = crate::tests_labels::shapes_text(&frame(&mut app));
        assert!(text.contains("8 biến (1 đã gắn object) · 4 bản ghi: 001, 002, 003, …"), "{text}");
        assert!(text.contains("Chưa gắn: Bien2, Bien3, Bien4 và 4 biến nữa"), "{text}");
        assert!(text.contains("Cảnh báo 3") && !text.contains("Cảnh báo 4") && text.contains("… và 3 cảnh báo nữa"), "{text}");
        assert!(text.contains("Tùy chọn nhập") && !text.contains("DẤU PHÂN CÁCH"), "tùy chọn thu lại sẵn: {text}");
        assert!(!text.contains("OBJECT GẮN"), "không còn bảng biến: {text}");
    }

    /// Mỗi khung hình xuất một bản ghi; có file trùng tên thì không bắt đầu; Dừng và tài liệu bị
    /// sửa thì dừng giữa chừng.
    #[test]
    fn xuat_tung_ban_ghi_moi_khung_hinh() {
        let mut app = app_with_records();
        let ctx = egui::Context::default();
        let dir = out_dir("run");
        let params = |overwrite: bool| json!({"folder": dir.to_string_lossy(), "format": "pdf", "prefix": "Ve ", "overwrite": overwrite});
        start(&mut app, &ctx, params(false), 1, 3);
        assert!(running(&ctx));
        tick(&mut app, &ctx);
        assert_eq!(listing(&dir), ["Ve 001.pdf"]);
        tick(&mut app, &ctx);
        tick(&mut app, &ctx);
        assert!(!running(&ctx));
        assert_eq!(listing(&dir), ["Ve 001.pdf", "Ve 002.pdf", "Ve 003.pdf"]);
        let Note(text, err) = note(&ctx);
        assert!(!err && text.starts_with("Đã xuất 3/3 file PDF."), "{text}");
        // Lần nữa, không ghi đè: không bắt đầu, báo file đã có.
        start(&mut app, &ctx, params(false), 1, 3);
        let Note(text, err) = note(&ctx);
        assert!(!running(&ctx) && err && text.contains("đã có"), "{text}");
        // Ghi đè, bấm Dừng sau bản ghi đầu.
        start(&mut app, &ctx, params(true), 1, 3);
        tick(&mut app, &ctx);
        let job = pstate::<Option<Job>>(&ctx, JOB).unwrap();
        set_pstate(&ctx, JOB, Some(Job { stop: true, ..job }));
        tick(&mut app, &ctx);
        let Note(text, err) = note(&ctx);
        assert!(!running(&ctx) && err && text.starts_with("Đã dừng. Đã xuất 1/3 file PDF."), "{text}");
        // Tài liệu bị sửa giữa chừng: dừng.
        start(&mut app, &ctx, params(true), 2, 3);
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap();
        tick(&mut app, &ctx);
        let Note(text, _) = note(&ctx);
        assert!(!running(&ctx) && text.contains("bị sửa trong lúc xuất") && text.contains("0/2"), "{text}");
        // Chưa chọn thư mục.
        start(&mut app, &ctx, json!({"folder": "", "format": "ai", "prefix": "", "overwrite": false}), 1, 3);
        let Note(text, err) = note(&ctx);
        assert!(!running(&ctx) && err && text.contains("Chưa chọn thư mục"), "{text}");
    }
}
