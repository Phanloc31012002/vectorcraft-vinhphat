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
    // Số gõ nhầm (1e12 cm) bị từ chối ngay, không làm phẳng đường cong thành hàng tỉ đoạn.
    for id in ["dancard.resize", "dancard.clipToSize"] {
        let e = s.execute(id, &json!({"width": 1e12, "height": 5})).unwrap_err().to_string();
        assert!(e.contains("tối đa 1000 cm"), "{id}: {e}");
    }
    run(&mut s, "dancard.resize", json!({"width": 999, "height": 5}));
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

/// Chữ nhật `w` × `h` pt ở (`x`, `y`), không viền.
fn plain_rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    let id = id_of(&run(s, "shape.rectangle", json!({"x": x, "y": y, "width": w, "height": h})));
    run(s, "paint.setStroke", json!({"none": true, "ids": [id.0]}));
    id
}

/// `content` cắt theo chữ nhật `mask` (x, y, rộng, cao) → id của clip group.
fn clip_to(s: &mut Session, content: NodeId, mask: (f64, f64, f64, f64)) -> NodeId {
    let m = plain_rect(s, mask.0, mask.1, mask.2, mask.3);
    run(s, "select.set", json!({"ids": [content.0, m.0]}));
    run(s, "object.clippingMask.make", json!({}));
    s.doc().unwrap().selection.objects[0]
}

fn group_of(s: &mut Session, ids: &[NodeId]) -> NodeId {
    run(s, "select.set", json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}));
    id_of(&run(s, "object.group", json!({})))
}

/// Card 100 × 60 pt ở (`x`, `y`) như file thiết kế: group gồm nền, ảnh đã clip và logo đã clip.
fn designed_card(s: &mut Session, x: f64, y: f64) -> NodeId {
    let bg = plain_rect(s, x, y, 100.0, 60.0);
    let photo = plain_rect(s, x + 5.0, y + 5.0, 60.0, 50.0);
    let photo = clip_to(s, photo, (x + 10.0, y + 10.0, 40.0, 40.0));
    let logo = plain_rect(s, x + 70.0, y + 10.0, 30.0, 30.0);
    let logo = clip_to(s, logo, (x + 72.0, y + 12.0, 20.0, 20.0));
    group_of(s, &[bg, photo, logo])
}

fn cut_marks_on(s: &mut Session, ids: &[NodeId]) -> Value {
    run(s, "select.set", json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}));
    run(s, "dancard.cutMarks", json!({"length": "2", "gap": "1"}))
}

/// Nét `m` (khung của nó) nằm ở một góc của bài `card` (x0, y0, x1, y1): trên đường thẳng của một
/// cạnh qua góc đó, cách góc không quá `reach` (cách bài + dài nét).
fn mark_at_corner(m: vectorcraft_geom::Rect, card: [f64; 4], reach: f64) -> bool {
    let [x0, y0, x1, y1] = card;
    let e = 1e-6;
    [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].iter().any(|&(cx, cy)| {
        let horizontal = (m.y0 - cy).abs() < e && (m.y1 - cy).abs() < e && m.x0 >= cx - reach - e && m.x1 <= cx + reach + e;
        let vertical = (m.x0 - cx).abs() < e && (m.x1 - cx).abs() < e && m.y0 >= cy - reach - e && m.y1 <= cy + reach + e;
        horizontal || vertical
    })
}

/// Cách bài 1 mm + dài nét 2 mm của [`cut_marks_on`] (pt).
const MARK_REACH: f64 = 3.0 * 72.0 / 25.4;

fn sheet_doc() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 600, "height": 400, "colorMode": "cmyk"}));
    s
}

/// Dấu cắt theo khối: group hay clip group là một bài, không đánh quanh chi tiết bên trong; tờ /
/// hàng đã dàn (group chỉ gồm các khối đặt rời nhau) thì mỗi khối một bài.
#[test]
fn dau_cat_theo_khoi_group_va_clip() {
    // Card là group có ảnh và logo đã clip: một bài, 8 nét ở bốn góc card, không nét nào trong card.
    // (DanCard trên Illustrator tách group có từ 2 clip group con, nên đánh quanh ảnh và logo.)
    let mut s = sheet_doc();
    let card = designed_card(&mut s, 100.0, 100.0);
    let out = cut_marks_on(&mut s, &[card]);
    assert_eq!((out["cards"].as_u64(), out["drawn"].as_u64()), (Some(1), Some(8)), "{out}");
    let d = &s.doc().unwrap().doc;
    for m in d.layers.last().unwrap().children().unwrap() {
        let b = m.geometric_bounds().unwrap();
        assert!(mark_at_corner(b, [100.0, 100.0, 200.0, 160.0], MARK_REACH), "nét ở góc card: {b:?}");
        assert!(!(b.x0 > 100.0 + 1e-6 && b.x1 < 200.0 - 1e-6 && b.y0 > 100.0 + 1e-6 && b.y1 < 160.0 - 1e-6), "{b:?}");
    }

    // Hai group chồng lên nhau (nền và phần chữ) gom thành card: vẫn một bài.
    let mut s = sheet_doc();
    let back = plain_rect(&mut s, 100.0, 100.0, 100.0, 60.0);
    let back = group_of(&mut s, &[back]);
    let text = plain_rect(&mut s, 120.0, 120.0, 40.0, 10.0);
    let text = group_of(&mut s, &[text]);
    let card = group_of(&mut s, &[back, text]);
    assert_eq!(cut_marks_on(&mut s, &[card])["cards"], json!(1));

    // Tờ đã dàn gồm 3 card như trên đặt cạnh nhau, nền tràn lề chồng nhau 3 pt: mỗi card một bài.
    let mut s = sheet_doc();
    let cards: Vec<NodeId> = (0..3).map(|i| designed_card(&mut s, 50.0 + f64::from(i) * 97.0, 100.0)).collect();
    let sheet = group_of(&mut s, &cards);
    assert_eq!(cut_marks_on(&mut s, &[sheet])["cards"], json!(3));
    // Chọn từng card thay cho cả tờ: như vậy.
    let mut s = sheet_doc();
    let cards: Vec<NodeId> = (0..3).map(|i| designed_card(&mut s, 50.0 + f64::from(i) * 100.0, 100.0)).collect();
    assert_eq!(cut_marks_on(&mut s, &cards)["cards"], json!(3));

    // Tờ gồm 2 hàng (group), mỗi hàng 2 card đã clip như Dàn card (ảnh tràn lề): 4 bài, nét ngoài
    // khung các mask, không nét nào trong tờ.
    let mut s = sheet_doc();
    let mut rows = vec![];
    for r in 0..2 {
        let mut row = vec![];
        for c in 0..2 {
            let (x, y) = (100.0 + f64::from(c) * 100.0, 100.0 + f64::from(r) * 60.0);
            let art = plain_rect(&mut s, x - 5.0, y - 5.0, 110.0, 70.0);
            row.push(clip_to(&mut s, art, (x, y, 100.0, 60.0)));
        }
        rows.push(group_of(&mut s, &row));
    }
    let sheet = group_of(&mut s, &rows);
    let out = cut_marks_on(&mut s, &[sheet]);
    assert_eq!(out["cards"], json!(4), "{out}");
    let d = &s.doc().unwrap().doc;
    for m in d.layers.last().unwrap().children().unwrap() {
        let b = m.geometric_bounds().unwrap();
        assert!(b.x1 <= 100.0 + 1e-6 || b.x0 >= 300.0 - 1e-6 || b.y1 <= 100.0 + 1e-6 || b.y0 >= 220.0 - 1e-6, "nét ngoài tờ: {b:?}");
    }
}

