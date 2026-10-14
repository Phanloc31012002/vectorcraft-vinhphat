//! Object → Rasterize: colour model, resolution, background, anti-aliasing, clipping mask and the
//! room around the art, starting from the document's raster effects settings. DanCard adds
//! "follow the transform box": a tilted card rasterizes along its own edges.
//!
//! Fields: the params of `object.rasterize`; `__docPpi` (the document's raster effects
//! resolution, for Use Document Raster Effects Resolution) is the dialog's own. OK runs the
//! command: one undo step. On an error the dialog stays open.

use serde_json::{Map, Value, json};
use vectorcraft_render::AntiAlias;

use super::DialogSpec;
use super::document_setup::{LABEL, check, choice};
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

pub(super) const KIND: &str = "rasterize";
const CMD: &str = "object.rasterize";
/// The resolution presets: (ppi, label).
const RESOLUTIONS: [(f64, &str); 3] = [(72.0, "Screen (72 ppi)"), (150.0, "Medium (150 ppi)"), (300.0, "High (300 ppi)")];
const USE_DOCUMENT: &str = "Use Document Raster Effects Resolution";

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| tl!("Rasterize").into(), body, confirm, min_width: 460.0, max_width: Some(460.0), ..DialogSpec::FORM };

/// Open the dialog on the document's raster effects settings (`document.rasterEffectsSettings`).
pub fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    let v = app.session.execute("document.rasterEffectsSettings", &json!({})).map_err(|e| e.to_string())?;
    let fields = json!({
        "ppi": v["resolution"],
        "__docPpi": v["resolution"],
        "__usesDocument": true,
        "colorModel": v["colorModel"],
        "background": v["background"],
        "antiAlias": if v["antiAlias"] == true { "art" } else { "none" },
        "clippingMask": v["clippingMask"],
        "addAround": v["addAround"],
        "followAngle": false,
    });
    app.ui.dialog = Some(Dialog::new(KIND, fields));
    Ok(Value::Null)
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let params: Map<String, Value> = d.fields.iter().filter(|(k, _)| !k.starts_with("__")).map(|(k, v)| (k.clone(), v.clone())).collect();
    let r = app.run(CMD, Value::Object(params));
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let own =
        if app.session.active().is_some_and(|s| s.doc.color_mode == vectorcraft_doc::ColorMode::Cmyk) { ("cmyk", "CMYK") } else { ("rgb", "RGB") };
    choice(ui, d, "colorModel", tl!("Color Model:"), &[own, ("grayscale", tl!("Grayscale")), ("bitmap", tl!("Bitmap"))]);
    ui.add_space(4.0);
    // Resolution: a preset, the document's, or Other with its own value.
    let ppi = d.f64("ppi", 72.0);
    let doc_ppi = d.f64("__docPpi", 72.0);
    let uses_document = d.bool("__usesDocument") && (ppi - doc_ppi).abs() < 1e-9;
    let preset = if uses_document { None } else { RESOLUTIONS.iter().position(|(p, _)| (*p - ppi).abs() < 1e-9) };
    let mut labels: Vec<&str> = RESOLUTIONS.iter().map(|(_, l)| tl!(l)).collect();
    labels.push(USE_DOCUMENT);
    labels.push(tl!("Other"));
    let shown = match (uses_document, preset) {
        (true, _) => RESOLUTIONS.len(),
        (false, Some(i)) => i,
        (false, None) => RESOLUTIONS.len() + 1,
    };
    widgets::label_row(ui, tl!("Resolution:"), LABEL, |ui| {
        if let Some(i) = widgets::dropdown(ui, "rasterize-resolution", labels.get(shown).copied().unwrap_or_default(), &labels, 260.0) {
            d.fields.insert("__usesDocument".into(), json!(i == RESOLUTIONS.len()));
            if let Some((p, _)) = RESOLUTIONS.get(i) {
                d.fields.insert("ppi".into(), json!(p));
            } else if i == RESOLUTIONS.len() {
                d.fields.insert("ppi".into(), json!(doc_ppi));
            } else if shown != RESOLUTIONS.len() + 1 {
                // Other keeps the current value to edit.
                d.fields.insert("ppi".into(), json!(ppi.round() + 1.0));
            }
        }
    });
    if shown == RESOLUTIONS.len() + 1 {
        widgets::label_row(ui, "", LABEL, |ui| {
            if let Some(v) = widgets::plain_field(ui, "rasterize-ppi", ppi, " ppi", 0, 80.0) {
                d.fields.insert("ppi".into(), json!(v));
            }
        });
    }
    ui.add_space(6.0);
    widgets::label_row(ui, tl!("Background:"), LABEL, |ui| {
        let cur = d.str("background");
        for (id, label) in [("white", tl!("White")), ("transparent", tl!("Transparent"))] {
            if widgets::radio(ui, label, cur == id, true) {
                d.fields.insert("background".into(), json!(id));
            }
            ui.add_space(12.0);
        }
    });
    ui.add_space(10.0);
    widgets::subheader(ui, tl!("Options"));
    widgets::label_row(ui, tl!("Anti-aliasing:"), LABEL, |ui| {
        let aa = AntiAlias::from_id(&d.str("antiAlias")).unwrap_or_default();
        if let Some(i) = widgets::dropdown(ui, "rasterize-aa", aa.label(), &AntiAlias::ALL.map(AntiAlias::label), 180.0) {
            d.fields.insert("antiAlias".into(), json!(AntiAlias::ALL.get(i).copied().unwrap_or_default().id()));
        }
    });
    ui.add_space(4.0);
    check(ui, d, "clippingMask", tl!("Create Clipping Mask"));
    let unit = app.session.general_unit();
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Add")).color(t.text));
        if let Some(v) = widgets::num_field(ui, "rasterize-add-around", Some(d.f64("addAround", 0.0)), unit, 80.0) {
            d.fields.insert("addAround".into(), json!(v));
        }
        ui.label(egui::RichText::new(tl!("Around Object")).color(t.text));
    });
    // DanCard: a tilted card (rotated here, or detected in an .ai/PDF file) rasterizes along its
    // transform box, so the image has no empty corners.
    check(ui, d, "followAngle", "Theo khung transform: card đặt nghiêng thì ảnh nghiêng theo (DanCard)");
    false
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use crate::VectorcraftApp;

    fn frame(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| crate::dialogs::show(app, ui.ctx()));
        out.textures_delta.clear();
    }

    #[test]
    fn rasterize_dialog_draws_and_ok_rasterizes() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap();
        crate::menus::invoke(&mut app, "object.rasterize", json!({}));
        assert_eq!(app.ui.dialog.as_ref().unwrap().kind, super::KIND);
        frame(&mut app);
        // Other resolutions show their own field; the dialog's own fields don't reach the command.
        let d = app.ui.dialog.as_mut().unwrap();
        d.fields.insert("ppi".into(), json!(144));
        d.fields.insert("antiAlias".into(), json!("type"));
        frame(&mut app);
        crate::dialogs::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none());
        let st = app.session.active().unwrap();
        let id = st.selection.objects[0];
        let vectorcraft_doc::NodeKind::Image(im) = &st.doc.node(id).unwrap().kind else { panic!("an image") };
        assert_eq!((im.width, im.height), (42, 42), "20 pt + stroke at 2 px/pt");
    }
}
