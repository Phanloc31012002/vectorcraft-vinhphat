//! DanCard fork (IN Vĩnh Phát): the backdrop. A heavy document renders on the worker
//! (`render_worker`) while the canvas shows its last frame moved into the new view, so what a pan
//! or a zoom out brings into view stayed empty until the new frame came, then popped in. The
//! backdrop is the whole art (artboards and everything on the pasteboard) rendered at a lower
//! resolution in the background: it shows at once wherever the frame doesn't reach, and the frame
//! for the new view then draws over it, sharp.
//!
//! - It is rendered in tiles on a thread of its own with a renderer of its own (the canvas's caches
//!   stay the canvas's), nearest the view first, one tile at a time and only while the canvas's
//!   worker has nothing to render: it never holds a frame up by more than one tile.
//! - It shows only the art the canvas shows: the same document, revision and view options (a tile
//!   of another revision is never shown), and only outside the rectangle the frame covers, so
//!   nothing is drawn twice (translucent art would darken).
//! - Only for documents whose frames take longer than [`HEAVY_MS`]; once the art has stayed the
//!   same for [`SETTLE_MS`] (a run of edits renders it once). Pixel Preview and a turned view do
//!   without; so does the web app (no threads).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use egui::{Color32, Shape, pos2};
use vectorcraft_doc::Document;
use vectorcraft_geom::{Affine, Point, Rect};
use vectorcraft_render::{RenderOptions, Renderer};

use super::Xf;
use crate::{CacheKey, VectorcraftApp, now_ms};

/// A document whose frames take longer than this (ms) gets a backdrop.
const HEAVY_MS: f64 = 16.0;
/// The art must have stayed the same this long (ms) before its backdrop is rendered.
const SETTLE_MS: f64 = 300.0;
/// Side of a tile (pixels).
const TILE: usize = 512;
/// The backdrop's largest side and pixel count, and its sharpest scale (pixels per point).
const MAX_SIDE: usize = 4096;
const MAX_PIXELS: f64 = 8.0e6;
const MAX_SCALE: f64 = 4.0;

/// Where the backdrop lies on the document and how it is cut into tiles.
#[derive(Clone, Debug, PartialEq)]
struct Layout {
    /// The document area the texture covers, and its pixels per document unit.
    bounds: Rect,
    scale: f64,
    /// The texture's size, and its tiles ([x, y, w, h] pixels), nearest the view first.
    size: [usize; 2],
    tiles: Vec<[usize; 4]>,
}

enum Msg {
    /// Lay out the backdrop of `doc` (rendered with `opts`; the view's centre `center`) and keep
    /// it for the tiles of run `run`.
    Start { run: u64, doc: Arc<Document>, opts: Box<RenderOptions>, max_side: usize, center: Point },
    /// Render tile `index` of run `run`.
    Tile { run: u64, index: usize },
}

enum Reply {
    Layout { run: u64, layout: Option<Layout> },
    Tile { run: u64, index: usize, size: [usize; 2], pixels: Vec<u8> },
}

/// The art the canvas rendered last: its key without the view, since when (ms), what to render it
/// with (the document until its backdrop starts: one closed since isn't kept), and the view's centre
/// then.
struct Latest {
    art: CacheKey,
    since: f64,
    doc: Option<Arc<Document>>,
    opts: RenderOptions,
    center: Point,
}

/// The canvas's backdrop ([`crate::CanvasCache::backdrop`]).
#[derive(Default)]
pub(crate) struct Backdrop {
    thread: Option<(Sender<Msg>, Receiver<Reply>)>,
    latest: Option<Latest>,
    /// The art in (or coming into) the texture, and its run.
    art: Option<CacheKey>,
    run: u64,
    layout: Option<Layout>,
    texture: Option<egui::TextureHandle>,
    /// The tiles in the texture.
    done: Vec<bool>,
    /// The next tile to ask for; a tile is being rendered.
    next: usize,
    busy: bool,
}

/// `key` without what only the view changes: the art it shows.
fn art(key: &CacheKey) -> CacheKey {
    CacheKey { interacting: false, zoom: 0.0, cx: 0.0, cy: 0.0, w: 0, h: 0, ppp: 0.0, rot: 0.0, pixel: None, ..key.clone() }
}

