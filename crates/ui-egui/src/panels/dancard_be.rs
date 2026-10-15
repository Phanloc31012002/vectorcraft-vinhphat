//! Mục Dàn bế của tab Dàn tối ưu (DanCard): dàn decal / tag theo khuôn bế, tự vẽ bốn chấm PON
//! tròn 5 mm theo khổ giấy nhập (`dancard.dieLayout`). Ô, mặc định và gợi ý như panel DanCard.

use egui::Ui;
use serde_json::json;

use super::{check_box, num_field, num_fields, style};
use crate::VectorcraftApp;

const AREA: &str = "danbe";

pub(super) fn section(app: &mut VectorcraftApp, ui: &mut Ui) {
    style::label(ui, "Dàn decal / tag theo khuôn bế");
    style::hint(ui, "Chọn từng mẫu: Khuôn → Bài; hai mặt: Khuôn → Trước → Sau.");
    let [paper_w, paper_h] = num_fields(app, ui, [("Ngang giấy (cm)", "bePaperW", "33"), ("Cao giấy (cm)", "bePaperH", "35.4")]);
    let [top, bottom, left, right] = num_fields(
        app,
        ui,
        [("Trên (mm)", "bePonTop", "10"), ("Dưới (mm)", "bePonBottom", "10"), ("Trái (mm)", "bePonLeft", "10"), ("Phải (mm)", "bePonRight", "10")],
    );
    style::hint(ui, "PON tròn 5 mm. Bốn khoảng đo từ mép tờ đến tâm chấm.");
    let [gap, margin] = num_fields(app, ui, [("Khe giữa 2 khuôn (mm)", "beGap", "2"), ("Khuôn cách mép tờ (mm)", "beMargin", "4")]);
    let mut clear = String::new();
    // Ô Né PON chiếm nửa hàng như DanCard (ô bên cạnh để trống).
    style::columns(ui, 2, |ui, i| {
        if i == 0 {
            clear = num_field(app, ui, "Né PON (mm)", "bePonClear", "7.5");
        }
    });
    style::hint(ui, "Khe tính giữa đường bế; né PON tính từ mép chấm.");
    let two = check_box(app, ui, "beTwoSided", "Dàn bế 2 mặt (tag)", false);
    style::hint(ui, "Lật ngang như KTS, không lật gương chữ/hình.");
    let multi = check_box(app, ui, "beMultiPerArtboard", "Dàn nhiều mẫu vào một tờ", false);
    style::hint(ui, "Mặc định mỗi mẫu một tờ; tick để gộp.");
    // Không chọn gì thì lệnh lấy mẫu theo layer Khuôn bế / Bài, nên chỉ cần tài liệu đang mở.
    let has_doc = app.session.active().is_some();
    if style::primary(ui, "Dàn bế", has_doc).clicked() {
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
    super::show_count_note(ui, AREA, "Chọn mẫu rồi bấm Dàn bế.");
}

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
        let order = ["NGANG GIẤY (CM)", "CAO GIẤY (CM)", "TRÊN (MM)", "DƯỚI (MM)", "TRÁI (MM)", "PHẢI (MM)"].map(at);
        assert!(order.windows(2).all(|w| w[0] < w[1]), "sáu ô theo đúng thứ tự người dùng yêu cầu");
        for s in [
            "KHE GIỮA 2 KHUÔN (MM)",
            "KHUÔN CÁCH MÉP TỜ (MM)",
            "NÉ PON (MM)",
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
