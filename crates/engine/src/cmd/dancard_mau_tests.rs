//! DanCard › Dàn theo mẫu: học mẫu → áp mẫu trên tài liệu dựng sẵn, các số tham chiếu của spec
//! (xếp tờ, bù lô, đối xứng mặt sau, tên PDF), copy ghi chú và lưu PDF.

use serde_json::{Value, json};
use vectorcraft_doc::{Artboard, Document, NodeId, NodeKind};
use vectorcraft_geom::Rect;

use super::{GAP, HALF, LAYER, Slot, clean_suffix, fill_batch, output_positions, pdf_name};
use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Hướng của bản đã dàn `id` (độ, ngược chiều kim đồng hồ, 0–360): bản là vector (không raster),
/// đứng thẳng ở 0° rồi xoay theo vị trí, nên góc khung transform của nó là hướng artwork.
fn copy_angle(d: &Document, id: NodeId) -> f64 {
    let n = d.node(id).unwrap();
    assert!(!matches!(n.kind, NodeKind::Image(_)), "bản đã dàn phải giữ vector");
    n.bbox_angle.rem_euclid(360.0)
}

/// Quên góc xoay của `ids` như file .ai/PDF (không lưu góc): Object › Transform › Reset Bounding Box.
fn forget_angles(s: &mut Session, ids: &[NodeId]) {
    run(s, "select.set", json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}));
    run(s, "object.resetBoundingBox", json!({}));
}

fn same_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    !(1e-6..=360.0 - 1e-6).contains(&d)
}

/// Tài liệu CMYK 800 × 600 pt.
fn doc() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    s
}

/// Card 100 × 60 pt nền vàng nhạt, ô đen 20 × 20 ở góc trên trái (hoặc chữ "A" khi `text`), gom
/// thành group rồi xoay `angle` độ ngược chiều kim đồng hồ.
fn card(s: &mut Session, x: f64, y: f64, angle: f64, text: bool) -> NodeId {
    let bg = id_of(&run(s, "shape.rectangle", json!({"x": x, "y": y, "width": 100, "height": 60})));
    run(s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 0.2, "k": 0}, "ids": [bg.0]}));
    let mark = if text {
        id_of(&run(s, "text.create", json!({"x": x + 10.0, "y": y + 30.0, "text": "A", "size": 20})))
    } else {
        let m = id_of(&run(s, "shape.rectangle", json!({"x": x + 4.0, "y": y + 4.0, "width": 20, "height": 20})));
        run(s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 0, "k": 1}, "ids": [m.0]}));
        m
    };
    run(s, "paint.setStroke", json!({"none": true, "ids": [bg.0]}));
    run(s, "select.set", json!({"ids": [bg.0, mark.0]}));
    let g = id_of(&run(s, "object.group", json!({})));
    run(s, "select.set", json!({"ids": [g.0]}));
    if angle != 0.0 {
        run(s, "object.rotate", json!({"angle": angle}));
    }
    g
}

