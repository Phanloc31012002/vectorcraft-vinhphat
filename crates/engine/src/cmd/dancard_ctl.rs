//! DanCard — tab CTL Offset: Đóng ghim giữa (`dcRunSignature8`), Keo gáy (`dcRunKeoGay`) và
//! Lưu AI + JPG + ZIP theo khổ vừa dàn (`dcLuuCtlOffsetAIPackage` cùng bộ nén ZIP của panel).
//!
//! - Đóng ghim giữa: chọn N trang theo thứ tự đọc (N chia hết cho 4), nhập khổ một trang. Có bìa
//!   thì 4 trang ngoài cùng thành tờ tự trở 4 trang (bìa không bóp). Phần ruột: A4 dư 4/8/12 trang
//!   khi chia 16 thì dàn tự trở 4 và/hoặc 8 trước, còn lại các tờ AB 16 trang (mặt A, mặt B); họ A5
//!   (≤ 15 × 21,15 cm) dàn tự trở 4/8/16 rồi tờ AB 32 trang. Tờ giấy cố định (85,8 × 63,8,
//!   85,8 × 62,5, 64,8 × 41,8, 42,8 × 31,3 cm), artboard đi qua tim PON giấy. Lưới, artboard và PON
//!   luôn theo khổ nhập; chỉ bài ruột bị bóp 1 mm mỗi tờ từ ngoài vào trong.
//! - Keo gáy: tuần tự, không bóp, không khe; A4 16 trang / tờ AB, A5 hai tay 16 (32 trang); phần dư
//!   tự trở TT16 / TT8 / TT4 (tờ nhỏ thu trang về cạnh 20,9 cm). Bìa dàn tay.
//! - Cả hai: trang vector được làm phẳng thành ảnh 500 ppi (trang chỉ có ảnh thì giữ), co đúng khổ,
//!   đưa về layer đang vẽ; PON giấy và PON cắt nằm trên hai layer riêng trên cùng, khóa lại; các
//!   artboard cũ được thay bằng các tờ mới; ghi chú "RUỘT N" ở mặt A (chữ thêm theo tham số thay cho
//!   hộp thoại ghi chú, không có icon). Cả lần dàn là một bước hoàn tác. Kết quả kèm `sizes`: các
//!   khổ vừa dàn để lưu AI + JPG + ZIP từng tờ (`dancard.saveCtlPackage`).
//!
//! Toạ độ của DanCard (y hướng lên) được viết lại theo trang của VectorCraft (y hướng xuống).

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Artboard, CharStyle, Document, Node, NodeId, NodeKind, Scaling, TextObject};
use vectorcraft_geom::{Affine, Point, Rect, Vec2, shapes};

use super::catalogue::image_only;
use super::layout::{active_layer, raster_in, union_of, vis};
use super::{number_param, roots_in_paint_order};
use crate::cmd::*;
use crate::{EngineError, Result, Session};

const MM: f64 = 72.0 / 25.4;

/// Tờ giấy (pt): 85,8 × 63,8 (tên xưởng "65×86"), tờ AB 85,8 × 62,5, 64,8 × 41,8 ("65×43"),
/// 42,8 × 31,3 ("43×32,5").
const SHEET_MAIN: (f64, f64) = (858.0 * MM, 638.0 * MM);
const SHEET_AB: (f64, f64) = (858.0 * MM, 625.0 * MM);
const SHEET_SMALL: (f64, f64) = (648.0 * MM, 418.0 * MM);
const SHEET_COVER_A5: (f64, f64) = (428.0 * MM, 313.0 * MM);

/// Khe giữa hàng và giữa cột (A|B, các cột canvas).
const GAP_ROW: f64 = 12.0 * MM;
const GAP_COL: f64 = 20.0 * MM;
/// Nửa canvas (pt): canvas của VectorCraft không giới hạn, giữ ±7200 pt như canvas của
/// Illustrator để các cột dàn giống DanCard (và tài liệu vẫn mở được ở đó).
const CANVAS_HALF: f64 = 7200.0;
/// Lề nhíp: Đóng ghim giữa TT8/AB A4, Keo gáy.
const SIG_TOP_GAP: f64 = 23.0 * MM;
const KEO_TOP_GAP: f64 = 23.7 * MM;
/// Bóp ruột mỗi tờ.
const BOP_PER_SHEET: f64 = MM;
/// Nét PON cắt: dài 2 mm, dày 1 pt.
const CUT_MARK: f64 = 2.0 * MM;
const CUT_STROKE: f64 = 1.0;
/// Cỡ chữ ghi chú.
const NOTE_SIZE: f64 = 13.0;
/// Ghi chú dài hơn chừng này ký tự bị cắt bớt.
const NOTE_MAX_CHARS: usize = 1000;

pub(super) const SIG_PAPER_LAYER: &str = "Pon CTL Offset tu dong";
pub(super) const SIG_CUT_LAYER: &str = "Pon cat CTL Offset tu dong";
pub(super) const KEO_PAPER_LAYER: &str = "Pon CTL Keo Gay tu dong";
pub(super) const KEO_CUT_LAYER: &str = "Pon cat CTL Keo Gay tu dong";

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "dancard.saddleStitch",
            "DanCard: Dàn Đóng ghim giữa (CTL Offset)",
            [],
            None,
            "{width, height: cm (khổ một trang, rộng × cao), cover?: bool (mặc định true: có bìa, 4 trang ngoài cùng 1, 2, N-1, N thành tờ tự trở 4 trang không bóp), notes?: bool (mặc định true: thêm ghi chú RUỘT n ở mặt A mỗi tờ ruột; false: bỏ qua ghi chú), coverNote?: ghi chú BÌA (không tự thêm chữ), insideNote?: chữ thêm sau RUỘT n (chữ đỏ), typeNotes?: {tt4, tt8, ab, small4, small8, small16, smallAB} (chữ thêm riêng theo loại tờ, thắng insideNote)} dàn các trang đang chọn (thứ tự đọc: hàng trên trước, trái → phải; số trang chia hết cho 4) thành CTL Offset bấm ghim giữa: tự trở 4/8 (A5 thêm 16) trước, rồi các tờ AB; tờ giấy cố định, trang vector làm phẳng 500 ppi, bóp ruột 1 mm mỗi tờ (PON giữ lưới gốc), PON giấy và PON cắt tự vẽ trên hai layer khóa trên cùng, artboard cũ thay bằng các tờ mới; một bước hoàn tác → {message, summary, faces, cutPon, notes, artboards: [tên], sizes: {total, groups: [{key, label, jobs: [{name, ab: [số artboard từ 1], w, h: pt}]}]}}",
            has_selection,
            saddle_stitch
        ),
        cmd!(
            "dancard.perfectBind",
            "DanCard: Dàn Keo gáy (CTL Offset)",
            [],
            None,
            "{width, height: cm (khổ một trang; A4 tối đa 21,2 × 30 cm), notes?: bool (mặc định true: thêm ghi chú RUỘT n ở mặt A; false: bỏ qua), insideNote?: chữ thêm sau RUỘT n (chữ đỏ), typeNotes?: {tt4, tt8, tt16, ab} (chữ thêm riêng theo loại tờ, thắng insideNote)} dàn các trang ruột đang chọn (thứ tự đọc, số trang chia hết cho 4) thành CTL keo gáy: tuần tự, không bóp, không khe; A4 16 trang / tờ AB 85,8 × 62,5 cm, A5 32 trang (hai tay 16), phần dư tự trở TT16/TT8/TT4 (tờ nhỏ thu trang về cạnh 20,9 cm); PON tự vẽ trên hai layer khóa trên cùng, artboard Ruột1-A, Ruột1-B…, một bước hoàn tác; bìa dàn tay → {message, summary, faces, cutPon, notes, columns, artboards: [tên], sizes: {total, groups: [{key, label, jobs: [{name, ab, w, h}]}]}}",
            has_selection,
            perfect_bind
        ),
        cmd!(
            "dancard.saveCtlPackage",
            "DanCard: Lưu AI + JPG + ZIP theo khổ (CTL Offset)",
            [],
            None,
            "{total: số artboard lúc dàn (sizes.total), jobs: [{name, ab: [số artboard từ 1], w, h: pt}] (jobs của một khổ trong sizes.groups: mỗi job một bộ file, A/B chung một bộ), suffix?: nội dung thêm sau tên file (RUOT n / BIA), folder: thư mục lưu (đường dẫn tuyệt đối)} lưu mỗi tờ thành <tên>.ai (các artboard của tờ), <tên>.jpg (CMYK, 300 ppi, chất lượng cao nhất, cả tờ A + B) và <tên>.zip (hai file đó, nén deflate); không ghi đè file có sẵn, không sửa tài liệu → {message, folder, files: [{baseName, ai, jpg, zip}], errors, zipErrors}",
            has_doc,
            save_ctl_package
        ),
    ]
}

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

// ---------------------------------------------------------------- mặt dàn (face)

/// Loại tờ, như `face.type` của DanCard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Tt4,
    Tt8,
    Tt16,
    Ab,
    Small4,
    Small8,
    Small16,
    SmallAb,
}

impl Kind {
    /// Khóa của DanCard (nhóm lưu file theo khổ).
    pub(super) fn key(self) -> &'static str {
        match self {
            Kind::Tt4 => "TT4",
            Kind::Tt8 => "TT8",
            Kind::Tt16 => "TT16",
            Kind::Ab => "AB",
            Kind::Small4 => "SMALL4",
            Kind::Small8 => "SMALL8",
            Kind::Small16 => "SMALL16",
            Kind::SmallAb => "SMALLAB",
        }
    }

    /// Khóa của ô ghi chú riêng (`typeNotes`).
    fn note_key(self) -> &'static str {
        match self {
            Kind::Tt4 => "tt4",
            Kind::Tt8 => "tt8",
            Kind::Tt16 => "tt16",
            Kind::Ab => "ab",
            Kind::Small4 => "small4",
            Kind::Small8 => "small8",
            Kind::Small16 => "small16",
            Kind::SmallAb => "smallAB",
        }
    }

    fn a5(self) -> bool {
        matches!(self, Kind::Small4 | Kind::Small8 | Kind::Small16 | Kind::SmallAb)
    }
}

/// Nhãn nút lưu của một khóa (`OFFSET_PDF_LABELS` của panel DanCard).
pub(super) fn size_label(key: &str) -> &str {
    match key {
        "BIA" => "Bìa",
        "TT4" | "SMALL4" => "Tự trở 4 trang",
        "TT8" | "SMALL8" => "Tự trở 8 trang",
        "TT16" | "SMALL16" => "Tự trở 16 trang",
        "AB" | "SMALLAB" => "Tờ AB",
        other => other,
    }
}

/// Nửa mặt 8 trang (Đóng ghim giữa A5): hàng trên (xoay 180°) và hàng dưới, trái → phải.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Half {
    pub top: Vec<i64>,
    pub bottom: Vec<i64>,
}

/// Khối 16 trang của Keo gáy A5 (SIG16), xoay `rotation` độ.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Block {
    pub top: Vec<i64>,
    pub bottom: Vec<i64>,
    pub rotation: f64,
}

/// Một mặt dàn = một artboard. Số trang tính từ 1.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Face {
    pub kind: Kind,
    /// Số tờ ruột (bìa 0); hai mặt của tờ AB chung một số.
    pub sheet: i64,
    /// Nhãn của DanCard (Đóng ghim giữa: nội bộ; Keo gáy: tên artboard).
    pub label: String,
    /// Mặt B của tờ AB (không ghi chú).
    pub side_b: bool,
    /// TT4 / SMALL4: một hàng 4 trang.
    pub row: Vec<i64>,
    /// TT8 / SMALL8 / AB: hàng trên (xoay 180°) và hàng dưới.
    pub top: Vec<i64>,
    pub bottom: Vec<i64>,
    /// SMALL16 / SMALLAB: nửa trái, nửa phải.
    pub panels: Vec<Half>,
    /// Keo gáy A5 AB / TT16: các khối 16 trang.
    pub blocks: Vec<Block>,
    /// Keo gáy TT8 A5: tự trở trên tờ nhỏ khi vừa.
    pub small_self_turn: bool,
}

impl Face {
    fn new(kind: Kind, sheet: i64, label: impl Into<String>) -> Self {
        Face {
            kind,
            sheet,
            label: label.into(),
            side_b: false,
            row: vec![],
            top: vec![],
            bottom: vec![],
            panels: vec![],
            blocks: vec![],
            small_self_turn: false,
        }
    }
}

/// Mọi trang của một mặt.
fn face_pages(f: &Face) -> Vec<i64> {
    let mut v: Vec<i64> = f.row.iter().chain(&f.top).chain(&f.bottom).copied().collect();
    for h in &f.panels {
        v.extend(h.top.iter().chain(&h.bottom));
    }
    for b in &f.blocks {
        v.extend(b.top.iter().chain(&b.bottom));
    }
    v
}

/// 16 trang a..a+7 và b−7..b của một tờ AB đóng ghim giữa: (mặt A, mặt B) — `buildToAB`.
fn build_ab(a: i64, b: i64) -> (Half, Half) {
    let (a2, b2) = (a + 4, b - 4);
    let c1 = ([b - 3, a + 3], [b, a]);
    let c2 = ([a + 2, b - 2], [a + 1, b - 1]);
    let c3 = ([b2 - 3, a2 + 3], [b2, a2]);
    let c4 = ([a2 + 2, b2 - 2], [a2 + 1, b2 - 1]);
    let join = |l: ([i64; 2], [i64; 2]), r: ([i64; 2], [i64; 2])| Half {
        top: vec![l.0[0], l.0[1], r.0[0], r.0[1]],
        bottom: vec![l.1[0], l.1[1], r.1[0], r.1[1]],
    };
    (join(c3, c1), join(c2, c4))
}

