//! DanCard fork (IN Vĩnh Phát): panels are resized by hand. A panel popped out next to the icon
//! column, or a floating group, is dragged larger or smaller by its edges and bottom corner (the
//! popped-out panel, pinned by its right edge, by its left edge and bottom-left corner); a
//! double-click on an edge gives it back its own size. The size is kept per panel (a floating
//! group's: per its first panel) with the preferences, in [`crate::state::UiState::dancard`], and
//! the panel's body fits it: a taller body scrolls ([`body_height`]).
//!
//! Moving needs nothing new: a popped-out panel dragged by its title floats, and a floating group
//! moves by its title bar (`floating`) or, in the shared docking, by the grip left of its tabs or a
//! tab. A panel floated keeps the size it had ([`float_size`]); a floating window of the shared
//! docking resizes by its bottom-right corner, which shows a grip ([`paint_grip`]).

use egui::{CursorIcon, Id, Pos2, Rect, Sense, Ui, Vec2, pos2, vec2};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Key of the sizes in `UiState::dancard`: `{panel id: [width, height]}`.
const SIZES: &str = "panelSizes";
/// Thickness of an edge handle, and side of a corner handle (points).
const EDGE: f32 = 6.0;
const CORNER: f32 = 14.0;
/// The smallest panel.
pub(crate) const MIN: Vec2 = Vec2::new(200.0, 160.0);

/// The size given to panel `id` by hand, if any.
pub(crate) fn size(app: &VectorcraftApp, id: &str) -> Option<Vec2> {
    let v = app.ui.dancard.get(SIZES)?.get(id)?;
    let (w, h) = (v.get(0)?.as_f64()? as f32, v.get(1)?.as_f64()? as f32);
    (w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0).then(|| vec2(w, h).max(MIN))
}

/// Keep `size` for panel `id` (`None`: its own size again).
pub(crate) fn set_size(app: &mut VectorcraftApp, id: &str, size: Option<Vec2>) {
    let sizes = app.ui.dancard.entry(SIZES).or_insert_with(|| json!({}));
    if !sizes.is_object() {
        *sizes = json!({});
    }
    if let Value::Object(map) = sizes {
        match size {
            Some(s) => {
                map.insert(id.to_string(), json!([s.x.round(), s.y.round()]));
            }
            None => {
                map.remove(id);
            }
        }
    }
}

fn body_id(id: &str) -> Id {
    Id::new(("dancard-panel-body-height", id))
}

fn shown_id(id: &str) -> Id {
    Id::new(("dancard-panel-shown-size", id))
}

/// Panel `id` showed popped out, `size` large, this frame (for [`float_size`]).
pub(crate) fn shown(ctx: &egui::Context, id: &str, size: Vec2) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| d.insert_temp(shown_id(id), (frame, size)));
}

/// The size panel `id` floats at (its whole window, title or tabs included): the one given to it
/// by hand, else the one it showed at popped out a frame or two ago; at most the window's. `None`:
/// the size any new floating window gets.
pub(crate) fn float_size(app: &VectorcraftApp, ctx: &egui::Context, id: &str) -> Option<Vec2> {
    let frame = ctx.cumulative_frame_nr();
    let popped = ctx.data(|d| d.get_temp::<(u64, Vec2)>(shown_id(id))).filter(|(f, _)| f.saturating_add(2) >= frame).map(|(_, s)| s);
    let s = size(app, id).or(popped).filter(|s| s.x.is_finite() && s.y.is_finite())?;
    let screen = ctx.content_rect().size();
    let fit = if screen.x.is_finite() && screen.y.is_finite() && screen.x > 0.0 && screen.y > 0.0 { s.min(screen) } else { s };
    Some(fit.max(MIN).round())
}

/// Say how tall panel `id`'s body may be this frame (`None`: as tall as it is), for
/// [`body_height`].
pub(crate) fn set_body_height(ctx: &egui::Context, id: &str, height: Option<f32>) {
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| match height {
        Some(h) => {
            d.insert_temp(body_id(id), (frame, h));
        }
        None => d.remove::<(u64, f32)>(body_id(id)),
    });
}

/// How tall panel `id`'s body may be: set this frame by its popped-out panel or floating group
/// when it was sized by hand (a hint from an earlier frame, where the panel showed elsewhere, is
/// none).
pub(crate) fn body_height(ctx: &egui::Context, id: &str) -> Option<f32> {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<(u64, f32)>(body_id(id))).filter(|(f, h)| *f == frame && h.is_finite() && *h > 0.0).map(|(_, h)| h)
}

