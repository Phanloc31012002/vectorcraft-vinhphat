//! DanCard — tab Variable: xuất mỗi bản ghi (data set) thành một file, như Batch của bảng Actions
//! với nguồn Data Sets trên Illustrator (bảng Actions của VectorCraft chưa có bước này).
//!
//! - Mỗi bản ghi được áp lên một **bản sao** của tài liệu, như `dataset.select` áp (chữ, ẩn/hiện;
//!   object bị khóa hay nằm trong layer ẩn/khóa giữ nguyên), rồi cả các artboard lưu thành một file:
//!   PDF (nhiều trang khi nhiều artboard, cùng thiết lập với PDF của tab Dàn theo mẫu) hoặc `.ai`
//!   (sửa tiếp được, kèm phần PDF như file AI của tab CTL Offset). Tài liệu đang mở không đổi,
//!   không thêm bước hoàn tác.
//! - Tên file: tiền tố + tên bản ghi (cột chọn ở "Tên bản ghi theo cột" khi nhập, vd. cột số
//!   nhảy), ký tự Windows không cho dùng đổi thành `-`; hai bản ghi ra trùng tên thì bản sau thêm
//!   ` (2)`, ` (3)`… Tên tính trên mọi bản ghi, nên xuất từng đoạn vẫn ra đúng tên đó.
//! - Không ghi đè file đã có, trừ khi bật `overwrite`: có file trùng tên thì báo và không ghi gì.
//! - Panel gọi lệnh này từng bản ghi một (`from` = `to`) để hiện tiến độ và dừng được giữa chừng;
//!   MCP / CLI gọi một lần cho cả đoạn.

use serde_json::{Value, json};
use vectorcraft_doc::{DataSet, DataValue, Document, NodeId, VariableKind};

use crate::cmd::*;
use crate::{EngineError, Result, Session};

const C: &str = "dancard.exportVariables";
/// Đường dẫn file dài tối đa (ký tự), dưới giới hạn 260 của Windows.
const MAX_PATH: usize = 250;
/// Số tên liệt kê trong một thông báo.
const LISTED: usize = 5;

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        query "dancard.exportVariables",
        "DanCard: Xuất file theo bản ghi Variable",
        [],
        None,
        "{folder: thư mục lưu (phải có sẵn; dryRun thì không cần), format?: pdf|ai (mặc định pdf), prefix?: chữ đứng trước tên bản ghi trong tên file, from?: bản ghi đầu (số thứ tự từ 1; mặc định 1), to?: bản ghi cuối (mặc định bản ghi cuối), overwrite?: bool (ghi đè file trùng tên; mặc định false: có file trùng thì báo và không ghi gì), dryRun?: bool (chỉ trả tên file và các file đã có, không ghi)} áp từng bản ghi Variables lên một bản sao tài liệu (như dataset.select) rồi lưu cả các artboard thành một file: PDF (nhiều trang khi nhiều artboard) hoặc .ai; tên file = tiền tố + tên bản ghi; tài liệu đang mở không đổi → {message, files: [tên], saved, total, records, existing: [tên], errors}",
        has_doc,
        export_variables
    )]
}

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

/// Định dạng file xuất.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Pdf,
    Ai,
}

impl Format {
    fn parse(p: &Value) -> Result<Self> {
        match str_param(p, "format").map(|f| f.trim().to_ascii_lowercase()).as_deref() {
            None | Some("" | "pdf") => Ok(Self::Pdf),
            Some("ai") => Ok(Self::Ai),
            Some(f) => Err(other(format!("Định dạng “{f}” không hợp lệ: chọn pdf hoặc ai."))),
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Ai => "ai",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Ai => "AI",
        }
    }
}

/// Số thứ tự bản ghi `key` (từ 1): số, hoặc chữ số (ô của panel); trống thì `None`.
fn index(p: &Value, key: &str) -> Result<Option<usize>> {
    let bad = || other(format!("“{key}” phải là số thứ tự bản ghi, từ 1 trở lên."));
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().and_then(|v| usize::try_from(v).ok()).filter(|v| *v >= 1).map(Some).ok_or_else(bad),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse::<usize>().ok().filter(|v| *v >= 1).map(Some).ok_or_else(bad),
        Some(_) => Err(bad()),
    }
}

/// Phần tên file an toàn trên Windows: ký tự cấm và ký tự điều khiển thành `-`, bỏ dấu chấm và
/// khoảng trắng ở cuối (Windows tự cắt đi).
fn sanitize(s: &str) -> String {
    let s: String =
        s.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '-' } else { c }).collect();
    s.trim().trim_end_matches(['.', ' ']).to_string()
}

