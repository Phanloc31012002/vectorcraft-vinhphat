//! DanCard: các lệnh và mặc định của bản IN Vĩnh Phát, chạy qua `Session` như panel gọi.

use serde_json::{Value, json};
use vectorcraft_doc::{NodeId, NodeKind, Unit};

use vectorcraft_tools::{PointerEvent, PointerKind};

use crate::{Prefs, Session, ViewInfo};

const CM: f64 = 72.0 / 2.54;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

/// Tài liệu CMYK có một card 9,2 × 5,6 cm (chữ nhật cam, không viền), đã xoay `angle` độ.
fn card(angle: f64) -> (Session, NodeId) {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let r = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 9.2 * CM, "height": 5.6 * CM})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0.6, "y": 1, "k": 0}}));
    run(&mut s, "paint.setStroke", json!({"none": true}));
    if angle != 0.0 {
        run(&mut s, "object.rotate", json!({"angle": angle}));
    }
    (s, r)
}

fn frame(s: &mut Session) -> Value {
    run(s, "dancard.transform", json!({"unit": "cm"}))["selection"].clone()
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() < 1e-3)
}

#[test]
fn mac_dinh_dancard_cm_ctrl_va_diem_neo() {
    let mut p = Prefs::default();
    assert!(!p.force_document_units && !p.ctrl_direct_selection && p.selection_tool_anchors, "upstream giữ nguyên");
    // Chỉ mục Units: các tùy chọn khác không đổi.
    p.apply_dancard_defaults(Some("Units"));
    assert_eq!((p.units_general.as_str(), p.force_document_units, p.ctrl_direct_selection), ("centimeters", true, false));
    let mut p = Prefs::default();
    p.apply_dancard_defaults(None);
    assert_eq!(p.units_general, "centimeters");
    assert_eq!(p.units_stroke, "points", "nét giữ pt");
    assert_eq!(p.units_type, "points", "chữ giữ pt");
    assert!(p.force_document_units && p.ctrl_direct_selection && !p.selection_tool_anchors);
    assert_eq!(p.dancard_defaults, super::DANCARD_DEFAULTS);
    // Reset trở về mặc định DanCard, không về point.
    let mut s = Session::new();
    s.apply_prefs(p);
    run(&mut s, "prefs.set", json!({"key": "unitsGeneral", "value": "millimeters"}));
    run(&mut s, "prefs.reset", json!({"category": "Units"}));
    assert_eq!(s.prefs.units_general, "centimeters");
    // Lưu rồi đọc lại giữ phiên bản mặc định đã áp.
    let back: Prefs = serde_json::from_value(s.prefs.to_json()).unwrap();
    assert_eq!(back.dancard_defaults, super::DANCARD_DEFAULTS);
    assert!(Prefs::default().to_json().get("dancardDefaults").is_none());
}

#[test]
fn moi_file_mo_ra_deu_dung_cm() {
    let mut src = Session::new();
    run(&mut src, "file.new", json!({"width": 200, "height": 200, "units": "millimeters"}));
    run(&mut src, "shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50}));
    let data = run(&mut src, "document.save", json!({}))["dataBase64"].as_str().unwrap().to_string();
    let mut p = Prefs::default();
    p.apply_dancard_defaults(None);
    let mut s = Session::new();
    s.apply_prefs(p);
    run(&mut s, "document.open", json!({"name": "mm.vectorcraft", "dataBase64": data}));
    assert_eq!(s.doc().unwrap().doc.units, Unit::Centimeters);
    // Tắt đi thì file giữ đơn vị của nó.
    s.prefs.force_document_units = false;
    run(&mut s, "document.open", json!({"name": "mm.vectorcraft", "dataBase64": data}));
    assert_eq!(s.doc().unwrap().doc.units, Unit::Millimeters);
}

#[test]
fn giu_ctrl_la_direct_selection_ca_voi_selection_tool() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 400, "height": 400}));
    let r = id_of(&run(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})));
    let v = ViewInfo::default();
    s.select_tool("selection", v).unwrap();
    assert_eq!(s.ctrl_tool(), None, "tắt: như upstream");
    s.prefs.ctrl_direct_selection = true;
    assert_eq!(s.ctrl_tool(), Some("directSelection"));
    // Bỏ chọn, rồi Ctrl + kéo góc dưới phải: chỉ điểm neo đó đi, như Direct Selection (Selection
    // tool thì kéo cả hình).
    run(&mut s, "select.none", json!({}));
    let mut ev = |kind, x, y| {
        let mut e = PointerEvent::new(kind, x, y);
        e.mods.cmd = true;
        s.pointer(&e, v).unwrap();
    };
    ev(PointerKind::Down, 100.0, 100.0);
    ev(PointerKind::Drag, 110.0, 120.0);
    ev(PointerKind::Drag, 120.0, 130.0);
    ev(PointerKind::Up, 120.0, 130.0);
    assert_eq!(s.tool_id(), "selection", "thả Ctrl ra là về Selection");
    let b = s.doc().unwrap().doc.node(r).unwrap().geometric_bounds().unwrap();
    assert!((b.x0, b.y0) == (0.0, 0.0) && (b.x1 - 120.0).abs() < 1e-6 && (b.y1 - 130.0).abs() < 1e-6, "{b:?}");
    // Direct Selection tự nó và các tool xem trang không đổi.
    s.select_tool("directSelection", v).unwrap();
    assert_eq!(s.ctrl_tool(), None);
    s.select_tool("hand", v).unwrap();
    assert_eq!(s.ctrl_tool(), None);
    s.select_tool("pen", v).unwrap();
    assert_eq!(s.ctrl_tool(), Some("directSelection"));
}

