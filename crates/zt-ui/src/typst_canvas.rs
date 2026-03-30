//! Frame Painter: Renders Typst's compiled `Frame` tree directly to GPUI Canvas.
//!
//! This is the core renderer for both the note view (continuous, no page boundaries)
//! and the PDF preview (paged, with shadows). It walks the `FrameItem` enum
//! exhaustively — the compiler enforces correctness when Typst adds new variants.

use gpui::*;
use typst::layout::{Frame, FrameItem, GroupItem, Size as TypstSize};
use typst::text::TextItem;
use typst::visualize::{Geometry, Shape, Paint as TypstPaint, Curve, CurveItem};

/// Conversion factor: Typst uses points (1pt = 1/72 inch).
/// GPUI uses logical pixels where 1px = 1/96 inch (CSS pixel convention).
/// So 1pt = 96/72 = 4/3 ≈ 1.333 px.
pub const PT_TO_PX: f32 = 96.0 / 72.0;

/// Render state tracked while walking the frame tree.
struct RenderState {
    /// Current transform offset (accumulated translation).
    offset_x: f32,
    offset_y: f32,
    /// Scale factor (for transforms).
    scale_x: f32,
    scale_y: f32,
    /// Viewport bounds for culling (absolute window coords).
    /// Items fully outside this range are skipped.
    viewport_top: f32,
    viewport_bottom: f32,
    /// If set, override ALL text fill colors with this color.
    text_color_override: Option<Hsla>,
}

impl RenderState {
    fn translate(&self, x: f32, y: f32) -> Self {
        Self {
            offset_x: self.offset_x + x * self.scale_x,
            offset_y: self.offset_y + y * self.scale_y,
            text_color_override: self.text_color_override,
            ..*self
        }
    }

    fn scale(&self, sx: f32, sy: f32) -> Self {
        Self {
            scale_x: self.scale_x * sx,
            scale_y: self.scale_y * sy,
            text_color_override: self.text_color_override,
            ..*self
        }
    }

}

/// Convert a Typst Paint to a GPUI Hsla color.
/// For now, only handles solid colors. Gradients/patterns fall back to gray.
fn paint_to_hsla(paint: &TypstPaint) -> Hsla {
    match paint {
        TypstPaint::Solid(color) => {
            let [r, g, b, a] = color.to_vec4_u8();
            Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }.into()
        }
        _ => {
            // Gradient/pattern: fall back to medium gray for now
            Rgba { r: 0.5, g: 0.5, b: 0.5, a: 1.0 }.into()
        }
    }
}

/// Collect link regions for hit-testing.
pub struct LinkRegion {
    pub bounds: Bounds<Pixels>,
    pub destination: typst::model::Destination,
}

/// Render a single Typst page frame to the GPUI window.
///
/// `origin` is the top-left corner of the page in window coordinates.
/// `links` collects clickable link regions for hit-testing.
/// Render a Typst frame.
/// `scale` is an additional scale factor applied on top of PT_TO_PX.
/// `viewport` is (top, bottom) in absolute window coordinates for culling.
pub fn render_frame(
    window: &mut Window,
    frame: &Frame,
    origin: Point<Pixels>,
    scale: f32,
    links: &mut Vec<LinkRegion>,
) {
    // Default viewport: huge range (no culling)
    render_frame_with_viewport(window, frame, origin, scale, -10000.0, 10000.0, links);
}

/// Render with viewport culling — items outside [viewport_top, viewport_bottom] are skipped.
pub fn render_frame_with_viewport(
    window: &mut Window,
    frame: &Frame,
    origin: Point<Pixels>,
    scale: f32,
    viewport_top: f32,
    viewport_bottom: f32,
    links: &mut Vec<LinkRegion>,
) {
    render_frame_styled(window, frame, origin, scale, viewport_top, viewport_bottom, None, links);
}

