//! Dàn KTS: các vector đối chiếu của DanCard (bộ tính JS chạy nguyên văn, xem
//! `dan_toi_uu_kts.md` §13) cho các hàm tính bố cục, và các lần dàn thật qua `Session`.
#![allow(clippy::type_complexity)] // bảng số liệu kiểm thử viết thẳng

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_geom::Rect;

use super::*;
use crate::Session;

const CM: f64 = 10.0 * MM;

fn usable(paper_cm: f64) -> f64 {
    paper_cm * CM - 2.0 * MARGIN
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.006
}

/// Bố cục theo cách ghi của bảng vector: `row[R×C@góc]`, `splitV(cut){… | …}`.
fn describe(p: &Plan) -> String {
    match &p.shape {
        Shape::Empty => "empty".into(),
        Shape::Strips { row, bands } => {
            let parts: Vec<String> = bands.iter().flatten().map(|b| format!("{}x{}@{}", b.repeat, b.cells, b.angle)).collect();
            format!("{}[{}]", if *row { "row" } else { "column" }, parts.join("+"))
        }
        Shape::Split { vertical, cut, first, second } => {
            format!("split{}({cut:.2}pt){{{} | {}}}", if *vertical { "V" } else { "H" }, describe(first), describe(second))
        }
    }
}

#[test]
fn cung_kho_nhu_dancard() {
    let cases = [
        ((9.0, 5.5), (33.0, 35.4), (20, 12, 8), "splitV(255.12pt){row[6x1@0] | row[3x2@0+2x4@90]}"),
        ((9.2, 5.6), (33.0, 35.4), (20, 8, 12), "splitV(260.79pt){row[6x1@0] | row[1x2@0+3x4@90]}"),
        ((5.4, 9.0), (33.0, 48.0), (31, 25, 6), "splitV(153.07pt){row[5x1@0] | row[4x5@0+2x3@90]}"),
        ((10.0, 15.0), (33.0, 35.4), (6, 6, 0), "row[2x3@0]"),
        ((8.7, 5.5), (32.0, 43.0), (25, 9, 16), "splitV(246.61pt){row[7x1@0] | row[1x2@0+4x4@90]}"),
    ];
    for ((w, h), (pw, ph), counts, shape) in cases {
        let plan = Uniform::new(w * CM, h * CM).solve(usable(pw), usable(ph), 2);
        assert_eq!((plan.count, plan.normal, plan.rotated), counts, "{w} × {h} trên {pw} × {ph}");
        assert_eq!(describe(&plan), shape, "{w} × {h} trên {pw} × {ph}");
    }
}

#[test]
fn vi_tri_va_thu_tu_cum_mau() {
    let plan = Uniform::new(9.0 * CM, 5.5 * CM).solve(usable(33.0), usable(35.4), 2);
    let mut slots = Vec::new();
    collect_slots(&plan, 0.0, 0.0, usable(33.0), usable(35.4), &mut slots);
    assert_eq!(slots.len(), 20);
    let ys = [0.0, -155.91, -311.81, -467.72, -623.62, -779.53];
    for (s, y) in slots.iter().take(6).zip(ys) {
        assert!(close(s.x, 0.0) && close(s.y, y) && close(s.w, 255.12) && close(s.h, 155.91) && s.angle == 0, "{s:?}");
    }
    let pairs = [(255.12, 0.0), (510.24, 0.0), (255.12, -155.91), (510.24, -155.91), (255.12, -311.81), (510.24, -311.81)];
    for (s, (x, y)) in slots.iter().skip(6).take(6).zip(pairs) {
        assert!(close(s.x, x) && close(s.y, y) && s.angle == 0, "{s:?}");
    }
    let xs = [255.12, 411.02, 566.93, 722.83];
    for (i, s) in slots.iter().skip(12).enumerate() {
        let (x, y) = (xs[i % 4], if i < 4 { -467.72 } else { -722.83 });
        assert!(close(s.x, x) && close(s.y, y) && close(s.w, 155.91) && close(s.h, 255.12) && s.angle == 90, "{s:?}");
    }
    // Lô [A × 10, B × 10]: mỗi mẫu một cụm kề nhau, cùng chiều trước.
    let mut batch = vec![0usize; 10];
    batch.extend([1usize; 10]);
    let ordered = order_slots(slots.clone(), &batch, true);
    let order: Vec<usize> = ordered.iter().map(|o| slots.iter().position(|s| s == o).unwrap()).collect();
    assert_eq!(order, [0, 6, 7, 1, 8, 9, 2, 10, 11, 3, 12, 13, 14, 15, 16, 17, 18, 19, 4, 5]);
    // Không chia nhiều mẫu: giữ nguyên.
    assert_eq!(order_slots(slots.clone(), &batch, false), slots);
}

