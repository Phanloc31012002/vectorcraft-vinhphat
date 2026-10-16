//! DanCard fork (IN Vĩnh Phát): is the editor's copy of the art that a `.ai` or a PDF carries
//! older than its page?
//!
//! A page records when its contents last changed (its `LastModified`), and each program's data
//! under `PieceInfo` when that data last changed (ISO 32000-1 §14.5, page-piece dictionaries): a
//! program that changes the page and not the data leaves the page the later of the two, the data
//! no longer describing it. A file saved by the editor has both dates the same.

use craft_pdf::syntax::object::{Dict, String as PdfString};

use crate::pages::{PRIVATE_DATA_OWNER, open};

/// Was the first page changed after the editor's copy of the art it carries was written: is its
/// `LastModified` later than that of the editor's data dictionary under `PieceInfo`? `false` when
/// the file can't be read, has no such data, or either date is missing or isn't a date.
pub fn page_newer_than_editing_data(bytes: &[u8], password: Option<&str>) -> bool {
    let Ok(pdf) = open(bytes, password) else { return false };
    let Some(page) = pdf.pages().first() else { return false };
    let date = |d: &Dict<'_>| d.get::<PdfString<'_>>(b"LastModified").and_then(|s| seconds(s.as_bytes()));
    let Some(data) = page.raw().get::<Dict<'_>>(b"PieceInfo").and_then(|p| p.get::<Dict<'_>>(PRIVATE_DATA_OWNER)) else { return false };
    matches!((date(page.raw()), date(&data)), (Some(page), Some(data)) if page > data)
}

/// A date `D:YYYYMMDDHHmmSSOHH'mm` (ISO 32000-1 §7.9.4: what follows the year can be left out,
/// `O` is `+`, `-` or `Z`) as seconds from 1970 in universal time; `None` when it isn't one.
fn seconds(date: &[u8]) -> Option<i64> {
    let s = std::str::from_utf8(date).ok()?.trim();
    let s = s.strip_prefix("D:").unwrap_or(s);
    // The number of `len` digits at `at`; `default` when the date ends before it.
    let field = |at: usize, len: usize, default: i64| -> Option<i64> {
        match s.get(at..at + len) {
            Some(p) if p.bytes().all(|b| b.is_ascii_digit()) => p.parse().ok(),
            Some(_) => None,
            None if s.len() <= at => Some(default),
            None => None,
        }
    };
    let year = field(0, 4, 0).filter(|_| s.len() >= 4)?;
    let (month, day) = (field(4, 2, 1)?, field(6, 2, 1)?);
    let (hour, minute, second) = (field(8, 2, 0)?, field(10, 2, 0)?, field(12, 2, 0)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // The offset from universal time: `+HH'mm`, `-HH'mm` or `Z`.
    let offset = match s.as_bytes().get(14) {
        None | Some(b'Z') => 0,
        Some(sign @ (b'+' | b'-')) => {
            let rest = s.get(15..)?.replace('\'', "");
            let hours: i64 = rest.get(0..2).unwrap_or("0").parse().ok()?;
            let minutes: i64 = rest.get(2..4).filter(|m| !m.is_empty()).unwrap_or("0").parse().ok()?;
            let off = hours * 3600 + minutes * 60;
            if *sign == b'+' { off } else { -off }
        }
        Some(_) => return None,
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days from 1970-01-01 to the date in the proleptic Gregorian calendar (H. Hinnant's
/// `days_from_civil`, <https://howardhinnant.github.io/date_algorithms.html>, public domain).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-page PDF whose page has the dates `page` and, under `PieceInfo`, `data` (each an
    /// entry such as `/LastModified (D:2026)`, or nothing).
    fn dated(page: &str, data: &str) -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] {page} \
                 /PieceInfo << /Illustrator << {data} /Private << /AIPrivateData1 4 0 R >> >> >> >>"
            ),
            "<< /Length 4 >>\nstream\nart.\nendstream".to_string(),
        ];
        let mut out = b"%PDF-1.5\n".to_vec();
        let mut offsets = vec![];
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
        for off in offsets {
            out.extend(format!("{off:010} 00000 n \n").bytes());
        }
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).bytes());
        out
    }

    #[test]
    fn dates_are_read_in_universal_time() {
        assert_eq!(seconds(b"D:19700101000000Z"), Some(0));
        assert_eq!(seconds(b"D:1970"), Some(0));
        assert_eq!(seconds(b"D:19700102"), Some(86_400));
        // 07:00 at +07'00' is midnight in universal time; 10:00 at -02'30 is 12:30.
        assert_eq!(seconds(b"D:19700101070000+07'00'"), Some(0));
        assert_eq!(seconds(b"D:19700101100000-02'30"), Some(12 * 3600 + 30 * 60));
        assert_eq!(seconds(b"D:20000301"), Some(days_from_civil(2000, 3, 1) * 86_400));
        assert_eq!(days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28), 2, "2000 is a leap year");
        for bad in [&b"D:20261345"[..], b"yesterday", b"D:20", b"D:2026x1", b"D:20260101120000?"] {
            assert_eq!(seconds(bad), None, "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn a_page_changed_after_its_editing_data_is_newer_than_it() {
        let (at, later) = ("/LastModified (D:20261002101134+08'00')", "/LastModified (D:20261002111134+08'00')");
        assert!(page_newer_than_editing_data(&dated(later, at), None));
        // Saved together, or the data the newer: not.
        assert!(!page_newer_than_editing_data(&dated(at, at), None));
        assert!(!page_newer_than_editing_data(&dated(at, later), None));
        // The same time in another zone: not.
        assert!(!page_newer_than_editing_data(&dated("/LastModified (D:20261002021134Z)", at), None));
        // Without a date, or with one that isn't: not.
        assert!(!page_newer_than_editing_data(&dated(later, ""), None));
        assert!(!page_newer_than_editing_data(&dated("", at), None));
        assert!(!page_newer_than_editing_data(&dated("/LastModified (soon)", at), None));
        assert!(!page_newer_than_editing_data(b"not a PDF", None));
    }
}
