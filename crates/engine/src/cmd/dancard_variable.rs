//! DanCard — tab Variable: nhập file dữ liệu CSV/TXT thành Variables của tài liệu.
//!
//! Tab Variable trên Illustrator chạy công cụ Variable Importer: mỗi cột của file thành một biến,
//! mỗi dòng thành một bản ghi (data set), object đặt tên trùng tên biến được gắn tự động rồi bản
//! ghi 1 hiện lên mẫu. Ở đây viết lại theo đặc tả (không chép mã gốc) trên mô hình Variables có sẵn
//! của VectorCraft (`variablecmds.rs`), cả lần nhập là một bước hoàn tác:
//! - đọc UTF-8 (có hoặc không BOM), UTF-16 theo BOM; file không phải UTF-8 thì đọc theo
//!   Windows-1258 (CSV của Excel trên Windows tiếng Việt); rồi chuẩn hoá NFC như DanCard v1.9.1
//!   (dấu thanh gõ rời ghép lại thành chữ dựng sẵn) cho cả tên cột lẫn nội dung ô;
//! - dấu phân cách: .txt là Tab, .csv là dấu phẩy, hoặc chấm phẩy khi dòng đầu chỉ có chấm phẩy;
//! - ô trong ngoặc kép giữ được dấu phân cách, xuống dòng và `""`; dòng trống bị bỏ, dòng ngắn
//!   được đệm ô trống, ô thừa của dòng dài bị bỏ (kèm cảnh báo);
//! - tên cột bắt đầu bằng `#` là biến ẩn/hiện (true/on hiện, false/off/trống ẩn, chữ khác hiện);
//!   `@` là biến ảnh (đường dẫn file ảnh; tương đối thì tính từ thư mục file dữ liệu; ô trống giữ
//!   ảnh như cũ), gắn object ảnh; `%` (biểu đồ) chưa nhận nên cột bị bỏ qua kèm cảnh báo;
//! - tên biến giữ nguyên chữ của dòng tiêu đề (có dấu, có khoảng trắng), chỉ đặt lại tên trống
//!   hoặc trùng; bản ghi tên Record-1, Record-2… hoặc theo một cột (trùng/trống thì Record 1…);
//! - gắn object theo tên (hoặc ghi chú Attributes) trùng tên biến, ở mọi tầng group, không tính
//!   layer; biến chữ chỉ gắn object chữ, biến ảnh chỉ gắn object ảnh; object bị khóa hoặc nằm
//!   trong layer ẩn/khóa được bỏ qua
//!   và báo lại;
//! - thay toàn bộ biến, bản ghi và liên kết cũ, rồi hiện bản ghi 1 (`dataset.select`).
//!
//! Duyệt các bản ghi trong bảng Variables (`dataset.next` / `dataset.prev`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use serde_json::{Value, json};
use vectorcraft_doc::{DataSet, DataValue, Document, Node, NodeId, NodeKind, Variable, VariableKind, Variables};

use crate::cmd::*;
use crate::{EngineError, Result, Session};

/// Tên bước hoàn tác.
const LABEL: &str = "Nhập Variable (DanCard)";
/// File dữ liệu lớn nhất đọc được (byte).
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// Số ô đọc tối đa từ file.
const MAX_CELLS: usize = 1_000_000;
/// Một ô dài tối đa (byte); phần dư bị cắt.
const MAX_CELL_BYTES: usize = 64 * 1024;
/// Số biến (cột) tối đa.
const MAX_COLUMNS: usize = 500;
/// Số bản ghi (dòng) tối đa.
const MAX_RECORDS: usize = 10_000;
/// Số giá trị (bản ghi × biến) tối đa lưu vào tài liệu.
const MAX_VALUES: usize = 500_000;
/// Tên biến, tên bản ghi dài tối đa (ký tự), như `variablecmds`.
const MAX_NAME: usize = 255;
/// Đoạn trống của ô chữ thành một khoảng trắng không ngắt (như Variable Importer), để khung chữ
/// không rỗng và giữ định dạng.
const NBSP: &str = "\u{A0}";
/// Số tên liệt kê trong một thông báo.
const LISTED: usize = 5;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "dancard.importVariables",
        "DanCard: Nhập dữ liệu Variable",
        [],
        None,
        "{path?: (file .csv hoặc .txt; bản web không đọc được đường dẫn), text?: (nội dung file thay cho path; cho web và MCP), delimiter?: auto|comma|semicolon|tab (auto: .txt là Tab; .csv là dấu phẩy hoặc chấm phẩy khi dòng đầu chỉ có chấm phẩy; text không tên file thì theo dấu dòng đầu dùng nhiều nhất), headers?: bool (dòng đầu là tên biến; mặc định true; false thì Variable1…), transpose?: bool (đảo hàng và cột: cột đầu là tên biến), bindBy?: name|note|none (gắn object có tên hoặc ghi chú Attributes trùng tên biến ở mọi tầng group, không tính layer; none: giữ các liên kết cũ của biến cùng tên cùng loại; mặc định name), datasetNames?: (tên hoặc số thứ tự cột đặt tên bản ghi; mặc định Record-1…; trùng hoặc trống thì Record 1…), backslashBreak?: bool (hai dấu \\\\ trong ô là xuống dòng), dryRun?: bool (chỉ báo trước, không đổi tài liệu)} nhập file dữ liệu thành Variables: mỗi cột một biến (chữ; tên cột bắt đầu bằng # là biến ẩn/hiện, @ là biến ảnh: đường dẫn file ảnh, tương đối thì tính từ thư mục file dữ liệu; % biểu đồ chưa nhận nên bỏ qua kèm cảnh báo), mỗi dòng một bản ghi; chuẩn hoá NFC tiếng Việt; thay toàn bộ biến và bản ghi cũ, gắn object rồi hiện bản ghi 1, một bước hoàn tác → {message, variables: [{name, kind, column, bound}], datasets: [tên], bound, applied?, skipped?, warnings, dryRun, delimiter, encoding}",
        has_doc,
        import_variables
    )]
}

// ---------------------------------------------------------------- tham số

/// Gắn object với biến theo gì.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindBy {
    Name,
    Note,
    /// Không gắn tự động: giữ liên kết cũ của biến cùng tên cùng loại.
    Nothing,
}

struct Options {
    /// `None`: tự nhận.
    delimiter: Option<char>,
    headers: bool,
    transpose: bool,
    bind_by: BindBy,
    /// Cột đặt tên bản ghi (tên hoặc số thứ tự).
    dataset_names: Option<String>,
    backslash_break: bool,
    dry_run: bool,
    /// Thư mục của file dữ liệu: đường dẫn ảnh tương đối tính từ đây.
    base: Option<std::path::PathBuf>,
}

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

fn options(p: &Value) -> Result<Options> {
    let choice = |key: &str| str_param(p, key).map(|v| v.trim().to_ascii_lowercase()).filter(|v| !v.is_empty());
    let delimiter = match choice("delimiter").as_deref() {
        None | Some("auto") => None,
        Some("comma" | ",") => Some(','),
        Some("semicolon" | ";") => Some(';'),
        Some("tab") => Some('\t'),
        Some(d) => return Err(other(format!("Dấu phân cách “{d}” không hợp lệ: chọn auto, comma, semicolon hoặc tab."))),
    };
    let bind_by = match choice("bindBy").as_deref() {
        None | Some("name") => BindBy::Name,
        Some("note") => BindBy::Note,
        Some("none") => BindBy::Nothing,
        Some(b) => return Err(other(format!("Cách gắn “{b}” không hợp lệ: chọn name (tên object), note (ghi chú) hoặc none."))),
    };
    let dataset_names = match p.get("datasetNames") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(nfc(s.trim())).filter(|s| !s.is_empty()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(_) => return Err(other("datasetNames phải là tên hoặc số thứ tự của một cột.")),
    };
    Ok(Options {
        delimiter,
        headers: bool_or(p, "headers", true),
        transpose: bool_or(p, "transpose", false),
        bind_by,
        dataset_names,
        backslash_break: bool_or(p, "backslashBreak", false),
        dry_run: bool_or(p, "dryRun", false),
        base: None,
    })
}

