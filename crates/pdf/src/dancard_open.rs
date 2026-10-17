//! DanCard fork (IN Vĩnh Phát): opening a heavy `.ai` or PDF faster.
//!
//! - [`ChecksOnce`]: the safety checks of opening a PDF ([`crate::pages`]: its file's streams, then
//!   every object of it, twice) are done once for a file while it is being read, however many
//!   times it is opened meanwhile. Reading a `.ai` opens it six times or more (the document it may
//!   carry, File Info, its page, the editor's data, the dates of both…), and for a file of 200 MB
//!   each round of checks takes seconds.
//! - [`ImportOptions::dancard_image_boxes`](crate::ImportOptions::dancard_image_boxes): a page read
//!   only to be compared with the editor's copy of its art, and for its type, has its images as
//!   boxes ([`Boxed`]: grey, see-through for an image with its own transparency), not decoded and
//!   stored again; [`boxed`] draws a document's images as the grey boxes, to compare it with such a
//!   page, and the comparison passes over the see-through ones ([`clear_box_areas`]).

use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};

use vectorcraft_doc::{Document, ImageBlob, Node, NodeKind};

/// The files being read, each with the checks done on it so far (see [`ChecksOnce`]).
static CHECKED: Mutex<Vec<Checked>> = Mutex::new(Vec::new());

/// The checks of the file at `at` (its first byte's address) of `len` bytes.
struct Checked {
    at: usize,
    len: usize,
    /// How many [`ChecksOnce`] hold it.
    holders: usize,
    /// Would the reader fail on one of its streams (`crate::streams::unsafe_file_streams`)?
    streams: Arc<OnceLock<bool>>,
    /// What is hostile in it, opened with each password (`crate::pages::hostile`, twice).
    hostile: Vec<(String, Arc<OnceLock<Option<&'static str>>>)>,
}

fn checked() -> MutexGuard<'static, Vec<Checked>> {
    CHECKED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn key(bytes: &[u8]) -> (usize, usize) {
    (bytes.as_ptr() as usize, bytes.len())
}

/// While this is alive, the safety checks of opening the PDF `bytes` are done once, and every
/// open of `bytes` meanwhile (on any thread) uses what they found. It borrows `bytes`, so they are
/// the same bytes for as long as it lives: their address and length name them.
pub struct ChecksOnce<'a> {
    bytes: &'a [u8],
}

impl<'a> ChecksOnce<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let (at, len) = key(bytes);
        let mut all = checked();
        match all.iter_mut().find(|c| (c.at, c.len) == (at, len)) {
            Some(c) => c.holders += 1,
            None => all.push(Checked { at, len, holders: 1, streams: Arc::default(), hostile: vec![] }),
        }
        Self { bytes }
    }
}

impl Drop for ChecksOnce<'_> {
    fn drop(&mut self) {
        let (at, len) = key(self.bytes);
        let mut all = checked();
        if let Some(i) = all.iter().position(|c| (c.at, c.len) == (at, len)) {
            let gone = all.get_mut(i).is_some_and(|c| {
                c.holders = c.holders.saturating_sub(1);
                c.holders == 0
            });
            if gone {
                all.swap_remove(i);
            }
        }
    }
}

/// `check()` (whether the reader would fail on a stream of `bytes`), done once while a
/// [`ChecksOnce`] holds `bytes`: the threads asking meanwhile wait for the first.
pub(crate) fn streams_once(bytes: &[u8], check: impl FnOnce() -> bool) -> bool {
    let (at, len) = key(bytes);
    let cell = checked().iter().find(|c| (c.at, c.len) == (at, len)).map(|c| c.streams.clone());
    match cell {
        Some(cell) => *cell.get_or_init(check),
        None => check(),
    }
}

/// `check()` (what is hostile in `bytes` opened with `password`), done once while a
/// [`ChecksOnce`] holds `bytes`.
pub(crate) fn hostile_once(bytes: &[u8], password: &str, check: impl FnOnce() -> Option<&'static str>) -> Option<&'static str> {
    let (at, len) = key(bytes);
    let cell = checked().iter_mut().find(|c| (c.at, c.len) == (at, len)).map(|c| match c.hostile.iter().find(|(p, _)| p == password) {
        Some((_, cell)) => cell.clone(),
        None => {
            let cell = Arc::<OnceLock<Option<&'static str>>>::default();
            c.hostile.push((password.to_string(), cell.clone()));
            cell
        }
    });
    match cell {
        Some(cell) => *cell.get_or_init(check),
        None => check(),
    }
}

/// The box an image of a page read with `dancard_image_boxes` becomes: one pixel, which an image
/// object stretches over its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Boxed {
    /// An opaque image: middle grey ([`box_image`]).
    Grey = 1,
    /// An image with its own transparency: see-through, and passed over by the comparison
    /// ([`clear_box_areas`]): what an effect drawn as such an image draws is VectorCraft's own
    /// drawing of the effect on the other side.
    Clear = 2,
    /// An image an alpha mask draws: white (the mask hides nothing, as the editor's copy of the art,
    /// read past its opacity masks, doesn't).
    White = 3,
}