/// Resize handles on the edges of a panel's box `rect` (the right one, or with `left` the left
/// one, and the bottom one) and on its bottom corner. While one is dragged → the box's new size,
/// as much larger or smaller as the pointer moved since the press, within [`MIN`]…`max` (the
/// corner it is pinned by stays: the top-left one, or with `left` the top-right one); `Some(None)`
/// after a double-click (its own size again).
pub(crate) fn handles(ui: &mut Ui, id: Id, rect: Rect, left: bool, max: Vec2) -> Option<Option<Vec2>> {
    let (side_x, corner_x) = if left { (rect.left(), rect.left()) } else { (rect.right() - EDGE, rect.right() - CORNER) };
    let side = Rect::from_min_size(pos2(side_x, rect.top() + 26.0), vec2(EDGE, (rect.height() - 26.0 - CORNER).max(0.0)));
    let bottom_x = if left { rect.left() + CORNER } else { rect.left() };
    let bottom = Rect::from_min_size(pos2(bottom_x, rect.bottom() - EDGE), vec2((rect.width() - CORNER).max(0.0), EDGE));
    let corner = Rect::from_min_size(pos2(corner_x, rect.bottom() - CORNER), vec2(CORNER, CORNER));
    let (pointer, press) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.press_origin()));
    let max = max.max(MIN);
    let mut out = None;
    for (k, r, cursor, wide, tall) in [
        (0, side, CursorIcon::ResizeHorizontal, true, false),
        (1, bottom, CursorIcon::ResizeVertical, false, true),
        (2, corner, if left { CursorIcon::ResizeNeSw } else { CursorIcon::ResizeNwSe }, true, true),
    ] {
        if r.width() <= 0.0 || r.height() <= 0.0 {
            continue;
        }
        let resp = ui.interact(r, id.with(k), Sense::click_and_drag()).on_hover_cursor(cursor);
        if k == 2 {
            grip(ui, r, left, resp.hovered() || resp.dragged());
        }
        if resp.double_clicked() {
            return Some(None);
        }
        // The size when the drag began, and where the button went down.
        let grab = id.with(("grab", k));
        if resp.drag_started()
            && let Some(p) = press
        {
            ui.ctx().data_mut(|d| d.insert_temp(grab, (rect.size(), p)));
        }
        if resp.dragged()
            && let Some(p) = pointer
            && let Some((size, from)) = ui.ctx().data(|d| d.get_temp::<(Vec2, egui::Pos2)>(grab))
        {
            let d = p - from;
            let w = if wide { if left { size.x - d.x } else { size.x + d.x } } else { rect.width() };
            let h = if tall { size.y + d.y } else { rect.height() };
            if w.is_finite() && h.is_finite() {
                out = Some(Some(vec2(w, h).clamp(MIN, max)));
            }
        }
    }
    out
}

/// Show where the corner `at` (bottom-right, or with `left` bottom-left) of a window that resizes
/// by it can be grabbed, as [`handles`] shows its own corner (the shared docking's floating windows
/// resize by their bottom-right 12 points).
pub(crate) fn paint_grip(ui: &Ui, at: Pos2, left: bool) {
    if !at.is_finite() {
        return;
    }
    let r = if left { Rect::from_min_size(at - vec2(0.0, 12.0), vec2(12.0, 12.0)) } else { Rect::from_min_max(at - vec2(12.0, 12.0), at) };
    let hot = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| r.contains(p));
    grip(ui, r, left, hot);
}

/// Three short diagonal strokes in the corner `r` (bottom-right, or with `left` bottom-left);
/// brighter when `hot`.
fn grip(ui: &Ui, r: Rect, left: bool, hot: bool) {
    let t = Tokens::get(ui.ctx());
    let c = if hot { t.text } else { t.text_dim };
    let step = r.width().min(r.height()) / 3.5;
    for k in [1.0, 2.0, 3.0] {
        let d = step * k;
        let (a, b) = if left {
            (pos2(r.left() + 2.0, r.bottom() - d), pos2(r.left() + d, r.bottom() - 2.0))
        } else {
            (pos2(r.right() - d, r.bottom() - 2.0), pos2(r.right() - 2.0, r.bottom() - d))
        };
        ui.painter().line_segment([a, b], egui::Stroke::new(1.0, c));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    #[test]
    fn sizes_are_kept_per_panel_and_cleared() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        assert_eq!(size(&app, "dancard"), None);
        set_size(&mut app, "dancard", Some(vec2(420.4, 600.6)));
        set_size(&mut app, "swatches", Some(vec2(10.0, 10.0)));
        assert_eq!(size(&app, "dancard"), Some(vec2(420.0, 601.0)));
        // Smaller than the smallest panel: the smallest.
        assert_eq!(size(&app, "swatches"), Some(MIN));
        set_size(&mut app, "dancard", None);
        assert_eq!(size(&app, "dancard"), None);
        // A hand-edited preferences file: ignored, not a crash.
        app.ui.dancard.insert(SIZES.into(), json!({"dancard": ["x", 2]}));
        assert_eq!(size(&app, "dancard"), None);
        app.ui.dancard.insert(SIZES.into(), json!(7));
        set_size(&mut app, "dancard", Some(vec2(300.0, 300.0)));
        assert_eq!(size(&app, "dancard"), Some(vec2(300.0, 300.0)));
    }

    #[test]
    fn floats_at_the_size_given_or_shown() {
        let ctx = egui::Context::default();
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        // Neither given nor shown popped out: the size any floating window gets.
        assert_eq!(float_size(&app, &ctx, "dancard"), None);
        shown(&ctx, "dancard", vec2(320.4, 640.0));
        assert_eq!(float_size(&app, &ctx, "dancard"), Some(vec2(320.0, 640.0)));
        // A size given by hand comes first.
        set_size(&mut app, "dancard", Some(vec2(500.0, 450.0)));
        assert_eq!(float_size(&app, &ctx, "dancard"), Some(vec2(500.0, 450.0)));
        // Never smaller than the smallest panel, never a size that isn't a number.
        shown(&ctx, "swatches", vec2(20.0, 20.0));
        assert_eq!(float_size(&app, &ctx, "swatches"), Some(MIN));
        shown(&ctx, "brushes", vec2(f32::NAN, 300.0));
        assert_eq!(float_size(&app, &ctx, "brushes"), None);
    }

    #[test]
    fn body_height_is_per_frame_hint() {
        let ctx = egui::Context::default();
        assert_eq!(body_height(&ctx, "dancard"), None);
        set_body_height(&ctx, "dancard", Some(380.0));
        assert_eq!(body_height(&ctx, "dancard"), Some(380.0));
        set_body_height(&ctx, "dancard", None);
        assert_eq!(body_height(&ctx, "dancard"), None);
    }
}