// ---------------------------------------------------------------- đọc file

fn too_big() -> EngineError {
    other(format!("File dữ liệu lớn quá {} MB.", MAX_BYTES / (1024 * 1024)))
}

/// Chữ của dữ liệu (đã chuẩn hoá NFC) và bảng mã đã đọc.
fn read_source(p: &Value, path: Option<&str>, warnings: &mut Vec<String>) -> Result<(String, &'static str)> {
    if let Some(text) = str_param(p, "text") {
        if text.len() > MAX_BYTES {
            return Err(too_big());
        }
        return Ok((nfc(text.strip_prefix('\u{FEFF}').unwrap_or(text)), "utf-8"));
    }
    let Some(path) = path else { return Err(other("Chưa chọn file dữ liệu (.csv hoặc .txt).")) };
    let (text, encoding) = decode(&read_bytes(path)?);
    if encoding == "windows-1258" {
        warnings.push("File không phải UTF-8: đã đọc theo bảng mã Windows-1258 (tiếng Việt). Nên lưu file dạng CSV UTF-8.".into());
    }
    Ok((nfc(text.strip_prefix('\u{FEFF}').unwrap_or(&text)), encoding))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_bytes(path: &str) -> Result<Vec<u8>> {
    let bytes = crate::cmd::fileio::read_file_head(path, MAX_BYTES + 1).map_err(|e| other(format!("Không đọc được file dữ liệu: {e}")))?;
    if bytes.len() > MAX_BYTES {
        return Err(too_big());
    }
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
fn read_bytes(_path: &str) -> Result<Vec<u8>> {
    Err(other("Bản web không đọc file theo đường dẫn: gửi nội dung file qua tham số text."))
}

/// Giải mã file: BOM UTF-8 / UTF-16 quyết định bảng mã; không BOM thì UTF-8, không phải UTF-8
/// thì Windows-1258.
fn decode(bytes: &[u8]) -> (String, &'static str) {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return (String::from_utf8_lossy(rest).into_owned(), "utf-8");
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return (utf16(rest, u16::from_le_bytes), "utf-16le");
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return (utf16(rest, u16::from_be_bytes), "utf-16be");
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), "utf-8"),
        Err(_) => (cp1258(bytes), "windows-1258"),
    }
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| unit(*c)).collect();
    String::from_utf16_lossy(&units)
}

/// Windows-1258, byte 0x80…0xFF (byte chưa định nghĩa giữ mã C1 như WHATWG). Tiếng Việt viết
/// dấu thanh thành ký tự rời (0xCC, 0xD2, 0xDE, 0xEC, 0xF2): NFC ghép lại sau.
const CP1258_HIGH: &str = "\
    \u{20AC}\u{81}\u{201A}\u{192}\u{201E}\u{2026}\u{2020}\u{2021}\u{2C6}\u{2030}\u{8A}\u{2039}\u{152}\
    \u{8D}\u{8E}\u{8F}\u{90}\u{2018}\u{2019}\u{201C}\u{201D}\u{2022}\u{2013}\u{2014}\u{2DC}\u{2122}\
    \u{9A}\u{203A}\u{153}\u{9D}\u{9E}\u{178}\u{A0}\u{A1}\u{A2}\u{A3}\u{A4}\u{A5}\u{A6}\u{A7}\u{A8}\u{A9}\
    \u{AA}\u{AB}\u{AC}\u{AD}\u{AE}\u{AF}\u{B0}\u{B1}\u{B2}\u{B3}\u{B4}\u{B5}\u{B6}\u{B7}\u{B8}\u{B9}\
    \u{BA}\u{BB}\u{BC}\u{BD}\u{BE}\u{BF}\u{C0}\u{C1}\u{C2}\u{102}\u{C4}\u{C5}\u{C6}\u{C7}\u{C8}\u{C9}\
    \u{CA}\u{CB}\u{300}\u{CD}\u{CE}\u{CF}\u{110}\u{D1}\u{309}\u{D3}\u{D4}\u{1A0}\u{D6}\u{D7}\u{D8}\u{D9}\
    \u{DA}\u{DB}\u{DC}\u{1AF}\u{303}\u{DF}\u{E0}\u{E1}\u{E2}\u{103}\u{E4}\u{E5}\u{E6}\u{E7}\u{E8}\u{E9}\
    \u{EA}\u{EB}\u{301}\u{ED}\u{EE}\u{EF}\u{111}\u{F1}\u{323}\u{F3}\u{F4}\u{1A1}\u{F6}\u{F7}\u{F8}\u{F9}\
    \u{FA}\u{FB}\u{FC}\u{1B0}\u{20AB}\u{FF}";

fn cp1258(bytes: &[u8]) -> String {
    let high: Vec<char> = CP1258_HIGH.chars().collect();
    bytes.iter().map(|&b| if b < 0x80 { char::from(b) } else { high.get(usize::from(b - 0x80)).copied().unwrap_or('\u{FFFD}') }).collect()
}

// ---------------------------------------------------------------- chuẩn hoá NFC (chữ Latin)

