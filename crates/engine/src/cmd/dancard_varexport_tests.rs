//! DanCard › Variable › Xuất file theo bản ghi: mỗi bản ghi một file (PDF / AI) đúng giá trị của
//! nó, tên ổn định, không ghi đè khi chưa cho phép, tài liệu đang mở không đổi.

use serde_json::{Value, json};
use vectorcraft_doc::{DataSet, Document, Node, NodeKind};

use super::*;
use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

/// Mẫu vé: chữ "So" (số nhảy), logo "Logo" ẩn/hiện theo bản ghi, chữ "Khoa" bị khóa; 3 bản ghi
/// đặt tên theo cột So (001, 002, 003), đang hiện bản ghi 1.
fn template() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 300, "height": 200}));
    let so = id_of(&run(&mut s, "text.create", json!({"x": 20, "y": 60, "text": "Số mẫu"})));
    run(&mut s, "object.setProps", json!({"id": so.0, "name": "So"}));
    let logo = id_of(&run(&mut s, "shape.rectangle", json!({"x": 100, "y": 100, "width": 50, "height": 50})));
    run(&mut s, "object.setProps", json!({"id": logo.0, "name": "Logo"}));
    let v = run(&mut s, "dancard.importVariables", json!({"text": "So,#Logo\n001,true\n002,false\n003,true\n", "datasetNames": "So"}));
    assert_eq!(v["datasets"], json!(["001", "002", "003"]), "{v}");
    s
}

fn find<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
    if n.name.as_deref() == Some(name) {
        return Some(n);
    }
    n.children().and_then(|c| c.iter().find_map(|c| find(c, name)))
}

fn named<'a>(d: &'a Document, name: &str) -> &'a Node {
    d.layers.iter().find_map(|l| find(l, name)).unwrap_or_else(|| panic!("không thấy object {name}"))
}

fn text_of(d: &Document, name: &str) -> String {
    match &named(d, name).kind {
        NodeKind::Text(t) => t.plain_text(),
        _ => panic!("{name} không phải chữ"),
    }
}

/// Tài liệu của file `.ai` đã xuất, mở lại như mở file.
fn reopen(path: &std::path::Path) -> Document {
    let mut s = Session::new();
    run(&mut s, "document.open", json!({"path": path.to_string_lossy()}));
    (*s.doc().unwrap().doc).clone()
}

fn dir(tag: &str) -> std::path::PathBuf {
    let d = vectorcraft_testkit::temp_dir(&format!("dancard-varexport-{tag}"));
    for e in std::fs::read_dir(&d).unwrap().flatten() {
        std::fs::remove_file(e.path()).unwrap();
    }
    d
}