/// File pon `w` × `h` pt (artboard ở gốc) có một dấu 10 × 10 pt ở (5, 5), lưu trong thư mục tạm.
fn pon_file(name: &str, w: f64, h: f64) -> String {
    let dir = std::env::temp_dir().join(format!("dancard-mau-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pon.vectorcraft").to_string_lossy().to_string();
    let _ = std::fs::remove_file(&path);
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": w, "height": h, "colorMode": "cmyk"}));
    run(&mut s, "shape.rectangle", json!({"x": 5, "y": 5, "width": 10, "height": 10}));
    run(&mut s, "document.save", json!({"path": path}));
    path
}

/// Góc trên trái vùng xếp tờ: canvas ± 7200 pt, chừa 10 mm.
fn corner() -> f64 {
    -HALF + GAP
}

#[test]
fn xep_to_theo_so_tham_chieu() {
    let far = Rect::new(5000.0, 5000.0, 5100.0, 5100.0);
    // §13.1: tờ 600 × 400 đầu tiên ở góc trên trái canvas.
    let one = output_positions((600.0, 400.0), None, 1, &[], far).unwrap();
    let (f, b) = one[0];
    assert!(near(f.x0, -7200.0 + 10.0 * 72.0 / 25.4) && near(f.y0, f.x0) && near(f.width(), 600.0) && near(f.height(), 400.0), "{f:?}");
    assert!(b.is_none());
    // §13.2: 26 tờ 1 mặt, 22 tờ một hàng.
    let many = output_positions((600.0, 400.0), None, 26, &[], far).unwrap();
    for (i, (f, _)) in many.iter().enumerate() {
        let (col, row) = ((i % 22) as f64, (i / 22) as f64);
        assert!(near(f.x0, corner() + col * (600.0 + GAP)) && near(f.y0, corner() + row * (400.0 + GAP)), "tờ {i}: {f:?}");
    }
    // Hai mặt 600 × 400 + 450 × 500: mặt sau cách mặt trước 10 mm, cặp 12 sang hàng thứ hai.
    let pairs = output_positions((600.0, 400.0), Some((450.0, 500.0)), 14, &[], far).unwrap();
    for (f, b) in &pairs {
        let b = b.unwrap();
        assert!(near(b.x0, f.x1 + GAP) && near(b.y0, f.y0) && near(b.width(), 450.0) && near(b.height(), 500.0), "{f:?} {b:?}");
    }
    assert!(near(pairs[11].0.y0, corner()) && near(pairs[12].0.y0, corner() + 500.0 + GAP) && near(pairs[12].0.x0, corner()), "{:?}", pairs[12]);
    // Artboard có sẵn đúng chỗ tờ đầu: tờ đầu dời sang phải.
    let taken = Rect::new(corner(), corner(), corner() + 600.0, corner() + 400.0);
    let pushed = output_positions((600.0, 400.0), None, 1, &[taken], far).unwrap();
    assert!(near(pushed[0].0.x0, taken.x1 + GAP), "{pushed:?}");
    // Khổ lớn hơn canvas, hoặc không còn chỗ.
    assert!(output_positions((20000.0, 400.0), None, 1, &[], far).unwrap_err().contains("vượt giới hạn canvas"));
    assert!(output_positions((10000.0, 10000.0), None, 2, &[], far).unwrap_err().contains("Không còn chỗ"));
}

#[test]
fn bu_lo_doi_xung_va_ten_pdf() {
    // §13.4: 8 vị trí, lô cuối s9, s10, s11 bù bằng con cuối rồi gần cuối.
    let (filled, extra) = fill_batch(&[9, 10, 11], 8);
    assert_eq!((filled, extra), (vec![9, 10, 11, 11, 10, 11, 10, 11], 5));
    assert_eq!(fill_batch(&[1, 2, 3], 3), (vec![1, 2, 3], 0));
    assert_eq!(fill_batch(&[7], 3), (vec![7, 7, 7], 2));
    // §13.3: mặt sau lật dx, 90° thành 270°, 0°/180° giữ nguyên.
    let s = Slot { dx: -70.0, dy: 5.0, w: 100.0, h: 50.0, angle: 90.0, tilt: 0.0 };
    assert_eq!(s.mirrored(), Slot { dx: 70.0, angle: 270.0, ..s });
    // Khung nghiêng: mặt sau nghiêng ngược lại.
    assert_eq!(Slot { tilt: 12.0, ..s }.mirrored().tilt, -12.0);
    assert!(near(Slot { angle: 180.0, ..s }.mirrored().angle, 180.0) && near(Slot { angle: 0.0, ..s }.mirrored().angle, 0.0));
    // §13.5
    assert_eq!(pdf_name(1, &clean_suffix(" thiep/sinh:nhat ")), "file 1 - thiep-sinh-nhat.pdf");
    assert_eq!(pdf_name(3, &clean_suffix("")), "file 3.pdf");
}

#[test]
fn hoc_mau_so_anh_roi_ap_mau_mot_mat() {
    let mut s = doc();
    // Bố cục dàn tay như file .ai (không lưu góc): con thẳng, con xoay 90°, con xoay 180°.
    let a = card(&mut s, 1000.0, 100.0, 0.0, false);
    let b = card(&mut s, 1200.0, 100.0, 90.0, false);
    let c = card(&mut s, 1400.0, 100.0, 180.0, false);
    forget_angles(&mut s, &[b, c]);
    let ids = json!([a.0, b.0, c.0]);
    let pending = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": ids}));
    assert_eq!((&pending["pending"], &pending["ids"]), (&json!(true), &ids), "{pending}");
    assert_eq!(pending["template"]["state"], json!("pending"));
    // Con chuẩn là con thẳng: góc so với nó.
    let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": ids, "reference": a.0}));
    assert_eq!(learned["angleSource"], json!("render"), "{learned}");
    assert_eq!(learned["angles"], json!([0.0, 90.0, 180.0]), "{learned}");
    assert!(learned["message"].as_str().unwrap().starts_with("OK: Đã HỌC 1 mặt (3 vị trí)"));
    // Con chuẩn là con xoay 90°: mọi góc lùi 90°.
    let other = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": ids, "reference": b.0}));
    assert_eq!(other["angles"], json!([270.0, 0.0, 90.0]), "{other}");
    // Khung và độ lệch so với tâm bố cục (1250, 130).
    let t = learned["template"].clone();
    let slot = |i: usize, k: &str| t["slots"][i][k].as_f64().unwrap();
    assert!(near(slot(0, "dx"), -200.0) && near(slot(0, "dy"), 0.0) && near(slot(1, "dx"), 0.0) && near(slot(2, "dx"), 200.0), "{t}");
    assert!(near(slot(1, "w"), 60.0) && near(slot(1, "h"), 100.0) && near(slot(0, "w"), 100.0), "{t}");

    // Áp mẫu cho một con nguồn trên artboard.
    let src = card(&mut s, 100.0, 300.0, 0.0, false);
    run(&mut s, "select.set", json!({"ids": [src.0]}));
    let pending_err = s.execute("dancard.applyTemplate", &json!({"template": pending["template"], "ponFront": "x"})).unwrap_err().to_string();
    assert!(pending_err.contains("chưa hoàn tất"), "{pending_err}");
    let no_pon = s.execute("dancard.applyTemplate", &json!({"template": t})).unwrap_err().to_string();
    assert!(no_pon.contains("Chưa chọn pon MẶT TRƯỚC"), "{no_pon}");
    let undo = s.doc().unwrap().history.undo.len();
    let pon = pon_file("mot-mat", 600.0, 300.0);
    let out = run(&mut s, "dancard.applyTemplate", json!({"template": t, "ponFront": pon}));
    assert_eq!((out["sheets"].as_u64(), out["firstArtboard"].as_u64()), (Some(1), Some(0)), "{out}");
    assert_eq!(out["notes"], json!([]), "{out}");
    assert!(out["message"].as_str().unwrap().starts_with("OK: Đã dàn 1 mặt 1 nguồn thành 1 artboard (1 mẫu/1 artboard)."), "{out}");
    let st = s.doc().unwrap();
    let d = &st.doc;
    // Artboard cũ đã xoá; tờ mới đúng khổ pon ở góc canvas.
    assert_eq!(d.artboards.len(), 1);
    let sheet = d.artboards[0].rect;
    assert!(near(sheet.x0, corner()) && near(sheet.y0, corner()) && near(sheet.width(), 600.0) && near(sheet.height(), 300.0), "{sheet:?}");
    let ids: Vec<NodeId> = out["ids"].as_array().unwrap().iter().map(|v| NodeId(v.as_u64().unwrap())).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(st.selection.objects, ids, "chọn sẵn các bản đã dàn");
    let c0 = sheet.center();
    for (i, id) in ids.iter().enumerate() {
        let n = d.node(*id).unwrap();
        let bb = n.geometric_bounds().unwrap();
        assert!(near(bb.width(), slot(i, "w")) && near(bb.height(), slot(i, "h")), "vị trí {i}: {bb:?}");
        assert!(near(bb.center().x, c0.x + slot(i, "dx")) && near(bb.center().y, c0.y + slot(i, "dy")), "vị trí {i}: {bb:?}");
        assert!(same_angle(copy_angle(d, *id), slot(i, "angle")), "vị trí {i}: {}", copy_angle(d, *id));
    }
    // Layer kết quả: 3 bản + pon (trên cùng), dấu pon giữ khoảng cách tới tâm tờ.
    let layer = d.layers.iter().find(|l| l.name.as_deref() == Some(LAYER)).unwrap();
    let kids = layer.children().unwrap();
    assert_eq!(kids.len(), 4);
    let mark = kids.last().unwrap().geometric_bounds().unwrap().center();
    assert!(near(mark.x, c0.x - 290.0) && near(mark.y, c0.y - 140.0), "{mark:?}");
    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    let d = &s.doc().unwrap().doc;
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
    assert_eq!(d.artboards.len(), 1);
    assert!(near(d.artboards[0].rect.x0, 0.0));
    assert!(d.layers.iter().all(|l| l.name.as_deref() != Some(LAYER)));
}

#[test]
fn hoc_mau_doc_goc_chu_va_ap_hai_mat_doi_xung() {
    let mut s = doc();
    let a = card(&mut s, 1000.0, 100.0, 0.0, true);
    let b = card(&mut s, 1200.0, 100.0, 90.0, true);
    forget_angles(&mut s, &[b]);
    let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "mirror", "ids": [a.0, b.0], "reference": a.0}));
    assert_eq!((&learned["angleSource"], &learned["angles"]), (&json!("geometry"), &json!([0.0, 90.0])), "{learned}");
    let t = learned["template"].clone();
    assert_eq!((&t["mode"], &t["backMirror"]), (&json!("two"), &json!(true)));
    let slot = |i: usize, k: &str| t["slots"][i][k].as_f64().unwrap();
    // Nguồn: cột trái mặt trước, cột phải mặt sau.
    let front = card(&mut s, 100.0, 300.0, 0.0, false);
    let back = card(&mut s, 400.0, 300.0, 0.0, false);
    run(&mut s, "select.set", json!({"ids": [front.0, back.0]}));
    let no_back =
        s.execute("dancard.applyTemplate", &json!({"template": t, "ponFront": pon_file("hai-mat-f", 400.0, 300.0)})).unwrap_err().to_string();
    assert!(no_back.contains("MẶT SAU"), "{no_back}");
    let out = run(
        &mut s,
        "dancard.applyTemplate",
        json!({"template": t, "ponFront": pon_file("hai-mat-f", 400.0, 300.0), "ponBack": pon_file("hai-mat-b", 300.0, 350.0)}),
    );
    assert_eq!(out["sheets"].as_u64(), Some(2), "{out}");
    let d = &s.doc().unwrap().doc;
    // F1, B1: mặt sau bên phải mặt trước, cùng mép trên, khổ pon mặt sau.
    let (fr, br) = (d.artboards[0].rect, d.artboards[1].rect);
    assert!(near(br.x0, fr.x1 + GAP) && near(br.y0, fr.y0) && near(br.width(), 300.0) && near(br.height(), 350.0), "{fr:?} {br:?}");
    let ids: Vec<NodeId> = out["ids"].as_array().unwrap().iter().map(|v| NodeId(v.as_u64().unwrap())).collect();
    assert_eq!(ids.len(), 4);
    for (k, id) in ids.iter().enumerate() {
        let (i, sheet, sign) = if k < 2 { (k, fr, 1.0) } else { (k - 2, br, -1.0) };
        let bb = d.node(*id).unwrap().geometric_bounds().unwrap();
        assert!(near(bb.center().x, sheet.center().x + sign * slot(i, "dx")) && near(bb.center().y, sheet.center().y + slot(i, "dy")), "{k}: {bb:?}");
        // Mặt sau: 90° thành 270°.
        let want = if k < 2 { slot(i, "angle") } else { (360.0 - slot(i, "angle")).rem_euclid(360.0) };
        assert!(same_angle(copy_angle(d, *id), want), "{k}: {}", copy_angle(d, *id));
    }
    // Hai cột không bằng nhau.
    let lone = card(&mut s, 100.0, 500.0, 0.0, false);
    run(&mut s, "select.set", json!({"ids": [front.0, back.0, lone.0]}));
    let uneven = s
        .execute(
            "dancard.applyTemplate",
            &json!({"template": t, "ponFront": pon_file("hai-mat-f", 400.0, 300.0), "ponBack": pon_file("hai-mat-b", 300.0, 350.0)}),
        )
        .unwrap_err()
        .to_string();
    assert!(uneven.contains("hai cột bằng nhau"), "{uneven}");
}

