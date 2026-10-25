//! The key renderer (spec §9): a [`KeySpec`] becomes a [`Tile`] at the size
//! the device publishes for the key. Layers, bottom to top: the background
//! (`tiny-skia`), the accent, the icon (`resvg`) and the label (embedded
//! Inter through `resvg`'s text layout). A tile encodes to the baseline JPEG
//! the device draws, or to PNG for previews.
//!
//! The device gets a [`Canvas`]: the panel background and every key drawn
//! into one image of the panel size, cropped to the area that changed.
//!
//! Rendering is a pure function of the spec, the size and the [`Theme`]:
//! no system fonts, no files, so the same spec gives the same pixels on
//! every machine.

pub mod icon;

use std::sync::Arc;

use resvg::tiny_skia::{
    self, FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Rect,
    Stroke, Transform,
};
use resvg::usvg::{self, fontdb};

use crate::feedback::Look;
use crate::profile::{Color, Key, KeySlot, KeyState};
use crate::stack::Stack;

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
    /// Text below the icon, or centered without one.
    pub label: Option<String>,
    /// Text of the corner badge. A [`Look`] with a badge shows its own
    /// symbol when this is `None`.
    pub badge: Option<String>,
    /// The automatic state's look (spec §9).
    pub look: Look,
    /// Where the [`Look::Running`] arc starts, in degrees clockwise from
    /// the top; the daemon turns it to animate the spinner.
    pub spinner: u16,
}

impl KeySpec {
    /// What `key` shows, with the profile's `accent`; a blank key without
    /// one.
    #[must_use]
    pub fn from_key(key: Option<&Key>, accent: Option<Color>) -> Self {
        Self {
            background: key.and_then(|key| key.color),
            accent,
            icon: key.and_then(|key| key.icon.clone()),
            label: key.and_then(|key| key.label.clone()),
            ..Self::default()
        }
    }

    /// This spec in the automatic state `look`, with the key's own
    /// `states.running`, `states.success` or `states.error` applied over
    /// the built-in look.
    #[must_use]
    pub fn with_look(mut self, key: Option<&Key>, look: Look) -> Self {
        self.look = look;
        if let Some(state) = look
            .state()
            .and_then(|name| key.and_then(|key| key.states.get(name)))
        {
            self.apply(state);
        }
        self
    }

    /// Replaces what `state` sets: icon, label, background and badge.
    pub fn apply(&mut self, state: &KeyState) {
        if let Some(icon) = &state.icon {
            self.icon = Some(icon.clone());
        }
        if let Some(label) = &state.label {
            self.label = Some(label.clone());
        }
        if let Some(color) = state.color {
            self.background = Some(color);
        }
        if let Some(badge) = &state.badge {
            self.badge = Some(badge.clone());
        }
    }
}

/// The specs of keys 1 to `keys` of `stack`'s page, every key with the
/// accent of the profile on top. The key count comes from the device's
/// geometry; a key that no layer defines is blank, one the profile lacks
/// shows the global key (spec §6.1).
#[must_use]
pub fn page(stack: &Stack<'_>, keys: u8) -> Vec<KeySpec> {
    let accent = stack.profile().profile.accent;
    (1..=keys)
        .map(|n| {
            let key = KeySlot::new(n)
                .and_then(|slot| stack.key(slot))
                .map(|(key, _)| key);
            KeySpec::from_key(key, accent)
        })
        .collect()
}

/// Where one key's tile goes on the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPlace {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Tile size.
    pub size: TileSize,
}

/// The whole display as the device publishes it: its size and the key
/// rectangles in table order (spec §5.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// Panel size.
    pub size: TileSize,
    /// The keys, numbered from 1 in this order.
    pub keys: Vec<KeyPlace>,
}

impl Panel {
    /// The bounding box of the keys at `indexes` (counted from 0), the area
    /// a change of those keys repaints; for every key it is the union of
    /// the key rectangles (435x434 at 23,6 on the Keypad). Indexes past the
    /// last key are ignored; `None` when no key is left.
    #[must_use]
    pub fn bounds(&self, indexes: impl IntoIterator<Item = usize>) -> Option<KeyPlace> {
        let (mut left, mut top, mut right, mut bottom) = (u16::MAX, u16::MAX, 0, 0);
        let mut any = false;
        for place in indexes.into_iter().filter_map(|i| self.keys.get(i)) {
            any = true;
            left = left.min(place.x);
            top = top.min(place.y);
            right = right.max(place.x.saturating_add(place.size.width));
            bottom = bottom.max(place.y.saturating_add(place.size.height));
        }
        any.then(|| KeyPlace {
            x: left,
            y: top,
            size: TileSize {
                width: right - left,
                height: bottom - top,
            },
        })
    }
}