/// Các cặp ghép chuẩn của chữ Latin (U+00C0…U+024F, U+1E00…U+1EFF): mỗi bộ ba ký tự là
/// (chữ dựng sẵn, chữ gốc, dấu), theo phân rã chuẩn của Unicode. Đủ cho mọi chữ tiếng Việt.
const LATIN_PAIRS: &str = "\
    \u{C0}A\u{300}\u{C1}A\u{301}\u{C2}A\u{302}\u{C3}A\u{303}\u{C4}A\u{308}\u{C5}A\u{30A}\u{C7}C\u{327}\
    \u{C8}E\u{300}\u{C9}E\u{301}\u{CA}E\u{302}\u{CB}E\u{308}\u{CC}I\u{300}\u{CD}I\u{301}\u{CE}I\u{302}\
    \u{CF}I\u{308}\u{D1}N\u{303}\u{D2}O\u{300}\u{D3}O\u{301}\u{D4}O\u{302}\u{D5}O\u{303}\u{D6}O\u{308}\
    \u{D9}U\u{300}\u{DA}U\u{301}\u{DB}U\u{302}\u{DC}U\u{308}\u{DD}Y\u{301}\u{E0}a\u{300}\u{E1}a\u{301}\
    \u{E2}a\u{302}\u{E3}a\u{303}\u{E4}a\u{308}\u{E5}a\u{30A}\u{E7}c\u{327}\u{E8}e\u{300}\u{E9}e\u{301}\
    \u{EA}e\u{302}\u{EB}e\u{308}\u{EC}i\u{300}\u{ED}i\u{301}\u{EE}i\u{302}\u{EF}i\u{308}\u{F1}n\u{303}\
    \u{F2}o\u{300}\u{F3}o\u{301}\u{F4}o\u{302}\u{F5}o\u{303}\u{F6}o\u{308}\u{F9}u\u{300}\u{FA}u\u{301}\
    \u{FB}u\u{302}\u{FC}u\u{308}\u{FD}y\u{301}\u{FF}y\u{308}\u{100}A\u{304}\u{101}a\u{304}\
    \u{102}A\u{306}\u{103}a\u{306}\u{104}A\u{328}\u{105}a\u{328}\u{106}C\u{301}\u{107}c\u{301}\
    \u{108}C\u{302}\u{109}c\u{302}\u{10A}C\u{307}\u{10B}c\u{307}\u{10C}C\u{30C}\u{10D}c\u{30C}\
    \u{10E}D\u{30C}\u{10F}d\u{30C}\u{112}E\u{304}\u{113}e\u{304}\u{114}E\u{306}\u{115}e\u{306}\
    \u{116}E\u{307}\u{117}e\u{307}\u{118}E\u{328}\u{119}e\u{328}\u{11A}E\u{30C}\u{11B}e\u{30C}\
    \u{11C}G\u{302}\u{11D}g\u{302}\u{11E}G\u{306}\u{11F}g\u{306}\u{120}G\u{307}\u{121}g\u{307}\
    \u{122}G\u{327}\u{123}g\u{327}\u{124}H\u{302}\u{125}h\u{302}\u{128}I\u{303}\u{129}i\u{303}\
    \u{12A}I\u{304}\u{12B}i\u{304}\u{12C}I\u{306}\u{12D}i\u{306}\u{12E}I\u{328}\u{12F}i\u{328}\
    \u{130}I\u{307}\u{134}J\u{302}\u{135}j\u{302}\u{136}K\u{327}\u{137}k\u{327}\u{139}L\u{301}\
    \u{13A}l\u{301}\u{13B}L\u{327}\u{13C}l\u{327}\u{13D}L\u{30C}\u{13E}l\u{30C}\u{143}N\u{301}\
    \u{144}n\u{301}\u{145}N\u{327}\u{146}n\u{327}\u{147}N\u{30C}\u{148}n\u{30C}\u{14C}O\u{304}\
    \u{14D}o\u{304}\u{14E}O\u{306}\u{14F}o\u{306}\u{150}O\u{30B}\u{151}o\u{30B}\u{154}R\u{301}\
    \u{155}r\u{301}\u{156}R\u{327}\u{157}r\u{327}\u{158}R\u{30C}\u{159}r\u{30C}\u{15A}S\u{301}\
    \u{15B}s\u{301}\u{15C}S\u{302}\u{15D}s\u{302}\u{15E}S\u{327}\u{15F}s\u{327}\u{160}S\u{30C}\
    \u{161}s\u{30C}\u{162}T\u{327}\u{163}t\u{327}\u{164}T\u{30C}\u{165}t\u{30C}\u{168}U\u{303}\
    \u{169}u\u{303}\u{16A}U\u{304}\u{16B}u\u{304}\u{16C}U\u{306}\u{16D}u\u{306}\u{16E}U\u{30A}\
    \u{16F}u\u{30A}\u{170}U\u{30B}\u{171}u\u{30B}\u{172}U\u{328}\u{173}u\u{328}\u{174}W\u{302}\
    \u{175}w\u{302}\u{176}Y\u{302}\u{177}y\u{302}\u{178}Y\u{308}\u{179}Z\u{301}\u{17A}z\u{301}\
    \u{17B}Z\u{307}\u{17C}z\u{307}\u{17D}Z\u{30C}\u{17E}z\u{30C}\u{1A0}O\u{31B}\u{1A1}o\u{31B}\
    \u{1AF}U\u{31B}\u{1B0}u\u{31B}\u{1CD}A\u{30C}\u{1CE}a\u{30C}\u{1CF}I\u{30C}\u{1D0}i\u{30C}\
    \u{1D1}O\u{30C}\u{1D2}o\u{30C}\u{1D3}U\u{30C}\u{1D4}u\u{30C}\u{1D5}\u{DC}\u{304}\u{1D6}\u{FC}\u{304}\
    \u{1D7}\u{DC}\u{301}\u{1D8}\u{FC}\u{301}\u{1D9}\u{DC}\u{30C}\u{1DA}\u{FC}\u{30C}\u{1DB}\u{DC}\u{300}\
    \u{1DC}\u{FC}\u{300}\u{1DE}\u{C4}\u{304}\u{1DF}\u{E4}\u{304}\u{1E0}\u{226}\u{304}\
    \u{1E1}\u{227}\u{304}\u{1E6}G\u{30C}\u{1E7}g\u{30C}\u{1E8}K\u{30C}\u{1E9}k\u{30C}\u{1EA}O\u{328}\
    \u{1EB}o\u{328}\u{1EC}\u{1EA}\u{304}\u{1ED}\u{1EB}\u{304}\u{1F0}j\u{30C}\u{1F4}G\u{301}\
    \u{1F5}g\u{301}\u{1F8}N\u{300}\u{1F9}n\u{300}\u{1FA}\u{C5}\u{301}\u{1FB}\u{E5}\u{301}\u{200}A\u{30F}\
    \u{201}a\u{30F}\u{202}A\u{311}\u{203}a\u{311}\u{204}E\u{30F}\u{205}e\u{30F}\u{206}E\u{311}\
    \u{207}e\u{311}\u{208}I\u{30F}\u{209}i\u{30F}\u{20A}I\u{311}\u{20B}i\u{311}\u{20C}O\u{30F}\
    \u{20D}o\u{30F}\u{20E}O\u{311}\u{20F}o\u{311}\u{210}R\u{30F}\u{211}r\u{30F}\u{212}R\u{311}\
    \u{213}r\u{311}\u{214}U\u{30F}\u{215}u\u{30F}\u{216}U\u{311}\u{217}u\u{311}\u{218}S\u{326}\
    \u{219}s\u{326}\u{21A}T\u{326}\u{21B}t\u{326}\u{21E}H\u{30C}\u{21F}h\u{30C}\u{226}A\u{307}\
    \u{227}a\u{307}\u{228}E\u{327}\u{229}e\u{327}\u{22A}\u{D6}\u{304}\u{22B}\u{F6}\u{304}\
    \u{22C}\u{D5}\u{304}\u{22D}\u{F5}\u{304}\u{22E}O\u{307}\u{22F}o\u{307}\u{230}\u{22E}\u{304}\
    \u{231}\u{22F}\u{304}\u{232}Y\u{304}\u{233}y\u{304}\u{1E00}A\u{325}\u{1E01}a\u{325}\u{1E02}B\u{307}\
    \u{1E03}b\u{307}\u{1E04}B\u{323}\u{1E05}b\u{323}\u{1E06}B\u{331}\u{1E07}b\u{331}\
    \u{1E08}\u{C7}\u{301}\u{1E09}\u{E7}\u{301}\u{1E0A}D\u{307}\u{1E0B}d\u{307}\u{1E0C}D\u{323}\
    \u{1E0D}d\u{323}\u{1E0E}D\u{331}\u{1E0F}d\u{331}\u{1E10}D\u{327}\u{1E11}d\u{327}\u{1E12}D\u{32D}\
    \u{1E13}d\u{32D}\u{1E14}\u{112}\u{300}\u{1E15}\u{113}\u{300}\u{1E16}\u{112}\u{301}\
    \u{1E17}\u{113}\u{301}\u{1E18}E\u{32D}\u{1E19}e\u{32D}\u{1E1A}E\u{330}\u{1E1B}e\u{330}\
    \u{1E1C}\u{228}\u{306}\u{1E1D}\u{229}\u{306}\u{1E1E}F\u{307}\u{1E1F}f\u{307}\u{1E20}G\u{304}\
    \u{1E21}g\u{304}\u{1E22}H\u{307}\u{1E23}h\u{307}\u{1E24}H\u{323}\u{1E25}h\u{323}\u{1E26}H\u{308}\
    \u{1E27}h\u{308}\u{1E28}H\u{327}\u{1E29}h\u{327}\u{1E2A}H\u{32E}\u{1E2B}h\u{32E}\u{1E2C}I\u{330}\
    \u{1E2D}i\u{330}\u{1E2E}\u{CF}\u{301}\u{1E2F}\u{EF}\u{301}\u{1E30}K\u{301}\u{1E31}k\u{301}\
    \u{1E32}K\u{323}\u{1E33}k\u{323}\u{1E34}K\u{331}\u{1E35}k\u{331}\u{1E36}L\u{323}\u{1E37}l\u{323}\
    \u{1E38}\u{1E36}\u{304}\u{1E39}\u{1E37}\u{304}\u{1E3A}L\u{331}\u{1E3B}l\u{331}\u{1E3C}L\u{32D}\
    \u{1E3D}l\u{32D}\u{1E3E}M\u{301}\u{1E3F}m\u{301}\u{1E40}M\u{307}\u{1E41}m\u{307}\u{1E42}M\u{323}\
    \u{1E43}m\u{323}\u{1E44}N\u{307}\u{1E45}n\u{307}\u{1E46}N\u{323}\u{1E47}n\u{323}\u{1E48}N\u{331}\
    \u{1E49}n\u{331}\u{1E4A}N\u{32D}\u{1E4B}n\u{32D}\u{1E4C}\u{D5}\u{301}\u{1E4D}\u{F5}\u{301}\
    \u{1E4E}\u{D5}\u{308}\u{1E4F}\u{F5}\u{308}\u{1E50}\u{14C}\u{300}\u{1E51}\u{14D}\u{300}\
    \u{1E52}\u{14C}\u{301}\u{1E53}\u{14D}\u{301}\u{1E54}P\u{301}\u{1E55}p\u{301}\u{1E56}P\u{307}\
    \u{1E57}p\u{307}\u{1E58}R\u{307}\u{1E59}r\u{307}\u{1E5A}R\u{323}\u{1E5B}r\u{323}\
    \u{1E5C}\u{1E5A}\u{304}\u{1E5D}\u{1E5B}\u{304}\u{1E5E}R\u{331}\u{1E5F}r\u{331}\u{1E60}S\u{307}\
    \u{1E61}s\u{307}\u{1E62}S\u{323}\u{1E63}s\u{323}\u{1E64}\u{15A}\u{307}\u{1E65}\u{15B}\u{307}\
    \u{1E66}\u{160}\u{307}\u{1E67}\u{161}\u{307}\u{1E68}\u{1E62}\u{307}\u{1E69}\u{1E63}\u{307}\
    \u{1E6A}T\u{307}\u{1E6B}t\u{307}\u{1E6C}T\u{323}\u{1E6D}t\u{323}\u{1E6E}T\u{331}\u{1E6F}t\u{331}\
    \u{1E70}T\u{32D}\u{1E71}t\u{32D}\u{1E72}U\u{324}\u{1E73}u\u{324}\u{1E74}U\u{330}\u{1E75}u\u{330}\
    \u{1E76}U\u{32D}\u{1E77}u\u{32D}\u{1E78}\u{168}\u{301}\u{1E79}\u{169}\u{301}\u{1E7A}\u{16A}\u{308}\
    \u{1E7B}\u{16B}\u{308}\u{1E7C}V\u{303}\u{1E7D}v\u{303}\u{1E7E}V\u{323}\u{1E7F}v\u{323}\
    \u{1E80}W\u{300}\u{1E81}w\u{300}\u{1E82}W\u{301}\u{1E83}w\u{301}\u{1E84}W\u{308}\u{1E85}w\u{308}\
    \u{1E86}W\u{307}\u{1E87}w\u{307}\u{1E88}W\u{323}\u{1E89}w\u{323}\u{1E8A}X\u{307}\u{1E8B}x\u{307}\
    \u{1E8C}X\u{308}\u{1E8D}x\u{308}\u{1E8E}Y\u{307}\u{1E8F}y\u{307}\u{1E90}Z\u{302}\u{1E91}z\u{302}\
    \u{1E92}Z\u{323}\u{1E93}z\u{323}\u{1E94}Z\u{331}\u{1E95}z\u{331}\u{1E96}h\u{331}\u{1E97}t\u{308}\
    \u{1E98}w\u{30A}\u{1E99}y\u{30A}\u{1EA0}A\u{323}\u{1EA1}a\u{323}\u{1EA2}A\u{309}\u{1EA3}a\u{309}\
    \u{1EA4}\u{C2}\u{301}\u{1EA5}\u{E2}\u{301}\u{1EA6}\u{C2}\u{300}\u{1EA7}\u{E2}\u{300}\
    \u{1EA8}\u{C2}\u{309}\u{1EA9}\u{E2}\u{309}\u{1EAA}\u{C2}\u{303}\u{1EAB}\u{E2}\u{303}\
    \u{1EAC}\u{1EA0}\u{302}\u{1EAD}\u{1EA1}\u{302}\u{1EAE}\u{102}\u{301}\u{1EAF}\u{103}\u{301}\
    \u{1EB0}\u{102}\u{300}\u{1EB1}\u{103}\u{300}\u{1EB2}\u{102}\u{309}\u{1EB3}\u{103}\u{309}\
    \u{1EB4}\u{102}\u{303}\u{1EB5}\u{103}\u{303}\u{1EB6}\u{1EA0}\u{306}\u{1EB7}\u{1EA1}\u{306}\
    \u{1EB8}E\u{323}\u{1EB9}e\u{323}\u{1EBA}E\u{309}\u{1EBB}e\u{309}\u{1EBC}E\u{303}\u{1EBD}e\u{303}\
    \u{1EBE}\u{CA}\u{301}\u{1EBF}\u{EA}\u{301}\u{1EC0}\u{CA}\u{300}\u{1EC1}\u{EA}\u{300}\
    \u{1EC2}\u{CA}\u{309}\u{1EC3}\u{EA}\u{309}\u{1EC4}\u{CA}\u{303}\u{1EC5}\u{EA}\u{303}\
    \u{1EC6}\u{1EB8}\u{302}\u{1EC7}\u{1EB9}\u{302}\u{1EC8}I\u{309}\u{1EC9}i\u{309}\u{1ECA}I\u{323}\
    \u{1ECB}i\u{323}\u{1ECC}O\u{323}\u{1ECD}o\u{323}\u{1ECE}O\u{309}\u{1ECF}o\u{309}\
    \u{1ED0}\u{D4}\u{301}\u{1ED1}\u{F4}\u{301}\u{1ED2}\u{D4}\u{300}\u{1ED3}\u{F4}\u{300}\
    \u{1ED4}\u{D4}\u{309}\u{1ED5}\u{F4}\u{309}\u{1ED6}\u{D4}\u{303}\u{1ED7}\u{F4}\u{303}\
    \u{1ED8}\u{1ECC}\u{302}\u{1ED9}\u{1ECD}\u{302}\u{1EDA}\u{1A0}\u{301}\u{1EDB}\u{1A1}\u{301}\
    \u{1EDC}\u{1A0}\u{300}\u{1EDD}\u{1A1}\u{300}\u{1EDE}\u{1A0}\u{309}\u{1EDF}\u{1A1}\u{309}\
    \u{1EE0}\u{1A0}\u{303}\u{1EE1}\u{1A1}\u{303}\u{1EE2}\u{1A0}\u{323}\u{1EE3}\u{1A1}\u{323}\
    \u{1EE4}U\u{323}\u{1EE5}u\u{323}\u{1EE6}U\u{309}\u{1EE7}u\u{309}\u{1EE8}\u{1AF}\u{301}\
    \u{1EE9}\u{1B0}\u{301}\u{1EEA}\u{1AF}\u{300}\u{1EEB}\u{1B0}\u{300}\u{1EEC}\u{1AF}\u{309}\
    \u{1EED}\u{1B0}\u{309}\u{1EEE}\u{1AF}\u{303}\u{1EEF}\u{1B0}\u{303}\u{1EF0}\u{1AF}\u{323}\
    \u{1EF1}\u{1B0}\u{323}\u{1EF2}Y\u{300}\u{1EF3}y\u{300}\u{1EF4}Y\u{323}\u{1EF5}y\u{323}\
    \u{1EF6}Y\u{309}\u{1EF7}y\u{309}\u{1EF8}Y\u{303}\u{1EF9}y\u{303}";

