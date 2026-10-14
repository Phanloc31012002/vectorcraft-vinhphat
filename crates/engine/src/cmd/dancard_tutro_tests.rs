//! Dàn Offset tự trở: vector đối chiếu (tính bằng chính mã của DanCard, sắp xếp ổn định) cho các hàm
//! tính, rồi vài lần dàn thật trên tài liệu tự dựng (các cặp hình chữ nhật).
#![allow(clippy::type_complexity)] // bảng số liệu kiểm thử viết thẳng

use super::*;

/// 1 cm theo hằng số mm của DanCard (như các vector đối chiếu).
const CM: f64 = 10.0 * MM;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.006
}

fn paper(w_cm: f64, h_cm: f64, margin_mm: f64, max_bop_mm: f64, repeat: bool) -> Paper {
    Paper { w: w_cm * 10.0 * MM, h: h_cm * 10.0 * MM, margin_mm, max_bop_mm, repeat }
}

/// Cách ghi bố cục của bảng vector: `row[RxC@a+…]`, `splitV(c){A | B}`.
fn plan_str(p: &Plan) -> String {
    match &p.shape {
        Shape::Empty => "empty".into(),
        Shape::Strips { row, bands } => {
            let b: Vec<String> = bands.iter().map(|b| format!("{}x{}@{}", b.repeat, b.cells, b.angle)).collect();
            format!("{}[{}]", if *row { "row" } else { "column" }, b.join("+"))
        }
        Shape::Split { vertical, cut, first, second } => {
            format!("{}({}){{{} | {}}}", if *vertical { "splitV" } else { "splitH" }, round2(*cut), plan_str(first), plan_str(second))
        }
    }
}

// ---------------------------------------------------------------- đọc số, thứ tự đọc, sắp xếp

#[test]
fn doc_so_nhu_parse_float_cua_dancard() {
    let w = |v: &str| number_text(Some(&json!(v)), "Ngang giấy", 0.1, None, false);
    let bop = |v: &str| number_text(Some(&json!(v)), "Bóp tối đa", 0.0, Some(50.0), false);
    let too_big = Err("Bóp tối đa không được lớn hơn 50.".to_string());
    assert_eq!(w("86"), Ok(86.0));
    assert_eq!(bop("86"), too_big);
    assert_eq!((w("8,6"), bop("8,6")), (Ok(8.6), Ok(8.6)));
    assert_eq!((w("86cm"), bop("86cm")), (Ok(86.0), too_big.clone()));
    assert_eq!((w(" 65"), bop(" 65")), (Ok(65.0), too_big.clone()));
    assert_eq!(w(""), Err("Ngang giấy phải là số.".to_string()));
    assert_eq!(bop(""), Err("Bóp tối đa phải là số.".to_string()));
    assert_eq!((w("abc"), bop("abc")), (Err("Ngang giấy phải là số.".to_string()), Err("Bóp tối đa phải là số.".to_string())));
    assert_eq!((w("0.095"), bop("0.095")), (Ok(0.095), Ok(0.095)));
    assert_eq!((w("0.05"), bop("0.05")), (Err("Ngang giấy không được nhỏ hơn 0.1.".to_string()), Ok(0.05)));
    assert_eq!((w("-1"), bop("-1")), (Err("Ngang giấy không được nhỏ hơn 0.1.".to_string()), Err("Bóp tối đa không được nhỏ hơn 0.".to_string())));
    assert_eq!((w("50.005"), bop("50.005")), (Ok(50.005), Ok(50.005)));
    assert_eq!((w("51"), bop("51")), (Ok(51.0), too_big));
    assert_eq!((w("1,5,2"), bop("1,5,2")), (Ok(1.5), Ok(1.5)));
    // Ô lề / bóp trống (hay thiếu) là 0; số JSON đọc thẳng.
    assert_eq!(number_text(Some(&json!("")), "Lề an toàn", 0.0, Some(50.0), true), Ok(0.0));
    assert_eq!(number_text(None, "Lề an toàn", 0.0, Some(50.0), true), Ok(0.0));
    assert_eq!(number_text(Some(&json!(43)), "Ngang giấy", 0.1, None, false), Ok(43.0));
    assert!(js_parse_float(".").is_nan() && js_parse_float("Infinity").is_infinite());
    assert_eq!((js_parse_float(".5"), js_parse_float("5.e1"), js_parse_float("-2e")), (0.5, 50.0, -2.0));
}

#[test]
fn thu_tu_doc_tung_hang_tu_tren_xuong() {
    // P1: hai hàng, hàng dưới lệch 2 pt vẫn cùng hàng.
    let p1 = [[0.0, 100.0, 90.0, 45.0], [100.0, 100.0, 190.0, 45.0], [0.0, 40.0, 90.0, -15.0], [100.0, 42.0, 190.0, -13.0]];
    assert_eq!(reading_order(&p1), vec![0, 1, 2, 3]);
    // P2: một hàng 4 con, xếp theo x.
    let p2 = [[200.0, 100.0, 290.0, 45.0], [0.0, 100.0, 90.0, 45.0], [300.0, 101.0, 390.0, 46.0], [100.0, 99.0, 190.0, 44.0]];
    assert_eq!(reading_order(&p2), vec![1, 3, 0, 2]);
    // P3: hàng 3 con: cặp thứ hai nằm vắt qua hai hàng.
    let p3 = [
        [0.0, 100.0, 90.0, 45.0],
        [100.0, 100.0, 190.0, 45.0],
        [200.0, 100.0, 290.0, 45.0],
        [0.0, 40.0, 90.0, -15.0],
        [100.0, 40.0, 190.0, -15.0],
        [200.0, 40.0, 290.0, -15.0],
    ];
    assert_eq!(reading_order(&p3), vec![0, 1, 2, 3, 4, 5]);
    assert!(reading_order(&[]).is_empty());
}

