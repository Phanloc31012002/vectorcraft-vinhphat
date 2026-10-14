//! CTL Offset: các vector đối chiếu của spec (`ctl_offset.md` §9, chạy nguyên văn JSX của DanCard)
//! cho các hàm tính, và các lần dàn thật qua `Session` với trang là ảnh (không làm phẳng).
//!
//! Khung của vector: mm tính từ góc trên trái artboard, x sang phải, y xuống dưới — đúng hệ trục
//! trang của VectorCraft, chỉ khác đơn vị.

use vectorcraft_doc::{ImageBlob, ImageObject};

// `Value`, `json`, `Document`, `Node`, `NodeId`, `NodeKind`, `Artboard`, `Affine`, `Point`, `Rect`
// và `Session` đến từ module cha.
use super::*;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn fail(s: &mut Session, id: &str, p: Value) -> String {
    match s.execute(id, &p) {
        Ok(v) => panic!("{id} chạy được: {v}"),
        Err(e) => e.to_string(),
    }
}

/// Số mm làm tròn 0,001 như bảng vector (bỏ số 0 thừa).
fn mm3(v: f64) -> String {
    let r = (v / MM * 1000.0).round() / 1000.0;
    // Không in "-0".
    if r == 0.0 { "0".into() } else { r.to_string() }
}

fn close(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 0.002, "{what}: {a} ≠ {b}");
}

fn tick(a: Point, b: Point, o: Point) -> String {
    format!("({},{})-({},{})", mm3(a.x - o.x), mm3(a.y - o.y), mm3(b.x - o.x), mm3(b.y - o.y))
}