#[test]
fn nhieu_kho_nhu_dancard() {
    let cases: [(&[(f64, f64)], usize, &[usize], (usize, usize, f64)); 4] = [
        (&[(9.0, 5.5), (5.5, 5.5)], 20, &[20, 0], (13, 24, 9334.49)),
        (&[(15.0, 10.0), (9.0, 5.5)], 6, &[6, 0], (5, 7, 4388.03)),
        (&[(9.0, 5.5), (9.0, 5.5), (4.0, 4.0)], 24, &[10, 10, 4], (18, 27, 9561.26)),
        (&[(20.0, 14.0), (9.0, 5.5), (5.0, 3.0)], 13, &[3, 2, 8], (15, 18, 6485.67)),
    ];
    for (specs, count, counts, (work, cuts, length)) in cases {
        let specs: Vec<(f64, f64)> = specs.iter().map(|(w, h)| (w * CM, h * CM)).collect();
        let m = mixed_plan(&specs, usable(33.0), usable(35.4)).unwrap();
        assert_eq!((m.count, m.counts.as_slice()), (count, counts), "{specs:?}");
        assert_eq!((m.work, m.cut_count), (work, cuts), "{specs:?}");
        assert!(close(m.cut_length, length), "{} ≠ {length}", m.cut_length);
        assert!(!m.capacity_limited);
    }
    // Ba khổ: không có DP chính xác, bộ tham lam thắng; đúng 13 vị trí của DanCard.
    let specs: Vec<(f64, f64)> = [(20.0, 14.0), (9.0, 5.5), (5.0, 3.0)].iter().map(|(w, h)| (w * CM, h * CM)).collect();
    assert!(slicing_profile(&specs, usable(33.0), usable(35.4)).is_none());
    let m = mixed_plan(&specs, usable(33.0), usable(35.4)).unwrap();
    let want = [
        (0, 0.0, 0.0, 566.93, 396.85, 0),
        (0, 0.0, 396.85, 396.85, 566.93, 90),
        (0, 396.85, 396.85, 396.85, 566.93, 90),
        (1, 566.93, 0.0, 255.12, 155.91, 0),
        (1, 566.93, 155.91, 255.12, 155.91, 0),
        (2, 566.93, 311.81, 141.73, 85.04, 0),
        (2, 708.66, 311.81, 141.73, 85.04, 0),
        (2, 822.05, 0.0, 85.04, 141.73, 90),
        (2, 822.05, 155.91, 85.04, 141.73, 90),
        (2, 793.7, 396.85, 85.04, 141.73, 90),
        (2, 793.7, 538.58, 85.04, 141.73, 90),
        (2, 793.7, 680.31, 85.04, 141.73, 90),
        (2, 793.7, 822.05, 85.04, 141.73, 90),
    ];
    assert_eq!(m.cells.len(), want.len());
    for (c, (model, x, y, w, h, angle)) in m.cells.iter().zip(want) {
        assert!(c.model == model && close(c.x, x) && close(c.y, y) && close(c.w, w) && close(c.h, h) && c.angle == angle, "{c:?}");
    }
    // Hai khổ: DP chính xác cho đúng bố cục thắng.
    let specs: Vec<(f64, f64)> = [(9.0, 5.5), (5.5, 5.5)].iter().map(|(w, h)| (w * CM, h * CM)).collect();
    let p = slicing_profile(&specs, usable(33.0), usable(35.4)).unwrap();
    assert_eq!((p.count, p.counts.as_slice()), (20, [20usize, 0].as_slice()));
}