/// Card trắng không nền gồm logo và mã QR cùng cỡ (group) với chữ: một bài. Tờ voucher ghép card
/// (toàn clip group, khác cỡ): mỗi clip group một bài. Group bọc ngoài một tờ: các bài của tờ.
/// Clip group có hai nửa cùng cỡ bên trong (card gấp): một bài.
#[test]
fn dau_cat_khoi_dac_biet() {
    let mut s = sheet_doc();
    let logo = plain_rect(&mut s, 110.0, 110.0, 30.0, 30.0);
    let logo = group_of(&mut s, &[logo]);
    let qr = plain_rect(&mut s, 160.0, 110.0, 30.0, 30.0);
    let qr = group_of(&mut s, &[qr]);
    let name = plain_rect(&mut s, 110.0, 150.0, 80.0, 6.0);
    let card = group_of(&mut s, &[logo, qr, name]);
    assert_eq!(cut_marks_on(&mut s, &[card])["cards"], json!(1));

    let mut s = sheet_doc();
    let voucher = plain_rect(&mut s, 40.0, 40.0, 160.0, 80.0);
    let voucher = clip_to(&mut s, voucher, (40.0, 40.0, 152.0, 72.0));
    let small = plain_rect(&mut s, 220.0, 40.0, 100.0, 64.0);
    let small = clip_to(&mut s, small, (220.0, 40.0, 92.0, 56.0));
    let sheet = group_of(&mut s, &[voucher, small]);
    assert_eq!(cut_marks_on(&mut s, &[sheet])["cards"], json!(2));

    let mut s = sheet_doc();
    let cards: Vec<NodeId> = (0..2)
        .map(|i| {
            let x = 60.0 + f64::from(i) * 130.0;
            let art = plain_rect(&mut s, x - 3.0, 97.0, 106.0, 66.0);
            clip_to(&mut s, art, (x, 100.0, 100.0, 60.0))
        })
        .collect();
    let sheet = group_of(&mut s, &cards);
    let wrapper = group_of(&mut s, &[sheet]);
    assert_eq!(cut_marks_on(&mut s, &[wrapper])["cards"], json!(2));

    // Card trắng gồm hai nửa: [logo, tên] và [điện thoại, địa chỉ] (mỗi thứ một group, hai dòng
    // chữ cùng cỡ): một bài, không tách theo nửa hay theo dòng.
    let mut s = sheet_doc();
    let line = |s: &mut Session, x: f64, y: f64, w: f64, h: f64| {
        let r = plain_rect(s, x, y, w, h);
        group_of(s, &[r])
    };
    let logo = line(&mut s, 110.0, 105.0, 20.0, 20.0);
    let name = line(&mut s, 135.0, 110.0, 60.0, 8.0);
    let top = group_of(&mut s, &[logo, name]);
    let phone = line(&mut s, 110.0, 135.0, 60.0, 8.0);
    let address = line(&mut s, 110.0, 147.0, 60.0, 8.0);
    let bottom = group_of(&mut s, &[phone, address]);
    let card = group_of(&mut s, &[top, bottom]);
    let out = cut_marks_on(&mut s, &[card]);
    assert_eq!(out["cards"], json!(1), "{out}");

    let mut s = sheet_doc();
    let left = plain_rect(&mut s, 100.0, 100.0, 92.0, 56.0);
    let left = group_of(&mut s, &[left]);
    let right = plain_rect(&mut s, 192.0, 100.0, 92.0, 56.0);
    let right = group_of(&mut s, &[right]);
    let halves = group_of(&mut s, &[left, right]);
    let folded = clip_to(&mut s, halves, (100.0, 100.0, 184.0, 56.0));
    let out = cut_marks_on(&mut s, &[folded]);
    assert_eq!(out["cards"], json!(1), "{out}");
}