#[test]
fn nhieu_nguon_mot_artboard_bu_con_cuoi() {
    let mut s = doc();
    let t = json!({
        "format": "danTheoMau", "version": 1, "state": "ready", "mode": "one", "backMirror": false,
        "slots": [
            {"dx": -60, "dy": 0, "w": 100, "h": 60, "angle": 0},
            {"dx": 60, "dy": 0, "w": 100, "h": 60, "angle": 0},
        ],
    });
    let srcs: Vec<u64> = (0..3u32).map(|i| card(&mut s, 50.0 + 150.0 * f64::from(i), 300.0, 0.0, false).0).collect();
    run(&mut s, "select.set", json!({"ids": srcs}));
    let pon = pon_file("nhieu", 300.0, 200.0);
    let out = run(&mut s, "dancard.applyTemplate", json!({"template": t, "ponFront": pon, "multiSourcePerArtboard": true}));
    assert_eq!(out["sheets"].as_u64(), Some(2), "{out}");
    let msg = out["message"].as_str().unwrap();
    assert!(msg.contains("(nhiều mẫu/1 artboard)") && msg.contains("Đã bù 1 slot"), "{msg}");
    assert_eq!(out["ids"].as_array().unwrap().len(), 4);
    // Dữ liệu mẫu hỏng.
    let mut bad = t.clone();
    bad["slots"][1]["w"] = json!(0);
    let e = s.execute("dancard.applyTemplate", &json!({"template": bad, "ponFront": pon})).unwrap_err().to_string();
    assert!(e.contains("không hợp lệ ở vị trí 2"), "{e}");
    let e = s.execute("dancard.applyTemplate", &json!({"template": null, "ponFront": pon})).unwrap_err().to_string();
    assert!(e.contains("Chưa có dữ liệu mẫu"), "{e}");
}