#[test]
fn nhieu_kho_bao_loi_nhu_dancard() {
    assert_eq!(mixed_plan(&[], 100.0, 100.0).unwrap_err(), "Không có vùng in/mẫu hợp lệ.");
    assert_eq!(mixed_plan(&[(10.0, 10.0), (0.0, 5.0)], 100.0, 100.0).unwrap_err(), "Không đo được kích thước mẫu 2.");
    assert_eq!(mixed_plan(&[(500.0, 500.0), (200.0, 300.0)], 100.0, 100.0).unwrap_err(), "Mẫu không vừa vùng in; không tự thu nhỏ bài.");
    assert_eq!(mixed_plan(&[(1.0, 1.0), (2.0, 1.0)], 1000.0, 1000.0).unwrap_err(), "Quá nhiều vị trí/mẫu để ghép; hãy chia thành nhóm nhỏ hơn.");
}

#[test]
fn dao_cat_va_dai_nhu_dancard() {
    let cell = |x: f64, y: f64, w: f64, h: f64, model: usize| Cell { x, y, w, h, angle: 0, model };
    let k = knife_plan(&[cell(0.0, 0.0, 10.0, 10.0, 0), cell(10.0, 0.0, 10.0, 10.0, 0), cell(0.0, 10.0, 20.0, 5.0, 0)], 30.0, 20.0).unwrap();
    assert_eq!((k.cut_count, k.work), (4, 4));
    assert!(close(k.cut_length, 70.0));
    let cuts: Vec<(bool, f64, f64, f64, f64, f64)> = k.cuts.iter().map(|c| (c.horizontal, c.at, c.x, c.y, c.w, c.h)).collect();
    assert_eq!(
        cuts,
        [
            (false, 20.0, 0.0, 0.0, 30.0, 20.0),
            (true, 15.0, 0.0, 0.0, 20.0, 20.0),
            (true, 10.0, 0.0, 0.0, 20.0, 15.0),
            (false, 10.0, 0.0, 0.0, 20.0, 10.0)
        ]
    );
    assert!(knife_plan(&[], 10.0, 10.0).is_none());
    // Dải: hàng 10 × 10 lên trên, con 5 × 3 xuống dưới; theo cột giữ nguyên.
    let slots = [cell(0.0, 0.0, 5.0, 3.0, 1), cell(0.0, 3.0, 10.0, 10.0, 0), cell(10.0, 3.0, 10.0, 10.0, 0)];
    let v = band_variants(&slots);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], [cell(0.0, 0.0, 10.0, 10.0, 0), cell(10.0, 0.0, 10.0, 10.0, 0), cell(0.0, 10.0, 5.0, 3.0, 1)]);
    assert_eq!(v[1], slots);
}

#[test]
fn ghep_cap_truoc_sau_theo_hang() {
    // Khung DanCard [trái, trên, phải, dưới]; hàng hai lệch 2 pt vẫn là một hàng.
    let recs = [
        Rec::new(0, 0.0, 100.0, 90.0, 45.0),
        Rec::new(1, 100.0, 100.0, 190.0, 45.0),
        Rec::new(2, 0.0, 40.0, 90.0, -15.0),
        Rec::new(3, 100.0, 42.0, 190.0, -13.0),
    ];
    assert_eq!(pair_records(&recs).unwrap(), [(0, 1), (2, 3)]);
    // Hàng lẻ.
    let e = pair_records(&recs[..3]).unwrap_err();
    assert!(e.starts_with("Không ghép rõ cặp trước/sau"), "{e}");
    // Chồng ngang.
    let overlap = [Rec::new(0, 0.0, 100.0, 90.0, 45.0), Rec::new(1, 80.0, 100.0, 170.0, 45.0)];
    assert!(pair_records(&overlap).unwrap_err().starts_with("Hai mặt nguồn đang chồng ngang"));
}