/// Các mặt dàn Đóng ghim giữa của `n` trang (lồng từ ngoài vào), `small`: họ A5.
pub(super) fn saddle_plan(n: i64, cover: bool, small: bool) -> Vec<Face> {
    let cum4 = |a: i64, b: i64| vec![b, a, a + 1, b - 1];
    let cum8 = |a: i64, b: i64| (vec![b - 3, a + 3, a + 2, b - 2], vec![b, a, a + 1, b - 1]);
    let mut faces = vec![];
    let (mut a, mut b, mut sheet) = (1i64, n, 0i64);
    if cover {
        let (kind, label) = if small { (Kind::Small4, "BIA A5 (4 trang)") } else { (Kind::Tt4, "TT4 BIA A4 (4 trang)") };
        faces.push(Face { row: cum4(a, b), ..Face::new(kind, 0, label) });
        a += 2;
        b -= 2;
    }
    if small {
        let order: &[i64] = match (b - a + 1).rem_euclid(32) {
            28 => &[4, 8, 16],
            24 => &[8, 16],
            20 => &[4, 16],
            16 => &[16],
            12 => &[4, 8],
            8 => &[8],
            4 => &[4],
            _ => &[],
        };
        for &count in order {
            sheet += 1;
            match count {
                4 => {
                    faces.push(Face { row: cum4(a, b), ..Face::new(Kind::Small4, sheet, "TU TRO A5 4 trang") });
                    a += 2;
                    b -= 2;
                }
                8 => {
                    let (top, bottom) = cum8(a, b);
                    faces.push(Face { top, bottom, ..Face::new(Kind::Small8, sheet, "TU TRO A5 8 trang") });
                    a += 4;
                    b -= 4;
                }
                _ => {
                    let (l, r) = build_ab(a, b);
                    faces.push(Face { panels: vec![l, r], ..Face::new(Kind::Small16, sheet, "TU TRO A5 16 trang") });
                    a += 8;
                    b -= 8;
                }
            }
        }
        let mut k = 0;
        while a < b {
            k += 1;
            sheet += 1;
            let (outer_a, outer_b) = build_ab(a, b);
            let (inner_a, inner_b) = build_ab(a + 8, b - 8);
            faces.push(Face { panels: vec![outer_a, inner_a], ..Face::new(Kind::SmallAb, sheet, format!("To{k} - A")) });
            faces.push(Face { panels: vec![outer_b, inner_b], side_b: true, ..Face::new(Kind::SmallAb, sheet, format!("To{k} - B")) });
            a += 16;
            b -= 16;
        }
    } else {
        let order: &[i64] = match (b - a + 1).rem_euclid(16) {
            12 => &[4, 8],
            8 => &[8],
            4 => &[4],
            _ => &[],
        };
        for &count in order {
            sheet += 1;
            if count == 4 {
                faces.push(Face { row: cum4(a, b), ..Face::new(Kind::Tt4, sheet, "TU TRO A4 4 trang") });
                a += 2;
                b -= 2;
            } else {
                let (top, bottom) = cum8(a, b);
                faces.push(Face { top, bottom, ..Face::new(Kind::Tt8, sheet, "TU TRO A4 8 trang") });
                a += 4;
                b -= 4;
            }
        }
        let mut k = 0;
        while a < b {
            k += 1;
            sheet += 1;
            let (fa, fb) = build_ab(a, b);
            a += 8;
            b -= 8;
            faces.push(Face { top: fa.top, bottom: fa.bottom, ..Face::new(Kind::Ab, sheet, format!("To{k} - A")) });
            faces.push(Face { top: fb.top, bottom: fb.bottom, side_b: true, ..Face::new(Kind::Ab, sheet, format!("To{k} - B")) });
        }
    }
    faces
}

/// Các mặt dàn Keo gáy của `n` trang (`dcKeoGayPlan`).
pub(super) fn keo_plan(n: i64, small: bool) -> std::result::Result<Vec<Face>, String> {
    if n <= 0 || n % 4 != 0 {
        return Err("Số trang ruột phải là bội số của 4.".into());
    }
    const SIG_A: ([i64; 4], [i64; 4]) = ([5, 12, 9, 8], [4, 13, 16, 1]);
    const SIG_B: ([i64; 4], [i64; 4]) = ([7, 10, 11, 6], [2, 15, 14, 3]);
    let block = |base: i64, side_a: bool, rotation: f64| {
        let (top, bottom) = if side_a { SIG_A } else { SIG_B };
        Block { top: top.iter().map(|v| base + v).collect(), bottom: bottom.iter().map(|v| base + v).collect(), rotation }
    };
    let capacity = if small { 32 } else { 16 };
    let full = n / capacity;
    let mut faces = vec![];
    let mut sheet = 0;
    for t in 0..full {
        let base = t * capacity;
        sheet += 1;
        let mut fa = Face::new(Kind::Ab, sheet, format!("Ruột{sheet}-A"));
        let mut fb = Face { side_b: true, ..Face::new(Kind::Ab, sheet, format!("Ruột{sheet}-B")) };
        if small {
            fa.blocks = vec![block(base, true, 90.0), block(base + 16, true, 90.0)];
            fb.blocks = vec![block(base + 16, false, -90.0), block(base, false, -90.0)];
        } else {
            let (a, b) = (block(base, true, 90.0), block(base, false, -90.0));
            (fa.top, fa.bottom, fb.top, fb.bottom) = (a.top, a.bottom, b.top, b.bottom);
        }
        faces.push(fa);
        faces.push(fb);
    }
    let mut rem = n - full * capacity;
    let mut base = full * capacity;
    if small && rem >= 16 {
        sheet += 1;
        faces.push(Face {
            blocks: vec![block(base, true, 90.0), block(base, false, -90.0)],
            ..Face::new(Kind::Tt16, sheet, format!("Ruột{sheet} TT16"))
        });
        base += 16;
        rem -= 16;
    }
    let (mut first, mut last) = (base + 1, n);
    if rem >= 8 {
        sheet += 1;
        faces.push(Face {
            top: vec![last - 3, first + 3, first + 2, last - 2],
            bottom: vec![last, first, first + 1, last - 1],
            small_self_turn: small,
            ..Face::new(Kind::Tt8, sheet, format!("Ruột{sheet} TT8"))
        });
        first += 4;
        last -= 4;
        rem -= 8;
    }
    if rem == 4 {
        sheet += 1;
        faces.push(Face { row: vec![first, last, first + 1, last - 1], ..Face::new(Kind::Tt4, sheet, format!("Ruột{sheet} TT4")) });
    }
    Ok(faces)
}

// ---------------------------------------------------------------- form và lưới danh định (Đóng ghim giữa)

#[derive(Clone, Copy, Debug, PartialEq)]
enum Anchor {
    /// Lưới tì mép dưới tờ (họ A5).
    Bottom,
    /// Canh giữa tờ (TT4 A4).
    Center,
    /// Chừa lề nhíp 23 mm (TT8/AB A4).
    Top,
}

/// Bản mô tả form (`formSpec`).
#[derive(Clone, Copy, Debug)]
struct Form {
    cols: usize,
    rows: usize,
    rotated: bool,
    gutter: f64,
    row_gap: f64,
    sheet: (f64, f64),
    anchor: Anchor,
    trim_cut: f64,
    /// PON giấy loại lớn (tờ 85,8 cm).
    large: bool,
    mid_axis: bool,
    row_seam: bool,
    corner_legs: bool,
    page_axes: bool,
}

fn form(kind: Kind) -> Form {
    match kind {
        Kind::Small4 => Form {
            cols: 2,
            rows: 2,
            rotated: true,
            gutter: 5.0 * MM,
            row_gap: 0.0,
            sheet: SHEET_COVER_A5,
            anchor: Anchor::Bottom,
            trim_cut: 0.0,
            large: false,
            mid_axis: true,
            row_seam: false,
            corner_legs: true,
            page_axes: true,
        },
        Kind::Small8 => Form {
            cols: 4,
            rows: 2,
            rotated: false,
            gutter: 8.0 * MM,
            row_gap: 0.0,
            sheet: SHEET_SMALL,
            anchor: Anchor::Bottom,
            trim_cut: 2.5 * MM,
            large: false,
            mid_axis: true,
            row_seam: true,
            corner_legs: true,
            page_axes: false,
        },
        Kind::Small16 | Kind::SmallAb => Form {
            cols: 4,
            rows: 4,
            rotated: true,
            gutter: 6.0 * MM,
            row_gap: 15.0 * MM,
            sheet: if kind == Kind::SmallAb { SHEET_AB } else { SHEET_MAIN },
            anchor: Anchor::Bottom,
            trim_cut: 0.0,
            large: true,
            mid_axis: true,
            row_seam: true,
            corner_legs: true,
            page_axes: true,
        },
        Kind::Tt4 => Form {
            cols: 2,
            rows: 2,
            rotated: true,
            gutter: 7.6 * MM,
            row_gap: 0.0,
            sheet: SHEET_SMALL,
            anchor: Anchor::Center,
            trim_cut: 0.0,
            large: false,
            mid_axis: true,
            row_seam: false,
            corner_legs: false,
            page_axes: false,
        },
        Kind::Tt8 | Kind::Ab | Kind::Tt16 => Form {
            cols: 4,
            rows: 2,
            rotated: false,
            gutter: 8.0 * MM,
            row_gap: 0.0,
            sheet: if kind == Kind::Ab { SHEET_AB } else { SHEET_MAIN },
            anchor: Anchor::Top,
            trim_cut: 0.0,
            large: true,
            mid_axis: false,
            row_seam: true,
            corner_legs: true,
            page_axes: false,
        },
    }
}

/// Kích thước của form cho một khổ trang (pt).
struct Dims {
    form: Form,
    cell_w: f64,
    cell_h: f64,
    trim_h: Option<f64>,
    content_w: f64,
    content_h: f64,
    top_gap: f64,
}

/// Lưới danh định của một mặt trên tờ `rect` (pt, y hướng xuống): luôn theo khổ nhập.
#[derive(Clone, Debug)]
pub(super) struct Grid {
    form: Form,
    /// SMALL8: chiều cao ô sau khi trừ allowance.
    pub trim_h: Option<f64>,
    pub cx: f64,
    pub cy: f64,
    pub grid_left: f64,
    pub grid_right: f64,
    pub grid_top: f64,
    pub gap_left: f64,
    pub gap_right: f64,
    pub col_lines: Vec<f64>,
    /// (y, hướng nét dọc: −1 lên, +1 xuống, 0 không có nét dọc).
    pub row_lines: Vec<(f64, i8)>,
}

/// Số mm như DanCard in: làm tròn 0,1 mm.
fn mm_text(v: f64) -> String {
    ((v / MM * 10.0).round() / 10.0).to_string()
}

/// Ô, nội dung và lề nhíp của form `kind` cho trang `pw` × `ph` pt; báo lỗi khi bài không lọt tờ.
fn dims(kind: Kind, pw: f64, ph: f64) -> std::result::Result<Dims, String> {
    let form = form(kind);
    let (cols, rows) = (form.cols as f64, form.rows as f64);
    let cell_w = if form.rotated { ph } else { pw };
    let full_h = if form.rotated { pw } else { ph };
    let required = ((full_h * rows + form.row_gap - form.sheet.1) / rows).max(0.0);
    let cell_h = full_h - form.trim_cut.min(required);
    if cell_h <= 0.0 || !cell_h.is_finite() {
        return Err("Chiều cao trang quá nhỏ cho form này.".into());
    }
    let trim_h = (form.trim_cut > 0.0).then_some(cell_h);
    let content_w = cell_w * cols + form.gutter;
    let content_h = cell_h * rows + form.row_gap;
    let top_gap = if form.anchor == Anchor::Top { SIG_TOP_GAP } else { 0.0 };
    let over_w = content_w - form.sheet.0;
    let over_h = content_h + top_gap - form.sheet.1;
    if over_w > 0.5 || over_h > 0.5 {
        let mut over = String::new();
        if over_w > 0.5 {
            over.push_str(&format!("ngang {} mm", mm_text(over_w)));
        }
        if over_w > 0.5 && over_h > 0.5 {
            over.push_str(" và ");
        }
        if over_h > 0.5 {
            over.push_str(&format!("dọc {} mm", mm_text(over_h)));
        }
        return Err(format!(
            "Khổ trang nhập quá lớn cho tờ {} x {} mm: bài cần {} x {} mm, dư {over}. Hãy nhập lại khổ trang nhỏ hơn.",
            mm_text(form.sheet.0),
            mm_text(form.sheet.1),
            mm_text(content_w),
            mm_text(content_h + top_gap)
        ));
    }
    Ok(Dims { form, cell_w, cell_h, trim_h, content_w, content_h, top_gap })
}

/// Khổ giấy (pt) của form `kind`; lỗi khi trang `pw` × `ph` pt không lọt tờ.
pub(super) fn paper_for(kind: Kind, pw: f64, ph: f64) -> std::result::Result<(f64, f64), String> {
    dims(kind, pw, ph).map(|d| d.form.sheet)
}

/// `nominalGridFor` trên tờ `rect`.
pub(super) fn grid_for(kind: Kind, pw: f64, ph: f64, rect: Rect) -> std::result::Result<Grid, String> {
    let Dims { form, cell_w, cell_h, trim_h, content_w, content_h, top_gap } = dims(kind, pw, ph)?;
    let (cols, rows) = (form.cols as f64, form.rows as f64);
    let c = rect.center();
    let grid_left = rect.x0 + (form.sheet.0 - content_w) / 2.0;
    let (grid_top, grid_bottom) = match form.anchor {
        Anchor::Bottom => (rect.y1 - content_h, rect.y1),
        Anchor::Center => (c.y - content_h / 2.0, c.y + content_h / 2.0),
        Anchor::Top => (rect.y0 + top_gap, rect.y0 + top_gap + content_h),
    };
    let gap_left = grid_left + cell_w * cols / 2.0;
    let col_lines = (0..=form.cols).map(|i| grid_left + i as f64 * cell_w + (if i * 2 > form.cols { form.gutter } else { 0.0 })).collect();
    let mut row_lines = vec![(grid_top, -1)];
    if form.row_gap > 0.0 {
        let block_bottom = grid_top + cell_h * rows / 2.0;
        row_lines.push((block_bottom, 1));
        row_lines.push((block_bottom + form.row_gap, -1));
    } else if form.row_seam {
        row_lines.push((grid_top + cell_h * rows / 2.0, 0));
    }
    row_lines.push((grid_bottom, 1));
    Ok(Grid {
        form,
        trim_h,
        cx: c.x,
        cy: c.y,
        grid_left,
        grid_right: grid_left + content_w,
        grid_top,
        gap_left,
        gap_right: gap_left + form.gutter,
        col_lines,
        row_lines,
    })
}

