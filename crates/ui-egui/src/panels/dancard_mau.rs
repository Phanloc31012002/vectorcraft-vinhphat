//! Tab "Dàn theo mẫu" của panel Công cụ bình (`pane-danmau` của DanCard): Học mẫu (1 mặt hoặc 2
//! mặt đối xứng, rồi Xác nhận con chuẩn), Áp mẫu theo file pon, Copy ghi chú và Lưu PDF đã dàn.
//!
//! Mẫu đã học được giữ trong `app.ui.dancard["mauTemplate"]` (lưu cùng tùy chọn, mở lại app vẫn
//! còn); lần Học mẫu đang chờ chọn con chuẩn chỉ giữ trong phiên. Học lại mà chưa xác nhận thì mẫu
//! cũ vẫn giữ nguyên.

use egui::Ui;
use serde_json::{Value, json};

use super::{accordion, check_box, choice, file_row, folder_row, style, text_box};
use crate::VectorcraftApp;

/// Học mẫu đang chờ chọn con chuẩn: kiểu học (one | mirror) và các con mẫu theo thứ tự vị trí.
#[derive(Clone, Default)]
struct Pending {
    mode: String,
    ids: Vec<u64>,
}

const PENDING: &str = "dancard-mau-pending";
const TEMPLATE: &str = "mauTemplate";
/// File pon nhận khi áp mẫu.
const PON: (&str, &[&str]) = ("File pon", &["ai", "pdf", "eps", "svg", "vectorcraft"]);

