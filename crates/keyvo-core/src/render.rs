//! The key renderer (spec §9): a [`KeySpec`] becomes a [`Tile`] at the size
//! the device publishes for the key. Layers, bottom to top: the background
//! (`tiny-skia`), the accent, the icon (`resvg`) and the label (embedded
//! Inter through `resvg`'s text layout). A tile encodes to the baseline JPEG
//! the device draws, or to PNG for previews.
//!
//! Rendering is a pure function of the spec, the size and the [`Theme`]:
//! no system fonts, no files, so the same spec gives the same pixels on
//! every machine.

pub mod icon;

use resvg::tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};
use resvg::usvg;

use crate::profile::Color;

/// Width and height of a tile in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileSize {
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

/// What one key shows. A field left `None` takes the theme's default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeySpec {
    /// Background color.
    pub background: Option<Color>,
    /// The profile's accent, the primary context indicator (spec §9); no
    /// bar without one.
    pub accent: Option<Color>,
    /// Icon reference, `lucide:<name>`; an unknown one draws
    /// [`icon::FALLBACK`].
    pub icon: Option<String>,
}

/// The look shared by every key. Sizes are fractions of the tile height, so
/// a device with other key sizes gets the same proportions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Background of a key without its own color.
    pub background: Color,
    /// Corner radius.
    pub radius: f32,
    /// Width of the accent bar.
    pub accent_width: f32,
    /// Height of the accent bar.
    pub accent_height: f32,
    /// Gap between the accent bar and the bottom edge.
    pub accent_margin: f32,
    /// Icon and label color on a dark background.
    pub light_foreground: Color,
    /// Icon and label color on a light background.
    pub dark_foreground: Color,
    /// Icon size of a key without a label.
    pub icon_size: f32,
    /// Vertical center of the icon of a key without a label.
    pub icon_center: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x26, 0x28, 0x2c),
            radius: 0.12,
            accent_width: 0.4,
            accent_height: 0.05,
            accent_margin: 0.07,
            light_foreground: Color::rgb(0xff, 0xff, 0xff),
            dark_foreground: Color::rgb(0x11, 0x11, 0x11),
            icon_size: 0.5,
            icon_center: 0.45,
        }
    }
}

impl Theme {
    /// The foreground with the stronger contrast on `background`.
    #[must_use]
    pub fn foreground(&self, background: Color) -> Color {
        // Rec. 709 luma on the encoded values; a light key gets dark ink.
        let luma = 0.2126 * f32::from(background.r)
            + 0.7152 * f32::from(background.g)
            + 0.0722 * f32::from(background.b);
        if luma > 140.0 {
            self.dark_foreground
        } else {
            self.light_foreground
        }
    }
}

/// Errors of the renderer.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// A tile with no pixels.
    #[error("cannot render a {width}x{height} tile")]
    EmptyTile {
        /// Requested width.
        width: u16,
        /// Requested height.
        height: u16,
    },
    /// The JPEG encoder failed.
    #[error("jpeg encoding failed: {0}")]
    Jpeg(#[source] image::ImageError),
    /// The PNG encoder failed.
    #[error("png encoding failed: {0}")]
    Png(String),
}

/// JPEG quality of a tile, inside the 85 to 95 of spec §5.5.
pub const JPEG_QUALITY: u8 = 90;

/// One rendered key.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pixmap: Pixmap,
}

impl Tile {
    /// Width and height.
    #[must_use]
    pub fn size(&self) -> TileSize {
        // The pixmap was created from a `TileSize`, so both fit.
        TileSize {
            width: u16::try_from(self.pixmap.width()).unwrap_or(u16::MAX),
            height: u16::try_from(self.pixmap.height()).unwrap_or(u16::MAX),
        }
    }

    /// The pixels as RGB, transparent parts over black (the panel color).
    #[must_use]
    pub fn rgb(&self) -> Vec<u8> {
        // Premultiplied RGBA: dropping alpha composites over black.
        self.pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[r, g, b, _]| [r, g, b])
            .collect()
    }

    /// The baseline JPEG the device draws (spec §5.5).
    ///
    /// # Errors
    ///
    /// [`RenderError::Jpeg`].
    pub fn jpeg(&self) -> Result<Vec<u8>, RenderError> {
        let size = self.size();
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
            .encode(
                &self.rgb(),
                u32::from(size.width),
                u32::from(size.height),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(RenderError::Jpeg)?;
        Ok(jpeg)
    }

    /// A PNG of the tile, with its transparency.
    ///
    /// # Errors
    ///
    /// [`RenderError::Png`].
    pub fn png(&self) -> Result<Vec<u8>, RenderError> {
        self.pixmap
            .encode_png()
            .map_err(|e| RenderError::Png(e.to_string()))
    }
}