/// Chọn chi tiết bên trong khối (Direct Selection, giữ Ctrl): dấu cắt vẫn quanh cả bài chứa nó,
/// không quanh chi tiết; chọn một hàng của tờ thì các bài của hàng đó.
#[test]
fn dau_cat_chon_chi_tiet_tinh_ca_bai() {
    // Tờ 3 card thiết kế cách nhau 30 pt.
    let mut s = sheet_doc();
    let cards: Vec<NodeId> = (0..3).map(|i| designed_card(&mut s, 50.0 + f64::from(i) * 130.0, 100.0)).collect();
    let sheet = group_of(&mut s, &cards);
    let (logo_art, photo_mask) = {
        let d = &s.doc().unwrap().doc;
        let kids = |id: NodeId| d.node(id).unwrap().children().unwrap().iter().map(|c| c.id).collect::<Vec<_>>();
        // Card 2: [nền, ảnh đã clip, logo đã clip]; trong clip group, con đầu là mask.
        let card2 = kids(cards[1]);
        let card3 = kids(cards[2]);
        (kids(card2[2])[1], kids(card3[1])[0])
    };
    // Chỉ chọn hình logo bên trong card 2: một bài, 8 nét ở bốn góc card 2.
    let out = cut_marks_on(&mut s, &[logo_art]);
    assert_eq!((out["cards"].as_u64(), out["drawn"].as_u64()), (Some(1), Some(8)), "{out}");
    let d = &s.doc().unwrap().doc;
    for m in d.layers.last().unwrap().children().unwrap() {
        let b = m.geometric_bounds().unwrap();
        assert!(mark_at_corner(b, [180.0, 100.0, 280.0, 160.0], MARK_REACH), "nét ở góc card 2: {b:?}");
    }
    // Chi tiết của card 2 và card 3 (cả mask của ảnh): hai bài; card 2 đã có dấu nên chỉ thêm 8 nét.
    let out = cut_marks_on(&mut s, &[logo_art, photo_mask]);
    assert_eq!((out["cards"].as_u64(), out["drawn"].as_u64()), (Some(2), Some(8)), "{out}");
    // Cả tờ: 3 bài.
    assert_eq!(cut_marks_on(&mut s, &[sheet])["cards"], json!(3));

    // Tờ 2 hàng × 2 card đã clip; chọn hàng dưới: 2 bài của hàng đó.
    let mut s = sheet_doc();
    let mut rows = vec![];
    for r in 0..2 {
        let mut row = vec![];
        for c in 0..2 {
            let (x, y) = (100.0 + f64::from(c) * 130.0, 100.0 + f64::from(r) * 90.0);
            let art = plain_rect(&mut s, x - 5.0, y - 5.0, 110.0, 70.0);
            row.push(clip_to(&mut s, art, (x, y, 100.0, 60.0)));
        }
        rows.push(group_of(&mut s, &row));
    }
    group_of(&mut s, &rows);
    let out = cut_marks_on(&mut s, &[rows[1]]);
    assert_eq!(out["cards"], json!(2), "{out}");
    let d = &s.doc().unwrap().doc;
    // Hàng trên ở y 100…160, hàng dưới ở y 190…250: nét của hàng dưới từ y 190 − 3 mm trở xuống.
    assert!(
        d.layers.last().unwrap().children().unwrap().iter().all(|m| m.geometric_bounds().unwrap().y0 >= 190.0 - MARK_REACH - 1e-6),
        "chỉ hàng dưới"
    );
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

#[test]
fn auto_save_pdf_dat_ten_va_luu_dung_so_trang() {
    let (mut s, ids) = card_sources(3, 1);
    // Hàng 1: trước + sau; hàng 2: 1 mặt (cột trái). Dời mẫu thứ 2 sang phải cùng hàng với mẫu 1.
    let mm = 72.0 / 25.4;
    run(&mut s, "select.set", json!({"ids": [ids[1].0]}));
    run(&mut s, "object.move", json!({"dx": 120.0 * mm, "dy": -80.0 * mm}));
    run(&mut s, "select.set", json!({"ids": [ids[0].0, ids[1].0, ids[2].0]}));
    let dry = run(&mut s, "dancard.autoSavePdf", json!({"prefix": "1088", "start": "7", "pad": "2", "boxes": "5, 10", "date": "14", "dryRun": true}));
    assert_eq!(
        dry["files"],
        json!([
            "1088 - 07 - 5 hop - cm - 14.pdf",
            "1088 - 08 - 10 hop - cm - 14.pdf",
            "1088 - 09 - 5 hop - 1 mat - cm - 14.pdf",
            "1088 - 10 - 10 hop - 1 mat - cm - 14.pdf"
        ]),
        "{dry}"
    );
    assert_eq!((dry["oneSide"].as_u64(), dry["twoSides"].as_u64()), (Some(1), Some(1)));
    let dir = std::env::temp_dir().join(format!("dancard-autosave-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = run(&mut s, "dancard.autoSavePdf", json!({"boxes": "5", "boxesInOrder": false, "date": "1", "folder": dir.to_string_lossy()}));
    assert_eq!(out["saved"], json!(2), "{out}");
    let two = std::fs::read(dir.join("1088 - 1 - 5 hop - cm - 1.pdf")).unwrap();
    let one = std::fs::read(dir.join("1088 - 2 - 5 hop - 1 mat - cm - 1.pdf")).unwrap();
    // Số trang: các đối tượng /Type /Page (không tính /Pages).
    let pages = |b: &[u8]| {
        let t = String::from_utf8_lossy(b).replace("/Type /Page", "/Type/Page");
        t.match_indices("/Type/Page").filter(|(i, _)| t.as_bytes().get(i + 10) != Some(&b's')).count()
    };
    assert!(two.starts_with(b"%PDF") && one.starts_with(b"%PDF"));
    assert_eq!((pages(&two), pages(&one)), (2, 1), "2 mặt: 2 trang; 1 mặt: 1 trang");
    // Không ghi đè file có sẵn.
    let again = s.execute("dancard.autoSavePdf", &json!({"boxes": "5", "date": "1", "folder": dir.to_string_lossy()})).unwrap_err().to_string();
    assert!(again.contains("đã tồn tại"), "{again}");
    let _ = std::fs::remove_dir_all(&dir);
    // Ghép số hộp theo thứ tự cần đúng số lượng.
    assert!(s.execute("dancard.autoSavePdf", &json!({"boxes": "5", "boxesInOrder": true, "dryRun": true})).is_err());
}

#[test]
fn clip_card_tu_xoay_theo_object() {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 600, "height": 600, "colorMode": "cmyk"}));
    let tall = id_of(&run(&mut s, "shape.rectangle", json!({"x": 10, "y": 10, "width": 60.0 * mm, "height": 95.0 * mm})));
    run(&mut s, "select.set", json!({"ids": [tall.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    let g = NodeId(out["ids"][0].as_u64().unwrap());
    let d = &s.doc().unwrap().doc;
    let b = d.node(g).unwrap().geometric_bounds().unwrap();
    assert!((b.width() / mm - 56.0).abs() < 1e-3 && (b.height() / mm - 92.0).abs() < 1e-3, "{b:?}");
}

#[test]
fn catalogue_a4_mot_cuon_a5_hai_cuon_xoay() {
    use super::catalogue::plan_for;
    let a4 = plan_for(211.0, 299.0).unwrap();
    assert_eq!((a4.copies, a4.rotate), (1, false));
    let a5 = plan_for(148.0, 210.0).unwrap();
    assert_eq!((a5.copies, a5.rotate), (2, true));
    assert!(plan_for(300.0, 400.0).is_none());
    // 4 trang A5 (vector) → 1 tờ, 2 mặt, mỗi mặt 2 cuốn.
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 800, "colorMode": "cmyk"}));
    let mut ids = vec![];
    for i in 0..4 {
        let id =
            id_of(&run(&mut s, "shape.rectangle", json!({"x": 10.0 + i as f64 * 160.0 * mm, "y": 10, "width": 148.0 * mm, "height": 210.0 * mm})));
        run(&mut s, "paint.setStroke", json!({"none": true, "ids": [id.0]}));
        ids.push(id.0);
    }
    run(&mut s, "select.set", json!({"ids": ids}));
    let out = run(&mut s, "dancard.catalogue", json!({"width": "14,8", "height": "21", "coverNote": ""}));
    assert_eq!((out["copies"].as_u64(), out["faces"].as_u64()), (Some(2), Some(2)), "{out}");
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 2);
    assert_eq!(out["groups"].as_array().unwrap().len(), 4);
    for g in out["groups"].as_array().unwrap() {
        let b = d.node(NodeId(g.as_u64().unwrap())).unwrap().geometric_bounds().unwrap();
        // Cuốn đã xoay: rộng = cao trang, cao = 2 × rộng trang.
        assert!((b.width() / mm - 210.0).abs() < 0.5 && (b.height() / mm - 296.0).abs() < 0.5, "{b:?}");
    }
    assert!(d.layers.iter().any(|l| l.name.as_deref() == Some("Pon Catalogue tu dong") && l.locked));
    // Số trang không chia hết cho 4: báo lỗi.
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "select.set", json!({"ids": [ids[0], ids[1], ids[2]]}));
    assert!(s.execute("dancard.catalogue", &json!({"width": 14.8, "height": 21})).is_err());
}