/// Render with viewport culling and an optional text color override.
///
/// When `text_color` is `Some(color)`, ALL text glyphs are painted in that color
/// regardless of what Typst's `#set text(fill: ...)` specified. This lets the
/// renderer apply theme colors without injecting Typst preamble source.
pub fn render_frame_styled(
    window: &mut Window,
    frame: &Frame,
    origin: Point<Pixels>,
    scale: f32,
    viewport_top: f32,
    viewport_bottom: f32,
    text_color: Option<Hsla>,
    links: &mut Vec<LinkRegion>,
) {
    let state = RenderState {
        offset_x: f32::from(origin.x),
        offset_y: f32::from(origin.y),
        scale_x: scale,
        scale_y: scale,
        viewport_top,
        viewport_bottom,
        text_color_override: text_color,
    };
    render_frame_inner(window, frame, &state, links);
}

fn render_frame_inner(
    window: &mut Window,
    frame: &Frame,
    state: &RenderState,
    links: &mut Vec<LinkRegion>,
) {
    for (pos, item) in frame.items() {
        let dx = pos.x.to_pt() as f32 * PT_TO_PX;
        let dy = pos.y.to_pt() as f32 * PT_TO_PX;
        let item_state = state.translate(dx, dy);

        // Viewport culling: skip items clearly outside visible area
        // Use a generous height estimate (50px) for non-group items
        let item_y = state.offset_y + dy * state.scale_y;
        let skip = match item {
            FrameItem::Group(g) => {
                let h = g.frame.height().to_pt() as f32 * PT_TO_PX * state.scale_y;
                item_y + h < state.viewport_top || item_y > state.viewport_bottom
            }
            _ => {
                item_y + 50.0 < state.viewport_top || item_y - 50.0 > state.viewport_bottom
            }
        };
        if skip { continue; }

        // Exhaustive match — compiler enforces completeness on Typst upgrades
        match item {
            FrameItem::Group(group) => {
                render_group(window, &item_state, group, links);
            }
            FrameItem::Text(text) => {
                render_text(window, &item_state, text);
            }
            FrameItem::Shape(shape, _span) => {
                render_shape(window, &item_state, shape);
            }
            FrameItem::Image(_image, _size, _span) => {
                // TODO: Image rendering
                // For now, draw a placeholder rectangle
                render_image_placeholder(window, &item_state, _size);
            }
            FrameItem::Link(dest, size) => {
                // Register clickable region for navigation
                let origin = point(px(item_state.offset_x), px(item_state.offset_y));
                let sz = gpui::size(
                    px(size.x.to_pt() as f32 * PT_TO_PX * item_state.scale_x),
                    px(size.y.to_pt() as f32 * PT_TO_PX * item_state.scale_y),
                );
                links.push(LinkRegion {
                    bounds: Bounds { origin, size: sz },
                    destination: dest.clone(),
                });
            }
            FrameItem::Tag(_tag) => {
                // Metadata — skip
            }
        }
    }
}

fn render_group(
    window: &mut Window,
    state: &RenderState,
    group: &GroupItem,
    links: &mut Vec<LinkRegion>,
) {
    // Apply the group's transform
    let tx = group.transform.tx.to_pt() as f32 * PT_TO_PX;
    let ty = group.transform.ty.to_pt() as f32 * PT_TO_PX;
    let sx = group.transform.sx.get() as f32;
    let sy = group.transform.sy.get() as f32;

    let child_state = state.translate(tx, ty).scale(sx, sy);

    // Handle clipping (simplified: use content mask for rectangular clips)
    if let Some(_clip) = &group.clip {
        // TODO: Convert Curve to ContentMask for proper clipping
        // For now, render without clip
    }

    render_frame_inner(window, &group.frame, &child_state, links);
}

/// Cache for mapping Typst font → GPUI FontId.
/// Uses a global cache since font resolution is expensive.
use std::collections::HashMap;
use std::cell::RefCell;