fn ticks(lines: &[Line], o: Point) -> Vec<String> {
    lines.iter().map(|l| tick(l.a, l.b, o)).collect()
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

// ---------------------------------------------------------------- V0: kế hoạch mặt dàn

fn list(v: &[i64]) -> String {
    format!("[{}]", v.iter().map(i64::to_string).collect::<Vec<_>>().join(","))
}

/// Một mặt như bảng V0.
fn describe(f: &Face, keo: bool) -> String {
    let mut s = format!("{} | sheet {} | \"{}\" |", f.kind.key(), f.sheet, f.label);
    if !f.row.is_empty() {
        s.push_str(&format!(" row {}", list(&f.row)));
    }
    if !f.top.is_empty() {
        s.push_str(&format!(" top {} bottom {}", list(&f.top), list(&f.bottom)));
    }
    if let [l, r] = f.panels.as_slice() {
        s.push_str(&format!(" L{{top {} bot {}}} R{{top {} bot {}}}", list(&l.top), list(&l.bottom), list(&r.top), list(&r.bottom)));
    }
    let blocks: Vec<String> = f.blocks.iter().map(|b| format!("blk(rot {} top {} bot {})", b.rotation, list(&b.top), list(&b.bottom))).collect();
    if !blocks.is_empty() {
        s.push(' ');
        s.push_str(&blocks.join(" "));
    }
    if keo && f.kind == Kind::Tt8 {
        s.push_str(&format!(" | smallSelfTurn {}", f.small_self_turn));
    }
    s
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

#[test]
fn ke_hoach_dong_ghim_giua_v0() {
    let cases: [(i64, bool, bool, &str); 6] = [
        (
            28,
            true,
            false,
            r#"TT4 | sheet 0 | "TT4 BIA A4 (4 trang)" | row [28,1,2,27]
            TT8 | sheet 1 | "TU TRO A4 8 trang" | top [23,6,5,24] bottom [26,3,4,25]
            AB | sheet 2 | "To1 - A" | top [15,14,19,10] bottom [18,11,22,7]
            AB | sheet 2 | "To1 - B" | top [9,20,13,16] bottom [8,21,12,17]"#,
        ),
        (
            12,
            false,
            false,
            r#"TT4 | sheet 1 | "TU TRO A4 4 trang" | row [12,1,2,11]
            TT8 | sheet 2 | "TU TRO A4 8 trang" | top [7,6,5,8] bottom [10,3,4,9]"#,
        ),
        (
            36,
            false,
            false,
            r#"TT4 | sheet 1 | "TU TRO A4 4 trang" | row [36,1,2,35]
            AB | sheet 2 | "To1 - A" | top [27,10,31,6] bottom [30,7,34,3]
            AB | sheet 2 | "To1 - B" | top [5,32,9,28] bottom [4,33,8,29]
            AB | sheet 3 | "To2 - A" | top [19,18,23,14] bottom [22,15,26,11]
            AB | sheet 3 | "To2 - B" | top [13,24,17,20] bottom [12,25,16,21]"#,
        ),
        (
            60,
            true,
            true,
            r#"SMALL4 | sheet 0 | "BIA A5 (4 trang)" | row [60,1,2,59]
            SMALL8 | sheet 1 | "TU TRO A5 8 trang" | top [55,6,5,56] bottom [58,3,4,57]
            SMALL16 | sheet 2 | "TU TRO A5 16 trang" | L{top [47,14,51,10] bot [50,11,54,7]} R{top [9,52,13,48] bot [8,53,12,49]}
            SMALLAB | sheet 3 | "To1 - A" | L{top [39,22,43,18] bot [42,19,46,15]} R{top [31,30,35,26] bot [34,27,38,23]}
            SMALLAB | sheet 3 | "To1 - B" | L{top [17,44,21,40] bot [16,45,20,41]} R{top [25,36,29,32] bot [24,37,28,33]}"#,
        ),
        (4, false, true, r#"SMALL4 | sheet 1 | "TU TRO A5 4 trang" | row [4,1,2,3]"#),
        (
            44,
            false,
            true,
            r#"SMALL4 | sheet 1 | "TU TRO A5 4 trang" | row [44,1,2,43]
            SMALL8 | sheet 2 | "TU TRO A5 8 trang" | top [39,6,5,40] bottom [42,3,4,41]
            SMALLAB | sheet 3 | "To1 - A" | L{top [31,14,35,10] bot [34,11,38,7]} R{top [23,22,27,18] bot [26,19,30,15]}
            SMALLAB | sheet 3 | "To1 - B" | L{top [9,36,13,32] bot [8,37,12,33]} R{top [17,28,21,24] bot [16,29,20,25]}"#,
        ),
    ];
    for (n, cover, small, want) in cases {
        let got: Vec<String> = saddle_plan(n, cover, small).iter().map(|f| describe(f, false)).collect();
        assert_eq!(got, lines_of(want), "N={n} bìa={cover} A5={small}");
    }
    // Mặt B của tờ AB không nhận ghi chú.
    let faces = saddle_plan(28, true, false);
    assert_eq!(faces.iter().map(|f| f.side_b).collect::<Vec<_>>(), [false, false, false, true]);
}

#[test]
fn ke_hoach_keo_gay_v0() {
    let cases: [(i64, bool, &str); 3] = [
        (
            28,
            false,
            r#"AB | sheet 1 | "Ruột1-A" | top [5,12,9,8] bottom [4,13,16,1]
            AB | sheet 1 | "Ruột1-B" | top [7,10,11,6] bottom [2,15,14,3]
            TT8 | sheet 2 | "Ruột2 TT8" | top [25,20,19,26] bottom [28,17,18,27] | smallSelfTurn false
            TT4 | sheet 3 | "Ruột3 TT4" | row [21,24,22,23]"#,
        ),
        (
            60,
            true,
            r#"AB | sheet 1 | "Ruột1-A" | blk(rot 90 top [5,12,9,8] bot [4,13,16,1]) blk(rot 90 top [21,28,25,24] bot [20,29,32,17])
            AB | sheet 1 | "Ruột1-B" | blk(rot -90 top [23,26,27,22] bot [18,31,30,19]) blk(rot -90 top [7,10,11,6] bot [2,15,14,3])
            TT16 | sheet 2 | "Ruột2 TT16" | blk(rot 90 top [37,44,41,40] bot [36,45,48,33]) blk(rot -90 top [39,42,43,38] bot [34,47,46,35])
            TT8 | sheet 3 | "Ruột3 TT8" | top [57,52,51,58] bottom [60,49,50,59] | smallSelfTurn true
            TT4 | sheet 4 | "Ruột4 TT4" | row [53,56,54,55]"#,
        ),
        (
            44,
            false,
            r#"AB | sheet 1 | "Ruột1-A" | top [5,12,9,8] bottom [4,13,16,1]
            AB | sheet 1 | "Ruột1-B" | top [7,10,11,6] bottom [2,15,14,3]
            AB | sheet 2 | "Ruột2-A" | top [21,28,25,24] bottom [20,29,32,17]
            AB | sheet 2 | "Ruột2-B" | top [23,26,27,22] bottom [18,31,30,19]
            TT8 | sheet 3 | "Ruột3 TT8" | top [41,36,35,42] bottom [44,33,34,43] | smallSelfTurn false
            TT4 | sheet 4 | "Ruột4 TT4" | row [37,40,38,39]"#,
        ),
    ];
    for (n, small, want) in cases {
        let got: Vec<String> = keo_plan(n, small).unwrap().iter().map(|f| describe(f, true)).collect();
        assert_eq!(got, lines_of(want), "N={n} A5={small}");
    }
    for bad in [0, -4, 6] {
        assert_eq!(keo_plan(bad, false).unwrap_err(), "Số trang ruột phải là bội số của 4.");
    }
}

// ---------------------------------------------------------------- lưới, PON, khổ giấy

fn sheet_at_origin(kind: Kind, pw: f64, ph: f64) -> Rect {
    let (w, h) = paper_for(kind, pw * MM, ph * MM).unwrap();
    Rect::new(0.0, 0.0, w, h)
}

/// PON cắt Đóng ghim giữa của một tờ đặt ở gốc toạ độ (mm của trang `pw` × `ph`).
fn saddle_ticks(kind: Kind, pw: f64, ph: f64) -> Vec<String> {
    let r = sheet_at_origin(kind, pw, ph);
    let (lines, clamps) = saddle_cut_pon(kind, pw * MM, ph * MM, r).unwrap();
    assert_eq!(clamps, 0, "{kind:?}: không mốc nào bị kéo về mép");
    assert!(lines.iter().all(|l| l.width == 1.0));
    ticks(&lines, Point::ZERO)
}

const TT4_A4_205: &str = "(30.2,4)-(30.2,2) (30.2,414)-(30.2,416) (320.2,4)-(320.2,2) (320.2,414)-(320.2,416) (327.8,4)-(327.8,2)
    (327.8,414)-(327.8,416) (617.8,4)-(617.8,2) (617.8,414)-(617.8,416) (324,4)-(324,2) (324,414)-(324,416)";
const AB_A4_205: &str = "(15,23)-(15,21) (15,23)-(13,23) (15,313)-(13,313) (15,603)-(15,605) (15,603)-(13,603)
    (425,23)-(425,21) (425,23)-(427,23) (425,313)-(427,313) (425,603)-(425,605) (425,603)-(427,603)
    (433,23)-(433,21) (433,23)-(431,23) (433,313)-(431,313) (433,603)-(433,605) (433,603)-(431,603)
    (843,23)-(843,21) (843,23)-(845,23) (843,313)-(845,313) (843,603)-(843,605) (843,603)-(845,603)";
const SMALL4_148: &str = "(1.5,17)-(1.5,15) (1.5,17)-(3.5,17) (1.5,313)-(1.5,311) (1.5,313)-(3.5,313) (211.5,17)-(211.5,15)
    (211.5,17)-(213.5,17) (211.5,313)-(211.5,311) (211.5,313)-(213.5,313) (216.5,17)-(216.5,15) (216.5,17)-(214.5,17)
    (216.5,313)-(216.5,311) (216.5,313)-(214.5,313) (426.5,17)-(426.5,15) (426.5,17)-(424.5,17) (426.5,313)-(426.5,311)
    (426.5,313)-(424.5,313) (214,17)-(214,15) (214,313)-(214,311)";
const SMALL8_148: &str = "(24,0)-(24,2) (24,0)-(22,0) (24,209)-(22,209) (24,418)-(24,416) (24,418)-(22,418)
    (320,0)-(320,2) (320,0)-(322,0) (320,209)-(322,209) (320,418)-(320,416) (320,418)-(322,418)
    (328,0)-(328,2) (328,0)-(326,0) (328,209)-(326,209) (328,418)-(328,416) (328,418)-(326,418)
    (624,0)-(624,2) (624,0)-(626,0) (624,209)-(626,209) (624,418)-(624,416) (624,418)-(626,418)
    (324,0)-(324,2) (324,418)-(324,416)";
const SMALL16_148: &str = "(6,31)-(6,29) (6,31)-(4,31) (6,327)-(6,329) (6,327)-(4,327) (6,342)-(6,340)
    (6,342)-(4,342) (6,638)-(6,636) (6,638)-(4,638) (426,31)-(426,29) (426,31)-(428,31)
    (426,327)-(426,329) (426,327)-(428,327) (426,342)-(426,340) (426,342)-(428,342) (426,638)-(426,636)
    (426,638)-(428,638) (432,31)-(432,29) (432,31)-(430,31) (432,327)-(432,329) (432,327)-(430,327)
    (432,342)-(432,340) (432,342)-(430,342) (432,638)-(432,636) (432,638)-(430,638) (852,31)-(852,29)
    (852,31)-(854,31) (852,327)-(852,329) (852,327)-(854,327) (852,342)-(852,340) (852,342)-(854,342)
    (852,638)-(852,636) (852,638)-(854,638) (429,31)-(429,29) (429,327)-(429,329) (429,342)-(429,340)
    (429,638)-(429,636) (216,31)-(216,29) (216,327)-(216,329) (216,342)-(216,340) (216,638)-(216,636)
    (642,31)-(642,29) (642,327)-(642,329) (642,342)-(642,340) (642,638)-(642,636)";
const SMALLAB_148: &str = "(6,18)-(6,16) (6,18)-(4,18) (6,314)-(6,316) (6,314)-(4,314) (6,329)-(6,327)
    (6,329)-(4,329) (6,625)-(6,623) (6,625)-(4,625) (426,18)-(426,16) (426,18)-(428,18)
    (426,314)-(426,316) (426,314)-(428,314) (426,329)-(426,327) (426,329)-(428,329) (426,625)-(426,623)
    (426,625)-(428,625) (432,18)-(432,16) (432,18)-(430,18) (432,314)-(432,316) (432,314)-(430,314)
    (432,329)-(432,327) (432,329)-(430,329) (432,625)-(432,623) (432,625)-(430,625) (852,18)-(852,16)
    (852,18)-(854,18) (852,314)-(852,316) (852,314)-(854,314) (852,329)-(852,327) (852,329)-(854,329)
    (852,625)-(852,623) (852,625)-(854,625) (429,18)-(429,16) (429,314)-(429,316) (429,329)-(429,327)
    (429,625)-(429,623) (216,18)-(216,16) (216,314)-(216,316) (216,329)-(216,327) (216,625)-(216,623)
    (642,18)-(642,16) (642,314)-(642,316) (642,329)-(642,327) (642,625)-(642,623)";
const SMALL4_150: &str = "(0,13)-(0,11) (0,13)-(2,13) (0,313)-(0,311) (0,313)-(2,313) (211.5,13)-(211.5,11)
    (211.5,13)-(213.5,13) (211.5,313)-(211.5,311) (211.5,313)-(213.5,313) (216.5,13)-(216.5,11) (216.5,13)-(214.5,13)
    (216.5,313)-(216.5,311) (216.5,313)-(214.5,313) (428,13)-(428,11) (428,13)-(426,13) (428,313)-(428,311)
    (428,313)-(426,313) (214,13)-(214,11) (214,313)-(214,311)";
const SMALL8_150: &str = "(20,0)-(20,2) (20,0)-(18,0) (20,209)-(18,209) (20,418)-(20,416) (20,418)-(18,418)
    (320,0)-(320,2) (320,0)-(322,0) (320,209)-(322,209) (320,418)-(320,416) (320,418)-(322,418)
    (328,0)-(328,2) (328,0)-(326,0) (328,209)-(326,209) (328,418)-(328,416) (328,418)-(326,418)
    (628,0)-(628,2) (628,0)-(630,0) (628,209)-(630,209) (628,418)-(628,416) (628,418)-(630,418)
    (324,0)-(324,2) (324,418)-(324,416)";

#[test]
fn pon_cat_dong_ghim_giua_v1_v3_v5() {
    assert_eq!(saddle_ticks(Kind::Tt4, 205.0, 290.0), words(TT4_A4_205));
    assert_eq!(saddle_ticks(Kind::Tt8, 205.0, 290.0), words(AB_A4_205));
    assert_eq!(saddle_ticks(Kind::Ab, 205.0, 290.0), words(AB_A4_205));
    assert_eq!(saddle_ticks(Kind::Small4, 148.0, 210.0), words(SMALL4_148));
    assert_eq!(saddle_ticks(Kind::Small8, 148.0, 210.0), words(SMALL8_148));
    assert_eq!(saddle_ticks(Kind::Small16, 148.0, 210.0), words(SMALL16_148));
    assert_eq!(saddle_ticks(Kind::SmallAb, 148.0, 210.0), words(SMALLAB_148));
    assert_eq!(saddle_ticks(Kind::Small4, 150.0, 211.5), words(SMALL4_150));
    assert_eq!(saddle_ticks(Kind::Small8, 150.0, 211.5), words(SMALL8_150));
    // Số nét mỗi tờ (§3.11).
    for (kind, n) in
        [(Kind::Tt4, 10), (Kind::Tt8, 20), (Kind::Ab, 20), (Kind::Small4, 18), (Kind::Small8, 22), (Kind::Small16, 44), (Kind::SmallAb, 44)]
    {
        assert_eq!(saddle_ticks(kind, 140.0, 200.0).len(), n, "{kind:?}");
    }
}

#[test]
fn pon_giay_hai_loai() {
    let small = ticks(&paper_pon(Rect::new(0.0, 0.0, 648.0 * MM, 418.0 * MM), false), Point::ZERO);
    assert_eq!(
        small,
        words(
            "(0,0)-(5,0) (0,0)-(0,4.976) (643,0)-(648,0) (648,0)-(648,4.976) (0,418)-(5,418) (0,418)-(0,413.024) (643,418)-(648,418) (648,418)-(648,413.024)"
        )
    );
    let large = paper_pon(Rect::new(0.0, 0.0, 858.0 * MM, 638.0 * MM), true);
    assert_eq!(ticks(&large[..2], Point::ZERO), words("(0,0)-(6.62,0) (0,0)-(0,7.44)"));
    assert!(large.iter().all(|l| l.width == 3.971));
}

#[test]
fn bai_qua_kho_bao_loi_e1_e3() {
    let err = |kind: Kind, w: f64, h: f64| paper_for(kind, w * MM, h * MM).unwrap_err();
    assert_eq!(
        err(Kind::Tt4, 210.0, 297.0),
        "Khổ trang nhập quá lớn cho tờ 648 x 418 mm: bài cần 601.6 x 420 mm, dư dọc 2 mm. Hãy nhập lại khổ trang nhỏ hơn."
    );
    assert_eq!(
        err(Kind::Ab, 215.0, 305.0),
        "Khổ trang nhập quá lớn cho tờ 858 x 625 mm: bài cần 868 x 633 mm, dư ngang 10 mm và dọc 8 mm. Hãy nhập lại khổ trang nhỏ hơn."
    );
    assert_eq!(
        err(Kind::Ab, 200.0, 302.0),
        "Khổ trang nhập quá lớn cho tờ 858 x 625 mm: bài cần 808 x 627 mm, dư dọc 2 mm. Hãy nhập lại khổ trang nhỏ hơn."
    );
    // Giới hạn A4 bìa: rộng tối đa 209 mm.
    assert!(paper_for(Kind::Tt4, 209.0 * MM, 297.0 * MM).is_ok());
    // SMALL8 luôn vừa chiều cao nhờ allowance 2,5 mm mỗi hàng.
    assert!(paper_for(Kind::Small8, 150.0 * MM, 211.5 * MM).is_ok());
}

/// PON cắt Keo gáy của mặt `k` trong kế hoạch, trên tờ ở gốc toạ độ.
fn keo_ticks(n: i64, typed: (f64, f64), k: usize) -> Vec<String> {
    let small = is_a5(typed.0, typed.1);
    let faces = keo_plan(n, small).unwrap();
    let f = &faces[k];
    let (pw, ph) = (typed.0.min(typed.1), typed.0.max(typed.1));
    let fits = pw * MM * 4.0 + 4.0 * MM + 1.0 <= 648.0 * MM && ph * MM * 2.0 + 1.0 <= 418.0 * MM;
    let ((w, h), small_paper) = keo_paper(f, small, fits);
    let (fw, fh) = keo_frame(pw, ph, small, small_paper);
    let gap4 = (if small { 5.0 } else { 6.0 }) * MM;
    ticks(&keo_cut_pon(f, fw * MM, fh * MM, Rect::new(0.0, 0.0, w, h), gap4), Point::ZERO)
}

#[test]
fn pon_cat_keo_gay_v6_v7_v8() {
    let ab_a4 = "(9,21.7)-(9,23.7) (9,617.7)-(9,619.7) (429,21.7)-(429,23.7) (429,617.7)-(429,619.7) (849,21.7)-(849,23.7)
        (849,617.7)-(849,619.7) (9,23.7)-(7,23.7) (849,23.7)-(851,23.7) (9,320.7)-(7,320.7) (849,320.7)-(851,320.7)
        (9,617.7)-(7,617.7) (849,617.7)-(851,617.7)";
    assert_eq!(keo_ticks(28, (210.0, 297.0), 0), words(ab_a4));
    assert_eq!(keo_ticks(28, (210.0, 297.0), 2), words(ab_a4), "TT8 A4 cùng lưới");
    let tt4_a4 = "(25.214,1.99)-(25.214,0.176) (25.214,417.824)-(25.214,416.01) (321.2,1.99)-(321.2,0.176) (321.2,417.824)-(321.2,416.01) (324,1.99)-(324,0.176)
        (324,417.824)-(324,416.01) (326.8,1.99)-(326.8,0.176) (326.8,417.824)-(326.8,416.01) (622.786,1.99)-(622.786,0.176) (622.786,417.824)-(622.786,416.01)";
    assert_eq!(keo_ticks(28, (210.0, 297.0), 3), words(tt4_a4));
    let blocks = "(9,21.7)-(9,23.7) (9,615.7)-(9,617.7) (219,21.7)-(219,23.7) (219,615.7)-(219,617.7) (429,21.7)-(429,23.7)
        (429,615.7)-(429,617.7) (639,21.7)-(639,23.7) (639,615.7)-(639,617.7) (849,21.7)-(849,23.7) (849,615.7)-(849,617.7)
        (7,23.7)-(9,23.7) (849,23.7)-(851,23.7) (7,319.7)-(9,319.7) (849,319.7)-(851,319.7) (7,615.7)-(9,615.7)
        (849,615.7)-(851,615.7)";
    assert_eq!(keo_ticks(60, (148.0, 210.0), 0), words(blocks));
    assert_eq!(keo_ticks(60, (148.0, 210.0), 2), words(blocks), "TT16 cùng lưới khối");
    let tt8_a5 = "(133,107)-(133,109) (133,529)-(133,531) (429,107)-(429,109) (429,529)-(429,531) (725,107)-(725,109)
        (725,529)-(725,531) (133,109)-(131,109) (725,109)-(727,109) (133,319)-(131,319) (725,319)-(727,319)
        (133,529)-(131,529) (725,529)-(727,529)";
    assert_eq!(keo_ticks(60, (148.0, 210.0), 3), words(tt8_a5));
    let tt4_a5 = "(2.3,7.214)-(2.3,9.205) (2.3,303.795)-(2.3,305.786) (211.7,7.214)-(211.7,9.205) (211.7,303.795)-(211.7,305.786) (214,7.214)-(214,9.205)
        (214,303.795)-(214,305.786) (216.3,7.214)-(216.3,9.205) (216.3,303.795)-(216.3,305.786) (425.7,7.214)-(425.7,9.205) (425.7,303.795)-(425.7,305.786)";
    assert_eq!(keo_ticks(60, (148.0, 210.0), 4), words(tt4_a5));
    let tt8_small = "(34,0.176)-(34,2) (34,416)-(34,417.824) (324,0.176)-(324,2) (324,416)-(324,417.824) (614,0.176)-(614,2)
        (614,416)-(614,417.824) (34,2)-(32,2) (614,2)-(616,2) (34,209)-(32,209) (614,209)-(616,209)
        (34,416)-(32,416) (614,416)-(616,416)";
    assert_eq!(keo_ticks(8, (145.0, 207.0), 0), words(tt8_small));
}

#[test]
fn keo_gay_khung_va_ty_le() {
    let frame = |w: f64, h: f64, small: bool, small_paper: bool| {
        let (fw, fh) = keo_frame(w, h, small, small_paper);
        (format!("{fw:.3}"), format!("{fh:.3}"))
    };
    assert_eq!(frame(210.0, 297.0, false, true), ("209.000".into(), "295.586".into()), "A4 TT4: rộng về 20,9 cm");
    assert_eq!(frame(148.0, 210.0, true, true), ("147.295".into(), "209.000".into()), "A5 TT4: cao về 20,9 cm");
    assert_eq!(frame(148.0, 210.0, true, false), ("148.000".into(), "210.000".into()));
    assert_eq!(frame(145.0, 207.0, true, true), ("145.000".into(), "207.000".into()), "tờ nhỏ không phóng to");
    assert_eq!(keo_page_scales(100.0, 200.0, 50.0, 100.0), Ok((0.5, 0.5)));
    assert_eq!(keo_page_scales(0.0, 200.0, 50.0, 100.0).unwrap_err(), "Không đo được kích thước trang ruột.");
    assert_eq!(keo_page_scales(1e-320, 200.0, 50.0, 100.0).unwrap_err(), "Không tính được tỷ lệ kích thước trang ruột.");
    // Ô của lưới khối: trang 8 (hàng trên, cột 3 của khối xoay 90°) ở góc trên trái.
    let faces = keo_plan(60, true).unwrap();
    let (g, cells) = keo_packed_grid(&faces[0], 148.0 * MM, 210.0 * MM, Rect::new(0.0, 0.0, 858.0 * MM, 625.0 * MM), 23.7 * MM);
    assert_eq!((mm3(g.x0), mm3(g.y0), mm3(g.width()), mm3(g.height())), ("9".into(), "23.7".into(), "840".into(), "592".into()));
    let c8 = cells.iter().find(|c| c.page == 8).unwrap();
    assert_eq!((mm3(c8.center.x), mm3(c8.center.y), c8.angle), ("114".into(), "97.7".into(), 270.0));
}

// ---------------------------------------------------------------- vị trí tờ trên canvas (V9, E6–E8)

fn at_mm(rects: &[Rect], start: Point) -> Vec<String> {
    rects.iter().map(|r| format!("({},{})", mm3(r.x0 - start.x), mm3(r.y0 - start.y))).collect()
}

#[test]
fn cot_canvas_v9_va_loi_e6_e8() {
    let papers = [(858.0 * MM, 625.0 * MM); 8];
    let rows: Vec<(usize, Option<usize>)> = (0..4).map(|i| (2 * i, Some(2 * i + 1))).collect();
    // Toạ độ DanCard (x, y lên) → trang (x, −y).
    let start = Point::new(-6000.0, 3000.0);
    let (rects, cols) = place_sheets(&papers, &rows, start, 858.0 * MM, false).unwrap();
    assert_eq!(at_mm(&rects, start), words("(0,0) (878,0) (0,637) (878,637) (1756,0) (2634,0) (1756,637) (2634,637)"));
    assert_eq!(cols, 2);
    let start = Point::new(2000.0, 3000.0);
    let (rects, _) = place_sheets(&papers, &rows, start, 858.0 * MM, false).unwrap();
    assert_eq!(at_mm(&rects, start), words("(0,0) (878,0) (0,637) (878,637) (-1756,0) (-878,0) (-1756,637) (-878,637)"));
    let tt4 = [(648.0 * MM, 418.0 * MM)];
    assert_eq!(
        place_sheets(&tt4, &[(0, None)], Point::new(7000.0, -6000.0), 648.0 * MM, false).unwrap_err(),
        "Tờ CTL Offset đầu tiên không nằm vừa bên phải vị trí object nguồn."
    );
    assert_eq!(
        place_sheets(&papers, &rows, Point::new(-6000.0, 6000.0), 858.0 * MM, false).unwrap_err(),
        "Không đủ chiều cao canvas từ vị trí object nguồn để đặt một tờ CTL Offset."
    );
    assert_eq!(
        place_sheets(&papers, &rows, Point::new(-6000.0, 6000.0), 858.0 * MM, true).unwrap_err(),
        "Không đủ chiều cao canvas từ vị trí object nguồn để đặt một tờ CTL Keo Gáy."
    );
    assert_eq!(
        place_sheets(&tt4, &[(0, None)], Point::new(-7180.0, -6000.0), 648.0 * MM, false).unwrap_err(),
        "Object nguồn đang quá sát biên canvas; không thể bắt đầu dàn đúng tại vị trí đó."
    );
}

#[test]
fn ke_hoach_luu_theo_kho() {
    let faces = saddle_plan(36, false, false);
    let papers: Vec<(f64, f64)> = faces.iter().map(|f| paper_for(f.kind, 200.0 * MM, 280.0 * MM).unwrap()).collect();
    let rects: Vec<Rect> = papers.iter().map(|(w, h)| Rect::new(0.0, 0.0, *w, *h)).collect();
    let plan = size_plan(&faces, &rects);
    assert_eq!(plan["total"], 5);
    let ab = &plan["groups"][1];
    assert_eq!((ab["key"].as_str(), ab["label"].as_str()), (Some("AB"), Some("Tờ AB")));
    assert_eq!(
        ab["jobs"],
        json!([{"name": "RUOT 2", "ab": [2, 3], "w": 2432.13, "h": 1771.65}, {"name": "RUOT 3", "ab": [4, 5], "w": 2432.13, "h": 1771.65}])
    );
    assert_eq!(plan["groups"][0]["jobs"][0]["name"], "RUOT 1");
    assert_eq!(size_label("SMALL16"), "Tự trở 16 trang");
    assert_eq!(size_label("XYZ"), "XYZ");
}

#[test]
fn cho_ghi_chu_a5_tranh_bai_va_pon() {
    let rect = Rect::new(0.0, 0.0, 428.0 * MM, 313.0 * MM);
    let art = Rect::new(1.5 * MM, 17.0 * MM, 426.5 * MM, 313.0 * MM);
    // Dải trên bài cao 15 mm; khối 100 × 10 pt nằm ngay ở điểm ưu tiên.
    let at = a5_note_placement(rect, art, 100.0, 10.0, &[], Point::new(20.0, 5.0), false, false).unwrap();
    assert_eq!((at.x, at.y), (20.0, 5.0));
    // Một dấu PON ở đó: khối dời sang bên phải dấu, cách 0,5 mm.
    let pon = Rect::new(10.0, 0.0, 30.0, 20.0);
    let at = a5_note_placement(rect, art, 100.0, 10.0, &[pon], Point::new(20.0, 5.0), false, false).unwrap();
    assert!(at.x >= 30.0 + 0.5 * MM - 1e-9 || at.y >= 20.0 + 0.5 * MM - 1e-9, "{at:?}");
    // Không có chỗ trống đủ lớn.
    assert!(a5_note_placement(rect, art, 2000.0, 10.0, &[], Point::new(20.0, 5.0), false, false).is_none());
}

// ---------------------------------------------------------------- lần dàn thật

/// Tài liệu CMYK có `n` trang ảnh 100 × 140 pt tên p1…pn, xếp hàng 8 theo thứ tự đọc từ góc trên
/// trái `anchor`, đang chọn hết.
fn pages_session(n: usize, anchor: Point) -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 800, "height": 600, "colorMode": "cmyk"}));
    let mut png = Vec::new();
    image::RgbaImage::from_raw(2, 2, vec![200; 16]).unwrap().write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    s.edit("Trang", |d, sel| {
        d.images.insert("ctl-page".into(), ImageBlob::png(png));
        let layer = d.default_layer();
        let mut ids = vec![];
        for i in 0..n {
            let (x, y) = (anchor.x + (i % 8) as f64 * 120.0, anchor.y + (i / 8) as f64 * 300.0);
            let im = ImageObject {
                key: "ctl-page".into(),
                width: 2,
                height: 2,
                xf: Affine::new([50.0, 0.0, 0.0, 70.0, x, y]),
                link: None,
                placement: Default::default(),
            };
            let mut node = Node::new(d.alloc_id(), NodeKind::Image(Box::new(im)));
            node.name = Some(format!("p{}", i + 1));
            ids.push(d.insert(layer, usize::MAX, node)?);
        }
        sel.set(ids);
        Ok(())
    })
    .unwrap();
    s
}

