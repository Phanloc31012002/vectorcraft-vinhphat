//! DanCard fork (IN Vĩnh Phát): images in a print file's editing data that the reader would leave
//! out, so that the file opens with its groups and every image in it.
//!
//! - An image [`super::super::dancard_compact`] kept as a JPEG file, when the data was too large to
//!   hold, and the note it writes at the top of such data.
//! - An image of more pixels than [`MAX_PIXELS`] (a large photo placed small, as print files have
//!   them) at a lower resolution, its CMYK inks kept as the reader keeps them at full size.
//! - [`read_trusting`]: the editing data read past what the reader can't draw yet, an object's
//!   opacity mask (which [`super::read`] names, so the file comes from its PDF part, its groups
//!   lost): the object is drawn whole, as before the reader named masks, with a note. For a file
//!   opened trusting its editing data whatever it draws (DanCard keeps the groups).

use vectorcraft_doc::ImageBlob;
use vectorcraft_doc::cmyk::Inks;

use super::{MAX_PIXELS, MAX_SOURCE_PIXELS, Reader, Structure, cmyk_rgb};

/// [`super::read`], an object's opacity mask passed over (the object drawn whole) with a note
/// rather than an error.
pub fn read_trusting(data: &[u8], large_canvas: bool) -> Result<Structure, String> {
    let mut r = Reader::new(data);
    r.large_canvas = large_canvas.then_some(1.0);
    r.dancard_masks = Some(0);
    r.run()?;
    let masks = r.dancard_masks.unwrap_or(0);
    let mut read = r.finish()?;
    if masks > 0 {
        read.warnings.insert(
            0,
            format!(
                "DanCard: {masks} object có opacity mask (mặt nạ độ trong) VectorCraft chưa đọc được từ bản chỉnh sửa: \
                 object hiện cả phần mask che, group giữ nguyên; xem kỹ trước khi in."
            ),
        );
    }
    Ok(read)
}

