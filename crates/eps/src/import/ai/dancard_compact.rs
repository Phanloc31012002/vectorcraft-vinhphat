//! DanCard fork (IN Vĩnh Phát): editing data too large to hold whole, decompressed with its images
//! kept as JPEG files (or, where that isn't enough, at a lower resolution).
//!
//! A catalogue of photos can carry hundreds of MB of compressed editing data that decompresses to
//! several GB (323 MB → 8.2 GB, nearly all of it the images' samples): more than
//! [`super::MAX_DATA`], so the file opened from its PDF part alone (empty when its art lies
//! outside the artboards), without its layers and groups. Here such data is decompressed as a
//! stream, twice: once to learn its images ([`Pass::scan`]), then to keep everything but the images'
//! samples as it is and write each image back as [`plan`] says ([`Pass::write`]): a large image
//! without an alpha channel as a JPEG file at quality [`JPEG_QUALITY`] (CMYK samples as a CMYK JPEG:
//! the ink amounts kept), which the reader takes as it is; others as samples. Where even that
//! doesn't fit, images come down to the highest resolution that does, each pixel the mean of the
//! block of pixels it stands for: the images with the most pixels to the inch on the page first, to
//! one resolution all of them share. The image's operands (size, matrix: the same place) and its
//! `%%BeginData:` count are rewritten for what is written.
//!
//! The sections are found as the lexer finds them (`lex.rs`): a `%%BeginData: n` comment at a line's
//! start, `XI` after it, the samples up to `%%EndData` near the `n` bytes counted from the end of
//! the comment's line; placed files' previews and `%%BeginDocument` sections are copied whole.
//! Only images of 8-bit samples written as binary, outside the copies for newer readers (`%_`), are
//! written anew; anything else is copied as it is.

use std::io::{self, Read};

use super::{Codec, MAX_FRAMES, MAX_ZSTD_WINDOW, damaged, too_large};
use crate::import::lex::find;

/// Most pixels an image is written at (the reader reads no more: `read::MAX_PIXELS`).
const MAX_PIXELS: u64 = 1 << 25;
/// The lowest resolution (pixels to the inch on the page) images are taken down to.
const MIN_PPI: f64 = 36.0;
/// Bytes read from the decompressor at a time.
const CHUNK: usize = 1 << 20;
/// Largest section read at once (an image's samples are held while they are lowered).
const MAX_SECTION: usize = 1 << 30;
/// What an image's rewritten operands and comment add, at most (bytes).
const OVERHEAD: u64 = 512;
/// The quality images are kept as JPEG files at.
const JPEG_QUALITY: u8 = 92;
/// How much smaller than their samples JPEG files are taken to be, for a plan: first as they
/// mostly are (at the quality above, photos come out 8 to 15 times smaller), then less, should
/// they come out larger.
const JPEG_RATIOS: [u64; 2] = [12, 6];
/// Images of fewer bytes of samples stay samples (a JPEG file of a small logo saves little).
#[cfg(not(test))]
const JPEG_MIN_BYTES: u64 = 256 << 10;
#[cfg(test)]
const JPEG_MIN_BYTES: u64 = 1;
/// Largest side of a JPEG file.
const JPEG_MAX_SIDE: u64 = 65_535;
/// The value of the image operands' `binary` (else 0 for hex, 1 for binary) that says the samples
/// are a JPEG file, as this module writes them (read by `read::image`).
pub(super) const JPEG_FILE: f64 = 2.0;

const PREVIEW: (&[u8], &[u8]) = (b"%AI26_BeginPlacedObjectPreview", b"%AI26_EndPlacedObjectPreview");
const DOCUMENT: (&[u8], &[u8]) = (b"%%BeginDocument", b"%%EndDocument");
const END_DATA: &[u8] = b"%%EndData";

/// `data` (compressed with `codec`) decompressed, its images kept as JPEG files or lowered to fit
/// in `max` bytes; the error of data too large when even the lowest resolution doesn't fit.
pub(super) fn decompress(codec: Codec, data: &[u8], max: u64) -> Result<Vec<u8>, String> {
    let mut scan = Pass::scan();
    scan.run(reader(codec, data))?;
    let budget = max.saturating_sub(max / 8);
    // As JPEG files first; when they come out larger than the plan took them to be, a plan that
    // takes them to be larger; then as samples.
    let ratios = JPEG_RATIOS.iter().map(|r| Some(*r)).chain([None]);
    for ratio in ratios {
        let jpeg = ratio.is_some();
        let Some((ppi, plan)) = plan(&scan.images, scan.len, budget, ratio) else { continue };
        let lowered = scan.images.iter().any(|(n, i)| plan.get(*n).copied().flatten().is_some_and(|w| w.size() != (i.w, i.h)));
        let as_jpeg = plan.iter().any(|p| matches!(p, Some(Write::Jpeg(..))));
        let mut write = Pass::write(plan, max);
        // The reader says so (after the first line, the header's).
        write.note = (lowered || as_jpeg).then(|| format!("%{LOWERED} {} {}\n", if lowered { ppi.round() } else { 0.0 }, u8::from(as_jpeg)));
        match write.run(reader(codec, data)) {
            Ok(()) => return Ok(write.out),
            Err(e) if jpeg && e == too_large() => continue,
            Err(e) => return Err(e),
        }
    }
    Err(too_large())
}