fn find_named<'a>(nodes: &'a [std::sync::Arc<Node>], name: &str) -> Option<&'a Node> {
    for n in nodes {
        if n.name.as_deref() == Some(name) && !n.is_layer() {
            return Some(n);
        }
        if let Some(found) = n.children().and_then(|c| find_named(c, name)) {
            return Some(found);
        }
    }
    None
}

/// Góc xoay thật của một trang ảnh (độ, ngược chiều kim đồng hồ, trong (−180, 180]).
fn rotation(n: &Node) -> f64 {
    let NodeKind::Image(im) = &n.kind else { panic!("không phải ảnh") };
    let c = im.xf.as_coeffs();
    let r = (-c[1]).atan2(c[0]).to_degrees().round();
    if r <= -180.0 { r + 360.0 } else { r }
}

fn layer_lines(d: &Document, name: &str) -> (Vec<Line>, bool) {
    let layer = d.layers.iter().find(|l| l.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("thiếu layer {name}"));
    let lines = layer
        .children()
        .unwrap()
        .iter()
        .map(|n| {
            let NodeKind::Path { path, .. } = &n.kind else { panic!("PON không phải path") };
            let pts: Vec<Point> = path.anchors().map(|(_, _, a)| a.p).collect();
            Line { a: pts[0], b: pts[1], width: 1.0 }
        })
        .collect();
    (lines, layer.locked)
}

