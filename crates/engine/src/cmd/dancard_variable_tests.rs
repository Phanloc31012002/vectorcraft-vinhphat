//! DanCard › Variable: đọc file dữ liệu (ngoặc kép, dấu phân cách, bảng mã, NFC tiếng Việt), tạo
//! biến và bản ghi, gắn object theo tên hoặc ghi chú, hiện bản ghi 1 trong một bước hoàn tác.

use serde_json::{Value, json};
use vectorcraft_doc::{NodeId, NodeKind, VariableKind};

use super::*;
use crate::Session;

fn rows(cells: &[&[&str]]) -> Vec<Vec<String>> {
    cells.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
}

fn grid(text: &str, delim: char) -> Vec<Vec<String>> {
    parse(text, delim).rows
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn session() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 600, "height": 400}));
    s
}

/// Một object chữ đặt tên `name`.
fn text(s: &mut Session, content: &str, name: &str) -> NodeId {
    let id = id_of(&run(s, "text.create", json!({"x": 20, "y": 60, "text": content})));
    run(s, "object.setProps", json!({"id": id.0, "name": name}));
    id
}

fn plain(s: &Session, id: NodeId) -> String {
    match &s.doc().unwrap().doc.node(id).unwrap().kind {
        NodeKind::Text(t) => t.plain_text(),
        _ => panic!("không phải chữ"),
    }
}

fn shown(s: &Session, id: NodeId) -> bool {
    s.doc().unwrap().doc.node(id).unwrap().visible
}

fn message(v: &Value) -> &str {
    v["message"].as_str().unwrap()
}