/// Tài liệu có `n` artboard 300 × 200 pt cách nhau 50 pt, artboard i có chữ nhật ở (20, 20) của nó.
fn boards(n: u32, with_art: bool) -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 300, "height": 200, "colorMode": "cmyk"}));
    s.edit("artboards", |d, _| {
        let base = d.artboards[0].clone();
        d.artboards = (0..n)
            .map(|i| Artboard {
                id: i + 1,
                name: format!("Artboard {}", i + 1),
                rect: Rect::new(f64::from(i) * 350.0, 0.0, f64::from(i) * 350.0 + 300.0, 200.0),
                ..base.clone()
            })
            .collect();
        Ok(())
    })
    .unwrap();
    if with_art {
        for i in 0..n {
            run(&mut s, "shape.rectangle", json!({"x": f64::from(i) * 350.0 + 20.0, "y": 20, "width": 100, "height": 50}));
        }
    }
    s
}

fn text_of(d: &Document, id: NodeId) -> String {
    let n = d.node(id).unwrap();
    let NodeKind::Text(t) = &n.kind else { panic!("chữ") };
    t.plain_text()
}

#[test]
fn copy_ghi_chu_hai_mat_va_mot_mat() {
    let mut s = boards(6, false);
    let note = id_of(&run(&mut s, "text.create", json!({"x": 20, "y": 40, "text": "Ghi chú", "size": 12})));
    run(&mut s, "select.set", json!({"ids": [note.0]}));
    let src = s.doc().unwrap().doc.node(note).unwrap().visual_bounds().unwrap();
    let e = s.execute("dancard.copyNotes", &json!({"prefixes": "F1, F2", "twoSided": true})).unwrap_err().to_string();
    assert!(e.contains("Cần 3 nội dung cho artboard 1, 3"), "{e}");
    let undo = s.doc().unwrap().history.undo.len();
    let out = run(&mut s, "dancard.copyNotes", json!({"prefixes": "F1, F2, F3", "twoSided": true}));
    assert!(out["message"].as_str().unwrap().contains("sang 2 artboard mặt trước tiếp theo."), "{out}");
    let d = &s.doc().unwrap().doc;
    assert_eq!(text_of(d, note), "F1 - Ghi chú");
    let copies: Vec<NodeId> = out["ids"].as_array().unwrap().iter().map(|v| NodeId(v.as_u64().unwrap())).collect();
    assert_eq!(copies.len(), 2);
    let src_now = d.node(note).unwrap().visual_bounds().unwrap();
    for (copy, (board, text)) in copies.iter().zip([(2usize, "F2 - Ghi chú"), (4, "F3 - Ghi chú")]) {
        assert_eq!(text_of(d, *copy), text);
        let b = d.node(*copy).unwrap().visual_bounds().unwrap();
        let r = d.artboards[board].rect;
        assert!(near(b.x0 - r.x0, src_now.x0) && near(b.y0 - r.y0, src_now.y0), "{b:?} {r:?}");
    }
    // Một bước hoàn tác: ghi chú gốc trở lại như cũ.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
    assert_eq!(text_of(&s.doc().unwrap().doc, note), "Ghi chú");
    // Bài 1 mặt, không nội dung: copy nguyên sang 5 artboard sau.
    run(&mut s, "select.set", json!({"ids": [note.0]}));
    let out = run(&mut s, "dancard.copyNotes", json!({"prefixes": "", "twoSided": false}));
    assert_eq!(out["ids"].as_array().unwrap().len(), 5, "{out}");
    let d = &s.doc().unwrap().doc;
    let last = NodeId(out["ids"][4].as_u64().unwrap());
    let b = d.node(last).unwrap().visual_bounds().unwrap();
    assert!(near(b.x0 - d.artboards[5].rect.x0, src.x0) && text_of(d, last) == "Ghi chú", "{b:?}");
    // Ghi chú trên mặt sau (artboard 2) khi copy 2 mặt.
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "select.set", json!({"ids": [note.0]}));
    run(&mut s, "object.move", json!({"dx": 350, "dy": 0}));
    let e = s.execute("dancard.copyNotes", &json!({"twoSided": true})).unwrap_err().to_string();
    assert!(e.contains("mặt trước (artboard số lẻ)"), "{e}");
}