struct Latin {
    split: HashMap<char, (char, char)>,
    join: HashMap<(char, char), char>,
}

fn latin() -> &'static Latin {
    static TABLES: OnceLock<Latin> = OnceLock::new();
    TABLES.get_or_init(|| {
        let chars: Vec<char> = LATIN_PAIRS.chars().collect();
        let mut t = Latin { split: HashMap::new(), join: HashMap::new() };
        for entry in chars.chunks_exact(3) {
            if let &[whole, base, mark] = entry {
                t.split.insert(whole, (base, mark));
                t.join.insert((base, mark), whole);
            }
        }
        t
    })
}

/// Lớp kết hợp chuẩn (canonical combining class) của dấu U+0300…U+036F; 0 cho ký tự khác.
fn ccc(c: char) -> u8 {
    match u32::from(c) {
        0x300..=0x314 | 0x33D..=0x344 | 0x346 | 0x34A..=0x34C | 0x350..=0x352 | 0x357 | 0x35B | 0x363..=0x36F => 230,
        0x315 | 0x31A | 0x358 => 232,
        0x316..=0x319
        | 0x31C..=0x320
        | 0x323..=0x326
        | 0x329..=0x333
        | 0x339..=0x33C
        | 0x347..=0x349
        | 0x34D
        | 0x34E
        | 0x353..=0x356
        | 0x359
        | 0x35A => 220,
        0x31B => 216,
        0x321 | 0x322 | 0x327 | 0x328 => 202,
        0x334..=0x338 => 1,
        0x345 => 240,
        0x35C | 0x35F | 0x362 => 233,
        0x35D | 0x35E | 0x360 | 0x361 => 234,
        _ => 0,
    }
}

