//! Tab "CTL Offset" của panel Công cụ bình (DanCard `pane-offset`): Đóng ghim giữa, Keo gáy, rồi
//! Lưu AI + JPG + ZIP theo khổ vừa dàn (`dancard.saddleStitch`, `dancard.perfectBind`,
//! `dancard.saveCtlPackage`). Ghi chú RUỘT N lấy từ các ô của tab thay cho hộp thoại ghi chú.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{Note, check_box, note_id, run_and_note, save, saved, show_note, text_box};
use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Kế hoạch lưu theo khổ của lần dàn gần nhất (`sizes` của lệnh dàn).
const SIZES_KEY: &str = "ctlSizes";
/// Thư mục lưu AI + JPG + ZIP.
const FOLDER_KEY: &str = "ctlFolder";

/// `parseFloat` của panel DanCard sau khi đổi dấu phẩy đầu tiên thành dấu chấm: số ở đầu chuỗi.
fn parse_float(s: &str) -> Option<f64> {
    let s = s.replacen(',', ".", 1);
    let t = s.trim_start();
    let run = t.char_indices().find(|(_, c)| !(c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))).map_or(t.len(), |(i, _)| i);
    (1..=run).rev().find_map(|n| t.get(..n).and_then(|h| h.parse::<f64>().ok())).filter(|v| v.is_finite())
}

fn hint(ui: &mut Ui, t: &Tokens, s: &str) {
    ui.label(RichText::new(s).color(t.text_dim));
}

/// Hai ô rộng × cao (cm) trên một hàng.
fn size_row(app: &mut VectorcraftApp, ui: &mut Ui, w_key: &str, h_key: &str) -> (String, String) {
    ui.horizontal(|ui| {
        let w = text_box(app, ui, w_key, "", 56.0);
        ui.label("×");
        let h = text_box(app, ui, h_key, "", 56.0);
        (w, h)
    })
    .inner
}

/// Chạy lệnh dàn `id` với khổ `w` × `h` (cm) và các tham số `extra`; giữ kế hoạch lưu theo khổ.
fn impose(app: &mut VectorcraftApp, ui: &Ui, id: &str, w: &str, h: &str, extra: Value) {
    let (Some(pw), Some(ph)) = (parse_float(w).filter(|v| *v > 0.0), parse_float(h).filter(|v| *v > 0.0)) else {
        crate::panels::set_pstate(ui.ctx(), &note_id("ctl"), Note("Nhập kích thước hợp lệ (rộng × cao, cm).".into(), true));
        return;
    };
    let mut params = extra;
    if let Some(o) = params.as_object_mut() {
        o.insert("width".into(), json!(pw));
        o.insert("height".into(), json!(ph));
    }
    match run_and_note(app, ui, "ctl", id, params) {
        Some(v) if v["sizes"]["groups"].as_array().is_some_and(|g| !g.is_empty()) => {
            app.ui.dancard.insert(SIZES_KEY.into(), v["sizes"].clone());
        }
        _ => {
            app.ui.dancard.remove(SIZES_KEY);
        }
    }
}

fn saddle(app: &mut VectorcraftApp, ui: &mut Ui, enabled: bool) {
    let t = Tokens::get(ui.ctx());
    hint(ui, &t, "Số trang chia hết cho 4. Chỉ bóp ruột, giữ nguyên PON.");
    hint(ui, &t, "Kích thước 1 trang (cm)");
    let (w, h) = size_row(app, ui, "ctlSigW", "ctlSigH");
    let cover = check_box(app, ui, "ctlSigCover", "Có bìa (chọn cả file: bìa + ruột)", true);
    let notes = check_box(app, ui, "ctlSigNotes", "Ghi chú RUỘT N (mặt A mỗi tờ)", true);
    let (mut cover_note, mut inside) = (String::new(), String::new());
    if notes {
        if cover {
            hint(ui, &t, "Ghi chú BÌA (không tự thêm chữ):");
            cover_note = text_box(app, ui, "ctlSigCoverNote", "", 220.0);
        }
        hint(ui, &t, "Ghi chú ruột (thêm sau RUỘT N, chữ đỏ):");
        inside = text_box(app, ui, "ctlSigInside", "", 220.0);
    }
    if ui.add_enabled(enabled, egui::Button::new("Dàn Đóng ghim giữa")).clicked() {
        let extra = json!({"cover": cover, "notes": notes, "coverNote": cover_note, "insideNote": inside});
        impose(app, ui, "dancard.saddleStitch", &w, &h, extra);
    }
}

fn perfect(app: &mut VectorcraftApp, ui: &mut Ui, enabled: bool) {
    let t = Tokens::get(ui.ctx());
    hint(
        ui,
        &t,
        "Khớp rộng × cao theo số nhập. AB 85,8 × 62,5 cm: A4 16 trang, A5 32 trang/tờ. A4 tối đa 21,2 × 30 cm; tờ nhỏ thu về cạnh 20,9 cm. Bìa dàn tay.",
    );
    hint(ui, &t, "Kích thước 1 trang (cm)");
    let (w, h) = size_row(app, ui, "ctlKeoW", "ctlKeoH");
    let notes = check_box(app, ui, "ctlKeoNotes", "Ghi chú RUỘT N (mặt A mỗi tờ)", true);
    let mut inside = String::new();
    if notes {
        hint(ui, &t, "Ghi chú ruột (thêm sau RUỘT N, chữ đỏ):");
        inside = text_box(app, ui, "ctlKeoInside", "", 220.0);
    }
    if ui.add_enabled(enabled, egui::Button::new("Dàn Keo gáy")).clicked() {
        impose(app, ui, "dancard.perfectBind", &w, &h, json!({"notes": notes, "insideNote": inside}));
    }
}