/// Một artboard của bảng vector: vị trí so với góc vùng chọn, khổ, các trang, các nét PON cắt.
struct Sheet {
    at: (f64, f64),
    size: (f64, f64),
    pages: Vec<(String, f64, f64, f64, f64, f64)>,
    cut: Vec<String>,
}

fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("số {s}"))
}

fn parse_sheets(text: &str) -> Vec<Sheet> {
    let mut out: Vec<Sheet> = vec![];
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.starts_with("AB") {
            let at = line.split("at(").nth(1).and_then(|r| r.split(')').next()).unwrap();
            let (x, y) = at.split_once(',').unwrap();
            let size = line.split("size ").nth(1).unwrap();
            let (w, h) = size.split_once('x').unwrap();
            out.push(Sheet { at: (num(x), num(y)), size: (num(w), num(h)), pages: vec![], cut: vec![] });
        } else if line.starts_with('p') {
            for tok in line.split_whitespace() {
                let (name, rest) = tok.split_once("@(").unwrap();
                let (c, rest) = rest.split_once(')').unwrap();
                let (cx, cy) = c.split_once(',').unwrap();
                let (w, rest) = rest.split_once('x').unwrap();
                let (h, r) = rest.split_once('r').unwrap();
                out.last_mut().unwrap().pages.push((name.to_string(), num(cx), num(cy), num(w), num(h), num(r)));
            }
        } else if line.starts_with('(') {
            out.last_mut().unwrap().cut.extend(words(line));
        }
    }
    out
}

/// So tài liệu sau lần dàn với bảng vector `text` (góc vùng chọn `anchor`, layer PON cắt `cut_layer`).
fn check_sheets(s: &Session, anchor: Point, text: &str, cut_layer: &str, paper_layer: &str) {
    let d = &s.doc().unwrap().doc;
    let sheets = parse_sheets(text);
    assert_eq!(d.artboards.len(), sheets.len(), "số artboard");
    let (cut, cut_locked) = layer_lines(d, cut_layer);
    let (paper, paper_locked) = layer_lines(d, paper_layer);
    assert!(cut_locked && paper_locked, "PON khóa");
    let names: Vec<_> = d.layers.iter().rev().take(2).map(|l| l.name.clone().unwrap_or_default()).collect();
    assert_eq!(names, [cut_layer, paper_layer], "PON cắt trên cùng, PON giấy thứ hai");
    assert_eq!(paper.len(), 8 * sheets.len());
    let mut k = 0;
    for (i, (sh, ab)) in sheets.iter().zip(&d.artboards).enumerate() {
        let o = ab.rect.origin();
        let tag = format!("AB{}", i + 1);
        close((o.x - anchor.x) / MM, sh.at.0, &format!("{tag} x"));
        close((o.y - anchor.y) / MM, sh.at.1, &format!("{tag} y"));
        close(ab.rect.width() / MM, sh.size.0, &format!("{tag} rộng"));
        close(ab.rect.height() / MM, sh.size.1, &format!("{tag} cao"));
        for (name, cx, cy, w, h, r) in &sh.pages {
            let n = find_named(&d.layers, name).unwrap_or_else(|| panic!("{tag}: thiếu {name}"));
            let b = n.geometric_bounds().unwrap();
            close((b.center().x - o.x) / MM, *cx, &format!("{tag} {name} tâm x"));
            close((b.center().y - o.y) / MM, *cy, &format!("{tag} {name} tâm y"));
            close(b.width() / MM, *w, &format!("{tag} {name} rộng"));
            close(b.height() / MM, *h, &format!("{tag} {name} cao"));
            assert_eq!(rotation(n), *r, "{tag} {name} góc");
        }
        let got = ticks(cut.get(k..k + sh.cut.len()).unwrap_or_else(|| panic!("{tag}: thiếu PON cắt")), o);
        assert_eq!(got, sh.cut, "{tag}: PON cắt");
        k += sh.cut.len();
    }
    assert_eq!(k, cut.len(), "tổng số nét PON cắt");
}

/// `sizes` trả về, bỏ nhãn nút, so với đuôi CTLPDF của DanCard.
fn check_sizes(v: &Value, tail: &str) {
    let mut got = v["sizes"].clone();
    for g in got["groups"].as_array_mut().unwrap() {
        g.as_object_mut().unwrap().remove("label");
    }
    assert_eq!(got, serde_json::from_str::<Value>(tail).unwrap());
}

const V1: &str = r#"
AB1 BIA / BIA name="" at(0,0) size 648x418
  p1@(175.2,106.5)290x205r90  p2@(472.8,106.5)290x205r-90  p28@(175.2,311.5)290x205r90  p27@(472.8,311.5)290x205r-90
  cut 10 (1pt):
    (30.2,4)-(30.2,2) (30.2,414)-(30.2,416) (320.2,4)-(320.2,2) (320.2,414)-(320.2,416) (327.8,4)-(327.8,2)
    (327.8,414)-(327.8,416) (617.8,4)-(617.8,2) (617.8,414)-(617.8,416) (324,4)-(324,2) (324,414)-(324,416)
AB2 TT8 / RUOT 1 name="" at(0,430) size 858x638
  p23@(117.75,168)204.5x290r180  p6@(322.25,168)204.5x290r180  p5@(535.75,168)204.5x290r180  p24@(740.25,168)204.5x290r180
  p26@(117.75,458)204.5x290r0  p3@(322.25,458)204.5x290r0  p4@(535.75,458)204.5x290r0  p25@(740.25,458)204.5x290r0
  cut 20 (1pt):
    (15,23)-(15,21) (15,23)-(13,23) (15,313)-(13,313) (15,603)-(15,605) (15,603)-(13,603)
    (425,23)-(425,21) (425,23)-(427,23) (425,313)-(427,313) (425,603)-(425,605) (425,603)-(427,603)
    (433,23)-(433,21) (433,23)-(431,23) (433,313)-(431,313) (433,603)-(433,605) (433,603)-(431,603)
    (843,23)-(843,21) (843,23)-(845,23) (843,313)-(845,313) (843,603)-(843,605) (843,603)-(845,603)
AB3 AB / RUOT 2 name="" at(0,1080) size 858x625
  p15@(118,168)204x290r180  p14@(322,168)204x290r180  p19@(536,168)204x290r180  p10@(740,168)204x290r180
  p18@(118,458)204x290r0  p11@(322,458)204x290r0  p22@(536,458)204x290r0  p7@(740,458)204x290r0
  cut 20 (1pt):
    (15,23)-(15,21) (15,23)-(13,23) (15,313)-(13,313) (15,603)-(15,605) (15,603)-(13,603)
    (425,23)-(425,21) (425,23)-(427,23) (425,313)-(427,313) (425,603)-(425,605) (425,603)-(427,603)
    (433,23)-(433,21) (433,23)-(431,23) (433,313)-(431,313) (433,603)-(433,605) (433,603)-(431,603)
    (843,23)-(843,21) (843,23)-(845,23) (843,313)-(845,313) (843,603)-(843,605) (843,603)-(845,603)
AB4 AB / RUOT 2 name="" at(878,1080) size 858x625
  p9@(118,168)204x290r180  p20@(322,168)204x290r180  p13@(536,168)204x290r180  p16@(740,168)204x290r180
  p8@(118,458)204x290r0  p21@(322,458)204x290r0  p12@(536,458)204x290r0  p17@(740,458)204x290r0
  cut 20 (1pt):
    (15,23)-(15,21) (15,23)-(13,23) (15,313)-(13,313) (15,603)-(15,605) (15,603)-(13,603)
    (425,23)-(425,21) (425,23)-(427,23) (425,313)-(427,313) (425,603)-(425,605) (425,603)-(427,603)
    (433,23)-(433,21) (433,23)-(431,23) (433,313)-(431,313) (433,603)-(433,605) (433,603)-(431,603)
    (843,23)-(843,21) (843,23)-(845,23) (843,313)-(845,313) (843,603)-(843,605) (843,603)-(845,603)
"#;

#[test]
fn dong_ghim_giua_v1_a4_co_bia_28_trang() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(28, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 20.5, "height": "29", "cover": true, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL Offset; đã vẽ 70 nét PON cắt.\n"), "{v}");
    assert_eq!(
        v["summary"],
        "Đã dàn 4 artboard.\nDư 8 -> tự trở trước (4 và/hoặc 8), rồi các tờ AB; PON giữ lưới gốc.\nTT4 dùng PON tự vẽ theo kích thước đã nhập.\nPON tự vẽ nằm trên cùng. Đã bỏ qua ghi chú."
    );
    check_sizes(
        &v,
        r#"{"total":4,"groups":[{"key":"BIA","jobs":[{"name":"BIA","ab":[1],"w":1836.85,"h":1184.88}]},{"key":"TT8","jobs":[{"name":"RUOT 1","ab":[2],"w":2432.13,"h":1808.5}]},{"key":"AB","jobs":[{"name":"RUOT 2","ab":[3,4],"w":2432.13,"h":1771.65}]}]}"#,
    );
    assert_eq!(v["artboards"], json!(["Bìa", "Ruột1 TT8", "Ruột2-A", "Ruột2-B"]));
    check_sheets(&s, anchor, V1, SIG_CUT_LAYER, SIG_PAPER_LAYER);
    // Cả lần dàn là một bước hoàn tác.
    run(&mut s, "edit.undo", json!({}));
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 1);
    assert!(d.layers.iter().all(|l| l.name.as_deref() != Some(SIG_CUT_LAYER)));
}

const V2: &str = r#"
AB1 TT4 / RUOT 1 name="" at(0,0) size 648x418
  p1@(175.2,106.75)290x204.5r90  p2@(472.8,106.75)290x204.5r-90  p12@(175.2,311.25)290x204.5r90  p11@(472.8,311.25)290x204.5r-90
  cut 10 (1pt):
    (30.2,4)-(30.2,2) (30.2,414)-(30.2,416) (320.2,4)-(320.2,2) (320.2,414)-(320.2,416) (327.8,4)-(327.8,2)
    (327.8,414)-(327.8,416) (617.8,4)-(617.8,2) (617.8,414)-(617.8,416) (324,4)-(324,2) (324,414)-(324,416)
AB2 TT8 / RUOT 2 name="" at(0,430) size 858x638
  p7@(118,168)204x290r180  p6@(322,168)204x290r180  p5@(536,168)204x290r180  p8@(740,168)204x290r180
  p10@(118,458)204x290r0  p3@(322,458)204x290r0  p4@(536,458)204x290r0  p9@(740,458)204x290r0
  cut 20 (1pt):
    (15,23)-(15,21) (15,23)-(13,23) (15,313)-(13,313) (15,603)-(15,605) (15,603)-(13,603)
    (425,23)-(425,21) (425,23)-(427,23) (425,313)-(427,313) (425,603)-(425,605) (425,603)-(427,603)
    (433,23)-(433,21) (433,23)-(431,23) (433,313)-(431,313) (433,603)-(433,605) (433,603)-(431,603)
    (843,23)-(843,21) (843,23)-(845,23) (843,313)-(845,313) (843,603)-(843,605) (843,603)-(845,603)
"#;