fn decompose(c: char, t: &Latin, out: &mut Vec<char>, depth: u8) {
    match c {
        '\u{340}' => out.push('\u{300}'),
        '\u{341}' => out.push('\u{301}'),
        '\u{343}' => out.push('\u{313}'),
        '\u{344}' => out.extend(['\u{308}', '\u{301}']),
        _ => match t.split.get(&c) {
            Some(&(base, mark)) if depth < 4 => {
                decompose(base, t, out, depth + 1);
                out.push(mark);
            }
            _ => out.push(c),
        },
    }
}

/// NFC cho chữ Latin: phân rã, xếp dấu theo lớp kết hợp (giữ thứ tự dấu cùng lớp), rồi ghép lại.
/// `e` + U+0302 + U+0301 và `ê` + U+0301 đều thành `ế`; chữ không có dạng dựng sẵn giữ nguyên.
/// Dấu chỉ ghép với chữ gốc đứng trước nó, nên làm từng đoạn (chữ gốc và các dấu sau nó): bộ nhớ
/// không lớn theo cỡ file.
fn nfc(s: &str) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    let t = latin();
    let mut out = String::with_capacity(s.len());
    let mut segment: Vec<char> = Vec::new();
    let mut split: Vec<char> = Vec::new();
    for c in s.chars() {
        split.clear();
        decompose(c, t, &mut split, 0);
        for &d in &split {
            if ccc(d) == 0 {
                compose_segment(&mut segment, t, &mut out);
            }
            segment.push(d);
        }
    }
    compose_segment(&mut segment, t, &mut out);
    out
}

/// Ghép một đoạn đã phân rã (chữ gốc nếu có, rồi các dấu) vào `out` và làm rỗng nó.
fn compose_segment(segment: &mut Vec<char>, t: &Latin, out: &mut String) {
    let start = usize::from(segment.first().is_some_and(|c| ccc(*c) == 0));
    if let Some(marks) = segment.get_mut(start..) {
        marks.sort_by_key(|m| ccc(*m));
    }
    let mut chars = segment.drain(..);
    let mut starter = if start == 1 { chars.next() } else { None };
    // Dấu chưa ghép được, và lớp của dấu cuối trong số đó: dấu sau chỉ ghép được khi không bị
    // chặn (chưa có dấu nào đứng giữa, hoặc dấu đứng giữa có lớp nhỏ hơn).
    let mut pending: Vec<char> = Vec::new();
    let mut last = 0u8;
    for c in chars {
        let class = ccc(c);
        if (pending.is_empty() || last < class)
            && let Some(whole) = starter.and_then(|st| t.join.get(&(st, c)).copied())
        {
            starter = Some(whole);
        } else {
            pending.push(c);
            last = class;
        }
    }
    out.extend(starter);
    out.extend(pending);
}

// ---------------------------------------------------------------- tách ô

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Field,
    Row,
    Eof,
}

/// Các dòng đọc được (đã bỏ dòng mà mọi ô đều trống) và những gì phải báo.
#[derive(Debug, Default)]
struct Grid {
    rows: Vec<Vec<String>>,
    /// Có ô dài quá [`MAX_CELL_BYTES`] bị cắt.
    cut_cells: bool,
    /// Dừng đọc ở [`MAX_CELLS`] ô.
    truncated: bool,
}

fn push_char(field: &mut String, c: char, cut: &mut bool) {
    if field.len() + c.len_utf8() <= MAX_CELL_BYTES {
        field.push(c);
    } else {
        *cut = true;
    }
}

/// Tách bản ghi: dòng kết thúc ở `\r`, `\n` hoặc `\r\n`. Ô bắt đầu bằng `"` là ô trong ngoặc
/// kép: `""` là một dấu `"`, dấu phân cách và xuống dòng bên trong được giữ, chữ sau dấu `"`
/// đóng (tới dấu phân cách hay cuối dòng) bị bỏ. Ô khác chạy tới dấu phân cách hay cuối dòng,
/// giữ nguyên dấu `"`. Không cắt khoảng trắng.
fn parse(text: &str, delim: char) -> Grid {
    let mut grid = Grid::default();
    let mut row: Vec<String> = Vec::new();
    let mut cells = 0usize;
    let mut chars = text.chars().peekable();
    loop {
        let mut field = String::new();
        let mut end = End::Field;
        let quoted = chars.peek() == Some(&'"');
        if quoted {
            chars.next();
            loop {
                match chars.next() {
                    None => {
                        end = End::Eof;
                        break;
                    }
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        push_char(&mut field, '"', &mut grid.cut_cells);
                    }
                    Some('"') => break,
                    Some(c) => push_char(&mut field, c, &mut grid.cut_cells),
                }
            }
        }
        if end != End::Eof {
            // Ô không ngoặc kép, hoặc phần sau dấu `"` đóng (bỏ đi).
            loop {
                match chars.next() {
                    None => {
                        end = End::Eof;
                        break;
                    }
                    Some(c) if c == delim => {
                        end = End::Field;
                        break;
                    }
                    Some('\r') => {
                        if chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        end = End::Row;
                        break;
                    }
                    Some('\n') => {
                        end = End::Row;
                        break;
                    }
                    Some(c) => {
                        if !quoted {
                            push_char(&mut field, c, &mut grid.cut_cells);
                        }
                    }
                }
            }
        }
        row.push(field);
        cells += 1;
        let stop = end == End::Eof || cells >= MAX_CELLS;
        if end != End::Field || stop {
            let done = std::mem::take(&mut row);
            if done.iter().any(|c| !c.is_empty()) {
                grid.rows.push(done);
            }
        }
        if stop {
            grid.truncated = end != End::Eof;
            break;
        }
    }
    grid
}