#[test]
fn luu_pdf_da_dan_tung_cap_va_tung_artboard() {
    let pages = |b: &[u8]| {
        let t = String::from_utf8_lossy(b).replace("/Type /Page", "/Type/Page");
        t.match_indices("/Type/Page").filter(|(i, _)| t.as_bytes().get(i + 10) != Some(&b's')).count()
    };
    let dir = std::env::temp_dir().join(format!("dancard-mau-pdf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let folder = dir.to_string_lossy().to_string();
    let mut s = boards(4, true);
    let out = run(&mut s, "dancard.saveTemplatePdf", json!({"mode": "pair", "suffix": " thiep/sinh:nhat ", "folder": folder}));
    assert_eq!(out["files"], json!(["file 1 - thiep-sinh-nhat.pdf", "file 2 - thiep-sinh-nhat.pdf"]), "{out}");
    let pdf = std::fs::read(dir.join("file 1 - thiep-sinh-nhat.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF") && pages(&pdf) == 2, "2 artboard / PDF");
    // Không ghi đè file có sẵn.
    let e = s.execute("dancard.saveTemplatePdf", &json!({"mode": "pair", "suffix": "thiep-sinh-nhat", "folder": folder})).unwrap_err().to_string();
    assert!(e.contains("File đã tồn tại: file 1 - thiep-sinh-nhat.pdf."), "{e}");
    let out = run(&mut s, "dancard.saveTemplatePdf", json!({"mode": "each", "folder": folder}));
    assert_eq!(out["saved"].as_u64(), Some(4), "{out}");
    assert_eq!(pages(&std::fs::read(dir.join("file 4.pdf")).unwrap()), 1);
    // Số artboard lẻ không lưu theo cặp; thiếu thư mục; artboard trống bị báo.
    let mut odd = boards(3, true);
    let e = odd.execute("dancard.saveTemplatePdf", &json!({"mode": "pair", "folder": folder})).unwrap_err().to_string();
    assert!(e.contains("số artboard chẵn"), "{e}");
    assert!(odd.execute("dancard.saveTemplatePdf", &json!({"folder": ""})).unwrap_err().to_string().contains("Chưa chọn thư mục"));
    let mut empty = boards(2, false);
    let e = empty.execute("dancard.saveTemplatePdf", &json!({"mode": "pair", "suffix": "trong", "folder": folder})).unwrap_err().to_string();
    assert!(e.contains("Không lưu được PDF nào") && e.contains("Không copy đủ object"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bố cục dàn ngay trong VectorCraft (góc xoay được lưu): góc nghiêng lẻ đọc từ khung transform; con
/// xoay đúng 90° giữ như đang thấy trên trang (khung 60 × 100) và hướng artwork so với con chuẩn đọc
/// như DanCard (ở đây so ảnh); áp mẫu giữ vector và đúng góc từng vị trí.
#[test]
fn hoc_mau_theo_khung_transform_va_ap_nghieng() {
    let mut s = doc();
    let a = card(&mut s, 1000.0, 100.0, 0.0, false);
    let b = card(&mut s, 1200.0, 100.0, 90.0, false);
    let c = card(&mut s, 1400.0, 100.0, 15.0, false);
    let ids = json!([a.0, b.0, c.0]);
    // Hướng artwork tính so với con chuẩn: lấy con xoay 90° làm chuẩn thì hai con kia là 270°.
    for (r, want) in [(a, [0.0, 90.0, 0.0]), (b, [270.0, 0.0, 270.0]), (c, [0.0, 90.0, 0.0])] {
        let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": ids, "reference": r.0}));
        assert_eq!(learned["angles"], json!(want), "{learned}");
        let t = &learned["template"];
        let slot = |i: usize, k: &str| t["slots"][i][k].as_f64().unwrap();
        // Chỉ phần nghiêng lẻ là góc nghiêng của vị trí; khung ngang × dọc như đang thấy trên trang.
        assert!(near(slot(0, "tilt"), 0.0) && near(slot(1, "tilt"), 0.0) && (slot(2, "tilt") - 15.0).abs() < 1e-6, "{t}");
        for (i, (w, h)) in [(100.0, 60.0), (60.0, 100.0), (100.0, 60.0)].into_iter().enumerate() {
            assert!(near(slot(i, "w"), w) && near(slot(i, "h"), h), "vị trí {i}: {t}");
        }
    }
    let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": ids, "reference": a.0}));
    let t = learned["template"].clone();
    // Nguồn xoay 30° trong app: được dựng thẳng trước khi dàn.
    let src = card(&mut s, 100.0, 300.0, 30.0, false);
    run(&mut s, "select.set", json!({"ids": [src.0]}));
    let out = run(&mut s, "dancard.applyTemplate", json!({"template": t, "ponFront": pon_file("nghieng", 700.0, 300.0)}));
    let d = &s.doc().unwrap().doc;
    let ids: Vec<NodeId> = out["ids"].as_array().unwrap().iter().map(|v| NodeId(v.as_u64().unwrap())).collect();
    assert_eq!(ids.len(), 3);
    let c0 = d.artboards[0].rect.center();
    for (i, (id, want)) in ids.iter().zip([0.0, 90.0, 15.0]).enumerate() {
        assert!((copy_angle(d, *id) - want).abs() < 1e-6, "vị trí {i}: {}", copy_angle(d, *id));
        let ob = d.oriented_bounds(&[*id], false).unwrap();
        assert!((ob.rect.width() - 100.0).abs() < 0.75 && (ob.rect.height() - 60.0).abs() < 0.75, "vị trí {i}: {ob:?}");
        let slot = |k: &str| t["slots"][i][k].as_f64().unwrap();
        assert!((ob.center().x - (c0.x + slot("dx"))).abs() < 1e-6 && (ob.center().y - (c0.y + slot("dy"))).abs() < 1e-6, "vị trí {i}");
    }
}

/// Group có object con ẩn lệch tâm, nghiêng 30° trong app: khung transform (xoay quanh tâm của nó)
/// tính cả object ẩn, khung vị trí thì chỉ phần nhìn thấy. Tâm vị trí học được phải là tâm phần nhìn
/// thấy trên trang, để lúc áp mẫu nghiêng quanh tâm đó đặt đúng chỗ bản gốc.
#[test]
fn hoc_mau_tam_vi_tri_theo_phan_nhin_thay() {
    let mut s = doc();
    let a = card(&mut s, 1000.0, 100.0, 0.0, false);
    let vis = id_of(&run(&mut s, "shape.rectangle", json!({"x": 1200, "y": 100, "width": 100, "height": 60})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 1, "k": 0}, "ids": [vis.0]}));
    let hidden = id_of(&run(&mut s, "shape.rectangle", json!({"x": 1320, "y": 100, "width": 40, "height": 60})));
    run(&mut s, "select.set", json!({"ids": [vis.0, hidden.0]}));
    let g = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "select.set", json!({"ids": [hidden.0]}));
    run(&mut s, "object.hide", json!({}));
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    run(&mut s, "object.rotate", json!({"angle": 30}));
    let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": [a.0, g.0], "reference": a.0}));
    let t = &learned["template"];
    let slot = |i: usize, k: &str| t["slots"][i][k].as_f64().unwrap();
    let d = &s.doc().unwrap().doc;
    let ca = d.node(a).unwrap().geometric_bounds().unwrap().center();
    // Tâm hình chữ nhật nhìn thấy (xoay quanh tâm nào thì tâm khung thẳng của nó vẫn là tâm của nó).
    let cv = d.node(vis).unwrap().geometric_bounds().unwrap().center();
    assert!(near(slot(1, "dx") - slot(0, "dx"), cv.x - ca.x) && near(slot(1, "dy") - slot(0, "dy"), cv.y - ca.y), "{t}");
    assert!(near(slot(1, "tilt"), 30.0) && near(slot(1, "w"), 100.0) && near(slot(1, "h"), 60.0), "{t}");
}

/// Hướng artwork đọc bằng so ảnh (card không có ảnh/chữ) trên con nghiêng có phần ẩn lệch một bên:
/// ảnh phải chụp đúng bản đã dựng thẳng, không lệch theo khung đã đặt về trang (ảnh lệch thì mất dấu
/// đen và đoán thành 0°). Con chuẩn xoay đứng 90°, nên đáp án đúng là 270°.
#[test]
fn hoc_mau_so_anh_con_nghieng_co_phan_an() {
    let mut s = doc();
    let a = card(&mut s, 1000.0, 100.0, 90.0, false);
    let bg = id_of(&run(&mut s, "shape.rectangle", json!({"x": 1200, "y": 100, "width": 100, "height": 60})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 0.2, "k": 0}, "ids": [bg.0]}));
    let mark = id_of(&run(&mut s, "shape.rectangle", json!({"x": 1204, "y": 104, "width": 20, "height": 20})));
    run(&mut s, "paint.setFill", json!({"color": {"c": 0, "m": 0, "y": 0, "k": 1}, "ids": [mark.0]}));
    run(&mut s, "paint.setStroke", json!({"none": true, "ids": [bg.0]}));
    let hidden = id_of(&run(&mut s, "shape.rectangle", json!({"x": 1300, "y": 100, "width": 100, "height": 60})));
    run(&mut s, "select.set", json!({"ids": [bg.0, mark.0, hidden.0]}));
    let g = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "select.set", json!({"ids": [hidden.0]}));
    run(&mut s, "object.hide", json!({}));
    run(&mut s, "select.set", json!({"ids": [g.0]}));
    run(&mut s, "object.rotate", json!({"angle": 30}));
    let learned = run(&mut s, "dancard.learnTemplate", json!({"mode": "one", "ids": [a.0, g.0], "reference": a.0}));
    assert_eq!(learned["angleSource"], "render", "{learned}");
    assert_eq!(learned["angles"], json!([0.0, 270.0]), "dựng thẳng 30° thì lệch con chuẩn đứng đúng 270°: {learned}");
    let t = &learned["template"];
    assert!((t["slots"][1]["tilt"].as_f64().unwrap() - 30.0).abs() < 1e-6, "{t}");
}