impl Reader<'_> {
    /// An object's opacity mask, read trusting the editing data ([`read_trusting`]): counted, and
    /// the object drawn whole → whether it was (otherwise the mask is named, as the reader does).
    pub(super) fn dancard_mask(&mut self) -> bool {
        let Some(n) = self.dancard_masks.as_mut() else { return false };
        *n += 1;
        true
    }

    /// An image `dancard_compact` kept as a JPEG file (`data`), placed by the matrix `m` as samples
    /// of its size are.
    pub(super) fn jpeg_image(&mut self, m: &[f64], data: &[u8]) -> Result<(), String> {
        let size = image::ImageReader::with_format(std::io::Cursor::new(data), image::ImageFormat::Jpeg).into_dimensions().ok();
        let size = size.filter(|(w, h)| *w > 0 && *h > 0 && u64::from(*w) * u64::from(*h) <= MAX_SOURCE_PIXELS);
        let (Some(size), [_, _, _, _, _, _]) = (size, m) else {
            self.warn("an image VectorCraft can't read was left out");
            return Ok(());
        };
        self.add_raster(ImageBlob::new("image/jpeg", data.to_vec()), None, m, size)
    }

    /// An image of `w` × `h` pixels, more than [`MAX_PIXELS`], at a lower resolution: each pixel
    /// the mean of a block of the image's, CMYK samples kept as ink amounts. `channels`, `one_bit`,
    /// `samples` and `alpha` (one byte a pixel) are as `image_from` reads them, `m` the matrix.
    pub(super) fn lowered_image(
        &mut self,
        m: &[f64],
        (w, h): (usize, usize),
        (channels, one_bit): (usize, bool),
        samples: &[u8],
        alpha: Option<&[u8]>,
    ) -> Result<(), String> {
        let [a, b, c, d, tx, ty] = m else { return Ok(()) };
        let mut step = 1usize;
        while (w.div_ceil(step) as u64).saturating_mul(h.div_ceil(step) as u64) > MAX_PIXELS {
            step += 1;
        }
        self.warn("very large images came in at a lower resolution");
        let (ow, oh) = (w.div_ceil(step), h.div_ceil(step));
        let row = if one_bit { (w * channels).div_ceil(8) } else { w * channels };
        let inks = channels == 4 && !one_bit;
        let mut out = Vec::with_capacity(ow * oh * 4);
        let mut opacity = Vec::with_capacity(if inks && alpha.is_some() { ow * oh } else { 0 });
        // A row of blocks: each one's four channels and alpha summed, and its pixels counted.
        let mut sums = vec![[0u32; 6]; ow];
        for oy in 0..oh {
            sums.iter_mut().for_each(|s| *s = [0; 6]);
            for y in oy * step..((oy + 1) * step).min(h) {
                let line = samples.get(y * row..(y + 1) * row).unwrap_or_default();
                for x in 0..w {
                    let s = |c: usize| {
                        let i = x * channels + c;
                        if one_bit { line.get(i / 8).map_or(0, |b| ((b >> (7 - i % 8)) & 1) * 255) } else { line.get(i).copied().unwrap_or(0) }
                    };
                    let px = match channels {
                        _ if inks => [s(0), s(1), s(2), s(3)],
                        1 => [s(0), s(0), s(0), 0],
                        4 => {
                            let key = [s(0), s(1), s(2), s(3)];
                            if self.cmyk_cache.len() > 1 << 16 {
                                self.cmyk_cache.clear();
                            }
                            let [r, g, b] = *self.cmyk_cache.entry(key).or_insert_with(|| cmyk_rgb(key));
                            [r, g, b, 0]
                        }
                        _ => [s(0), s(1), s(2), 0],
                    };
                    let a = alpha.and_then(|o| o.get(y * w + x)).copied().unwrap_or(255);
                    if let Some(sum) = sums.get_mut(x / step) {
                        for (t, v) in sum.iter_mut().zip([px[0], px[1], px[2], px[3], a, 1]) {
                            *t += u32::from(v);
                        }
                    }
                }
            }
            for s in &sums {
                let n = s[5].max(1);
                let mean = |v: u32| u8::try_from((v + n / 2) / n).unwrap_or(u8::MAX);
                if inks {
                    out.extend_from_slice(&[mean(s[0]), mean(s[1]), mean(s[2]), mean(s[3])]);
                    if alpha.is_some() {
                        opacity.push(mean(s[4]));
                    }
                } else {
                    out.extend_from_slice(&[mean(s[0]), mean(s[1]), mean(s[2]), mean(s[4])]);
                }
            }
        }
        // Each pixel stands for a block of the image's: the matrix scaled by the block's size.
        let (sx, sy) = (w as f64 / ow as f64, h as f64 / oh as f64);
        let m = [a * sx, b * sx, c * sy, d * sy, *tx, *ty];
        let size = (ow as u32, oh as u32);
        let blob = if inks {
            Inks::new(size.0, size.1, out).and_then(|i| ImageBlob::cmyk_tiff(&i))
        } else {
            image::RgbaImage::from_raw(size.0, size.1, out).and_then(|img| {
                let mut png = vec![];
                img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
                Some(ImageBlob::new("image/png", png))
            })
        };
        let Some(blob) = blob else {
            self.warn("an image VectorCraft can't read was left out");
            return Ok(());
        };
        self.add_raster(blob, alpha.filter(|_| inks).map(|_| opacity.as_slice()), &m, size)
    }

    /// The note `dancard_compact` writes at the top of data too large to hold, compacted as it
    /// decompressed (`LOWERED`, the resolution images came down to, 0 for none, and 1 when they
    /// are JPEG files) → whether `line` is that note, told in the warnings.
    pub(super) fn dancard_note(&mut self, line: &str) -> bool {
        let Some(rest) = line.strip_prefix(super::super::dancard_compact::LOWERED) else { return false };
        let mut it = rest.split_whitespace();
        let ppi: f64 = it.next().and_then(|v| v.parse().ok()).filter(|v: &f64| v.is_finite()).unwrap_or(0.0);
        let what = match (it.next() == Some("1"), ppi > 0.0) {
            (true, false) => "ảnh lớn trong file được giữ đủ độ phân giải, lưu dạng JPEG chất lượng cao".to_string(),
            (true, true) => format!("ảnh lớn trong file được lưu dạng JPEG chất lượng cao, ảnh quá mịn giảm còn khoảng {ppi} ppi trên trang"),
            (false, _) => format!("ảnh trong file được giảm còn khoảng {ppi} ppi trên trang"),
        };
        self.warn(&format!(
            "DanCard: file rất nặng (bản chỉnh sửa giải nén ra hơn {} MB) nên {what} để mở được cả file; group giữ nguyên.",
            super::super::MAX_DATA >> 20
        ));
        true
    }
}