/// The comment that says the images were lowered, and to about how many pixels to the inch.
pub(super) const LOWERED: &str = "DanCard_ImagesLowered:";

fn reader<'a>(codec: Codec, data: &'a [u8]) -> Box<dyn Read + 'a> {
    match codec {
        Codec::Zlib => Box::new(flate2::read::ZlibDecoder::new(data)),
        Codec::Zstd => {
            // Frames one after the other, padded with zeros.
            let end = data.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
            Box::new(ZstdFrames { rest: data.get(..end).unwrap_or_default(), current: None, frames: 0 })
        }
    }
}

/// Zstandard frames one after the other, read as one stream.
struct ZstdFrames<'a> {
    rest: &'a [u8],
    current: Option<ruzstd::decoding::StreamingDecoder<&'a [u8], ruzstd::decoding::FrameDecoder>>,
    frames: usize,
}

impl Read for ZstdFrames<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if let Some(d) = self.current.as_mut() {
                let n = d.read(buf)?;
                if n > 0 || buf.is_empty() {
                    return Ok(n);
                }
                // The frame is done: on to the next.
                if let Some(d) = self.current.take() {
                    self.rest = d.into_inner();
                }
            }
            if self.rest.is_empty() {
                return Ok(0);
            }
            self.frames += 1;
            if self.frames > MAX_FRAMES {
                return Err(io::Error::other("too many frames"));
            }
            let d = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(self.rest, MAX_ZSTD_WINDOW)
                .map_err(|e| io::Error::other(e.to_string()))?;
            self.current = Some(d);
        }
    }
}

/// An image a pass found: its size, colour samples a pixel and alpha channel, its section's samples'
/// length, and how many pixels to the inch it has on the page, across and down.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Image {
    w: u64,
    h: u64,
    channels: u64,
    alpha: bool,
    /// Its alpha channel is opaque inside its outermost pixels (pages exported with one: only the
    /// last column, which the page's width cuts through, is partly see-through), or it has none.
    opaque: bool,
    samples: u64,
    ppi: (f64, f64),
}

impl Image {
    /// Bytes of samples at `w` × `h`.
    fn bytes(&self, w: u64, h: u64) -> u64 {
        w.saturating_mul(h).saturating_mul(self.channels + u64::from(self.alpha))
    }

    /// Kept as a JPEG file (in a plan that keeps them so): large, opaque (but for its outermost
    /// pixels, which then show whole: a pixel's width at the image's edge).
    fn jpegable(&self) -> bool {
        self.opaque && matches!(self.channels, 1 | 3 | 4) && self.samples >= JPEG_MIN_BYTES
    }

    /// The size it is written at for a resolution of at most `ppi` (and [`MAX_PIXELS`], and sides
    /// a JPEG file can have).
    fn at(&self, ppi: f64) -> (u64, u64) {
        let scale = |n: u64, have: f64| if have > ppi { ((n as f64) * ppi / have).ceil().max(1.0) as u64 } else { n };
        let (mut w, mut h) = (scale(self.w, self.ppi.0), scale(self.h, self.ppi.1));
        if w.saturating_mul(h) > MAX_PIXELS {
            let f = (MAX_PIXELS as f64 / (w as f64 * h as f64)).sqrt();
            (w, h) = (((w as f64) * f).floor().max(1.0) as u64, ((h as f64) * f).floor().max(1.0) as u64);
        }
        if w.max(h) > JPEG_MAX_SIDE {
            let f = JPEG_MAX_SIDE as f64 / w.max(h) as f64;
            (w, h) = (((w as f64) * f).floor().max(1.0) as u64, ((h as f64) * f).floor().max(1.0) as u64);
        }
        (w.min(self.w), h.min(self.h))
    }
}

/// How an image is written: its samples at a size, or a JPEG file of that size.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Write {
    Samples(u64, u64),
    Jpeg(u64, u64),
}

impl Write {
    fn size(&self) -> (u64, u64) {
        match *self {
            Self::Samples(w, h) | Self::Jpeg(w, h) => (w, h),
        }
    }
}

/// The highest resolution at which all the data (`total` bytes as it is) fits in `budget`, found by
/// halving, and how each image (by its index among the sections) is written there, `None` as it is:
/// with a `jpeg` ratio, large images as JPEG files (taken to be that many times smaller than their
/// samples). `None` when even [`MIN_PPI`] doesn't fit.
fn plan(images: &[(usize, Image)], total: u64, budget: u64, jpeg: Option<u64>) -> Option<(f64, Sizes)> {
    let kept: u64 = images.iter().map(|(_, i)| i.samples).sum();
    let other = total.saturating_sub(kept);
    let size = |ppi: f64| -> u64 {
        images
            .iter()
            .map(|(_, i)| {
                let (w, h) = i.at(ppi);
                let bytes = i.bytes(w, h).min(i.samples);
                OVERHEAD + jpeg.filter(|_| i.jpegable()).map_or(bytes, |ratio| bytes / ratio.max(1))
            })
            .fold(other, u64::saturating_add)
    };
    let top = images.iter().map(|(_, i)| i.ppi.0.max(i.ppi.1)).fold(MIN_PPI, f64::max);
    if size(MIN_PPI) > budget {
        return None;
    }
    let ppi = if size(top) <= budget {
        top
    } else {
        let (mut lo, mut hi) = (MIN_PPI, top);
        for _ in 0..48 {
            let mid = (lo + hi) / 2.0;
            if size(mid) <= budget { lo = mid } else { hi = mid }
        }
        lo
    };
    let count = images.iter().map(|(n, _)| n + 1).max().unwrap_or(0);
    let mut out = vec![None; count];
    for (n, i) in images {
        let (w, h) = i.at(ppi);
        let write = if jpeg.is_some() && i.jpegable() { Some(Write::Jpeg(w, h)) } else { ((w, h) != (i.w, i.h)).then_some(Write::Samples(w, h)) };
        if let Some(slot) = out.get_mut(*n) {
            *slot = write;
        }
    }
    Some((ppi, out))
}

