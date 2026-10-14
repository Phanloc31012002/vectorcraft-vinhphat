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
        super::run_and_note(app, ui, "danbe", "dancard.dieLayout", params);
    }
    super::show_note(ui, "danbe", &t, "Chọn mẫu rồi bấm Dàn bế.");
}