fn warned(v: &Value, part: &str) -> bool {
    v["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains(part))
}

#[test]
fn tach_o_ngoac_kep_dau_phan_cach_va_xuong_dong() {
    let g = grid("Ten,Ghi chu\n\"Lộc, Phan\",\"nói \"\"chào\"\"\nhai dòng\"\n", ',');
    assert_eq!(g, rows(&[&["Ten", "Ghi chu"], &["Lộc, Phan", "nói \"chào\"\nhai dòng"]]));
    // Chữ sau dấu " đóng bị bỏ; ô không bắt đầu bằng " giữ nguyên dấu ", kể cả khi có khoảng trắng
    // đứng trước.
    assert_eq!(grid("\"ab\"cd,x\na\"b, \"ab\"", ','), rows(&[&["ab", "x"], &["a\"b", " \"ab\""]]));
    // CR, CRLF, dòng trống và dòng toàn ô trống bị bỏ; không cắt khoảng trắng.
    assert_eq!(grid("a;b\r\n\r\n c ;d\r\r;\n", ';'), rows(&[&["a", "b"], &[" c ", "d"]]));
    // Ngoặc kép không đóng: lấy tới cuối file. Ô trống cuối dòng vẫn là một ô.
    assert_eq!(grid("a\t\n\"x\ty", '\t'), rows(&[&["a", ""], &["x\ty"]]));
    assert!(grid("", ',').is_empty() && grid("\n\n,,\n", ',').is_empty());
}

#[test]
fn chon_dau_phan_cach_theo_duoi_file() {
    assert_eq!(pick_delimiter(None, Some("csv"), "a,b;c\n").unwrap(), ',');
    assert_eq!(pick_delimiter(None, Some("csv"), "a;b;c\nx,y").unwrap(), ';');
    assert_eq!(pick_delimiter(None, Some("txt"), "a,b").unwrap(), '\t');
    assert_eq!(pick_delimiter(None, None, "a\tb\tc,d").unwrap(), '\t');
    assert_eq!(pick_delimiter(None, None, "a;b").unwrap(), ';');
    assert_eq!(pick_delimiter(None, None, "a,b;c,d").unwrap(), ',');
    assert_eq!(pick_delimiter(Some(';'), Some("xlsx"), "a,b").unwrap(), ';');
    assert!(pick_delimiter(None, Some("xlsx"), "a,b").is_err());
}

#[test]
fn chuan_hoa_nfc_tieng_viet() {
    assert_eq!(nfc("e\u{302}\u{301}"), "\u{1EBF}", "ế từ e + dấu mũ + dấu sắc");
    assert_eq!(nfc("\u{EA}\u{301}"), "\u{1EBF}", "ế từ ê dựng sẵn + dấu sắc rời");
    assert_eq!(nfc("a\u{306}\u{323}"), "\u{1EB7}", "ặ: dấu nặng xếp trước dấu trăng");
    assert_eq!(nfc("\u{103}\u{323}"), "\u{1EB7}");
    assert_eq!(nfc("u\u{323}\u{31B}"), "\u{1EF1}", "ự: dấu móc xếp trước dấu nặng");
    assert_eq!(nfc("o\u{31B}\u{300}"), "\u{1EDD}");
    assert_eq!(nfc("Ho\u{323} te\u{302}n"), "H\u{1ECD} t\u{EA}n");
    assert_eq!(nfc("H\u{1ECD} t\u{EA}n"), "H\u{1ECD} t\u{EA}n");
    assert_eq!(nfc("plain ASCII"), "plain ASCII");
    // Dấu đứng đầu, chữ không có dạng dựng sẵn: giữ nguyên.
    assert_eq!(nfc("\u{301}x"), "\u{301}x");
    assert_eq!(nfc("q\u{301}"), "q\u{301}");
    assert_eq!(nfc("\u{110}\u{301}"), "\u{110}\u{301}");
}

#[test]
fn doc_bang_ma_utf8_utf16_va_windows_1258() {
    assert_eq!(decode(b"\xEF\xBB\xBFTen"), ("Ten".to_string(), "utf-8"));
    let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain("Lộc".encode_utf16().flat_map(u16::to_le_bytes)).collect();
    assert_eq!(decode(&le), ("Lộc".to_string(), "utf-16le"));
    let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("Lộc".encode_utf16().flat_map(u16::to_be_bytes)).collect();
    assert_eq!(decode(&be), ("Lộc".to_string(), "utf-16be"));
    // CSV của Excel trên Windows tiếng Việt: ê + dấu nặng rời (0xF2); NFC ghép thành ệ.
    let (t, encoding) = decode(b"Vi\xEA\xF2t \xF0\xE3");
    assert_eq!(encoding, "windows-1258");
    assert_eq!(nfc(&t), "Vi\u{1EC7}t \u{111}\u{103}");
}

#[test]
fn gia_tri_an_hien_va_doan_chu() {
    let got: Vec<bool> = ["TRUE", " on ", "false", "off", "", "0", "x", "\u{A0}Off\u{FEFF}"].into_iter().map(visible).collect();
    assert_eq!(got, [true, true, false, false, false, true, true, false]);
    assert_eq!(text_value("", false), "\u{A0}");
    assert_eq!(text_value("a\r\nb\rc", false), "a\nb\nc");
    assert_eq!(text_value("a\n  \nb", false), "a\n\u{A0}\nb");
    assert_eq!(text_value("a\\\\b", true), "a\nb");
    assert_eq!(text_value("a\\\\b", false), "a\\\\b");
}

#[test]
fn bien_theo_dong_tieu_de() {
    let g = grid("Ten,#Hien,@Anh,%BieuDo,Ten, ,Họ tên \n1,2,3,4,5,6,7", ',');
    let mut w = Vec::new();
    let cols = columns(&Table::new(&g, false), true, &mut w);
    let got: Vec<(&str, VariableKind, usize)> = cols.iter().map(|c| (c.name.as_str(), c.kind, c.index)).collect();
    assert_eq!(
        got,
        [
            ("Ten", VariableKind::Text, 0),
            ("Hien", VariableKind::Visibility, 1),
            ("Anh", VariableKind::Image, 2),
            ("Ten 2", VariableKind::Text, 4),
            ("Variable6", VariableKind::Text, 5),
            ("Họ tên", VariableKind::Text, 6),
        ]
    );
    // % bỏ qua, một tên trùng, một tên trống.
    assert_eq!(w.len(), 3, "{w:?}");
    // Không có dòng tiêu đề: Variable1…, đều là chữ.
    let cols = columns(&Table::new(&g, false), false, &mut Vec::new());
    let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Variable1", "Variable2", "Variable3", "Variable4", "Variable5", "Variable6", "Variable7"]);
    assert!(cols.iter().all(|c| c.kind == VariableKind::Text));
    // Đảo hàng/cột: cột đầu là tên biến; dòng ngắn được đệm, ô thừa bị bỏ.
    let g = grid("Ten,An,Binh\n#Hien,true\nGhi,x,y,z\n", ',');
    let t = Table::new(&g, true);
    let cols = columns(&t, true, &mut Vec::new());
    assert_eq!(cols.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Ten", "Hien", "Ghi"]);
    assert_eq!((t.height(), t.cell(1, 0), t.cell(2, 1), t.cell(2, 2), t.cell(3, 2)), (3, "An", "", "y", ""));
    assert_eq!(t.long_rows(), 1);
}

#[test]
fn ten_ban_ghi() {
    let g = grid("Ma,Ten\nA1,An\nA2,Bình\n", ',');
    let t = Table::new(&g, false);
    let cols = columns(&t, true, &mut Vec::new());
    let names = |o: &Options, w: &mut Vec<String>| datasets(&t, &cols, o, w).unwrap().into_iter().map(|d| d.name).collect::<Vec<_>>();
    let mut o = options(&json!({})).unwrap();
    assert_eq!(names(&o, &mut Vec::new()), ["Record-1", "Record-2"]);
    o.dataset_names = Some("Ma".into());
    assert_eq!(names(&o, &mut Vec::new()), ["A1", "A2"]);
    o.dataset_names = Some("2".into());
    assert_eq!(names(&o, &mut Vec::new()), ["An", "Bình"]);
    o.dataset_names = Some("Không có".into());
    assert!(datasets(&t, &cols, &o, &mut Vec::new()).is_err());
    // Trùng hoặc trống: đặt lại tất cả thành Record 1…, có cảnh báo.
    let g = grid("Ma,Ten\nA1,An\n A1 ,Bình\n,Chi\n", ',');
    let t = Table::new(&g, false);
    let cols = columns(&t, true, &mut Vec::new());
    o.dataset_names = Some("Ma".into());
    let mut w = Vec::new();
    let got: Vec<String> = datasets(&t, &cols, &o, &mut w).unwrap().into_iter().map(|d| d.name).collect();
    assert_eq!(got, ["Record 1", "Record 2", "Record 3"]);
    assert!(w.iter().any(|m| m.contains("“A1”") && m.contains("(trống)")), "{w:?}");
}

const DATA: &str = "Ten,SoDT,#Logo\nNguyễn Văn An,0901 234 567,true\nTrần Thị Bình,,off\n";

#[test]
fn nhap_gan_theo_ten_va_hien_ban_ghi_1() {
    let mut s = session();
    let ten = text(&mut s, "Tên mẫu", "Ten");
    // SoDT nằm trong một group: vẫn tìm được.
    let so = text(&mut s, "Số mẫu", "SoDT");
    run(&mut s, "select.set", json!({"ids": [so.0]}));
    run(&mut s, "object.group", json!({}));
    // Logo là một group (biến ẩn/hiện gắn được mọi object).
    run(&mut s, "shape.rectangle", json!({"x": 300, "y": 20, "width": 50, "height": 50}));
    let logo = id_of(&run(&mut s, "object.group", json!({})));
    run(&mut s, "object.setProps", json!({"id": logo.0, "name": "Logo"}));
    // Chữ bị khóa cùng tên bị bỏ qua và báo lại; layer cùng tên không phải object.
    let locked = text(&mut s, "Khóa", "Ten");
    run(&mut s, "object.setProps", json!({"id": locked.0, "locked": true}));
    run(&mut s, "layer.new", json!({"name": "Ten"}));

    let v = run(&mut s, "dancard.importVariables", json!({"text": DATA}));
    assert!(message(&v).starts_with("OK: Đã nhập 3 biến, 2 bản ghi, gắn 3 object; đang hiện bản ghi “Record-1”."), "{v}");
    assert_eq!(v["bound"], 3);
    assert_eq!(v["datasets"], json!(["Record-1", "Record-2"]));
    assert_eq!(v["variables"][2], json!({"name": "Logo", "kind": "visibility", "column": 3, "bound": 1}));
    assert_eq!((v["applied"].as_u64(), v["skipped"].as_u64()), (Some(3), Some(0)));
    assert_eq!((v["delimiter"].as_str(), v["dryRun"].as_bool()), (Some("comma"), Some(false)));
    assert!(warned(&v, "bị khóa"), "{v}");
    let vars = &s.doc().unwrap().doc.variables;
    assert_eq!(vars.objects_of("Ten"), [ten]);
    assert_eq!(vars.objects_of("SoDT"), [so]);
    assert_eq!(vars.active_dataset.as_deref(), Some("Record-1"));
    // Bản ghi 1 đã hiện trên mẫu.
    assert_eq!(plain(&s, ten), "Nguyễn Văn An");
    assert_eq!(plain(&s, so), "0901 234 567");
    assert!(shown(&s, logo));
    // Bản ghi 2: ô trống thành khoảng trắng không ngắt, "off" ẩn logo.
    run(&mut s, "dataset.next", json!({}));
    assert_eq!(plain(&s, ten), "Trần Thị Bình");
    assert_eq!(plain(&s, so), "\u{A0}");
    assert!(!shown(&s, logo));
    // Cả lần nhập (thay biến + hiện bản ghi 1) là một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(plain(&s, ten), "Nguyễn Văn An");
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().doc.variables.variables.is_empty());
    assert_eq!(plain(&s, ten), "Tên mẫu");
    assert_eq!(plain(&s, so), "Số mẫu");
}

#[test]
fn gan_theo_ghi_chu_kiem_tra_truoc_va_thay_bien_cu() {
    let mut s = session();
    let t = text(&mut s, "x", "Khác");
    // Ghi chú gõ với dấu thanh rời vẫn khớp tên cột dựng sẵn (NFC).
    run(&mut s, "attributes.set", json!({"ids": [t.0], "note": "Ho\u{323} te\u{302}n"}));
    let data = "H\u{1ECD} t\u{EA}n,Ghi\nLộc,a\n";
    let v = run(&mut s, "dancard.importVariables", json!({"text": data, "bindBy": "note", "dryRun": true}));
    assert!(message(&v).starts_with("OK: Kiểm tra (chưa đổi tài liệu): sẽ nhập 2 biến, 1 bản ghi, gắn 1 object."), "{v}");
    assert_eq!(v["variables"][0]["bound"], 1);
    assert!(s.doc().unwrap().doc.variables.variables.is_empty(), "kiểm tra trước không đổi tài liệu");
    assert_eq!(plain(&s, t), "x");
    // Theo tên: không object nào khớp, báo rõ tên cần đặt.
    let v = run(&mut s, "dancard.importVariables", json!({"text": data}));
    assert_eq!(v["bound"], 0);
    assert!(message(&v).contains("Chưa gắn được object nào") && message(&v).contains("“H\u{1ECD} t\u{EA}n”"), "{v}");
    // Theo ghi chú: gắn, hiện bản ghi 1 và thay biến của lần nhập trước.
    let v = run(&mut s, "dancard.importVariables", json!({"text": "H\u{1ECD} t\u{EA}n\nPhan\n", "bindBy": "note"}));
    assert_eq!(v["bound"], 1);
    assert!(message(&v).contains("Đã thay 2 biến và 1 bản ghi cũ"), "{v}");
    assert_eq!(plain(&s, t), "Phan");
    let names: Vec<String> = s.doc().unwrap().doc.variables.variables.iter().map(|v| v.name.clone()).collect();
    assert_eq!(names, ["H\u{1ECD} t\u{EA}n"]);
}

#[test]
fn khong_gan_tu_dong_giu_lien_ket_tay() {
    let mut s = session();
    let t = text(&mut s, "x", "Khác");
    run(&mut s, "dancard.importVariables", json!({"text": "Ten\nA\n", "bindBy": "none"}));
    run(&mut s, "variable.bind", json!({"variable": "Ten", "ids": [t.0]}));
    let v = run(&mut s, "dancard.importVariables", json!({"text": "Ten,#Hien\nB,true\nC,false\n", "bindBy": "none"}));
    assert_eq!(v["bound"], 1);
    assert_eq!(plain(&s, t), "B");
    run(&mut s, "dataset.next", json!({}));
    assert_eq!(plain(&s, t), "C");
}

#[test]
fn chuyen_hang_cot_va_dau_xuong_dong() {
    let mut s = session();
    let t = text(&mut s, "x", "Ten");
    let v = run(&mut s, "dancard.importVariables", json!({"text": "Ten\tAn\\\\Phan\tBinh\n", "transpose": true, "backslashBreak": true}));
    assert_eq!((v["datasets"].as_array().unwrap().len(), v["delimiter"].as_str()), (2, Some("tab")));
    assert_eq!(plain(&s, t), "An\nPhan");
}

#[test]
fn cot_anh_doi_file_anh_theo_ban_ghi() {
    use crate::tests_links::{BLUE, Folder, RED, image, place, png, write};
    let dir = Folder::new("dancard-var-anh");
    let (red, blue) = (dir.file("do.png"), dir.file("xanh.png"));
    write(&red, &png(40, 30, RED));
    write(&blue, &png(80, 60, BLUE));
    let mut s = session();
    let anh = place(&mut s, &red);
    run(&mut s, "object.setProps", json!({"id": anh.0, "name": "Anh"}));
    // Chữ trùng tên biến ảnh: không gắn, có cảnh báo.
    text(&mut s, "x", "Anh");
    // Đường dẫn tương đối tính từ thư mục file dữ liệu; ô trống giữ ảnh như cũ.
    let csv = dir.file("du lieu.csv");
    write(&csv, "Ten,@Anh\nAn,xanh.png\nBình,\n".as_bytes());
    let v = run(&mut s, "dancard.importVariables", json!({"path": csv}));
    assert_eq!(v["variables"][1], json!({"name": "Anh", "kind": "image", "column": 2, "bound": 1}), "{v}");
    assert!(warned(&v, "không phải ảnh"), "{v}");
    let link = |s: &Session| image(s, anh).link.map(|l| l.path);
    assert_eq!(link(&s), Some(blue.clone()), "bản ghi 1 đổi ảnh");
    assert!(!s.doc().unwrap().doc.variables.datasets[1].values.contains_key("Anh"), "ô trống không có giá trị");
    run(&mut s, "dataset.next", json!({}));
    assert_eq!(link(&s), Some(blue), "bản ghi 2 giữ ảnh");
}

#[test]
fn loi_ro_rang_khong_sap() {
    let mut s = session();
    for p in [
        json!({}),
        json!({"text": ""}),
        json!({"text": "\n,,\n"}),
        json!({"text": "%BieuDo\ny.csv"}),
        json!({"text": "a", "delimiter": "pipe"}),
        json!({"text": "a", "bindBy": "tag"}),
        json!({"text": "a\n1", "datasetNames": "b"}),
        json!({"path": "/khong/co/file.csv"}),
    ] {
        let e = s.execute("dancard.importVariables", &p).unwrap_err();
        assert!(matches!(e, EngineError::Other(_)), "{p}: {e:?}");
    }
    assert!(s.doc().unwrap().doc.variables.variables.is_empty());
    // Dòng dài hơn dòng tiêu đề: bỏ ô thừa và báo lại, không sập.
    let v = run(&mut s, "dancard.importVariables", json!({"text": "a,b\n1,2,3,4\n"}));
    assert_eq!(v["variables"].as_array().unwrap().len(), 2);
    assert!(warned(&v, "nhiều ô hơn"), "{v}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn doc_file_csv_cham_phay_windows_1258() {
    let dir = vectorcraft_testkit::temp_dir("dancard-variable");
    let path = dir.join("du-lieu.csv");
    std::fs::write(&path, b"Ten;Gia\r\nVi\xEA\xF2t;1\r\n").unwrap();
    let mut s = session();
    let t = text(&mut s, "x", "Ten");
    let v = run(&mut s, "dancard.importVariables", json!({"path": path.to_string_lossy()}));
    assert_eq!((v["delimiter"].as_str(), v["encoding"].as_str()), (Some("semicolon"), Some("windows-1258")));
    assert!(warned(&v, "Windows-1258"), "{v}");
    assert_eq!(plain(&s, t), "Vi\u{1EC7}t");
    // File .txt là Tab.
    let txt = dir.join("du-lieu.txt");
    std::fs::write(&txt, "\u{FEFF}Ten\tGia\nAn\t2\n").unwrap();
    let v = run(&mut s, "dancard.importVariables", json!({"path": txt.to_string_lossy()}));
    assert_eq!((v["delimiter"].as_str(), v["encoding"].as_str()), (Some("tab"), Some("utf-8")));
    assert_eq!(plain(&s, t), "An");
}