/// Số ảnh trong tài liệu (lệnh dàn không được tự raster).
fn images(s: &Session) -> usize {
    let mut n = 0;
    s.doc().unwrap().doc.walk(|node| {
        if matches!(node.kind, NodeKind::Image(_)) {
            n += 1;
        }
    });
    n
}

/// Mẫu xoay 90° trong app và mẫu đặt nghiêng 17°: Dàn card đo theo khung transform, dựng thẳng rồi
/// dàn, không báo sai khổ, không raster; mọi bản đúng 9,2 × 5,6 và đứng thẳng trong khung clip.
#[test]
fn dan_card_do_theo_khung_transform_khong_raster() {
    let (mut s, ids) = card_sources(2, 1);
    run(&mut s, "select.set", json!({"ids": [ids[0].0]}));
    run(&mut s, "object.rotate", json!({"angle": 90}));
    run(&mut s, "select.set", json!({"ids": [ids[1].0]}));
    run(&mut s, "object.rotate", json!({"angle": 17}));
    run(&mut s, "select.set", json!({"ids": [ids[0].0, ids[1].0]}));
    let out = run(&mut s, "dancard.layout", json!({"layout": "9.2x5.6"}));
    assert_eq!(out["models"].as_u64(), Some(2), "{out}");
    assert_eq!(images(&s), 0, "không raster");
    let d = &s.doc().unwrap().doc;
    let group = d.node(NodeId(out["groups"][0].as_u64().unwrap())).unwrap();
    assert_eq!(group.children().unwrap().len(), 20);
    for c in group.children().unwrap() {
        let kids = c.children().unwrap();
        let mask = kids[0].geometric_bounds().unwrap();
        let (w, h) = (mask.width() / CM, mask.height() / CM);
        assert!((w.max(h) - 9.2).abs() < 1e-3 && (w.min(h) - 5.6).abs() < 1e-3, "{w} × {h}");
        // Bài đã dựng thẳng: lấp đúng khung clip (bài còn nghiêng thì khung thẳng của nó to hơn).
        let art = kids[1].geometric_bounds().unwrap();
        assert!((art.width() - mask.width()).abs() < 0.01 && (art.height() - mask.height()).abs() < 0.01, "{art:?} {mask:?}");
    }
}

/// Sai khổ: tự sửa bằng clip đúng khổ ở tâm, giữ vector (DanCard raster ở bước này).
#[test]
fn dan_card_sai_kho_tu_clip_giu_vector() {
    let mm = 72.0 / 25.4;
    let (mut s, ids) = card_sources(1, 1);
    let big = id_of(&run(&mut s, "shape.rectangle", json!({"x": 400.0 * mm, "y": 200.0 * mm, "width": 100.0 * mm, "height": 62.0 * mm})));
    run(&mut s, "select.set", json!({"ids": [ids[0].0, big.0]}));
    let out = run(&mut s, "dancard.layout", json!({"layout": "9.2x5.6"}));
    assert_eq!(out["models"].as_u64(), Some(2), "{out}");
    assert_eq!(images(&s), 0, "không raster");
    let d = &s.doc().unwrap().doc;
    for c in d.node(NodeId(out["groups"][0].as_u64().unwrap())).unwrap().children().unwrap() {
        assert!(c.clips());
        let b = c.children().unwrap()[0].geometric_bounds().unwrap();
        assert!((b.width().max(b.height()) / mm - 92.0).abs() < 1e-3 && (b.width().min(b.height()) / mm - 56.0).abs() < 1e-3, "{b:?}");
    }
}

/// Clip card trên card đặt nghiêng: mask theo khung transform (cùng góc, đúng 9,2 × 5,6).
#[test]
fn clip_card_theo_khung_transform() {
    let (mut s, id) = card(25.0);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    let g = NodeId(out["ids"][0].as_u64().unwrap());
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 25.0) && close(&f["width"], 9.2) && close(&f["height"], 5.6), "{f}");
    // Clip kích thước mặc định cũng theo khung transform.
    let (mut s, id) = card(25.0);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    let out = run(&mut s, "dancard.clipToSize", json!({"width": 9, "height": 5.4}));
    run(&mut s, "select.set", json!({"ids": [out["ids"][0]]}));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 25.0) && close(&f["width"], 9.0) && close(&f["height"], 5.4), "{f}");
}

