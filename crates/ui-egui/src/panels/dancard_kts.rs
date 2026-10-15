//! Panel Dàn tối ưu › Dàn KTS (lệnh `dancard.ktsLayout`, `dcDanToiUu` của DanCard): khổ giấy,
//! dàn 2 mặt đối xứng, dàn nhiều mẫu vào một tờ. Kết quả "[[COUNT:n]] …" hiện dòng đậm
//! "n con / tờ" rồi tới ghi chú, như `showCountResult` của DanCard. Khi số vị trí không chia đều
//! cho số mẫu, panel hỏi "Dừng lại" / "Tiếp tục" như hộp thoại "Dàn nhiều loại".

use egui::Ui;
use serde_json::{Value, json};

use super::{Note, check_box, count_prefix, note_id, num_fields, run_and_note, style};
use crate::VectorcraftApp;
use crate::panels::{pstate, set_pstate};

const AREA: &str = "kts";
const COMMAND: &str = "dancard.ktsLayout";
/// Câu hỏi "không chia đều" đang chờ trả lời (rỗng: không có).
const ASK: &str = "dancard-kts-ask";

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Dàn tối ưu không pon");
    style::hint(ui, "Chọn các bài, nhập khổ giấy rồi dàn.");
    let [width, height] = num_fields(app, ui, [("Ngang giấy (cm)", "ktsSheetW", "33"), ("Dọc giấy (cm)", "ktsSheetH", "35.4")]);
    let two_sided = check_box(app, ui, "ktsTwoSided", "Dàn 2 mặt đối xứng", false);
    style::hint(ui, "Xếp theo hàng: trái = trước, phải = sau.");
    let multi = check_box(app, ui, "ktsMulti", "Dàn nhiều mẫu vào một tờ", false);
    style::hint_strong(ui, "Tự nhân bản tối đa con lớn trước, rồi nhân bản con nhỏ lấp phần còn lại.");
    style::hint(ui, "Mẫu đã dàn được xóa khỏi nguồn; mẫu chưa dàn được giữ lại.");
    style::hint(ui, "Lề 3 mm, khe 0 mm; không PON.");
    let params = |uneven: Option<bool>| {
        let mut p = json!({"width": width, "height": height, "twoSided": two_sided, "multiPerArtboard": multi});
        if let Some(go) = uneven {
            p["continueUneven"] = json!(go);
        }
        p
    };
    if style::primary(ui, "Dàn tối ưu", super::has_selection(app)).clicked() {
        set_pstate(ui.ctx(), ASK, String::new());
        run(app, ui, params(None));
    }
    let question: String = pstate(ui.ctx(), ASK);
    if !question.is_empty() {
        style::title(ui, "Dàn nhiều loại");
        style::text(ui, question.as_str());
        let mut answer = None;
        style::columns(ui, 2, |ui, i| {
            let (label, go) = if i == 0 { ("Dừng lại", false) } else { ("Tiếp tục", true) };
            if style::button(ui, label, true, true).clicked() {
                answer = Some(go);
            }
        });
        if let Some(go) = answer {
            set_pstate(ui.ctx(), ASK, String::new());
            run(app, ui, params(Some(go)));
        }
    }
    super::show_count_note(ui, AREA, "Chọn một bài rồi bấm Dàn tối ưu.");
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
            "DÀN TỐI ƯU KHÔNG PON",
            "NGANG GIẤY (CM)",
            "DỌC GIẤY (CM)",
            "35.4",
            "Dàn 2 mặt đối xứng",
            "Xếp theo hàng: trái = trước, phải = sau.",
            "Dàn nhiều mẫu vào một tờ",
            "Lề 3 mm, khe 0 mm; không PON.",
            "Dàn tối ưu",
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
