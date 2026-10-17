//! DanCard fork (IN Vĩnh Phát): mở file `.ai` / PDF nặng nhanh hơn.
//!
//! A `.ai` (or a PDF saved with the editor's copy of its art) whose editing copy DanCard trusts
//! (`dancard::trusts_editing_data`: always, unless the page was changed after it) opens from that
//! copy; its page (the PDF part) is read only for the type it sets and to compare the copy with
//! (the warning when VectorCraft draws it differently). So:
//!
//! - the page is read with its images as boxes (`vectorcraft_pdf::ImportOptions::dancard_image_boxes`):
//!   a print file's photos are neither decoded and stored again for it nor drawn to compare it
//!   (`vectorcraft_eps` draws the copy's images as the same boxes then);
//! - the editing copy is decoded and read (`vectorcraft_eps::read_editing`) on another thread
//!   while the page is read.
//!
//! When the copy can't be used after all, the file is read again as VectorCraft reads it (its page
//! with its images), so what opens is never a page of boxes.

use vectorcraft_doc::Document;

use super::super::pdfimport;
use crate::Result;
use crate::cmd::fileio::LoadOptions;

/// The stack of the thread that reads the editing copy (its art is read by recursion too).
#[cfg(not(target_arch = "wasm32"))]
const STACK: usize = 64 << 20;

/// The `.ai` or PDF `bytes` read as [`super::load_with`] reads it, faster, when it has an editing
/// copy DanCard trusts and that copy can be used: its document and notes (DanCard's first, as
/// `dancard::note_kept_groups` puts them). `None` otherwise: the caller reads it as VectorCraft does.
/// `editor_document`: a `.ai` file.
pub(super) fn read_trusted(bytes: &[u8], opts: &LoadOptions, editor_document: bool) -> Option<Result<(Document, Vec<String>)>> {
    if opts.pages.is_some() || !opts.layers || !opts.editing_data {
        return None;
    }
    let password = opts.password.as_deref();
    if !vectorcraft_pdf::has_illustrator_data(bytes, password) {
        return None;
    }
    let (trust, _) = crate::cmd::dancard::trusts_editing_data(bytes, opts);
    if !trust {
        return None;
    }
    let page = || pdfimport::import_with_image_boxes(bytes, opts, editor_document);
    let editing = || vectorcraft_pdf::illustrator_data(bytes, password).and_then(|private| vectorcraft_eps::read_editing(&private, trust));
    let (page, editing) = both(page, editing)?;
    // A page that can't be read (a placeholder page…), or of text alone: as VectorCraft reads it.
    let page = page.ok().filter(|r| !r.only_text)?;
    let outlined = opts.text_as == vectorcraft_pdf::TextAs::Outlines;
    let (doc, mut warnings, read) = vectorcraft_eps::layered_from(editing?, page.document, page.warnings, outlined, trust, page.user_unit);
    // Not read from the editing copy, or with a box of the page in it: the page with its images.
    if !read || vectorcraft_pdf::has_image_boxes(&doc) {
        return None;
    }
    crate::cmd::dancard::note_kept_groups(&mut warnings);
    // The layers hold the art that the PDF part leaves out (as `through_editing_data`).
    warnings.retain(|w| w != vectorcraft_pdf::OFF_ARTBOARD_NOTE && w != vectorcraft_pdf::PAST_PAGE_NOTE);
    Some(Ok((doc, warnings)))
}