/// The look shared by every key. Sizes are fractions of the tile height, so
/// a device with other key sizes gets the same proportions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Background of a key without its own color.
    pub background: Color,
    /// The panel around the keys, of a profile without an accent.
    pub panel: Color,
    /// How far the panel turns toward the profile's accent, 0 to 1.
    pub panel_tint: f32,
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
    /// Icon size of a key with a label.
    pub labeled_icon_size: f32,
    /// Vertical center of the icon of a key with a label.
    pub labeled_icon_center: f32,
    /// Largest label font size.
    pub label_size: f32,
    /// Smallest label font size; a label that does not fit at this size is
    /// cut.
    pub min_label_size: f32,
    /// Free space left and right of a label.
    pub label_margin: f32,
    /// Label baseline of a key with an icon.
    pub label_baseline: f32,
    /// How far a running key's icon fades toward the background, 0 to 1.
    pub dim: f32,
    /// Gap between the icon and the spinner arc.
    pub spinner_gap: f32,
    /// Stroke width of the spinner arc.
    pub spinner_width: f32,
    /// Length of the spinner arc, in degrees.
    pub spinner_sweep: f32,
    /// Spinner color on a key of a profile without an accent.
    pub spinner: Color,
    /// Diameter of the corner badge.
    pub badge_size: f32,
    /// Gap between the badge and the top and right edges.
    pub badge_margin: f32,
    /// Badge of a custom state.
    pub badge: Color,
    /// Badge of [`Look::RunningLong`].
    pub running: Color,
    /// Badge of [`Look::Success`].
    pub success: Color,
    /// Badge of [`Look::Error`].
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x26, 0x28, 0x2c),
            panel: Color::rgb(0x00, 0x00, 0x00),
            panel_tint: 0.2,
            radius: 0.12,
            accent_width: 0.4,
            accent_height: 0.05,
            accent_margin: 0.07,
            light_foreground: Color::rgb(0xff, 0xff, 0xff),
            dark_foreground: Color::rgb(0x11, 0x11, 0x11),
            icon_size: 0.5,
            icon_center: 0.45,
            labeled_icon_size: 0.4,
            labeled_icon_center: 0.36,
            label_size: 0.17,
            min_label_size: 0.12,
            label_margin: 0.06,
            label_baseline: 0.8,
            dim: 0.6,
            spinner_gap: 0.06,
            spinner_width: 0.045,
            spinner_sweep: 90.0,
            spinner: Color::rgb(0xff, 0xff, 0xff),
            badge_size: 0.3,
            badge_margin: 0.05,
            badge: Color::rgb(0x5c, 0x63, 0x6e),
            running: Color::rgb(0xf0, 0x8c, 0x00),
            success: Color::rgb(0x2f, 0x9e, 0x44),
            error: Color::rgb(0xe0, 0x31, 0x31),
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
    /// A page number the profile does not have.
    #[error("profile `{profile}` has no page {page}; it has {pages}")]
    NoSuchPage {
        /// Profile id.
        profile: String,
        /// The requested page, counted from 1.
        page: usize,
        /// Pages in the profile.
        pages: usize,
    },
    /// A crop that does not lie inside the canvas.
    #[error("{width}x{height} at {x},{y} is outside the {canvas_width}x{canvas_height} canvas")]
    OutsideCanvas {
        /// Left edge of the crop.
        x: u16,
        /// Top edge of the crop.
        y: u16,
        /// Crop width.
        width: u16,
        /// Crop height.
        height: u16,
        /// Canvas width.
        canvas_width: u16,
        /// Canvas height.
        canvas_height: u16,
    },
    /// A key the panel does not have.
    #[error("no key {key} on a panel of {keys} keys")]
    NoSuchKey {
        /// The key, counted from 1.
        key: usize,
        /// Keys on the panel.
        keys: usize,
    },
}