#[test]
fn doc_kho_giay_nhu_parse_float() {
    assert_eq!(js_parse_float("33"), Some(33.0));
    assert_eq!(js_parse_float("33cm"), Some(33.0));
    assert_eq!(js_parse_float(" 1e1x"), Some(10.0));
    assert_eq!(js_parse_float(".5"), Some(0.5));
    assert_eq!(js_parse_float("abc"), None);
    assert_eq!(js_parse_float("."), None);
    let p = json!({"width": "35,4", "height": 48, "bad": "", "neg": "-1"});
    assert!(close(paper_param(&p, "width", 33.0, "Ngang giấy").unwrap(), 35.4 * CM));
    assert!(close(paper_param(&p, "height", 33.0, "Dọc giấy").unwrap(), 48.0 * CM));
    assert!(close(paper_param(&p, "missing", 33.0, "Ngang giấy").unwrap(), 33.0 * CM));
    assert_eq!(paper_param(&p, "bad", 33.0, "Ngang giấy").unwrap_err().to_string(), "Ngang giấy phải là số cm lớn hơn 0.");
    assert_eq!(paper_param(&p, "neg", 33.0, "Dọc giấy").unwrap_err().to_string(), "Dọc giấy phải là số cm lớn hơn 0.");
}

// ---------------------------------------------------------------- qua Session

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Bài chữ nhật `w` × `h` pt ở (`x`, `y`), tô cyan `c`, không viền.
fn card(s: &mut Session, x: f64, y: f64, w: f64, h: f64, c: f64) -> NodeId {
    let id = NodeId(run(s, "shape.rectangle", json!({"x": x, "y": y, "width": w, "height": h}))["id"].as_u64().unwrap());
    run(s, "paint.setFill", json!({"color": {"c": c, "m": 0.2, "y": 0, "k": 0}, "ids": [id.0]}));
    run(s, "paint.setStroke", json!({"none": true, "ids": [id.0]}));
    id
}

fn new_doc(width: f64) -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": width, "height": 600, "colorMode": "cmyk"}));
    s
}

fn near(a: &Rect, b: &Rect) -> bool {
    [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1].iter().all(|d| d.abs() < 1e-6)
}