/// Cache key: (family, weight_number, is_italic)
type FontCacheKey = (String, u16, bool);

thread_local! {
    static FONT_ID_CACHE: RefCell<HashMap<FontCacheKey, Option<FontId>>> = RefCell::new(HashMap::new());
}

fn typst_weight_to_gpui(w: u16) -> FontWeight {
    match w {
        0..=149 => FontWeight::THIN,
        150..=249 => FontWeight::EXTRA_LIGHT,
        250..=349 => FontWeight::LIGHT,
        350..=449 => FontWeight::NORMAL,
        450..=549 => FontWeight::MEDIUM,
        550..=649 => FontWeight::SEMIBOLD,
        650..=749 => FontWeight::BOLD,
        750..=849 => FontWeight::EXTRA_BOLD,
        _ => FontWeight::BLACK,
    }
}

/// Try to resolve a Typst font to a GPUI FontId.
///
/// Returns `None` when GPUI doesn't have the **exact same** font family.
/// `resolve_font` silently falls back to a default when the family is missing,
/// which causes glyph-ID mismatches (math symbols become garbage).  We detect
/// the fallback by comparing the resolved font's family against the requested
/// one and rejecting mismatches.
fn resolve_gpui_font(window: &Window, typst_font: &typst::text::Font) -> Option<FontId> {
    let info = typst_font.info();
    let family = &info.family;
    let weight_num = info.variant.weight.to_number();
    let is_italic = info.variant.style == typst::text::FontStyle::Italic;
    let key = (family.clone(), weight_num, is_italic);

    FONT_ID_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(&key) {
            return *cached;
        }

        let weight = typst_weight_to_gpui(weight_num);
        let style = if is_italic { gpui::FontStyle::Italic } else { gpui::FontStyle::Normal };

        let gpui_font = gpui::Font {
            family: SharedString::from(family.clone()),
            features: FontFeatures::default(),
            fallbacks: None,
            weight,
            style,
        };

        let ts = window.text_system();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let font_id = ts.resolve_font(&gpui_font);
            // Verify the resolved font is actually the family we asked for.
            // GPUI silently falls back to a default font when the family is
            // missing, which would cause glyph-ID mismatches for math fonts.
            if let Some(resolved) = ts.get_font_for_id(font_id) {
                if resolved.family != gpui_font.family {
                    return None; // family mismatch → fall back to outlines
                }
            }
            Some(font_id)
        }))
        .ok()
        .flatten();

        cache.insert(key, result);
        result
    })
}

fn render_text(
    window: &mut Window,
    state: &RenderState,
    text: &TextItem,
) {
    let color = state.text_color_override.unwrap_or_else(|| paint_to_hsla(&text.fill));
    let font_size_px = text.size.to_pt() as f32 * PT_TO_PX * state.scale_x;

    // Try GPU-accelerated paint_glyph — only if GPUI has the EXACT same font.
    // Typst's bundled fonts (Libertinus, New CM, DejaVu) aren't in GPUI's font db,
    // so we must fall back to path outlines for those. Only use paint_glyph for
    // system fonts that both Typst and GPUI share (same glyph IDs).
    if let Some(font_id) = resolve_gpui_font(window, &text.font) {
        let mut x = 0.0_f32;
        let mut y = 0.0_f32;

        for glyph in &text.glyphs {
            let gx = x + glyph.x_offset.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_x;
            let gy = y - glyph.y_offset.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_y;

            let origin = point(
                px(state.offset_x + gx),
                px(state.offset_y + gy),
            );

            let _ = window.paint_glyph(
                origin,
                font_id,
                // SAFETY: GlyphId is a transparent newtype over u32
                unsafe { std::mem::transmute::<u32, GlyphId>(glyph.id as u32) },
                px(font_size_px),
                color,
            );

            x += glyph.x_advance.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_x;
            y += glyph.y_advance.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_y;
        }
        return;
    }

    // Fallback: render as path outlines (slow — one paint_path per glyph)
    let upem = text.font.units_per_em() as f32;
    let scale_x = font_size_px / upem;
    let scale_y = -font_size_px / upem;

    let mut x = 0.0_f32;
    let mut y = 0.0_f32;

    for glyph in &text.glyphs {
        let gx = x + glyph.x_offset.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_x;
        let gy = y - glyph.y_offset.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_y;

        let origin_x = state.offset_x + gx;
        let origin_y = state.offset_y + gy;

        if let Some(path) = glyph_outline_path(
            &text.font, glyph.id,
            origin_x, origin_y,
            scale_x, scale_y,
        ) {
            window.paint_path(path, color);
        }

        x += glyph.x_advance.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_x;
        y += glyph.y_advance.at(text.size).to_pt() as f32 * PT_TO_PX * state.scale_y;
    }
}