#[test]
fn sap_xep_on_dinh_khong_panic() {
    let v = js_sorted(vec![(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd'), (2, 'e')], |a, b| f64::from(a.0 - b.0));
    assert_eq!(v, vec![(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c'), (2, 'e')]);
    // Hàm so sánh có dung sai (không bắc cầu) vẫn cho đủ phần tử.
    let xs: Vec<f64> = (0..50).map(|i| f64::from(i % 7) * 0.3).collect();
    let sorted = js_sorted(xs.clone(), |a, b| if (a - b).abs() > 0.5 { a - b } else { 0.0 });
    assert_eq!(sorted.len(), xs.len());
}

// ---------------------------------------------------------------- bộ tính một khổ (§9.1)

struct Want {
    id: &'static str,
    piece: (f64, f64),
    sheet: (f64, f64),
    margin: f64,
    bop: f64,
    pairs: usize,
    repeat: bool,
    full_tight: Option<(u64, u64)>,
    pick_mm: (f64, f64),
    cell: (f64, f64),
    count: (u64, u64, u64),
    plan: &'static str,
    used: (usize, usize, usize),
    first: (f64, f64),
}

const SINGLES: &[Want] = &[
    Want {
        id: "S1",
        piece: (9.0, 5.5),
        sheet: (86.0, 65.0),
        margin: 0.0,
        bop: 1.0,
        pairs: 1,
        repeat: true,
        full_tight: Some((54, 57)),
        pick_mm: (0.8, 0.9),
        cell: (252.85, 153.35),
        count: (57, 36, 21),
        plan: "column[3x12@0+3x7@90]",
        used: (57, 0, 0),
        first: (0.28, 1842.52),
    },
    Want {
        id: "S2",
        piece: (9.0, 5.5),
        sheet: (86.0, 65.0),
        margin: 0.0,
        bop: 1.0,
        pairs: 1,
        repeat: false,
        full_tight: Some((54, 57)),
        pick_mm: (0.8, 0.9),
        cell: (252.85, 153.35),
        count: (57, 36, 21),
        plan: "column[3x12@0+3x7@90]",
        used: (1, 0, 56),
        first: (966.05, 1842.52),
    },
    Want {
        id: "S3",
        piece: (8.7, 5.5),
        sheet: (86.0, 65.0),
        margin: 0.0,
        bop: 1.0,
        pairs: 30,
        repeat: false,
        full_tight: Some((55, 60)),
        pick_mm: (1.0, 0.9),
        cell: (243.78, 153.35),
        count: (60, 60, 0),
        plan: "row[12x5@0]",
        used: (30, 0, 30),
        first: (0.0, 1842.52),
    },
    Want {
        id: "S4",
        piece: (8.7, 5.5),
        sheet: (86.0, 65.0),
        margin: 0.0,
        bop: 0.0,
        pairs: 30,
        repeat: false,
        full_tight: None,
        pick_mm: (0.0, 0.0),
        cell: (246.61, 155.91),
        count: (55, 30, 25),
        plan: "splitV(739.84){row[10x3@0+1x4@90] | row[7x3@90]}",
        used: (30, 0, 25),
        first: (11.34, 1842.52),
    },
    Want {
        id: "S5",
        piece: (14.8, 21.0),
        sheet: (86.0, 65.0),
        margin: 5.0,
        bop: 1.0,
        pairs: 3,
        repeat: false,
        full_tight: Some((8, 8)),
        pick_mm: (0.0, 0.0),
        cell: (419.53, 595.28),
        count: (8, 0, 8),
        plan: "row[4x2@90]",
        used: (3, 0, 5),
        first: (28.35, 1828.35),
    },
    Want {
        id: "S6",
        piece: (20.0, 30.0),
        sheet: (43.0, 32.5),
        margin: 0.0,
        bop: 1.0,
        pairs: 2,
        repeat: true,
        full_tight: Some((1, 1)),
        pick_mm: (0.0, 0.0),
        cell: (566.93, 850.39),
        count: (1, 1, 0),
        plan: "row[1x1@0]",
        used: (1, 0, 0),
        first: (42.52, 921.26),
    },
    Want {
        id: "S7",
        piece: (10.0, 10.0),
        sheet: (33.0, 35.4),
        margin: 0.0,
        bop: 2.0,
        pairs: 5,
        repeat: false,
        full_tight: Some((3, 3)),
        pick_mm: (0.0, 0.0),
        cell: (283.46, 283.46),
        count: (3, 3, 0),
        plan: "row[3x1@0]",
        used: (3, 2, 0),
        first: (184.25, 1003.46),
    },
    Want {
        id: "S10",
        piece: (9.2, 5.6),
        sheet: (33.0, 35.4),
        margin: 0.0,
        bop: 1.0,
        pairs: 1,
        repeat: true,
        full_tight: Some((9, 10)),
        pick_mm: (0.0, 1.0),
        cell: (260.79, 155.91),
        count: (10, 1, 9),
        plan: "row[1x1@0+3x3@90]",
        used: (10, 0, 0),
        first: (0.0, 1003.46),
    },
    Want {
        id: "S11",
        piece: (21.2, 30.0),
        sheet: (43.0, 32.5),
        margin: 0.0,
        bop: 2.0,
        pairs: 1,
        repeat: true,
        full_tight: Some((1, 1)),
        pick_mm: (0.0, 0.0),
        cell: (600.94, 850.39),
        count: (1, 1, 0),
        plan: "row[1x1@0]",
        used: (1, 0, 0),
        first: (8.5, 921.26),
    },
    Want {
        id: "S12",
        piece: (16.3, 16.3),
        sheet: (43.0, 32.5),
        margin: 0.0,
        bop: 2.0,
        pairs: 2,
        repeat: false,
        full_tight: Some((1, 2)),
        pick_mm: (0.0, 0.6),
        cell: (462.05, 460.35),
        count: (2, 2, 0),
        plan: "row[2x1@0]",
        used: (2, 0, 0),
        first: (147.4, 921.26),
    },
];

fn single(w: &Want) -> SinglePlan {
    let p = paper(w.sheet.0, w.sheet.1, w.margin, w.bop, w.repeat);
    plan_single(w.piece.0 * CM, w.piece.1 * CM, &p, w.pairs).unwrap_or_else(|e| panic!("{}: {e}", w.id))
}

#[test]
fn mot_kho_dung_vector_doi_chieu() {
    for w in SINGLES {
        let id = w.id;
        let (sw, sh) = (w.piece.0 * CM, w.piece.1 * CM);
        let p = paper(w.sheet.0, w.sheet.1, w.margin, w.bop, w.repeat);
        // Số con ở độ sâu 1, không bóp và bóp tối đa.
        if let Some((full, tight)) = w.full_tight {
            let max_bop = (w.bop * MM).min(sw.min(sh) * 0.2);
            let (half, usable_h) = ((p.w - 2.0 * w.margin * MM) / 2.0, p.h - 2.0 * w.margin * MM);
            assert_eq!(solve_layout(sw, sh, half, usable_h, 1).count, full, "{id}");
            assert_eq!(solve_layout(sw - max_bop, sh - max_bop, half, usable_h, 1).count, tight, "{id}");
        }
        let r = single(w);
        assert!(near(r.pick_dw / MM, w.pick_mm.0) && near(r.pick_dh / MM, w.pick_mm.1), "{id}: bóp {} × {}", r.pick_dw / MM, r.pick_dh / MM);
        assert!(near(r.cell_w, w.cell.0) && near(r.cell_h, w.cell.1), "{id}: {} × {}", r.cell_w, r.cell_h);
        assert_eq!((r.plan.count, r.plan.normal, r.plan.rotated), w.count, "{id}");
        assert_eq!(plan_str(&r.plan), w.plan, "{id}");
        assert_eq!((r.used, r.leftover, r.empty), w.used, "{id}");
        assert_eq!(r.slots.len() as u64, r.plan.count, "{id}");
        let s0 = r.slots[0];
        assert!(near(s0.x, w.first.0) && near(s0.y, w.first.1), "{id}: {s0:?}");
    }
}

/// Bảng chỗ (§9.1): (x, y, rộng, cao, góc) mặt trước, rồi (x, y, góc) mặt sau của các chỗ dùng tới.
fn check_slots(id: &str, r: &SinglePlan, front: &[(f64, f64, f64, f64, i32)], back: &[(f64, f64, i32)]) {
    for (k, &(x, y, w, h, a)) in front.iter().enumerate() {
        let s = r.slots[k];
        assert!(near(s.x, x) && near(s.y, y) && near(s.w, w) && near(s.h, h) && s.angle == a, "{id} #{k}: {s:?}");
    }
    assert_eq!(back.len(), r.used, "{id}");
    for (k, &(x, y, a)) in back.iter().enumerate() {
        let m = mirror(&r.slots[k], r.margin, r.usable_w);
        assert!(near(m.x, x) && near(m.y, y) && m.angle == a, "{id} sau #{k}: {m:?}");
    }
}

#[test]
fn mot_kho_bang_cho_va_mat_sau_doi_xung() {
    let by_id = |id: &str| SINGLES.iter().find(|w| w.id == id).map(single).unwrap();
    let r = by_id("S5");
    let (w, h) = (595.28, 419.53);
    check_slots(
        "S5",
        &r,
        &[
            (28.35, 1828.35, w, h, 90),
            (623.62, 1828.35, w, h, 90),
            (28.35, 1408.82, w, h, 90),
            (609.45, 1408.82, w, h, 90),
            (14.17, 989.29, w, h, 90),
            (609.45, 989.29, w, h, 90),
            (14.17, 569.76, w, h, 90),
            (609.45, 569.76, w, h, 90),
        ],
        &[(1814.17, 1828.35, -90), (1218.9, 1828.35, -90), (1814.17, 1408.82, -90)],
    );
    let r = by_id("S10");
    let (n, t) = ((260.79, 155.91), (155.91, 260.79));
    check_slots(
        "S10",
        &r,
        &[
            (0.0, 1003.46, n.0, n.1, 0),
            (0.0, 847.56, t.0, t.1, 90),
            (155.91, 847.56, t.0, t.1, 90),
            (311.81, 847.56, t.0, t.1, 90),
            (0.0, 586.77, t.0, t.1, 90),
            (155.91, 586.77, t.0, t.1, 90),
            (311.81, 586.77, t.0, t.1, 90),
            (0.0, 325.98, t.0, t.1, 90),
            (155.91, 325.98, t.0, t.1, 90),
            (311.81, 325.98, t.0, t.1, 90),
        ],
        &[
            (674.65, 1003.46, 0),
            (779.53, 847.56, -90),
            (623.62, 847.56, -90),
            (467.72, 847.56, -90),
            (779.53, 586.77, -90),
            (623.62, 586.77, -90),
            (467.72, 586.77, -90),
            (779.53, 325.98, -90),
            (623.62, 325.98, -90),
            (467.72, 325.98, -90),
        ],
    );
    assert_eq!(r.assignments.len(), 1);
    assert_eq!((r.assignments[0].0, r.assignments[0].1.len()), (0, 10), "lặp 1 cặp vào cả 10 chỗ");
    let r = by_id("S12");
    check_slots("S12", &r, &[(147.4, 921.26, 462.05, 460.35, 0), (147.4, 460.91, 462.05, 460.35, 0)], &[(609.45, 921.26, 0), (609.45, 460.91, 0)]);
    // S1: 12 chỗ đầu theo thứ tự đọc.
    let r = by_id("S1");
    let (n, t) = ((252.85, 153.35), (153.35, 252.85));
    let first12 = [
        (0.28, 1842.52, n.0, n.1, 0),
        (253.13, 1842.52, n.0, n.1, 0),
        (505.98, 1842.52, n.0, n.1, 0),
        (758.83, 1842.52, t.0, t.1, 90),
        (912.19, 1842.52, t.0, t.1, 90),
        (1065.54, 1842.52, t.0, t.1, 90),
        (0.28, 1689.17, n.0, n.1, 0),
        (253.13, 1689.17, n.0, n.1, 0),
        (505.98, 1689.17, n.0, n.1, 0),
        (758.83, 1589.67, t.0, t.1, 90),
        (912.19, 1589.67, t.0, t.1, 90),
        (1065.54, 1589.67, t.0, t.1, 90),
    ];
    for (k, &(x, y, w, h, a)) in first12.iter().enumerate() {
        let s = r.slots[k];
        assert!(near(s.x, x) && near(s.y, y) && near(s.w, w) && near(s.h, h) && s.angle == a, "S1 #{k}: {s:?}");
    }
    // S2: chỉ chỗ 0 được dồn sát tim giấy; các chỗ không dùng giữ nguyên.
    let r = by_id("S2");
    assert!(near(r.slots[1].x, 252.85) && near(r.slots[6].x, 0.0), "{:?}", &r.slots[..7]);
    // S4: các chỗ không dùng không bị dời.
    let r = by_id("S4");
    assert!(near(r.slots[30].x, 0.0) && near(r.slots[30].y, 907.09), "{:?}", r.slots[30]);
    // S6: lặp vòng nhưng chỉ 1 chỗ: cặp 2 không dàn, cũng không tính là còn lại (như DanCard).
    let r = by_id("S6");
    assert_eq!((r.assignments.len(), r.leftover, r.empty), (1, 0, 0));
    // S3: mỗi cặp một chỗ, theo thứ tự đọc.
    let r = by_id("S3");
    assert!(r.assignments.iter().enumerate().all(|(k, (p, s))| *p == k && s.len() == 1) && r.assignments.len() == 30);
}

#[test]
fn mot_kho_canh_giua_to_theo_cho() {
    // dy (y hướng lên) dời cả cụm cho giữa tờ; dx = 0 vì hai mặt đối xứng qua tim giấy.
    for (id, dy) in [("S1", -1.13), ("S2", -844.58), ("S5", -487.56), ("S10", -32.60), ("S12", -0.28)] {
        let r = single(SINGLES.iter().find(|w| w.id == id).unwrap());
        assert!(r.center.0.abs() < 1e-6 && near(r.center.1, dy), "{id}: {:?}", r.center);
    }
}

#[test]
fn mot_kho_bao_loi() {
    let p = paper(86.0, 65.0, 0.0, 1.0, false);
    assert_eq!(plan_single(50.0 * CM, 50.0 * CM, &p, 1).err().as_deref(), Some(NOT_FIT_SINGLE));
    let tiny = paper(1.0, 1.0, 5.0, 1.0, false);
    assert_eq!(plan_single(9.0 * CM, 5.5 * CM, &tiny, 1).err().as_deref(), Some("Khổ giấy phải lớn hơn lề an toàn hai bên."));
    // Bài quá nhỏ so với tờ: chặn trước khi tính.
    let e = plan_single(1.0, 1.0, &p, 1).err().unwrap_or_default();
    assert!(e.starts_with("Bài quá nhỏ"), "{e}");
}

#[test]
fn mot_kho_thong_bao_nhu_dancard() {
    let msg = |id: &str| {
        let w = SINGLES.iter().find(|w| w.id == id).unwrap();
        single_message(&paper(w.sheet.0, w.sheet.1, w.margin, w.bop, w.repeat), &single(w), w.pairs)
    };
    assert_eq!(
        msg("S1"),
        "OK:[[COUNT:57]] Sức chứa tờ 86 × 65 cm: 57 chỗ mỗi nửa. Đã dàn 57 con mặt trước + 57 con mặt sau (lặp lại 1 mẫu cho đầy tờ). KT con dàn: 8.92 × 5.41 cm (đã bóp ngang 0.8 mm, dọc 0.9 mm để dàn thêm được con). Lề an toàn 0 mm. Đã dàn kín tờ. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
    );
    assert_eq!(
        msg("S4"),
        "OK:[[COUNT:55]] Sức chứa tờ 86 × 65 cm: 55 chỗ mỗi nửa. Đã dàn 30 con mặt trước + 30 con mặt sau. KT con dàn: 8.7 × 5.5 cm (giữ nguyên, không cần bóp). Lề an toàn 0 mm. Hết mẫu, tờ còn trống 25 chỗ mỗi nửa. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
    );
    assert_eq!(
        msg("S7"),
        "OK:[[COUNT:3]] Sức chứa tờ 33 × 35.4 cm: 3 chỗ mỗi nửa. Đã dàn 3 con mặt trước + 3 con mặt sau. KT con dàn: 10 × 10 cm (giữ nguyên, không cần bóp). Lề an toàn 0 mm. Còn 2 cặp chưa dàn, đã để nguyên chỗ cũ để dàn tiếp vào tờ sau. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
    );
    // S6: lặp vòng hơn số chỗ vẫn báo "Đã dàn kín tờ." (như DanCard).
    let s6 = msg("S6");
    assert!(
        s6.contains("Đã dàn 1 con mặt trước + 1 con mặt sau. KT con dàn: 20 × 30 cm (giữ nguyên, không cần bóp). Lề an toàn 0 mm. Đã dàn kín tờ."),
        "{s6}"
    );
}

// ---------------------------------------------------------------- bộ xếp nhiều khổ (§9.2)

fn specs_cm(list: &[(f64, f64)]) -> Vec<(f64, f64)> {
    list.iter().map(|&(w, h)| (w * CM, h * CM)).collect()
}

/// (cặp, x, y, góc) mặt trước theo thứ tự đọc, rồi (x, góc) mặt sau cùng thứ tự.
fn check_mixed(id: &str, r: &MixedPlan, front: &[(usize, f64, f64, i32)], back: &[(f64, i32)]) {
    let slots: Vec<(usize, Slot)> = r.assignments.iter().flat_map(|(p, s)| s.iter().map(move |s| (*p, *s))).collect();
    assert_eq!(slots.len(), front.len(), "{id}");
    for (k, ((p, s), &(wp, x, y, a))) in slots.iter().zip(front).enumerate() {
        assert!(*p == wp && near(s.x, x) && near(s.y, y) && s.angle == a, "{id} #{k}: cặp {p} {s:?}");
        let (bx, ba) = back[k];
        let m = mirror(s, r.margin, r.usable_w);
        assert!(near(m.x, bx) && near(m.y, y) && m.angle == ba, "{id} sau #{k}: {m:?}");
    }
}

#[test]
fn nhieu_kho_dung_vector_doi_chieu() {
    // M1: thứ tự diện tích giảm dần xếp được hết ngay; 10 × 15 xoay vì cạnh thừa ngắn hơn.
    let specs = specs_cm(&[(9.0, 5.5), (10.0, 15.0), (9.0, 5.5)]);
    assert_eq!(build_orders(&specs), vec![vec![1, 0, 2], vec![0, 2, 1], vec![2, 0, 1], vec![0, 1, 2], vec![2, 1, 0]]);
    let r = plan_mixed(&specs, &paper(86.0, 65.0, 0.0, 1.0, false)).unwrap();
    assert!(near(r.pick_dw, 0.0) && near(r.pick_dh, 0.0));
    assert_eq!((r.best.placed.len(), r.best.normal), (3, 2));
    assert!(near(r.best.area, 200076.88), "{}", r.best.area);
    let want = [(1, 0.0, 0.0, 425.2, 283.46, 90), (0, 0.0, 283.46, 255.12, 155.91, 0), (2, 0.0, 439.37, 255.12, 155.91, 0)];
    for (p, w) in r.best.placed.iter().zip(want) {
        let at = near(p.x, w.1) && near(p.y, w.2) && near(p.w, w.3) && near(p.h, w.4);
        assert!(p.pair == w.0 && at && p.angle == w.5 && p.cut == Cut::Vertical, "{p:?}");
    }
    check_mixed("M1", &r, &[(1, 793.7, 1842.52, 90), (0, 793.7, 1559.06, 0), (2, 793.7, 1403.15, 0)], &[(1218.9, -90), (1388.98, 0), (1388.98, 0)]);

    // M2: nhiều cặp nhất (2 card + A5) thắng một tờ 20 × 30; 4 cặp còn lại giữ nguyên.
    let specs = specs_cm(&[(20.0, 30.0), (20.0, 30.0), (20.0, 30.0), (20.0, 30.0), (9.0, 5.5), (9.0, 5.5), (14.8, 21.0)]);
    let orders: Vec<Vec<usize>> = vec![
        vec![0, 1, 2, 3, 6, 4, 5],
        vec![1, 2, 3, 6, 4, 5, 0],
        vec![2, 3, 6, 4, 5, 0, 1],
        vec![5, 4, 6, 3, 2, 1, 0],
        vec![4, 6, 3, 2, 1, 0, 5],
        vec![6, 3, 2, 1, 0, 5, 4],
        vec![4, 5, 6, 0, 1, 2, 3],
        vec![5, 6, 0, 1, 2, 3, 4],
        vec![6, 0, 1, 2, 3, 4, 5],
    ];
    assert_eq!(build_orders(&specs), orders);
    let p2 = paper(43.0, 32.5, 0.0, 0.0, false);
    let r = plan_mixed(&specs, &p2).unwrap();
    assert_eq!((r.best.placed.len(), r.best.normal), (3, 2));
    assert!(near(r.best.area, 329283.15), "{}", r.best.area);
    let want = [(5, 0.0, 0.0, 255.12, 155.91, 0), (4, 255.12, 0.0, 255.12, 155.91, 0), (6, 0.0, 155.91, 595.28, 419.53, 90)];
    for (p, w) in r.best.placed.iter().zip(want) {
        let at = near(p.x, w.1) && near(p.y, w.2) && near(p.w, w.3) && near(p.h, w.4);
        assert!(p.pair == w.0 && at && p.angle == w.5 && p.cut == Cut::Horizontal, "{p:?}");
    }
    check_mixed("M2", &r, &[(5, 14.17, 921.26, 0), (4, 269.29, 921.26, 0), (6, 14.17, 765.35, 90)], &[(949.61, 0), (694.49, 0), (609.45, -90)]);
    assert_eq!(
        mixed_message(&p2, &r, 7),
        "OK:[[COUNT:3]] Tờ 43 × 32.5 cm: đã dàn 3 / 7 cặp. Mỗi cặp có 1 mặt trước + 1 mặt sau đối xứng tự trở. Giữ nguyên kích thước, không cần bóp. Lề an toàn 0 mm. Còn 4 cặp chưa dàn, giữ nguyên ở vị trí cũ để chạy tờ tiếp theo. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
    );

    // M3, M3b, M3c: bóp tổng (pt) ít nhất mà xếp được hết; hoà thì bóp dọc trước.
    let a4 = paper(43.0, 32.5, 0.0, 2.0, false);
    for (id, piece, pick, area, first_x, second_y) in [
        ("M3", (21.6, 15.0), (1.0, 0.0), 518271.44, 0.0, 496.06),
        ("M3b", (21.55, 16.3), (0.5, 0.5), 561460.72, 0.0, 460.63),
        ("M3c", (16.3, 16.3), (0.0, 0.5), 425665.57, 147.4, 460.63),
    ] {
        let specs = specs_cm(&[piece, piece]);
        assert_eq!(build_orders(&specs), vec![vec![0, 1], vec![1, 0]], "{id}");
        let r = plan_mixed(&specs, &a4).unwrap();
        assert!(near(r.pick_dw / MM, pick.0) && near(r.pick_dh / MM, pick.1), "{id}: {} {}", r.pick_dw / MM, r.pick_dh / MM);
        assert!(r.best.placed.len() == 2 && r.best.normal == 2 && near(r.best.area, area), "{id}: {}", r.best.area);
        check_mixed(id, &r, &[(0, first_x, 921.26, 0), (1, first_x, second_y, 0)], &[(609.45, 0), (609.45, 0)]);
    }

    // M4: lặp lại bị bỏ qua khi chọn nhiều cặp.
    let specs = specs_cm(&[(9.0, 5.5), (5.5, 9.0)]);
    let p4 = paper(33.0, 35.4, 0.0, 1.0, true);
    let r = plan_mixed(&specs, &p4).unwrap();
    assert!(r.best.placed.len() == 2 && r.best.normal == 1 && near(r.best.area, 79548.64), "{}", r.best.area);
    check_mixed("M4", &r, &[(0, 212.6, 1003.46, 0), (1, 212.6, 847.56, 90)], &[(467.72, 0), (467.72, -90)]);
    assert_eq!(
        mixed_message(&p4, &r, 2),
        "OK:[[COUNT:2]] Tờ 33 × 35.4 cm: đã dàn 2 / 2 cặp. Mỗi cặp có 1 mặt trước + 1 mặt sau đối xứng tự trở. Giữ nguyên kích thước, không cần bóp. Lề an toàn 0 mm. Đã bỏ qua Lặp lại mẫu vì đang chọn nhiều cặp: mỗi cặp chỉ dàn một lần. Đã dàn hết các cặp nguồn chọn vào tờ này. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
    );

    // M6: không cặp nào vừa nửa tờ.
    let specs = specs_cm(&[(50.0, 50.0), (45.0, 70.0)]);
    assert_eq!(plan_mixed(&specs, &paper(86.0, 65.0, 0.0, 1.0, false)).err().as_deref(), Some(NOT_FIT_MIXED));
}

// ---------------------------------------------------------------- dàn thật trên tài liệu

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

/// Hình chữ nhật tô cam, không viền, `w` × `h` pt ở (`x`, `y`), tên `name`.
fn rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64, name: &str) -> NodeId {
    let id = id_of(&run(s, "shape.rectangle", json!({"x": x, "y": y, "width": w, "height": h})));
    run(s, "paint.setFill", json!({"color": {"c": 0, "m": 0.6, "y": 1, "k": 0}, "ids": [id.0]}));
    run(s, "paint.setStroke", json!({"none": true, "ids": [id.0]}));
    let name = name.to_string();
    s.edit("Đặt tên", |d, _| {
        if let Some(n) = d.node_mut(id) {
            n.name = Some(name);
        }
        Ok(())
    })
    .unwrap();
    id
}

/// Tài liệu CMYK 800 × 600 pt, mỗi cặp một hàng: mặt trước "trước i" ở x = 0, mặt sau "sau i" ở
/// x = 700 (ngoài artboard); tất cả đang chọn.
fn session_with(pairs: &[((f64, f64), (f64, f64))]) -> (Session, Vec<(NodeId, NodeId)>) {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let mut ids = vec![];
    for (i, ((fw, fh), (bw, bh))) in pairs.iter().enumerate() {
        let y = i as f64 * 500.0;
        let f = rect(&mut s, 0.0, y, *fw, *fh, &format!("trước {i}"));
        let b = rect(&mut s, 700.0, y, *bw, *bh, &format!("sau {i}"));
        ids.push((f, b));
    }
    run(&mut s, "select.set", json!({"ids": ids.iter().flat_map(|(f, b)| [f.0, b.0]).collect::<Vec<_>>()}));
    (s, ids)
}

fn layer<'a>(d: &'a Document, name: &str) -> &'a Node {
    d.layers.iter().find(|l| l.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("thiếu layer {name}"))
}

fn named(list: &[Arc<Node>], name: &str) -> Rect {
    list.iter().find(|n| n.name.as_deref() == Some(name)).and_then(|n| n.geometric_bounds()).unwrap_or_else(|| panic!("thiếu {name}"))
}

#[test]
fn dan_mot_kho_hai_cap_doi_xung_qua_tim_giay() {
    let card = (9.0 * CM, 5.5 * CM);
    let (mut s, ids) = session_with(&[(card, card), (card, card)]);
    let undo = s.doc().unwrap().history.undo.len();
    let out = run(&mut s, "dancard.tuTro", json!({"width": "86", "height": "65", "margin": "0", "maxSqueeze": "1", "repeatFill": false}));
    assert_eq!(
        out["message"],
        json!(
            "OK:[[COUNT:57]] Sức chứa tờ 86 × 65 cm: 57 chỗ mỗi nửa. Đã dàn 2 con mặt trước + 2 con mặt sau. KT con dàn: 8.92 × 5.41 cm (đã bóp ngang 0.8 mm, dọc 0.9 mm để dàn thêm được con). Lề an toàn 0 mm. Hết mẫu, tờ còn trống 55 chỗ mỗi nửa. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
        )
    );
    let counts = (out["count"].as_u64(), out["placed"].as_u64(), out["leftover"].as_u64(), out["mixed"].as_bool());
    assert_eq!(counts, (Some(57), Some(2), Some(0), Some(false)));
    assert_eq!(out["squeezeMm"], json!([0.8, 0.9]));
    let st = s.doc().unwrap();
    let d = &st.doc;
    // Một artboard đúng khổ giấy, bên phải mọi bài; khung artboard cũ đã xoá.
    assert_eq!(d.artboards.len(), 1);
    assert_eq!(d.artboards[0].name, "Tự trở 2");
    let sheet = d.artboards[0].rect;
    assert!(near(sheet.width(), 86.0 * CM) && near(sheet.height(), 65.0 * CM), "{sheet:?}");
    assert!(near(sheet.x0, 700.0 + 9.0 * CM + PAGE_GAP) && near(sheet.y0, 0.0), "{sheet:?}");
    // Nguồn đã dàn bị xoá.
    assert!(ids.iter().all(|(f, b)| d.node(*f).is_none() && d.node(*b).is_none()));
    // Ba layer mới trên cùng: mặt trước, mặt sau, PON giấy (khóa, 8 nét ở 4 góc).
    let names: Vec<String> = d.layers.iter().map(|l| l.name.clone().unwrap_or_default()).collect();
    assert_eq!(&names[names.len() - 3..], [FRONT_LAYER, BACK_LAYER, PON_LAYER]);
    let fronts: &[Arc<Node>] = layer(d, FRONT_LAYER).children().unwrap();
    let backs: &[Arc<Node>] = layer(d, BACK_LAYER).children().unwrap();
    assert_eq!((fronts.len(), backs.len()), (2, 2));
    let pon = layer(d, PON_LAYER);
    assert!(pon.locked);
    let marks = pon.children().unwrap();
    assert_eq!(marks.len(), 8);
    let m0 = marks[0].geometric_bounds().unwrap();
    assert!(near(m0.x0, sheet.x0) && near(m0.y0, sheet.y0) && near(m0.width(), 3.0 * MM), "{m0:?}");
    // Đã bóp đúng khổ con dàn; mặt sau cùng hàng, đối xứng qua tim giấy; hàng canh giữa tờ.
    let mid = sheet.center();
    let cell = (9.0 * CM - 0.8 * MM, 5.5 * CM - 0.9 * MM);
    for i in 0..2 {
        let (f, b) = (named(fronts, &format!("trước {i}")), named(backs, &format!("sau {i}")));
        assert!(near(f.width(), cell.0) && near(f.height(), cell.1), "{f:?}");
        assert!(near(b.width(), cell.0) && near(b.height(), cell.1), "{b:?}");
        assert!(near(f.y0, b.y0) && near(f.center().x + b.center().x, 2.0 * mid.x), "{f:?} {b:?}");
        assert!(near(f.center().y, mid.y), "{f:?}");
    }
    // Cụm mặt trước dồn sát tim giấy: cặp 2 chạm tim, cặp 1 ngay bên trái.
    let (f0, f1) = (named(fronts, "trước 0"), named(fronts, "trước 1"));
    assert!(near(f1.x1, mid.x) && near(f0.x1, f1.x0), "{f0:?} {f1:?}");
    // Chọn sẵn 4 bản đã dàn.
    assert_eq!(st.selection.objects.len(), 4);
    // Một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    let st = s.doc().unwrap();
    assert_eq!(st.history.undo.len(), undo);
    assert_eq!(st.doc.artboards.len(), 1);
    assert_eq!(st.doc.artboards[0].name, "Artboard 1");
    assert!(ids.iter().all(|(f, b)| st.doc.node(*f).is_some() && st.doc.node(*b).is_some()));
    assert!(st.doc.layers.iter().all(|l| l.name.as_deref() != Some(FRONT_LAYER)));
}

#[test]
fn lap_lai_mot_cap_cho_day_to() {
    let card = (9.2 * CM, 5.6 * CM);
    let (mut s, _) = session_with(&[(card, card)]);
    let out = run(&mut s, "dancard.tuTro", json!({"width": 33, "height": "35,4", "maxSqueeze": 1, "repeatFill": true}));
    assert_eq!((out["count"].as_u64(), out["placed"].as_u64()), (Some(10), Some(10)), "{out}");
    let d = &s.doc().unwrap().doc;
    let fronts: &[Arc<Node>] = layer(d, FRONT_LAYER).children().unwrap();
    let backs: &[Arc<Node>] = layer(d, BACK_LAYER).children().unwrap();
    assert_eq!((fronts.len(), backs.len()), (10, 10));
    let mid = d.artboards[0].rect.center();
    let boxes = |list: &[Arc<Node>]| list.iter().map(|n| n.geometric_bounds().unwrap()).collect::<Vec<_>>();
    let (fb, bb) = (boxes(fronts), boxes(backs));
    // Bóp dọc 1 mm: 1 con ngang 260,79 × 155,91, 9 con xoay 155,91 × 260,79.
    assert_eq!(fb.iter().filter(|b| near(b.width(), 155.91) && near(b.height(), 260.79)).count(), 9, "{fb:?}");
    // Mỗi con mặt trước có đúng con mặt sau cùng khổ, cùng hàng, đối xứng qua tim giấy.
    for f in &fb {
        assert!(
            bb.iter().any(|b| near(f.center().x + b.center().x, 2.0 * mid.x) && near(f.center().y, b.center().y) && near(f.width(), b.width())),
            "{f:?}"
        );
    }
}

#[test]
fn cap_chua_vua_giu_nguyen_cho_cu() {
    let sq = (10.0 * CM, 10.0 * CM);
    let (mut s, ids) = session_with(&[(sq, sq); 5]);
    let out = run(&mut s, "dancard.tuTro", json!({"width": "33", "height": "35,4", "margin": "0", "maxSqueeze": "2"}));
    assert_eq!(
        out["message"],
        json!(
            "OK:[[COUNT:3]] Sức chứa tờ 33 × 35.4 cm: 3 chỗ mỗi nửa. Đã dàn 3 con mặt trước + 3 con mặt sau. KT con dàn: 10 × 10 cm (giữ nguyên, không cần bóp). Lề an toàn 0 mm. Còn 2 cặp chưa dàn, đã để nguyên chỗ cũ để dàn tiếp vào tờ sau. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
        )
    );
    let d = &s.doc().unwrap().doc;
    for (i, (f, b)) in ids.iter().enumerate() {
        assert_eq!((d.node(*f).is_some(), d.node(*b).is_some()), (i >= 3, i >= 3), "cặp {i}");
    }
    assert_eq!(layer(d, FRONT_LAYER).children().unwrap().len(), 3);
}

#[test]
fn dan_nhieu_kho_sat_le_tren_va_tim_giay() {
    let (card, a5) = ((9.0 * CM, 5.5 * CM), (10.0 * CM, 15.0 * CM));
    let (mut s, _) = session_with(&[(card, card), (a5, a5)]);
    let out = run(&mut s, "dancard.tuTro", json!({"width": 86, "height": 65, "margin": "", "maxSqueeze": "1"}));
    assert_eq!(
        out["message"],
        json!(
            "OK:[[COUNT:2]] Tờ 86 × 65 cm: đã dàn 2 / 2 cặp. Mỗi cặp có 1 mặt trước + 1 mặt sau đối xứng tự trở. Giữ nguyên kích thước, không cần bóp. Lề an toàn 0 mm. Đã dàn hết các cặp nguồn chọn vào tờ này. In xong lật ngang tờ giấy rồi in lại bằng chính bản kẽm đó."
        )
    );
    assert_eq!(out["mixed"], json!(true));
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 1);
    assert_eq!(d.artboards[0].name, "Tự trở nhiều khổ");
    let sheet = d.artboards[0].rect;
    let mid = sheet.center();
    let fronts: &[Arc<Node>] = layer(d, FRONT_LAYER).children().unwrap();
    let backs: &[Arc<Node>] = layer(d, BACK_LAYER).children().unwrap();
    // Khổ 10 × 15 xoay 90° nằm trên cùng, sát mép trên tờ và sát tim giấy; card ngay dưới nó.
    let (f1, b1) = (named(fronts, "trước 1"), named(backs, "sau 1"));
    assert!(near(f1.width(), 15.0 * CM) && near(f1.height(), 10.0 * CM), "{f1:?}");
    assert!(near(f1.y0, sheet.y0) && near(f1.x1, mid.x), "{f1:?} {sheet:?}");
    assert!(near(b1.width(), 15.0 * CM) && near(b1.x0, mid.x) && near(b1.y0, f1.y0), "{b1:?}");
    let (f0, b0) = (named(fronts, "trước 0"), named(backs, "sau 0"));
    assert!(near(f0.y0, sheet.y0 + 10.0 * CM) && near(f0.x0, f1.x0), "{f0:?}");
    assert!(near(f0.center().x + b0.center().x, 2.0 * mid.x) && near(b0.y0, f0.y0), "{f0:?} {b0:?}");
}

#[test]
fn bao_loi_nhu_dancard_va_khong_doi_tai_lieu() {
    let card = (9.0 * CM, 5.5 * CM);
    let err = |s: &mut Session, p: Value| s.execute("dancard.tuTro", &p).unwrap_err().to_string();
    let ok_paper = json!({"width": "86", "height": "65"});
    // Số object lẻ.
    let (mut s, ids) = session_with(&[(card, card)]);
    let extra = rect(&mut s, 0.0, 300.0, card.0, card.1, "thừa");
    let (f0, b0) = ids[0];
    run(&mut s, "select.set", json!({"ids": [f0.0, b0.0, extra.0]}));
    let undo = s.doc().unwrap().history.undo.len();
    let e = err(&mut s, ok_paper.clone());
    assert!(e.contains("Dàn tự trở cần chọn số object chẵn: từng cặp trái = mặt trước, phải = mặt sau. Tool nhận 3 object, đọc hợp lệ 3."), "{e}");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo, "lỗi không đổi tài liệu");
    // Ô nhập sai, giấy nhỏ hơn lề, bài không vừa nửa tờ.
    run(&mut s, "select.set", json!({"ids": [f0.0, b0.0]}));
    let undo = s.doc().unwrap().history.undo.len();
    assert!(err(&mut s, json!({"width": "abc", "height": "65"})).contains("Ngang giấy phải là số."));
    assert!(err(&mut s, json!({"width": "86", "height": ""})).contains("Dọc giấy phải là số."));
    assert!(err(&mut s, json!({"width": "86", "height": "65", "maxSqueeze": "51"})).contains("Bóp tối đa không được lớn hơn 50."));
    assert!(err(&mut s, json!({"width": "1", "height": "1", "margin": "5"})).contains("Khổ giấy phải lớn hơn lề an toàn hai bên."));
    assert!(err(&mut s, json!({"width": "10", "height": "5"})).contains(NOT_FIT_SINGLE));
    assert_eq!(s.doc().unwrap().history.undo.len(), undo, "lỗi không đổi tài liệu");
    assert_eq!(s.doc().unwrap().doc.artboards.len(), 1);
    // Không chọn gì.
    run(&mut s, "select.none", json!({}));
    assert!(err(&mut s, ok_paper.clone()).contains("Hãy chọn cặp mặt trước + mặt sau trước khi dàn."));
    // Hai mặt của một cặp lệch khung quá 0,5 mm.
    let (mut s, _) = session_with(&[(card, (9.0 * CM, 5.6 * CM))]);
    let e = err(&mut s, ok_paper.clone());
    assert!(e.contains("Cặp thứ 1 có mặt trước và mặt sau lệch khung quá 0.5 mm."), "{e}");
    // Cùng khổ với cặp đầu theo khổ lớn của cặp, nhưng mặt sau lệch quá 0,5 mm so với cặp đầu.
    let wide = |w_mm: f64| (w_mm * MM, 50.0 * MM);
    let (mut s, _) = session_with(&[(wide(100.0), wide(100.0)), (wide(99.6), wide(99.2))]);
    let e = err(&mut s, ok_paper);
    assert!(e.contains("Cặp thứ 2 (mặt sau) lệch quá 0.5 mm so với cặp đầu."), "{e}");
}
