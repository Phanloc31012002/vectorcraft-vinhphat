//! Tab "Dàn theo mẫu" của panel Công cụ bình (`pane-danmau` của DanCard): Học mẫu (1 mặt hoặc 2
//! mặt đối xứng, rồi Xác nhận con chuẩn), Áp mẫu theo file pon, Copy ghi chú và Lưu PDF đã dàn.
//!
//! Mẫu đã học được giữ trong `app.ui.dancard["mauTemplate"]` (lưu cùng tùy chọn, mở lại app vẫn
//! còn); lần Học mẫu đang chờ chọn con chuẩn chỉ giữ trong phiên. Học lại mà chưa xác nhận thì mẫu
//! cũ vẫn giữ nguyên.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// Học mẫu đang chờ chọn con chuẩn: kiểu học (one | mirror) và các con mẫu theo thứ tự vị trí.
#[derive(Clone, Default)]
struct Pending {
    mode: String,
    ids: Vec<u64>,
}

const PENDING: &str = "dancard-mau-pending";
const TEMPLATE: &str = "mauTemplate";

/// Mẫu đã học: (2 mặt, số vị trí).
fn template_info(v: Option<&Value>) -> Option<(bool, usize)> {
    let v = v.filter(|v| v.is_object())?;
    let slots = v.get("slots").and_then(Value::as_array).map_or(0, Vec::len);
    Some((v.get("mode").and_then(Value::as_str) == Some("two"), slots))
}

/// Ô chữ một dòng có chữ gợi ý khi trống, gắn với giá trị đã lưu `key`.
fn hinted_field(app: &mut VectorcraftApp, ui: &mut Ui, key: &str, hint: &str, tip: &str) -> String {
    let mut v = super::saved(app, key, "");
    let r = ui.add(egui::TextEdit::singleline(&mut v).hint_text(hint).desired_width(220.0)).on_hover_text(tip);
    if r.changed() {
        super::save(app, key, &v);
    }
    v
}