#[test]
fn dong_ghim_giua_v2_tt4_ruot_bop_1mm() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(12, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": "20,5", "height": 29, "cover": false, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL Offset; đã vẽ 30 nét PON cắt.\n"));
    assert_eq!(
        v["summary"],
        "Đã dàn 2 artboard.\nDư 12 -> tự trở trước (4 và/hoặc 8), rồi các tờ AB; PON giữ lưới gốc.\nTT4 dùng PON tự vẽ theo kích thước đã nhập.\nPON tự vẽ nằm trên cùng. Đã bỏ qua ghi chú."
    );
    check_sizes(
        &v,
        r#"{"total":2,"groups":[{"key":"TT4","jobs":[{"name":"RUOT 1","ab":[1],"w":1836.85,"h":1184.88}]},{"key":"TT8","jobs":[{"name":"RUOT 2","ab":[2],"w":2432.13,"h":1808.5}]}]}"#,
    );
    check_sheets(&s, anchor, V2, SIG_CUT_LAYER, SIG_PAPER_LAYER);
}

const V3: &str = r#"
AB1 BIA / BIA name="" at(0,0) size 428x313
  p1@(106.5,91)210x148r90  p2@(321.5,91)210x148r-90  p60@(106.5,239)210x148r90  p59@(321.5,239)210x148r-90
  cut 18 (1pt):
    (1.5,17)-(1.5,15) (1.5,17)-(3.5,17) (1.5,313)-(1.5,311) (1.5,313)-(3.5,313) (211.5,17)-(211.5,15)
    (211.5,17)-(213.5,17) (211.5,313)-(211.5,311) (211.5,313)-(213.5,313) (216.5,17)-(216.5,15) (216.5,17)-(214.5,17)
    (216.5,313)-(216.5,311) (216.5,313)-(214.5,313) (426.5,17)-(426.5,15) (426.5,17)-(424.5,17) (426.5,313)-(426.5,311)
    (426.5,313)-(424.5,313) (214,17)-(214,15) (214,313)-(214,311)
AB2 SMALL8 / RUOT 1 name="" at(0,325) size 648x418
  p55@(98.25,105)147.5x210r180  p6@(245.75,105)147.5x210r180  p5@(402.25,105)147.5x210r180  p56@(549.75,105)147.5x210r180
  p58@(98.25,313)147.5x210r0  p3@(245.75,313)147.5x210r0  p4@(402.25,313)147.5x210r0  p57@(549.75,313)147.5x210r0
  cut 22 (1pt):
    (24,0)-(24,2) (24,0)-(22,0) (24,209)-(22,209) (24,418)-(24,416) (24,418)-(22,418)
    (320,0)-(320,2) (320,0)-(322,0) (320,209)-(322,209) (320,418)-(320,416) (320,418)-(322,418)
    (328,0)-(328,2) (328,0)-(326,0) (328,209)-(326,209) (328,418)-(328,416) (328,418)-(326,418)
    (624,0)-(624,2) (624,0)-(626,0) (624,209)-(626,209) (624,418)-(624,416) (624,418)-(626,418)
    (324,0)-(324,2) (324,418)-(324,416)
AB3 SMALL16 / RUOT 2 name="" at(0,755) size 858x638
  p10@(111,105.5)210x147r-90  p7@(321,105.5)210x147r90  p8@(537,105.5)210x147r-90  p9@(747,105.5)210x147r90
  p51@(111,252.5)210x147r-90  p54@(321,252.5)210x147r90  p53@(537,252.5)210x147r-90  p52@(747,252.5)210x147r90
  p14@(111,416.5)210x147r-90  p11@(321,416.5)210x147r90  p12@(537,416.5)210x147r-90  p13@(747,416.5)210x147r90
  p47@(111,563.5)210x147r-90  p50@(321,563.5)210x147r90  p49@(537,563.5)210x147r-90  p48@(747,563.5)210x147r90
  cut 44 (1pt):
    (6,31)-(6,29) (6,31)-(4,31) (6,327)-(6,329) (6,327)-(4,327) (6,342)-(6,340)
    (6,342)-(4,342) (6,638)-(6,636) (6,638)-(4,638) (426,31)-(426,29) (426,31)-(428,31)
    (426,327)-(426,329) (426,327)-(428,327) (426,342)-(426,340) (426,342)-(428,342) (426,638)-(426,636)
    (426,638)-(428,638) (432,31)-(432,29) (432,31)-(430,31) (432,327)-(432,329) (432,327)-(430,327)
    (432,342)-(432,340) (432,342)-(430,342) (432,638)-(432,636) (432,638)-(430,638) (852,31)-(852,29)
    (852,31)-(854,31) (852,327)-(852,329) (852,327)-(854,327) (852,342)-(852,340) (852,342)-(854,342)
    (852,638)-(852,636) (852,638)-(854,638) (429,31)-(429,29) (429,327)-(429,329) (429,342)-(429,340)
    (429,638)-(429,636) (216,31)-(216,29) (216,327)-(216,329) (216,342)-(216,340) (216,638)-(216,636)
    (642,31)-(642,29) (642,327)-(642,329) (642,342)-(642,340) (642,638)-(642,636)
AB4 SMALLAB / RUOT 3 name="" at(0,1405) size 858x625
  p18@(111,92.75)210x146.5r-90  p15@(321,92.75)210x146.5r90  p34@(537,92.75)210x146.5r-90  p31@(747,92.75)210x146.5r90
  p43@(111,239.25)210x146.5r-90  p46@(321,239.25)210x146.5r90  p27@(537,239.25)210x146.5r-90  p30@(747,239.25)210x146.5r90
  p22@(111,403.75)210x146.5r-90  p19@(321,403.75)210x146.5r90  p38@(537,403.75)210x146.5r-90  p35@(747,403.75)210x146.5r90
  p39@(111,550.25)210x146.5r-90  p42@(321,550.25)210x146.5r90  p23@(537,550.25)210x146.5r-90  p26@(747,550.25)210x146.5r90
  cut 44 (1pt):
    (6,18)-(6,16) (6,18)-(4,18) (6,314)-(6,316) (6,314)-(4,314) (6,329)-(6,327)
    (6,329)-(4,329) (6,625)-(6,623) (6,625)-(4,625) (426,18)-(426,16) (426,18)-(428,18)
    (426,314)-(426,316) (426,314)-(428,314) (426,329)-(426,327) (426,329)-(428,329) (426,625)-(426,623)
    (426,625)-(428,625) (432,18)-(432,16) (432,18)-(430,18) (432,314)-(432,316) (432,314)-(430,314)
    (432,329)-(432,327) (432,329)-(430,329) (432,625)-(432,623) (432,625)-(430,625) (852,18)-(852,16)
    (852,18)-(854,18) (852,314)-(852,316) (852,314)-(854,314) (852,329)-(852,327) (852,329)-(854,329)
    (852,625)-(852,623) (852,625)-(854,625) (429,18)-(429,16) (429,314)-(429,316) (429,329)-(429,327)
    (429,625)-(429,623) (216,18)-(216,16) (216,314)-(216,316) (216,329)-(216,327) (216,625)-(216,623)
    (642,18)-(642,16) (642,314)-(642,316) (642,329)-(642,327) (642,625)-(642,623)
AB5 SMALLAB / RUOT 3 name="" at(878,1405) size 858x625
  p40@(111,92.75)210x146.5r-90  p41@(321,92.75)210x146.5r90  p24@(537,92.75)210x146.5r-90  p25@(747,92.75)210x146.5r90
  p21@(111,239.25)210x146.5r-90  p20@(321,239.25)210x146.5r90  p37@(537,239.25)210x146.5r-90  p36@(747,239.25)210x146.5r90
  p44@(111,403.75)210x146.5r-90  p45@(321,403.75)210x146.5r90  p28@(537,403.75)210x146.5r-90  p29@(747,403.75)210x146.5r90
  p17@(111,550.25)210x146.5r-90  p16@(321,550.25)210x146.5r90  p33@(537,550.25)210x146.5r-90  p32@(747,550.25)210x146.5r90
  cut 44 (1pt):
    (6,18)-(6,16) (6,18)-(4,18) (6,314)-(6,316) (6,314)-(4,314) (6,329)-(6,327)
    (6,329)-(4,329) (6,625)-(6,623) (6,625)-(4,625) (426,18)-(426,16) (426,18)-(428,18)
    (426,314)-(426,316) (426,314)-(428,314) (426,329)-(426,327) (426,329)-(428,329) (426,625)-(426,623)
    (426,625)-(428,625) (432,18)-(432,16) (432,18)-(430,18) (432,314)-(432,316) (432,314)-(430,314)
    (432,329)-(432,327) (432,329)-(430,329) (432,625)-(432,623) (432,625)-(430,625) (852,18)-(852,16)
    (852,18)-(854,18) (852,314)-(852,316) (852,314)-(854,314) (852,329)-(852,327) (852,329)-(854,329)
    (852,625)-(852,623) (852,625)-(854,625) (429,18)-(429,16) (429,314)-(429,316) (429,329)-(429,327)
    (429,625)-(429,623) (216,18)-(216,16) (216,314)-(216,316) (216,329)-(216,327) (216,625)-(216,623)
    (642,18)-(642,16) (642,314)-(642,316) (642,329)-(642,327) (642,625)-(642,623)
"#;

#[test]
fn dong_ghim_giua_v3_a5_co_bia_60_trang() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(60, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 14.8, "height": 21, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL Offset; đã vẽ 172 nét PON cắt.\n"));
    assert_eq!(
        v["summary"],
        "Đã dàn 5 artboard.\nA5: artboard/PON lấy theo kích thước trang đã nhập; ruột được bóp dần, PON giữ lưới gốc.\nPON tự vẽ nằm trên cùng. Đã bỏ qua ghi chú."
    );
    check_sizes(
        &v,
        r#"{"total":5,"groups":[{"key":"BIA","jobs":[{"name":"BIA","ab":[1],"w":1213.23,"h":887.24}]},{"key":"SMALL8","jobs":[{"name":"RUOT 1","ab":[2],"w":1836.85,"h":1184.88}]},{"key":"SMALL16","jobs":[{"name":"RUOT 2","ab":[3],"w":2432.13,"h":1808.5}]},{"key":"SMALLAB","jobs":[{"name":"RUOT 3","ab":[4,5],"w":2432.13,"h":1771.65}]}]}"#,
    );
    check_sheets(&s, anchor, V3, SIG_CUT_LAYER, SIG_PAPER_LAYER);
}