fn listing(d: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn ten_file_sach_khong_trung_va_on_dinh() {
    let set = |name: &str| DataSet { name: name.into(), ..Default::default() };
    let sets = [set("A/B"), set("a-b"), set("con"), set("Lộc: 01?"), set("..."), set("A-B")];
    assert_eq!(file_names(&sets, "", Format::Pdf), ["A-B.pdf", "a-b (2).pdf", "con_.pdf", "Lộc- 01-.pdf", "Record-5.pdf", "A-B (3).pdf"]);
    assert_eq!(file_names(&sets[..1], "Vé ", Format::Ai), ["Vé A-B.ai"]);
    assert!(reserved("COM1") && reserved("nul.txt") && reserved("Lpt9") && !reserved("COM10") && !reserved("Console"));
}

#[test]
fn moi_ban_ghi_mot_file_ai_dung_gia_tri_tai_lieu_khong_doi() {
    let mut s = template();
    let before = (s.doc().unwrap().revision, s.doc().unwrap().history.undo.len());
    let out = dir("ai");
    let v = run(&mut s, "dancard.exportVariables", json!({"folder": out.to_string_lossy(), "format": "ai", "prefix": "Vé "}));
    assert_eq!((v["saved"].as_u64(), v["total"].as_u64(), v["records"].as_u64()), (Some(3), Some(3), Some(3)), "{v}");
    assert_eq!(listing(&out), ["Vé 001.ai", "Vé 002.ai", "Vé 003.ai"]);
    for (file, so, logo) in [("Vé 001.ai", "001", true), ("Vé 002.ai", "002", false), ("Vé 003.ai", "003", true)] {
        let d = reopen(&out.join(file));
        assert_eq!(text_of(&d, "So"), so, "{file}");
        assert_eq!(named(&d, "Logo").visible, logo, "{file}");
    }
    // Tài liệu đang mở giữ bản ghi 1, không thêm bước hoàn tác.
    let st = s.doc().unwrap();
    assert_eq!((st.revision, st.history.undo.len()), before);
    assert_eq!(text_of(&st.doc, "So"), "001");
    assert_eq!(st.doc.variables.active_dataset.as_deref(), Some("001"));
}

#[test]
fn xuat_pdf_tung_doan_ten_giu_nguyen() {
    let mut s = template();
    let out = dir("pdf");
    let folder = out.to_string_lossy().into_owned();
    let v = run(&mut s, "dancard.exportVariables", json!({"folder": folder, "from": 2, "to": "3"}));
    assert_eq!(v["files"], json!(["002.pdf", "003.pdf"]), "{v}");
    assert!(v["message"].as_str().unwrap().contains("bản ghi 2–3"), "{v}");
    // Panel xuất từng bản ghi: from = to.
    run(&mut s, "dancard.exportVariables", json!({"folder": folder, "from": "1", "to": 1}));
    assert_eq!(listing(&out), ["001.pdf", "002.pdf", "003.pdf"]);
    for f in listing(&out) {
        assert!(std::fs::read(out.join(&f)).unwrap().starts_with(b"%PDF-"), "{f}");
    }
}

#[test]
fn khong_ghi_de_khi_chua_cho_phep() {
    let mut s = template();
    let out = dir("overwrite");
    let folder = out.to_string_lossy().into_owned();
    std::fs::write(out.join("002.pdf"), b"cu").unwrap();
    // Kiểm tra trước báo file đã có; xuất thật thì không ghi gì.
    let v = run(&mut s, "dancard.exportVariables", json!({"folder": folder, "dryRun": true}));
    assert_eq!((v["existing"].clone(), v["saved"].as_u64()), (json!(["002.pdf"]), Some(0)), "{v}");
    let e = s.execute("dancard.exportVariables", &json!({"folder": folder})).unwrap_err().to_string();
    assert!(e.contains("002.pdf") && e.contains("Ghi đè"), "{e}");
    assert_eq!(listing(&out), ["002.pdf"]);
    assert_eq!(std::fs::read(out.join("002.pdf")).unwrap(), b"cu");
    // Bật ghi đè: ghi cả ba.
    let v = run(&mut s, "dancard.exportVariables", json!({"folder": folder, "overwrite": true}));
    assert_eq!(v["saved"].as_u64(), Some(3), "{v}");
    assert!(std::fs::read(out.join("002.pdf")).unwrap().starts_with(b"%PDF-"));
}

#[test]
fn loi_ro_rang_khong_sap() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 100, "height": 100}));
    // Chưa có bản ghi.
    let e = s.execute("dancard.exportVariables", &json!({"dryRun": true})).unwrap_err().to_string();
    assert!(e.contains("chưa có bản ghi"), "{e}");
    let mut s = template();
    let out = dir("errors");
    let folder = out.to_string_lossy().into_owned();
    for p in [
        json!({"folder": folder, "from": 4}),
        json!({"folder": folder, "from": 0}),
        json!({"folder": folder, "from": 3, "to": 2}),
        json!({"folder": folder, "to": "ba"}),
        json!({"folder": folder, "format": "png"}),
        json!({"folder": ""}),
        json!({"folder": out.join("khong-co").to_string_lossy()}),
        json!({"folder": folder, "prefix": "x".repeat(300)}),
    ] {
        let e = s.execute("dancard.exportVariables", &p).unwrap_err();
        assert!(matches!(e, EngineError::Other(_)), "{p}: {e:?}");
    }
    assert!(listing(&out).is_empty());
    // Kiểm tra trước không cần thư mục.
    let v = run(&mut s, "dancard.exportVariables", json!({"dryRun": true, "format": "ai"}));
    assert_eq!(v["files"], json!(["001.ai", "002.ai", "003.ai"]), "{v}");
}