impl Backdrop {
    /// The canvas renders the art of `key`: `doc`, with `opts`.
    pub(crate) fn note(&mut self, key: &CacheKey, doc: &Arc<Document>, opts: &RenderOptions) {
        if key.interacting || key.pixel.is_some() {
            return;
        }
        let (art, center) = (art(key), Point::new(key.cx, key.cy));
        match &mut self.latest {
            Some(latest) if latest.art == art => latest.center = center,
            _ => self.latest = Some(Latest { art, since: now_ms(), doc: Some(doc.clone()), opts: opts.clone(), center }),
        }
    }

    /// Nothing kept (the art changed, and the document is light now).
    fn clear(&mut self) {
        (self.art, self.layout, self.texture, self.next, self.busy) = (None, None, None, 0, false);
        self.done.clear();
    }

    /// Render the backdrop of the latest art.
    fn start(&mut self, ctx: &egui::Context) {
        let Some(latest) = &mut self.latest else { return };
        let Some(doc) = latest.doc.take() else { return };
        let (art, opts, center) = (latest.art.clone(), Box::new(latest.opts.clone()), latest.center);
        if self.thread.is_none() {
            self.thread = spawn(ctx.clone());
        }
        let Some((tx, _)) = &self.thread else { return };
        self.run += 1;
        let max_side = ctx.input(|i| i.max_texture_side).clamp(TILE, MAX_SIDE);
        let sent = tx.send(Msg::Start { run: self.run, doc, opts, max_side, center }).is_ok();
        // Kept until the layout comes: a texture of the same size is drawn into again.
        let texture = self.texture.take();
        self.clear();
        self.texture = texture;
        if sent {
            self.art = Some(art);
        } else {
            // The thread ended: another one for the next art.
            self.thread = None;
        }
    }

    /// Take in what the thread rendered.
    fn receive(&mut self, ctx: &egui::Context) {
        loop {
            let Some((_, rx)) = &self.thread else { return };
            let reply = match rx.try_recv() {
                Ok(reply) => reply,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    // The thread ended: another one for the next art.
                    (self.thread, self.busy) = (None, false);
                    return;
                }
            };
            match reply {
                Reply::Layout { run, layout } if run == self.run => {
                    match layout {
                        Some(layout) => {
                            if self.texture.as_ref().is_none_or(|t| t.size() != layout.size) {
                                let blank = egui::ColorImage::filled(layout.size, Color32::TRANSPARENT);
                                self.texture = Some(ctx.load_texture("dancard-backdrop", blank, egui::TextureOptions::LINEAR));
                            }
                            self.done = vec![false; layout.tiles.len()];
                            self.layout = Some(layout);
                        }
                        // Nothing to draw.
                        None => (self.layout, self.texture) = (None, None),
                    }
                    (self.next, self.busy) = (0, false);
                }
                Reply::Tile { run, index, size, pixels } if run == self.run => {
                    self.busy = false;
                    if let (Some(texture), Some(layout)) = (&mut self.texture, &self.layout)
                        && let Some(&[x, y, w, h]) = layout.tiles.get(index)
                        && size == [w, h]
                        && pixels.len() == w * h * 4
                    {
                        texture.set_partial([x, y], egui::ColorImage::from_rgba_premultiplied([w, h], &pixels), egui::TextureOptions::LINEAR);
                        if let Some(done) = self.done.get_mut(index) {
                            *done = true;
                        }
                    }
                }
                // Of an earlier art.
                Reply::Layout { .. } | Reply::Tile { .. } => {}
            }
        }
    }
}

/// Each frame: take in the tiles rendered; once the art shown has settled, start its backdrop (a
/// heavy document's); ask for the next tile while the canvas's worker has nothing to render.
pub(super) fn pump(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let heavy = app.canvas.last_ms > HEAVY_MS;
    // No worker (the web app, or rendering on the UI thread asked for): no backdrop either.
    let Some(idle) = app.canvas.worker.as_ref().map(|w| w.pending.is_none()) else { return };
    let b = &mut app.canvas.backdrop;
    b.receive(ctx);
    if let Some(latest) = &b.latest
        && b.art.as_ref() != Some(&latest.art)
    {
        let waited = now_ms() - latest.since;
        if !heavy {
            b.clear();
        } else if waited >= SETTLE_MS {
            b.start(ctx);
        } else {
            ctx.request_repaint_after(Duration::from_millis((SETTLE_MS - waited).clamp(1.0, SETTLE_MS) as u64));
        }
    }
    let run = b.run;
    if idle
        && !b.busy
        && let (Some(layout), Some((tx, _))) = (&b.layout, &b.thread)
        && b.next < layout.tiles.len()
    {
        if tx.send(Msg::Tile { run, index: b.next }).is_ok() {
            b.next += 1;
            b.busy = true;
        } else {
            b.thread = None;
        }
    }
}

