//! Mục "Dàn Offset" của tab Dàn tối ưu (DanCard `dcDanTuTro`): tự trở, hai mặt chung tờ, in lật
//! ngang, chạy lệnh `dancard.tuTro`. Như DanCard: kết quả có `[[COUNT:n]]` thì hiện "n con / tờ" chữ
//! đậm, phần còn lại của thông báo ở dòng dưới.

use egui::Ui;
use serde_json::json;

use super::{check_box, count_prefix, num_fields, style};
use crate::VectorcraftApp;

/// Vùng thông báo của mục này (`super::run_and_note`).
const AREA: &str = "tutro";

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::hint(ui, "Tự trở: hai mặt chung tờ, in lật ngang.");
    let [width, height] = num_fields(app, ui, [("Ngang giấy (cm)", "tutroSheetW", "86"), ("Dọc giấy (cm)", "tutroSheetH", "65")]);
    let [margin, squeeze] = num_fields(app, ui, [("Lề an toàn (mm)", "tutroMargin", "0"), ("Bóp tối đa (mm)", "tutroMaxBop", "1")]);
    style::hint(ui, "Xếp từng cặp: trái = trước, phải = sau; hai mặt cùng khung. Bóp 0 mm: giữ kích thước, không PON cắt.");
    style::hint(ui, "Chỉ xóa cặp đã dàn; cặp chưa vừa được giữ lại.");
    let repeat = check_box(app, ui, "tutroRepeat", "Lặp lại 1 cặp cho đầy tờ", false);
    style::hint(ui, "Chỉ lặp khi chọn đúng một cặp.");
    if style::primary(ui, "Dàn Offset", super::has_selection(app)).clicked() {
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
    super::show_count_note(ui, AREA, "Chọn cặp rồi bấm Dàn Offset.");
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
        let want = ["NGANG GIẤY (CM)", "86", "DỌC GIẤY (CM)", "65", "LỀ AN TOÀN (MM)", "BÓP TỐI ĐA (MM)", "Lặp lại 1 cặp cho đầy tờ"];
        for s in want.into_iter().chain(["Dàn Offset", "Chọn cặp rồi bấm Dàn Offset."]) {
            assert!(text.contains(s), "{s}: {text}");
        }
        assert_eq!(super::count_prefix("[[COUNT:57]] Sức chứa tờ"), Some(("57", "Sức chứa tờ")));
        assert_eq!(super::count_prefix("[[COUNT:]] x"), None);
        assert_eq!(super::count_prefix("Sức chứa tờ"), None);
        let text = painted_text(&mut app, |app, ui| {
            crate::panels::set_pstate(ui.ctx(), &super::super::note_id(super::AREA), super::super::Note("[[COUNT:57]] Sức chứa tờ".into(), false));
            super::section(app, ui);
        });
        assert!(text.contains("57 con / tờ") && text.contains("Sức chứa tờ") && !text.contains("[[COUNT"), "{text}");
    }
}