#[test]
fn raster_card_xoay_dung_khung_cmyk() {
    let (mut s, _) = card(20.0);
    let out = run(&mut s, "dancard.rasterize", json!({}));
    let img = &out["images"][0];
    assert_eq!(img["cmyk"], json!(true), "{out}");
    // 9,2 × 5,6 cm ở 500 ppi.
    assert_eq!((img["width"].as_u64(), img["height"].as_u64()), (Some(1811), Some(1102)));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 20.0) && close(&f["width"], 9.2) && close(&f["height"], 5.6), "{f}");
    let st = s.doc().unwrap();
    let id = st.selection.objects[0];
    let NodeKind::Image(im) = &st.doc.node(id).unwrap().kind else { panic!("ảnh") };
    let blob = &st.doc.images[&im.key];
    assert!(blob.is_cmyk(), "TIFF CMYK giữ mực");
    let inks = blob.cmyk().unwrap();
    // Giữa ảnh: M 60 %, Y 100 %, không C, không K.
    let mid = ((inks.height / 2 * inks.width + inks.width / 2) * 4) as usize;
    let px = &inks.data[mid..mid + 4];
    assert!(px[0] < 3 && px[1].abs_diff(153) < 3 && px[2] > 252 && px[3] < 3, "{px:?}");
    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    assert!(matches!(s.doc().unwrap().doc.node(s.doc().unwrap().selection.objects[0]).unwrap().kind, NodeKind::Path { .. }));
}

#[test]
fn raster_clip_group_chi_lay_phan_nhin_thay() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    // Nội dung 300 × 200 pt, mask 100 × 50 pt ở giữa.
    let content = id_of(&run(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 300, "height": 200})));
    let mask = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 75, "width": 100, "height": 50})));
    run(&mut s, "select.set", json!({"ids": [content.0, mask.0]}));
    run(&mut s, "object.clippingMask.make", json!({}));
    // Chọn object con (nội dung) vẫn raster cả clip group.
    run(&mut s, "select.set", json!({"ids": [content.0]}));
    let out = run(&mut s, "dancard.rasterize", json!({"ppi": 72}));
    assert_eq!((out["images"][0]["width"].as_u64(), out["images"][0]["height"].as_u64()), (Some(100), Some(50)), "{out}");
    let st = s.doc().unwrap();
    let b = st.doc.node(st.selection.objects[0]).unwrap().geometric_bounds().unwrap();
    assert!((b.x0 - 100.0).abs() < 1e-6 && (b.y0 - 75.0).abs() < 1e-6 && (b.width() - 100.0).abs() < 1e-6, "{b:?}");
    // Tài liệu RGB: báo đổi sang CMYK.
    let mut rgb = Session::new();
    run(&mut rgb, "file.new", json!({"width": 200, "height": 200}));
    run(&mut rgb, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}));
    let e = rgb.execute("dancard.rasterize", &json!({})).unwrap_err().to_string();
    assert!(e.contains("CMYK"), "{e}");
}