/// Draw the backdrop for the view `xf` of the art of `key` (the frame wanted), where the frame
/// the canvas shows (`app.canvas.texture`, rendered for `app.canvas.key`) doesn't reach.
pub(super) fn draw(app: &VectorcraftApp, painter: &egui::Painter, xf: &Xf, key: &CacheKey) {
    let b = &app.canvas.backdrop;
    let (Some(texture), Some(layout), Some(shown)) = (&b.texture, &b.layout, &b.art) else { return };
    if key.pixel.is_some() || *shown != art(key) || !b.done.iter().any(|d| *d) {
        return;
    }
    let rect = xf.rect;
    let frame = match (&app.canvas.texture, &app.canvas.key) {
        (Some(_), Some(k)) if k.doc == key.doc => {
            // A frame turned since, or of document pixels, isn't a rectangle the backdrop goes round.
            if k.pixel.is_some() || (k.rot - key.rot).abs() >= 1e-9 {
                return;
            }
            // Where the canvas draws it (`canvas::show`).
            let old = Xf { rect, zoom: k.zoom, center: Point::new(k.cx, k.cy), rot: xf.rot };
            egui::Rect::from_two_pos(xf.to_screen(old.to_doc(rect.min)), xf.to_screen(old.to_doc(rect.max)))
        }
        _ => egui::Rect::NOTHING,
    };
    let bands = outside(rect, frame);
    if bands.is_empty() {
        return;
    }
    let ([tw, th], l) = (layout.size.map(|v| v as f32), layout);
    let mut mesh = egui::Mesh::with_texture(texture.id());
    for (&[x, y, w, h], _) in l.tiles.iter().zip(&b.done).filter(|(_, done)| **done) {
        let at = |px: usize, py: usize| (l.bounds.x0 + px as f64 / l.scale, l.bounds.y0 + py as f64 / l.scale);
        let ((x0, y0), (x1, y1)) = (at(x, y), at(x + w, y + h));
        let (u0, v0, u1, v1) = (x as f32 / tw, y as f32 / th, (x + w) as f32 / tw, (y + h) as f32 / th);
        let base = mesh.vertices.len() as u32;
        for (pos, uv) in xf.quad(Rect::new(x0, y0, x1, y1)).into_iter().zip([pos2(u0, v0), pos2(u1, v0), pos2(u1, v1), pos2(u0, v1)]) {
            mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: Color32::WHITE });
        }
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    for band in bands {
        painter.with_clip_rect(band).add(Shape::mesh(mesh.clone()));
    }
}

/// The parts of `rect` outside `hole` (a side of `hole` within half a point of `rect`'s covers
/// it).
fn outside(rect: egui::Rect, hole: egui::Rect) -> Vec<egui::Rect> {
    let hole = hole.intersect(rect);
    if !hole.is_positive() {
        return vec![rect];
    }
    let mut out = Vec::with_capacity(4);
    if hole.top() > rect.top() + 0.5 {
        out.push(egui::Rect::from_min_max(rect.min, pos2(rect.right(), hole.top())));
    }
    if hole.bottom() < rect.bottom() - 0.5 {
        out.push(egui::Rect::from_min_max(pos2(rect.left(), hole.bottom()), rect.max));
    }
    if hole.left() > rect.left() + 0.5 {
        out.push(egui::Rect::from_min_max(pos2(rect.left(), hole.top()), pos2(hole.left(), hole.bottom())));
    }
    if hole.right() < rect.right() - 0.5 {
        out.push(egui::Rect::from_min_max(pos2(hole.right(), hole.top()), pos2(rect.right(), hole.bottom())));
    }
    out
}