/// Catalogue: trang nghiêng lẻ được dựng thẳng, giữ vector; trang ngang xoay 90° cho vừa cuốn dọc
/// giữ đúng hướng đang thấy (như DanCard đo khung trang), không bị bóp méo.
#[test]
fn catalogue_dung_thang_trang_giu_vector() {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 2000, "height": 1500, "colorMode": "cmyk"}));
    let mut ids = vec![];
    for i in 0..4 {
        // Trang 1 thiết kế ngang 210 × 148, xoay 90° thành đứng; trang 3 đặt lệch 3°.
        let (w, h) = if i == 1 { (210.0 * mm, 148.0 * mm) } else { (148.0 * mm, 210.0 * mm) };
        let id = id_of(&run(&mut s, "shape.rectangle", json!({"x": 20.0 + i as f64 * 200.0, "y": 20, "width": w, "height": h})));
        let turn = [0.0, 90.0, 0.0, 3.0][i];
        if turn != 0.0 {
            run(&mut s, "select.set", json!({"ids": [id.0]}));
            run(&mut s, "object.rotate", json!({"angle": turn}));
        }
        ids.push(id.0);
    }
    run(&mut s, "select.set", json!({"ids": ids}));
    run(&mut s, "dancard.catalogue", json!({"width": 14.8, "height": 21}));
    assert_eq!(images(&s), 0, "không raster");
    let d = &s.doc().unwrap().doc;
    let mut turns = vec![];
    for id in &ids {
        let n = d.node(NodeId(*id)).unwrap();
        let b = n.geometric_bounds().unwrap();
        // Trang đứng (hoặc nằm khi cuốn xoay): cạnh dài 21 cm, cạnh ngắn 14,8 cm (ruột có thể bóp 0,25 mm).
        let (long, short) = (b.width().max(b.height()) / mm, b.width().min(b.height()) / mm);
        assert!((long - 210.0).abs() < 1.0 && (short - 148.0).abs() < 1.0, "{long} × {short}: không được bóp méo");
        assert!(((n.bbox_angle / 90.0).round() * 90.0 - n.bbox_angle).abs() < 1e-6, "phần nghiêng lẻ đã bỏ: {}", n.bbox_angle);
        turns.push(n.bbox_angle);
    }
    // Trang ngang đã xoay đứng giữ hướng đó: lệch 90° so với các trang khác.
    assert!(((turns[1] - turns[0]).rem_euclid(180.0) - 90.0).abs() < 1e-6, "{turns:?}");
    assert!((turns[2] - turns[0]).rem_euclid(180.0) < 1e-6 && (turns[3] - turns[0]).rem_euclid(180.0) < 1e-6, "{turns:?}");
}

/// Dựng thẳng chỉ bỏ phần nghiêng lẻ (-45°, 45°]: xoay đúng 90° trong app giữ hướng đang thấy; góc
/// dò từ file .ai (không lưu góc) thì sau khi dựng thẳng, khung transform của object và mọi object
/// con đều thẳng.
#[test]
fn dung_thang_chi_bo_phan_nghieng_le() {
    let (s, id) = card(90.0);
    assert!(super::upright_affine(&s.doc().unwrap().doc, id).is_none(), "xoay đúng 90°: không dựng lại");
    let (s, id) = card(100.0);
    let d = &s.doc().unwrap().doc;
    let (_, tilt, detected) = super::upright_affine(d, id).unwrap();
    assert!((tilt - 10.0).abs() < 1e-9 && !detected);
    let up = super::upright_copy(d, id).unwrap();
    assert!((up.bbox_angle - 90.0).abs() < 1e-12, "còn đúng 90°: {}", up.bbox_angle);
    let b = up.geometric_bounds().unwrap();
    assert!((b.width() / CM - 5.6).abs() < 1e-6 && (b.height() / CM - 9.2).abs() < 1e-6, "{b:?}");
    // Group nghiêng 17° như file .ai: mọi góc lưu là 0, góc dò từ hình dạng.
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let a = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 9.2 * CM, "height": 5.6 * CM})));
    let m = id_of(&run(&mut s, "shape.rectangle", json!({"x": 110, "y": 110, "width": 20, "height": 20})));
    run(&mut s, "select.set", json!({"ids": [a.0, m.0]}));
    let g = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    run(&mut s, "object.rotate", json!({"angle": 17}));
    s.edit("quên góc", |d, _| {
        fn zero(n: &mut vectorcraft_doc::Node) {
            n.bbox_angle = 0.0;
            for c in n.children_mut().into_iter().flatten() {
                zero(std::sync::Arc::make_mut(c));
            }
        }
        zero(d.node_mut(g).unwrap());
        Ok(())
    })
    .unwrap();
    let (_, tilt, detected) = super::upright_affine(&s.doc().unwrap().doc, g).unwrap();
    assert!((tilt - 17.0).abs() < 1e-6 && detected, "{tilt}");
    s.edit("dựng thẳng", |d, _| Ok(super::make_upright(d, g))).unwrap();
    let n = s.doc().unwrap().doc.node(g).unwrap().clone();
    let b = n.geometric_bounds().unwrap();
    assert!((b.width() / CM - 9.2).abs() < 1e-6 && (b.height() / CM - 5.6).abs() < 1e-6, "nội dung đã thẳng: {b:?}");
    assert_eq!(n.bbox_angle, 0.0);
    assert!(n.children().unwrap().iter().all(|c| c.bbox_angle == 0.0), "object con không còn góc âm");
}

/// Clip và Resize theo khung transform: rộng × cao là ngang × dọc như đang thấy trên trang, kể cả
/// khi card đã xoay 90° trong app (mask không cắt nhầm chiều).
#[test]
fn clip_resize_card_xoay_90_theo_huong_dang_thay() {
    let (mut s, id) = card(90.0);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    let out = run(&mut s, "dancard.clipToSize", json!({"width": 5.6, "height": 9.2}));
    let g = NodeId(out["ids"][0].as_u64().unwrap());
    let d = &s.doc().unwrap().doc;
    let mask = d.node(g).unwrap().children().unwrap()[0].geometric_bounds().unwrap();
    assert!((mask.width() / CM - 5.6).abs() < 1e-6 && (mask.height() / CM - 9.2).abs() < 1e-6, "{mask:?}");
    let card_b = d.node(id).unwrap().geometric_bounds().unwrap();
    assert!((card_b.width() - mask.width()).abs() < 1e-6 && (card_b.height() - mask.height()).abs() < 1e-6, "mask phủ đúng card, không cắt");

    let (mut s, id) = card(90.0);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    run(&mut s, "dancard.resize", json!({"width": 6, "height": 10}));
    let b = s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap();
    assert!((b.width() / CM - 6.0).abs() < 1e-3 && (b.height() / CM - 10.0).abs() < 1e-3, "{b:?}");
    // Xoay 100°: co giãn dọc trục card nghiêng 10°, vẫn nhập theo hướng đang thấy.
    let (mut s, _) = card(100.0);
    run(&mut s, "dancard.resize", json!({"width": 6, "height": 10}));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 100.0) && close(&f["width"], 10.0) && close(&f["height"], 6.0), "{f}");
}