#[test]
fn object_rasterize_theo_khung_xoay() {
    let (mut s, _) = card(30.0);
    // Mặc định như Illustrator: ảnh đứng thẳng, ôm cả card nghiêng.
    let upright = run(&mut s, "object.rasterize", json!({"ppi": 72, "background": "white"}));
    let w = upright["width"].as_u64().unwrap();
    assert!(w as f64 > 9.2 * CM + 10.0, "{upright}");
    run(&mut s, "edit.undo", json!({}));
    let tilted = run(&mut s, "object.rasterize", json!({"ppi": 72, "background": "white", "followAngle": true}));
    assert_eq!(tilted["width"].as_u64(), Some((9.2 * CM).ceil() as u64), "{tilted}");
    let f = frame(&mut s);
    assert!(close(&f["angle"], 30.0), "{f}");
    // Nền trắng phủ kín: tài liệu CMYK lưu TIFF CMYK.
    let st = s.doc().unwrap();
    let NodeKind::Image(im) = &st.doc.node(st.selection.objects[0]).unwrap().kind else { panic!("ảnh") };
    assert!(st.doc.images[&im.key].is_cmyk());
}

#[test]
fn resize_card_nghieng_giu_tam_va_goc() {
    let (mut s, _) = card(30.0);
    let before = frame(&mut s);
    let out = run(&mut s, "dancard.resize", json!({"width": "9", "height": "5,4"}));
    assert_eq!(out["errors"], json!([]), "{out}");
    let f = frame(&mut s);
    assert!(close(&f["angle"], 30.0) && close(&f["width"], 9.0) && close(&f["height"], 5.4), "{f}");
    assert!(close(&f["center"][0], before["center"][0].as_f64().unwrap()) && close(&f["center"][1], before["center"][1].as_f64().unwrap()));
    assert!(s.execute("dancard.resize", &json!({"width": 0, "height": 5})).is_err());
}

#[test]
fn dau_cat_tu_dong_tam_net_va_khong_ve_trung() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 600, "height": 400, "colorMode": "cmyk"}));
    // Hai card sát nhau ở giữa artboard: cạnh chung không có nét chạy dọc.
    let a = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 100, "height": 60})));
    let b = id_of(&run(&mut s, "shape.rectangle", json!({"x": 200, "y": 100, "width": 100, "height": 60})));
    run(&mut s, "paint.setStroke", json!({"none": true, "ids": [a.0, b.0]}));
    run(&mut s, "select.set", json!({"ids": [a.0, b.0]}));
    let out = run(&mut s, "dancard.cutMarks", json!({"length": "2", "gap": "1"}));
    // Mỗi card 8 nét; nét dọc trên/dưới ở cạnh chung (x = 200) trùng nhau nên chỉ vẽ một lần,
    // nét ngang ở cạnh chung đè lên card bên kia nên bỏ.
    assert_eq!(out["cards"], json!(2), "{out}");
    let drawn = out["drawn"].as_u64().unwrap();
    assert_eq!(drawn, 10, "{out}");
    let d = &s.doc().unwrap().doc;
    let layer = d.layers.last().unwrap();
    assert_eq!(layer.name.as_deref(), Some("Dau cat tu dong"));
    let marks = layer.children().unwrap();
    assert_eq!(marks.len(), 10);
    for m in marks {
        let b = m.geometric_bounds().unwrap();
        assert!((b.width() - 2.0 * 72.0 / 25.4).abs() < 1e-6 || (b.height() - 2.0 * 72.0 / 25.4).abs() < 1e-6, "{b:?}");
        // Không nét nào nằm trong card.
        assert!(!(b.x0 > 100.0 + 1e-6 && b.x1 < 300.0 - 1e-6 && b.y0 > 100.0 + 1e-6 && b.y1 < 160.0 - 1e-6), "{b:?}");
    }
    // Bấm lại: không vẽ trùng.
    let again = run(&mut s, "dancard.cutMarks", json!({"length": "2", "gap": "1"}));
    assert_eq!(again["drawn"], json!(0), "{again}");
    assert!(again["message"].as_str().unwrap().contains("đã tồn tại"));
    assert!(s.execute("dancard.cutMarks", &json!({"length": 0})).is_err());
}