#[test]
fn dan_hai_mat_doi_xung_mot_buoc_hoan_tac() {
    let mut s = new_doc(800.0);
    let (w, h) = (9.0 * CM, 5.5 * CM);
    let front = card(&mut s, 20.0, 40.0, w, h, 1.0);
    let back = card(&mut s, 320.0, 40.0, w, h, 0.0);
    run(&mut s, "select.set", json!({"ids": [front.0, back.0]}));
    let layers_before = s.doc().unwrap().doc.layers.len();
    let old_board = s.doc().unwrap().doc.artboards[0].rect;
    let r = run(&mut s, "dancard.ktsLayout", json!({"width": "33", "height": "35,4", "twoSided": true}));
    let msg = r["message"].as_str().unwrap();
    assert!(msg.starts_with("OK:[[COUNT:20]] Đã tạo 1 cặp artboard trước/sau đối xứng và canh giữa artwork"), "{msg}");
    assert_eq!((r["count"].as_u64(), r["copies"].as_u64(), r["sheets"].as_u64(), r["artboards"].as_u64()), (Some(20), Some(40), Some(1), Some(2)));

    let st = s.doc().unwrap();
    let d = &st.doc;
    // Artboard cũ bị xóa; tên tờ mới đánh số như trước khi xóa.
    let names: Vec<&str> = d.artboards.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Dan toi uu - Mat truoc 2", "Dan toi uu - Mat sau 3"]);
    let (fa, ba) = (d.artboards[0].rect, d.artboards[1].rect);
    assert!(close(fa.width(), 33.0 * CM) && close(fa.height(), 35.4 * CM), "{fa:?}");
    assert!(close(ba.x0 - fa.x1, 10.0 * MM) && close(ba.y0, fa.y0) && close(ba.width(), fa.width()));
    assert!(close(fa.x0, old_board.x1 + 10.0 * MM), "tờ mới nằm bên phải mọi thứ đang có: {fa:?}");
    assert!(close(fa.y0, old_board.y0), "mép trên ngang mép trên của artboard cũ: {fa:?}");

    // Hai layer mới, mỗi layer 20 con; bài nguồn giữ nguyên.
    assert_eq!(d.layers.len(), layers_before + 2);
    let layer = |name: &str| d.layers.iter().find(|l| l.name.as_deref() == Some(name)).unwrap().clone();
    let boxes = |name: &str| -> Vec<Rect> { layer(name).children().unwrap().iter().map(|n| vis(n).unwrap()).collect() };
    let (fronts, backs) = (boxes("Dan toi uu - Mat truoc"), boxes("Dan toi uu - Mat sau"));
    assert_eq!((fronts.len(), backs.len()), (20, 20));
    assert!(d.node(front).is_some() && d.node(back).is_some());
    assert_eq!(st.selection.objects.len(), 40);
    assert_eq!(st.active_layer, Some(layer("Dan toi uu - Mat truoc").id));
    // 8 con xoay đứng; cụm con canh giữa tờ, trong lề 3 mm.
    assert_eq!(fronts.iter().filter(|b| b.height() > b.width()).count(), 8);
    let u = union_of(fronts.iter().copied()).unwrap();
    assert!(close(u.center().x, fa.center().x) && close(u.center().y, fa.center().y), "{u:?}");
    assert!(u.x0 >= fa.x0 + MARGIN - 1e-6 && u.x1 <= fa.x1 - MARGIN + 1e-6 && u.y0 >= fa.y0 + MARGIN - 1e-6 && u.y1 <= fa.y1 - MARGIN + 1e-6);
    // Mặt sau: mỗi con mặt trước có con đối xứng ngang ở cùng độ cao trên tờ sau.
    for f in &fronts {
        let (rx0, rx1) = (f.x0 - fa.x0, f.x1 - fa.x0);
        let want = Rect::new(ba.x0 + fa.width() - rx1, f.y0, ba.x0 + fa.width() - rx0, f.y1);
        assert!(backs.iter().any(|b| near(b, &want)), "thiếu con đối xứng của {f:?}");
    }

    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    let d = &s.doc().unwrap().doc;
    assert_eq!((d.artboards.len(), d.layers.len()), (1, layers_before));
    assert_eq!(d.artboards[0].name, "Artboard 1");
}

#[test]
fn nhieu_mau_khong_chia_deu_thi_hoi() {
    let mut s = new_doc(1200.0);
    let (w, h) = (9.0 * CM, 5.5 * CM);
    let ids: Vec<u64> = [20.0, 300.0, 580.0].iter().map(|x| card(&mut s, *x, 40.0, w, h, 0.5).0).collect();
    run(&mut s, "select.set", json!({"ids": ids}));
    let p = json!({"width": 33, "height": 35.4, "multiPerArtboard": true});
    let e = s.execute("dancard.ktsLayout", &p).unwrap_err().to_string();
    assert!(e.starts_with("20 vị trí trên 1 tờ không chia đều cho 3 mẫu. Nếu tiếp tục"), "{e}");
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 1, "chưa dàn gì");
    let e = s
        .execute("dancard.ktsLayout", &json!({"width": 33, "height": 35.4, "multiPerArtboard": true, "continueUneven": false}))
        .unwrap_err()
        .to_string();
    assert_eq!(e, "Đã dừng dàn vì số mẫu không chia đều cho số vị trí trên các tờ.");
    let r = run(&mut s, "dancard.ktsLayout", json!({"width": 33, "height": 35.4, "multiPerArtboard": true, "continueUneven": true}));
    assert_eq!(r["counts"], json!([7, 7, 6]));
    let msg = r["message"].as_str().unwrap();
    assert!(msg.ends_with("Đã dàn đủ 3 mẫu; 2 slot dư lặp mẫu đầu theo từng cụm."), "{msg}");
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 1);
}