/// How each image (by its index among the sections) is written, `None` as it is.
type Sizes = Vec<Option<Write>>;

/// One pass over the decompressed data.
struct Pass {
    /// Writing (else only learning the images).
    write: bool,
    out: Vec<u8>,
    /// Bytes the data has (scanning) or written (writing).
    len: u64,
    max: u64,
    /// The images that can be lowered, by their index among the sections.
    images: Vec<(usize, Image)>,
    plan: Sizes,
    sections: usize,
    /// A line to write after the first.
    note: Option<String>,
}

impl Pass {
    fn scan() -> Self {
        Self { write: false, out: vec![], len: 0, max: u64::MAX, images: vec![], plan: vec![], sections: 0, note: None }
    }

    fn write(plan: Sizes, max: u64) -> Self {
        Self { write: true, out: vec![], len: 0, max, images: vec![], plan, sections: 0, note: None }
    }

    fn emit(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.len = self.len.saturating_add(bytes.len() as u64);
        if self.write {
            if self.len > self.max {
                return Err(too_large());
            }
            self.out.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn run(&mut self, src: Box<dyn Read + '_>) -> Result<(), String> {
        let mut input = Input { src, buf: vec![], pos: 0, eof: false };
        let mut line = vec![];
        // The last line with something on it: where it starts in what is written, and its text.
        let mut last: Option<(usize, Vec<u8>)> = None;
        // In a raster section, the colour space its `XN` names.
        let (mut raster, mut space) = (false, None::<String>);
        while input.line(&mut line).map_err(|_| damaged("its compression"))? {
            let text = trim_eol(&line);
            let unhidden = text.strip_prefix(b"%_").unwrap_or(text);
            if text.starts_with(b"%AI5_BeginRaster") || text.starts_with(b"%AI5_EndRaster") {
                raster = text.starts_with(b"%AI5_BeginRaster");
                space = None;
            } else if raster && let Some(s) = xn_space(unhidden) {
                space = Some(s);
            }
            if let Some(n) = begin_data(unhidden) {
                let hidden = unhidden.len() != text.len();
                if self.section(&mut input, &line, n, hidden, last.take(), space.as_deref())? {
                    continue;
                }
            }
            let at = self.out.len();
            self.emit(&line)?;
            if let Some((_, end)) = [PREVIEW, DOCUMENT].iter().find(|(b, _)| unhidden.starts_with(b)) {
                // Copied whole, up to its end marker (read as the line after).
                let raw = input.until(end).map_err(|_| damaged("its compression"))?;
                self.emit(&raw)?;
                last = None;
                continue;
            }
            if !text.is_empty() {
                last = Some((at, text.to_vec()));
            }
            if let Some(note) = self.note.take() {
                self.emit(note.as_bytes())?;
            }
        }
        Ok(())
    }

    /// The section after the `%%BeginData: n` line `line` (on `%_` lines: `hidden`), the line before
    /// it `last` (where it was written, its text) → false when it isn't one (no `XI` after it: the
    /// line is a comment like any other).
    fn section(
        &mut self,
        input: &mut Input<'_>,
        line: &[u8],
        n: usize,
        hidden: bool,
        last: Option<(usize, Vec<u8>)>,
        space: Option<&str>,
    ) -> Result<bool, String> {
        let bad = |_| damaged("its compression");
        if n > MAX_SECTION {
            return Err(too_large());
        }
        // `XI` (`%_XI` on a hidden object's lines) after blanks; the samples after its line's end.
        let mut region = vec![];
        let skip = loop {
            let skip = region.iter().take_while(|b| is_space(**b)).count();
            if region.len() >= skip + 6 || !input.more(&mut region, 64).map_err(bad)? {
                break skip;
            }
        };
        let rest = region.get(skip..).unwrap_or_default();
        let hidden_xi = rest.starts_with(b"%_XI");
        if !hidden_xi && !rest.starts_with(b"XI") {
            input.unread(&region);
            return Ok(false);
        }
        let mut data = skip + if hidden_xi { 4 } else { 2 };
        match (region.get(data), region.get(data + 1)) {
            (Some(b'\r'), Some(b'\n')) => data += 2,
            (Some(b'\r' | b'\n'), _) => data += 1,
            _ => {}
        }
        // The count leaves out an alpha channel: the samples end where `%%EndData` starts, near the
        // count's end.
        while region.len() < n {
            let want = (n - region.len()).min(CHUNK);
            if !input.more(&mut region, want).map_err(bad)? {
                break;
            }
        }
        let lo = n.saturating_sub(4).max(data);
        let hi = n.saturating_add(n.saturating_mul(2)).saturating_add(64);
        let mut from = lo;
        let end = loop {
            if let Some(i) = region.get(from..region.len().min(hi)).and_then(|w| find(w, END_DATA)) {
                break from + i;
            }
            from = region.len().saturating_sub(END_DATA.len()).max(lo);
            let want = CHUNK.min(hi.saturating_sub(region.len()));
            if region.len() >= hi || !input.more(&mut region, want).map_err(bad)? {
                break n.min(region.len()).max(data);
            }
        };
        input.unread(region.get(end..).unwrap_or_default());
        region.truncate(end);
        let index = self.sections;
        self.sections += 1;
        let samples = region.get(data..).unwrap_or_default();
        // An image that can be lowered: its operands on the line before, not a copy for newer readers.
        let image = last
            .as_ref()
            .filter(|_| !hidden && !hidden_xi)
            .and_then(|(_, text)| Operands::read(text))
            .and_then(|op| op.image(space, samples.len()).map(|i| (op, i)));
        if !self.write {
            if let Some((_, mut i)) = image {
                i.opaque = !i.alpha || opaque_inside(samples, &i);
                self.images.push((index, i));
            }
            self.emit(line)?;
            self.emit(&region)?;
            return Ok(true);
        }
        let image = image.map(|(op, mut i)| {
            i.opaque = !i.alpha || opaque_inside(samples, &i);
            (op, i)
        });
        let written = match (image, self.plan.get(index).copied().flatten(), last.as_ref()) {
            (Some((op, i)), Some(write), Some((at, _))) => rewrite(samples, &op, &i, write).map(|(o, c, b)| (o, c, b, *at)),
            _ => None,
        };
        match written {
            Some((operands, counted, bytes, at)) => {
                // The operands' line (and the line ends after it) written again.
                let dropped = self.out.len().saturating_sub(at);
                self.out.truncate(at);
                self.len = self.len.saturating_sub(dropped as u64);
                self.emit(operands.as_bytes())?;
                self.emit(format!("\n%%BeginData: {}\nXI\n", counted + 3).as_bytes())?;
                self.emit(&bytes)?;
            }
            None => {
                self.emit(line)?;
                self.emit(&region)?;
            }
        }
        Ok(true)
    }
}

/// The image `i` (its operands `op`, its `samples`) written as `write` says → its operands, the
/// bytes its `%%BeginData:` count counts (all but an alpha channel), and its bytes.
fn rewrite(samples: &[u8], op: &Operands, i: &Image, write: Write) -> Option<(String, usize, Vec<u8>)> {
    let (w, h) = write.size();
    match write {
        Write::Samples(..) => {
            let lowered = shrink(samples, i, w, h)?;
            Some((op.rewritten(i, w, h, false), lowered.color, lowered.bytes))
        }
        Write::Jpeg(..) => {
            let lowered;
            let color = if (w, h) == (i.w, i.h) {
                samples.get(..usize::try_from(i.w.checked_mul(i.h)?.checked_mul(i.channels)?).ok()?)?
            } else {
                lowered = shrink(samples, i, w, h)?;
                lowered.bytes.get(..lowered.color)?
            };
            let file = jpeg(color, i.channels, w, h)?;
            Some((op.rewritten(i, w, h, true), file.len(), file))
        }
    }
}

/// Is the alpha channel after the colour samples of `i` opaque inside its outermost pixels?
fn opaque_inside(samples: &[u8], i: &Image) -> bool {
    let (Ok(w), Ok(h), Ok(ch)) = (usize::try_from(i.w), usize::try_from(i.h), usize::try_from(i.channels)) else { return false };
    let Some(alpha) = w.checked_mul(h).and_then(|n| n.checked_mul(ch)).and_then(|color| samples.get(color..color.checked_add(w * h)?)) else {
        return false;
    };
    (1..h.saturating_sub(1)).all(|y| alpha.get(y * w + 1..(y + 1) * w - 1).is_some_and(|row| row.iter().all(|a| *a == 255)))
}

/// A JPEG file of `w` × `h` pixels of `channels` 8-bit samples (grey, RGB, or CMYK ink amounts).
fn jpeg(samples: &[u8], channels: u64, w: u64, h: u64) -> Option<Vec<u8>> {
    let kind = match channels {
        1 => jpeg_encoder::ColorType::Luma,
        3 => jpeg_encoder::ColorType::Rgb,
        4 => jpeg_encoder::ColorType::Cmyk,
        _ => return None,
    };
    let mut out = vec![];
    jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY).encode(samples, u16::try_from(w).ok()?, u16::try_from(h).ok()?, kind).ok()?;
    Some(out)
}

/// Samples lowered: the colour samples' length (the `%%BeginData:` count leaves out the alpha
/// channel), and all of them.
struct Lowered {
    color: usize,
    bytes: Vec<u8>,
}

/// An image's operands: `[a b c d tx ty] llx lly urx ury w h bits kind alpha r binary …`.
struct Operands {
    matrix: [f64; 6],
    nums: Vec<f64>,
}

impl Operands {
    fn read(line: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(line).ok()?.trim();
        let (m, nums) = s.strip_prefix('[')?.split_once(']')?;
        let parse = |t: &str| t.split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>().ok();
        let matrix: [f64; 6] = parse(m)?.try_into().ok()?;
        let nums = parse(nums)?;
        (nums.len() >= 11 && matrix.iter().chain(&nums).all(|v| v.is_finite())).then_some(Self { matrix, nums })
    }