#[test]
fn dong_ghim_giua_v4_a5_nhap_nam_ngang() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(4, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 21, "height": 14.8, "cover": false, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL Offset; đã vẽ 18 nét PON cắt.\n"));
    check_sizes(&v, r#"{"total":1,"groups":[{"key":"SMALL4","jobs":[{"name":"RUOT 1","ab":[1],"w":1213.23,"h":887.24}]}]}"#);
    let v4 = r#"
AB1 SMALL4 / RUOT 1 name="" at(0,0) size 428x313
  p1@(106.5,91.25)210x147.5r180  p2@(321.5,91.25)210x147.5r0  p4@(106.5,238.75)210x147.5r180  p3@(321.5,238.75)210x147.5r0
  cut 18 (1pt):
    (1.5,17)-(1.5,15) (1.5,17)-(3.5,17) (1.5,313)-(1.5,311) (1.5,313)-(3.5,313) (211.5,17)-(211.5,15)
    (211.5,17)-(213.5,17) (211.5,313)-(211.5,311) (211.5,313)-(213.5,313) (216.5,17)-(216.5,15) (216.5,17)-(214.5,17)
    (216.5,313)-(216.5,311) (216.5,313)-(214.5,313) (426.5,17)-(426.5,15) (426.5,17)-(424.5,17) (426.5,313)-(426.5,311)
    (426.5,313)-(424.5,313) (214,17)-(214,15) (214,313)-(214,311)
"#;
    check_sheets(&s, anchor, v4, SIG_CUT_LAYER, SIG_PAPER_LAYER);
}

#[test]
fn dong_ghim_giua_v5_a5_lon_nhat() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(12, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 15, "height": 21.15, "cover": true, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL Offset; đã vẽ 40 nét PON cắt.\n"));
    let v5 = r#"
AB1 BIA / BIA name="" at(0,0) size 428x313
  p1@(105.75,88)211.5x150r90  p2@(322.25,88)211.5x150r-90  p12@(105.75,238)211.5x150r90  p11@(322.25,238)211.5x150r-90
  cut 18 (1pt):
    (0,13)-(0,11) (0,13)-(2,13) (0,313)-(0,311) (0,313)-(2,313) (211.5,13)-(211.5,11)
    (211.5,13)-(213.5,13) (211.5,313)-(211.5,311) (211.5,313)-(213.5,313) (216.5,13)-(216.5,11) (216.5,13)-(214.5,13)
    (216.5,313)-(216.5,311) (216.5,313)-(214.5,313) (428,13)-(428,11) (428,13)-(426,13) (428,313)-(428,311)
    (428,313)-(426,313) (214,13)-(214,11) (214,313)-(214,311)
AB2 SMALL8 / RUOT 1 name="" at(0,325) size 648x418
  p7@(95.25,105.75)149.5x211.5r180  p6@(244.75,105.75)149.5x211.5r180  p5@(403.25,105.75)149.5x211.5r180  p8@(552.75,105.75)149.5x211.5r180
  p10@(95.25,312.25)149.5x211.5r0  p3@(244.75,312.25)149.5x211.5r0  p4@(403.25,312.25)149.5x211.5r0  p9@(552.75,312.25)149.5x211.5r0
  cut 22 (1pt):
    (20,0)-(20,2) (20,0)-(18,0) (20,209)-(18,209) (20,418)-(20,416) (20,418)-(18,418)
    (320,0)-(320,2) (320,0)-(322,0) (320,209)-(322,209) (320,418)-(320,416) (320,418)-(322,418)
    (328,0)-(328,2) (328,0)-(326,0) (328,209)-(326,209) (328,418)-(328,416) (328,418)-(326,418)
    (628,0)-(628,2) (628,0)-(630,0) (628,209)-(630,209) (628,418)-(628,416) (628,418)-(630,418)
    (324,0)-(324,2) (324,418)-(324,416)
"#;
    check_sheets(&s, anchor, v5, SIG_CUT_LAYER, SIG_PAPER_LAYER);
    // Ở đường nối SMALL8 (trang cao hơn 209 mm), hàng dưới nằm trên hàng trên.
    let d = &s.doc().unwrap().doc;
    let art = d.layers.iter().find(|l| l.children().is_some_and(|c| c.iter().any(|n| n.name.as_deref() == Some("p7")))).unwrap();
    let order: Vec<String> = art.children().unwrap().iter().filter_map(|n| n.name.clone()).collect();
    let at = |n: &str| order.iter().position(|x| x == n).unwrap();
    assert!(at("p10") > at("p7") && at("p3") > at("p6"), "{order:?}");
}

const V6: &str = r#"
AB1 AB / RUOT 1 name="Ruột1-A" at(0,0) size 858x625
  p5@(114,172.2)210x297r180  p12@(324,172.2)210x297r180  p9@(534,172.2)210x297r180  p8@(744,172.2)210x297r180
  p4@(114,469.2)210x297r0  p13@(324,469.2)210x297r0  p16@(534,469.2)210x297r0  p1@(744,469.2)210x297r0
  cut 12 (1pt):
    (9,21.7)-(9,23.7) (9,617.7)-(9,619.7) (429,21.7)-(429,23.7) (429,617.7)-(429,619.7) (849,21.7)-(849,23.7)
    (849,617.7)-(849,619.7) (9,23.7)-(7,23.7) (849,23.7)-(851,23.7) (9,320.7)-(7,320.7) (849,320.7)-(851,320.7)
    (9,617.7)-(7,617.7) (849,617.7)-(851,617.7)
AB2 AB / RUOT 1 name="Ruột1-B" at(878,0) size 858x625
  p7@(114,172.2)210x297r180  p10@(324,172.2)210x297r180  p11@(534,172.2)210x297r180  p6@(744,172.2)210x297r180
  p2@(114,469.2)210x297r0  p15@(324,469.2)210x297r0  p14@(534,469.2)210x297r0  p3@(744,469.2)210x297r0
  cut 12 (1pt):
    (9,21.7)-(9,23.7) (9,617.7)-(9,619.7) (429,21.7)-(429,23.7) (429,617.7)-(429,619.7) (849,21.7)-(849,23.7)
    (849,617.7)-(849,619.7) (9,23.7)-(7,23.7) (849,23.7)-(851,23.7) (9,320.7)-(7,320.7) (849,320.7)-(851,320.7)
    (9,617.7)-(7,617.7) (849,617.7)-(851,617.7)
AB3 TT8 / RUOT 2 name="Ruột2 TT8" at(0,637) size 858x638
  p25@(114,172.2)210x297r180  p20@(324,172.2)210x297r180  p19@(534,172.2)210x297r180  p26@(744,172.2)210x297r180
  p28@(114,469.2)210x297r0  p17@(324,469.2)210x297r0  p18@(534,469.2)210x297r0  p27@(744,469.2)210x297r0
  cut 12 (1pt):
    (9,21.7)-(9,23.7) (9,617.7)-(9,619.7) (429,21.7)-(429,23.7) (429,617.7)-(429,619.7) (849,21.7)-(849,23.7)
    (849,617.7)-(849,619.7) (9,23.7)-(7,23.7) (849,23.7)-(851,23.7) (9,320.7)-(7,320.7) (849,320.7)-(851,320.7)
    (9,617.7)-(7,617.7) (849,617.7)-(851,617.7)
AB4 TT4 / RUOT 3 name="Ruột3 TT4" at(0,1287) size 648x418
  p21@(173.207,104.5)295.586x209r90  p22@(474.793,104.5)295.586x209r-90  p24@(173.207,313.5)295.586x209r90  p23@(474.793,313.5)295.586x209r-90
  cut 10 (1pt):
    (25.214,1.99)-(25.214,0.176) (25.214,417.824)-(25.214,416.01) (321.2,1.99)-(321.2,0.176) (321.2,417.824)-(321.2,416.01) (324,1.99)-(324,0.176)
    (324,417.824)-(324,416.01) (326.8,1.99)-(326.8,0.176) (326.8,417.824)-(326.8,416.01) (622.786,1.99)-(622.786,0.176) (622.786,417.824)-(622.786,416.01)
"#;

#[test]
fn keo_gay_v6_a4_28_trang() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(28, anchor);
    let v = run(&mut s, "dancard.perfectBind", json!({"width": 21, "height": 29.7, "notes": false}));
    assert!(
        v["message"]
            .as_str()
            .unwrap()
            .starts_with("OK: Đã dàn CTL keo gáy trên 1 cột canvas; đã vẽ 46 nét PON cắt. A4: 16 trang/tờ AB, AB trước, tự trở sau.\n"),
        "{v}"
    );
    check_sizes(
        &v,
        r#"{"total":4,"groups":[{"key":"AB","jobs":[{"name":"RUOT 1","ab":[1,2],"w":2432.13,"h":1771.65}]},{"key":"TT8","jobs":[{"name":"RUOT 2","ab":[3],"w":2432.13,"h":1808.5}]},{"key":"TT4","jobs":[{"name":"RUOT 3","ab":[4],"w":1836.85,"h":1184.88}]}]}"#,
    );
    let d = &s.doc().unwrap().doc;
    let names: Vec<&str> = d.artboards.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Ruột1-A", "Ruột1-B", "Ruột2 TT8", "Ruột3 TT4"]);
    check_sheets(&s, anchor, V6, KEO_CUT_LAYER, KEO_PAPER_LAYER);
}

const V7: &str = r#"
AB1 AB / RUOT 1 name="Ruột1-A" at(0,0) size 858x625
  p8@(114,97.7)210x148r-90  p1@(324,97.7)210x148r90  p24@(534,97.7)210x148r-90  p17@(744,97.7)210x148r90
  p9@(114,245.7)210x148r-90  p16@(324,245.7)210x148r90  p25@(534,245.7)210x148r-90  p32@(744,245.7)210x148r90
  p12@(114,393.7)210x148r-90  p13@(324,393.7)210x148r90  p28@(534,393.7)210x148r-90  p29@(744,393.7)210x148r90
  p5@(114,541.7)210x148r-90  p4@(324,541.7)210x148r90  p21@(534,541.7)210x148r-90  p20@(744,541.7)210x148r90
  cut 16 (1pt):
    (9,21.7)-(9,23.7) (9,615.7)-(9,617.7) (219,21.7)-(219,23.7) (219,615.7)-(219,617.7) (429,21.7)-(429,23.7)
    (429,615.7)-(429,617.7) (639,21.7)-(639,23.7) (639,615.7)-(639,617.7) (849,21.7)-(849,23.7) (849,615.7)-(849,617.7)
    (7,23.7)-(9,23.7) (849,23.7)-(851,23.7) (7,319.7)-(9,319.7) (849,319.7)-(851,319.7) (7,615.7)-(9,615.7)
    (849,615.7)-(851,615.7)
AB2 AB / RUOT 1 name="Ruột1-B" at(878,0) size 858x625
  p18@(114,97.7)210x148r-90  p23@(324,97.7)210x148r90  p2@(534,97.7)210x148r-90  p7@(744,97.7)210x148r90
  p31@(114,245.7)210x148r-90  p26@(324,245.7)210x148r90  p15@(534,245.7)210x148r-90  p10@(744,245.7)210x148r90
  p30@(114,393.7)210x148r-90  p27@(324,393.7)210x148r90  p14@(534,393.7)210x148r-90  p11@(744,393.7)210x148r90
  p19@(114,541.7)210x148r-90  p22@(324,541.7)210x148r90  p3@(534,541.7)210x148r-90  p6@(744,541.7)210x148r90
  cut 16 (1pt):
    (9,21.7)-(9,23.7) (9,615.7)-(9,617.7) (219,21.7)-(219,23.7) (219,615.7)-(219,617.7) (429,21.7)-(429,23.7)
    (429,615.7)-(429,617.7) (639,21.7)-(639,23.7) (639,615.7)-(639,617.7) (849,21.7)-(849,23.7) (849,615.7)-(849,617.7)
    (7,23.7)-(9,23.7) (849,23.7)-(851,23.7) (7,319.7)-(9,319.7) (849,319.7)-(851,319.7) (7,615.7)-(9,615.7)
    (849,615.7)-(851,615.7)