#[test]
fn nhieu_kho_ghep_mot_to_xoa_nguon_da_dan() {
    let mut s = new_doc(800.0);
    let big = card(&mut s, 20.0, 20.0, 15.0 * CM, 10.0 * CM, 1.0);
    let small = card(&mut s, 20.0, 330.0, 9.0 * CM, 5.5 * CM, 0.0);
    run(&mut s, "select.set", json!({"ids": [big.0, small.0]}));
    let e = s.execute("dancard.ktsLayout", &json!({})).unwrap_err().to_string();
    assert_eq!(e, "Tick Dàn nhiều mẫu vào một tờ để ghép các kích thước khác nhau.");
    let r = run(&mut s, "dancard.ktsLayout", json!({"multiPerArtboard": true}));
    let msg = r["message"].as_str().unwrap();
    assert!(msg.starts_with("OK:[[COUNT:6]] Đã tạo 1 artboard"), "{msg}");
    assert!(msg.contains("số con từng mẫu: 6, 0.") && msg.contains("Đã xóa 1 mẫu nguồn đã dàn; còn 1 mẫu chưa dàn được giữ nguyên."), "{msg}");
    let d = &s.doc().unwrap().doc;
    assert!(d.node(big).is_none() && d.node(small).is_some());
    let layer = d.layers.iter().find(|l| l.name.as_deref() == Some("Dan toi uu")).unwrap();
    assert_eq!(layer.children().unwrap().len(), 6);
    assert_eq!(d.artboards.len(), 1);
    assert_eq!(d.artboards[0].name, "Dan toi uu 2");
}

#[test]
fn loi_nhu_dancard() {
    let mut s = new_doc(800.0);
    let a = card(&mut s, 20.0, 40.0, 9.0 * CM, 5.5 * CM, 1.0);
    run(&mut s, "select.set", json!({"ids": [a.0]}));
    let err = |s: &mut Session, p: Value| s.execute("dancard.ktsLayout", &p).unwrap_err().to_string();
    assert_eq!(err(&mut s, json!({"twoSided": true})), "Dàn 2 mặt cần chọn số object chẵn: từng cặp trái = trước, phải = sau.");
    assert_eq!(err(&mut s, json!({"width": "abc"})), "Ngang giấy phải là số cm lớn hơn 0.");
    assert_eq!(err(&mut s, json!({"height": "0,5"})), "Khổ giấy phải lớn hơn lề an toàn 3 mm mỗi cạnh.");
    assert_eq!(err(&mut s, json!({"width": "5", "height": "5"})), "Bài mẫu không nằm vừa trong vùng in sau khi trừ lề 3 mm.");
    // Bài quá nhỏ so với khổ giấy: chặn, không treo.
    let tiny = card(&mut s, 400.0, 400.0, 1.0, 1.0, 1.0);
    run(&mut s, "select.set", json!({"ids": [tiny.0]}));
    assert!(err(&mut s, json!({})).starts_with("Bài mẫu quá nhỏ so với khổ giấy"));
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 1);
    // Mỗi mẫu một tờ khi không tick nhiều mẫu.
    let b = card(&mut s, 20.0, 300.0, 9.0 * CM, 5.5 * CM, 0.0);
    run(&mut s, "select.set", json!({"ids": [a.0, b.0]}));
    let r = run(&mut s, "dancard.ktsLayout", json!({}));
    assert_eq!((r["sheets"].as_u64(), r["copies"].as_u64()), (Some(2), Some(40)));
    assert!(r["message"].as_str().unwrap().ends_with("Mỗi mẫu được dàn đầy trên một tờ riêng."));
    let d = &s.doc().unwrap().doc;
    let names: Vec<&str> = d.artboards.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Dan toi uu 2", "Dan toi uu 3"]);
    // Tờ thứ hai bên phải tờ đầu, cách 10 mm.
    assert!(close(d.artboards[1].rect.x0 - d.artboards[0].rect.x1, 10.0 * MM));
}