/// Một nét PON: hai đầu và độ dày (pt).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Line {
    pub a: Point,
    pub b: Point,
    pub width: f64,
}

/// PON giấy: 8 nét góc, tim nằm đúng mép tờ `r` (§2.7).
pub(super) fn paper_pon(r: Rect, large: bool) -> Vec<Line> {
    let (arm, leg, width) = if large { (18.766, 21.091, 3.971) } else { (14.173, 14.106, 1.995) };
    let (l, t, rr, b) = (r.x0, r.y0, r.x1, r.y1);
    [
        ((l, t), (l + arm, t)),
        ((l, t), (l, t + leg)),
        ((rr - arm, t), (rr, t)),
        ((rr, t), (rr, t + leg)),
        ((l, b), (l + arm, b)),
        ((l, b), (l, b - leg)),
        ((rr - arm, b), (rr, b)),
        ((rr, b), (rr, b - leg)),
    ]
    .into_iter()
    .map(|((x1, y1), (x2, y2))| Line { a: Point::new(x1, y1), b: Point::new(x2, y2), width })
    .collect()
}

/// PON cắt Đóng ghim giữa (`addAutoCutPon`) → (nét, số mốc bị kéo về mép tờ).
pub(super) fn saddle_cut_pon(kind: Kind, pw: f64, ph: f64, rect: Rect) -> std::result::Result<(Vec<Line>, usize), String> {
    let g = grid_for(kind, pw, ph, rect)?;
    let (xlo, xhi, ylo, yhi) = (rect.x0, rect.x1, rect.y0, rect.y1);
    let mut clamps = 0usize;
    let mut clamp = |v: f64, lo: f64, hi: f64| {
        if v < lo {
            if lo - v > 0.001 {
                clamps += 1;
            }
            lo
        } else if v > hi {
            if v - hi > 0.001 {
                clamps += 1;
            }
            hi
        } else {
            v
        }
    };
    // Vẽ theo hướng mong muốn; tràn ra ngoài tờ thì quay ngược; cả hai phía chật thì về phía rộng hơn.
    let tick = |s: f64, dir: i8, lo: f64, hi: f64| {
        let want = s + f64::from(dir) * CUT_MARK;
        if (lo..=hi).contains(&want) {
            return want;
        }
        let back = s - f64::from(dir) * CUT_MARK;
        if (lo..=hi).contains(&back) {
            return back;
        }
        if hi - s > s - lo { hi } else { lo }
    };
    let mut axes: Vec<(f64, i8)> = vec![];
    let mut add = |x: f64, dir: i8| {
        if !axes.iter().any(|a| (a.0 - x).abs() < 0.001) {
            axes.push((x, dir));
        }
    };
    add(g.grid_left, -1);
    add(g.gap_left, 1);
    add(g.gap_right, -1);
    add(g.grid_right, 1);
    if g.form.mid_axis {
        add(g.cx, 0);
    }
    if g.form.page_axes {
        for x in g.col_lines.iter().skip(1).take(g.form.cols.saturating_sub(1)) {
            add(*x, 0);
        }
    }
    let mut out = vec![];
    for &(x, hdir) in &axes {
        for &(y, vdir) in &g.row_lines {
            if vdir != 0 {
                let (ax, ay) = (clamp(x, xlo, xhi), clamp(y, ylo, yhi));
                out.push(Line { a: Point::new(ax, ay), b: Point::new(ax, tick(ay, vdir, ylo, yhi)), width: CUT_STROKE });
            }
            if hdir != 0 && g.form.corner_legs {
                let (ax, ay) = (clamp(x, xlo, xhi), clamp(y, ylo, yhi));
                out.push(Line { a: Point::new(ax, ay), b: Point::new(tick(ax, hdir, xlo, xhi), ay), width: CUT_STROKE });
            }
        }
    }
    Ok((out, clamps))
}

// ---------------------------------------------------------------- Keo gáy: khung, lưới khối, PON cắt

/// Tỷ lệ (×1, không phải %) co trang `w` × `h` về `tw` × `th` (`dcKeoGayPageScales`).
pub(super) fn keo_page_scales(w: f64, h: f64, tw: f64, th: f64) -> std::result::Result<(f64, f64), String> {
    let ok = |v: f64| v.is_finite() && v > 0.0;
    if !(ok(w) && ok(h) && ok(tw) && ok(th)) {
        return Err("Không đo được kích thước trang ruột.".into());
    }
    let (x, y) = (tw / w, th / h);
    if !(ok(x) && ok(y)) {
        return Err("Không tính được tỷ lệ kích thước trang ruột.".into());
    }
    Ok((x, y))
}

/// Khung trang (mm) trên tờ: tờ nhỏ thu đều trang để cạnh nằm ngang tờ tối đa 20,9 cm.
pub(super) fn keo_frame(w: f64, h: f64, small: bool, small_paper: bool) -> (f64, f64) {
    let scale = if small_paper { (209.0 / if small { h } else { w }).min(1.0) } else { 1.0 };
    (w * scale, h * scale)
}

/// Một ô của lưới khối Keo gáy: trang, góc xoay (độ, ngược chiều kim đồng hồ), tâm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Cell {
    pub page: i64,
    pub angle: f64,
    pub center: Point,
}

/// `dcKeoGayPackedGrid`: các khối 16 trang nằm ngang trên tờ `rect` (y hướng xuống) → (khung, ô).
pub(super) fn keo_packed_grid(face: &Face, pw: f64, ph: f64, rect: Rect, top_gap: f64) -> (Rect, Vec<Cell>) {
    let width = face.blocks.len() as f64 * 2.0 * ph;
    let height = 4.0 * pw;
    let left = (rect.x0 + rect.x1 - width) / 2.0;
    let top = rect.y0 + top_gap;
    let mut cells = vec![];
    for (bi, block) in face.blocks.iter().enumerate() {
        let ccw = block.rotation > 0.0;
        for (r, row) in [&block.top, &block.bottom].into_iter().enumerate() {
            for (c, page) in row.iter().enumerate().take(4) {
                let col = if ccw { r } else { 1 - r };
                let rr = if ccw { 3usize.saturating_sub(c) } else { c };
                cells.push(Cell {
                    page: *page,
                    angle: (if r == 0 { 180.0 } else { 0.0 }) + block.rotation,
                    center: Point::new(left + ((bi * 2 + col) as f64 + 0.5) * ph, top + (rr as f64 + 0.5) * pw),
                });
            }
        }
    }
    (Rect::new(left, top, left + width, top + height), cells)
}

/// PON cắt Keo gáy của mặt `face` trên tờ `rect` với khung trang `pw` × `ph` pt.
pub(super) fn keo_cut_pon(face: &Face, pw: f64, ph: f64, rect: Rect, gap4: f64) -> Vec<Line> {
    let l = if face.kind == Kind::Tt4 { 5.642 } else { 2.0 * MM };
    // Mọi đầu nét nằm trong tờ, cách mép 0,5 pt.
    let safe = |p: Point| Point::new((rect.x0 + 0.5).max((rect.x1 - 0.5).min(p.x)), (rect.y1 - 0.5).min((rect.y0 + 0.5).max(p.y)));
    let mut out = vec![];
    let mut line = |a: Point, b: Point| out.push(Line { a: safe(a), b: safe(b), width: CUT_STROKE });
    let c = rect.center();
    if !face.blocks.is_empty() {
        let (g, _) = keo_packed_grid(face, pw, ph, rect, KEO_TOP_GAP);
        let w = g.width();
        let axes = [g.x0, g.x0 + w / 4.0, g.x0 + w / 2.0, g.x0 + w * 3.0 / 4.0, g.x1];
        for x in axes {
            line(Point::new(x, g.y0 - l), Point::new(x, g.y0));
            line(Point::new(x, g.y1), Point::new(x, g.y1 + l));
        }
        for y in [g.y0, g.y0 + g.height() / 2.0, g.y1] {
            line(Point::new(g.x0 - l, y), Point::new(g.x0, y));
            line(Point::new(g.x1, y), Point::new(g.x1 + l, y));
        }
    } else if face.kind == Kind::Tt4 {
        let cum_w = ph;
        let total = cum_w * 2.0 + gap4;
        let left = c.x - total / 2.0;
        let o = 0.2 * MM;
        let axes = [left - o, left + cum_w + o, c.x, left + cum_w + gap4 - o, left + total + o];
        let (top, bottom) = (c.y - pw, c.y + pw);
        // Nét hướng ra ngoài bài; sát mép giấy thì quay vào trong.
        let top_end = if top - l < rect.y0 + 0.5 { top + l } else { top - l };
        let bottom_end = if bottom + l > rect.y1 - 0.5 { bottom - l } else { bottom + l };
        for x in axes {
            line(Point::new(x, top_end), Point::new(x, top));
            line(Point::new(x, bottom), Point::new(x, bottom_end));
        }
    } else {
        let (left, right) = (c.x - pw * 2.0, c.x + pw * 2.0);
        let y_top = if face.small_self_turn { c.y - ph } else { rect.y0 + KEO_TOP_GAP };
        let (y_mid, y_bot) = (y_top + ph, y_top + ph * 2.0);
        for x in [left, c.x, right] {
            line(Point::new(x, y_top - l), Point::new(x, y_top));
            line(Point::new(x, y_bot), Point::new(x, y_bot + l));
        }
        for y in [y_top, y_mid, y_bot] {
            line(Point::new(left, y), Point::new(left - l, y));
            line(Point::new(right, y), Point::new(right + l, y));
        }
    }
    out
}

// ---------------------------------------------------------------- vị trí các tờ trên canvas

/// Đặt các hàng tờ (`rows`: mặt A, mặt B nếu có) từ góc trên trái `start` của vùng chọn, xuống
/// dưới; chạm đáy canvas thì sang cột kế bên (phải trước, rồi trái) — `reserveRow`. `keo`: mặt B
/// cách mép trái hàng `main_w + 20 mm`. → (khung từng mặt, số cột đã dùng).
pub(super) fn place_sheets(
    papers: &[(f64, f64)],
    rows: &[(usize, Option<usize>)],
    start: Point,
    main_w: f64,
    keo: bool,
) -> std::result::Result<(Vec<Rect>, usize), String> {
    let what = if keo { "CTL Keo Gáy" } else { "CTL Offset" };
    let guard = 10.0 * MM + 22.0;
    let safe = Rect::new(-CANVAS_HALF + guard, -CANVAS_HALF + guard, CANVAS_HALF - guard, CANVAS_HALF - guard);
    if !(start.x.is_finite() && start.y.is_finite()) || start.x < safe.x0 || start.y < safe.y0 {
        return Err("Object nguồn đang quá sát biên canvas; không thể bắt đầu dàn đúng tại vị trí đó.".into());
    }
    let step = main_w * 2.0 + GAP_COL + GAP_COL;
    let full = || format!("Đã hết chỗ trong canvas (±7200 pt) để dàn tiếp {what}. Hãy dời các trang nguồn về gần tâm canvas rồi chạy lại.");
    let mut used: Vec<i64> = vec![0];
    let (mut col_left, mut cur_t) = (start.x, start.y);
    let mut rects = vec![Rect::ZERO; papers.len()];
    for &(a, b) in rows {
        let Some(&pa) = papers.get(a) else { continue };
        let pb = b.and_then(|b| papers.get(b).copied());
        let (row_w, row_h, b_off) = match pb {
            Some(pb) => {
                let slot = if keo { main_w } else { pa.0 };
                (slot + GAP_COL + pb.0, pa.1.max(pb.1), slot + GAP_COL)
            }
            None => (pa.0, pa.1, 0.0),
        };
        if start.y + row_h > safe.y1 {
            return Err(format!("Không đủ chiều cao canvas từ vị trí object nguồn để đặt một tờ {what}."));
        }
        if used.len() == 1 && col_left + row_w > safe.x1 {
            return Err(format!("Tờ {what} đầu tiên không nằm vừa bên phải vị trí object nguồn."));
        }
        let mut tries = 0;
        while cur_t + row_h > safe.y1 || col_left + row_w > safe.x1 {
            tries += 1;
            let found = (1..1000i64).flat_map(|d| [d, -d]).find_map(|off| {
                let left = start.x + off as f64 * step;
                (!used.contains(&off) && left >= safe.x0 && left + row_w <= safe.x1).then_some((off, left))
            });
            let Some((off, left)) = found.filter(|_| tries < 2000) else { return Err(full()) };
            used.push(off);
            col_left = left;
            cur_t = start.y;
        }
        if let Some(r) = rects.get_mut(a) {
            *r = Rect::new(col_left, cur_t, col_left + pa.0, cur_t + pa.1);
        }
        if let (Some(b), Some(pb)) = (b, pb)
            && let Some(r) = rects.get_mut(b)
        {
            *r = Rect::new(col_left + b_off, cur_t, col_left + b_off + pb.0, cur_t + pb.1);
        }
        cur_t += row_h + GAP_ROW;
    }
    Ok((rects, used.len()))
}

/// Một file trong kế hoạch lưu theo khổ.
struct SizeJob {
    name: String,
    ab: Vec<usize>,
    w: f64,
    h: f64,
}