AB3 TT16 / RUOT 2 name="Ruột2 TT16" at(0,637) size 858x638
  p40@(114,97.7)210x148r-90  p33@(324,97.7)210x148r90  p34@(534,97.7)210x148r-90  p39@(744,97.7)210x148r90
  p41@(114,245.7)210x148r-90  p48@(324,245.7)210x148r90  p47@(534,245.7)210x148r-90  p42@(744,245.7)210x148r90
  p44@(114,393.7)210x148r-90  p45@(324,393.7)210x148r90  p46@(534,393.7)210x148r-90  p43@(744,393.7)210x148r90
  p37@(114,541.7)210x148r-90  p36@(324,541.7)210x148r90  p35@(534,541.7)210x148r-90  p38@(744,541.7)210x148r90
  cut 16 (1pt):
    (9,21.7)-(9,23.7) (9,615.7)-(9,617.7) (219,21.7)-(219,23.7) (219,615.7)-(219,617.7) (429,21.7)-(429,23.7)
    (429,615.7)-(429,617.7) (639,21.7)-(639,23.7) (639,615.7)-(639,617.7) (849,21.7)-(849,23.7) (849,615.7)-(849,617.7)
    (7,23.7)-(9,23.7) (849,23.7)-(851,23.7) (7,319.7)-(9,319.7) (849,319.7)-(851,319.7) (7,615.7)-(9,615.7)
    (849,615.7)-(851,615.7)
AB4 TT8 / RUOT 3 name="Ruột3 TT8" at(0,1287) size 858x638
  p57@(207,214)148x210r180  p52@(355,214)148x210r180  p51@(503,214)148x210r180  p58@(651,214)148x210r180
  p60@(207,424)148x210r0  p49@(355,424)148x210r0  p50@(503,424)148x210r0  p59@(651,424)148x210r0
  cut 12 (1pt):
    (133,107)-(133,109) (133,529)-(133,531) (429,107)-(429,109) (429,529)-(429,531) (725,107)-(725,109)
    (725,529)-(725,531) (133,109)-(131,109) (725,109)-(727,109) (133,319)-(131,319) (725,319)-(727,319)
    (133,529)-(131,529) (725,529)-(727,529)
AB5 TT4 / RUOT 4 name="Ruột4 TT4" at(0,1937) size 428x313
  p53@(107,82.852)209x147.295r90  p54@(321,82.852)209x147.295r-90  p56@(107,230.148)209x147.295r90  p55@(321,230.148)209x147.295r-90
  cut 10 (1pt):
    (2.3,7.214)-(2.3,9.205) (2.3,303.795)-(2.3,305.786) (211.7,7.214)-(211.7,9.205) (211.7,303.795)-(211.7,305.786) (214,7.214)-(214,9.205)
    (214,303.795)-(214,305.786) (216.3,7.214)-(216.3,9.205) (216.3,303.795)-(216.3,305.786) (425.7,7.214)-(425.7,9.205) (425.7,303.795)-(425.7,305.786)
"#;

#[test]
fn keo_gay_v7_a5_60_trang() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(60, anchor);
    let v = run(&mut s, "dancard.perfectBind", json!({"width": 14.8, "height": 21, "notes": false}));
    assert!(
        v["message"]
            .as_str()
            .unwrap()
            .starts_with("OK: Đã dàn CTL keo gáy trên 1 cột canvas; đã vẽ 70 nét PON cắt. A5: 32 trang/tờ AB (2 tay 16), AB trước, tự trở sau.\n")
    );
    assert_eq!(
        v["summary"],
        "Đã dàn keo gáy: 5 artboard.\nA5: 32 trang/tờ AB (2 tay 16); phần dư tự trở TT16/TT8/TT4.\nPON tự động nằm trên cùng. Đã bỏ qua ghi chú."
    );
    check_sizes(
        &v,
        r#"{"total":5,"groups":[{"key":"AB","jobs":[{"name":"RUOT 1","ab":[1,2],"w":2432.13,"h":1771.65}]},{"key":"TT16","jobs":[{"name":"RUOT 2","ab":[3],"w":2432.13,"h":1808.5}]},{"key":"TT8","jobs":[{"name":"RUOT 3","ab":[4],"w":2432.13,"h":1808.5}]},{"key":"TT4","jobs":[{"name":"RUOT 4","ab":[5],"w":1213.23,"h":887.24}]}]}"#,
    );
    check_sheets(&s, anchor, V7, KEO_CUT_LAYER, KEO_PAPER_LAYER);
}

#[test]
fn keo_gay_v8_tt8_vua_to_nho() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(8, anchor);
    let v = run(&mut s, "dancard.perfectBind", json!({"width": 14.5, "height": 20.7, "notes": false}));
    assert!(v["message"].as_str().unwrap().starts_with("OK: Đã dàn CTL keo gáy trên 1 cột canvas; đã vẽ 12 nét PON cắt."));
    check_sizes(&v, r#"{"total":1,"groups":[{"key":"TT8","jobs":[{"name":"RUOT 1","ab":[1],"w":1836.85,"h":1184.88}]}]}"#);
    let v8 = r#"
AB1 TT8 / RUOT 1 name="Ruột1 TT8" at(0,0) size 648x418
  p5@(106.5,105.5)145x207r180  p4@(251.5,105.5)145x207r180  p3@(396.5,105.5)145x207r180  p6@(541.5,105.5)145x207r180
  p8@(106.5,312.5)145x207r0  p1@(251.5,312.5)145x207r0  p2@(396.5,312.5)145x207r0  p7@(541.5,312.5)145x207r0
  cut 12 (1pt):
    (34,0.176)-(34,2) (34,416)-(34,417.824) (324,0.176)-(324,2) (324,416)-(324,417.824) (614,0.176)-(614,2)
    (614,416)-(614,417.824) (34,2)-(32,2) (614,2)-(616,2) (34,209)-(32,209) (614,209)-(616,209)
    (34,416)-(32,416) (614,416)-(616,416)
"#;
    check_sheets(&s, anchor, v8, KEO_CUT_LAYER, KEO_PAPER_LAYER);
}

#[test]
fn loi_va_dung_khong_sua_tai_lieu() {
    let anchor = Point::new(-6000.0, -6000.0);
    // E1: bìa A4 rộng 21 cm không lọt tờ 64,8 × 41,8.
    let mut s = pages_session(8, anchor);
    let before = s.doc().unwrap().doc.clone();
    let e = fail(&mut s, "dancard.saddleStitch", json!({"width": 21, "height": 29.7, "cover": true}));
    assert!(e.ends_with("Khổ trang nhập quá lớn cho tờ 648 x 418 mm: bài cần 601.6 x 420 mm, dư dọc 2 mm. Hãy nhập lại khổ trang nhỏ hơn."), "{e}");
    assert!(s.doc().unwrap().doc == before, "lỗi trước khi sửa gì");
    // E5: Keo gáy A4 tối đa 21,2 × 30 cm, kể cả nhập nằm ngang.
    for (w, h) in [(21.3, 30.0), (30.0, 21.2)] {
        let e = fail(&mut s, "dancard.perfectBind", json!({"width": w, "height": h}));
        assert!(e.ends_with("Khung A4 tối đa 21,2 × 30 cm (rộng × cao)."), "{e}");
    }
    // Kích thước không hợp lệ.
    let e = fail(&mut s, "dancard.saddleStitch", json!({"width": "abc", "height": 29}));
    assert!(e.ends_with("Nhập kích thước hợp lệ (rộng × cao, cm)."), "{e}");
    // E4: 6 trang.
    let mut s = pages_session(6, anchor);
    for id in ["dancard.saddleStitch", "dancard.perfectBind"] {
        let e = fail(&mut s, id, json!({"width": 14.8, "height": 21}));
        assert!(e.ends_with("Số trang phải là BỘI SỐ CỦA 4.\nĐang chọn: 6"), "{id}: {e}");
    }
    // E7: không đủ chiều cao canvas từ vị trí nguồn.
    let mut s = pages_session(16, Point::new(-6000.0, 6000.0));
    let e = fail(&mut s, "dancard.saddleStitch", json!({"width": 20, "height": 28, "cover": false}));
    assert!(e.ends_with("Không đủ chiều cao canvas từ vị trí object nguồn để đặt một tờ CTL Offset."), "{e}");
}

#[test]
fn dan_lai_thay_pon_cu_va_bo_qua_trang_trong_layer_pon() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(12, anchor);
    run(&mut s, "dancard.saddleStitch", json!({"width": 20.5, "height": 29, "cover": false, "notes": false}));
    // Chọn cả trang lẫn PON cũ: PON cũ bị bỏ qua, layer PON được vẽ lại (không nhân đôi).
    let d = &s.doc().unwrap().doc;
    let mut ids: Vec<u64> = (1..=12).map(|i| find_named(&d.layers, &format!("p{i}")).unwrap().id.0).collect();
    let pon = d.layers.iter().find(|l| l.name.as_deref() == Some(SIG_PAPER_LAYER)).unwrap();
    ids.push(pon.children().unwrap()[0].id.0);
    s.edit("mở khóa", |d, _| {
        for l in d.layers.iter_mut() {
            std::sync::Arc::make_mut(l).locked = false;
        }
        Ok(())
    })
    .unwrap();
    run(&mut s, "select.set", json!({"ids": ids}));
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 20.5, "height": 29, "cover": false, "notes": false}));
    assert_eq!(v["faces"], 2);
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.layers.iter().filter(|l| l.name.as_deref() == Some(SIG_PAPER_LAYER)).count(), 1);
    assert_eq!(d.artboards.len(), 2);
}

fn texts(d: &Document) -> Vec<(String, Rect, bool)> {
    fn walk(nodes: &[std::sync::Arc<Node>], out: &mut Vec<(String, Rect, bool)>) {
        for n in nodes {
            if let NodeKind::Text(t) = &n.kind {
                let red = t
                    .runs
                    .first()
                    .is_some_and(|r| r.style.fill == vectorcraft_color::Paint::solid(vectorcraft_color::Color::cmyk(0.0, 1.0, 1.0, 0.0)));
                out.push((t.plain_text(), n.visual_bounds().unwrap(), red));
            }
            if let Some(c) = n.children() {
                walk(c, out);
            }
        }
    }
    let mut out = vec![];
    walk(&d.layers, &mut out);
    out
}

#[test]
fn ghi_chu_ruot_mat_a_a4() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(28, anchor);
    let v = run(&mut s, "dancard.saddleStitch", json!({"width": 20.5, "height": 29, "coverNote": "BÌA KMC", "insideNote": "IN 2 MẶT"}));
    assert_eq!(v["notes"], 3, "bìa + TT8 + mặt A của tờ AB");
    assert!(v["summary"].as_str().unwrap().ends_with("Ghi chú đã xử lý."));
    let d = &s.doc().unwrap().doc;
    let notes = texts(d);
    let find = |t: &str| notes.iter().find(|n| n.0 == t).unwrap_or_else(|| panic!("thiếu ghi chú {t}: {notes:?}"));
    // Bìa: chữ đen dọc, mép trái vào 9 mm, mép dưới lên 15 mm.
    let (_, b, red) = find("BÌA KMC");
    let ab = d.artboards[0].rect;
    assert!(!red && b.height() > b.width());
    close((b.x0 - ab.x0) / MM, 9.0, "bìa trái");
    close((ab.y1 - b.y1) / MM, 15.0, "bìa dưới");
    // Ruột: chữ đỏ ngang, mép trái vào 20 mm, mép trên xuống 11 mm.
    let (_, b, red) = find("RUỘT 1 IN 2 MẶT");
    let ab = d.artboards[1].rect;
    assert!(*red && b.width() > b.height());
    close((b.x0 - ab.x0) / MM, 20.0, "ruột trái");
    close((b.y0 - ab.y0) / MM, 11.0, "ruột trên");
    assert!(find("RUỘT 2 IN 2 MẶT").1.intersect(d.artboards[2].rect).area() > 0.0, "chỉ mặt A");
    // Không chữ thêm: RUỘT n màu đen.
    let mut s = pages_session(12, anchor);
    run(&mut s, "dancard.saddleStitch", json!({"width": 20.5, "height": 29, "cover": false, "typeNotes": {"tt8": "TT8 RIÊNG"}}));
    let notes = texts(&s.doc().unwrap().doc);
    assert!(notes.iter().any(|n| n.0 == "RUỘT 1" && !n.2) && notes.iter().any(|n| n.0 == "RUỘT 2 TT8 RIÊNG" && n.2), "{notes:?}");
}