    /// The image they describe, as the reader reads it, when it can be lowered (8-bit samples,
    /// binary, all there in `have` bytes).
    fn image(&self, space: Option<&str>, have: usize) -> Option<Image> {
        let [_, _, _, _, w, h, bits, kind, alpha, _, binary, ..] = self.nums.as_slice() else { return None };
        if *bits != 8.0 || *binary != 1.0 || !(1.0..=1e7).contains(w) || !(1.0..=1e7).contains(h) || w.fract() != 0.0 || h.fract() != 0.0 {
            return None;
        }
        let channels = match space.unwrap_or_default() {
            "DeviceGray" => 1,
            "DeviceCMYK" => 4,
            "DeviceRGB" => 3,
            _ => match *kind as i64 {
                1 => 1,
                4 => 4,
                _ => 3,
            },
        };
        let [a, b, c, d, ..] = self.matrix;
        let (sx, sy) = (a.hypot(b), c.hypot(d));
        if !(sx > 1e-9 && sy > 1e-9) {
            return None;
        }
        let image = Image {
            w: *w as u64,
            h: *h as u64,
            channels,
            alpha: *alpha > 0.0,
            opaque: *alpha <= 0.0,
            samples: have as u64,
            ppi: (72.0 / sx, 72.0 / sy),
        };
        (image.bytes(image.w, image.h) <= have as u64).then_some(image)
    }