impl Boxed {
    pub(crate) fn image(self) -> &'static ImageBlob {
        static BOXES: LazyLock<[ImageBlob; 3]> = LazyLock::new(|| [[128, 128, 128, 255], [0, 0, 0, 0], [255, 255, 255, 255]].map(pixel));
        let [grey, clear, white] = &*BOXES;
        match self {
            Boxed::Grey => grey,
            Boxed::Clear => clear,
            Boxed::White => white,
        }
    }
}

/// A PNG of one pixel of `rgba`.
fn pixel(rgba: [u8; 4]) -> ImageBlob {
    let mut png = vec![];
    // Encoding one pixel can't fail; were it to, the box is an empty blob (drawn as nothing, on both
    // sides of the comparison alike).
    let _ = image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    ImageBlob::new("image/png", png)
}

/// The box an opaque image of a page read with `dancard_image_boxes` is: middle grey, opaque.
pub fn box_image() -> &'static ImageBlob {
    Boxed::Grey.image()
}

/// Is `blob` one of the boxes images become ([`Boxed`])?
fn is_box(blob: &ImageBlob) -> bool {
    [Boxed::Grey, Boxed::Clear, Boxed::White].iter().any(|b| b.image().bytes == blob.bytes)
}

/// Has `doc` (a page) images as boxes: was it read with `dancard_image_boxes`?
pub fn has_image_boxes(doc: &Document) -> bool {
    doc.images.values().any(is_box)
}

/// Where the images with their own transparency of `doc` (a page read with
/// `dancard_image_boxes`) are: the frames of their see-through boxes, which a comparison passes
/// over.
pub fn clear_box_areas(doc: &Document) -> Vec<kurbo::Rect> {
    let clear = Boxed::Clear.image();
    let mut out = vec![];
    doc.walk(|n| {
        if let NodeKind::Image(im) = &n.kind
            && doc.images.get(&im.key).is_some_and(|b| b.bytes == clear.bytes)
            && let Some(b) = n.geometric_bounds()
        {
            out.push(b);
        }
    });
    out
}

