//! Original archive lettering and artwork shared by menus and gameplay overlays.
use crate::{assets::Assets, bsp, texture};
use anyhow::{ensure, Context, Result};
use macroquad::prelude::*;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

pub const INK: Color = Color::new(0.199, 0.043, 0.043, 1.);
pub const SELECTED: Color = Color::new(0.51, 0.11, 0.07, 1.);
pub const PAPER_TEXT: Color = Color::new(0.94, 0.89, 0.78, 1.);

/// Keep gameplay lettering readable above the original reference resolution,
/// while leaving enough horizontal space in small or narrow windows.
pub fn overlay_scale() -> f32 {
    (screen_height() / 900.).max(1.).min(screen_width() / 900.).clamp(0.6, 4.)
}

#[derive(Clone, Copy)]
pub struct Canvas {
    scale: f32,
    offset: Vec2,
}
impl Canvas {
    pub fn new(w: f32, h: f32) -> Self {
        let scale = (w / 640.).min(h / 480.).max(0.001);
        Self {
            scale,
            offset: (vec2(w, h) - vec2(640., 480.) * scale) * 0.5,
        }
    }
    pub fn rect(self, r: Rect) -> Rect {
        Rect::new(
            self.offset.x + r.x * self.scale,
            self.offset.y + r.y * self.scale,
            r.w * self.scale,
            r.h * self.scale,
        )
    }
    pub fn pointer(self, p: Vec2) -> Vec2 {
        (p - self.offset) / self.scale
    }
}

pub struct BitmapFont {
    texture: Texture2D,
    indices: Vec<i32>,
    glyphs: Vec<Rect>,
    ink_bounds: Vec<Option<Rect>>,
    height: f32,
}
pub(crate) struct TextTransform {
    pub visible_center: bool,
    pub radians: f32,
    pub anchor: Vec2,
}

fn font_character(c: char) -> char {
    match c {
        '’' | '‘' => '\'',
        '“' | '”' => '"',
        '—' | '–' => '-',
        _ => c,
    }
}

// Unlike drawing, this lookup must never substitute the question-mark glyph.
fn direct_glyph_index(indices: &[i32], c: char) -> Option<usize> {
    indices.get(font_character(c) as usize).copied()
        .filter(|&i| i >= 0).map(|i| i as usize)
}

fn vertical_ink_bounds(ink: impl Iterator<Item = Rect>, scale: f32) -> Option<(f32, f32)> {
    ink.fold(None, |bounds, r| {
        Some(bounds.map_or((r.y * scale, r.bottom() * scale), |(top, bottom): (f32, f32)|
            (top.min(r.y * scale), bottom.max(r.bottom() * scale))))
    })
}