/// Card trắng: khổ lấy từ khung path không tô 92 × 56, chữ nhỏ lệch góc. Dàn card tự clip theo
/// khung đó (DanCard raster thành ảnh đúng khung): không báo sai khổ, các bản đặt theo khung.
#[test]
fn dan_card_tu_sua_theo_khung_khong_to() {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 330.0 * mm, "height": 480.0 * mm, "colorMode": "cmyk"}));
    let (x, y) = (400.0 * mm, 0.0);
    let frame_id = id_of(&run(&mut s, "shape.rectangle", json!({"x": x, "y": y, "width": 92.0 * mm, "height": 56.0 * mm})));
    run(&mut s, "paint.setFill", json!({"none": true, "ids": [frame_id.0]}));
    run(&mut s, "paint.setStroke", json!({"none": true, "ids": [frame_id.0]}));
    let logo = id_of(&run(&mut s, "shape.rectangle", json!({"x": x + 5.0 * mm, "y": y + 5.0 * mm, "width": 30.0 * mm, "height": 15.0 * mm})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 1, "y": 1, "k": 0}, "ids": [logo.0]}));
    run(&mut s, "select.set", json!({"ids": [frame_id.0, logo.0]}));
    let card_id = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "select.set", json!({"ids": [card_id.0]}));
    let out = run(&mut s, "dancard.layout", json!({"layout": "9.2x5.6"}));
    assert_eq!(images(&s), 0, "không raster");
    let d = &s.doc().unwrap().doc;
    let group = d.node(NodeId(out["groups"][0].as_u64().unwrap())).unwrap();
    let ab = d.artboards[0].rect;
    let mut masks = vec![];
    for c in group.children().unwrap() {
        assert!(c.clips());
        let m = c.children().unwrap()[0].geometric_bounds().unwrap();
        assert!((m.width().max(m.height()) / mm - 92.0).abs() < 1e-3 && (m.width().min(m.height()) / mm - 56.0).abs() < 1e-3, "{m:?}");
        masks.push(m);
    }
    // Bản đặt theo khung card: các khung không chồng nhau và cả khối nằm giữa artboard.
    for (i, a) in masks.iter().enumerate() {
        for b in &masks[i + 1..] {
            let ov = a.intersect(*b);
            assert!(ov.width() <= 1e-6 || ov.height() <= 1e-6, "{a:?} chồng {b:?}");
        }
    }
    let all = masks.iter().skip(1).fold(masks[0], |u, m| u.union(*m));
    assert!((all.center() - ab.center()).hypot() < 1e-3, "khối khung card nằm giữa artboard: {all:?} / {ab:?}");
}

/// Một card thử: nền 92 × 56 mm tô hồng ở (`x`, `y`) và một thanh đen 42 × 10 mm tràn ra ngoài cạnh
/// phải của nền 30 mm, gom thành group. Trả (group, nền, thanh).
fn card_with_tail(s: &mut Session, x: f64, y: f64) -> (NodeId, NodeId, NodeId) {
    let mm = 72.0 / 25.4;
    let bg = id_of(&run(s, "shape.rectangle", json!({"x": x, "y": y, "width": 92.0 * mm, "height": 56.0 * mm})));
    run(s, "paint.setFill", json!({"color": {"c": 0, "m": 0.2, "y": 0, "k": 0}, "ids": [bg.0]}));
    run(s, "paint.setStroke", json!({"none": true, "ids": [bg.0]}));
    let tail = id_of(&run(s, "shape.rectangle", json!({"x": x + 80.0 * mm, "y": y + 20.0, "width": 42.0 * mm, "height": 10.0 * mm})));
    run(s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 0, "k": 1}, "ids": [tail.0]}));
    run(s, "paint.setStroke", json!({"none": true, "ids": [tail.0]}));
    run(s, "select.set", json!({"ids": [bg.0, tail.0]}));
    let g = id_of(&run(s, "object.group", json!({})));
    (g, bg, tail)
}

/// Khung mask của clip group `g` (clipping path là con đầu tiên).
fn mask_of(s: &Session, g: NodeId) -> vectorcraft_geom::Rect {
    s.doc().unwrap().doc.node(g).unwrap().children().unwrap()[0].geometric_bounds().unwrap()
}