/// Renders key specs into tiles.
#[derive(Debug, Clone, Default)]
pub struct Renderer {
    theme: Theme,
}

impl Renderer {
    /// A renderer with the default theme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders `spec` at `size`.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTile`] for a zero width or height.
    pub fn tile(&self, spec: &KeySpec, size: TileSize) -> Result<Tile, RenderError> {
        let mut pixmap = Pixmap::new(u32::from(size.width), u32::from(size.height)).ok_or(
            RenderError::EmptyTile {
                width: size.width,
                height: size.height,
            },
        )?;
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        let background = spec.background.unwrap_or(self.theme.background);
        if let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) {
            fill_rounded(&mut pixmap, rect, self.theme.radius * height, background);
        }
        if let Some(accent) = spec.accent {
            let bar_width = self.theme.accent_width * height;
            let bar_height = self.theme.accent_height * height;
            let top = height - self.theme.accent_margin * height - bar_height;
            if let Some(rect) =
                Rect::from_xywh((width - bar_width) / 2.0, top, bar_width, bar_height)
            {
                fill_rounded(&mut pixmap, rect, bar_height / 2.0, accent);
            }
        }
        let foreground = self.theme.foreground(background);
        if let Some(reference) = &spec.icon {
            let extent = self.theme.icon_size * height;
            let center = self.theme.icon_center * height;
            draw_icon(
                &mut pixmap,
                reference,
                foreground,
                (width - extent) / 2.0,
                center - extent / 2.0,
                extent,
            );
        }
        Ok(Tile { pixmap })
    }
}

/// A `tiny-skia` paint of `color`, anti-aliased.
fn paint(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, 0xff);
    paint.anti_alias = true;
    paint
}

/// Draws the icon `reference`, or [`icon::FALLBACK`] when it does not
/// resolve, in `color` into the square of `side` at `left`, `top`.
fn draw_icon(pixmap: &mut Pixmap, reference: &str, color: Color, left: f32, top: f32, side: f32) {
    let Ok(svg) = icon::lookup(reference).or_else(|_| icon::lookup(icon::FALLBACK)) else {
        return;
    };
    // Lucide strokes with `currentColor`; tinting is a text substitution.
    let svg = svg.replace("currentColor", &color.to_string());
    let Ok(tree) = usvg::Tree::from_str(&svg, &usvg::Options::default()) else {
        return;
    };
    let scale = side / tree.size().width();
    resvg::render(
        &tree,
        Transform::from_row(scale, 0.0, 0.0, scale, left, top),
        &mut pixmap.as_mut(),
    );
}

/// Fills `rect` with corners of `radius`, clamped to half its sides.
fn fill_rounded(pixmap: &mut Pixmap, rect: Rect, radius: f32, color: Color) {
    // Control point distance of a cubic that approximates a quarter circle.
    const KAPPA: f32 = 0.552_284_8;
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let inset = radius * (1.0 - KAPPA);
    let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let mut path = PathBuilder::new();
    path.move_to(left + radius, top);
    path.line_to(right - radius, top);
    path.cubic_to(right - inset, top, right, top + inset, right, top + radius);
    path.line_to(right, bottom - radius);
    path.cubic_to(
        right,
        bottom - inset,
        right - inset,
        bottom,
        right - radius,
        bottom,
    );
    path.line_to(left + radius, bottom);
    path.cubic_to(
        left + inset,
        bottom,
        left,
        bottom - inset,
        left,
        bottom - radius,
    );
    path.line_to(left, top + radius);
    path.cubic_to(left, top + inset, left + inset, top, left + radius, top);
    path.close();
    if let Some(path) = path.finish() {
        pixmap.fill_path(
            &path,
            &paint(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_size_is_refused() {
        let error = Renderer::new()
            .tile(
                &KeySpec::default(),
                TileSize {
                    width: 0,
                    height: 118,
                },
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "cannot render a 0x118 tile");
    }

    #[test]
    fn transparent_corners_are_black_in_rgb() {
        let tile = Renderer::new()
            .tile(
                &KeySpec::default(),
                TileSize {
                    width: 40,
                    height: 40,
                },
            )
            .unwrap();
        let rgb = tile.rgb();
        assert_eq!(rgb.len(), 40 * 40 * 3);
        assert_eq!(&rgb[..3], &[0, 0, 0]);
    }
}