/// Dòng đầu của dữ liệu (tới `\r` hoặc `\n` đầu tiên).
fn first_line(text: &str) -> &str {
    text.split(['\r', '\n']).next().unwrap_or("")
}

/// Dấu phân cách: theo lựa chọn; tự nhận thì theo đuôi file như Variable Importer (.txt Tab, .csv
/// dấu phẩy, hoặc chấm phẩy khi dòng đầu không có dấu phẩy mà có chấm phẩy); chữ không có tên file
/// thì theo dấu dòng đầu dùng nhiều nhất.
fn pick_delimiter(chosen: Option<char>, ext: Option<&str>, text: &str) -> Result<char> {
    if let Some(d) = chosen {
        return Ok(d);
    }
    let line = first_line(text);
    match ext {
        Some("txt" | "tsv" | "tab") => Ok('\t'),
        Some("csv") => Ok(if !line.contains(',') && line.contains(';') { ';' } else { ',' }),
        Some(e) => {
            Err(other(format!("Chỉ nhận file .csv (dấu phẩy hoặc chấm phẩy) hoặc .txt (Tab), không nhận file .{e}; hoặc chọn dấu phân cách.")))
        }
        None => {
            let count = |d: char| line.matches(d).count();
            let (tabs, semis, commas) = (count('\t'), count(';'), count(','));
            Ok(if tabs > commas && tabs >= semis {
                '\t'
            } else if semis > commas {
                ';'
            } else {
                ','
            })
        }
    }
}

fn delimiter_label(d: char) -> &'static str {
    match d {
        '\t' => "tab",
        ';' => "semicolon",
        _ => "comma",
    }
}

// ---------------------------------------------------------------- bảng, biến, bản ghi

/// Bảng nguồn: các dòng đọc được, đảo hàng/cột khi `transpose`. Mọi dòng dài bằng dòng đầu: ô
/// thiếu là ô trống, ô thừa bị bỏ.
struct Table<'a> {
    rows: &'a [Vec<String>],
    /// Số ô của dòng đầu.
    width: usize,
    transpose: bool,
}

impl<'a> Table<'a> {
    fn new(rows: &'a [Vec<String>], transpose: bool) -> Self {
        Table { rows, width: rows.first().map_or(0, Vec::len), transpose }
    }
    /// Số cột (biến).
    fn columns(&self) -> usize {
        if self.transpose { self.rows.len() } else { self.width }
    }
    /// Số dòng (tiêu đề và bản ghi).
    fn height(&self) -> usize {
        if self.transpose { self.width } else { self.rows.len() }
    }
    fn cell(&self, r: usize, c: usize) -> &'a str {
        let (i, j) = if self.transpose { (c, r) } else { (r, c) };
        if j >= self.width {
            return "";
        }
        self.rows.get(i).and_then(|row| row.get(j)).map_or("", String::as_str)
    }
    /// Số dòng dài hơn dòng đầu (ô thừa bị bỏ).
    fn long_rows(&self) -> usize {
        self.rows.iter().skip(1).filter(|r| r.len() > self.width).count()
    }
}

/// Một biến sẽ tạo: cột nguồn (từ 0), tên, loại.
#[derive(Clone, Debug)]
struct Column {
    index: usize,
    name: String,
    kind: VariableKind,
}

/// Các biến theo dòng tiêu đề: `#` ẩn/hiện, `@` ảnh, còn lại chữ; `%` (biểu đồ) bỏ qua. Tên giữ nguyên chữ
/// (đã NFC, cắt khoảng trắng hai đầu); tên trống thành `VariableN`, tên trùng thêm số. Không có
/// tiêu đề: `Variable1…`, đều là chữ.
fn columns(table: &Table, headers: bool, warnings: &mut Vec<String>) -> Vec<Column> {
    let width = table.columns();
    if width > MAX_COLUMNS {
        warnings.push(format!("Chỉ nhận {MAX_COLUMNS} cột đầu (file có {width} cột)."));
    }
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for i in 0..width.min(MAX_COLUMNS) {
        let n = i + 1;
        let (raw, kind) = if headers {
            let h = table.cell(0, i);
            if let Some(rest) = h.strip_prefix('#') {
                (rest, VariableKind::Visibility)
            } else if let Some(rest) = h.strip_prefix('@') {
                (rest, VariableKind::Image)
            } else if h.starts_with('%') {
                warnings.push(format!("Bỏ qua cột {n} “{}”: DanCard chưa nhận cột biểu đồ (Graph).", h.trim()));
                continue;
            } else {
                (h, VariableKind::Text)
            }
        } else {
            ("", VariableKind::Text)
        };
        let mut base: String = raw.trim().chars().take(MAX_NAME).collect();
        if base.is_empty() {
            base = format!("Variable{n}");
            if headers {
                warnings.push(format!("Cột {n} không có tên: đặt là “{base}”."));
            }
        }
        let mut name = base.clone();
        let mut k = 2;
        while used.contains(&name) {
            let stem: String = base.chars().take(MAX_NAME - 8).collect();
            name = format!("{stem} {k}");
            k += 1;
        }
        if name != base {
            warnings.push(format!("Tên biến “{base}” bị trùng: cột {n} đổi thành “{name}”."));
        }
        used.insert(name.clone());
        out.push(Column { index: i, name, kind });
    }
    out
}

/// Chữ của một ô cho biến chữ: `\r\n`, `\r`, `\n` là xuống dòng (đoạn mới); `\\` cũng vậy khi
/// `backslash_break`; đoạn trống hoặc chỉ có khoảng trắng thành một khoảng trắng không ngắt.
fn text_value(cell: &str, backslash_break: bool) -> String {
    let mut s = cell.replace("\r\n", "\n").replace('\r', "\n");
    if backslash_break {
        s = s.replace("\\\\", "\n");
    }
    s.split('\n').map(|p| if p.trim().is_empty() { NBSP } else { p }).collect::<Vec<_>>().join("\n")
}

/// Giá trị của một ô cho biến ẩn/hiện: `true`/`on` hiện, `false`/`off` ẩn (không phân biệt hoa
/// thường, bỏ khoảng trắng hai đầu); ô trống ẩn; chữ khác hiện.
fn visible(cell: &str) -> bool {
    let v = cell.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}').to_lowercase();
    match v.as_str() {
        "true" | "on" => true,
        "false" | "off" => false,
        rest => !rest.is_empty(),
    }
}

/// Liệt kê tối đa [`LISTED`] tên trong ngoặc kép.
fn listed<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let all: Vec<&str> = names.into_iter().collect();
    let mut s = all.iter().take(LISTED).map(|n| format!("“{n}”")).collect::<Vec<_>>().join(", ");
    if all.len() > LISTED {
        s.push_str(", …");
    }
    s
}