/// Extract a glyph's outline as a GPUI Path.
/// `origin_x/y` is the baseline position in pixels.
/// `scale_x` is positive, `scale_y` is negative (flips Y from font-up to screen-down).
fn glyph_outline_path(
    font: &typst::text::Font,
    glyph_id: u16,
    origin_x: f32,
    origin_y: f32,
    scale_x: f32,
    scale_y: f32,
) -> Option<Path<Pixels>> {
    use ttf_parser::GlyphId;

    let ttf = font.ttf();
    let id = GlyphId(glyph_id);

    struct Builder {
        path: PathBuilder,
        origin_x: f32,
        origin_y: f32,
        scale_x: f32,
        scale_y: f32,
    }

    impl Builder {
        fn pt(&self, x: f32, y: f32) -> Point<Pixels> {
            point(px(self.origin_x + x * self.scale_x), px(self.origin_y + y * self.scale_y))
        }
    }

    impl ttf_parser::OutlineBuilder for Builder {
        fn move_to(&mut self, x: f32, y: f32) {
            self.path.move_to(self.pt(x, y));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.path.line_to(self.pt(x, y));
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            // GPUI's curve_to is quadratic bezier: (to, ctrl)
            self.path.curve_to(self.pt(x, y), self.pt(x1, y1));
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            // GPUI's cubic_bezier_to: (to, ctrl_a, ctrl_b)
            self.path.cubic_bezier_to(
                self.pt(x, y),     // endpoint
                self.pt(x1, y1),   // control point 1
                self.pt(x2, y2),   // control point 2
            );
        }
        fn close(&mut self) {
            self.path.close();
        }
    }

    let mut builder = Builder {
        path: PathBuilder::fill(),
        origin_x,
        origin_y,
        scale_x,
        scale_y,
    };

    ttf.outline_glyph(id, &mut builder)?;
    builder.path.build().ok()
}