    /// The operands of the image `i` written at `w` × `h`: the matrix scaled so each pixel covers
    /// the block it stands for, the size and the bounds in the image's space; as a `jpeg` file, its
    /// `binary` [`JPEG_FILE`] and no alpha channel (a JPEG file has none).
    fn rewritten(&self, i: &Image, w: u64, h: u64, jpeg: bool) -> String {
        let (fx, fy) = (i.w as f64 / w as f64, i.h as f64 / h as f64);
        let [a, b, c, d, tx, ty] = self.matrix;
        let m = [a * fx, b * fx, c * fy, d * fy, tx, ty];
        let mut nums = self.nums.clone();
        for (k, v) in [(0, 0.0), (1, 0.0), (2, w as f64), (3, h as f64), (4, w as f64), (5, h as f64)] {
            if let Some(slot) = nums.get_mut(k) {
                *slot = v;
            }
        }
        if jpeg {
            for (k, v) in [(8, 0.0), (10, JPEG_FILE)] {
                if let Some(slot) = nums.get_mut(k) {
                    *slot = v;
                }
            }
        }
        let num = |v: &f64| if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", *v as i64) } else { format!("{v}") };
        format!("[ {} ] {}", m.iter().map(num).collect::<Vec<_>>().join(" "), nums.iter().map(num).collect::<Vec<_>>().join(" "))
    }
}

/// The samples of `i` (rows of colour samples, then an alpha channel) at `w` × `h`, each pixel the
/// mean of the block it covers.
fn shrink(samples: &[u8], i: &Image, w: u64, h: u64) -> Option<Lowered> {
    let (sw, sh, ch) = (usize::try_from(i.w).ok()?, usize::try_from(i.h).ok()?, usize::try_from(i.channels).ok()?);
    let (w, h) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    if w == 0 || h == 0 || w > sw || h > sh {
        return None;
    }
    let color_len = sw.checked_mul(sh)?.checked_mul(ch)?;
    let mut bytes = plane(samples.get(..color_len)?, (sw, sh, ch), (w, h));
    let color = bytes.len();
    if i.alpha {
        let alpha = samples.get(color_len..color_len.checked_add(sw.checked_mul(sh)?)?)?;
        bytes.extend(plane(alpha, (sw, sh, 1), (w, h)));
    }
    Some(Lowered { color, bytes })
}

/// One plane of `ch` samples a pixel, `sw` × `sh`, at `w` × `h` (no larger).
fn plane(src: &[u8], (sw, sh, ch): (usize, usize, usize), (w, h): (usize, usize)) -> Vec<u8> {
    // The output column each input column falls in (u64: the product passes 32 bits).
    let to = |x: usize, n: usize, of: usize| ((x as u64).saturating_mul(n as u64) / (of.max(1) as u64)) as usize;
    let mut out = Vec::with_capacity(w * h * ch);
    let mut sums = vec![0u64; w * ch];
    let mut counts = vec![0u64; w];
    let mut y = 0;
    for oy in 0..h {
        sums.iter_mut().for_each(|s| *s = 0);
        counts.iter_mut().for_each(|c| *c = 0);
        while y < sh && to(y, h, sh) == oy {
            let row = src.get(y * sw * ch..(y + 1) * sw * ch).unwrap_or_default();
            for x in 0..sw {
                let ox = to(x, w, sw);
                if let Some(c) = counts.get_mut(ox) {
                    *c += 1;
                }
                for k in 0..ch {
                    if let Some(s) = sums.get_mut(ox * ch + k) {
                        *s += u64::from(row.get(x * ch + k).copied().unwrap_or(0));
                    }
                }
            }
            y += 1;
        }
        for (ox, n) in counts.iter().enumerate() {
            let n = (*n).max(1);
            for k in 0..ch {
                let s = sums.get(ox * ch + k).copied().unwrap_or(0);
                out.push(u8::try_from((s + n / 2) / n).unwrap_or(u8::MAX));
            }
        }
    }
    out
}

/// The decompressed data, read a line or a stretch at a time, with bytes read too far put back.
struct Input<'a> {
    src: Box<dyn Read + 'a>,
    buf: Vec<u8>,
    pos: usize,
    eof: bool,
}