/// Tên thiết bị Windows (CON, NUL, COM1…): không dùng được làm tên file, dù có đuôi.
fn reserved(stem: &str) -> bool {
    let up = stem.split('.').next().unwrap_or(stem).trim().to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|d| up.strip_prefix(d).is_some_and(|n| n.len() == 1 && n.chars().all(|c| ('1'..='9').contains(&c))))
}

/// Tên file của mọi bản ghi (theo thứ tự): tiền tố + tên bản ghi, đã làm sạch, không trùng nhau
/// (không phân biệt hoa thường, như Windows).
fn file_names(sets: &[DataSet], prefix: &str, format: Format) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(sets.len());
    for (i, ds) in sets.iter().enumerate() {
        let mut stem = sanitize(&format!("{prefix}{}", ds.name));
        if stem.is_empty() {
            stem = format!("Record-{}", i + 1);
        }
        if reserved(&stem) {
            stem.push('_');
        }
        let mut name = format!("{stem}.{}", format.ext());
        let mut k = 2;
        while !seen.insert(name.to_lowercase()) {
            name = format!("{stem} ({k}).{}", format.ext());
            k += 1;
        }
        out.push(name);
    }
    out
}

/// Bản ghi `ds` áp lên tài liệu `d` như `dataset.select` (chữ thay bằng giá trị của bản ghi, ẩn/hiện
/// theo bản ghi; object bị khóa, nằm trong layer ẩn/khóa, hay giá trị không hợp loại biến thì giữ
/// nguyên) → (số chỗ đã áp, số chỗ bỏ qua).
fn apply_record(d: &mut Document, ds: &DataSet) -> (usize, usize) {
    // Biến theo thứ tự định nghĩa: object gắn hai biến chữ lấy giá trị của biến sau, như dataset.select.
    let mut pairs: Vec<(NodeId, String)> = vec![];
    for v in &d.variables.variables {
        pairs.extend(d.variables.objects_of(&v.name).into_iter().map(|id| (id, v.name.clone())));
    }
    let (mut applied, mut skipped) = (0, 0);
    for (id, var) in pairs {
        let kind = d.variables.variable(&var).map(|v| v.kind);
        let Some((kind, value)) = kind.zip(ds.values.get(&var).cloned()).filter(|(k, v)| v.fits(*k)) else {
            skipped += 1;
            continue;
        };
        if !d.variable_target(id) {
            skipped += 1;
            continue;
        }
        let done = match (kind, &value) {
            (VariableKind::Text, DataValue::Text(text)) => crate::cmd::textedit::set_plain_text(d, id, text).is_ok(),
            (VariableKind::Visibility, DataValue::Visible(show)) => d.node_mut(id).is_some_and(|n| {
                n.visible = *show;
                true
            }),
            _ => false,
        };
        if done {
            applied += 1;
        } else {
            skipped += 1;
        }
    }
    d.variables.active_dataset = Some(ds.name.clone());
    (applied, skipped)
}

/// File của bản ghi `ds` áp lên bản sao của `src`.
fn record_file(src: &Document, ds: &DataSet, format: Format) -> Result<Vec<u8>> {
    let mut d = src.clone();
    apply_record(&mut d, ds);
    match format {
        // Cùng thiết lập với PDF của tab Dàn theo mẫu.
        Format::Pdf => {
            let settings = json!({"compatibility": "1.4", "preserveEditing": false, "thumbnails": false, "fastWebView": true});
            crate::cmd::fileio::pdf::encode(C, &d, &settings).map(|(bytes, _)| bytes)
        }
        // Như file AI của tab CTL Offset: sửa tiếp được, nhúng file liên kết và profile màu.
        Format::Ai => crate::cmd::fileio::encode(&d, "ai", &json!({"compress": true, "includeLinked": true, "embedProfiles": true})),
    }
}

/// Liệt kê tối đa [`LISTED`] tên.
fn listed(names: &[String]) -> String {
    let more = names.len().saturating_sub(LISTED);
    let head = names.iter().take(LISTED).cloned().collect::<Vec<_>>().join(", ");
    if more > 0 { format!("{head} và {more} file nữa") } else { head }
}