impl BitmapFont {
    pub fn load(assets: &mut Assets, name: &str) -> Result<Self> {
        let tokens = bsp::tokens(&String::from_utf8_lossy(
            &assets.read(&format!("fonts/{name}.ritualfont"))?,
        ))?;
        let start = tokens
            .iter()
            .position(|s| s == "indirections")
            .context("Font indirections")?
            + 2;
        let end = tokens[start..]
            .iter()
            .position(|s| s == "}")
            .context("Font end")?
            + start;
        let indices = tokens[start..end]
            .iter()
            .map(|s| s.parse())
            .collect::<std::result::Result<Vec<i32>, _>>()?;
        ensure!(indices.len() == 256, "Invalid bitmap font indirections");
        let start = tokens
            .iter()
            .position(|s| s == "locations")
            .context("Font locations")?
            + 2;
        let mut glyphs = Vec::new();
        let mut i = start;
        while tokens.get(i).is_some_and(|s| s == "{") {
            let values = tokens
                .get(i + 1..i + 5)
                .context("Truncated glyph")?
                .iter()
                .map(|s| s.parse::<f32>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ensure!(
                values
                    .iter()
                    .all(|n| n.is_finite() && (0. ..=4096.).contains(n)),
                "Invalid glyph"
            );
            glyphs.push(Rect::new(values[0], values[1], values[2], values[3]));
            i += 6;
        }
        ensure!(
            indices
                .iter()
                .all(|&i| i == -1 || (i >= 0 && (i as usize) < glyphs.len())),
            "Invalid glyph reference"
        );
        // Measure the visible ink once, using the same atlas that is rendered.
        // Transparent padding belongs to the line metrics, not button centering.
        let path = if name == "asrafel" && assets.contains("gfx/fonts/asrafel.tga") {
            "gfx/fonts/asrafel.tga".to_owned()
        } else {format!("gfx/2d/fonts/{name}.tga")};
        let atlas = texture::decode(assets, &path)?;
        let ink_bounds = glyphs.iter().map(|glyph| {
            let mut left = atlas.width as usize;
            let mut top = atlas.height as usize;
            let mut right = 0;
            let mut bottom = 0;
            for y in glyph.y.floor() as usize..(glyph.bottom().ceil() as usize).min(atlas.height as usize) {
                for x in glyph.x.floor() as usize..(glyph.right().ceil() as usize).min(atlas.width as usize) {
                    // The atlas contains nearly transparent residue extending to
                    // glyph-cell edges. Measure the visible contour at half opacity,
                    // rather than letting that residue determine the text center.
                    if atlas.pixels[(y * atlas.width as usize + x) * 4 + 3] >= 128 {
                        left = left.min(x);
                        top = top.min(y);
                        right = right.max(x + 1);
                        bottom = bottom.max(y + 1);
                    }
                }
            }
            (right > left && bottom > top).then(|| Rect::new(
                left as f32 - glyph.x, top as f32 - glyph.y,
                (right - left) as f32, (bottom - top) as f32))
        }).collect();
        let texture = Texture2D::from_rgba8(atlas.width, atlas.height, &atlas.pixels);
        texture.set_filter(FilterMode::Linear);
        Ok(Self {
            texture,
            indices,
            glyphs,
            ink_bounds,
            height: tokens
                .get(2)
                .context("Font height")?
                .parse::<f32>()?
                .clamp(1., 128.),
        })
    }
    fn glyph_index(&self, c: char) -> Option<usize> {
        let c = font_character(c);
        let n = c as usize;
        let i = self
            .indices
            .get(n)
            .copied()
            .unwrap_or(self.indices[b'?' as usize]);
        (i >= 0).then_some(i as usize)
    }
    pub(crate) fn supports_character(&self, c: char) -> bool {
        direct_glyph_index(&self.indices, c).is_some()
    }
    pub fn glyph(&self, c: char) -> Rect {
        self.glyph_index(c).map_or(Rect::default(), |i| self.glyphs[i])
    }
    pub fn height(&self) -> f32 {
        self.height
    }
    pub(crate) fn visible_vertical_bounds(&self, text: &str, size: f32) -> Option<(f32, f32)> {
        vertical_ink_bounds(text.chars().filter_map(|c| self.glyph_index(c)
            .and_then(|i| self.ink_bounds[i])), size / self.height)
    }
    pub fn width(&self, text: &str, size: f32) -> f32 {
        self.width_spaced(text, size, 0.)
    }
    pub fn width_spaced(&self, text: &str, size: f32, spacing: f32) -> f32 {
        (text.chars().map(|c| self.glyph(c).w).sum::<f32>()
            + text.chars().count().saturating_sub(1) as f32 * spacing) * size / self.height
    }
    pub fn left(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.draw(
            Canvas {
                scale: 1.,
                offset: Vec2::ZERO,
            },
            text,
            Rect::new(x, y - size, self.width(text, size), size),
            size,
            color,
        );
    }
    pub fn draw_left(&self, canvas: Canvas, text: &str, mut r: Rect, size: f32, color: Color) {
        r.w = r.w.min(self.width(text, size));
        self.draw(canvas, text, r, size, color);
    }
    pub fn draw(&self, canvas: Canvas, text: &str, r: Rect, size: f32, color: Color) {
        self.draw_spaced(canvas, text, r, size, color, 0.);
    }
    pub fn draw_spaced(&self, canvas: Canvas, text: &str, r: Rect, size: f32, color: Color, spacing: f32) {
        self.draw_transformed(canvas, text, r, size, color, spacing,
            TextTransform {visible_center: false, radians: 0., anchor: r.center()});
    }
    pub(crate) fn draw_button(&self, canvas: Canvas, text: &str, r: Rect, size: f32, color: Color) {
        self.draw_transformed(canvas, text, r, size, color, 0.,
            TextTransform {visible_center: true, radians: 0., anchor: r.center()});
    }
    pub(crate) fn draw_transformed(&self, canvas: Canvas, text: &str, r: Rect, size: f32, color: Color,
        spacing: f32, transform: TextTransform) {
        let width: f32 = text.chars().map(|c| self.glyph(c).w).sum::<f32>()
            + text.chars().count().saturating_sub(1) as f32 * spacing;
        let scale = (size / self.height).min(r.w / width.max(1.));
        let mut x = r.x + (r.w - width * scale) * 0.5;
        let mut y = r.y + (r.h - self.height * scale) * 0.5;
        if transform.visible_center {
            let ink = self.visible_vertical_bounds(text, self.height);
            if let Some((top, bottom)) = ink {
                let visible_height = (bottom - top) * scale;
                y = r.y + r.h * 0.5 - visible_height * 0.5 - top * scale;
            }
        }
        let pivot = canvas.rect(Rect::new(transform.anchor.x, transform.anchor.y, 0., 0.)).point();
        for c in text.chars() {
            let glyph = self.glyph(c);
            if glyph.w > 0. && glyph.h > 0. {
                let dest = canvas.rect(Rect::new(x, y, glyph.w * scale, glyph.h * scale));
                draw_texture_ex(
                    &self.texture,
                    dest.x,
                    dest.y,
                    color,
                    DrawTextureParams {
                        dest_size: Some(dest.size()),
                        source: Some(glyph),
                        rotation: transform.radians,
                        pivot: Some(pivot),
                        ..Default::default()
                    },
                );
            }
            x += (glyph.w + spacing) * scale;
        }
    }
}

pub fn load_texture(assets: &mut Assets, path: &str) -> Result<Texture2D> {
    let im = texture::decode(assets, path)?;
    let tex = Texture2D::from_rgba8(im.width, im.height, &im.pixels);
    tex.set_filter(FilterMode::Linear);
    Ok(tex)
}

pub struct Ui {
    pub font: BitmapFont,
    pub body: BitmapFont,
    pub courier: BitmapFont,
    left: Texture2D,
    right: Texture2D,
    talk: Texture2D,
    plaque: [Texture2D; 2],
    backdrop: Texture2D,
    pause: Texture2D,
    cursor: Texture2D,
    arrow: Texture2D,
}
thread_local! { static SHARED: RefCell<Weak<Ui>> = const { RefCell::new(Weak::new()) }; }
impl Ui {
    pub fn load(assets: &mut Assets) -> Result<Rc<Self>> {
        if let Some(ui) = SHARED.with(|s| s.borrow().upgrade()) {
            return Ok(ui);
        }
        let ui = Rc::new(Self {
            font: BitmapFont::load(assets, "asrafel")?,
            body: BitmapFont::load(assets, "verdana-12")?,
            courier: BitmapFont::load(assets, "courier-14")?,
            left: load_texture(assets, "ui/dialog/leftframe.tga")?,
            right: load_texture(assets, "ui/dialog/rightframe.tga")?,
            talk: load_texture(assets, "gfx/2d/talkmenu.tga")?,
            plaque: [
                load_texture(assets, "ui/buttons/plaque.tga")?,
                load_texture(assets, "ui/buttons/plaque_hover.tga")?,
            ],
            backdrop: load_texture(assets, "gfx/2d/backtile.tga")?,
            pause: load_texture(assets, "ui/pausewatch/pausewatch.tga")?,
            cursor: load_texture(assets, "gfx/2d/mouse_arrow.tga")?,
            arrow: load_texture(assets, "ui/control/hslider_next.tga")?,
        });
        SHARED.with(|s| *s.borrow_mut() = Rc::downgrade(&ui));
        Ok(ui)
    }
    pub fn panel(&self, r: Rect) {
        nine(&self.left, Rect::new(r.x, r.y, r.w * 0.5, r.h), 16.);
        nine(
            &self.right,
            Rect::new(r.x + r.w * 0.5, r.y, r.w * 0.5, r.h),
            16.,
        );
    }
    pub fn dialog(&self, r: Rect) {
        nine(&self.talk, r, 16.);
    }
    pub fn plaque(&self, r: Rect, selected: bool) {
        nine(&self.plaque[selected as usize], r, 16.);
    }
    pub fn tile(&self, r: Rect) {
        let size = 96.;
        for y in 0..(r.h / size).ceil() as usize {
            for x in 0..(r.w / size).ceil() as usize {
                let w = (r.w - x as f32 * size).min(size);
                let h = (r.h - y as f32 * size).min(size);
                draw_texture_ex(
                    &self.backdrop,
                    r.x + x as f32 * size,
                    r.y + y as f32 * size,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(w, h)),
                        source: Some(Rect::new(0., 0., w / size * 64., h / size * 64.)),
                        ..Default::default()
                    },
                );
            }
        }
    }
    pub fn center(&self, text: &str, r: Rect, size: f32, color: Color) {
        self.font.draw(
            Canvas {
                scale: 1.,
                offset: Vec2::ZERO,
            },
            text,
            r,
            size,
            color,
        );
    }
    pub fn label(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.font.left(text, x + 1., y + 1., size, BLACK);
        self.font.left(text, x, y, size, color);
    }
    pub fn fit_label(&self, text: &str, x: f32, y: f32, size: f32, width: f32, color: Color) {
        self.label(
            text,
            x,
            y,
            size * (width / self.font.width(text, size).max(1.)).min(1.),
            color,
        );
    }
    pub fn toast(&self, text: &str, y: f32) {
        let s = overlay_scale();
        let size = 22. * s;
        let w = (self.font.width(text, size) + 56. * s)
            .min(760. * s).min(screen_width() - 32. * s);
        let mut rows = crate::story::wrap(text, w - 44. * s, |t| self.font.width(t, size));
        if rows.len() > 4 {
            rows.truncate(4);
            let last = rows.last_mut().unwrap();
            while self.font.width(&format!("{last}..."), size) > w - 44. * s && !last.is_empty() {
                last.pop();
            }
            last.push_str("...");
        }
        let height = (18. + 26. * rows.len().max(1) as f32) * s;
        let y = y.clamp(12. * s, (screen_height() - height - 12. * s).max(12. * s));
        let r = Rect::new((screen_width() - w) * 0.5, y, w, height);
        self.dialog(r);
        for (i, row) in rows.iter().enumerate() {
            self.center(row, Rect::new(r.x + 22. * s, r.y + (8. + i as f32 * 26.) * s,
                r.w - 44. * s, 26. * s), size, WHITE);
        }
    }
    pub fn paused(&self, prompt: &str) {
        let s = (screen_height() / 600.).clamp(0.7, 1.5 * overlay_scale());
        draw_texture_ex(
            &self.pause,
            screen_width() * 0.5 - 48. * s,
            22. * s,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(96., 96.) * s),
                ..Default::default()
            },
        );
        self.center(
            "Paused",
            Rect::new(screen_width() * 0.5 - 80. * s, 126. * s, 160. * s, 28. * s),
            26. * s,
            PAPER_TEXT,
        );
        self.center(
            prompt,
            Rect::new(screen_width() * 0.5 - 180. * s, 158. * s, 360. * s, 24. * s),
            20. * s,
            PAPER_TEXT,
        );
    }
    pub fn marker(&self, r: Rect) {
        draw_texture_ex(
            &self.arrow,
            r.x,
            r.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(r.size()),
                ..Default::default()
            },
        );
    }
    pub fn cursor(&self) {
        let p = mouse_position();
        let scale = (screen_height() / 600.).clamp(0.75, 2.);
        draw_texture_ex(
            &self.cursor,
            p.0,
            p.1,
            WHITE,
            DrawTextureParams {
                dest_size: Some(
                    vec2(24., 24. * self.cursor.height() / self.cursor.width()) * scale,
                ),
                ..Default::default()
            },
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_ink_bounds_include_accents_and_descenders_at_each_scale() {
        let glyphs = [Rect::new(0., 4., 8., 12.), // ordinary letter
            Rect::new(0., 1., 8., 15.), // umlaut
            Rect::new(0., 5., 8., 17.)]; // descender
        for scale in [0.5, 1., 2.] {
            assert_eq!(vertical_ink_bounds(glyphs.into_iter(), scale),
                Some((scale, 22. * scale)));
        }
        assert_eq!(vertical_ink_bounds(std::iter::empty(), 1.), None);
    }

    #[test]
    fn subtitle_coverage_does_not_use_fallback_glyph() {
        let mut indices = vec![-1; 256];
        indices[b'?' as usize] = 0;
        indices[b'A' as usize] = 1;
        assert_eq!(direct_glyph_index(&indices, 'A'), Some(1));
        assert_eq!(direct_glyph_index(&indices, '?'), Some(0));
        assert_eq!(direct_glyph_index(&indices, 'ä'), None);
        assert_eq!(direct_glyph_index(&indices, '漢'), None);
    }

    #[test]
    fn subtitle_coverage_matches_rendered_punctuation() {
        let mut indices = vec![-1; 256];
        indices[b'\'' as usize] = 0;
        indices[b'"' as usize] = 1;
        indices[b'-' as usize] = 2;
        assert_eq!(direct_glyph_index(&indices, '’'), Some(0));
        assert_eq!(direct_glyph_index(&indices, '“'), Some(1));
        assert_eq!(direct_glyph_index(&indices, '—'), Some(2));
    }
}

fn nine(t: &Texture2D, r: Rect, b: f32) {
    let dx = b.min(r.w * 0.25);
    let dy = b.min(r.h * 0.25);
    let sx = [0., b, t.width() - b, t.width()];
    let sy = [0., b, t.height() - b, t.height()];
    let x = [r.x, r.x + dx, r.right() - dx, r.right()];
    let y = [r.y, r.y + dy, r.bottom() - dy, r.bottom()];
    for row in 0..3 {
        for col in 0..3 {
            draw_texture_ex(
                t,
                x[col],
                y[row],
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(x[col + 1] - x[col], y[row + 1] - y[row])),
                    source: Some(Rect::new(
                        sx[col],
                        sy[row],
                        sx[col + 1] - sx[col],
                        sy[row + 1] - sy[row],
                    )),
                    ..Default::default()
                },
            );
        }
    }
}