/// Clip card như `_dcClipToSizeCore` của DanCard: card có nền đúng khổ (phần tràn ra ngoài) clip theo
/// tâm nền; chọn object con của card đã clip thì lấy clip cha (đúng khổ thì giữ, khổ khác thì clip lại
/// cả clip group theo tâm mask); hai object con của cùng một card chỉ clip một lần.
#[test]
fn clip_card_neo_theo_nen_va_clip_cha() {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 1200, "height": 800, "colorMode": "cmyk"}));
    let (g, bg, tail) = card_with_tail(&mut s, 100.0, 100.0);
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    assert!(out["message"].as_str().unwrap().contains("dùng chung logic Clip"), "{out}");
    let clip = NodeId(out["ids"][0].as_u64().unwrap());
    let bgb = s.doc().unwrap().doc.node(bg).unwrap().geometric_bounds().unwrap();
    let m = mask_of(&s, clip);
    assert!((m.center() - bgb.center()).hypot() < 1e-6, "tâm mask là tâm nền, không lệch theo phần tràn: {m:?} / {bgb:?}");
    assert!((m.width() / mm - 92.0).abs() < 1e-6 && (m.height() / mm - 56.0).abs() < 1e-6, "{m:?}");
    // Chọn object con: clip cha đã đúng khổ thì giữ nguyên, không lồng thêm.
    run(&mut s, "select.set", json!({"ids": [tail.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    assert_eq!(out["ids"], json!([clip.0]), "{out}");
    // Khổ khác: clip lại cả clip group, tâm là tâm mask cũ; hai object con chỉ một lần.
    run(&mut s, "select.set", json!({"ids": [bg.0, tail.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({"width": 9, "height": 5.5}));
    let ids = out["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 1, "{out}");
    let outer = NodeId(ids[0].as_u64().unwrap());
    assert_ne!(outer, clip);
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.node(outer).unwrap().children().unwrap()[1].id, clip, "clip group cũ nằm trong clip group mới");
    let m2 = mask_of(&s, outer);
    assert!((m2.center() - m.center()).hypot() < 1e-6 && (m2.width() / mm - 90.0).abs() < 1e-6 && (m2.height() / mm - 55.0).abs() < 1e-6, "{m2:?}");
}

/// Clip card trên card đặt nghiêng 10° (xoay trong app): mask nghiêng theo trục card, đúng khổ, tâm là
/// tâm nền; card xoay đứng 90° thì khung đứng 5,6 × 9,2 như đang thấy.
#[test]
fn clip_card_nghieng_va_dung_theo_huong_dang_thay() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 1200, "height": 800, "colorMode": "cmyk"}));
    let (g, bg, _) = card_with_tail(&mut s, 100.0, 100.0);
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    run(&mut s, "object.rotate", json!({"angle": 10}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    let clip = NodeId(out["ids"][0].as_u64().unwrap());
    run(&mut s, "select.set", json!({"ids": [clip.0]}));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 10.0) && close(&f["width"], 9.2) && close(&f["height"], 5.6), "{f}");
    let bgc = s.doc().unwrap().doc.node(bg).unwrap().geometric_bounds().unwrap().center();
    let mc = mask_of(&s, clip).center();
    assert!((mc - bgc).hypot() < 1e-6, "{mc:?} / {bgc:?}");
    // Card xoay đứng: khung đứng như đang thấy trên trang.
    let (mut s, id) = card(90.0);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    let out = run(&mut s, "dancard.clipCard", json!({}));
    let m = mask_of(&s, NodeId(out["ids"][0].as_u64().unwrap()));
    assert!((m.width() / CM - 5.6).abs() < 1e-6 && (m.height() / CM - 9.2).abs() < 1e-6, "{m:?}");
}

/// Nút Clip như DanCard: tâm mask là tâm phần nhìn thấy, kể cả khi card đặt nghiêng và có object ẩn
/// lòi ra một bên (khung transform tính cả object ẩn); xong thì Expand Appearance.
#[test]
fn clip_theo_tam_phan_nhin_thay_va_expand() {
    let mm = 72.0 / 25.4;
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 1200, "height": 800, "colorMode": "cmyk"}));
    let bg = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 92.0 * mm, "height": 56.0 * mm})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0.2, "y": 0, "k": 0}, "ids": [bg.0]}));
    let hidden = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100.0 + 92.0 * mm, "y": 100, "width": 40.0 * mm, "height": 56.0 * mm})));
    run(&mut s, "select.set", json!({"ids": [bg.0, hidden.0]}));
    let g = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "select.set", json!({"ids": [hidden.0]}));
    run(&mut s, "object.hide", json!({}));
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    run(&mut s, "object.rotate", json!({"angle": 12}));
    let out = run(&mut s, "dancard.clipToSize", json!({"width": 9.2, "height": 5.6}));
    let clip = NodeId(out["ids"][0].as_u64().unwrap());
    let bgc = s.doc().unwrap().doc.node(bg).unwrap().geometric_bounds().unwrap().center();
    let mc = mask_of(&s, clip).center();
    assert!((mc - bgc).hypot() < 1e-6, "tâm phần nhìn thấy, không theo khung có object ẩn: {mc:?} / {bgc:?}");
    run(&mut s, "select.set", json!({"ids": [clip.0]}));
    let f = frame(&mut s);
    assert!(close(&f["angle"], 12.0) && close(&f["width"], 9.2) && close(&f["height"], 5.6), "{f}");
    // Expand Appearance như DanCard: hiệu ứng Offset Path thành hình thật.
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 600, "height": 600, "colorMode": "cmyk"}));
    let r = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 100, "height": 60})));
    run(&mut s, "paint.setStroke", json!({"none": true}));
    run(&mut s, "effect.apply", json!({"effect": "path.offsetPath", "params": {"offset": 10}}));
    run(&mut s, "select.set", json!({"ids": [r.0]}));
    run(&mut s, "dancard.clipToSize", json!({"width": 5, "height": 3}));
    let n = s.doc().unwrap().doc.node(r).unwrap().clone();
    assert!(n.appearance.effects.is_empty(), "đã Expand Appearance");
    assert!((n.geometric_bounds().unwrap().width() - 120.0).abs() < 0.1, "{:?}", n.geometric_bounds());
}

/// Dựng thẳng một group có góc dò từ hình dạng (Node::group không lưu góc) mà các object con đã xoay
/// trong app (góc đã lưu): khung của các object con đi theo phép xoay về thẳng, không giữ góc cũ.
#[test]
fn dung_thang_group_do_goc_giu_dung_khung_con() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let a = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 9.2 * CM, "height": 5.6 * CM})));
    // Object nhỏ ở giữa card: xoay cùng card vẫn nằm trong card.
    let b = id_of(&run(&mut s, "shape.rectangle", json!({"x": 210, "y": 170, "width": 40, "height": 20})));
    run(&mut s, "select.set", json!({"ids": [a.0, b.0]}));
    run(&mut s, "object.rotate", json!({"angle": 10}));
    let g = id_of(&run(&mut s, "object.group", json!({})));
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.node(g).unwrap().bbox_angle, 0.0, "group mới không lưu góc");
    assert!((d.node(a).unwrap().bbox_angle - 10.0).abs() < 1e-9);
    let (_, tilt, detected) = super::upright_affine(d, g).unwrap();
    assert!((tilt - 10.0).abs() < 1e-6 && detected, "{tilt}");
    s.edit("dựng thẳng", |d, _| Ok(super::make_upright(d, g))).unwrap();
    let d = &s.doc().unwrap().doc;
    for id in [g, a, b] {
        assert_eq!(d.node(id).unwrap().bbox_angle, 0.0, "{id:?}: khung thẳng như nội dung");
    }
    let ab = d.node(a).unwrap().geometric_bounds().unwrap();
    assert!((ab.width() / CM - 9.2).abs() < 1e-6 && (ab.height() / CM - 5.6).abs() < 1e-6, "{ab:?}");
}

/// File .ai có một layer `Art`: group `Pair` (hai hình vuông đen) và một hình vuông nữa ngoài
/// group mà trang PDF không vẽ: bản chỉnh sửa vẽ khác trang (VectorCraft gốc khi đó mở theo trang
/// PDF, rã group). `dates`: ngày sửa của trang và của bản chỉnh sửa.
fn ai_ve_khac_trang(dates: Option<(&str, &str)>) -> Vec<u8> {
    use vectorcraft_testkit::ai;
    let pair = format!("0 Ae\nu\n0 g\n{}f\n{}f\nU\n{}", ai::rect(10.0, 50.0, 40.0, 90.0), ai::rect(50.0, 50.0, 80.0, 90.0), ai::named("Pair"));
    let extra = format!("0 0 1 0 k\n{}f\n", ai::rect(120.0, 10.0, 160.0, 50.0));
    let data = ai::editing_data(200.0, 100.0, &ai::layer("Art", &format!("{pair}{extra}")));
    let page = "0 g 10 50 30 40 re f 50 50 30 40 re f";
    match dates {
        Some((page_date, data_date)) => ai::ai_dated(data.as_bytes(), page, page_date, data_date),
        None => ai::ai(data.as_bytes(), page),
    }
}