#[test]
fn ghi_chu_a5_nam_trong_to_va_tranh_pon() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(60, anchor);
    let v =
        run(&mut s, "dancard.saddleStitch", json!({"width": 14.8, "height": 21, "coverNote": "BÌA", "insideNote": "GHI CHÚ RẤT DÀI ĐỂ XUỐNG DÒNG"}));
    assert_eq!(v["notes"], 4, "bìa, TT8, TT16, mặt A của tờ AB");
    let d = &s.doc().unwrap().doc;
    let (cut, _) = layer_lines(d, SIG_CUT_LAYER);
    for (text, b, _) in texts(d) {
        let ab = d.artboards.iter().find(|a| a.rect.intersect(b).area() > 0.0).unwrap_or_else(|| panic!("{text}: ngoài mọi tờ"));
        let r = ab.rect;
        assert!(b.x0 >= r.x0 && b.x1 <= r.x1 && b.y0 >= r.y0 && b.y1 <= r.y1, "{text}: {b:?} ngoài {r:?}");
        // Mỗi nét PON cắt (nở thêm nửa nét) không chạm khối chữ.
        for l in &cut {
            let o = Rect::from_points(l.a, l.b).inflate(0.5, 0.5);
            let touches = o.x0 < b.x1 && o.x1 > b.x0 && o.y0 < b.y1 && o.y1 > b.y0;
            assert!(!touches, "{text} chạm PON {o:?}");
        }
    }
}

#[test]
fn keo_gay_ghi_chu_mat_a() {
    let anchor = Point::new(-6000.0, -6000.0);
    let mut s = pages_session(60, anchor);
    let v = run(&mut s, "dancard.perfectBind", json!({"width": 14.8, "height": 21, "insideNote": "KEO"}));
    assert_eq!(v["notes"], 4, "AB mặt A, TT16, TT8, TT4");
    let notes = texts(&s.doc().unwrap().doc);
    for n in ["RUỘT 1 KEO", "RUỘT 2 KEO", "RUỘT 3 KEO", "RUỘT 4 KEO"] {
        assert!(notes.iter().any(|t| t.0 == n && t.2), "{n}: {notes:?}");
    }
}

// ---------------------------------------------------------------- lưu AI + JPG + ZIP

#[cfg(not(target_arch = "wasm32"))]
mod save {
    use super::*;

    /// Tài liệu CMYK 2 artboard 100 × 100 pt (cách 20 pt), mỗi artboard một hình chữ nhật, thêm một
    /// object ẩn và một object nằm ngoài.
    fn two_boards() -> Session {
        let mut s = Session::new();
        run(&mut s, "file.new", json!({"width": 100, "height": 100, "colorMode": "cmyk"}));
        s.edit("artboard", |d, _| {
            let id = d.next_artboard_id();
            d.artboards.push(Artboard {
                id,
                name: "Ruột1-B".into(),
                rect: Rect::new(120.0, 0.0, 220.0, 100.0),
                show_center_mark: false,
                show_cross_hairs: false,
            });
            if let Some(a) = d.artboards.first_mut() {
                a.name = "Ruột1-A".into();
            }
            Ok(())
        })
        .unwrap();
        for (x, name) in [(10.0, "a"), (130.0, "b"), (500.0, "ngoai")] {
            let id = run(&mut s, "shape.rectangle", json!({"x": x, "y": 10, "width": 50, "height": 50}))["id"].as_u64().unwrap();
            s.edit("tên", |d, _| {
                d.node_mut(NodeId(id)).unwrap().name = Some(name.into());
                Ok(())
            })
            .unwrap();
        }
        let hidden = run(&mut s, "shape.rectangle", json!({"x": 20, "y": 20, "width": 10, "height": 10}))["id"].as_u64().unwrap();
        s.edit("ẩn", |d, _| {
            d.node_mut(NodeId(hidden)).unwrap().visible = false;
            Ok(())
        })
        .unwrap();
        s
    }

    fn folder(tag: &str) -> String {
        let dir = vectorcraft_testkit::temp_dir(&format!("ctl-{tag}"));
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        dir.to_string_lossy().into_owned()
    }

    #[test]
    fn to_tam_giu_vi_tri_so_voi_artboard() {
        let s = two_boards();
        let d = &s.doc().unwrap().doc;
        let pages = [package::Page { index: 1, rect: d.artboards[1].rect, name: "Ruột1-B".into() }];
        let tmp = package::sheet_document(d, "RUOT 1", &pages).unwrap();
        assert_eq!(tmp.artboards.len(), 1);
        assert_eq!(tmp.artboards[0].name, "Ruột1-B");
        let group = tmp.layers.iter().flat_map(|l| l.children().unwrap().iter()).find(|n| n.name.as_deref() == Some("CTL Ruột1-B")).unwrap();
        let names: Vec<_> = group.children().unwrap().iter().filter_map(|n| n.name.clone()).collect();
        assert_eq!(names, ["b"], "chỉ object chạm tờ, không object ẩn");
        // Hình b cách góc trái artboard 10 pt, như ở tài liệu gốc.
        let b = group.children().unwrap()[0].geometric_bounds().unwrap();
        assert_eq!((b.x0 - tmp.artboards[0].rect.x0, b.y0 - tmp.artboards[0].rect.y0), (10.0, 10.0));
        // Tờ không có gì thì báo lỗi.
        let empty = [package::Page { index: 0, rect: Rect::new(900.0, 900.0, 1000.0, 1000.0), name: String::new() }];
        assert_eq!(package::sheet_document(d, "x", &empty).unwrap_err(), "Artboard 1 không có object để lưu.");
    }

    #[test]
    fn zip_hai_muc_nen_deflate() {
        let ai = [7u8; 5000];
        let jpg = b"\xff\xd8 jpeg".to_vec();
        let zip = package::zip_deflate(&[("A B.ai", ai.as_slice()), ("A B.jpg", jpg.as_slice())], (0, 33)).unwrap();
        assert!(zip.len() < ai.len(), "đã nén");
        assert_eq!(package::zip_listing(&zip).unwrap(), vec![("A B.ai".to_string(), 5000), ("A B.jpg".to_string(), jpg.len() as u64)]);
        // Giải nén mục đầu: đúng bytes.
        let (n, e) = (u16::from_le_bytes([zip[26], zip[27]]) as usize, u16::from_le_bytes([zip[28], zip[29]]) as usize);
        let size = u32::from_le_bytes([zip[18], zip[19], zip[20], zip[21]]) as usize;
        let mut out = vec![];
        std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(&zip[30 + n + e..30 + n + e + size]), &mut out).unwrap();
        assert_eq!(out, ai);
        assert!(package::zip_listing(&zip[..zip.len() - 1]).is_none());
    }

    #[test]
    fn luu_ai_jpg_zip_mot_to() {
        let mut s = two_boards();
        let dir = folder("save");
        let jobs = json!([{"name": "RUOT 1", "ab": [1, 2], "w": 100, "h": 100}]);
        let v = run(&mut s, "dancard.saveCtlPackage", json!({"total": 2, "jobs": jobs, "suffix": " KMC ", "folder": dir}));
        let msg = v["message"].as_str().unwrap();
        assert!(msg.starts_with(&format!("OK: Đã lưu 1 AI + JPG vào {dir}: RUOT 1 KMC.ai.")) && msg.ends_with("Đã nén 1 ZIP."), "{msg}");
        let path = |ext: &str| std::path::Path::new(&dir).join(format!("RUOT 1 KMC.{ext}"));
        let ai = std::fs::read(path("ai")).unwrap();
        let jpg = std::fs::read(path("jpg")).unwrap();
        let zip = std::fs::read(path("zip")).unwrap();
        assert!(ai.starts_with(b"%PDF") && jpg.starts_with(&[0xff, 0xd8]));
        assert_eq!(
            package::zip_listing(&zip).unwrap(),
            vec![("RUOT 1 KMC.ai".to_string(), ai.len() as u64), ("RUOT 1 KMC.jpg".to_string(), jpg.len() as u64)]
        );
        // File AI mở lại được: hai artboard A, B cạnh nhau, giữ tên.
        let back = crate::cmd::fileio::load("RUOT 1 KMC.ai", &ai).unwrap().doc;
        let names: Vec<_> = back.artboards.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Ruột1-A", "Ruột1-B"]);
        // Tài liệu nguồn không đổi; không bao giờ ghi đè.
        assert_eq!(s.doc().unwrap().doc.artboards.len(), 2);
        let e = fail(&mut s, "dancard.saveCtlPackage", json!({"total": 2, "jobs": jobs, "suffix": "KMC", "folder": dir}));
        assert!(e.ends_with("File đã tồn tại: RUOT 1 KMC.ai."), "{e}");
    }

    #[test]
    fn luu_kiem_tra_truoc_khi_ghi() {
        let mut s = two_boards();
        let dir = folder("check");
        let good = json!([{"name": "BIA", "ab": [1], "w": 100, "h": 100}]);
        let cases = [
            (
                json!({"total": 3, "jobs": good, "folder": dir}),
                "Tài liệu đang mở có 2 artboard, lúc dàn là 3. Tài liệu đã khác lúc dàn — hãy dàn lại rồi lưu AI + JPG.",
            ),
            (
                json!({"total": 2, "jobs": [{"name": "BIA", "ab": [1], "w": 90, "h": 100}], "folder": dir}),
                "Artboard 1 không còn đúng khổ của BIA. Tài liệu đã khác lúc dàn — hãy dàn lại rồi lưu AI + JPG.",
            ),
            (
                json!({"total": 2, "jobs": [{"name": "BIA", "ab": [3], "w": 100, "h": 100}], "folder": dir}),
                "Không có artboard số 3. Tài liệu đã khác lúc dàn — hãy dàn lại rồi lưu AI + JPG.",
            ),
            (
                json!({"total": 2, "jobs": [{"name": "BIA", "ab": [], "w": 100, "h": 100}], "folder": dir}),
                "Thiếu thông tin tờ cần lưu — hãy dàn lại rồi lưu AI + JPG.",
            ),
            (json!({"total": 2, "jobs": [], "folder": dir}), "Không có tờ nào để lưu."),
            (
                json!({"total": 2, "jobs": good, "suffix": "a/b", "folder": dir}),
                r#"Nội dung tên file không được chứa \ / : * ? " < > | hoặc xuống dòng."#,
            ),
            (json!({"total": 2, "jobs": good, "suffix": "abc.", "folder": dir}), "Tên file không được kết thúc bằng dấu chấm."),
            (json!({"total": 2, "jobs": good}), "Chưa chọn thư mục lưu AI + JPG - đã huỷ."),
            (
                json!({"total": 2, "jobs": [{"name": "BIA", "ab": [1], "w": 100, "h": 100}, {"name": "bia", "ab": [2], "w": 100, "h": 100}], "folder": dir}),
                "Hai tờ trùng tên AI + JPG: bia.",
            ),
        ];
        for (p, want) in cases {
            let e = fail(&mut s, "dancard.saveCtlPackage", p.clone());
            assert!(e.ends_with(want), "{p}: {e}");
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "chưa ghi file nào");
    }
}