/// Tài liệu CMYK 330 × 480 mm có `n` mẫu card 9,2 × 5,6 cm đặt ngoài artboard, `cols` cột.
fn card_sources(n: usize, cols: usize) -> (Session, Vec<NodeId>) {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 330.0 * mm, "height": 480.0 * mm, "colorMode": "cmyk"}));
    let mut ids = vec![];
    for i in 0..n {
        let (row, col) = (i / cols, i % cols);
        let x = 400.0 * mm + col as f64 * 120.0 * mm;
        let y = row as f64 * 80.0 * mm;
        let id = id_of(&run(&mut s, "shape.rectangle", json!({"x": x, "y": y, "width": 92.0 * mm, "height": 56.0 * mm})));
        run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0.2 * i as f64, "y": 1, "k": 0}, "ids": [id.0]}));
        run(&mut s, "paint.setStroke", json!({"none": true, "ids": [id.0]}));
        ids.push(id);
    }
    run(&mut s, "select.set", json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}));
    (s, ids)
}

#[test]
fn dan_card_mot_mat_hai_mau() {
    let (mut s, _) = card_sources(2, 1);
    let out = run(&mut s, "dancard.layout", json!({"layout": "9.2x5.6"}));
    assert_eq!((out["models"].as_u64(), out["twoSides"].as_bool()), (Some(2), Some(false)), "{out}");
    let d = &s.doc().unwrap().doc;
    let g = NodeId(out["groups"][0].as_u64().unwrap());
    let group = d.node(g).unwrap();
    assert_eq!(group.children().unwrap().len(), 20, "20 chỗ");
    // Mỗi bản là clip group đúng 9,2 × 5,6 (ngang) hoặc 5,6 × 9,2 (dọc).
    for c in group.children().unwrap() {
        assert!(c.clips());
        let b = c.children().unwrap()[0].geometric_bounds().unwrap();
        let (w, h) = (b.width() * 2.54 / 72.0, b.height() * 2.54 / 72.0);
        assert!((w.max(h) - 9.2).abs() < 1e-3 && (w.min(h) - 5.6).abs() < 1e-3, "{w} × {h}");
    }
    // Cụm nằm giữa artboard 1; chỉ còn 1 artboard (in 1 mặt).
    assert_eq!(d.artboards.len(), 1);
    let gb = group.geometric_bounds().unwrap();
    let ab = d.artboards[0].rect;
    assert!((gb.center().x - ab.center().x).abs() < 0.5 && (gb.center().y - ab.center().y).abs() < 0.5, "{gb:?} {ab:?}");
    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().doc.node(g).is_none());
}

#[test]
fn dan_card_hai_mat_doi_xung() {
    let (mut s, _) = card_sources(4, 2);
    let out = run(&mut s, "dancard.layout", json!({"layout": "9.2x5.6", "rotation": -90}));
    assert_eq!((out["models"].as_u64(), out["twoSides"].as_bool()), (Some(2), Some(true)), "{out}");
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 2, "tự thêm artboard 2");
    let front = d.node(NodeId(out["groups"][0].as_u64().unwrap())).unwrap();
    let back = d.node(NodeId(out["groups"][1].as_u64().unwrap())).unwrap();
    assert_eq!((front.children().unwrap().len(), back.children().unwrap().len()), (20, 20));
    let (fb, bb) = (front.geometric_bounds().unwrap(), back.geometric_bounds().unwrap());
    assert!(d.artboards[0].rect.contains(fb.center()) && d.artboards[1].rect.contains(bb.center()));
    // Đối xứng: cụm mặt sau cùng khổ với mặt trước.
    assert!((fb.width() - bb.width()).abs() < 0.5 && (fb.height() - bb.height()).abs() < 0.5);
}

#[test]
fn dan_card_bao_loi_so_mau_va_sai_kho() {
    let (mut s, _) = card_sources(3, 1);
    let e = s.execute("dancard.layout", &json!({"layout": "9.2x5.6"})).unwrap_err().to_string();
    assert!(e.contains("Số mẫu 3 không hợp lệ"), "{e}");
    // Sai khổ (9 × 5 cm) không sửa được: báo lỗi, tài liệu giữ nguyên.
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 330.0 * mm, "height": 480.0 * mm, "colorMode": "cmyk"}));
    run(&mut s, "shape.rectangle", json!({"x": 1200, "y": 0, "width": 90.0 * mm, "height": 50.0 * mm}));
    let undo = s.doc().unwrap().history.undo.len();
    let e = s.execute("dancard.layout", &json!({"layout": "9.2x5.6"})).unwrap_err().to_string();
    assert!(e.contains("Sai kích thước"), "{e}");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
    assert!(s.execute("dancard.layout", &json!({"layout": "1x1"})).is_err());
}
