//! Panel "Công cụ bình" (DanCard, IN Vĩnh Phát): khung transform của thứ đang chọn — góc xoay,
//! rộng × cao, tâm — đọc bằng lệnh `dancard.transform`, khớp số với bảng Transform. Chọn clip
//! group, group lồng nhiều tầng hay object con đều đọc thẳng, không phải dò clip cha/con. Riêng
//! hình chữ nhật đặt nghiêng trong file .ai/PDF (không lưu góc) thì panel cho khung dò được, kèm số
//! của bảng Transform để đối chiếu. Chỉ tính lại khi tài liệu, vùng chọn hoặc đơn vị đổi.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "dancard";

/// Kết quả lần đọc trước và khóa của nó (tài liệu, lần sửa, vùng chọn, đơn vị, Use Preview Bounds).
#[derive(Clone, Default)]
struct Cache {
    key: (usize, u64, Vec<u64>, String, bool),
    value: Value,
}

/// Số kiểu Việt Nam, như bảng Transform: tối đa 4 chữ số lẻ, bỏ số 0 thừa, dấu phẩy thập phân.
fn num(v: &Value) -> String {
    let Some(x) = v.as_f64() else { return "–".into() };
    let s = format!("{x:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.replace('.', ",") }
}

fn source_label(s: &Value) -> &'static str {
    match s.as_str() {
        Some("transform") => "khung xoay",
        Some("detected") => "tự dò góc từ hình dạng",
        _ => "khung thẳng",
    }
}

fn frame_rows(ui: &mut Ui, f: &Value, unit: &str, t: &Tokens, id: &str) {
    let mono = |ui: &mut Ui, s: String| {
        ui.label(RichText::new(s).monospace().color(t.text));
    };
    let dim = |ui: &mut Ui, s: &str| {
        ui.label(RichText::new(s).color(t.text_dim));
    };
    egui::Grid::new(("dancard-frame", id)).num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        dim(ui, "Góc:");
        mono(ui, format!("{}°", num(&f["angle"])));
        dim(ui, "");
        dim(ui, source_label(&f["source"]));
        ui.end_row();
        dim(ui, "Rộng:");
        mono(ui, format!("{} {unit}", num(&f["width"])));
        dim(ui, "Cao:");
        mono(ui, format!("{} {unit}", num(&f["height"])));
        ui.end_row();
        // Tâm theo thước: bằng X/Y của bảng Transform khi chọn điểm tâm.
        dim(ui, "Tâm X:");
        mono(ui, num(&f["rulerCenter"][0]));
        dim(ui, "Tâm Y:");
        mono(ui, num(&f["rulerCenter"][1]));
        ui.end_row();
    });
    if let Some(tb) = f.get("transformBox") {
        ui.label(
            RichText::new(format!("Bảng Transform (khung thẳng): {} × {} {unit}", num(&tb["width"]), num(&tb["height"])))
                .color(t.text_dim),
        );
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let unit = Some(pstate::<String>(ui.ctx(), "dancard-unit")).filter(|u| !u.is_empty()).unwrap_or_else(|| "cm".into());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Đơn vị:").color(t.text_dim));
        for u in ["cm", "mm", "pt"] {
            if ui.selectable_label(unit == u, u).clicked() {
                set_pstate(ui.ctx(), "dancard-unit", u.to_string());
            }
        }
    });
    widgets::divider(ui);
    let preview = app.session.prefs.use_preview_bounds;
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, "Chưa mở tài liệu.");
        return;
    };
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();
    let key = (std::sync::Arc::as_ptr(&st.doc) as usize, st.revision, selected, unit.clone(), preview);
    if key.2.is_empty() {
        widgets::dim_label(ui, "Chọn card, clip group, group… để đọc khung transform.");
        return;
    }
    let id = egui::Id::new("dancard-transform-cache");
    let cached: Option<Cache> = ui.data(|d| d.get_temp(id));
    let value = match cached {
        Some(c) if c.key == key => c.value,
        _ => {
            let v = app.session.execute("dancard.transform", &json!({"unit": unit})).unwrap_or(Value::Null);
            ui.data_mut(|d| d.insert_temp(id, Cache { key, value: v.clone() }));
            v
        }
    };
    let Some(sel) = value.get("selection").filter(|s| s.is_object()) else {
        widgets::dim_label(ui, "Không đo được vùng chọn.");
        return;
    };
    let count = value["count"].as_u64().unwrap_or(0);
    ui.label(RichText::new(if count > 1 { format!("Cả vùng chọn ({count} object)") } else { "Object đang chọn".into() }).strong().color(t.text));
    frame_rows(ui, sel, &unit, &t, "selection");
    if count > 1
        && let Some(objects) = value["objects"].as_array()
    {
        widgets::divider(ui);
        egui::CollapsingHeader::new(format!("Từng object ({count})")).default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for (i, o) in objects.iter().enumerate() {
                    ui.label(
                        RichText::new(format!(
                            "{}. {}  {}°  {} × {}",
                            i + 1,
                            o["kind"].as_str().unwrap_or(""),
                            num(&o["angle"]),
                            num(&o["width"]),
                            num(&o["height"])
                        ))
                        .monospace()
                        .color(t.text),
                    );
                }
            });
        });
    }
    widgets::divider(ui);
    if ui.button("Sao chép số đo (JSON)").clicked() {
        ui.ctx().copy_text(serde_json::to_string_pretty(&value).unwrap_or_default());
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;
    use crate::tests_labels::painted_text;

    #[test]
    fn hien_khung_xoay_cua_card_dang_chon() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        // Card 9,2 × 5,6 cm xoay 20°.
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 260.787, "height": 158.740})).unwrap();
        app.run("object.rotate", json!({"angle": 20})).unwrap();
        let text = painted_text(&mut app, super::show);
        assert!(text.contains("20°"), "{text}");
        assert!(text.contains("9,2 cm") && text.contains("5,6 cm"), "{text}");
        // Không chọn gì: nhắc chọn object.
        app.run("select.none", json!({})).unwrap();
        assert!(painted_text(&mut app, super::show).contains("Chọn card"));
    }

    /// Clip group: đúng khung mask như bảng Transform, không phải phần giao với nội dung.
    #[test]
    fn clip_group_khop_bang_transform() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        // Nội dung chỉ phủ nửa trái của mask 200 × 100 pt.
        let content = app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].clone();
        let mask = app.run("shape.rectangle", json!({"x": 100, "y": 50, "width": 200, "height": 100})).unwrap()["id"].clone();
        app.run("select.set", json!({"ids": [content, mask]})).unwrap();
        app.run("object.clippingMask.make", json!({})).unwrap();
        let b = app.selection_box().unwrap();
        let v = app.session.execute("dancard.transform", &json!({"unit": "pt"})).unwrap();
        assert!((v["selection"]["width"].as_f64().unwrap() - b.rect.width()).abs() < 1e-3, "{v}");
        assert!((v["selection"]["height"].as_f64().unwrap() - b.rect.height()).abs() < 1e-3, "{v}");
        assert_eq!(v["selection"]["width"], json!(200.0));
    }
}