/// `doc` with its images drawn as [`box_image`] boxes, as on a page read with
/// `dancard_image_boxes` (an image's own transparency isn't read: its opacity mask goes too).
pub fn boxed(doc: &Document) -> Document {
    fn unmask(nodes: &mut [Arc<Node>]) {
        for n in nodes {
            if matches!(n.kind, NodeKind::Image(_)) {
                if n.extras().mask.is_some() {
                    Arc::make_mut(n).extras_mut().mask = None;
                }
            } else if n.children().is_some_and(|c| c.iter().any(|c| has_masked_image(c)))
                && let Some(c) = Arc::make_mut(n).children_mut()
            {
                unmask(c);
            }
        }
    }
    fn has_masked_image(n: &Node) -> bool {
        match &n.kind {
            NodeKind::Image(_) => n.extras().mask.is_some(),
            _ => n.children().is_some_and(|c| c.iter().any(|c| has_masked_image(c))),
        }
    }
    let mut d = doc.clone();
    let grey = box_image();
    for b in d.images.values_mut() {
        *b = grey.clone();
    }
    unmask(&mut d.layers);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_are_kept_while_the_file_is_held() {
        let a = b"%PDF-1.7 one".to_vec();
        let b = b"%PDF-1.7 two".to_vec();
        let mut runs = 0;
        // Not held: each open checks.
        assert!(!streams_once(&a, || {
            runs += 1;
            false
        }));
        assert!(!streams_once(&a, || {
            runs += 1;
            false
        }));
        assert_eq!(runs, 2);
        {
            let _held = ChecksOnce::new(&a);
            let _again = ChecksOnce::new(&a);
            for _ in 0..3 {
                assert!(streams_once(&a, || {
                    runs += 1;
                    true
                }));
            }
            assert_eq!(runs, 3, "once while held");
            assert_eq!(hostile_once(&a, "", || Some("x")), Some("x"));
            assert_eq!(hostile_once(&a, "", || None), Some("x"), "the same password: what was found");
            assert_eq!(hostile_once(&a, "pw", || None), None, "another password: checked again");
            // Another file isn't held.
            assert!(!streams_once(&b, || false));
            assert!(streams_once(&b, || true));
        }
        // Let go: checked again.
        assert!(!streams_once(&a, || false));
        assert!(checked().iter().all(|c| c.len != a.len() || c.at != a.as_ptr() as usize));
    }

    fn png(img: image::DynamicImage) -> Vec<u8> {
        let mut out = vec![];
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    /// A page read with `dancard_image_boxes` has each image as a box of one pixel stretched over
    /// its frame (the same place and size, nothing decoded): grey when opaque, see-through when the
    /// image has its own transparency (and then an area the comparison passes over).
    #[test]
    fn a_page_read_with_image_boxes_has_its_images_as_boxes_of_their_frames() {
        use kurbo::{Affine, Rect};
        use vectorcraft_doc::ImageObject;
        let mut d = Document::new(200.0, 200.0);
        let opaque = png(image::RgbImage::from_fn(4, 2, |x, _| image::Rgb([if x < 2 { 255 } else { 0 }, 0, 0])).into());
        let clear = png(image::RgbaImage::from_fn(4, 2, |x, _| image::Rgba([0, 0, 255, if x == 3 { 0 } else { 255 }])).into());
        d.images.insert("opaque".into(), ImageBlob::new("image/png", opaque));
        d.images.insert("clear".into(), ImageBlob::new("image/png", clear));
        let l = d.layers[0].id;
        for (i, key) in ["opaque", "clear"].into_iter().enumerate() {
            let im = ImageObject {
                key: key.into(),
                width: 4,
                height: 2,
                xf: Affine::translate((20.0, 30.0 + 50.0 * i as f64)) * Affine::scale(10.0),
                link: None,
                placement: Default::default(),
            };
            let id = d.alloc_id();
            d.insert(Some(l), i, Node::new(id, NodeKind::Image(Box::new(im)))).unwrap();
        }
        let pdf = crate::export(&d, &crate::PdfOptions::default()).unwrap();
        let images = |doc: &Document| {
            let mut found = vec![];
            doc.walk(|n| {
                if let NodeKind::Image(im) = &n.kind {
                    let kind = [Boxed::Grey, Boxed::Clear, Boxed::White]
                        .into_iter()
                        .find(|b| doc.images.get(&im.key).is_some_and(|x| x.bytes == b.image().bytes));
                    found.push(((im.width, im.height), n.geometric_bounds().unwrap(), kind));
                }
            });
            found.sort_by(|a, b| a.1.y0.total_cmp(&b.1.y0));
            found
        };
        let whole = images(&crate::import_with_report(&pdf, &crate::ImportOptions::default()).unwrap().document);
        let read = crate::import_with_report(&pdf, &crate::ImportOptions { dancard_image_boxes: true, ..Default::default() }).unwrap().document;
        let boxes = images(&read);
        assert_eq!((whole.len(), boxes.len()), (2, 2), "{whole:?} {boxes:?}");
        let near =
            |a: Rect, b: Rect| (a.x0 - b.x0).abs() < 0.01 && (a.y0 - b.y0).abs() < 0.01 && (a.x1 - b.x1).abs() < 0.01 && (a.y1 - b.y1).abs() < 0.01;
        assert_eq!((whole[0].0, whole[0].2, whole[1].2), ((4, 2), None, None));
        assert_eq!((boxes[0].0, boxes[0].2), ((1, 1), Some(Boxed::Grey)), "the opaque image");
        assert_eq!((boxes[1].0, boxes[1].2), ((1, 1), Some(Boxed::Clear)), "the one with transparency");
        assert!(near(whole[0].1, boxes[0].1) && near(whole[1].1, boxes[1].1), "{whole:?} {boxes:?}");
        assert!(has_image_boxes(&read));
        let areas = clear_box_areas(&read);
        assert!(areas.len() == 1 && near(areas[0], whole[1].1), "{areas:?}");
    }

    #[test]
    fn a_boxed_document_draws_its_images_as_grey_boxes_without_their_masks() {
        use kurbo::Affine;
        use vectorcraft_doc::{ImageObject, OpacityMask};
        let mut d = Document::new(100.0, 100.0);
        let l = d.layers[0].id;
        d.images.insert("photo".into(), ImageBlob::new("image/jpeg", vec![1, 2, 3]));
        let image = |id| {
            Node::new(
                id,
                NodeKind::Image(Box::new(ImageObject {
                    key: "photo".into(),
                    width: 40,
                    height: 30,
                    xf: Affine::IDENTITY,
                    link: None,
                    placement: Default::default(),
                })),
            )
        };
        let (a, m, g, b) = (d.alloc_id(), d.alloc_id(), d.alloc_id(), d.alloc_id());
        let mut masked = image(a);
        masked.extras_mut().mask = Some(Box::new(OpacityMask::new(image(m), true)));
        d.insert(Some(l), 0, Node::group(g, vec![Arc::new(masked), Arc::new(image(b))])).unwrap();
        assert!(!has_image_boxes(&d));
        let boxed = boxed(&d);
        assert!(has_image_boxes(&boxed) && boxed.images.values().all(|b| b.bytes == box_image().bytes));
        assert!(boxed.node(a).is_some_and(|n| n.extras().mask.is_none()), "the mask goes");
        assert!(d.node(a).is_some_and(|n| n.extras().mask.is_some()), "the document keeps it");
        assert_eq!(d.images["photo"].bytes.as_slice(), &[1, 2, 3]);
    }
}