/// The backdrop of `doc`: its art and artboards at most `max_side` pixels a side, [`MAX_PIXELS`] in
/// all and [`MAX_SCALE`] pixels a point, cut into tiles, those nearest `center` first. None when
/// there is nothing to draw.
fn layout(doc: &Document, max_side: usize, center: Point) -> Option<Layout> {
    let b = doc.artboards.iter().fold(doc.art_bounds(), |acc, ab| vectorcraft_geom::union_opt(acc, Some(ab.rect)))?;
    if ![b.x0, b.y0, b.x1, b.y1].iter().all(|v| v.is_finite()) || b.width() <= 0.0 || b.height() <= 0.0 {
        return None;
    }
    let side = max_side as f64;
    let scale = (side / b.width()).min(side / b.height()).min((MAX_PIXELS / (b.width() * b.height())).sqrt()).min(MAX_SCALE);
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let w = ((b.width() * scale).ceil() as usize).clamp(1, max_side);
    let h = ((b.height() * scale).ceil() as usize).clamp(1, max_side);
    let bounds = Rect::new(b.x0, b.y0, b.x0 + w as f64 / scale, b.y0 + h as f64 / scale);
    let mut tiles = Vec::new();
    for y in (0..h).step_by(TILE) {
        for x in (0..w).step_by(TILE) {
            tiles.push([x, y, TILE.min(w - x), TILE.min(h - y)]);
        }
    }
    let far = |&[x, y, w, h]: &[usize; 4]| {
        let c = Point::new(bounds.x0 + (x as f64 + w as f64 / 2.0) / scale, bounds.y0 + (y as f64 + h as f64 / 2.0) / scale);
        (c - center).hypot()
    };
    tiles.sort_by(|a, b| far(a).total_cmp(&far(b)));
    Some(Layout { bounds, scale, size: [w, h], tiles })
}