/// JPEG quality of a tile: 100, all quantisation values 1, as Options+
/// sends (spec §5.5).
pub const JPEG_QUALITY: u8 = 100;

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

    /// The pixels as RGB, transparent parts over black.
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

    /// The baseline JPEG the device draws (spec §5.5), transparent parts
    /// over black.
    ///
    /// # Errors
    ///
    /// [`RenderError::Jpeg`].
    pub fn jpeg(&self) -> Result<Vec<u8>, RenderError> {
        self.encode(&self.rgb())
    }

    fn encode(&self, rgb: &[u8]) -> Result<Vec<u8>, RenderError> {
        let size = self.size();
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
            .encode(
                rgb,
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

/// The whole display as one image (spec §5.5, §9): the panel background of
/// the active profile with every key's tile at its place. It is kept
/// between paints; a change redraws the keys it touches and the device gets
/// a crop of the area that changed, so a key and the background around it
/// always come from the same image.
#[derive(Debug, Clone, PartialEq)]
pub struct Canvas {
    pixmap: Pixmap,
    panel: Panel,
    background: Color,
}

impl Canvas {
    /// The panel it was made for.
    #[must_use]
    pub fn panel(&self) -> &Panel {
        &self.panel
    }

    /// The panel background it was made with.
    #[must_use]
    pub fn background(&self) -> Color {
        self.background
    }

    /// The pixels of `area`, as a tile to encode.
    ///
    /// # Errors
    ///
    /// [`RenderError::OutsideCanvas`] for an area that does not lie inside
    /// the canvas or has no pixels.
    pub fn crop(&self, area: KeyPlace) -> Result<Tile, RenderError> {
        let outside = || RenderError::OutsideCanvas {
            x: area.x,
            y: area.y,
            width: area.size.width,
            height: area.size.height,
            canvas_width: self.panel.size.width,
            canvas_height: self.panel.size.height,
        };
        let fits = u32::from(area.x) + u32::from(area.size.width) <= self.pixmap.width()
            && u32::from(area.y) + u32::from(area.size.height) <= self.pixmap.height();
        if !fits {
            return Err(outside());
        }
        let rect = IntRect::from_xywh(
            i32::from(area.x),
            i32::from(area.y),
            u32::from(area.size.width),
            u32::from(area.size.height),
        )
        .ok_or_else(outside)?;
        let pixmap = self.pixmap.clone_rect(rect).ok_or_else(outside)?;
        Ok(Tile { pixmap })
    }
}

/// Renders key specs into tiles.
#[derive(Debug, Clone)]
pub struct Renderer {
    theme: Theme,
    /// Only the embedded font: labels look the same on every machine.
    fonts: Arc<fontdb::Database>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    /// A renderer with the default theme.
    #[must_use]
    pub fn new() -> Self {
        let mut fonts = fontdb::Database::new();
        fonts.load_font_data(INTER.to_vec());
        Self {
            theme: Theme::default(),
            fonts: Arc::new(fonts),
        }
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
        let label = spec
            .label
            .as_deref()
            .and_then(|text| self.fit_label(text, size));
        let (extent, center) = if label.is_some() {
            (self.theme.labeled_icon_size, self.theme.labeled_icon_center)
        } else {
            (self.theme.icon_size, self.theme.icon_center)
        };
        let (extent, center) = (extent * height, center * height);
        if let Some(reference) = &spec.icon {
            let ink = if matches!(spec.look, Look::Running | Look::RunningLong) {
                mix(foreground, background, self.theme.dim)
            } else {
                foreground
            };
            draw_icon(
                &mut pixmap,
                reference,
                ink,
                (width - extent) / 2.0,
                center - extent / 2.0,
                extent,
            );
        }
        if spec.look == Look::Running {
            let arc = SpinnerArc {
                center: (width / 2.0, center),
                radius: extent / 2.0 + self.theme.spinner_gap * height,
                start: f32::from(spec.spinner),
                sweep: self.theme.spinner_sweep,
                width: self.theme.spinner_width * height,
            };
            draw_arc(&mut pixmap, &arc, spec.accent.unwrap_or(self.theme.spinner));
        }
        if let Some(label) = label {
            let baseline = if spec.icon.is_some() {
                self.theme.label_baseline * height
            } else {
                // Centered on the icon's center line: half the cap height
                // below it.
                self.theme.icon_center * height + CAP_HEIGHT / 2.0 * label.font_size
            };
            if let Some(tree) = self.text_tree(
                &label.text,
                label.font_size,
                foreground,
                size,
                (width / 2.0, baseline),
            ) {
                resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
            }
        }
        self.draw_badge(&mut pixmap, spec, size);
        Ok(Tile { pixmap })
    }

    /// The corner badge: the look's color and symbol, the spec's text in
    /// place of the symbol; a custom state's text in the neutral color.
    fn draw_badge(&self, pixmap: &mut Pixmap, spec: &KeySpec, size: TileSize) {
        let (color, symbol) = match spec.look {
            Look::RunningLong => (self.theme.running, "lucide:refresh-cw"),
            Look::Success => (self.theme.success, "lucide:check"),
            Look::Error => (self.theme.error, "lucide:x"),
            Look::Idle | Look::Running if spec.badge.is_some() => (self.theme.badge, ""),
            Look::Idle | Look::Running => return,
        };
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        let diameter = self.theme.badge_size * height;
        let margin = self.theme.badge_margin * height;
        let (cx, cy) = (width - margin - diameter / 2.0, margin + diameter / 2.0);
        if let Some(circle) = PathBuilder::from_circle(cx, cy, diameter / 2.0) {
            pixmap.fill_path(
                &circle,
                &paint(color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        let ink = self.theme.foreground(color);
        if let Some(text) = spec.badge.as_deref().map(str::trim) {
            let font_size = (BADGE_TEXT * diameter).round();
            let baseline = cy + CAP_HEIGHT / 2.0 * font_size;
            if let Some(tree) = self.text_tree(text, font_size, ink, size, (cx, baseline)) {
                resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
            }
        } else {
            let extent = BADGE_ICON * diameter;
            draw_icon(
                pixmap,
                symbol,
                ink,
                cx - extent / 2.0,
                cy - extent / 2.0,
                extent,
            );
        }
    }

    /// The panel background of a profile with `accent`: the theme's panel
    /// color, tinted with the accent by [`Theme::panel_tint`].
    #[must_use]
    pub fn panel_color(&self, accent: Option<Color>) -> Color {
        accent.map_or(self.theme.panel, |accent| {
            mix(self.theme.panel, accent, self.theme.panel_tint)
        })
    }

    /// A tile of `size` in the panel background of a profile with
    /// `accent`.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTile`] for a zero width or height.
    pub fn backdrop(&self, accent: Option<Color>, size: TileSize) -> Result<Tile, RenderError> {
        let mut pixmap = Pixmap::new(u32::from(size.width), u32::from(size.height)).ok_or(
            RenderError::EmptyTile {
                width: size.width,
                height: size.height,
            },
        )?;
        let color = self.panel_color(accent);
        pixmap.fill(tiny_skia::Color::from_rgba8(
            color.r, color.g, color.b, 0xff,
        ));
        Ok(Tile { pixmap })
    }

    /// A [`Canvas`] of `panel` in the panel background of a profile with
    /// `accent`, with no key drawn yet.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTile`] for a panel of zero size.
    pub fn canvas(&self, accent: Option<Color>, panel: &Panel) -> Result<Canvas, RenderError> {
        let Tile { pixmap } = self.backdrop(accent, panel.size)?;
        Ok(Canvas {
            pixmap,
            panel: panel.clone(),
            background: self.panel_color(accent),
        })
    }

    /// Draws `spec` as key `index` (counted from 0) of the canvas: the key's
    /// rectangle is reset to the panel background, then the tile goes on
    /// it, so nothing of the key's earlier tile stays in its rounded
    /// corners.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoSuchKey`] for a key the panel does not have,
    /// [`RenderError::EmptyTile`] for a key of zero size.
    pub fn draw(
        &self,
        canvas: &mut Canvas,
        index: usize,
        spec: &KeySpec,
    ) -> Result<(), RenderError> {
        let place = *canvas.panel.keys.get(index).ok_or(RenderError::NoSuchKey {
            key: index + 1,
            keys: canvas.panel.keys.len(),
        })?;
        let tile = self.tile(spec, place.size)?;
        if let Some(rect) = Rect::from_xywh(
            f32::from(place.x),
            f32::from(place.y),
            f32::from(place.size.width),
            f32::from(place.size.height),
        ) {
            canvas
                .pixmap
                .fill_rect(rect, &paint(canvas.background), Transform::identity(), None);
        }
        canvas.pixmap.draw_pixmap(
            i32::from(place.x),
            i32::from(place.y),
            tile.pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        Ok(())
    }

    /// The whole panel: the [`Canvas`] of the specs' accent with the tile of
    /// `specs[i]` at `panel.keys[i]` (a blank tile where `specs` runs out),
    /// for previews.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTile`] for a panel or key of zero size.
    pub fn panel(&self, specs: &[KeySpec], panel: &Panel) -> Result<Tile, RenderError> {
        let accent = specs.iter().find_map(|spec| spec.accent);
        let mut canvas = self.canvas(accent, panel)?;
        let blank = KeySpec::default();
        for index in 0..panel.keys.len() {
            self.draw(&mut canvas, index, specs.get(index).unwrap_or(&blank))?;
        }
        Ok(Tile {
            pixmap: canvas.pixmap,
        })
    }

    /// The largest font size, from the theme's label size down to its
    /// minimum, at which `text` fits the key's width; below the minimum the
    /// text is cut and ends with an ellipsis. `None` for a blank label.
    #[must_use]
    pub fn fit_label(&self, text: &str, size: TileSize) -> Option<FittedLabel> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let height = f32::from(size.height);
        let room = f32::from(size.width) - 2.0 * self.theme.label_margin * height;
        let largest = (self.theme.label_size * height).round();
        let smallest = (self.theme.min_label_size * height).round();
        let fits = |label: &FittedLabel| {
            let center = (f32::from(size.width) / 2.0, height / 2.0);
            self.text_tree(
                &label.text,
                label.font_size,
                self.theme.light_foreground,
                size,
                center,
            )
            .is_some_and(|tree| tree.root().abs_bounding_box().width() <= room)
        };
        let mut font_size = largest;
        while font_size >= smallest {
            let label = FittedLabel {
                text: text.to_owned(),
                font_size,
            };
            if fits(&label) {
                return Some(label);
            }
            font_size -= 1.0;
        }
        let chars: Vec<char> = text.chars().collect();
        (1..chars.len())
            .rev()
            .map(|keep| FittedLabel {
                text: format!(
                    "{}\u{2026}",
                    chars[..keep].iter().collect::<String>().trim_end()
                ),
                font_size: smallest,
            })
            .find(|label| fits(label))
            .or(Some(FittedLabel {
                text: "\u{2026}".to_owned(),
                font_size: smallest,
            }))
    }

    /// The SVG tree of `text` at `font_size` on a tile of `size`, centered
    /// on `x` with its baseline at `baseline`.
    fn text_tree(
        &self,
        text: &str,
        font_size: f32,
        color: Color,
        size: TileSize,
        (x, baseline): (f32, f32),
    ) -> Option<usvg::Tree> {
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}">"#,
                r#"<text x="{x}" y="{y}" font-family="{family}" font-weight="{weight}" "#,
                r#"font-size="{size}" text-anchor="middle" fill="{color}">{text}</text></svg>"#,
            ),
            w = size.width,
            h = size.height,
            x = x,
            y = baseline,
            family = FONT_FAMILY,
            weight = FONT_WEIGHT,
            size = font_size,
            color = color,
            text = escape(text),
        );
        let options = usvg::Options {
            fontdb: Arc::clone(&self.fonts),
            font_family: FONT_FAMILY.to_owned(),
            ..usvg::Options::default()
        };
        usvg::Tree::from_str(&svg, &options).ok()
    }
}

/// A label as it is drawn: the text, cut if needed, and its font size in
/// pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedLabel {
    /// The text, ending with an ellipsis when it was cut.
    pub text: String,
    /// Font size in pixels.
    pub font_size: f32,
}

/// The embedded label font: Inter 4.1, semibold weight (SIL OFL 1.1,
/// <https://github.com/rsms/inter>, licence text in
/// `assets/fonts/LICENSE.txt`).
const INTER: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
/// The family name inside [`INTER`].
const FONT_FAMILY: &str = "Inter";
/// The weight of [`INTER`].
const FONT_WEIGHT: u16 = 600;
/// Inter's cap height as a fraction of the font size (2048 of 2816 units).
const CAP_HEIGHT: f32 = 0.727;

/// Badge text size as a fraction of the badge diameter.
const BADGE_TEXT: f32 = 0.62;
/// Badge symbol size as a fraction of the badge diameter.
const BADGE_ICON: f32 = 0.62;

/// `a` moved toward `b` by `t` (0 is `a`, 1 is `b`).
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) / 255.0;
    tiny_skia::Color::from_rgba(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b), 1.0)
        .map_or(a, |mixed| {
            let mixed = mixed.to_color_u8();
            Color::rgb(mixed.red(), mixed.green(), mixed.blue())
        })
}