fn mo_file(s: &mut Session, bytes: &[u8], extra: Value) -> Value {
    mo_file_ten(s, "card.ai", bytes, extra)
}

fn mo_file_ten(s: &mut Session, name: &str, bytes: &[u8], extra: Value) -> Value {
    let mut p = json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(bytes)});
    if let (Some(p), Some(e)) = (p.as_object_mut(), extra.as_object()) {
        p.extend(e.clone());
    }
    run(s, "document.open", p)
}

fn ten_layer(s: &Session) -> Vec<String> {
    s.doc().unwrap().doc.layers.iter().map(|l| l.name.clone().unwrap_or_default()).collect()
}

#[test]
fn mo_file_ai_giu_nguyen_group_ca_khi_ve_khac_trang_pdf() {
    let mut s = Session::new();
    let r = mo_file(&mut s, &ai_ve_khac_trang(None), json!({}));
    assert_eq!(ten_layer(&s), ["Art"], "{r}");
    let art = s.doc().unwrap().doc.layers[0].children().unwrap().clone();
    let pair = art.iter().find(|n| n.name.as_deref() == Some("Pair")).expect("group Pair");
    assert!(matches!(&pair.kind, NodeKind::Group { children, .. } if children.len() == 2), "{:?}", pair.kind);
    assert_eq!(art.len(), 2, "group và hình vuông ngoài group");
    let w = r["warnings"].to_string();
    let first = r["warnings"][0].as_str().unwrap_or_default();
    assert!(first.starts_with("DanCard: giữ nguyên group") && first.contains("khác trang PDF"), "đứng đầu, thanh trạng thái hiện nó: {w}");
    assert!(!w.contains("all the same"), "lời báo tiếng Anh được nói lại bằng tiếng Việt: {w}");
    // `checked`: như VectorCraft gốc, theo trang PDF (group gom lại theo bản chỉnh sửa).
    let r = mo_file(&mut s, &ai_ve_khac_trang(None), json!({"editingData": "checked"}));
    assert_ne!(ten_layer(&s), ["Art"], "{r}");
    assert!(r["warnings"].to_string().contains("weren't read"), "{r}");
}

#[test]
fn mo_file_ai_co_opacity_mask_van_giu_group() {
    use vectorcraft_testkit::ai;
    // Hình đầu của group có opacity mask (mask trong ArtDictionary, hình của nó giữa X= và X+):
    // VectorCraft gốc không đọc được mask nên mở theo trang PDF, rã group; DanCard đọc tiếp, hình
    // hiện cả phần mask che, kèm lời báo.
    let mask = "%_/ArtDictionary :\n%_/Mask :\n%_1 /Bool (Linked) ,\n%_0 /Bool (Disabled) ,\n%_X=\n%_0 0 0 1 k\n%_10 50 m\n%_40 50 L\n\
                %_40 90 L\n%_10 90 L\n%_10 50 L\n%_f\n%_X+\n%_ (Art) ,\n%_1 /Bool (Clipping) ,\n%_0 /Bool (Inverted) ,\n%_; (AI9_Mask) ,\n%_;\n";
    let pair = format!("0 Ae\nu\n0 g\n{}f\n{mask}{}f\nU\n{}", ai::rect(10.0, 50.0, 40.0, 90.0), ai::rect(50.0, 50.0, 80.0, 90.0), ai::named("Pair"));
    let data = ai::editing_data(200.0, 100.0, &ai::layer("Art", &pair));
    let file = ai::ai(data.as_bytes(), "0 g 10 50 30 40 re f 50 50 30 40 re f");
    let mut s = Session::new();
    let r = mo_file(&mut s, &file, json!({}));
    assert_eq!(ten_layer(&s), ["Art"], "{r}");
    let art = s.doc().unwrap().doc.layers[0].children().unwrap().clone();
    let pair = art.iter().find(|n| n.name.as_deref() == Some("Pair")).expect("group Pair");
    assert!(matches!(&pair.kind, NodeKind::Group { children, .. } if children.len() == 2), "{:?}", pair.kind);
    let first = r["warnings"][0].as_str().unwrap_or_default();
    assert!(first.starts_with("DanCard: 1 object có opacity mask"), "{r}");
}

#[test]
fn trang_pdf_sua_sau_ban_chinh_sua_thi_mo_theo_trang() {
    let mut s = Session::new();
    let (truoc, sau) = ("D:20261002101134+08'00'", "D:20261002111134+08'00'");
    let r = mo_file(&mut s, &ai_ve_khac_trang(Some((sau, truoc))), json!({}));
    assert_ne!(ten_layer(&s), ["Art"], "{r}");
    assert_eq!(r["warnings"][0], super::STALE_NOTE, "{r}");
    // Lưu cùng lúc (file Illustrator lưu): theo bản chỉnh sửa.
    let r = mo_file(&mut s, &ai_ve_khac_trang(Some((truoc, truoc))), json!({}));
    assert_eq!(ten_layer(&s), ["Art"], "{r}");
}

#[test]
fn mo_file_eps_cung_giu_nguyen_group() {
    use vectorcraft_testkit::ai;
    // Bản chỉnh sửa như file .ai trên, trang PostScript chỉ vẽ group.
    let pair = format!("0 Ae\nu\n0 g\n{}f\n{}f\nU\n{}", ai::rect(10.0, 50.0, 40.0, 90.0), ai::rect(50.0, 50.0, 80.0, 90.0), ai::named("Pair"));
    let extra = format!("0 0 1 0 k\n{}f\n", ai::rect(120.0, 10.0, 160.0, 50.0));
    let data = ai::editing_data(200.0, 100.0, &ai::layer("Art", &format!("{pair}{extra}")));
    let eps = ai::eps(data.as_bytes(), "0 setgray 10 50 30 40 rectfill 50 50 30 40 rectfill\n");
    let mut s = Session::new();
    let r = mo_file_ten(&mut s, "card.eps", &eps, json!({}));
    assert_eq!(ten_layer(&s), ["Art"], "{r}");
    assert!(r["warnings"][0].as_str().unwrap_or_default().starts_with("DanCard: giữ nguyên group"), "{r}");
    // `checked`: như VectorCraft gốc, theo trang.
    let r = mo_file_ten(&mut s, "card.eps", &eps, json!({"editingData": "checked"}));
    assert_ne!(ten_layer(&s), ["Art"], "{r}");
}