/// The backdrop's thread (none on the web).
fn spawn(ctx: egui::Context) -> Option<(Sender<Msg>, Receiver<Reply>)> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let (tx, jobs) = channel::<Msg>();
    let (done, rx) = channel::<Reply>();
    std::thread::Builder::new()
        .name("dancard-backdrop".into())
        .spawn(move || {
            let mut renderer = Renderer::new();
            let mut current: Option<(u64, Arc<Document>, Box<RenderOptions>, Layout)> = None;
            while let Ok(msg) = jobs.recv() {
                let reply = match msg {
                    Msg::Start { run, doc, opts, max_side, center } => {
                        let layout = layout(&doc, max_side, center);
                        current = layout.clone().map(|l| (run, doc, opts, l));
                        Reply::Layout { run, layout }
                    }
                    Msg::Tile { run, index } => {
                        let Some((_, doc, opts, l)) = current.as_ref().filter(|c| c.0 == run) else { continue };
                        let Some(&[x, y, w, h]) = l.tiles.get(index) else { continue };
                        let last = index + 1 >= l.tiles.len();
                        let view =
                            Affine::translate((-(x as f64), -(y as f64))) * Affine::scale(l.scale) * Affine::translate((-l.bounds.x0, -l.bounds.y0));
                        // A tile that fails to render (a bug) is left out; the thread goes on with a
                        // new renderer.
                        let reply =
                            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| renderer.render(doc, w as u32, h as u32, view, opts))) {
                                Ok(img) => Reply::Tile { run, index, size: [img.width as usize, img.height as usize], pixels: img.pixels },
                                Err(_) => {
                                    renderer = Renderer::new();
                                    Reply::Tile { run, index, size: [0, 0], pixels: Vec::new() }
                                }
                            };
                        // The document isn't kept past its backdrop's last tile (it may be closed by
                        // now).
                        if last {
                            current = None;
                        }
                        reply
                    }
                };
                if done.send(reply).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        })
        .ok()?;
    Some((tx, rx))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    /// The parts of the canvas the frame doesn't cover: none when it covers it all, four bands
    /// round a frame in the middle, the whole canvas without a frame.
    #[test]
    fn the_backdrop_shows_only_outside_the_frame() {
        let r = |x0: f32, y0: f32, x1: f32, y1: f32| egui::Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
        let canvas = r(0.0, 0.0, 100.0, 80.0);
        assert!(outside(canvas, canvas).is_empty());
        assert!(outside(canvas, r(-0.2, 0.1, 100.3, 79.8)).is_empty(), "rounding errors cover it");
        assert_eq!(outside(canvas, egui::Rect::NOTHING), vec![canvas]);
        assert_eq!(outside(canvas, r(200.0, 0.0, 300.0, 80.0)), vec![canvas]);
        let bands = outside(canvas, r(20.0, 10.0, 70.0, 50.0));
        assert_eq!(bands, vec![r(0.0, 0.0, 100.0, 10.0), r(0.0, 50.0, 100.0, 80.0), r(0.0, 10.0, 20.0, 50.0), r(70.0, 10.0, 100.0, 50.0)]);
        // Panned right: only the strip on the left.
        assert_eq!(outside(canvas, r(30.0, 0.0, 130.0, 80.0)), vec![r(0.0, 0.0, 30.0, 80.0)]);
    }

    /// The backdrop covers the art and the artboards, within the size limits, in tiles that cover
    /// it once, those nearest the view first.
    #[test]
    fn the_backdrop_is_laid_out_in_tiles_nearest_the_view_first() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 1000, "height": 500})).unwrap();
        app.run("shape.rectangle", json!({"x": 1500, "y": 100, "width": 100, "height": 100})).unwrap();
        let doc = app.session.active().unwrap().doc.clone();
        let l = layout(&doc, 4096, Point::new(1600.0, 500.0)).unwrap();
        let ab = doc.artboards[0].rect;
        assert!(l.bounds.x0 <= ab.x0 && l.bounds.y0 <= ab.y0 && l.bounds.x1 >= 1600.0 && l.bounds.y1 >= ab.y1, "{:?}", l.bounds);
        let [w, h] = l.size;
        assert!(w <= 4096 && h <= 4096 && (w * h) as f64 <= MAX_PIXELS * 1.01 && l.scale <= MAX_SCALE);
        let area: usize = l.tiles.iter().map(|t| t[2] * t[3]).sum();
        assert_eq!(area, w * h, "the tiles cover the texture once");
        assert!(l.tiles.iter().all(|t| t[0] + t[2] <= w && t[1] + t[3] <= h && t[2] <= TILE && t[3] <= TILE));
        // The first tile is the one at the view's centre (the rectangle on the pasteboard).
        let first = l.tiles[0];
        let x = l.bounds.x0 + first[0] as f64 / l.scale;
        assert!(x + first[2] as f64 / l.scale >= 1500.0, "{first:?} at {x}");
        // A small document is drawn at most MAX_SCALE pixels a point; nothing to draw, no backdrop.
        let mut small = (*doc).clone();
        small.artboards.truncate(1);
        small.artboards[0].rect = Rect::new(0.0, 0.0, 100.0, 50.0);
        small.layers.clear();
        let l = layout(&small, 4096, Point::ZERO).unwrap();
        assert_eq!((l.scale, l.size), (MAX_SCALE, [400, 200]));
        small.artboards.clear();
        assert_eq!(layout(&small, 4096, Point::ZERO), None);
    }

    /// A key for the art of document `doc` at `revision`, seen at 100 % centred on (150, 100) in a
    /// 300 × 200 canvas.
    fn key(doc: usize, revision: u64) -> CacheKey {
        CacheKey {
            doc,
            revision,
            interacting: false,
            zoom: 1.0,
            cx: 150.0,
            cy: 100.0,
            w: 300,
            h: 200,
            outline: false,
            trim: false,
            templates: true,
            ppp: 1.0,
            hidden: vec![],
            rot: 0.0,
            anti_alias: true,
            placed: 0,
            pixel: None,
            smooth_images: true,
            isolated: None,
            rich_black: false,
            guide: ([0, 0, 0], false),
        }
    }

    /// The backdrop's art: the same for any view of the same art, another after an edit.
    #[test]
    fn the_backdrop_follows_the_art_not_the_view() {
        let k = key(1, 4);
        let panned = CacheKey { cx: 300.0, zoom: 0.5, w: 900, ppp: 2.0, interacting: false, ..k.clone() };
        assert_eq!(art(&k), art(&panned));
        assert_ne!(art(&k), art(&CacheKey { revision: 5, ..k.clone() }));
        assert_ne!(art(&k), art(&CacheKey { outline: true, ..k.clone() }));
        assert_ne!(art(&k), art(&CacheKey { templates: false, ..k.clone() }), "View › Hide Template");
    }

    /// A heavy document: the backdrop is rendered once the art has settled, a tile at a time while
    /// the worker is idle, and drawn where the frame doesn't reach; an edit starts it again, a light
    /// document drops it.
    #[test]
    fn a_heavy_document_gets_a_backdrop() {
        let ctx = egui::Context::default();
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
        app.canvas.worker = crate::render_worker::Worker::spawn(ctx.clone());
        app.canvas.last_ms = 50.0;
        let st = app.session.active().unwrap();
        let (doc, k) = (st.doc.clone(), key(st.uid as usize, st.revision));
        app.canvas.backdrop.note(&k, &doc, &RenderOptions::default());
        // Not settled yet: nothing started.
        pump(&mut app, &ctx);
        assert!(app.canvas.backdrop.art.is_none());
        app.canvas.backdrop.latest.as_mut().unwrap().since -= SETTLE_MS;
        let deadline = now_ms() + 30_000.0;
        while app.canvas.backdrop.done.is_empty() || !app.canvas.backdrop.done.iter().all(|d| *d) {
            pump(&mut app, &ctx);
            assert!(now_ms() < deadline, "the backdrop is rendered");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.canvas.backdrop.art, Some(art(&k)));
        let backdrop = app.canvas.backdrop.texture.as_ref().unwrap().id();
        // Each tile lands where its art is: the texture rebuilt from what was uploaded shows the
        // rectangle (10, 10, 50 × 40) where it is on the document, and nothing beside it.
        let layout = app.canvas.backdrop.layout.clone().unwrap();
        let [tw, th] = layout.size;
        let mut pixels = vec![Color32::TRANSPARENT; tw * th];
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        for (id, deltas) in &out.textures_delta.set {
            if *id != backdrop {
                continue;
            }
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                let [x0, y0] = delta.pos.unwrap_or([0, 0]);
                for (k, p) in image.pixels.iter().enumerate() {
                    pixels[(y0 + k / image.size[0]) * tw + x0 + k % image.size[0]] = *p;
                }
            }
        }
        out.textures_delta.clear();
        let at = |x: f64, y: f64| pixels[((y - layout.bounds.y0) * layout.scale) as usize * tw + ((x - layout.bounds.x0) * layout.scale) as usize];
        assert!(at(35.0, 30.0).a() > 0 && at(12.0, 12.0).a() > 0 && at(58.0, 48.0).a() > 0, "the rectangle");
        assert!(at(5.0, 30.0).a() == 0 && at(70.0, 30.0).a() == 0 && at(35.0, 60.0).a() == 0 && at(250.0, 150.0).a() == 0, "beside it");
        assert_eq!(pixels.len(), tw * th);
        // The clip rects of the backdrop drawn for a frame rendered for `frame` (none: no frame).
        let clips = |app: &mut VectorcraftApp, frame: Option<CacheKey>| {
            app.canvas.key = frame;
            app.canvas.texture =
                app.canvas.key.as_ref().map(|_| ctx.load_texture("frame", egui::ColorImage::filled([2, 2], Color32::RED), Default::default()));
            let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(300.0, 200.0));
            let xf = Xf { rect, zoom: 1.0, center: Point::new(150.0, 100.0), rot: 0.0 };
            let app = &*app;
            let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(rect), ..Default::default() }, |ui| draw(app, ui.painter(), &xf, &k));
            out.textures_delta.clear();
            out.shapes.iter().filter(|s| matches!(&s.shape, Shape::Mesh(m) if m.texture_id == backdrop)).map(|s| s.clip_rect).collect::<Vec<_>>()
        };
        let whole = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(300.0, 200.0));
        assert_eq!(clips(&mut app, None), vec![whole], "no frame: all of the canvas");
        assert_eq!(clips(&mut app, Some(k.clone())), vec![], "the frame for this view covers it all");
        // The view moved 100 points left of the frame's: the frame is drawn 100 points to the
        // right, the backdrop in the strip the move brought in on the left.
        let left = clips(&mut app, Some(CacheKey { cx: 250.0, ..k.clone() }));
        assert_eq!(left, vec![egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 200.0))], "the strip the pan brought in");
        // Pixel Preview: none.
        assert_eq!(clips(&mut app, Some(CacheKey { pixel: Some([0, 0, 300, 200]), ..k.clone() })), vec![]);
        // An edit: the backdrop of the old art isn't drawn any more, the new art's is rendered.
        let edited = key(k.doc, k.revision + 1);
        app.canvas.backdrop.note(&edited, &doc, &RenderOptions::default());
        app.canvas.backdrop.latest.as_mut().unwrap().since -= SETTLE_MS;
        pump(&mut app, &ctx);
        assert_eq!(app.canvas.backdrop.art, Some(art(&edited)));
        assert!(app.canvas.backdrop.done.iter().all(|d| !*d));
        // A light document drops it.
        app.canvas.last_ms = 2.0;
        app.canvas.backdrop.note(&key(k.doc, k.revision + 2), &doc, &RenderOptions::default());
        pump(&mut app, &ctx);
        assert!(app.canvas.backdrop.texture.is_none() && app.canvas.backdrop.art.is_none());
    }
}
