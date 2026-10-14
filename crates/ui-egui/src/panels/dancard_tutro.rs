//! Mục "Dàn Offset" của tab Dàn tối ưu (DanCard `dcDanTuTro`): tự trở, hai mặt chung tờ, in lật
//! ngang, chạy lệnh `dancard.tuTro`. Như DanCard: kết quả có `[[COUNT:n]]` thì hiện "n con / tờ" chữ
//! đậm, phần còn lại của thông báo ở dòng dưới.

use egui::{RichText, Ui};
use serde_json::json;

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Vùng thông báo của mục này (`super::run_and_note`).
const AREA: &str = "tutro";

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let hint = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    hint(ui, "Tự trở: hai mặt chung tờ, in lật ngang.");
    let (mut width, mut height, mut margin, mut squeeze) = (String::new(), String::new(), String::new(), String::new());
    egui::Grid::new("dancard-tutro").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        hint(ui, "Ngang giấy (cm)");
        hint(ui, "Dọc giấy (cm)");
        ui.end_row();
        width = super::text_box(app, ui, "tutroSheetW", "86", 64.0);
        height = super::text_box(app, ui, "tutroSheetH", "65", 64.0);
        ui.end_row();
        hint(ui, "Lề an toàn (mm)");
        hint(ui, "Bóp tối đa (mm)");
        ui.end_row();
        margin = super::text_box(app, ui, "tutroMargin", "0", 64.0);
        squeeze = super::text_box(app, ui, "tutroMaxBop", "1", 64.0);
        ui.end_row();
    });
    hint(ui, "Xếp từng cặp: trái = trước, phải = sau; hai mặt cùng khung. Bóp 0 mm: giữ kích thước, không PON cắt.");
    hint(ui, "Chỉ xóa cặp đã dàn; cặp chưa vừa được giữ lại.");
    let repeat = super::check_box(app, ui, "tutroRepeat", "Lặp lại 1 cặp cho đầy tờ", false);
    hint(ui, "Chỉ lặp khi chọn đúng một cặp.");
    ui.add_space(6.0);
    let has_selection = app.session.active().is_some_and(|st| !st.selection.objects.is_empty());
    if ui.add_enabled(has_selection, egui::Button::new("Dàn Offset")).clicked() {
        // Như panel DanCard: ô lề / bóp để trống là 0.
        let or_zero = |s: String| if s.is_empty() { "0".to_string() } else { s };
        let params = json!({"width": width, "height": height, "margin": or_zero(margin), "maxSqueeze": or_zero(squeeze), "repeatFill": repeat});
        if let Some(v) = super::run_and_note(app, ui, AREA, "dancard.tuTro", params) {
            // Thanh trạng thái: "n con / tờ" thay cho thẻ [[COUNT:n]].
            let text = v["message"].as_str().unwrap_or_default().trim_start_matches("OK:").trim();
            if let Some((n, rest)) = count_prefix(text) {
                app.status(format!("{n} con / tờ. {}", rest.lines().next().unwrap_or_default()));
            }
        }
    }
    result(ui, &t);
}

/// `[[COUNT:n]] phần còn lại` → (n, phần còn lại), như `showCountResult` của DanCard.
fn count_prefix(s: &str) -> Option<(&str, &str)> {
    let rest = s.strip_prefix("[[COUNT:")?;
    let end = rest.find("]]")?;
    let n = rest.get(..end)?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((n, rest.get(end + 2..)?.trim_start()))
}

/// Kết quả lần dàn gần nhất: "n con / tờ" chữ đậm khi có, lỗi màu đỏ, chưa chạy thì lời nhắc.
fn result(ui: &mut Ui, t: &Tokens) {
    let super::Note(text, err) = crate::panels::pstate::<super::Note>(ui.ctx(), &super::note_id(AREA));
    if !err && let Some((n, rest)) = count_prefix(&text) {
        ui.label(RichText::new(format!("{n} con / tờ")).strong().color(t.text));
        ui.label(RichText::new(rest).color(t.text));
        return;
    }
    super::show_note(ui, AREA, t, "Chọn cặp rồi bấm Dàn Offset.");
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;
    use crate::tests_labels::painted_text;

    #[test]
    fn hien_cac_o_mac_dinh_va_so_con_moi_to() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        let text = painted_text(&mut app, super::section);
        let want = ["Ngang giấy (cm)", "86", "Dọc giấy (cm)", "65", "Lề an toàn (mm)", "Bóp tối đa (mm)", "Lặp lại 1 cặp cho đầy tờ"];
        for s in want.into_iter().chain(["Dàn Offset", "Chọn cặp rồi bấm Dàn Offset."]) {
            assert!(text.contains(s), "{s}: {text}");
        }
        assert_eq!(super::count_prefix("[[COUNT:57]] Sức chứa tờ"), Some(("57", "Sức chứa tờ")));
        assert_eq!(super::count_prefix("[[COUNT:]] x"), None);
        assert_eq!(super::count_prefix("Sức chứa tờ"), None);
    }
}