/// `a()` and `b()`, `b` on a thread of its own while `a` runs: `None` when that thread can't start
/// or stops on a bug (the caller then reads the file as VectorCraft does, on its own thread). On the
/// web (no threads) one after the other.
fn both<A, B: Send>(a: impl FnOnce() -> A, b: impl FnOnce() -> B + Send) -> Option<(A, B)> {
    #[cfg(target_arch = "wasm32")]
    {
        let x = a();
        Some((x, b()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|s| {
        let other = std::thread::Builder::new().name("dancard-editing-copy".into()).stack_size(STACK).spawn_scoped(s, b).ok()?;
        let x = a();
        other.join().ok().map(|y| (x, y))
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_doc::{Document, NodeKind};
    use vectorcraft_testkit::ai;

    use crate::Session;

    /// A 2 × 2 RGB image (red, green / blue, white) over `(10, 50)`–`(40, 90)` and a black square,
    /// the editing copy's art (on a page of `width` × 100 pt), and a page of 200 × 100 pt drawing the
    /// same.
    fn card(width: f64) -> Vec<u8> {
        let raster = "%AI5_File:\n%AI5_BeginRaster\n() 1 XG\n/DeviceRGB XN\n[ 15 0 0 20 10 90 ] 2 2 0 Xh\n\
                      [ 15 0 0 20 10 90 ] 0 0 2 2 2 2 8 3 0 0 0 0 4 4 0 0\n%%BeginData: 24\nXI\nFF000000FF000000FFFFFFFF\n\
                      %%EndData\nXH\n%AI5_EndRaster\nN\n";
        let art = format!("0 Ae\nu\n{raster}0 g\n{}f\nU\n{}", ai::rect(120.0, 10.0, 160.0, 50.0), ai::named("Card"));
        let data = ai::editing_data(width, 100.0, &ai::layer("Art", &art));
        let page = "q 30 0 0 40 10 50 cm BI /W 2 /H 2 /CS /RGB /BPC 8 /F /AHx ID FF000000FF000000FFFFFFFF> EI Q 0 g 120 10 40 40 re f";
        ai::ai(data.as_bytes(), page)
    }

    fn open(s: &mut Session, bytes: &[u8]) -> serde_json::Value {
        s.execute("document.open", &json!({"name": "card.ai", "dataBase64": vectorcraft_format::base64_encode(bytes)})).unwrap()
    }

    /// An image's size in pixels and its four corner pixels.
    type Seen = ((u32, u32), Vec<[u8; 4]>);

    /// The images of `doc` ([`Seen`]).
    fn images(doc: &Document) -> Vec<Seen> {
        let mut out = vec![];
        doc.walk(|n| {
            if let NodeKind::Image(im) = &n.kind {
                let blob = doc.images.get(&im.key).expect("blob");
                assert_ne!(blob.bytes, vectorcraft_pdf::box_image().bytes, "a box of the page");
                let img = image::load_from_memory(&blob.bytes).expect("decodes").to_rgba8();
                let (w, h) = img.dimensions();
                let corners = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)].map(|(x, y)| img.get_pixel(x, y).0);
                out.push(((im.width, im.height), corners.to_vec()));
            }
        });
        out
    }

    const PIXELS: [[u8; 4]; 4] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];

    /// Mở nhanh: ảnh của trang chỉ là ô để so, ảnh trong file mở ra là ảnh của bản chỉnh sửa, group
    /// giữ nguyên, và hình hai bên không khác (ô xám ở cả hai bên).
    #[test]
    fn mo_nhanh_anh_trong_file_la_anh_cua_ban_chinh_sua() {
        let mut s = Session::new();
        let r = open(&mut s, &card(200.0));
        let doc = &s.doc().unwrap().doc;
        let art = doc.layers[0].children().unwrap();
        assert_eq!(doc.layers[0].name.as_deref(), Some("Art"), "{r}");
        let card = art.iter().find(|n| n.name.as_deref() == Some("Card")).expect("group Card");
        assert!(matches!(&card.kind, NodeKind::Group { children, .. } if children.len() == 2), "{:?}", card.kind);
        assert_eq!(images(doc), [((2, 2), PIXELS.to_vec())]);
        let warnings = r["warnings"].as_array().cloned().unwrap_or_default();
        assert!(warnings.iter().all(|w| !w.as_str().unwrap_or_default().contains("khác trang PDF")), "{r}");
    }

    /// Bản chỉnh sửa không dùng được (của một trang khác cỡ): file mở theo trang PDF với ảnh thật
    /// của trang, không phải ô xám.
    #[test]
    fn ban_chinh_sua_khong_dung_duoc_thi_mo_theo_trang_du_anh() {
        let mut s = Session::new();
        let r = open(&mut s, &card(300.0));
        let doc = &s.doc().unwrap().doc;
        assert!(r.to_string().contains("different page"), "{r}");
        assert_eq!(images(doc), [((2, 2), PIXELS.to_vec())], "{r}");
    }
}