/// Dòng chọn file pon: tên file đã chọn, nút Chọn… và Bỏ.
fn pon_row(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, key: &'static str, label: &str) {
    ui.label(RichText::new(label).color(t.text_dim));
    let path = super::saved(app, key, "");
    ui.horizontal(|ui| {
        let shown = if path.is_empty() { "(chưa chọn)".to_string() } else { path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string() };
        ui.label(RichText::new(shown).color(t.text)).on_hover_text(&path);
        if ui.button("Chọn…").clicked() {
            crate::picks::button(app, move |app| {
                let pick = crate::FilePick { filters: vec![("File pon", &["ai", "pdf", "eps", "svg", "vectorcraft"])], ..Default::default() };
                if let Some(p) = crate::picks::open(app, &pick) {
                    super::save(app, key, &p);
                }
            });
        }
        if !path.is_empty() && ui.button("Bỏ").clicked() {
            super::save(app, key, "");
        }
    });
}

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    let selected: Vec<u64> = app.session.active().map(|st| st.selection.objects.iter().map(|i| i.0).collect()).unwrap_or_default();
    let has_doc = app.session.active().is_some();
    let has_selection = !selected.is_empty();

    ui.label(RichText::new("Dàn theo mẫu (học bố cục → gán nhiều mẫu nguồn)").strong().color(t.text));
    dim(ui, "1. Chọn bài đã dàn tay → Học mẫu.\n2. Chọn nguồn → Áp mẫu. Hai mặt: trái = trước, phải = sau.");
    ui.add_space(4.0);

    // ---- Học mẫu: chọn loại bố cục (hộp thoại của DanCard), rồi xác nhận con chuẩn.
    dim(ui, "Học mẫu - bấm đúng loại bố cục đang chọn:");
    let mut pending = crate::panels::pstate::<Pending>(ui.ctx(), PENDING);
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [("one", "Học 1 mặt"), ("mirror", "Học 2 mặt - Mặt SAU đối xứng")] {
            if ui.add_enabled(has_selection, egui::Button::new(label)).clicked() {
                pending = Pending::default();
                if let Some(v) = super::run_and_note(app, ui, "mauHoc", "dancard.learnTemplate", json!({"mode": mode})) {
                    let ids = v["ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                    pending = Pending { mode: mode.to_string(), ids };
                }
            }
        }
    });
    if !pending.ids.is_empty() {
        let confirm =
            ui.add_enabled(has_selection, egui::Button::new("Xác nhận con chuẩn")).on_hover_text("Chọn 1 con mẫu cùng chiều với con nguồn rồi bấm.");
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
    super::show_note(ui, "mauHoc", &t, "Chọn bản mẫu rồi bấm Học mẫu.");
    let info = template_info(app.ui.dancard.get(TEMPLATE));
    let status = match info {
        Some((true, n)) => format!("Mẫu đã học: 2 mặt (mặt SAU đối xứng), {n} vị trí."),
        Some((false, n)) => format!("Mẫu đã học: 1 mặt, {n} vị trí."),
        None => "Chưa có mẫu đã học.".to_string(),
    };
    dim(ui, &status);
    let two = info.is_some_and(|(two, _)| two);

    // ---- Áp mẫu
    ui.add_space(6.0);
    let multi = super::check_box(app, ui, "mauMulti", "Dàn nhiều mẫu nguồn khác nhau vào 1 artboard", false);
    dim(ui, "Không tick: mỗi nguồn một artboard. Tick: gộp nguồn; thiếu thì bù từ nguồn cuối.");
    if two {
        pon_row(app, ui, &t, "mauPonF", "Pon MẶT TRƯỚC (.ai)");
        pon_row(app, ui, &t, "mauPonB", "Pon MẶT SAU (.ai)");
    } else {
        pon_row(app, ui, &t, "mauPonF", "File pon (.ai) - artboard cho mỗi con");
    }
    if ui.add_enabled(has_selection, egui::Button::new("Áp mẫu")).clicked() {
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
    super::show_note(ui, "mauAp", &t, "Chọn con nguồn rồi bấm Áp mẫu.");

    // ---- Copy ghi chú
    ui.add_space(6.0);
    egui::CollapsingHeader::new("Copy ghi chú").id_salt("dancard-acc-mau-note").show(ui, |ui| {
        dim(ui, "Chọn ghi chú trên tờ đầu. Để trống ô F1, F2… để copy nguyên.");
        let prefixes =
            hinted_field(app, ui, "mauNotePrefixes", "F1, F2, F3...", "F1, F2… theo thứ tự tờ; ngăn bằng dấu phẩy. Để trống để copy nguyên ghi chú.");
        ui.horizontal_wrapped(|ui| {
            let buttons = [
                (false, "Điền ghi chú bài 1 mặt", "Chọn ghi chú trên tờ đầu; điền lần lượt artboard 1, 2, 3…"),
                (true, "Điền ghi chú bài 2 mặt", "Chọn ghi chú trên mặt trước đầu tiên; điền artboard 1, 3, 5…"),
            ];
            for (two_sided, label, tip) in buttons {
                if ui.add_enabled(has_selection, egui::Button::new(label)).on_hover_text(tip).clicked() {
                    super::run_and_note(app, ui, "mauNote", "dancard.copyNotes", json!({"prefixes": prefixes, "twoSided": two_sided}));
                }
            }
        });
        super::show_note(ui, "mauNote", &t, "Chọn ghi chú rồi bấm Điền.");
    });

    // ---- Lưu PDF đã dàn
    egui::CollapsingHeader::new("Lưu PDF đã dàn").id_salt("dancard-acc-mau-pdf").show(ui, |ui| {
        dim(ui, "Cách lưu");
        let modes = [("each", "Từng artboard - 1 PDF / artboard"), ("pair", "Mặt trước + mặt sau - 1 PDF / 2 artboard")];
        let mode = super::saved(app, "mauSaveMode", "each");
        let current = modes.iter().find(|m| m.0 == mode).or(modes.first()).map_or("", |m| m.1);
        let labels: Vec<&str> = modes.iter().map(|m| m.1).collect();
        if let Some(i) = widgets::dropdown_names(ui, "dancard-mau-save-mode", current, &labels, 260.0)
            && let Some((key, _)) = modes.get(i)
        {
            super::save(app, "mauSaveMode", key);
        }
        dim(ui, "Nội dung sau file 1, file 2...");
        let suffix = hinted_field(app, ui, "mauSaveSuffix", "vd: thiep sinh nhat", "Tên file: file 1 - <nội dung>.pdf, file 2 - <nội dung>.pdf…");
        dim(ui, "Thư mục lưu");
        let folder = super::saved(app, "mauSaveFolder", "");
        ui.horizontal(|ui| {
            let shown = if folder.is_empty() { "(chưa chọn)".to_string() } else { folder.clone() };
            ui.label(RichText::new(shown).color(t.text));
            if ui.button("Chọn…").clicked() {
                crate::picks::button(app, |app| {
                    if let Some(f) = crate::picks::folder(app) {
                        super::save(app, "mauSaveFolder", &f);
                    }
                });
            }
        });
        if ui.add_enabled(has_doc, egui::Button::new("Lưu PDF")).clicked() {
            let params = json!({"mode": super::saved(app, "mauSaveMode", "each"), "suffix": suffix, "folder": folder});
            super::run_and_note(app, ui, "mauPdf", "dancard.saveTemplatePdf", params);
        }
        super::show_note(ui, "mauPdf", &t, "Áp mẫu xong rồi bấm Lưu PDF.");
    });
}