/// Kế hoạch lưu theo khổ (`dcCtlOffsetPdfPlan`): mặt `k` là artboard `k + 1`.
pub(super) fn size_plan(faces: &[Face], rects: &[Rect]) -> Value {
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let mut groups: Vec<(String, Vec<SizeJob>)> = vec![];
    for (k, (f, r)) in faces.iter().zip(rects).enumerate() {
        let cover = f.sheet <= 0;
        let key = if cover { "BIA".to_string() } else { f.kind.key().to_string() };
        let name = if cover { "BIA".to_string() } else { format!("RUOT {}", f.sheet) };
        let gi = match groups.iter().position(|g| g.0 == key) {
            Some(i) => i,
            None => {
                groups.push((key, vec![]));
                groups.len() - 1
            }
        };
        let Some(g) = groups.get_mut(gi) else { continue };
        match g.1.iter_mut().find(|j| j.name == name) {
            Some(j) => j.ab.push(k + 1),
            None => g.1.push(SizeJob { name, ab: vec![k + 1], w: round2(r.width()), h: round2(r.height()) }),
        }
    }
    let groups: Vec<Value> = groups
        .iter()
        .map(|(key, jobs)| {
            let jobs: Vec<Value> = jobs.iter().map(|j| json!({"name": j.name, "ab": j.ab, "w": j.w, "h": j.h})).collect();
            json!({"key": key, "label": size_label(key), "jobs": jobs})
        })
        .collect();
    json!({"total": rects.len(), "groups": groups})
}

// ---------------------------------------------------------------- đo và sửa trên tài liệu

fn scaling() -> Scaling {
    Scaling { strokes: true, patterns: true, ..Scaling::default() }
}

fn finite(r: &Rect) -> bool {
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite())
}

/// Khung thật của một trang (`sizeMM`): clip group theo clipping path, còn lại theo vùng nhìn thấy.
fn page_box(n: &Node) -> Option<Rect> {
    if let NodeKind::Group { children, clip: true } = &n.kind
        && let Some(b) = children.first().and_then(|m| m.geometric_bounds())
    {
        return Some(b).filter(finite);
    }
    vis(n).filter(finite)
}

fn box_of(d: &Document, id: NodeId) -> Option<Rect> {
    d.node(id).and_then(page_box)
}

fn apply(d: &mut Document, id: NodeId, m: Affine) {
    if let Some(n) = d.node_mut(id) {
        n.transform(m, scaling());
    }
}

/// Xoay `deg` độ ngược chiều kim đồng hồ (như Illustrator) quanh `c`.
fn rotation_about(deg: f64, c: Point) -> Affine {
    Affine::translate(c.to_vec2()) * Affine::rotate(-deg.to_radians()) * Affine::translate(-c.to_vec2())
}

fn scale_about(sx: f64, sy: f64, c: Point) -> Affine {
    Affine::translate(c.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-c.to_vec2())
}

/// Xoay `id` quanh tâm khung của nó.
fn rotate(d: &mut Document, id: NodeId, deg: f64) {
    if deg.rem_euclid(360.0) == 0.0 {
        return;
    }
    if let Some(b) = box_of(d, id) {
        apply(d, id, rotation_about(deg, b.center()));
    }
}

/// Đặt tâm khung của `id` tại `at` (`centerItem`).
fn center(d: &mut Document, id: NodeId, at: Point) {
    if let Some(b) = box_of(d, id) {
        apply(d, id, Affine::translate(at - b.center()));
    }
}

/// Co cả cụm `ids` quanh tâm chung (`squeezeAboutCentre`).
fn squeeze(d: &mut Document, ids: &[NodeId], sx: f64, sy: f64) {
    if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0) {
        return;
    }
    let Some(u) = union_of(ids.iter().filter_map(|id| box_of(d, *id))) else { return };
    let m = scale_about(sx, sy, u.center());
    for id in ids {
        apply(d, *id, m);
    }
}

/// Trang số `p` (từ 1) đã chuẩn bị.
fn page(pages: &[NodeId], p: i64) -> Option<NodeId> {
    usize::try_from(p - 1).ok().and_then(|i| pages.get(i)).copied()
}

/// `id` nằm trong một layer tên thuộc `names`.
fn in_layer_named(d: &Document, id: NodeId, names: &[&str]) -> bool {
    d.ancestry(id)
        .is_some_and(|a| a.iter().any(|x| d.node(*x).is_some_and(|n| n.is_layer() && n.name.as_deref().is_some_and(|nm| names.contains(&nm)))))
}

/// Thứ tự đọc (§2.1): hàng trên trước (cùng hàng nếu mép trên lệch không quá 50 mm), trái → phải.
/// Sắp ổn định (chèn) theo đúng hàm so sánh của DanCard, kể cả khi nó không bắc cầu. → (trang,
/// góc trên trái vùng chọn).
fn reading_order(d: &Document, ids: &[NodeId]) -> Result<(Vec<NodeId>, Point)> {
    let mut items: Vec<(NodeId, Rect)> = vec![];
    // Như vùng chọn của Illustrator: object trên cùng trước.
    for id in ids.iter().rev() {
        let n = d.node(*id).ok_or(EngineError::NoNode(*id))?;
        let b = vis(n).filter(finite).ok_or_else(|| other("Có trang không đo được kích thước."))?;
        items.push((*id, b));
    }
    let start =
        items.iter().fold(None, |acc: Option<Point>, (_, b)| Some(acc.map_or(Point::new(b.x0, b.y0), |p| Point::new(p.x.min(b.x0), p.y.min(b.y0)))));
    let start = start.ok_or_else(|| other("Không xác định được vị trí object nguồn."))?;
    let after = |a: &Rect, b: &Rect| {
        let dy = a.y0 - b.y0;
        if dy.abs() > 50.0 * MM { dy > 0.0 } else { a.x0 > b.x0 }
    };
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 {
            let (Some(x), Some(y)) = (items.get(j - 1), items.get(j)) else { break };
            if !after(&x.1, &y.1) {
                break;
            }
            items.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok((items.into_iter().map(|(id, _)| id).collect(), start))
}

/// Layer đang vẽ (không phải layer PON của lệnh), else layer trên cùng không phải PON, else layer mới.
fn artwork_layer(d: &mut Document, hint: Option<NodeId>, pon: &[&str]) -> Result<NodeId> {
    if let Some(h) = hint
        && d.node(h).is_some_and(Node::is_layer)
        && !in_layer_named(d, h, pon)
    {
        return Ok(h);
    }
    if let Some(l) = d.layers.iter().rev().find(|l| !l.name.as_deref().is_some_and(|n| pon.contains(&n))).map(|l| l.id) {
        return Ok(l);
    }
    let n = crate::cmd::layer::make_layer(d, None);
    Ok(d.insert(None, 0, n)?)
}

/// Xoá các layer cấp đầu tên thuộc `names` (PON tự vẽ của lần dàn trước).
fn remove_layers(d: &mut Document, names: &[&str]) -> Result<()> {
    let old: Vec<NodeId> = d.layers.iter().filter(|l| l.name.as_deref().is_some_and(|n| names.contains(&n))).map(|l| l.id).collect();
    if !old.is_empty() && old.len() < d.layers.len() {
        d.remove_many(&old)?;
    }
    Ok(())
}

/// Layer PON `name` mới trên cùng chứa `lines` (K 100 %, không tô), khóa lại.
fn add_pon_layer(d: &mut Document, name: &str, lines: &[Line]) -> Result<NodeId> {
    let ink = Paint::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0));
    let mut layer = crate::cmd::layer::make_layer(d, Some(name));
    let nodes: Vec<Arc<Node>> = lines
        .iter()
        .map(|l| {
            let mut n = Node::path(d.alloc_id(), shapes::line(l.a, l.b), Appearance::basic(Paint::None, ink.clone(), l.width));
            n.name = Some("PON".into());
            Arc::new(n)
        })
        .collect();
    if let Some(ch) = layer.children_mut() {
        *ch = nodes;
    }
    layer.locked = true;
    Ok(d.insert(None, usize::MAX, layer)?)
}

/// Làm phẳng một trang (`dcFlattenForImposition`, khung `frame`), đưa lên trên cùng layer `art`.
fn flatten_into(d: &mut Document, id: NodeId, frame: Rect, art: NodeId) -> Result<NodeId> {
    let n = d.node(id).ok_or(EngineError::NoNode(id))?;
    if n.is_layer() {
        return Err(other("Chọn các trang (object), không chọn cả layer."));
    }
    let flat = if image_only(n) {
        id
    } else {
        if !(finite(&frame) && frame.width() > 0.0 && frame.height() > 0.0) {
            return Err(other("Không xác định được khung để raster."));
        }
        raster_in(d, id, frame)?
    };
    d.move_node(flat, Some(art), usize::MAX)?;
    Ok(flat)
}

/// Artboard của các mặt, thay các artboard cũ (`dcRemoveOldCanvasArtboards`).
fn set_artboards(d: &mut Document, rects: &[Rect], names: &[String]) {
    let first = d.next_artboard_id();
    d.artboards = rects
        .iter()
        .zip(names)
        .enumerate()
        .map(|(i, (r, name))| Artboard {
            id: first.saturating_add(u32::try_from(i).unwrap_or(0)),
            name: name.clone(),
            rect: *r,
            show_center_mark: false,
            show_cross_hairs: false,
        })
        .collect();
}

// ---------------------------------------------------------------- tham số chung

/// Khổ một trang (mm) từ `width`, `height` (cm).
fn page_size_mm(p: &Value) -> Result<(f64, f64)> {
    let ok = |v: f64| (0.01..1000.0).contains(&v);
    match (number_param(p, "width"), number_param(p, "height")) {
        (Some(w), Some(h)) if ok(w) && ok(h) => Ok((w * 10.0, h * 10.0)),
        _ => Err(other("Nhập kích thước hợp lệ (rộng × cao, cm).")),
    }
}

/// Họ A5: cạnh ngắn ≤ 150 mm và cạnh dài ≤ 211,5 mm.
fn is_a5(w: f64, h: f64) -> bool {
    w.min(h) <= 150.0 && w.max(h) <= 211.5
}

/// `cover`: true, "true", 1 hoặc "1" (như DanCard); không có: true (mặc định của panel).
fn cover_flag(p: &Value) -> bool {
    match p.get("cover") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true" || s == "1",
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        Some(_) => false,
    }
}

/// Ghi chú thay cho hộp thoại `Nhập ghi chú`.
struct Notes {
    on: bool,
    cover: String,
    inside: String,
    by_type: Value,
}

fn clean_note(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n").trim().chars().take(NOTE_MAX_CHARS).collect()
}

impl Notes {
    fn parse(p: &Value) -> Self {
        Notes {
            on: bool_or(p, "notes", true),
            cover: str_param(p, "coverNote").map(clean_note).unwrap_or_default(),
            inside: str_param(p, "insideNote").map(clean_note).unwrap_or_default(),
            by_type: p.get("typeNotes").cloned().unwrap_or(Value::Null),
        }
    }

    /// Chữ riêng của loại tờ `kind` (ô riêng, else `insideNote`).
    fn specific(&self, kind: Kind) -> String {
        let own = self.by_type.get(kind.note_key()).and_then(Value::as_str).map(clean_note).unwrap_or_default();
        if own.is_empty() { self.inside.clone() } else { own }
    }

    /// "RUỘT n" + chữ riêng; đỏ khi có chữ riêng.
    fn inner(&self, f: &Face) -> (String, bool) {
        let extra = self.specific(f.kind);
        if extra.is_empty() { (format!("RUỘT {}", f.sheet), false) } else { (format!("RUỘT {} {extra}", f.sheet), true) }
    }
}

// ---------------------------------------------------------------- ghi chú

fn note_object(text: &str, red: bool, size: f64) -> TextObject {
    let ink = if red { Color::cmyk(0.0, 1.0, 1.0, 0.0) } else { Color::cmyk(0.0, 0.0, 0.0, 1.0) };
    let style = CharStyle { size, fill: Paint::solid(ink), ..CharStyle::default() };
    let mut t = TextObject::point(Point::ZERO, text, style);
    t.para = crate::cmd::create::new_type_para();
    crate::cmd::typecmd::refresh_bounds(&mut t);
    t
}

fn note_node(d: &mut Document, text: &str, red: bool, size: f64) -> Node {
    let mut n = Node::new(d.alloc_id(), NodeKind::Text(Box::new(note_object(text, red, size))));
    n.name = Some("Ghi chú CTL".into());
    n
}

fn node_box(n: &Node) -> Option<Rect> {
    n.visual_bounds().or_else(|| n.geometric_bounds()).filter(finite)
}

fn turn(n: &mut Node, deg: f64) {
    if let Some(b) = node_box(n) {
        n.transform(rotation_about(deg, b.center()), scaling());
    }
}

fn shift(n: &mut Node, by: Vec2) {
    n.transform(Affine::translate(by), scaling());
}

/// Bề ngang của dòng chữ ghi chú `text` (cỡ 13 pt).
fn text_width(text: &str) -> f64 {
    note_object(text, false, NOTE_SIZE).bounds().map_or(0.0, |b| b.width())
}

/// Ghi chú dọc (đọc từ dưới lên): mép trái vào 9 mm, mép dưới lên 15 mm (`addNote4`).
fn note_vertical(d: &mut Document, text: &str, red: bool, rect: Rect) -> Option<Node> {
    if text.is_empty() {
        return None;
    }
    let mut n = note_node(d, text, red, NOTE_SIZE);
    turn(&mut n, 90.0);
    let b = node_box(&n)?;
    shift(&mut n, Vec2::new(rect.x0 + 9.0 * MM - b.x0, rect.y1 - 15.0 * MM - b.y1));
    Some(n)
}

/// Ghi chú ngang: mép trái vào 20 mm, mép trên xuống 11 mm (`addNote8`).
fn note_horizontal(d: &mut Document, text: &str, red: bool, rect: Rect) -> Option<Node> {
    if text.is_empty() {
        return None;
    }
    let mut n = note_node(d, text, red, NOTE_SIZE);
    let b = node_box(&n)?;
    shift(&mut n, Vec2::new(rect.x0 + 20.0 * MM - b.x0, rect.y0 + 11.0 * MM - b.y0));
    Some(n)
}