/// Cột đặt tên bản ghi: tên biến, chữ tiêu đề (có hoặc không ký hiệu `#`/`@`/`%`), hoặc số thứ
/// tự cột (từ 1).
fn dataset_name_column(table: &Table, columns: &[Column], given: &str, headers: bool) -> Option<usize> {
    if let Some(c) = columns.iter().find(|c| c.name == given) {
        return Some(c.index);
    }
    let width = table.columns();
    if headers
        && let Some(i) = (0..width).find(|&i| {
            let h = table.cell(0, i).trim();
            h == given || h.strip_prefix(['#', '@', '%']).is_some_and(|r| r.trim() == given)
        })
    {
        return Some(i);
    }
    given.parse::<usize>().ok().filter(|n| (1..=width).contains(n)).map(|n| n - 1)
}

/// Các bản ghi: một dòng một bản ghi, giá trị theo loại biến.
fn datasets(table: &Table, columns: &[Column], o: &Options, warnings: &mut Vec<String>) -> Result<Vec<DataSet>> {
    let start = usize::from(o.headers);
    let available = table.height().saturating_sub(start);
    let limit = MAX_RECORDS.min(MAX_VALUES.checked_div(columns.len()).unwrap_or(MAX_RECORDS));
    if available > limit {
        warnings.push(format!("Chỉ nhận {limit} bản ghi đầu (file có {available} bản ghi)."));
    }
    let count = available.min(limit);
    let name_column = match &o.dataset_names {
        None => None,
        Some(given) => {
            Some(dataset_name_column(table, columns, given, o.headers).ok_or_else(|| other(format!("Không có cột “{given}” để đặt tên bản ghi.")))?)
        }
    };
    let mut names: Vec<String> = (0..count)
        .map(|k| match name_column {
            Some(c) => table.cell(start + k, c).trim().chars().take(MAX_NAME).collect(),
            None => format!("Record-{}", k + 1),
        })
        .collect();
    // Tên bản ghi phải khác nhau và không trống (VectorCraft cắt khoảng trắng và không nhận tên
    // trống): có một tên hỏng thì đặt lại tất cả, như Variable Importer.
    let clashes = {
        let mut seen = BTreeSet::new();
        let mut bad: Vec<&str> = Vec::new();
        for n in &names {
            if (n.is_empty() || !seen.insert(n.as_str())) && !bad.contains(&n.as_str()) {
                bad.push(if n.is_empty() { "(trống)" } else { n.as_str() });
            }
        }
        (!bad.is_empty()).then(|| listed(bad))
    };
    if let Some(list) = clashes {
        warnings.push(format!("Tên bản ghi bị trùng hoặc để trống ({list}): đặt lại tất cả thành “Record 1”, “Record 2”…"));
        names = (1..=count).map(|k| format!("Record {k}")).collect();
    }
    Ok(names
        .into_iter()
        .enumerate()
        .map(|(k, name)| {
            let r = start + k;
            let values: BTreeMap<String, DataValue> = columns
                .iter()
                .filter_map(|c| {
                    let cell = table.cell(r, c.index);
                    let value = match c.kind {
                        VariableKind::Text => DataValue::Text(text_value(cell, o.backslash_break)),
                        VariableKind::Visibility => DataValue::Visible(visible(cell)),
                        VariableKind::Image => DataValue::Image(image_path(cell, o.base.as_deref())?),
                        // `columns` makes none.
                        VariableKind::Graph => return None,
                    };
                    Some((c.name.clone(), value))
                })
                .collect();
            DataSet { name, values }
        })
        .collect())
}

/// Đường dẫn file ảnh trong ô của biến ảnh (bỏ khoảng trắng và ngoặc kép hai đầu): tương đối thì
/// tính từ thư mục file dữ liệu `base`. Ô trống: không có giá trị, ảnh của bản ghi đó giữ như cũ.
fn image_path(cell: &str, base: Option<&std::path::Path>) -> Option<String> {
    let t = cell.trim().trim_matches('"').trim();
    if t.is_empty() {
        return None;
    }
    let path = std::path::Path::new(t);
    match base {
        Some(base) if !path.is_absolute() => Some(base.join(path).to_string_lossy().into_owned()),
        _ => Some(t.to_string()),
    }
}

// ---------------------------------------------------------------- tìm object để gắn

/// Object sẽ gắn và những object trùng tên mà không gắn được.
#[derive(Debug, Default)]
struct Art {
    /// (object, tên biến).
    pairs: Vec<(NodeId, String)>,
    /// Số object gắn với từng biến (theo thứ tự `columns`).
    per_column: Vec<usize>,
    /// Trùng tên nhưng bị khóa hoặc nằm trong layer ẩn/khóa.
    locked: Vec<String>,
    /// Trùng tên một biến chữ nhưng không phải object chữ.
    not_type: Vec<String>,
    /// Trùng tên một biến ảnh nhưng không phải object ảnh.
    not_image: Vec<String>,
}

/// Mọi object của tài liệu ở mọi tầng group (kể cả trong clip group), không tính layer. Bên
/// trong object chữ, compound path, blend, biểu đồ… không phải object riêng nên không đi vào.
fn candidates<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
    if !n.is_layer() {
        out.push(n);
    }
    let opens = match &n.kind {
        NodeKind::Layer { .. } => true,
        NodeKind::Group { .. } => n.extras().graph.is_none(),
        _ => false,
    };
    if opens {
        for c in n.children().into_iter().flatten() {
            candidates(c, out);
        }
    }
}

fn find_art(d: &Document, columns: &[Column], by: BindBy) -> Art {
    let mut art = Art { per_column: vec![0; columns.len()], ..Art::default() };
    if by == BindBy::Nothing {
        // Giữ liên kết cũ của biến còn đó, cùng tên cùng loại.
        for (id, names) in &d.variables.bindings {
            for name in names {
                let Some(i) = columns.iter().position(|c| &c.name == name) else { continue };
                let same_kind = d.variables.variable(name).map(|v| v.kind) == columns.get(i).map(|c| c.kind);
                if same_kind && d.node(*id).is_some() {
                    if let Some(n) = art.per_column.get_mut(i) {
                        *n += 1;
                    }
                    art.pairs.push((*id, name.clone()));
                }
            }
        }
        return art;
    }
    let index: HashMap<&str, usize> = columns.iter().enumerate().map(|(i, c)| (c.name.as_str(), i)).collect();
    let mut nodes = Vec::new();
    for l in &d.layers {
        candidates(l, &mut nodes);
    }
    for n in nodes {
        let key = match by {
            BindBy::Name => n.name.as_deref(),
            BindBy::Note => n.extras().attrs.as_deref().map(|a| a.note.as_str()),
            BindBy::Nothing => None,
        };
        // Tên/ghi chú so với tên biến sau khi cắt khoảng trắng và chuẩn hoá NFC (tên gõ trên
        // máy khác có thể để dấu thanh rời).
        let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else { continue };
        let key = nfc(key);
        let Some(&i) = index.get(key.as_str()) else { continue };
        let Some(column) = columns.get(i) else { continue };
        if column.kind == VariableKind::Text && !matches!(n.kind, NodeKind::Text(_)) {
            art.not_type.push(key);
            continue;
        }
        if column.kind == VariableKind::Image && !matches!(n.kind, NodeKind::Image(_)) {
            art.not_image.push(key);
            continue;
        }
        if !d.variable_target(n.id) {
            art.locked.push(key);
            continue;
        }
        if let Some(c) = art.per_column.get_mut(i) {
            *c += 1;
        }
        art.pairs.push((n.id, column.name.clone()));
    }
    art
}

// ---------------------------------------------------------------- lệnh