impl Input<'_> {
    /// Up to `n` more bytes onto `out` → whether any came.
    fn more(&mut self, out: &mut Vec<u8>, n: usize) -> io::Result<bool> {
        if self.pos >= self.buf.len() && !self.fill()? {
            return Ok(false);
        }
        let have = self.buf.get(self.pos..).unwrap_or_default();
        let take = have.len().min(n.max(1));
        out.extend_from_slice(have.get(..take).unwrap_or_default());
        self.pos += take;
        Ok(take > 0)
    }

    /// The next line, up to and with its first line end byte, into `line` → false at the end.
    fn line(&mut self, line: &mut Vec<u8>) -> io::Result<bool> {
        line.clear();
        loop {
            let have = self.buf.get(self.pos..).unwrap_or_default();
            if let Some(i) = have.iter().position(|b| matches!(b, b'\r' | b'\n')) {
                line.extend_from_slice(have.get(..=i).unwrap_or_default());
                self.pos += i + 1;
                return Ok(true);
            }
            line.extend_from_slice(have);
            self.pos = self.buf.len();
            if !self.fill()? {
                return Ok(!line.is_empty());
            }
        }
    }

    /// The bytes up to `marker` (not it: it is read next), or to the end.
    fn until(&mut self, marker: &[u8]) -> io::Result<Vec<u8>> {
        let mut out = vec![];
        let mut from = 0;
        loop {
            if let Some(i) = out.get(from..).and_then(|w| find(w, marker)) {
                let at = from + i;
                self.unread(out.get(at..).unwrap_or_default());
                out.truncate(at);
                return Ok(out);
            }
            from = out.len().saturating_sub(marker.len());
            if !self.more(&mut out, CHUNK)? {
                return Ok(out);
            }
        }
    }

    /// Put `bytes` back, to be read next.
    fn unread(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let rest = self.buf.split_off(self.pos.min(self.buf.len()));
        self.buf.clear();
        self.buf.extend_from_slice(bytes);
        self.buf.extend(rest);
        self.pos = 0;
    }

    /// More bytes from the decompressor → false at its end.
    fn fill(&mut self) -> io::Result<bool> {
        if self.eof {
            return Ok(false);
        }
        self.buf.drain(..self.pos.min(self.buf.len()));
        self.pos = 0;
        let start = self.buf.len();
        self.buf.resize(start + CHUNK, 0);
        let n = loop {
            match self.src.read(self.buf.get_mut(start..).unwrap_or_default()) {
                Ok(n) => break n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.buf.truncate(start);
                    return Err(e);
                }
            }
        };
        self.buf.truncate(start + n);
        self.eof = n == 0;
        Ok(n > 0)
    }
}