/// Số cm của một cạnh (pt) như nút của DanCard: làm tròn 0,1 cm.
fn cm(pt: f64) -> String {
    ((pt / 2.834645669).round() / 10.0).to_string()
}

/// Chữ trên nút lưu của một nhóm khổ: "Lưu AI + JPG + ZIP Tờ AB — 85.8 × 62.5 cm (2 file)".
fn button_text(group: &Value) -> Option<String> {
    let jobs = group["jobs"].as_array()?;
    let first = jobs.first()?;
    let label = group["label"].as_str().or_else(|| group["key"].as_str()).unwrap_or("");
    let (w, h) = (first["w"].as_f64()?, first["h"].as_f64()?);
    let count = if jobs.len() > 1 { format!(" ({} file)", jobs.len()) } else { String::new() };
    Some(format!("Lưu AI + JPG + ZIP {label} — {} × {} cm{count}", cm(w), cm(h)))
}

/// Chạy `dancard.saveCtlPackage` với `params` vào thư mục `folder`; thông báo dưới vùng lưu.
fn save_package(app: &mut VectorcraftApp, ctx: &egui::Context, params: &Value, folder: &str) {
    let mut p = params.clone();
    if let Some(o) = p.as_object_mut() {
        o.insert("folder".into(), json!(folder));
    }
    let (text, err) = match app.run("dancard.saveCtlPackage", p) {
        Ok(v) => (v["message"].as_str().unwrap_or("Xong.").to_string(), false),
        Err(e) => (e, true),
    };
    let text = text.trim_start_matches("OK:").trim_start_matches("ERR:").trim().to_string();
    app.status(text.lines().next().unwrap_or_default().to_string());
    crate::panels::set_pstate(ctx, &note_id("ctlSave"), Note(text, err));
}

/// Vùng "Lưu AI + JPG + ZIP theo khổ vừa dàn": hiện sau một lần dàn thành công.
fn save_box(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(sizes) = app.ui.dancard.get(SIZES_KEY).cloned() else { return };
    let buttons: Vec<(String, Value)> =
        sizes["groups"].as_array().into_iter().flatten().filter_map(|g| Some((button_text(g)?, g["jobs"].clone()))).collect();
    if buttons.is_empty() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.separator();
    ui.label(RichText::new("Lưu AI + JPG + ZIP theo khổ vừa dàn").strong().color(t.text));
    hint(ui, &t, "Mỗi tờ một bộ; A/B chung một JPG và ZIP.");
    hint(ui, &t, "Nội dung sau RUỘT N / BÌA");
    let suffix = text_box(app, ui, "ctlSuffix", "", 220.0);
    hint(ui, &t, "VD: CATALOGUE KMC -- IN 1.100 tờ…");
    hint(ui, &t, "Thư mục lưu");
    let folder = saved(app, FOLDER_KEY, "");
    ui.horizontal(|ui| {
        let shown = if folder.is_empty() { "(chưa chọn: hỏi khi lưu)".to_string() } else { folder.clone() };
        ui.label(RichText::new(shown).color(t.text));
        if ui.button("Chọn…").clicked() {
            crate::picks::button(app, |app| {
                if let Some(f) = crate::picks::folder(app) {
                    save(app, FOLDER_KEY, &f);
                }
            });
        }
    });
    let total = sizes["total"].clone();
    for (text, jobs) in buttons {
        if !ui.button(text).clicked() {
            continue;
        }
        let params = json!({"total": total, "jobs": jobs, "suffix": suffix});
        let folder = saved(app, FOLDER_KEY, "");
        if folder.is_empty() {
            // Chưa có thư mục: chọn rồi lưu luôn (hộp chọn có thể trả lời sau).
            let ctx = ui.ctx().clone();
            crate::picks::button(app, move |app| {
                if let Some(f) = crate::picks::folder(app) {
                    save(app, FOLDER_KEY, &f);
                    save_package(app, &ctx, &params, &f);
                }
            });
        } else {
            save_package(app, ui.ctx(), &params, &folder);
        }
    }
    show_note(ui, "ctlSave", &t, "Bấm nút của khổ cần lưu.");
}

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new("Dàn CTL Offset (bấm ghim giữa, tự chọn khổ)").strong().color(t.text));
    hint(ui, &t, "Chọn trang theo thứ tự đọc; nhập khổ một trang.");
    let enabled = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    egui::CollapsingHeader::new("Đóng ghim giữa").id_salt("dancard-ctl-sig").show(ui, |ui| saddle(app, ui, enabled));
    egui::CollapsingHeader::new("Keo gáy").id_salt("dancard-ctl-keo").show(ui, |ui| perfect(app, ui, enabled));
    show_note(ui, "ctl", &t, "Nhập kích thước rồi bấm Dàn.");
    save_box(app, ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_nhu_parse_float_cua_panel() {
        assert_eq!(parse_float("20,5"), Some(20.5));
        assert_eq!(parse_float(" 21.15cm"), Some(21.15));
        assert_eq!(parse_float("1,5,5"), Some(1.5));
        assert_eq!(parse_float("abc"), None);
        assert_eq!(parse_float(""), None);
        assert_eq!(cm(1836.85), "64.8");
        assert_eq!(cm(1771.65), "62.5");
        let g = json!({"key": "AB", "label": "Tờ AB", "jobs": [{"name": "RUOT 2", "ab": [2, 3], "w": 2432.13, "h": 1771.65}, {"name": "RUOT 3", "ab": [4, 5], "w": 2432.13, "h": 1771.65}]});
        assert_eq!(button_text(&g).unwrap(), "Lưu AI + JPG + ZIP Tờ AB — 85.8 × 62.5 cm (2 file)");
    }
}