/// Thay thư viện biến của tài liệu rồi hiện bản ghi đầu: → kết quả của `dataset.select`.
fn commit(s: &mut Session, columns: &[Column], datasets: Vec<DataSet>, pairs: &[(NodeId, String)]) -> Result<Option<Value>> {
    let first = datasets.first().map(|d| d.name.clone());
    s.edit(LABEL, |d, _| {
        let mut vars = Variables {
            variables: columns.iter().map(|c| Variable { name: c.name.clone(), kind: c.kind }).collect(),
            datasets,
            ..Variables::default()
        };
        for (id, name) in pairs {
            vars.bind(*id, name);
        }
        d.variables = vars.into();
        Ok(())
    })?;
    // Bảng Variables đang chọn một biến không còn nữa thì không chọn gì (như `variable.delete`).
    let st = s.doc_mut()?;
    if st.variables_highlight.as_ref().is_some_and(|h| st.doc.variables.variable(h).is_none()) {
        st.variables_highlight = None;
        st.revision += 1;
    }
    match first {
        Some(name) => s.execute("dataset.select", &json!({ "name": name })).map(Some),
        None => Ok(None),
    }
}

/// Câu báo khi không gắn được object nào.
fn nothing_bound(by: BindBy, columns: &[Column], art: &Art) -> String {
    if !art.locked.is_empty() || !art.not_type.is_empty() || !art.not_image.is_empty() {
        return " Chưa gắn được object nào: các object trùng tên đều bị khóa, nằm trong layer ẩn/khóa hoặc không đúng loại (biến chữ gắn object chữ, biến ảnh gắn object ảnh; xem cảnh báo).".into();
    }
    let wanted = listed(columns.iter().map(|c| c.name.as_str()));
    match by {
        BindBy::Name => format!(
            " Chưa gắn được object nào: không có object nào đặt tên trùng tên biến ({wanted}). Đặt tên object (tab Đổi tên hoặc bảng Layers) rồi nhập lại, hoặc gắn tay trong bảng Variables."
        ),
        BindBy::Note => format!(
            " Chưa gắn được object nào: không có object nào có ghi chú (Attributes › Note) trùng tên biến ({wanted}). Ghi tên biến vào ghi chú của object rồi nhập lại, hoặc gắn tay trong bảng Variables."
        ),
        BindBy::Nothing => " Không gắn object tự động (chọn gắn theo tên hoặc ghi chú), gắn tay trong bảng Variables.".into(),
    }
}

fn import_variables(s: &mut Session, p: &Value) -> Result<Value> {
    let mut o = options(p)?;
    let mut warnings: Vec<String> = Vec::new();
    let path = str_param(p, "path").map(str::trim).filter(|v| !v.is_empty());
    o.base = path.and_then(|f| std::path::Path::new(f).parent()).map(std::path::Path::to_path_buf);
    let (text, encoding) = read_source(p, path, &mut warnings)?;
    let ext = path.and_then(|f| std::path::Path::new(f).extension()).and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    let delim = pick_delimiter(o.delimiter, ext.as_deref(), &text)?;
    let grid = parse(&text, delim);
    if grid.truncated {
        warnings.push(format!("File có quá {MAX_CELLS} ô: chỉ đọc phần đầu."));
    }
    if grid.cut_cells {
        warnings.push(format!("Có ô dài quá {} KB: đã cắt bớt.", MAX_CELL_BYTES / 1024));
    }
    if grid.rows.is_empty() {
        return Err(other("File dữ liệu trống: không có dòng nào có dữ liệu."));
    }
    let table = Table::new(&grid.rows, o.transpose);
    let long = table.long_rows();
    if long > 0 {
        warnings.push(format!("{long} dòng có nhiều ô hơn dòng đầu ({} ô): bỏ các ô thừa.", table.width));
    }
    let columns = columns(&table, o.headers, &mut warnings);
    if columns.is_empty() {
        let why = if warnings.is_empty() { String::new() } else { format!(" {}", warnings.join(" ")) };
        return Err(other(format!("Không có biến nào để nhập (DanCard nhận biến chữ, ẩn/hiện và ảnh).{why}")));
    }
    let datasets = datasets(&table, &columns, &o, &mut warnings)?;
    let names: Vec<String> = datasets.iter().map(|d| d.name.clone()).collect();
    let doc = &s.doc()?.doc;
    let old = (doc.variables.variables.len(), doc.variables.datasets.len());
    let art = find_art(doc, &columns, o.bind_by);
    if !art.locked.is_empty() {
        warnings.push(format!(
            "Bỏ qua {} object bị khóa hoặc nằm trong layer ẩn/khóa: {}.",
            art.locked.len(),
            listed(art.locked.iter().map(String::as_str))
        ));
    }
    if !art.not_type.is_empty() {
        warnings.push(format!(
            "{} object trùng tên biến chữ nhưng không phải chữ (Type) nên không gắn: {}.",
            art.not_type.len(),
            listed(art.not_type.iter().map(String::as_str))
        ));
    }
    if !art.not_image.is_empty() {
        warnings.push(format!(
            "{} object trùng tên biến ảnh nhưng không phải ảnh nên không gắn: {}.",
            art.not_image.len(),
            listed(art.not_image.iter().map(String::as_str))
        ));
    }
    let bound = art.pairs.len();
    let mut applied = None;
    if !o.dry_run {
        // Thay thư viện và hiện bản ghi 1: một bước hoàn tác (lỗi giữa chừng thì quay lại hết).
        let own_group = s.doc()?.undo_group.is_none();
        if own_group {
            s.begin_named_undo_group(Some(LABEL.into()));
        }
        let r = commit(s, &columns, datasets, &art.pairs);
        if own_group {
            s.end_undo_group(r.is_err());
        }
        applied = r?;
    }
    let (nv, nd) = (columns.len(), names.len());
    let mut message = if o.dry_run {
        format!("OK: Kiểm tra (chưa đổi tài liệu): sẽ nhập {nv} biến, {nd} bản ghi")
    } else {
        format!("OK: Đã nhập {nv} biến, {nd} bản ghi")
    };
    if bound > 0 {
        message.push_str(&format!(", gắn {bound} object"));
        if let Some(first) = names.first()
            && !o.dry_run
        {
            message.push_str(&format!("; đang hiện bản ghi “{first}”"));
        }
        message.push('.');
    } else {
        message.push('.');
        message.push_str(&nothing_bound(o.bind_by, &columns, &art));
    }
    if old.0 > 0 || old.1 > 0 {
        let verb = if o.dry_run { "Sẽ thay" } else { "Đã thay" };
        message.push_str(&format!(" {verb} {} biến và {} bản ghi cũ.", old.0, old.1));
    }
    if !warnings.is_empty() {
        message.push_str(&format!(" Có {} cảnh báo.", warnings.len()));
    }
    let variables: Vec<Value> = columns
        .iter()
        .zip(&art.per_column)
        .map(|(c, n)| json!({"name": c.name, "kind": c.kind.label(), "column": c.index + 1, "bound": n}))
        .collect();
    let mut out = json!({
        "message": message,
        "variables": variables,
        "datasets": names,
        "bound": bound,
        "warnings": warnings,
        "dryRun": o.dry_run,
        "delimiter": delimiter_label(delim),
        "encoding": encoding,
    });
    if let Some(v) = applied
        && let Some(m) = out.as_object_mut()
    {
        m.insert("applied".into(), v.get("applied").cloned().unwrap_or(Value::Null));
        m.insert("skipped".into(), v.get("skipped").cloned().unwrap_or(Value::Null));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "dancard_variable_tests.rs"]
mod tests;