/// Ghi chú tờ nhỏ của Keo gáy: dọc như `addNote4`; tờ A5 43 × 32,5 hết lề trái thì nằm ngang ở dải
/// trắng phía trên (chữ thu nhỏ cho vừa). `pw`, `ph`: khung trang của mặt.
#[allow(clippy::too_many_arguments)]
fn keo_note_small(d: &mut Document, text: &str, red: bool, rect: Rect, a5: bool, pw: f64, ph: f64, gap4: f64) -> Option<Node> {
    let n = note_vertical(d, text, red, rect)?;
    let b = node_box(&n)?;
    let small4 = a5 && (rect.width() - SHEET_COVER_A5.0).abs() < 0.01;
    let grid_left = (rect.x0 + rect.x1 - (2.0 * ph + gap4)) / 2.0;
    if !(small4 && rect.x0 + 9.0 * MM + b.width() + 2.0 * MM > grid_left) {
        return Some(n);
    }
    let flat = note_node(d, text, red, NOTE_SIZE);
    let b = node_box(&flat)?;
    let avail_h = (rect.height() - 2.0 * pw) / 2.0 - 3.0 * MM;
    let avail_w = rect.width() - 40.0 * MM;
    let scale = 1f64.min(avail_h / b.height()).min(avail_w / b.width());
    // Không đủ lề trắng: bỏ ghi chú này (DanCard để lại chữ ở gốc toạ độ).
    if !(scale > 0.0 && scale.is_finite()) {
        return None;
    }
    let mut n = if scale < 1.0 { note_node(d, text, red, NOTE_SIZE * scale) } else { flat };
    let b = node_box(&n)?;
    shift(&mut n, Vec2::new(rect.x0 + 20.0 * MM - b.x0, rect.y0 + 0.5 * MM - b.y0));
    Some(n)
}

