//! Panel "Công cụ bình" (DanCard, IN Vĩnh Phát): khung transform thật của thứ đang chọn — góc
//! xoay, rộng × cao theo khung, tâm — đọc bằng lệnh `dancard.transform`. Chọn clip group, group lồng
//! nhiều tầng hay object con đều đọc thẳng, không phải dò clip cha/con. Chỉ tính lại khi tài liệu
//! hoặc vùng chọn đổi.

use egui::{RichText, Ui};
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "dancard";

/// Kết quả lần đọc trước và khóa của nó (tài liệu, lần sửa, vùng chọn, đơn vị).
#[derive(Clone, Default)]
struct Cache {
    key: (usize, u64, Vec<u64>, String),
    value: Value,
}

/// Số kiểu Việt Nam: dấu phẩy thập phân.
fn num(v: &Value, decimals: usize) -> String {
    v.as_f64().map_or_else(|| "–".into(), |x| format!("{x:.decimals$}").replace('.', ","))
}

fn source_label(s: &Value) -> &'static str {
    match s.as_str() {
        Some("transform") => "góc lưu trong VectorCraft",
        Some("detected") => "tự dò từ hình dạng",
        _ => "khung thẳng",
    }
}

fn frame_rows(ui: &mut Ui, f: &Value, unit: &str, t: &Tokens) {
    let mono = |ui: &mut Ui, s: String| {
        ui.label(RichText::new(s).monospace().color(t.text));
    };
    mono(ui, format!("Góc:  {}°", num(&f["angle"], 2)));
    ui.label(RichText::new(source_label(&f["source"])).small().color(t.text_dim));
    ui.columns(2, |cols| {
        mono(&mut cols[0], format!("Rộng: {} {unit}", num(&f["width"], 2)));
        mono(&mut cols[1], format!("Cao:  {} {unit}", num(&f["height"], 2)));
    });
    // Tâm so với artboard chứa nó (nếu có), không thì so với trang.
    let (c, label) = match f.get("onArtboard") {
        Some(o) => (&o["center"], format!("so với artboard {}", f["artboard"].as_u64().map_or(0, |i| i + 1))),
        None => (&f["center"], "so với trang".to_string()),
    };
    ui.columns(2, |cols| {
        mono(&mut cols[0], format!("Tâm X: {}", num(&c[0], 2)));
        mono(&mut cols[1], format!("Tâm Y: {}", num(&c[1], 2)));
    });
    ui.label(RichText::new(label).small().color(t.text_dim));
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
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, "Chưa mở tài liệu.");
        return;
    };
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();
    let key = (std::sync::Arc::as_ptr(&st.doc) as usize, st.revision, selected, unit.clone());
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
    frame_rows(ui, sel, &unit, &t);
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
                            num(&o["angle"], 1),
                            num(&o["width"], 2),
                            num(&o["height"], 2)
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
        assert!(text.contains("20,00°"), "{text}");
        assert!(text.contains("9,20 cm") && text.contains("5,60 cm"), "{text}");
        // Không chọn gì: nhắc chọn object.
        app.run("select.none", json!({})).unwrap();
        assert!(painted_text(&mut app, super::show).contains("Chọn card"));
    }
}