fn export_variables(s: &mut Session, p: &Value) -> Result<Value> {
    let format = Format::parse(p)?;
    let prefix = str_param(p, "prefix").unwrap_or_default().to_string();
    let overwrite = bool_or(p, "overwrite", false);
    let dry = bool_or(p, "dryRun", false);
    let folder = str_param(p, "folder").map(str::trim).unwrap_or_default().to_string();
    let st = s.doc()?;
    let src = &st.doc;
    let sets = &src.variables.datasets;
    let n = sets.len();
    if n == 0 {
        return Err(other("Tài liệu chưa có bản ghi nào: nhập file dữ liệu ở tab Variable trước."));
    }
    let from = index(p, "from")?.unwrap_or(1);
    let to = index(p, "to")?.unwrap_or(n);
    if from > n || to > n {
        return Err(other(format!("Tài liệu có {n} bản ghi: chọn số thứ tự từ 1 đến {n}.")));
    }
    if from > to {
        return Err(other(format!("Bản ghi đầu ({from}) phải nhỏ hơn hoặc bằng bản ghi cuối ({to}).")));
    }
    if folder.is_empty() && !dry {
        return Err(other("Chưa chọn thư mục lưu. Hãy bấm \"Chọn…\" để chọn thư mục."));
    }
    let names = file_names(sets, &prefix, format);
    let plan: Vec<(&DataSet, &String)> = sets.iter().zip(&names).skip(from - 1).take(to + 1 - from).collect();
    let files: Vec<String> = plan.iter().map(|(_, name)| (*name).clone()).collect();
    if let Some(long) = files.iter().find(|name| folder.chars().count() + 1 + name.chars().count() > MAX_PATH) {
        return Err(other(format!("Đường dẫn quá dài cho file “{long}”: chọn thư mục ngắn hơn hoặc rút gọn tiền tố / tên bản ghi.")));
    }
    let total = files.len();
    // Các file đã có trong thư mục (chỉ biết khi có thư mục, trên máy).
    #[cfg(not(target_arch = "wasm32"))]
    let existing: Vec<String> =
        if folder.is_empty() { vec![] } else { files.iter().filter(|f| std::path::Path::new(&folder).join(f).exists()).cloned().collect() };
    #[cfg(target_arch = "wasm32")]
    let existing: Vec<String> = vec![];
    if dry {
        let mut message = format!("{total} file {} sẽ được lưu: {}.", format.label(), listed(&files));
        if !existing.is_empty() {
            let (k, list) = (existing.len(), listed(&existing));
            message.push_str(&if overwrite {
                format!("\n{k} file đã có sẽ bị ghi đè: {list}.")
            } else {
                format!("\n{k} file đã có trong thư mục: {list}. Bật \"Ghi đè file trùng tên\" hoặc chọn thư mục khác.")
            });
        }
        return Ok(
            json!({"message": message, "files": files, "saved": 0, "total": total, "records": n, "existing": existing, "errors": [], "dryRun": true}),
        );
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (&plan, src, overwrite);
        Err(other("Bản web không lưu được file vào thư mục."))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = std::path::Path::new(&folder);
        if !dir.is_dir() {
            return Err(other(format!("Thư mục lưu không mở được: {folder}\nHãy bấm \"Chọn…\" để chọn đúng thư mục.")));
        }
        if !overwrite && !existing.is_empty() {
            return Err(other(format!(
                "{} file đã có trong thư mục: {}.\nKhông ghi gì. Bật \"Ghi đè file trùng tên\" hoặc chọn thư mục khác.",
                existing.len(),
                listed(&existing)
            )));
        }
        let mut saved = 0;
        let mut errors = vec![];
        for (ds, name) in &plan {
            let path = dir.join(name.as_str()).to_string_lossy().into_owned();
            let written = record_file(src, ds, format).and_then(|bytes| {
                if overwrite { crate::cmd::fileio::write_file(&path, &bytes) } else { crate::cmd::fileio::write_new_file(&path, &bytes) }
            });
            match written {
                Ok(()) => saved += 1,
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        let range = if total == n {
            String::new()
        } else if from == to {
            format!(" (bản ghi {from})")
        } else {
            format!(" (bản ghi {from}–{to})")
        };
        let mut message = format!("Đã xuất {saved}/{total} file {}{range}.\nThư mục: {folder}", format.label());
        if !errors.is_empty() {
            message.push_str(&format!("\nLỗi:\n- {}", errors.join("\n- ")));
        }
        if saved == 0 {
            return Err(other(message));
        }
        Ok(json!({"message": message, "files": files, "saved": saved, "total": total, "records": n, "existing": existing, "errors": errors}))
    }
}

#[cfg(test)]
#[path = "dancard_varexport_tests.rs"]
mod tests;