/// Ngắt dòng `contents` theo bề ngang `max_w` (đo chữ thật), từ dài quá thì cắt (`wrapText`).
fn wrap_text(contents: &str, max_w: f64) -> Result<String> {
    let fits = |s: &str| text_width(s) <= max_w + 0.001;
    let mut lines: Vec<String> = vec![];
    for para in contents.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let combined = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if fits(&combined) {
                line = combined;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let mut rest: Vec<char> = word.chars().collect();
            while !rest.is_empty() && !fits(&rest.iter().collect::<String>()) {
                let (mut lo, mut hi, mut best) = (1usize, rest.len(), 0usize);
                while lo <= hi {
                    let mid = (lo + hi) / 2;
                    if fits(&rest.iter().take(mid).collect::<String>()) {
                        best = mid;
                        lo = mid + 1;
                    } else {
                        hi = mid - 1;
                    }
                }
                if best == 0 {
                    return Err(other("Không đủ chỗ ghi chú A5 ở cỡ chữ hiện tại."));
                }
                lines.push(rest.iter().take(best).collect());
                rest = rest.get(best..).map(<[char]>::to_vec).unwrap_or_default();
            }
            line = rest.into_iter().collect();
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

/// `a5NotePlacement` (y hướng xuống): góc trên trái của khối ghi chú `w` × `h` trong dải trống quanh
/// bài `art` trên tờ `rect`, không chạm PON (`obstacles`), ưu tiên `prefer`; `side_only`: chỉ dải
/// trái/phải; `left_only`: chỉ dải trái (SMALL8).
#[allow(clippy::too_many_arguments)]
pub(super) fn a5_note_placement(
    rect: Rect,
    art: Rect,
    w: f64,
    h: f64,
    obstacles: &[Rect],
    prefer: Point,
    side_only: bool,
    left_only: bool,
) -> Option<Point> {
    let (inset, clear, pc) = (MM, 2.0 * MM, 0.5 * MM);
    let (l, t, r, b) = (rect.x0 + inset, rect.y0 + inset, rect.x1 - inset, rect.y1 - inset);
    let mut zones = vec![];
    if !side_only {
        zones.push([l, t, r, b.min(art.y0 - clear)]);
        zones.push([l, t.max(art.y1 + clear), r, b]);
    }
    zones.push([l, t, r.min(art.x0 - clear), b]);
    if !left_only {
        zones.push([l.max(art.x1 + clear), t, r, b]);
    }
    for [zl, zt, zr, zb] in zones {
        if zr - zl < w - 0.001 || zb - zt < h - 0.001 {
            continue;
        }
        let mut xs = vec![zl.max((zr - w).min(prefer.x))];
        let mut ys = vec![(zb - h).min(zt.max(prefer.y))];
        if !left_only {
            xs.extend([zl, zr - w]);
            ys.extend([zt, zb - h]);
        }
        for o in obstacles {
            if o.x1 < zl - pc || o.x0 > zr + pc || o.y1 < zt - pc || o.y0 > zb + pc {
                continue;
            }
            xs.extend([o.x1 + pc, o.x0 - pc - w]);
            ys.extend([o.y1 + pc, o.y0 - pc - h]);
        }
        if left_only {
            xs.extend([zl, zr - w]);
            ys.extend([zb - h, zt]);
        }
        for &y in &ys {
            for &x in &xs {
                if x < zl - 0.001 || x + w > zr + 0.001 || y < zt - 0.001 || y + h > zb + 0.001 {
                    continue;
                }
                let clear_of = |o: &Rect| !(x < o.x1 + pc && x + w > o.x0 - pc && y + h > o.y0 - pc && y < o.y1 + pc);
                if obstacles.iter().all(clear_of) {
                    return Some(Point::new(x, y));
                }
            }
        }
    }
    None
}

/// Ghi chú tờ A5 (`addNoteA5`): đặt trọn trong dải trống quanh bài, tránh PON; lỗi thì dừng lệnh.
fn note_a5(d: &mut Document, text: &str, red: bool, face: &Face, rect: Rect, pages: &[NodeId], pons: &[Line]) -> Result<Option<Node>> {
    if text.is_empty() {
        return Ok(None);
    }
    let mut art: Option<Rect> = None;
    for p in face_pages(face) {
        let id = page(pages, p).ok_or_else(|| other("Thiếu trang để đo vùng trống ghi chú."))?;
        if let Some(b) = d.node(id).and_then(vis).filter(finite) {
            art = Some(art.map_or(b, |a| a.union(b)));
        }
    }
    let art = art.ok_or_else(|| other("Không đo được vùng trống ghi chú A5."))?;
    // PON danh định (cả hai layer), nở thêm nửa nét, chạm tờ.
    let obstacles: Vec<Rect> = pons
        .iter()
        .map(|l| {
            let hw = l.width / 2.0;
            Rect::new(l.a.x.min(l.b.x) - hw, l.a.y.min(l.b.y) - hw, l.a.x.max(l.b.x) + hw, l.a.y.max(l.b.y) + hw)
        })
        .filter(|o| o.x1 >= rect.x0 && o.x0 <= rect.x1 && o.y1 >= rect.y0 && o.y0 <= rect.y1)
        .collect();
    let small8 = face.kind == Kind::Small8;
    let anchor = match face.kind {
        Kind::Small4 => Point::new(rect.x0 + 6.637 * MM, rect.y0 + 5.342 * MM),
        Kind::Small16 | Kind::SmallAb => Point::new(rect.x0 + 12.968 * MM, rect.y0 + 17.202 * MM),
        _ => Point::new(rect.x0 + 8.0 * MM, rect.y0 + 8.0 * MM),
    };
    let attempt = |d: &mut Document, content: &str, vertical: bool, side_only: bool| -> Option<(Node, Rect, Point)> {
        let mut n = note_node(d, content, red, NOTE_SIZE);
        if vertical {
            turn(&mut n, 90.0);
        }
        let b = node_box(&n)?;
        shift(&mut n, anchor - Point::new(b.x0, b.y0));
        let block = node_box(&n)?;
        let prefer = if small8 { Point::new(rect.x0 + 9.0 * MM, rect.y1 - 15.0 * MM - block.height()) } else { Point::new(block.x0, block.y0) };
        let at = a5_note_placement(rect, art, block.width(), block.height(), &obstacles, prefer, side_only, small8)?;
        Some((n, block, at))
    };
    let mut found = attempt(d, text, small8, small8);
    if found.is_none() {
        found = attempt(d, text, true, true);
    }
    if found.is_none() && !small8 {
        let wrapped = wrap_text(text, rect.width() - 2.0 * MM)?;
        found = attempt(d, &wrapped, false, false);
    }
    if found.is_none() {
        let wrapped = wrap_text(text, rect.height() - (if small8 { 16.0 } else { 2.0 }) * MM)?;
        found = attempt(d, &wrapped, true, true);
    }
    let (mut n, block, at) = found.ok_or_else(|| other("Ghi chú A5 quá dài để nằm trong vùng trống của tờ. Hãy rút ngắn ghi chú."))?;
    shift(&mut n, at - Point::new(block.x0, block.y0));
    Ok(Some(n))
}

/// Đưa các ghi chú lên trên cùng layer `art`.
fn insert_notes(d: &mut Document, art: NodeId, notes: Vec<Node>) -> Result<usize> {
    let count = notes.len();
    for n in notes {
        d.insert(Some(art), usize::MAX, n)?;
    }
    Ok(count)
}

// ---------------------------------------------------------------- Đóng ghim giữa: đặt trang

/// Hai trang cạnh nhau (trái `a`, phải `b`) gom thành một group (`buildPairGroup`).
fn pair_group(d: &mut Document, pages: &[NodeId], a: i64, b: i64, pw: f64) -> Result<Option<NodeId>> {
    let (Some(l), Some(r)) = (page(pages, a), page(pages, b)) else { return Ok(None) };
    let (Some(bl), Some(br)) = (box_of(d, l), box_of(d, r)) else { return Ok(None) };
    apply(d, r, Affine::translate(Point::new(bl.center().x + pw, bl.center().y) - br.center()));
    let gid = d.alloc_id();
    Ok(Some(d.wrap(&[l, r], |children| {
        let mut g = Node::group(gid, children);
        g.name = Some(format!("Trang {a} + {b}"));
        g
    })?))
}

/// TT4 / SMALL4 (`placeFace4`): hai cặp xoay ±90°, bóp dọc theo số tờ.
fn place_face4(d: &mut Document, face: &Face, g: &Grid, pages: &[NodeId], pw: f64, ph: f64) -> Result<()> {
    let row = |i: usize| face.row.get(i).copied().unwrap_or(0);
    let acy = if face.kind == Kind::Small4 { g.grid_top + pw } else { g.cy };
    let left = pair_group(d, pages, row(0), row(1), pw)?;
    let right = pair_group(d, pages, row(2), row(3), pw)?;
    if let Some(gl) = left {
        rotate(d, gl, 90.0);
    }
    if let Some(gr) = right {
        rotate(d, gr, -90.0);
    }
    let cum_w = ph;
    let gap = if face.kind == Kind::Small4 { 5.0 * MM } else { 7.6 * MM };
    let edge = g.cx - (cum_w * 2.0 + gap) / 2.0;
    if let Some(gl) = left {
        center(d, gl, Point::new(edge + cum_w / 2.0, acy));
    }
    if let Some(gr) = right {
        center(d, gr, Point::new(edge + cum_w + gap + cum_w / 2.0, acy));
    }
    let reduce = face.sheet as f64 * BOP_PER_SHEET;
    if reduce > 0.0 {
        let sy = (2.0 * pw - reduce) / (2.0 * pw);
        for gid in [left, right].into_iter().flatten() {
            squeeze(d, &[gid], 1.0, sy);
        }
    }
    Ok(())
}

/// TT8 / AB / SMALL8 (`placeFace8`): hàng trên xoay 180°, bóp ngang từng cụm hai trang.
fn place_face8(d: &mut Document, face: &Face, g: &Grid, pages: &[NodeId], art: NodeId, pw: f64, ph: f64) -> Result<()> {
    let gutter = 8.0 * MM;
    let edge = g.cx - (pw * 4.0 + gutter) / 2.0;
    let col_x = |c: usize| edge + c as f64 * pw + (if c >= 2 { gutter } else { 0.0 }) + pw / 2.0;
    let trim_h = if face.kind == Kind::Small8 { g.trim_h.unwrap_or(ph) } else { ph };
    let at = |list: &[i64], c: usize| list.get(c).and_then(|p| page(pages, *p));
    for c in 0..4 {
        if let Some(id) = at(&face.top, c) {
            rotate(d, id, 180.0);
            center(d, id, Point::new(col_x(c), g.grid_top + ph / 2.0));
        }
        if let Some(id) = at(&face.bottom, c) {
            center(d, id, Point::new(col_x(c), g.grid_top + 2.0 * trim_h - ph / 2.0));
        }
    }
    let reduce = face.sheet as f64 * BOP_PER_SHEET;
    if reduce <= 0.0 {
        return Ok(());
    }
    let sx = (2.0 * pw - reduce) / (2.0 * pw);
    for [c0, c1] in [[0usize, 1], [2, 3]] {
        let members: Vec<NodeId> = [at(&face.top, c0), at(&face.top, c1), at(&face.bottom, c0), at(&face.bottom, c1)].into_iter().flatten().collect();
        squeeze(d, &members, sx, 1.0);
        // Như `ungroupInto`: cụm về đáy layer, hàng dưới nằm trên hàng trên ở đường nối.
        for id in members.iter().rev() {
            d.move_node(*id, Some(art), 0)?;
        }
    }
    Ok(())
}

/// Nửa 8 trang của tờ A5 16 trang (`placeSmallPanel`): lưới 4 × 2 rồi xoay cả khối `angle` độ.
#[allow(clippy::too_many_arguments)]
fn place_small_panel(d: &mut Document, half: Option<&Half>, c: Point, angle: f64, sheet: i64, pages: &[NodeId], pw: f64, ph: f64) -> Result<()> {
    let Some(half) = half else { return Ok(()) };
    let row_gap = 15.0 * MM;
    let (raw_w, raw_h) = (pw * 4.0 + row_gap, ph * 2.0);
    let mut members: Vec<Option<NodeId>> = vec![None; 8];
    for (r, list) in [&half.top, &half.bottom].into_iter().enumerate() {
        for col in 0..4 {
            let Some(id) = list.get(col).and_then(|p| page(pages, *p)) else { continue };
            if r == 0 {
                rotate(d, id, 180.0);
            }
            let x = c.x - raw_w / 2.0 + col as f64 * pw + (if col >= 2 { row_gap } else { 0.0 }) + pw / 2.0;
            let y = c.y - raw_h / 2.0 + r as f64 * ph + ph / 2.0;
            center(d, id, Point::new(x, y));
            if let Some(m) = members.get_mut(r * 4 + col) {
                *m = Some(id);
            }
        }
    }
    let ids: Vec<NodeId> = members.iter().flatten().copied().collect();
    if ids.len() != 8 {
        return Err(other(format!("Khối ruột A5 thiếu trang để dàn: {}/8.", ids.len())));
    }
    let gid = d.alloc_id();
    let panel = d
        .wrap(&ids, |children| {
            let mut g = Node::group(gid, children);
            g.name = Some("Khối ruột A5".into());
            g
        })
        .map_err(|e| other(format!("Không tạo được khối dàn ruột A5: {e}")))?;
    rotate(d, panel, angle);
    center(d, panel, c);
    if sheet <= 0 {
        return Ok(());
    }
    // Bóp riêng từng khối hai cột quanh tâm của nó: rãnh 15 mm vẫn trống và rộng thêm đúng mức bóp.
    let reduce = sheet as f64 * BOP_PER_SHEET;
    if reduce >= 2.0 * pw {
        return Err(other("Mức bóp ruột A5 lớn hơn khối trang gốc."));
    }
    let sy = (2.0 * pw - reduce) / (2.0 * pw);
    for block in [[0usize, 1, 4, 5], [2, 3, 6, 7]] {
        let part: Vec<NodeId> = block.iter().filter_map(|i| members.get(*i).copied().flatten()).collect();
        if part.is_empty() {
            return Err(other("Không bóp được khối ruột A5: Không tạo được khối bóp ruột A5."));
        }
        squeeze(d, &part, 1.0, sy);
    }
    Ok(())
}

/// SMALL16 / SMALLAB (`placeSmallFace16`): hai nửa 8 trang xoay +90° và −90°.
fn place_small_face16(d: &mut Document, face: &Face, g: &Grid, pages: &[NodeId], pw: f64, ph: f64) -> Result<()> {
    let panel_h = pw * 4.0 + 15.0 * MM;
    let acy = g.grid_top + panel_h / 2.0;
    let panel_w = ph * 2.0;
    let gutter = 6.0 * MM;
    let left_cx = g.cx - (panel_w * 2.0 + gutter) / 2.0 + panel_w / 2.0;
    let right_cx = left_cx + panel_w + gutter;
    place_small_panel(d, face.panels.first(), Point::new(left_cx, acy), 90.0, face.sheet, pages, pw, ph)?;
    place_small_panel(d, face.panels.get(1), Point::new(right_cx, acy), -90.0, face.sheet, pages, pw, ph)
}

// ---------------------------------------------------------------- dancard.saddleStitch

/// Tên artboard của một mặt Đóng ghim giữa.
fn saddle_name(f: &Face) -> String {
    let n = f.sheet;
    if n <= 0 {
        return "Bìa".into();
    }
    match f.kind {
        Kind::Tt4 => format!("Ruột{n} TT4"),
        Kind::Tt8 => format!("Ruột{n} TT8"),
        Kind::Small4 => format!("Ruột{n} TT4 A5"),
        Kind::Small8 => format!("Ruột{n} TT8 A5"),
        Kind::Small16 | Kind::Tt16 => format!("Ruột{n} TT16 A5"),
        Kind::Ab | Kind::SmallAb => format!("Ruột{n}-{}", if f.side_b { "B" } else { "A" }),
    }
}

fn saddle_stitch(s: &mut Session, p: &Value) -> Result<Value> {
    let (typed_w, typed_h) = page_size_mm(p)?;
    let cover = cover_flag(p);
    let notes = Notes::parse(p);
    let small = is_a5(typed_w, typed_h);
    // A5 nhập nằm ngang: lưới luôn dùng trang đứng, trang nguồn xoay +90° sau khi co.
    let landscape = small && typed_w > typed_h;
    let (page_w, page_h) = if landscape { (typed_h, typed_w) } else { (typed_w, typed_h) };
    let (src_w, src_h) = (typed_w * MM, typed_h * MM);
    let (pw, ph) = (page_w * MM, page_h * MM);

    let roots = roots_in_paint_order(s)?;
    let layer_hint = active_layer(s);
    let st = s.doc()?;
    // Bỏ các object nằm trong layer PON cũ.
    let ids: Vec<NodeId> = roots.into_iter().filter(|id| !in_layer_named(&st.doc, *id, &[SIG_PAPER_LAYER, SIG_CUT_LAYER])).collect();
    if ids.is_empty() {
        return Err(other("Chưa chọn trang ruột."));
    }
    let n = ids.len();
    if !n.is_multiple_of(4) {
        return Err(other(format!("Số trang phải là BỘI SỐ CỦA 4.\nĐang chọn: {n}")));
    }
    let n_pages = i64::try_from(n).map_err(|_| other("Quá nhiều trang."))?;
    let (order, start) = reading_order(&st.doc, &ids)?;
    let faces = saddle_plan(n_pages, cover, small);
    // Khổ giấy (bài quá khổ báo lỗi ở đây, trước khi sửa gì), rồi vị trí các tờ.
    let papers = faces.iter().map(|f| paper_for(f.kind, pw, ph)).collect::<std::result::Result<Vec<_>, String>>().map_err(other)?;
    let main_w = papers.iter().map(|p| p.0).fold(0.0, f64::max);
    let solo = |k: Kind| matches!(k, Kind::Tt4 | Kind::Tt8 | Kind::Small4 | Kind::Small8 | Kind::Small16);
    let mut rows = vec![];
    let mut k = 0;
    while let Some(f) = faces.get(k) {
        if solo(f.kind) || k + 1 >= faces.len() {
            rows.push((k, None));
            k += 1;
        } else {
            rows.push((k, Some(k + 1)));
            k += 2;
        }
    }
    let (rects, _) = place_sheets(&papers, &rows, start, main_w, false).map_err(other)?;
    let names: Vec<String> = faces.iter().map(saddle_name).collect();
    let inner_pages = if cover { n_pages - 4 } else { n_pages };
    let (cut_count, clamps, note_count, art) = s.edit("Dàn CTL Offset (DanCard)", |d, sel| {
        let art = artwork_layer(d, layer_hint, &[SIG_PAPER_LAYER, SIG_CUT_LAYER])?;
        remove_layers(d, &[SIG_PAPER_LAYER, SIG_CUT_LAYER])?;
        // Trang: làm phẳng tại khung thật, co đúng khổ nhập (không đều), A5 nằm ngang thì dựng đứng.
        let mut pages = Vec::with_capacity(order.len());
        for id in &order {
            let frame = box_of(d, *id).unwrap_or(Rect::ZERO);
            let flat = flatten_into(d, *id, frame, art)?;
            if let Some(b) = box_of(d, flat).filter(|b| b.width() > 0.0 && b.height() > 0.0) {
                apply(d, flat, scale_about(src_w / b.width(), src_h / b.height(), b.center()));
            }
            if landscape {
                rotate(d, flat, 90.0);
            }
            pages.push(flat);
        }
        // PON danh định, vẽ trước khi đặt bài: PON cắt trên cùng, PON giấy thứ hai, khóa cả hai.
        let (mut cut, mut paper, mut clamps) = (vec![], vec![], 0);
        for (f, r) in faces.iter().zip(&rects) {
            let (lines, c) = saddle_cut_pon(f.kind, pw, ph, *r).map_err(other)?;
            cut.extend(lines);
            clamps += c;
        }
        for (f, r) in faces.iter().zip(&rects) {
            paper.extend(paper_pon(*r, form(f.kind).large));
        }
        add_pon_layer(d, SIG_PAPER_LAYER, &paper)?;
        add_pon_layer(d, SIG_CUT_LAYER, &cut)?;
        // Đặt bài và bóp ruột; PON không bao giờ bị dời.
        for (f, r) in faces.iter().zip(&rects) {
            let g = grid_for(f.kind, pw, ph, *r).map_err(other)?;
            match f.kind {
                Kind::Tt4 | Kind::Small4 => place_face4(d, f, &g, &pages, pw, ph)?,
                Kind::Small16 | Kind::SmallAb => place_small_face16(d, f, &g, &pages, pw, ph)?,
                _ => place_face8(d, f, &g, &pages, art, pw, ph)?,
            }
        }
        // Ghi chú: mặt A mỗi tờ; bìa chỉ khi có chữ.
        let mut made = vec![];
        if notes.on {
            let pons: Vec<Line> = paper.iter().chain(&cut).copied().collect();
            for (f, r) in faces.iter().zip(&rects) {
                if f.side_b {
                    continue;
                }
                let (text, red) = if f.sheet <= 0 { (notes.cover.clone(), false) } else { notes.inner(f) };
                let node = if f.kind.a5() {
                    note_a5(d, &text, red, f, *r, &pages, &pons)?
                } else if f.kind == Kind::Tt4 {
                    note_vertical(d, &text, red, *r)
                } else {
                    note_horizontal(d, &text, red, *r)
                };
                made.extend(node);
            }
        }
        let note_count = insert_notes(d, art, made)?;
        set_artboards(d, &rects, &names);
        sel.clear();
        Ok((cut.len(), clamps, note_count, art))
    })?;
    if let Some(st) = s.active_mut() {
        st.active_layer = Some(art);
    }
    let mut message = format!("OK: Đã dàn CTL Offset; đã vẽ {cut_count} nét PON cắt.");
    if clamps > 0 {
        message.push_str(&format!(" Lưu ý: {clamps} mốc PON bị kéo về mép artboard vì lưới danh định của form này lòi ra ngoài khổ giấy."));
    }
    let mut summary = format!("Đã dàn {} artboard.\n", faces.len());
    if small {
        summary.push_str("A5: artboard/PON lấy theo kích thước trang đã nhập; ruột được bóp dần, PON giữ lưới gốc.\n");
    } else {
        summary.push_str(&format!("Dư {} -> tự trở trước (4 và/hoặc 8), rồi các tờ AB; PON giữ lưới gốc.\n", inner_pages.rem_euclid(16)));
    }
    if faces.iter().any(|f| f.kind == Kind::Tt4) {
        summary.push_str("TT4 dùng PON tự vẽ theo kích thước đã nhập.\n");
    }
    summary.push_str("PON tự vẽ nằm trên cùng. ");
    summary.push_str(if notes.on { "Ghi chú đã xử lý." } else { "Đã bỏ qua ghi chú." });
    Ok(json!({
        "message": format!("{message}\n{summary}"),
        "summary": summary,
        "faces": faces.len(),
        "cutPon": cut_count,
        "notes": note_count,
        "artboards": names,
        "sizes": size_plan(&faces, &rects),
    }))
}

// ---------------------------------------------------------------- dancard.perfectBind

/// Khổ giấy của một mặt Keo gáy và có phải tờ nhỏ không.
fn keo_paper(f: &Face, small: bool, tt8_fits: bool) -> ((f64, f64), bool) {
    match f.kind {
        Kind::Ab => (SHEET_AB, false),
        Kind::Tt4 if small => (SHEET_COVER_A5, true),
        Kind::Tt4 => (SHEET_SMALL, true),
        _ if f.small_self_turn && tt8_fits => (SHEET_SMALL, true),
        _ => (SHEET_MAIN, false),
    }
}

fn perfect_bind(s: &mut Session, p: &Value) -> Result<Value> {
    let (typed_w, typed_h) = page_size_mm(p)?;
    let notes = Notes::parse(p);
    let small = is_a5(typed_w, typed_h);
    let landscape = small && typed_w > typed_h;
    let (page_w, page_h) = if landscape { (typed_h, typed_w) } else { (typed_w, typed_h) };
    if !small && (page_w > 212.0 + 1e-6 || page_h > 300.0 + 1e-6) {
        return Err(other("Khung A4 tối đa 21,2 × 30 cm (rộng × cao)."));
    }
    let gap4 = (if small { 5.0 } else { 6.0 }) * MM;
    let roots = roots_in_paint_order(s)?;
    let layer_hint = active_layer(s);
    if roots.is_empty() {
        return Err(other("Chưa chọn trang ruột."));
    }
    let n = roots.len();
    if !n.is_multiple_of(4) {
        return Err(other(format!("Số trang phải là BỘI SỐ CỦA 4.\nĐang chọn: {n}")));
    }
    let n_pages = i64::try_from(n).map_err(|_| other("Quá nhiều trang."))?;
    let faces = keo_plan(n_pages, small).map_err(other)?;
    let st = s.doc()?;
    let (order, start) = reading_order(&st.doc, &roots)?;
    // Đo trước mọi trang, khi chưa sửa gì.
    let mut sources = Vec::with_capacity(order.len());
    for (i, id) in order.iter().enumerate() {
        let b = box_of(&st.doc, *id).unwrap_or(Rect::ZERO);
        keo_page_scales(b.width(), b.height(), typed_w * MM, typed_h * MM).map_err(|e| other(format!("Trang ruột {}: {e}", i + 1)))?;
        sources.push(b);
    }
    // Khổ giấy, khung trang của từng mặt, kiểm tra vừa tờ.
    let (pw0, ph0) = (page_w * MM, page_h * MM);
    let tt8_fits = pw0 * 4.0 + 4.0 * MM + 1.0 <= SHEET_SMALL.0 && ph0 * 2.0 + 1.0 <= SHEET_SMALL.1;
    let mut papers = vec![];
    let mut frames = vec![];
    let mut smalls = vec![];
    let mut page_frames: Vec<Option<(f64, f64)>> = vec![None; n];
    for f in &faces {
        let (paper, small_paper) = keo_paper(f, small, tt8_fits);
        let (fw, fh) = keo_frame(page_w, page_h, small, small_paper);
        for pg in face_pages(f) {
            if let Some(slot) = usize::try_from(pg - 1).ok().and_then(|i| page_frames.get_mut(i)) {
                *slot = Some((fw, fh));
            }
        }
        let tt4_extra = if small { 4.0 * MM + 1.0 } else { 0.0 };
        if f.kind == Kind::Tt4 && (2.0 * fh * MM + gap4 > paper.0 + 0.01 || 2.0 * fw * MM + tt4_extra > paper.1 + 0.01) {
            return Err(other("Khổ trang không vừa tờ tự trở 4 trang; hãy giảm khổ trang."));
        }
        if !f.blocks.is_empty() && (fh * 4.0 * MM + 4.0 * MM + 1.0 > paper.0 || fw * 4.0 * MM + KEO_TOP_GAP + 0.5 > paper.1) {
            return Err(other(if f.kind == Kind::Ab {
                "Khổ trang A5 không vừa tờ AB 85,8 × 62,5 cm; hãy giảm khổ trang."
            } else {
                "Khổ trang A5 không vừa tờ tự trở 65 × 86; hãy giảm khổ trang."
            }));
        }
        papers.push(paper);
        frames.push((fw * MM, fh * MM));
        smalls.push(small_paper);
    }
    let mut rows = vec![];
    let mut k = 0;
    while let Some(f) = faces.get(k) {
        if f.kind == Kind::Ab && faces.get(k + 1).is_some_and(|b| b.kind == Kind::Ab) {
            rows.push((k, Some(k + 1)));
            k += 2;
        } else {
            rows.push((k, None));
            k += 1;
        }
    }
    let (rects, columns) = place_sheets(&papers, &rows, start, SHEET_MAIN.0, true).map_err(other)?;
    let names: Vec<String> = faces.iter().map(|f| f.label.clone()).collect();
    let (cut_count, note_count, art) = s.edit("Dàn CTL Keo gáy (DanCard)", |d, sel| {
        let art = artwork_layer(d, layer_hint, &[KEO_PAPER_LAYER, KEO_CUT_LAYER])?;
        // Trang: làm phẳng, co đúng khung của mặt theo trục tài liệu, kiểm tra lại, A5 nằm ngang dựng đứng.
        let mut pages = Vec::with_capacity(order.len());
        for (i, (id, frame)) in order.iter().zip(&sources).enumerate() {
            let flat = flatten_into(d, *id, *frame, art)?;
            let b = box_of(d, flat)
                .filter(|b| b.width() > 0.0 && b.height() > 0.0)
                .ok_or_else(|| other(format!("Không đo được kích thước trang ruột {}", i + 1)))?;
            let (fw, fh) = page_frames.get(i).copied().flatten().ok_or_else(|| other(format!("Thiếu trang ruột {}", i + 1)))?;
            let (tw, th) = if landscape { (fh * MM, fw * MM) } else { (fw * MM, fh * MM) };
            let (sx, sy) = keo_page_scales(b.width(), b.height(), tw, th).map_err(|e| other(format!("Trang ruột {}: {e}", i + 1)))?;
            apply(d, flat, scale_about(sx, sy, b.center()));
            let exact = box_of(d, flat).unwrap_or(Rect::ZERO);
            if (exact.width() - tw).abs() > 0.01 * MM || (exact.height() - th).abs() > 0.01 * MM {
                return Err(other(format!("Trang ruột {}: không khớp kích thước đã nhập.", i + 1)));
            }
            if landscape {
                rotate(d, flat, 90.0);
            }
            pages.push(flat);
        }
        remove_layers(d, &[KEO_PAPER_LAYER, KEO_CUT_LAYER])?;
        // Đặt bài, không bóp.
        for ((f, r), &(pw, ph)) in faces.iter().zip(&rects).zip(&frames) {
            let c = r.center();
            if !f.blocks.is_empty() {
                let (_, cells) = keo_packed_grid(f, pw, ph, *r, KEO_TOP_GAP);
                for cell in cells {
                    let id = page(&pages, cell.page).ok_or_else(|| other(format!("Thiếu trang ruột {}", cell.page)))?;
                    rotate(d, id, cell.angle);
                    center(d, id, cell.center);
                }
            } else if f.kind == Kind::Tt4 {
                // Tâm ô danh định quanh tâm tờ.
                for (i, pg) in f.row.iter().enumerate().take(4) {
                    let id = page(&pages, *pg).ok_or_else(|| other(format!("Thiếu trang ruột {pg}")))?;
                    let left = i < 2;
                    rotate(d, id, if left { 90.0 } else { -90.0 });
                    let x = c.x + (if left { -1.0 } else { 1.0 }) * (ph + gap4) / 2.0;
                    let y = c.y + (if i.is_multiple_of(2) { -1.0 } else { 1.0 }) * pw / 2.0;
                    center(d, id, Point::new(x, y));
                }
            } else {
                let cum_top = if f.small_self_turn { c.y - ph } else { r.y0 + KEO_TOP_GAP };
                for col in 0..4 {
                    let x = c.x - 2.0 * pw + (col as f64 + 0.5) * pw;
                    if let Some(id) = f.top.get(col).and_then(|pg| page(&pages, *pg)) {
                        rotate(d, id, 180.0);
                        center(d, id, Point::new(x, cum_top + ph / 2.0));
                    }
                    if let Some(id) = f.bottom.get(col).and_then(|pg| page(&pages, *pg)) {
                        center(d, id, Point::new(x, cum_top + ph * 1.5));
                    }
                }
            }
        }
        // PON từ cùng các số danh định, vẽ sau khi đặt bài; PON cắt trên cùng, khóa cả hai.
        let (mut paper, mut cut) = (vec![], vec![]);
        for (((f, r), &(pw, ph)), &small_paper) in faces.iter().zip(&rects).zip(&frames).zip(&smalls) {
            paper.extend(paper_pon(*r, !small_paper));
            cut.extend(keo_cut_pon(f, pw, ph, *r, gap4));
        }
        add_pon_layer(d, KEO_PAPER_LAYER, &paper)?;
        add_pon_layer(d, KEO_CUT_LAYER, &cut)?;
        // Ghi chú RUỘT n ở mặt A.
        let mut made = vec![];
        if notes.on {
            for (((f, r), &(pw, ph)), &small_paper) in faces.iter().zip(&rects).zip(&frames).zip(&smalls) {
                if f.side_b {
                    continue;
                }
                let (text, red) = notes.inner(f);
                let node = if small_paper { keo_note_small(d, &text, red, *r, small, pw, ph, gap4) } else { note_horizontal(d, &text, red, *r) };
                made.extend(node);
            }
        }
        let note_count = insert_notes(d, art, made)?;
        set_artboards(d, &rects, &names);
        sel.clear();
        Ok((cut.len(), note_count, art))
    })?;
    if let Some(st) = s.active_mut() {
        st.active_layer = Some(art);
    }
    let family =
        if small { "A5: 32 trang/tờ AB (2 tay 16), AB trước, tự trở sau." } else { "A4: 16 trang/tờ AB, AB trước, tự trở sau." };
    let message = format!("OK: Đã dàn CTL keo gáy trên {columns} cột canvas; đã vẽ {cut_count} nét PON cắt. {family}");
    let summary = format!(
        "Đã dàn keo gáy: {} artboard.\n{}PON tự động nằm trên cùng. {}",
        faces.len(),
        if small {
            "A5: 32 trang/tờ AB (2 tay 16); phần dư tự trở TT16/TT8/TT4.\n"
        } else {
            "A4: 16 trang/tờ AB; phần dư tự trở TT8/TT4.\n"
        },
        if notes.on { "Ghi chú RUỘT N đã điền (mặt A)." } else { "Đã bỏ qua ghi chú." }
    );
    Ok(json!({
        "message": format!("{message}\n{summary}"),
        "summary": summary,
        "faces": faces.len(),
        "cutPon": cut_count,
        "notes": note_count,
        "columns": columns,
        "artboards": names,
        "sizes": size_plan(&faces, &rects),
    }))
}

// ---------------------------------------------------------------- dancard.saveCtlPackage

#[cfg(target_arch = "wasm32")]
fn save_ctl_package(_: &mut Session, _: &Value) -> Result<Value> {
    Err(other("Bản web không lưu được AI + JPG + ZIP."))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_ctl_package(s: &mut Session, p: &Value) -> Result<Value> {
    package::save(s, p)
}

/// Lưu AI + JPG + ZIP (chỉ bản desktop: ghi file).
#[cfg(not(target_arch = "wasm32"))]
mod package {
    use std::path::Path;
    use std::sync::Arc;

    use serde_json::{Value, json};
    use vectorcraft_doc::{Artboard, Document, Node, NodeKind};
    use vectorcraft_geom::{Affine, Rect};

    use super::{other, scaling};
    use crate::cmd::fileio;
    use crate::cmd::*;
    use crate::{Result, Session};

    const LABEL: &str = "AI + JPG";
    const CHANGED: &str = " Tài liệu đã khác lúc dàn — hãy dàn lại rồi lưu AI + JPG.";
    const MISSING: &str = "Thiếu thông tin tờ cần lưu — hãy dàn lại rồi lưu AI + JPG.";
    /// Khe giữa các artboard của file tạm (pt).
    const SHEET_GAP: f64 = 20.0;

    /// Một artboard cần lưu.
    pub(super) struct Page {
        pub index: usize,
        pub rect: Rect,
        pub name: String,
    }

    /// Một bộ file: `<base>.ai`, `.jpg`, `.zip`.
    pub(super) struct Job {
        pub base: String,
        pub pages: Vec<Page>,
    }

    /// Ký tự cấm trong tên file: `\ / : * ? " < > |`.
    fn reserved(c: char) -> bool {
        matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }

    /// Đọc và kiểm tra các job (`jobs` của một khổ trong `sizes`) với tài liệu `d`.
    pub(super) fn parse_jobs(d: &Document, p: &Value, suffix: &str) -> Result<Vec<Job>> {
        let list = p.get("jobs").and_then(Value::as_array).filter(|l| l.len() <= 1000).ok_or_else(|| other(MISSING))?;
        let mut jobs: Vec<Job> = vec![];
        for j in list {
            let name =
                j.get("name").and_then(Value::as_str).filter(|n| !n.is_empty() && !n.contains(['=', '@', '|'])).ok_or_else(|| other(MISSING))?;
            let numbers = j.get("ab").and_then(Value::as_array).filter(|a| !a.is_empty() && a.len() <= 64).ok_or_else(|| other(MISSING))?;
            let size = |k: &str| j.get(k).and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0);
            let (Some(want_w), Some(want_h)) = (size("w"), size("h")) else { return Err(other(MISSING)) };
            let mut base: String = name.trim().chars().map(|c| if reserved(c) { '-' } else { c }).collect();
            if !suffix.is_empty() {
                base = format!("{base} {suffix}");
            }
            let mut pages = vec![];
            for v in numbers {
                let no = v.as_i64().or_else(|| v.as_f64().filter(|x| x.is_finite() && (x - x.round()).abs() < 1e-9).map(|x| x.round() as i64));
                let board = no.and_then(|n| usize::try_from(n).ok()).filter(|n| *n >= 1).and_then(|n| d.artboards.get(n - 1).map(|a| (n, a)));
                let Some((no, a)) = board else {
                    return Err(other(format!("Không có artboard số {v}.{CHANGED}")));
                };
                if (a.rect.width() - want_w).abs() > 1.0 || (a.rect.height() - want_h).abs() > 1.0 {
                    return Err(other(format!("Artboard {no} không còn đúng khổ của {base}.{CHANGED}")));
                }
                pages.push(Page { index: no - 1, rect: a.rect, name: a.name.clone() });
            }
            if base.trim().is_empty() || pages.is_empty() {
                return Err(other(MISSING));
            }
            if jobs.iter().any(|o| o.base.to_lowercase() == base.to_lowercase()) {
                return Err(other(format!("Hai tờ trùng tên {LABEL}: {base}.")));
            }
            jobs.push(Job { base, pages });
        }
        if jobs.is_empty() {
            return Err(other("Không có tờ nào để lưu."));
        }
        Ok(jobs)
    }

    fn unlock_all(n: &mut Node) {
        n.locked = false;
        if let Some(ch) = n.children_mut() {
            for c in ch {
                unlock_all(Arc::make_mut(c));
            }
        }
    }

    /// Bản sao một layer (đang hiện, được in) cho các tờ `pages`: mỗi tờ một group "CTL <tên
    /// artboard>" chứa các object cấp đầu chạm vào tờ (nguyên object, mở khóa), dời theo artboard đích.
    fn copy_layer(d: &mut Document, layer: &Node, pages: &[Page], targets: &[Rect], copied: &mut [bool], depth: usize) -> Option<Node> {
        let NodeKind::Layer { printable, template, .. } = &layer.kind else { return None };
        if !layer.visible || !*printable || *template || depth > 64 {
            return None;
        }
        let children = layer.children()?;
        let mut kept: Vec<Arc<Node>> = vec![];
        for (pi, (page, target)) in pages.iter().zip(targets).enumerate() {
            let by = target.origin() - page.rect.origin();
            let r = page.rect;
            let mut items = vec![];
            for c in children.iter() {
                if c.is_layer() || !c.visible || matches!(&c.kind, NodeKind::Group { children, .. } if children.is_empty()) {
                    continue;
                }
                let Some(b) = c.visual_bounds().or_else(|| c.geometric_bounds()) else { continue };
                if b.x1 < r.x0 || b.x0 > r.x1 || b.y1 < r.y0 || b.y0 > r.y1 {
                    continue;
                }
                let mut copy = d.reid(c);
                unlock_all(&mut copy);
                copy.transform(Affine::translate(by), scaling());
                items.push(Arc::new(copy));
            }
            if items.is_empty() {
                continue;
            }
            if let Some(f) = copied.get_mut(pi) {
                *f = true;
            }
            let mut g = Node::group(d.alloc_id(), items);
            g.name = Some(format!("CTL {}", if page.name.is_empty() { format!("Artboard {}", page.index + 1) } else { page.name.clone() }));
            kept.push(Arc::new(g));
        }
        for c in children.iter().filter(|c| c.is_layer()) {
            if let Some(sub) = copy_layer(d, c, pages, targets, copied, depth + 1) {
                kept.push(Arc::new(sub));
            }
        }
        let mut out = layer.clone();
        out.id = d.alloc_id();
        out.locked = false;
        out.visible = true;
        if let Some(ch) = out.children_mut() {
            *ch = kept;
        }
        Some(out)
    }

    /// Tài liệu tạm của một job: cùng hệ màu, mẫu màu, ảnh, symbol… của tài liệu nguồn; các layer
    /// cùng tên, cùng thứ tự; mỗi tờ một artboard (A trái, B phải cách 20 pt), bài giữ đúng vị trí so
    /// với góc trên trái artboard của nó.
    pub(super) fn sheet_document(src: &Document, title: &str, pages: &[Page]) -> std::result::Result<Document, String> {
        let max_w = pages.iter().map(|p| p.rect.width()).fold(1.0, f64::max);
        let max_h = pages.iter().map(|p| p.rect.height()).fold(1.0, f64::max);
        let targets: Vec<Rect> = (0..pages.len())
            .map(|i| {
                let x = i as f64 * (max_w + SHEET_GAP);
                Rect::new(x, 0.0, x + max_w, max_h)
            })
            .collect();
        let mut d = src.clone();
        d.title = title.to_string();
        d.text_threads.clear();
        d.guides.clear();
        d.views.clear();
        d.slices.clear();
        d.assets.clear();
        d.pattern_edit = None;
        d.mask_edit = None;
        d.last_view = None;
        d.layers_open = None;
        d.puppet = None;
        let mut copied = vec![false; pages.len()];
        let layers: Vec<Arc<Node>> = src.layers.iter().filter_map(|l| copy_layer(&mut d, l, pages, &targets, &mut copied, 0)).map(Arc::new).collect();
        if let Some((page, _)) = pages.iter().zip(&copied).find(|(_, c)| !**c) {
            return Err(format!("Artboard {} không có object để lưu.", page.index + 1));
        }
        d.layers = layers;
        d.artboards = pages
            .iter()
            .zip(&targets)
            .enumerate()
            .map(|(i, (p, r))| Artboard {
                id: u32::try_from(i + 1).unwrap_or(1),
                name: p.name.clone(),
                rect: *r,
                show_center_mark: false,
                show_cross_hairs: false,
            })
            .collect();
        d.prune_variable_bindings();
        Ok(d)
    }

    /// Ngày giờ DOS (giờ Việt Nam, UTC+7) cho các mục ZIP: (giờ, ngày).
    fn dos_now() -> (u16, u16) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let local = i64::try_from(secs).unwrap_or(0).saturating_add(7 * 3600);
        let (days, rem) = (local.div_euclid(86_400), local.rem_euclid(86_400));
        // Ngày dương lịch từ số ngày kể từ 1970-01-01 (civil_from_days).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        if !(1980..=2107).contains(&year) {
            return (0, (1 << 5) | 1);
        }
        let date = ((year - 1980) << 9) | (month << 5) | day;
        let time = ((rem / 3600) << 11) | (((rem % 3600) / 60) << 5) | ((rem % 60) / 2);
        (u16::try_from(time).unwrap_or(0), u16::try_from(date).unwrap_or(0))
    }

    /// ZIP các file `entries` (tên phẳng, UTF-8), nén deflate mức cao nhất.
    pub(super) fn zip_deflate(entries: &[(&str, &[u8])], (time, date): (u16, u16)) -> std::result::Result<Vec<u8>, String> {
        use std::io::Write as _;
        let too_big = || "ZIP quá lớn (tối đa 4 GB mỗi file).".to_string();
        let count = u16::try_from(entries.len()).map_err(|_| too_big())?;
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            enc.write_all(data).map_err(|e| e.to_string())?;
            let packed = enc.finish().map_err(|e| e.to_string())?;
            let mut crc = flate2::Crc::new();
            crc.update(data);
            let size = u32::try_from(data.len()).map_err(|_| too_big())?;
            let packed_size = u32::try_from(packed.len()).map_err(|_| too_big())?;
            let offset = u32::try_from(out.len()).map_err(|_| too_big())?;
            let name = name.as_bytes();
            let name_len = u16::try_from(name.len()).map_err(|_| "Tên file quá dài để nén.".to_string())?;
            // Phần chung: cờ (tên UTF-8), cách nén 8 (deflate), giờ, ngày, CRC, cỡ nén, cỡ thật, độ dài tên.
            let mut shared = Vec::with_capacity(26);
            for v in [1u16 << 11, 8, time, date] {
                shared.extend_from_slice(&v.to_le_bytes());
            }
            for v in [crc.sum(), packed_size, size] {
                shared.extend_from_slice(&v.to_le_bytes());
            }
            shared.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&shared);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&packed);
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&shared);
            // Độ dài extra và comment, số đĩa, thuộc tính trong và ngoài.
            central.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name);
        }
        let start = u32::try_from(out.len()).map_err(|_| too_big())?;
        let central_len = u32::try_from(central.len()).map_err(|_| too_big())?;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&central_len.to_le_bytes());
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        Ok(out)
    }

    /// Các mục của một ZIP (đọc thư mục trung tâm): (tên, cỡ thật).
    pub(super) fn zip_listing(zip: &[u8]) -> Option<Vec<(String, u64)>> {
        let u16_at = |at: usize| zip.get(at..at.checked_add(2)?).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |at: usize| zip.get(at..at.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let end = zip.len().checked_sub(22)?;
        if u32_at(end)? != 0x0605_4b50 {
            return None;
        }
        let count = usize::from(u16_at(end + 10)?);
        let mut at = usize::try_from(u32_at(end + 16)?).ok()?;
        let mut out = vec![];
        for _ in 0..count {
            if u32_at(at)? != 0x0201_4b50 {
                return None;
            }
            let size = u64::from(u32_at(at + 24)?);
            let name_len = usize::from(u16_at(at + 28)?);
            let extra = usize::from(u16_at(at + 30)?) + usize::from(u16_at(at + 32)?);
            let name = String::from_utf8(zip.get(at + 46..at + 46 + name_len)?.to_vec()).ok()?;
            out.push((name, size));
            at += 46 + name_len + extra;
        }
        Some(out)
    }

    fn exists(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// AI + JPG của một job → (bytes AI, bytes JPG). Lỗi JPG sau khi AI đã lưu: AI được giữ.
    fn export_job(src: &Document, job: &Job, ai: &Path, jpg: &Path) -> std::result::Result<(Vec<u8>, Vec<u8>), String> {
        let d = sheet_document(src, &job.base, &job.pages)?;
        // AI chỉnh sửa được, nén, nhúng file liên kết và profile màu; kèm phần PDF để máy khác
        // (Illustrator ở xưởng in) mở được bài.
        let ai_bytes =
            fileio::encode(&d, "ai", &json!({"compress": true, "includeLinked": true, "embedProfiles": true})).map_err(|e| format!("Lỗi: {e}"))?;
        fileio::write_new_file(&text(ai), &ai_bytes).map_err(|e| format!("Lỗi: {e}"))?;
        // Một ảnh của cả tài liệu (A + B và khe giữa), CMYK, 300 ppi, chất lượng cao nhất, baseline.
        let params = json!({"useArtboards": false, "colorModel": "cmyk", "quality": 100, "method": "baseline", "ppi": 300, "antiAlias": "type", "embedIcc": true});
        let jpg_bytes = match fileio::encode(&d, "jpg", &params) {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => return Err("AI đã lưu; xuất JPG lỗi: Chưa tạo được JPG.".into()),
            Err(e) => return Err(format!("AI đã lưu; xuất JPG lỗi: {e}")),
        };
        fileio::write_new_file(&text(jpg), &jpg_bytes).map_err(|e| format!("AI đã lưu; xuất JPG lỗi: {e}"))?;
        Ok((ai_bytes, jpg_bytes))
    }

    /// ZIP hai file của một job, kiểm tra lại file đã ghi; lỗi thì xoá ZIP, giữ AI/JPG.
    fn pack(job: &Job, zip: &Path, ai: &[u8], jpg: &[u8]) -> std::result::Result<(), String> {
        if exists(zip) {
            return Err("ZIP đã có sẵn; không ghi đè.".into());
        }
        if ai.is_empty() || jpg.is_empty() {
            return Err("Thiếu file AI/JPG hoặc file rỗng.".into());
        }
        let (ai_name, jpg_name) = (format!("{}.ai", job.base), format!("{}.jpg", job.base));
        let bytes = zip_deflate(&[(ai_name.as_str(), ai), (jpg_name.as_str(), jpg)], dos_now())?;
        let path = text(zip);
        fileio::write_new_file(&path, &bytes).map_err(|e| e.to_string())?;
        let check = fileio::read_file(&path).ok().and_then(|b| zip_listing(&b));
        let ok = match &check {
            Some(list) if list.len() == 2 => {
                let has = |name: &str, size: usize| list.iter().any(|(n, s)| n == name && usize::try_from(*s).ok() == Some(size));
                if has(&ai_name, ai.len()) && has(&jpg_name, jpg.len()) {
                    Ok(())
                } else {
                    Err("ZIP lỗi: mục nén không khớp file.".to_string())
                }
            }
            _ => Err("ZIP lỗi: số mục nén không khớp.".to_string()),
        };
        if ok.is_err() {
            // Bỏ ZIP hỏng (không xoá được thì thôi); AI và JPG vẫn còn.
            let _ = std::fs::remove_file(zip);
        }
        ok
    }

    pub(super) fn save(s: &mut Session, p: &Value) -> Result<Value> {
        let suffix = str_param(p, "suffix").unwrap_or("").trim().to_string();
        if suffix.chars().any(|c| reserved(c) || c.is_control()) {
            return Err(other(r#"Nội dung tên file không được chứa \ / : * ? " < > | hoặc xuống dòng."#));
        }
        if suffix.ends_with(['.', ' ']) {
            return Err(other("Tên file không được kết thúc bằng dấu chấm."));
        }
        let st = s.doc().map_err(|_| other("Chưa mở tài liệu nào."))?;
        let src = st.doc.clone();
        let count = src.artboards.len();
        let total = super::super::int_param(p, "total");
        if total != i64::try_from(count).ok() {
            let said = total.map_or_else(|| "?".to_string(), |t| t.to_string());
            return Err(other(format!("Tài liệu đang mở có {count} artboard, lúc dàn là {said}.{CHANGED}")));
        }
        let jobs = parse_jobs(&src, p, &suffix)?;
        let folder =
            str_param(p, "folder").map(str::trim).filter(|f| !f.is_empty()).ok_or_else(|| other("Chưa chọn thư mục lưu AI + JPG - đã huỷ."))?;
        let dir = Path::new(folder);
        if !dir.is_absolute() || !std::fs::metadata(dir).is_ok_and(|m| m.is_dir()) {
            return Err(other(format!("Không tìm thấy thư mục lưu: {folder}")));
        }
        // Không ghi gì trước khi mọi tên đều hợp lệ; không bao giờ ghi đè.
        for job in &jobs {
            if exists(&dir.join(format!("{}.ai", job.base))) {
                return Err(other(format!("File đã tồn tại: {}.ai.", job.base)));
            }
            if format!("{folder}/{}", job.base).chars().count() > 245 {
                return Err(other("Tên/đường dẫn quá dài; hãy rút ngắn nội dung hoặc chọn thư mục ngắn hơn."));
            }
            if exists(&dir.join(format!("{}.jpg", job.base))) || exists(&dir.join(format!("{}.zip", job.base))) {
                return Err(other(format!("File đã tồn tại: {} (.jpg hoặc .zip).", job.base)));
            }
        }
        let mut saved = vec![];
        let mut errors = vec![];
        let mut files = vec![];
        let mut zip_errors = vec![];
        let mut zipped = 0usize;
        for job in &jobs {
            let (ai, jpg, zip) = (dir.join(format!("{}.ai", job.base)), dir.join(format!("{}.jpg", job.base)), dir.join(format!("{}.zip", job.base)));
            match export_job(&src, job, &ai, &jpg) {
                Ok((ai_bytes, jpg_bytes)) => {
                    saved.push(format!("{}.ai", job.base));
                    let packed = pack(job, &zip, &ai_bytes, &jpg_bytes);
                    match &packed {
                        Ok(()) => zipped += 1,
                        Err(e) => zip_errors.push(format!("{}: {e}", job.base)),
                    }
                    let zip_path = if packed.is_ok() { json!(text(&zip)) } else { Value::Null };
                    files.push(json!({"baseName": job.base, "ai": text(&ai), "jpg": text(&jpg), "zip": zip_path}));
                }
                Err(e) => errors.push(format!("{}.ai: {e}", job.base)),
            }
        }
        if saved.is_empty() {
            let more = if errors.is_empty() { String::new() } else { format!(" {}", errors.join(" | ")) };
            return Err(other(format!("Không lưu được {LABEL} nào.{more}")));
        }
        let mut message = format!("OK: Đã lưu {} {LABEL} vào {folder}: {}.", saved.len(), saved.join(", "));
        if !errors.is_empty() {
            message.push_str(&format!(" Lỗi {} file: {}", errors.len(), errors.join(" | ")));
        }
        message.push_str(&format!(" Đã nén {zipped} ZIP."));
        if !zip_errors.is_empty() {
            message.push_str(&format!(" AI/JPG được giữ lại. Lỗi: {}", zip_errors.join(" | ")));
        }
        Ok(json!({"message": message, "folder": folder, "files": files, "errors": errors, "zipErrors": zip_errors}))
    }
}

#[cfg(test)]
#[path = "dancard_ctl_tests.rs"]
mod tests;