/// Mẫu đã học: (2 mặt, số vị trí).
fn template_info(v: Option<&Value>) -> Option<(bool, usize)> {
    let v = v.filter(|v| v.is_object())?;
    let slots = v.get("slots").and_then(Value::as_array).map_or(0, Vec::len);
    Some((v.get("mode").and_then(Value::as_str) == Some("two"), slots))
}

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let selected: Vec<u64> = app.session.active().map(|st| st.selection.objects.iter().map(|i| i.0).collect()).unwrap_or_default();
    let has_doc = app.session.active().is_some();
    let has_selection = !selected.is_empty();

    style::label(ui, "Dàn theo mẫu (học bố cục → gán nhiều mẫu nguồn)");
    style::hint(ui, "1. Chọn bài đã dàn tay → Học mẫu.\n2. Chọn nguồn → Áp mẫu. Hai mặt: trái = trước, phải = sau.");

    // ---- Học mẫu: chọn loại bố cục (hộp thoại của DanCard), rồi xác nhận con chuẩn.
    style::label(ui, "Học mẫu - bấm đúng loại bố cục đang chọn");
    let mut pending = crate::panels::pstate::<Pending>(ui.ctx(), PENDING);
    for (mode, label) in [("one", "Học 1 mặt"), ("mirror", "Học 2 mặt - Mặt SAU đối xứng")] {
        if style::primary(ui, label, has_selection).clicked() {
            pending = Pending::default();
            if let Some(v) = super::run_and_note(app, ui, "mauHoc", "dancard.learnTemplate", json!({"mode": mode})) {
                let ids = v["ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                pending = Pending { mode: mode.to_string(), ids };
            }
        }
    }
    if !pending.ids.is_empty() {
        let confirm = style::button(ui, "Xác nhận con chuẩn", has_selection, true).on_hover_text("Chọn 1 con mẫu cùng chiều với con nguồn rồi bấm.");
        if confirm.clicked() {
            if let [one] = selected.as_slice() {
                let params = json!({"mode": pending.mode, "ids": pending.ids, "reference": one});
                if let Some(v) = super::run_and_note(app, ui, "mauHoc", "dancard.learnTemplate", params)
                    && v["pending"].as_bool() == Some(false)
                {
                    app.ui.dancard.insert(TEMPLATE.into(), v["template"].clone());
                    pending = Pending::default();
                }
            } else {
                crate::panels::set_pstate(
                    ui.ctx(),
                    &super::note_id("mauHoc"),
                    super::Note("Hãy chọn đúng 1 con mẫu cùng chiều với con nguồn.".into(), true),
                );
            }
        }
    }
    crate::panels::set_pstate(ui.ctx(), PENDING, pending);
    super::show_note(ui, "mauHoc", "Chọn bản mẫu rồi bấm Học mẫu.");
    let info = template_info(app.ui.dancard.get(TEMPLATE));
    let status = match info {
        Some((true, n)) => format!("Mẫu đã học: 2 mặt (mặt SAU đối xứng), {n} vị trí."),
        Some((false, n)) => format!("Mẫu đã học: 1 mặt, {n} vị trí."),
        None => "Chưa có mẫu đã học.".to_string(),
    };
    style::hint_strong(ui, &status);
    let two = info.is_some_and(|(two, _)| two);

    // ---- Áp mẫu
    let multi = check_box(app, ui, "mauMulti", "Dàn nhiều mẫu nguồn khác nhau vào 1 artboard", false);
    style::hint(ui, "Không tick: mỗi nguồn một artboard. Tick: gộp nguồn; thiếu thì bù từ nguồn cuối.");
    if two {
        style::label(ui, "Pon MẶT TRƯỚC (.ai)");
        file_row(app, ui, "mauPonF", "Chưa chọn", PON);
        style::label(ui, "Pon MẶT SAU (.ai)");
        file_row(app, ui, "mauPonB", "Chưa chọn", PON);
    } else {
        style::label(ui, "File pon (.ai) - artboard cho mỗi con");
        file_row(app, ui, "mauPonF", "Chưa chọn", PON);
    }
    if style::primary(ui, "Áp mẫu", has_selection).clicked() {
        let template = app.ui.dancard.get(TEMPLATE).cloned().unwrap_or(Value::Null);
        let mut params = json!({"template": template, "multiSourcePerArtboard": multi, "ponFront": super::saved(app, "mauPonF", "")});
        if two {
            params["ponBack"] = json!(super::saved(app, "mauPonB", ""));
        }
        if let Some(v) = super::run_and_note(app, ui, "mauAp", "dancard.applyTemplate", params)
            && let Some(i) = v["firstArtboard"].as_u64()
        {
            // Các tờ mới nằm ở góc canvas: đưa tờ đầu vào cửa sổ. Không được thì thôi, kết quả vẫn đúng.
            let _ = app.run("view.goToArtboard", json!({"index": i}));
        }
    }
    super::show_note(ui, "mauAp", "Chọn con nguồn rồi bấm Áp mẫu.");

    // ---- Copy ghi chú
    accordion(app, ui, "mauNote", "Copy ghi chú", false, |app, ui| {
        style::hint(ui, "Chọn ghi chú trên tờ đầu. Để trống ô F1, F2… để copy nguyên.");
        let prefixes = text_box(app, ui, "mauNotePrefixes", "", "F1, F2, F3...");
        let buttons = [
            (false, "Điền ghi chú bài 1 mặt", "Chọn ghi chú trên tờ đầu; điền lần lượt artboard 1, 2, 3…"),
            (true, "Điền ghi chú bài 2 mặt", "Chọn ghi chú trên mặt trước đầu tiên; điền artboard 1, 3, 5…"),
        ];
        let mut run = None;
        style::columns(ui, 2, |ui, i| {
            if let Some((two_sided, label, tip)) = buttons.get(i)
                && style::button(ui, label, has_selection, true).on_hover_text(*tip).clicked()
            {
                run = Some(*two_sided);
            }
        });
        if let Some(two_sided) = run {
            super::run_and_note(app, ui, "mauNote", "dancard.copyNotes", json!({"prefixes": prefixes, "twoSided": two_sided}));
        }
        super::show_note(ui, "mauNote", "Chọn ghi chú rồi bấm Điền.");
    });

    // ---- Lưu PDF đã dàn
    accordion(app, ui, "mauPdf", "Lưu PDF đã dàn", false, |app, ui| {
        style::label(ui, "Cách lưu");
        let modes = [("each", "Từng artboard - 1 PDF / artboard"), ("pair", "Mặt trước + mặt sau - 1 PDF / 2 artboard")];
        let mode = choice(app, ui, "mauSaveMode", &modes, "each", true);
        style::label(ui, "Nội dung sau file 1, file 2...");
        let suffix = text_box(app, ui, "mauSaveSuffix", "", "vd: thiep sinh nhat");
        style::label(ui, "Thư mục lưu");
        let folder = folder_row(app, ui, "mauSaveFolder", "");
        if style::primary(ui, "Lưu PDF", has_doc).clicked() {
            let params = json!({"mode": mode, "suffix": suffix, "folder": folder});
            super::run_and_note(app, ui, "mauPdf", "dancard.saveTemplatePdf", params);
        }
        super::show_note(ui, "mauPdf", "Áp mẫu xong rồi bấm Lưu PDF.");
    });
}