/// A stroked circular arc; angles in degrees clockwise from the top.
struct SpinnerArc {
    center: (f32, f32),
    radius: f32,
    start: f32,
    sweep: f32,
    width: f32,
}

/// Strokes `arc` in `color` with round caps.
fn draw_arc(pixmap: &mut Pixmap, arc: &SpinnerArc, color: Color) {
    const SEGMENTS: u16 = 24;
    let point = |degrees: f32| {
        let radians = degrees.to_radians();
        (
            arc.center.0 + arc.radius * radians.sin(),
            arc.center.1 - arc.radius * radians.cos(),
        )
    };
    let mut path = PathBuilder::new();
    let (x, y) = point(arc.start);
    path.move_to(x, y);
    for step in 1..=SEGMENTS {
        let (x, y) = point(arc.start + arc.sweep * f32::from(step) / f32::from(SEGMENTS));
        path.line_to(x, y);
    }
    if let Some(path) = path.finish() {
        let stroke = Stroke {
            width: arc.width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint(color), &stroke, Transform::identity(), None);
    }
}

/// `text` with the XML special characters escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(c),
        }
    }
    escaped
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
    fn tiles_encode_as_baseline_jpeg_without_chroma_subsampling() {
        let jpeg = Renderer::new()
            .tile(
                &KeySpec::default(),
                TileSize {
                    width: 118,
                    height: 118,
                },
            )
            .unwrap()
            .jpeg()
            .unwrap();
        let sof = jpeg
            .windows(2)
            .position(|marker| marker == [0xff, 0xc0])
            .unwrap();
        // Length, precision, height, width, component count, then id,
        // sampling (h << 4 | v) and table per component.
        let header = &jpeg[sof + 2..sof + 19];
        assert_eq!(&header[3..7], &[0, 118, 0, 118]);
        assert_eq!(header[7], 3);
        let sampling: Vec<_> = header[8..].chunks(3).map(|c| c[1]).collect();
        assert_eq!(sampling, [0x11, 0x11, 0x11], "4:4:4");
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

    #[test]
    fn the_panel_is_black_without_an_accent_and_tinted_with_one() {
        let renderer = Renderer::new();
        assert_eq!(renderer.panel_color(None), Color::rgb(0, 0, 0));
        let accent = Color::rgb(0x4e, 0x8b, 0xd6);
        assert_eq!(renderer.panel_color(Some(accent)), Color::rgb(16, 28, 43));
    }

    #[test]
    fn the_backdrop_is_the_panel_color_everywhere() {
        let accent = Color::rgb(0x4e, 0x8b, 0xd6);
        let tile = Renderer::new()
            .backdrop(
                Some(accent),
                TileSize {
                    width: 6,
                    height: 4,
                },
            )
            .unwrap();
        assert_eq!(tile.rgb(), [16, 28, 43].repeat(6 * 4));
        assert!(
            Renderer::new()
                .backdrop(
                    None,
                    TileSize {
                        width: 0,
                        height: 4
                    }
                )
                .is_err()
        );
    }

    #[test]
    fn transparent_corners_take_the_canvas_background() {
        let renderer = Renderer::new();
        let size = TileSize {
            width: 40,
            height: 40,
        };
        let place = KeyPlace { x: 5, y: 5, size };
        let panel = Panel {
            size: TileSize {
                width: 50,
                height: 50,
            },
            keys: vec![place],
        };
        let accent = Some(Color::rgb(0x4e, 0x8b, 0xd6));
        let mut canvas = renderer.canvas(accent, &panel).unwrap();
        assert_eq!(canvas.background(), Color::rgb(16, 28, 43));
        renderer.draw(&mut canvas, 0, &KeySpec::default()).unwrap();
        let rgb = canvas.crop(place).unwrap().rgb();
        assert_eq!(&rgb[..3], &[16, 28, 43]);
        // An opaque pixel keeps the tile's color.
        let middle = (20 * 40 + 20) * 3;
        let tile = renderer.tile(&KeySpec::default(), size).unwrap().rgb();
        assert_eq!(&rgb[middle..middle + 3], &tile[middle..middle + 3]);
    }
}
