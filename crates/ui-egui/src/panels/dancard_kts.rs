//! Panel Dàn tối ưu › Dàn KTS (lệnh `dancard.ktsLayout`, `dcDanToiUu` của DanCard): khổ giấy,
//! dàn 2 mặt đối xứng, dàn nhiều mẫu vào một tờ. Kết quả "[[COUNT:n]] …" hiện dòng đậm
//! "n con / tờ" rồi tới ghi chú, như `showCountResult` của DanCard. Khi số vị trí không chia đều
//! cho số mẫu, panel hỏi "Dừng lại" / "Tiếp tục" như hộp thoại "Dàn nhiều loại".

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{Note, check_box, count_prefix, note_id, run_and_note, text_box};
use crate::VectorcraftApp;
use crate::panels::{pstate, set_pstate};
use crate::theme::Tokens;

const AREA: &str = "kts";
const COMMAND: &str = "dancard.ktsLayout";
/// Câu hỏi "không chia đều" đang chờ trả lời (rỗng: không có).
const ASK: &str = "dancard-kts-ask";

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let hint = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    ui.label(RichText::new("Dàn tối ưu không pon").strong().color(t.text));
    hint(ui, "Chọn các bài, nhập khổ giấy rồi dàn.");
    let (width, height) = egui::Grid::new("dancard-kts-paper")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Ngang giấy (cm)").color(t.text_dim));
            ui.label(RichText::new("Dọc giấy (cm)").color(t.text_dim));
            ui.end_row();
            let w = text_box(app, ui, "ktsSheetW", "33", 80.0);
            let h = text_box(app, ui, "ktsSheetH", "35.4", 80.0);
            ui.end_row();
            (w, h)
        })
        .inner;
    let two_sided = check_box(app, ui, "ktsTwoSided", "Dàn 2 mặt đối xứng", false);
    hint(ui, "Xếp theo hàng: trái = trước, phải = sau.");
    let multi = check_box(app, ui, "ktsMulti", "Dàn nhiều mẫu vào một tờ", false);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Tự nhân bản tối đa con lớn trước, rồi nhân bản con nhỏ lấp phần còn lại.").strong().color(t.text_dim));
        ui.label(RichText::new("Mẫu đã dàn được xóa khỏi nguồn; mẫu chưa dàn được giữ lại.").color(t.text_dim));
    });
    hint(ui, "Lề 3 mm, khe 0 mm; không PON.");
    ui.add_space(6.0);
    let params = |uneven: Option<bool>| {
        let mut p = json!({"width": width, "height": height, "twoSided": two_sided, "multiPerArtboard": multi});
        if let Some(go) = uneven {
            p["continueUneven"] = json!(go);
        }
        p
    };
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    let button = egui::Button::new("Dàn tối ưu").min_size(egui::vec2(ui.available_width(), 0.0));
    if ui.add_enabled(has_selection, button).clicked() {
        set_pstate(ui.ctx(), ASK, String::new());
        run(app, ui, params(None));
    }
    let question: String = pstate(ui.ctx(), ASK);
    if !question.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("Dàn nhiều loại").strong().color(t.text));
        ui.label(RichText::new(question.as_str()).color(t.text));
        let mut answer = None;
        ui.horizontal(|ui| {
            if ui.button("Dừng lại").clicked() {
                answer = Some(false);
            }
            if ui.button("Tiếp tục").clicked() {
                answer = Some(true);
            }
        });
        let Some(go) = answer else { return };
        set_pstate(ui.ctx(), ASK, String::new());
        run(app, ui, params(Some(go)));
    }
    result(ui, &t);
}

/// Chạy lệnh. Câu hỏi "không chia đều" (lệnh dừng, chưa dàn gì) thành câu hỏi của panel thay
/// cho lỗi; dàn xong thì đưa khung nhìn tới tờ đầu tiên vừa tạo, như DanCard chọn artboard đó.
fn run(app: &mut VectorcraftApp, ui: &Ui, params: Value) {
    let done = run_and_note(app, ui, AREA, COMMAND, params).is_some();
    let Note(text, err) = pstate::<Note>(ui.ctx(), &note_id(AREA));
    if err && text.contains("không chia đều cho") && text.contains("Nếu tiếp tục") {
        set_pstate(ui.ctx(), ASK, text);
        set_pstate(ui.ctx(), &note_id(AREA), Note::default());
        return;
    }
    if !done {
        return;
    }
    if let Some((n, rest)) = count_prefix(&text) {
        app.status(format!("{n} con / tờ. {rest}"));
    }
    // Chỉ là khung nhìn: không tới được (chưa có cửa sổ vẽ) thì bỏ qua.
    let _ = app.run("view.goToArtboard", json!({"index": 0}));
}

/// Ô kết quả: "n con / tờ" đậm rồi ghi chú; lỗi và thông báo khác như các tab khác.
fn result(ui: &mut Ui, t: &Tokens) {
    super::show_count_note(ui, AREA, t, "Chọn một bài rồi bấm Dàn tối ưu.");
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::tests_labels::painted_text;

    #[test]
    fn doc_so_con_moi_to() {
        assert_eq!(count_prefix("[[COUNT:20]] Đã tạo 1 artboard"), Some(("20", "Đã tạo 1 artboard")));
        assert_eq!(count_prefix("[[COUNT:]] x"), None);
        assert_eq!(count_prefix("[[COUNT:2a]] x"), None);
        assert_eq!(count_prefix("Đã dừng dàn"), None);
    }

    /// Các ô như DanCard; kết quả "[[COUNT:n]]" thành dòng "n con / tờ"; câu hỏi chia không đều.
    #[test]
    fn o_nhap_va_ket_qua_nhu_dancard() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        let text = painted_text(&mut app, section);
        for s in [
            "Dàn tối ưu không pon",
            "Ngang giấy (cm)",
            "Dọc giấy (cm)",
            "35.4",
            "Dàn 2 mặt đối xứng",
            "Xếp theo hàng: trái = trước, phải = sau.",
            "Dàn nhiều mẫu vào một tờ",
            "Lề 3 mm, khe 0 mm; không PON.",
            "Chọn một bài rồi bấm Dàn tối ưu.",
        ] {
            assert!(text.contains(s), "{s}: {text}");
        }
        let text = painted_text(&mut app, |app, ui| {
            set_pstate(ui.ctx(), &note_id(AREA), Note("[[COUNT:20]] Đã tạo 1 artboard".into(), false));
            section(app, ui);
        });
        assert!(text.contains("20 con / tờ") && text.contains("Đã tạo 1 artboard") && !text.contains("[[COUNT"), "{text}");
        let text = painted_text(&mut app, |app, ui| {
            set_pstate(ui.ctx(), ASK, "20 vị trí trên 1 tờ không chia đều cho 3 mẫu.".to_string());
            section(app, ui);
        });
        assert!(text.contains("Dàn nhiều loại") && text.contains("Dừng lại") && text.contains("Tiếp tục"), "{text}");
    }
}