fn trim_eol(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\n").or_else(|| line.strip_suffix(b"\r")).unwrap_or(line)
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

/// The count of a `%%BeginData: n` comment.
fn begin_data(text: &[u8]) -> Option<usize> {
    let n = text.strip_prefix(b"%%BeginData:")?;
    std::str::from_utf8(n).ok()?.split_whitespace().next()?.parse().ok()
}

/// The colour space a `/Name XN` line names.
fn xn_space(text: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(text).ok()?.trim();
    let name = s.strip_suffix("XN")?.trim().strip_prefix('/')?;
    (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use kurbo::Point;
    use vectorcraft_doc::NodeKind;

    use super::*;

    /// Editing data of a 300 × 200 page with an RGB image of `w` × `h` pixels placed over
    /// 10 × 5 pt at (20, 150) art space (its top left), with an alpha channel; the samples: a
    /// gradient across, alpha 255 left of the middle and 0 right of it, and a rectangle after it.
    fn data(w: u32, h: u32) -> Vec<u8> {
        data_with(w, h, ("DeviceRGB", 3), true)
    }

    /// [`data`] in the colour space `space` of `channels` samples a pixel (the gradient in the
    /// first, the second at 255 minus it, the others 0), with an alpha channel or not.
    fn data_with(w: u32, h: u32, (space, channels): (&str, u32), alpha: bool) -> Vec<u8> {
        let (sx, sy) = (10.0 / f64::from(w), 5.0 / f64::from(h));
        let mut s = format!(
            "%!PS-Adobe-3.0\n%%Creator: test\n%AI5_FileFormat 14.0\n%AI3_Cropmarks: 0 0 300 200\n%%EndComments\n%%BeginSetup\n%%EndSetup\n\
             %AI5_BeginLayer\n1 1 1 1 0 0 1 0 79 128 255 0 50 0 Lb\n(L) Ln\n0 A\n0 Xw\n%AI5_File:\n%AI5_BeginRaster\n() 1 XG\n/{space} XN\n\
             [ {sx} 0 0 {sy} 20 150 ] {w} {h} 0 Xh\n[ {sx} 0 0 {sy} 20 150 ] 0 0 {w} {h} {w} {h} 8 {channels} {} 0 1 0 4 4 0 0\n%%BeginData: {}\nXI\n",
            u8::from(alpha),
            w * h * channels + 3
        )
        .into_bytes();
        for _ in 0..h {
            for x in 0..w {
                let v = (x * 255 / (w - 1).max(1)) as u8;
                s.extend((0..channels).map(|c| match c {
                    0 => v,
                    1 => 255 - v,
                    _ => 0,
                }));
            }
        }
        if alpha {
            for _ in 0..h {
                s.extend((0..w).map(|x| if x < w / 2 { 255 } else { 0 }));
            }
        }
        s.extend_from_slice(b"%%EndData\nXH\n%AI5_EndRaster\nN\n0 g\n200 20 m\n200 60 L\n260 60 L\n260 20 L\n200 20 L\nf\nLB\n%AI5_EndLayer--\n%%PageTrailer\n%%EOF\n");
        s
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn image_of(data: &[u8]) -> (image::RgbaImage, kurbo::Affine, usize) {
        let doc = super::super::read(data, false).unwrap().doc;
        let art = doc.layers[0].children().unwrap().clone();
        let NodeKind::Image(im) = &art[0].kind else { panic!("{:?}", art[0].kind) };
        let png = image::load_from_memory(&doc.images.get(&im.key).unwrap().bytes).unwrap().to_rgba8();
        (png, im.xf, art.len())
    }

    #[test]
    fn data_too_large_comes_in_with_its_images_lowered_in_their_place() {
        // (Small: in the tests the reader itself lowers images of more than 4096 pixels.)
        let raw = data(64, 32);
        // As it is, under the limit: the same.
        let (full, full_xf, n) = image_of(&decompress(Codec::Zlib, &zlib(&raw), u64::MAX).unwrap());
        assert_eq!((full.dimensions(), n), ((64, 32), 2));
        // 8 KB of samples into 4 KB: lowered, the rectangle after it kept.
        let small = decompress(Codec::Zlib, &zlib(&raw), 4_000).unwrap();
        assert!(small.len() <= 4_000, "{}", small.len());
        let (img, xf, n) = image_of(&small);
        assert_eq!(n, 2, "the rectangle after the image");
        let warnings = super::super::read(&small, false).unwrap().warnings;
        assert!(warnings.iter().any(|w| w.starts_with("DanCard:") && w.contains("ppi")), "the reader says so: {warnings:?}");
        let (w, h) = img.dimensions();
        assert!(w < 64 && h < 32 && w >= 16 && (f64::from(w) / f64::from(h) - 2.0).abs() < 0.1, "{w} × {h}");
        // The same place on the page: its corners.
        for (p, q) in [(Point::ZERO, Point::ZERO), (Point::new(f64::from(w), f64::from(h)), Point::new(64.0, 32.0))] {
            assert!((xf * p - full_xf * q).hypot() < 1e-6, "{:?} {:?}", xf * p, full_xf * q);
        }
        // Each pixel the mean of its block: the gradient across, the alpha channel's edge.
        let (left, right) = (img.get_pixel(0, 0).0, img.get_pixel(w - 1, h - 1).0);
        assert!(left[0] < 12 && left[1] > 243 && left[3] == 255, "{left:?}");
        assert!(right[0] > 243 && right[3] == 0, "{right:?}");
    }

    #[test]
    fn large_images_without_an_alpha_channel_are_kept_as_jpeg_files() {
        for (space, channels) in [("DeviceRGB", 3), ("DeviceCMYK", 4)] {
            let raw = data_with(64, 32, (space, channels), false);
            let full = super::super::read(&raw, false).unwrap();
            let full_xf = image_of(&raw).1;
            // 6 to 8 KB of samples: in 5 KB the samples don't fit, a JPEG file of them does.
            let out = decompress(Codec::Zlib, &zlib(&raw), 5_000).unwrap();
            assert!(out.len() <= 5_000, "{}", out.len());
            let r = super::super::read(&out, false).unwrap();
            let art = r.doc.layers[0].children().unwrap().clone();
            assert_eq!(art.len(), full.doc.layers[0].children().unwrap().len(), "{space}: the rectangle after it");
            let NodeKind::Image(im) = &art[0].kind else { panic!("{:?}", art[0].kind) };
            let blob = r.doc.images.get(&im.key).unwrap();
            assert_eq!((blob.mime.as_str(), im.width, im.height, im.xf), ("image/jpeg", 64, 32, full_xf), "{space}: all of it, in its place");
            assert_eq!(blob.is_cmyk(), channels == 4, "{space}");
            if channels == 4 {
                // The ink amounts kept: cyan across, magenta the other way, no yellow or black.
                let inks = blob.cmyk().unwrap();
                let px = |x: usize| inks.data[x * 4..x * 4 + 4].to_vec();
                let (left, right) = (px(0), px(63));
                assert!(left[0] < 12 && left[1] > 243 && left[2] < 12 && left[3] < 12, "{left:?}");
                assert!(right[0] > 243 && right[1] < 12, "{right:?}");
            }
            assert!(r.warnings.iter().any(|w| w.starts_with("DanCard:") && w.contains("JPEG")), "{:?}", r.warnings);
        }
    }

    #[test]
    fn an_alpha_channel_see_through_only_at_the_edge_doesnt_keep_an_image_from_a_jpeg_file() {
        // A page exported with an alpha channel: opaque but for its last column.
        let mut raw = data_with(64, 32, ("DeviceCMYK", 4), true);
        let at = raw.windows(3).position(|w| w == b"XI\n").unwrap() + 3 + 64 * 32 * 4;
        for (k, a) in raw[at..at + 64 * 32].iter_mut().enumerate() {
            *a = if k % 64 == 63 { 128 } else { 255 };
        }
        let out = decompress(Codec::Zlib, &zlib(&raw), 5_000).unwrap();
        let r = super::super::read(&out, false).unwrap();
        let NodeKind::Image(im) = &r.doc.layers[0].children().unwrap()[0].kind else { panic!() };
        let blob = r.doc.images.get(&im.key).unwrap();
        assert_eq!((blob.mime.as_str(), im.width, blob.is_cmyk()), ("image/jpeg", 64, true));
        // See-through inside: samples, lowered.
        for (k, a) in raw[at..at + 64 * 32].iter_mut().enumerate() {
            *a = if k == 64 * 10 + 20 { 0 } else { 255 };
        }
        let out = decompress(Codec::Zlib, &zlib(&raw), 5_000).unwrap();
        let r = super::super::read(&out, false).unwrap();
        let NodeKind::Image(im) = &r.doc.layers[0].children().unwrap()[0].kind else { panic!() };
        assert_eq!(r.doc.images.get(&im.key).unwrap().mime, "image/png");
        assert!(im.width < 64, "{}", im.width);
    }

    #[test]
    fn what_cant_be_lowered_is_kept_as_it_is() {
        // The section of a hidden object's copy, hex samples, and data too large however low.
        let raw = data(40, 20);
        let hex = String::from_utf8_lossy(&raw).replace("8 3 1 0 1 0 4 4 0 0", "8 3 1 0 0 0 4 4 0 0").into_bytes();
        let out = decompress(Codec::Zlib, &zlib(&hex), 100_000).unwrap();
        assert_eq!(out, hex);
        assert!(decompress(Codec::Zlib, &zlib(&raw), 1_000).is_err());
    }

    #[test]
    fn damaged_data_is_an_error_or_reads_but_never_panics() {
        // Cut short anywhere, and bytes changed (counts, sizes, the matrix, the samples).
        let raw = data(48, 24);
        let mut cases: Vec<Vec<u8>> = (0..raw.len()).step_by(37).map(|n| raw[..n].to_vec()).collect();
        for at in (0..raw.len()).step_by(29) {
            for b in [b'9', b' ', b'\n', b'%', 0, 255] {
                let mut c = raw.clone();
                c[at] = b;
                cases.push(c);
            }
        }
        for c in cases {
            for max in [600, 3_000, 100_000] {
                if let Ok(out) = decompress(Codec::Zlib, &zlib(&c), max) {
                    assert!(out.len() as u64 <= max);
                    let _ = super::super::read(&out, false);
                }
            }
        }
    }

    #[test]
    fn the_plan_lowers_the_finest_images_first() {
        let i = |ppi: f64, w: u64| Image { w, h: w, channels: 4, alpha: false, opaque: true, samples: w * w * 4, ppi: (ppi, ppi) };
        let images = [(0, i(1200.0, 1000)), (2, i(300.0, 1000))];
        // Room for both at 600 ppi or so: the first lowered, the second as it is.
        let budget = 4_000_000 + 1_000_000 + 4 * OVERHEAD;
        let (ppi, plan) = plan(&images, 8_000_000, budget, None).unwrap();
        assert!((590.0..=610.0).contains(&ppi), "{ppi}");
        assert_eq!(plan.len(), 3);
        let (w, h) = plan[0].unwrap().size();
        assert!((480..=520).contains(&w) && w == h, "{w}");
        assert_eq!((plan[1], plan[2]), (None, None));
        assert!(matches!(plan[0], Some(Write::Samples(..))));
        assert!(super::plan(&images, 8_000_000, 1000, None).is_none());
        // As JPEG files taken to be 8 times smaller: both as they are, 1 MB in all.
        let (_, jpeg) = super::plan(&images, 8_000_000, 1_100_000, Some(8)).unwrap();
        assert_eq!((jpeg[0], jpeg[2]), (Some(Write::Jpeg(1000, 1000)), Some(Write::Jpeg(1000, 1000))));
    }

    #[test]
    fn several_zstd_frames_read_as_one_stream() {
        // Frames as the format writes them: `ruzstd` can't write one, so a raw frame of each byte
        // string by hand (magic, a header with the content size, one raw last block).
        let frame = |bytes: &[u8]| {
            let mut f = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, bytes.len() as u8];
            let header = ((bytes.len() as u32) << 3) | 1;
            f.extend_from_slice(&header.to_le_bytes()[..3]);
            f.extend_from_slice(bytes);
            f
        };
        let mut packed = frame(b"%AI5_ hello\n");
        packed.extend(frame(b"world\n"));
        packed.extend([0, 0, 0]);
        let mut out = vec![];
        reader(Codec::Zstd, &packed).read_to_end(&mut out).unwrap();
        assert_eq!(out, b"%AI5_ hello\nworld\n");
    }
}
