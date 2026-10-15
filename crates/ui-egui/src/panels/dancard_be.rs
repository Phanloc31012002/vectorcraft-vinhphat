//! Mục Dàn bế của tab Dàn tối ưu (DanCard): dàn decal / tag theo khuôn bế, tự vẽ bốn chấm PON
//! tròn 5 mm theo khổ giấy nhập (`dancard.dieLayout`). Ô, mặc định và gợi ý như panel DanCard.

use egui::{RichText, Ui};
use serde_json::json;

use crate::VectorcraftApp;
use crate::theme::Tokens;

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    ui.label(RichText::new("Dàn decal / tag theo khuôn bế").strong().color(t.text));
    dim(ui, "Chọn từng mẫu: Khuôn → Bài; hai mặt: Khuôn → Trước → Sau.");

    let (mut paper_w, mut paper_h) = (String::new(), String::new());
    egui::Grid::new("dancard-be-paper").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dim(ui, "Ngang giấy (cm)");
        dim(ui, "Cao giấy (cm)");
        ui.end_row();
        paper_w = super::text_box(app, ui, "bePaperW", "33", 72.0);
        paper_h = super::text_box(app, ui, "bePaperH", "35.4", 72.0);
        ui.end_row();
    });

    let mut pon = [String::new(), String::new(), String::new(), String::new()];
    egui::Grid::new("dancard-be-pon").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        for s in ["Trên (mm)", "Dưới (mm)", "Trái (mm)", "Phải (mm)"] {
            dim(ui, s);
        }
        ui.end_row();
        for (v, key) in pon.iter_mut().zip(["bePonTop", "bePonBottom", "bePonLeft", "bePonRight"]) {
            *v = super::text_box(app, ui, key, "10", 44.0);
        }
        ui.end_row();
    });
    dim(ui, "PON tròn 5 mm. Bốn khoảng đo từ mép tờ đến tâm chấm.");

    let (mut gap, mut margin, mut clear) = (String::new(), String::new(), String::new());
    egui::Grid::new("dancard-be-gap").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        dim(ui, "Khe giữa 2 khuôn (mm)");
        dim(ui, "Khuôn cách mép tờ (mm)");
        ui.end_row();
        gap = super::text_box(app, ui, "beGap", "2", 72.0);
        margin = super::text_box(app, ui, "beMargin", "4", 72.0);
        ui.end_row();
        dim(ui, "Né PON (mm)");
        ui.end_row();
        clear = super::text_box(app, ui, "bePonClear", "7.5", 72.0);
        ui.end_row();
    });
    dim(ui, "Khe tính giữa đường bế; né PON tính từ mép chấm.");

    let two = super::check_box(app, ui, "beTwoSided", "Dàn bế 2 mặt (tag)", false);
    dim(ui, "Lật ngang như KTS, không lật gương chữ/hình.");
    let multi = super::check_box(app, ui, "beMultiPerArtboard", "Dàn nhiều mẫu vào một tờ", false);
    dim(ui, "Mặc định mỗi mẫu một tờ; tick để gộp.");

    ui.add_space(6.0);
    // Không chọn gì thì lệnh lấy mẫu theo layer Khuôn bế / Bài, nên chỉ cần tài liệu đang mở.
    let has_doc = app.session.active().is_some();
    if ui.add_enabled(has_doc, egui::Button::new("Dàn bế")).clicked() {
        let [top, bottom, left, right] = pon;
        let params = json!({
            "paperW": paper_w,
            "paperH": paper_h,
            "ponTop": top,
            "ponBottom": bottom,
            "ponLeft": left,
            "ponRight": right,
            "gap": gap,
            "margin": margin,
            "ponClear": clear,
            "twoSided": two,
            "multiPerArtboard": multi,
        });
        if super::run_and_note(app, ui, AREA, "dancard.dieLayout", params).is_some() {
            let super::Note(text, _) = super::pstate::<super::Note>(ui.ctx(), &super::note_id(AREA));
            if let Some((n, rest)) = super::count_prefix(&text) {
                app.status(format!("{n} con / tờ. {rest}"));
            }
            // Tờ mới đặt từ góc trên trái canvas như DanCard: đưa khung nhìn tới tờ đầu tiên vừa tạo.
            // Chỉ là khung nhìn: không tới được (chưa có cửa sổ vẽ) thì bỏ qua.
            let _ = app.run("view.goToArtboard", json!({"index": 0}));
        }
    }
    super::show_count_note(ui, AREA, &t, "Chọn mẫu rồi bấm Dàn bế.");
}

const AREA: &str = "danbe";

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::tests_labels::painted_text;

    /// Như `dan_be_panel.test.js`: sáu ô khổ giấy / PON đúng thứ tự, mặc định và đơn vị; khe, lề, né
    /// PON; gợi ý ngắn đủ ý; hai ô đánh dấu độc lập, mặc định không tick; nút Dàn bế; không còn chọn
    /// file PON. Kết quả "[[COUNT:n]]" thành dòng "n con / tờ".
    #[test]
    fn o_nhap_va_ket_qua_nhu_dancard() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        let text = painted_text(&mut app, section);
        let at = |s: &str| text.find(s).unwrap_or_else(|| panic!("thiếu {s}: {text}"));
        let order = ["Ngang giấy (cm)", "Cao giấy (cm)", "Trên (mm)", "Dưới (mm)", "Trái (mm)", "Phải (mm)"].map(at);
        assert!(order.windows(2).all(|w| w[0] < w[1]), "sáu ô theo đúng thứ tự người dùng yêu cầu");
        for s in [
            "Khe giữa 2 khuôn (mm)",
            "Khuôn cách mép tờ (mm)",
            "Né PON (mm)",
            "Khuôn → Bài",
            "Khuôn → Trước → Sau",
            "PON tròn 5 mm",
            "mép tờ đến tâm chấm",
            "đường bế",
            "mép chấm",
            "Lật ngang",
            "không lật gương",
            "Mặc định mỗi mẫu một tờ",
            "Dàn bế 2 mặt (tag)",
            "Dàn nhiều mẫu vào một tờ",
            "35.4",
            "7.5",
            "Chọn mẫu rồi bấm Dàn bế.",
        ] {
            at(s);
        }
        assert!(!text.contains("Chọn PON AI"));
        assert!(
            !super::super::saved_bool(&app, "beTwoSided", false) && !super::super::saved_bool(&app, "beMultiPerArtboard", false),
            "mặc định một mặt, mỗi mẫu một tờ"
        );
        let text = painted_text(&mut app, |app, ui| {
            super::super::set_pstate(
                ui.ctx(),
                &super::super::note_id(AREA),
                super::super::Note("[[COUNT:42]] Dàn silhouette thật: 42 con.".into(), false),
            );
            section(app, ui);
        });
        assert!(text.contains("42 con / tờ") && text.contains("Dàn silhouette thật: 42 con.") && !text.contains("[[COUNT"), "{text}");
    }
}