fn render_shape(
    window: &mut Window,
    state: &RenderState,
    shape: &Shape,
) {
    match &shape.geometry {
        Geometry::Rect(size) => {
            let bounds = Bounds {
                origin: point(px(state.offset_x), px(state.offset_y)),
                size: gpui::size(
                    px(size.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                    px(size.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                ),
            };

            let bg = shape.fill.as_ref()
                .map(|p| paint_to_hsla(p))
                .unwrap_or(Hsla::transparent_black());

            let (border_width, border_color) = shape.stroke.as_ref()
                .map(|s| (px(s.thickness.to_pt() as f32 * PT_TO_PX * state.scale_x), paint_to_hsla(&s.paint)))
                .unwrap_or((px(0.0), Hsla::transparent_black()));

            window.paint_quad(PaintQuad {
                bounds,
                corner_radii: Corners::default(),
                background: bg.into(),
                border_widths: Edges::all(border_width),
                border_color,
                border_style: BorderStyle::Solid,
            });
        }
        Geometry::Line(target) => {
            let start = point(px(state.offset_x), px(state.offset_y));
            let end = point(
                px(state.offset_x + target.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                px(state.offset_y + target.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
            );

            if let Some(stroke) = &shape.stroke {
                let thickness = stroke.thickness.to_pt() as f32 * PT_TO_PX * state.scale_x;
                let color = paint_to_hsla(&stroke.paint);

                let mut builder = PathBuilder::stroke(px(thickness));
                builder.move_to(start);
                builder.line_to(end);
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            }
        }
        Geometry::Curve(curve) => {
            render_curve(window, state, curve, &shape.fill, &shape.stroke);
        }
    }
}

fn render_curve(
    window: &mut Window,
    state: &RenderState,
    curve: &Curve,
    fill: &Option<TypstPaint>,
    stroke: &Option<typst::visualize::FixedStroke>,
) {
    // Render fill
    if let Some(paint) = fill {
        let color = paint_to_hsla(paint);
        let mut builder = PathBuilder::fill();

        for item in curve.0.iter() {
            match item {
                CurveItem::Move(p) => {
                    builder.move_to(point(
                        px(state.offset_x + p.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                        px(state.offset_y + p.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                    ));
                }
                CurveItem::Line(p) => {
                    builder.line_to(point(
                        px(state.offset_x + p.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                        px(state.offset_y + p.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                    ));
                }
                CurveItem::Cubic(cp1, cp2, end) => {
                    builder.cubic_bezier_to(
                        point(
                            px(state.offset_x + cp1.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + cp1.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                        point(
                            px(state.offset_x + cp2.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + cp2.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                        point(
                            px(state.offset_x + end.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + end.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                    );
                }
                CurveItem::Close => {
                    builder.close();
                }
            }
        }

        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }

    // Render stroke
    if let Some(stroke) = stroke {
        let color = paint_to_hsla(&stroke.paint);
        let thickness = stroke.thickness.to_pt() as f32 * PT_TO_PX * state.scale_x;
        let mut builder = PathBuilder::stroke(px(thickness));

        for item in curve.0.iter() {
            match item {
                CurveItem::Move(p) => {
                    builder.move_to(point(
                        px(state.offset_x + p.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                        px(state.offset_y + p.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                    ));
                }
                CurveItem::Line(p) => {
                    builder.line_to(point(
                        px(state.offset_x + p.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                        px(state.offset_y + p.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                    ));
                }
                CurveItem::Cubic(cp1, cp2, end) => {
                    builder.cubic_bezier_to(
                        point(
                            px(state.offset_x + cp1.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + cp1.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                        point(
                            px(state.offset_x + cp2.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + cp2.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                        point(
                            px(state.offset_x + end.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
                            px(state.offset_y + end.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
                        ),
                    );
                }
                CurveItem::Close => {
                    builder.close();
                }
            }
        }

        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}

fn render_image_placeholder(
    window: &mut Window,
    state: &RenderState,
    size: &TypstSize,
) {
    let bounds = Bounds {
        origin: point(px(state.offset_x), px(state.offset_y)),
        size: gpui::size(
            px(size.x.to_pt() as f32 * PT_TO_PX * state.scale_x),
            px(size.y.to_pt() as f32 * PT_TO_PX * state.scale_y),
        ),
    };

    window.paint_quad(PaintQuad {
        bounds,
        corner_radii: Corners::default(),
        background: Rgba { r: 0.2, g: 0.2, b: 0.3, a: 1.0 }.into(),
        border_widths: Edges::all(px(1.0)),
        border_color: Rgba { r: 0.3, g: 0.3, b: 0.5, a: 1.0 }.into(),
        border_style: BorderStyle::Solid,
    });
}

/// A GPUI element that renders a Typst PagedDocument.
/// Displays pages vertically with gaps and shadows (PDF preview mode).
pub struct TypstDocumentView {}

impl TypstDocumentView {
    pub fn new(_pages: Vec<Frame>) -> Self {
        Self {}
    }
}
