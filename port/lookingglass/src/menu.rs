//! Original archive menu art/layouts, with a small, explicit Rust action router.
//! URC commands are data only: no original console/script commands are executed.
use crate::{
    assets::Assets,
    audio, bsp,
    powerups::Difficulty,
    preferences::{Preferences, BINDINGS, SIZES},
    save::{Slot, Store},
    texture,
    ui::{Canvas, Ui},
};
use anyhow::{ensure, Context, Result};
use macroquad::prelude::*;
use std::collections::BTreeMap;
mod save_screen;
mod chapters;
pub use chapters::ChapterChoice;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Main,
    Settings,
    Video,
    Audio,
    Game,
    Controls,
    LoadSave,
    NewGame,
    Quit,
    Credits,
    ConfirmNew(Difficulty),
    ConfirmLoad(Slot),
    ConfirmSave(Slot),
}
impl Page {
    fn parent(self) -> Option<Self> {
        match self {
            Self::Main => None,
            Self::Video | Self::Audio | Self::Game | Self::Controls => Some(Self::Settings),
            Self::Credits => Some(Self::Quit),
            Self::ConfirmNew(_) => Some(Self::NewGame),
            Self::ConfirmLoad(_) | Self::ConfirmSave(_) => Some(Self::LoadSave),
            _ => Some(Self::Main),
        }
    }
    fn layout(self) -> &'static str {
        match self {
            Self::Settings | Self::Video | Self::Audio | Self::Game | Self::Controls => "controls",
            Self::LoadSave | Self::ConfirmLoad(_) | Self::ConfirmSave(_) => "loadsave",
            Self::NewGame | Self::ConfirmNew(_) => "newgame",
            Self::Quit => "quit",
            Self::Credits => "credits",
            _ => "main",
        }
    }
    fn settings(self) -> bool {
        matches!(
            self,
            Self::Video | Self::Audio | Self::Game | Self::Controls
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Resume,
    Chapters,
    Save(Slot),
    Load(Slot),
    NewGame(Difficulty),
    Quit,
}
#[derive(Clone, Copy)]
enum Click {
    Page(Page),
    Back,
    Action(Action),
    Row(usize),
    Apply,
    Cancel,
    Reset,
    CreditScroll(i32),
    ControlsPage(i32),
    Device,
    Slot(usize),
    SaveSelection,
    LoadSelection,
}
struct Button {
    rect: Rect,
    text: String,
    art: String,
    hover: String,
    click: Click,
    lettering: Option<Lettering>,
    setting: Option<SettingView>,
}
#[derive(Clone)]
struct SettingView {
    label: Widget,
    control: Widget,
}
#[derive(Clone)]
struct Lettering {
    offset: Vec2,
    scale: f32,
    justify: (String, String),
    spacing: f32,
    angle: f32,
    visible_center: bool,
    font: String,
    color: Color,
    horizontal_start: bool,
}
impl Lettering {
    fn from_widget(w: &Widget) -> Self {
        let number = |key: &str, index: usize, fallback: f32| {
            w.fields.get(key).and_then(|v| v.get(index))
                .and_then(|s| s.parse::<f32>().ok()).filter(|n| n.is_finite())
                .unwrap_or(fallback)
        };
        let alignment = |index| w.fields.get("fontjustify").and_then(|v| v.get(index))
            .cloned().unwrap_or_else(|| "center".into());
        Self {
            offset: vec2(number("fontpos", 0, 0.), number("fontpos", 1, 0.)),
            scale: number("fontscale", 0, 1.).clamp(0.1, 4.),
            justify: (alignment(0), alignment(1)),
            spacing: number("fontspacing", 0, 0.),
            angle: -number("fontangle", 0, 0.).rem_euclid(360.).to_radians(),
            visible_center: w.kind == "Button" && alignment(1) == "center",
            font: w.value("font").into(),
            color: Color::new(number("fgcolor", 0, crate::ui::INK.r),
                number("fgcolor", 1, crate::ui::INK.g), number("fgcolor", 2, crate::ui::INK.b),
                number("fgcolor", 3, 1.)),
            horizontal_start: w.kind == "Button" && w.fields.contains_key("fontpos"),
        }
    }
    fn text_rect(&self, button: Rect, text_width: f32, size: f32) -> Rect {
        // Trial interpretation: a button's declared fontpos X is its text start,
        // not an ornament margin followed by additional horizontal centering.
        let left = if self.horizontal_start {
            self.offset.x.clamp(0., button.w)
        } else {0.};
        let button = Rect::new(button.x+left, button.y, button.w-left, button.h);
        let scale = (button.w / text_width.max(1.)).min(button.h / size.max(1.)).min(1.);
        let width = text_width * scale;
        let height = size * scale;
        let align = |space: f32, offset: f32, mode: &str| match mode {
            "left" | "top" => offset.min(space).max(0.),
            "right" | "bottom" => (space - offset).max(0.), _ => space * 0.5,
        };
        let x = if self.horizontal_start {button.x} else {
            button.x + align(button.w - width, self.offset.x, &self.justify.0)
        };
        Rect::new(x,
            button.y + align(button.h - height, self.offset.y, &self.justify.1), width, height)
    }
}
impl Button {
    fn slider_rect(&self) -> Option<Rect> {
        if let Some(setting) = &self.setting {
            (setting.control.kind == "Slider").then_some(setting.control.rect)
        } else {
            Some(Rect::new(88., self.rect.y + 26., 200., 16.))
        }
    }
    fn text(rect: Rect, text: impl Into<String>, click: Click) -> Self {
        Self {
            rect,
            text: text.into(),
            art: "ui/buttons/apply".into(),
            hover: "ui/buttons/apply_hover".into(),
            click,
            lettering: None,
            setting: None,
        }
    }
}

#[derive(Clone, Debug)]
struct Widget {
    kind: String,
    fields: BTreeMap<String, Vec<String>>,
    rect: Rect,
}
impl Widget {
    fn value(&self, name: &str) -> &str {
        self.fields
            .get(name)
            .and_then(|v| v.first())
            .map_or("", String::as_str)
    }
    fn art(&self) -> &str {
        self.fields
            .get("staticshader")
            .and_then(|v| v.get(1))
            .map_or_else(|| self.value("shader"), String::as_str)
    }
    fn button(&self, click: Click) -> Button {
        Button {
            rect: self.rect,
            text: self.value("title").into(),
            art: self.art().into(),
            hover: self.value("hovershader").into(),
            click,
            lettering: Some(Lettering::from_widget(self)),
            setting: None,
        }
    }
}
fn layout(source: &str) -> Result<Vec<Widget>> {
    ensure!(source.len() < 256 * 1024, "Menu definition too large");
    let mut widgets = Vec::new();
    let mut current: Option<Widget> = None;
    let mut resource = false;
    for line in source.lines() {
        let tokens = bsp::tokens(line)?;
        let Some(key) = tokens.first() else {
            continue;
        };
        if key == "resource" {
            resource = true;
            continue;
        }
        if resource {
            current = Some(Widget {
                kind: key.clone(),
                fields: BTreeMap::new(),
                rect: Rect::default(),
            });
            resource = false;
        } else if key == "}" {
            if let Some(mut w) = current.take() {
                let r = w
                    .fields
                    .get("rect")
                    .context("Menu widget has no rectangle")?;
                ensure!(r.len() == 4, "Invalid menu rectangle");
                let v = r
                    .iter()
                    .map(|n| n.parse::<f32>())
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ensure!(
                    v.iter().all(|n| n.is_finite() && n.abs() <= 4096.) && v[2] >= 0. && v[3] >= 0.,
                    "Invalid menu bounds"
                );
                w.rect = Rect::new(v[0], v[1], v[2], v[3]);
                widgets.push(w);
            }
        } else if key != "{" {
            if let Some(w) = &mut current {
                w.fields
                    .insert(key.to_ascii_lowercase(), tokens[1..].to_vec());
            }
        }
    }
    ensure!(
        current.is_none() && !resource && !widgets.is_empty(),
        "Incomplete menu layout"
    );
    Ok(widgets)
}

fn main_layout(bytes: &[u8]) -> Result<Vec<Widget>> {
    let (source, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    layout(&source)
}

fn menu_texture_path(
    assets: &Assets,
    specs: &BTreeMap<String, texture::MaterialSpec>,
    key: &str,
) -> Option<String> {
    texture::resolve(assets, key, specs)
}

fn main_background_glow(art: &str) -> bool {
    art.starts_with("ui/main/main") && art.ends_with("_glow")
}

#[cfg(test)]
fn setting_label<'a>(widgets: &'a [Widget], cvar: &str, fallback: &'a str) -> &'a str {
    setting_view(widgets, cvar).map_or(fallback, |(label, _)| label.value("title"))
}
fn setting_view<'a>(widgets: &'a [Widget], cvar: &str) -> Option<(&'a Widget, &'a Widget)> {
    let index = widgets.iter().position(|w| w.value("linkcvar") == cvar)?;
    let control = &widgets[index];
    let label = widgets[..index].iter().rev().find(|w| w.kind == "Label"
        && w.value("groupid") == control.value("groupid") && !w.value("title").is_empty())?;
    Some((label, control))
}
fn setting_cvars(page: Page) -> &'static [&'static str] {
    match page {
        Page::Video => &["r_mode", "r_fullscreen"],
        Page::Audio => &["s_musicvolume", "s_volume"],
        Page::Game => &["sensitivity", "m_invert_pitch", "cg_cameradist", "cl_run", "ui_subtitles"],
        _ => &[],
    }
}
fn bounds(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect::new(x, y, a.right().max(b.right())-x, a.bottom().max(b.bottom())-y)
}
fn highlighted(selected: usize, mouse_navigation: bool, hit: Option<usize>, index: usize) -> bool {
    if mouse_navigation { hit == Some(index) } else { selected == index }
}
fn setting_buttons(widgets: &[Widget], page: Page) -> Vec<Button> {
    setting_cvars(page).iter().enumerate().filter_map(|(row, cvar)| {
        let (label, control) = setting_view(widgets, cvar)?;
        Some(Button { rect: bounds(label.rect, control.rect), text: label.value("title").into(),
            art: String::new(), hover: String::new(), click: Click::Row(row), lettering: None,
            setting: Some(SettingView {label: label.clone(), control: control.clone()}) })
    }).collect()
}

fn binding_labels(bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    let mut labels = BTreeMap::new();
    for line in text.lines() {
        let t = bsp::tokens(line)?;
        if t.first().is_some_and(|s| s == "binditem") && t.len() >= 3 {
            labels.insert(t[2].clone(), t[1].clone());
        }
    }
    Ok(labels)
}

fn binding_command(label: &str) -> Option<&'static str> {
    Some(match label {
        "Forward" => "+forward", "Backward" => "+back",
        "Step left" => "+moveleft", "Step right" => "+moveright",
        "Jump / climb" => "+moveup", "Dive / descend" => "+movedown",
        "Interact" => "+use", "Run / walk" => "+speed", "Cheshire hint" => "cheshire",
        "Primary attack" => "+attackright", "Alternate attack" => "+attackleft",
        "Previous toy" => "prevweapon", "Next toy" => "nextweapon",
        "Change view" => "+cameralook", "Pause" => "pause",
        "Quick save" => "savegame quicksave", "Quick load" => "loadgame quicksave",
        "Look left" => "+left", "Look right" => "+right",
        "Vorpal Blade" => "use knife", "Cards" => "use cards", "Mallet" => "use mallet",
        "Jackbomb" => "use jackbomb", "Ice Wand" => "use icewand", "Jacks" => "use jacks",
        "Demon Dice" => "use demondice", "Eye Staff" => "use eyestaff",
        "Blunderbuss" => "use blunderbuss", "Pocket Watch" => "use watch",
        _ => return None,
    })
}

fn load_texture(assets: &mut Assets, path: &str, additive: bool) -> Result<Texture2D> {
    Ok(load_texture_geometry(assets, path, additive)?.0)
}

fn load_texture_geometry(assets: &mut Assets, path: &str, additive: bool) -> Result<(Texture2D, Option<(f32, f32)>)> {
    let mut im = texture::decode(assets, path)?;
    // Black-backed additive menu highlights can use the stock alpha pipeline.
    if additive {
        for p in im.pixels.chunks_exact_mut(4) {
            let a = p[0].max(p[1]).max(p[2]);
            p[3] = ((p[3] as u16 * a as u16) / 255) as u8;
            if a > 0 {
                for c in &mut p[..3] {
                    *c = ((*c as u16 * 255) / a as u16) as u8;
                }
            }
        }
    }
    let tex = Texture2D::from_rgba8(im.width, im.height, &im.pixels);
    tex.set_filter(FilterMode::Linear);
    let mut top = im.height as usize;
    let mut bottom = 0;
    for (i, pixel) in im.pixels.chunks_exact(4).enumerate() {
        if pixel[3] >= 128 {
            let y = i / im.width as usize;
            top = top.min(y);
            bottom = bottom.max(y + 1);
        }
    }
    let bounds = (bottom > top).then_some((top as f32 / im.height as f32,
        bottom as f32 / im.height as f32));
    Ok((tex, bounds))
}

pub struct Menu {
    pub page: Option<Page>,
    pub message: String,
    pub post_game: bool,
    pub frontend: bool,
    layouts: BTreeMap<String, Vec<Widget>>,
    textures: BTreeMap<String, Texture2D>,
    button_bounds: BTreeMap<String, (f32, f32)>,
    binding_labels: BTreeMap<String, String>,
    map_titles: BTreeMap<String, String>,
    pub(crate) chapter_labels: crate::chapters::Labels,
    ui: std::rc::Rc<Ui>,
    selected: usize,
    previous_pointer: Vec2,
    mouse_navigation: bool,
    draft: Preferences,
    draft_audio: audio::Settings,
    binding: Option<usize>,
    save_labels: [String; 6],
    save_dates: [String; 6],
    save_usable: [bool; 6],
    save_previews: [Option<Texture2D>; 6],
    save_selected: usize,
    save_film: save_screen::Film,
    controls_pad: bool,
    controls_page: usize,
    using_pad: bool,
    credits: Vec<String>,
    credit_offset: usize,
    mirror: crate::npc::Puppet,
    mirror_target: RenderTarget,
    material: crate::character::SkinMaterial,
}
impl Menu {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        let specs = texture::read_materials(assets)?;
        let mut layouts = BTreeMap::new();
        let mut credits = Vec::new();
        for name in ["main", "controls", "loadsave", "newgame", "quit", "credits"] {
            let data = assets.read(&format!("ui/{name}.urc"))?;
            let (source, _, _) = encoding_rs::WINDOWS_1252.decode(&data);
            if name == "credits" {
                for line in source.lines() {
                    let t = bsp::tokens(line)?;
                    if t.first().is_some_and(|s| s == "addline") {
                        credits.push(t.get(1).cloned().unwrap_or_default());
                    }
                }
            }
            layouts.insert(
                name.into(),
                main_layout(&data)?,
            );
        }
        // The original scroller starts off-page; paging should begin at its text.
        let first = credits
            .iter()
            .position(|line| !line.trim().is_empty())
            .unwrap_or(credits.len());
        credits.drain(..first);
        ensure!(layouts["credits"].iter().any(|w| w.value("name") == "title1"),
            "Missing original credits text area");
        for name in ["newgame", "loadgame", "settings", "quit", "backtogame"] {
            ensure!(
                layouts["main"]
                    .iter()
                    .any(|w| w.kind == "Button" && w.value("name") == name),
                "Missing original menu button: {name}"
            );
        }
        for name in [
            "BigSaveShot",
            "bigcover_left",
            "bigcover_right",
            "MapName",
            "DateTime",
            "PageNum",
            "PrevButton",
            "NextButton",
            "DeleteButton",
            "LoadingButton",
            "SaveButton",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain((1..=6).flat_map(|i| [format!("LoadSaveShot{i}"), format!("LoadButton{i}")]))
        {
            ensure!(
                layouts["loadsave"].iter().any(|w| w.value("name") == name),
                "Missing original save widget {name}"
            );
        }
        let mut names = vec![
            "ui/control/head_2_video",
            "ui/control/head_2_audio",
            "ui/control/head_2_control",
            "ui/control/head_2_game",
            "ui/buttons/apply",
            "ui/buttons/apply_hover",
            "ui/control/hslider_bar",
            "ui/control/hslider_indicator",
            "ui/control/hslider_prev",
            "ui/control/hslider_next",
            "ui/control/hslider_prev_pressed",
            "ui/control/hslider_next_pressed",
            "ui/control/checkbox_checked",
            "ui/control/checkbox_unchecked",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for w in layouts.values().flatten() {
            names.extend(
                [w.art(), w.value("hovershader"), w.value("checked_shader"), w.value("unchecked_shader")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        let mut textures = BTreeMap::new();
        let mut button_bounds = BTreeMap::new();
        for name in names {
            let key = name.to_ascii_lowercase();
            if textures.contains_key(&key) {
                continue;
            }
            if let Some(candidate) = menu_texture_path(assets, &specs, &key) {
                let additive = specs.get(&key).is_some_and(|s| s.stages.first()
                    .is_some_and(|stage| matches!(stage.blend,
                        crate::materials::Blend::Add | crate::materials::Blend::AlphaAdd)))
                    || name.ends_with("_glow") && name.contains("/button_")
                    || name.ends_with("game_glow")
                    || name.ends_with("settings_glow")
                    || name.ends_with("quit_glow");
                let (texture, bounds) = load_texture_geometry(assets, &candidate, additive)?;
                if let Some(bounds) = bounds {
                    button_bounds.insert(key.clone(), bounds);
                }
                textures.insert(key, texture);
            }
        }
        let mut mirror =
            crate::npc::Puppet::load(assets, "alice", &["idle_stand_rocktoes"], &specs)?;
        mirror.show_attachments(false);
        let list = layouts["controls"].iter().find(|w| w.kind == "FakkBindList")
            .context("Missing original binding-list layout")?;
        let binding_labels = if assets.contains(list.value("filename")) {
            binding_labels(&assets.read(list.value("filename"))?)?
        } else {BTreeMap::new()};
        let mut map_titles = BTreeMap::new();
        if assets.contains("ui/maplist.scr") {
            let data = assets.read("ui/maplist.scr")?;
            let (source, _, _) = encoding_rs::WINDOWS_1252.decode(&data);
            for line in source.lines() {
                let tokens = bsp::tokens(line)?;
                if tokens.len() >= 4 && tokens[0].parse::<u32>().is_ok() {
                    map_titles.insert(tokens[2].to_ascii_lowercase(), tokens[3].clone());
                }
            }
        }
        let mut chapter_labels = crate::chapters::Labels::default();
        if let Some(back) = layouts["newgame"].iter().find(|w| w.kind == "Button"
            && w.value("stuffcommand").contains("popmenu") && !w.value("title").is_empty()) {
            chapter_labels.back = back.value("title").into();
        }
        for (i, name) in ["easy", "medium", "hard", "nightmare"].iter().enumerate() {
            if let Some(w) = layouts["newgame"].iter().find(|w| w.value("name") == *name
                && !w.value("title").is_empty()) {
                chapter_labels.difficulties[i] = w.value("title").into();
            }
        }
        Ok(Self {
            page: None,
            message: String::new(),
            post_game: false,
            frontend: false,
            layouts,
            textures,
            button_bounds,
            binding_labels,
            map_titles,
            chapter_labels,
            ui: Ui::load(assets)?,
            selected: 0,
            previous_pointer: Vec2::splat(-10000.),
            mouse_navigation: false,
            draft: Preferences::default(),
            draft_audio: audio::Settings::default(),
            binding: None,
            save_labels: Default::default(),
            save_dates: Default::default(),
            save_usable: [false; 6],
            save_previews: Default::default(),
            save_selected: 0,
            save_film: save_screen::Film::load(assets)?,
            controls_pad: false,
            controls_page: 0,
            using_pad: false,
            credits,
            credit_offset: 0,
            mirror,
            mirror_target: render_target_ex(
                384,
                480,
                RenderTargetParams {
                    depth: true,
                    ..Default::default()
                },
            ),
            material: crate::character::skin_material()?,
        })
    }
    pub fn open(&mut self, page: Page, preferences: &Preferences, audio: audio::Settings) {
        self.page = Some(page);
        self.selected = 0;
        self.binding = None;
        self.draft = preferences.clone();
        self.draft_audio = audio;
        self.previous_pointer = Vec2::from(mouse_position());
        self.mouse_navigation = false;
        self.message.clear();
        if page == Page::LoadSave {
            self.selected = self.save_selected;
            self.save_film.open(self.save_selected, get_time());
        }
    }
    pub fn close(&mut self) {
        self.page = None;
        self.save_film.stop();
    }
    fn art(&self, canvas: Canvas, name: &str, rect: Rect, alpha: f32) {
        if let Some(t) = self.textures.get(&name.to_ascii_lowercase()) {
            let r = canvas.rect(rect);
            draw_texture_ex(
                t,
                r.x,
                r.y,
                Color::new(1., 1., 1., alpha),
                DrawTextureParams {
                    dest_size: Some(r.size()),
                    ..Default::default()
                },
            );
        }
    }
    fn draw_lettering(&self, canvas: Canvas, widget: &Widget, text: &str, selected: bool) {
        let lettering = Lettering::from_widget(widget);
        self.draw_styled_text(canvas, widget.rect, widget.art(), text, &lettering, selected);
    }
    fn center_button_text(&self, mut text: Rect, button: Rect, art: &str) -> Rect {
        let (top, bottom) = self.button_bounds.get(&art.to_ascii_lowercase())
            .copied().unwrap_or((0., 1.));
        // Normal artwork is the stable reference, even when hover art is drawn.
        let visible_top = button.y + top * button.h;
        let visible_height = (bottom - top) * button.h;
        text.y = visible_top + visible_height * 0.5 - text.h * 0.5;
        text
    }
    fn draw_styled_text(&self, canvas: Canvas, rect: Rect, art: &str, text: &str, lettering: &Lettering, selected: bool) {
        let font = match lettering.font.as_str() {
            "verdana-12" => &self.ui.body, "courier-14" => &self.ui.courier, _ => &self.ui.font,
        };
        let size = font.height() * lettering.scale;
        let r = lettering.text_rect(rect, font.width_spaced(text, size, lettering.spacing), size);
        let r = if lettering.visible_center {self.center_button_text(r, rect, art)} else {r};
        let anchor = vec2(match lettering.justify.0.as_str() {
            "left" => r.x, "right" => r.right(), _ => r.center().x,
        }, match lettering.justify.1.as_str() {
            "top" => r.y, "bottom" => r.bottom(), _ => r.center().y,
        });
        font.draw_transformed(canvas, text, r, size.min(r.h),
            if selected {Color::from_hex(0x831c12)} else {lettering.color}, lettering.spacing,
            crate::ui::TextTransform {visible_center: lettering.visible_center,
                radians: lettering.angle, anchor});
    }
    pub(crate) fn localize_choices(&self, choices: &mut [crate::campaign::LevelChoice], maps: &[String]) {
        for choice in choices {
            if let Some(title) = self.map_titles.get(&maps[choice.map].to_ascii_lowercase()) {
                choice.title = if choice.entry.is_some() {
                    format!("{title} - return")
                } else {title.clone()};
            }
        }
    }
    fn credits_widget(&self) -> &Widget {
        self.layouts["credits"].iter().find(|w| w.value("name") == "title1")
            .expect("validated original credits text area")
    }
    fn credits_geometry(&self) -> (f32, f32, usize) {
        let widget = self.credits_widget();
        let lettering = Lettering::from_widget(widget);
        let font = match lettering.font.as_str() {
            "verdana-12" => &self.ui.body, "courier-14" => &self.ui.courier, _ => &self.ui.font,
        };
        let size = (font.height() * lettering.scale).min(widget.rect.h);
        let spacing = widget.fields.get("fontspacing").and_then(|v| v.get(1))
            .and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(0.);
        let step = (size + spacing).max(1.);
        let rows = (((widget.rect.h - size).max(0.) / step).floor() as usize + 1).min(1024);
        (size, step, rows)
    }
    fn slider_thumb_size(&self, rect: Rect) -> Vec2 {
        self.textures.get("ui/control/hslider_indicator").map_or(vec2(rect.h, rect.h), |t| {
            let scale = (rect.h / t.height()).min(1.);
            vec2((t.width() * scale).min(rect.w), t.height() * scale)
        })
    }
    fn draw_slider(&self, canvas: Canvas, rect: Rect, value: f32) {
        let bar_height = self.textures.get("ui/control/hslider_bar")
            .map_or(rect.h, |t| t.height().min(rect.h));
        self.art(canvas, "ui/control/hslider_bar",
            Rect::new(rect.x, rect.y+(rect.h-bar_height)*0.5, rect.w, bar_height), 1.);
        let size = self.slider_thumb_size(rect);
        self.art(canvas, "ui/control/hslider_indicator",
            Rect::new(rect.x+value.clamp(0.,1.)*(rect.w-size.x), rect.y+(rect.h-size.y)*0.5, size.x, size.y), 1.);
    }
    fn draw_setting(&self, canvas: Canvas, page: Page, row: usize, setting: &SettingView, selected: bool) {
        self.draw_lettering(canvas, &setting.label, setting.label.value("title"), selected);
        let control = &setting.control;
        if let Some(value) = self.slider(page, row) {
            self.draw_slider(canvas, control.rect, value);
        } else if let Some(value) = self.toggle(page, row) {
            let art = control.value(if value {"checked_shader"} else {"unchecked_shader"});
            if let Some(texture) = self.textures.get(art) {
                let scale = (control.rect.h/texture.height()).min(control.rect.w/texture.width()).min(1.);
                self.art(canvas, art, Rect::new(control.rect.x, control.rect.y+(control.rect.h-texture.height()*scale)*0.5,
                    texture.width()*scale, texture.height()*scale), 1.);
            }
        } else if page == Page::Video && row == 0 {
            let (w,h) = SIZES[self.draft.resolution];
            self.draw_lettering(canvas, control, &format!("{w} x {h}"), selected);
            for (art, right) in [("ui/control/hslider_prev", false), ("ui/control/hslider_next", true)] {
                if let Some(texture) = self.textures.get(art) {
                    let scale = (control.rect.h/texture.height()).min(1.);
                    let width = texture.width()*scale;
                    let height = texture.height()*scale;
                    self.art(canvas, art, Rect::new(if right {control.rect.right()-width} else {control.rect.x},
                        control.rect.y+(control.rect.h-height)*0.5, width, height), 1.);
                }
            }
        }
    }
    fn button_named(&self, layout: &str, name: &str, click: Click) -> Button {
        self.layouts[layout]
            .iter()
            .find(|w| w.kind == "Button" && w.value("name") == name)
            .expect("validated original menu button")
            .button(click)
    }
    fn binding_area(&self) -> Rect {
        self.layouts["controls"].iter().find(|w| w.kind == "FakkBindList")
            .expect("validated original binding-list layout").rect
    }
    fn buttons(&self, page: Page) -> Vec<Button> {
        let back = || self.layouts[page.layout()].iter()
            .find(|w| w.kind == "Button" && w.value("stuffcommand").contains("popmenu")
                && !w.value("title").is_empty())
            .map(|w| w.button(Click::Back)).unwrap_or_else(|| Button {
            rect: Rect::new(0., 456., 80., 24.),
            text: String::new(),
            art: "ui/back_button".into(),
            hover: "ui/back_button_hover".into(),
            click: Click::Back,
            lettering: None,
            setting: None,
        });
        let mut buttons = match page {
            Page::Main => {
                let mut buttons = vec![
                    self.button_named("main", "newgame", Click::Page(Page::NewGame)),
                    self.button_named("main", "loadgame", Click::Page(Page::LoadSave)),
                    self.button_named("main", "settings", Click::Page(Page::Settings)),
                    self.button_named("main", "quit", Click::Page(Page::Quit)),
                    self.button_named("main", "backtogame", Click::Action(Action::Resume)),
                ];
                if self.post_game || self.frontend {
                    buttons.pop();
                }
                buttons
            }
            Page::Settings => {
                let mut buttons = vec![back()];
                let pages = [Page::Video, Page::Audio, Page::Controls, Page::Game];
                for w in self.layouts["controls"]
                    .iter()
                    .filter(|w| {
                        w.kind == "Button"
                            && w.value("groupid") == "group_main"
                            && w.value("stuffcommand").contains("activategroup")
                    })
                {
                    let Some(page) = pages.iter().copied().find(|page|
                        w.value("stuffcommand").contains(match page {
                            Page::Video => "group_video", Page::Audio => "group_audio",
                            Page::Controls => "group_control", _ => "group_game",
                        })) else {continue};
                    buttons.push(w.button(Click::Page(page)));
                }
                buttons
            }
            Page::NewGame => {
                let mut buttons = self.layouts["newgame"]
                    .iter()
                    .filter(|w| {
                        w.kind == "Button" && w.value("stuffcommand").contains("seta skill")
                    })
                    .filter_map(|w| {
                        let tokens = w.value("stuffcommand").replace(';', " ");
                        let tokens = tokens.split_whitespace().collect::<Vec<_>>();
                        let d = tokens.windows(3).find(|t| t[0] == "seta" && t[1] == "skill")
                            .and_then(|t| Difficulty::parse(t[2]).ok())?;
                        Some(w.button(if self.frontend {
                        Click::Action(Action::NewGame(d))
                    } else {
                        Click::Page(Page::ConfirmNew(d))
                    }))})
                    .collect::<Vec<_>>();
                buttons.push(back());
                buttons
            }
            Page::Quit => self.layouts["quit"]
                .iter()
                .filter(|w| w.kind == "Button")
                .filter_map(|w| {
                    let command = w.value("stuffcommand");
                    let click = if command.split_whitespace().next() == Some("quit") {
                        Click::Action(Action::Quit)
                    } else if command.contains("pushmenu credits") {
                        Click::Page(Page::Credits)
                    } else if command.contains("popmenu") {Click::Back} else {return None};
                    Some(w.button(click))
                })
                .collect(),
            Page::ConfirmNew(_) | Page::ConfirmLoad(_) | Page::ConfirmSave(_) => {
                let action = match page {
                    Page::ConfirmNew(d) => Action::NewGame(d),
                    Page::ConfirmLoad(s) => Action::Load(s),
                    Page::ConfirmSave(s) => Action::Save(s),
                    _ => unreachable!(),
                };
                vec![
                    Button::text(
                        Rect::new(205., 264., 100., 35.),
                        "Yes",
                        Click::Action(action),
                    ),
                    Button::text(Rect::new(335., 264., 100., 35.), "No", Click::Back),
                ]
            }
            Page::LoadSave => {
                let mut buttons = (0..6)
                    .map(|i| Button {
                        rect: self.slot_rect(i),
                        text: String::new(),
                        art: String::new(),
                        hover: String::new(),
                        click: Click::Slot(i),
                        lettering: None,
                        setting: None,
                    })
                    .collect::<Vec<_>>();
                buttons.push(self.button_named("loadsave", "LoadingButton", Click::LoadSelection));
                if !self.frontend {
                    buttons.push(self.button_named("loadsave", "SaveButton", Click::SaveSelection));
                }
                buttons.push(back());
                buttons
            }
            Page::Controls => {
                let area = self.binding_area();
                // Device switching and pagination are Rust additions inside
                // the original binding-list rectangle.
                let row_height = ((area.h - 100.) / 8.).max(1.);
                let first_row = area.y + 36.;
                let pager_y = area.bottom() - 64.;
                let pager_width = (area.w - 45.) * 0.5;
                let mut buttons = vec![Button {
                    rect: Rect::new(area.x, area.y, area.w, 28.),
                    text: if self.controls_pad {
                        "Controller"
                    } else {
                        "Keyboard / Mouse"
                    }
                    .into(),
                    art: String::new(),
                    hover: String::new(),
                    click: Click::Device,
                    lettering: None,
                    setting: None,
                }];
                let start = self.controls_page * 8;
                for (row, (label, _)) in BINDINGS.iter().enumerate().skip(start).take(8) {
                    buttons.push(Button {
                        rect: Rect::new(area.x, first_row + (row - start) as f32 * row_height, area.w, row_height),
                        text: (*label).into(),
                        art: String::new(),
                        hover: String::new(),
                        click: Click::Row(row),
                        lettering: None,
                        setting: None,
                    });
                }
                if self.controls_pad && self.controls_page == BINDINGS.len().div_ceil(8) {
                    for (i, label) in ["Look Sensitivity", "Invert Look", "Stick Deadzone"]
                        .into_iter()
                        .enumerate()
                    {
                        buttons.push(Button {
                            rect: Rect::new(area.x, first_row + i as f32 * row_height * 2., area.w, row_height * 2.),
                            text: label.into(),
                            art: String::new(),
                            hover: String::new(),
                            click: Click::Row(BINDINGS.len() + i),
                            lettering: None,
                            setting: None,
                        });
                    }
                }
                for (x, label, delta) in [(area.x, "Previous", -1), (area.right()-pager_width, "Next", 1)] {
                    buttons.push(Button::text(
                        Rect::new(x, pager_y, pager_width, 32.),
                        label,
                        Click::ControlsPage(delta),
                    ));
                }
                buttons.push(Button::text(
                    Rect::new(area.center().x - area.w * 0.25, area.bottom()-32., area.w * 0.5, 32.),
                    "Reset",
                    Click::Reset,
                ));
                let group = match page {
                    Page::Video => "group_video", Page::Audio => "group_audio", _ => "group_game",
                };
                for (click, command) in [(Click::Apply, "ui_setcvars"), (Click::Cancel, "ui_restorecvars")] {
                    if let Some(w) = self.layouts["controls"].iter().find(|w| w.kind == "Button"
                        && w.value("groupid") == group && w.value("stuffcommand").contains(command)) {
                        buttons.push(w.button(click));
                    }
                }
                buttons
            }
            Page::Credits => vec![
                back(),
                Button::text(
                    Rect::new(320., 410., 100., 35.),
                    "Previous",
                    Click::CreditScroll(-1),
                ),
                Button::text(
                    Rect::new(430., 410., 100., 35.),
                    "Next",
                    Click::CreditScroll(1),
                ),
            ],
            _ => {
                let mut buttons = setting_buttons(&self.layouts["controls"], page);
                if page == Page::Audio {
                    // Mute is a Rust feature without an original widget.
                    buttons.push(Button { rect: Rect::new(68., 298., 220., 24.),
                        text: "Mute".into(), art: String::new(), hover: String::new(),
                        click: Click::Row(2), lettering: None, setting: None });
                }
                let group = match page {
                    Page::Video => "group_video", Page::Audio => "group_audio", _ => "group_game",
                };
                for (click, command) in [(Click::Apply, "ui_setcvars"), (Click::Cancel, "ui_restorecvars")] {
                    if let Some(w) = self.layouts["controls"].iter().find(|w| w.kind == "Button"
                        && w.value("groupid") == group && w.value("stuffcommand").contains(command)) {
                        buttons.push(w.button(click));
                    }
                }
                buttons
            }
        };
        // Adapted dialogs and paging have no original layout. Reuse the original
        // generic button lettering, while keeping their own rectangles and actions.
        if matches!(page, Page::ConfirmNew(_) | Page::ConfirmLoad(_) | Page::ConfirmSave(_) | Page::Credits) {
            if let Some(template) = self.layouts["controls"].iter().find(|w|
                w.kind == "Button" && w.value("stuffcommand").contains("ui_setcvars")) {
                for b in &mut buttons {
                    if b.lettering.is_none() {
                        b.lettering = Some(Lettering::from_widget(template));
                        b.art = template.art().into();
                        b.hover = template.value("hovershader").into();
                    }
                }
            }
            for b in &mut buttons {
                let shader = match b.text.as_str() {"Yes" => "ui/quit/yes_glow", "No" => "ui/quit/no_glow", _ => continue};
                if let Some(w) = self.layouts["quit"].iter().find(|w| w.value("hovershader") == shader
                    && !w.value("title").is_empty()) {
                    b.text = w.value("title").into();
                }
            }
        }
        if page == Page::Settings || page.settings() {
            let widgets = &self.layouts["controls"];
            let group = match page {
                Page::Video => "group_video", Page::Audio => "group_audio",
                Page::Game => "group_game", Page::Controls => "group_control",
                _ => "group_main",
            };
            for b in &mut buttons {
                let command = match b.click {
                    Click::Apply => Some("ui_setcvars"),
                    Click::Cancel => Some("ui_restorecvars"),
                    Click::Reset => Some("exec default.cfg"),
                    Click::Back => Some("popmenu"),
                    _ => None,
                };
                if let Some(command) = command {
                    // The control page has no original Apply/Cancel: reuse their
                    // archive labels from another settings group, but keep Rust actions.
                    let title = widgets.iter().find(|w| w.kind == "Button"
                        && w.value("groupid") == group && w.value("stuffcommand").contains(command))
                        .or_else(|| widgets.iter().find(|w| w.kind == "Button"
                            && w.value("stuffcommand").contains(command)));
                    if let Some(w) = title {
                        let old_rect = b.rect;
                        let click = b.click;
                        *b = w.button(click);
                        b.lettering = Some(Lettering::from_widget(w));
                        // Controls has added Apply/Cancel actions at the original
                        // Reset location. Keep Reset on its separate adapted row.
                        if page == Page::Controls && matches!(click, Click::Apply | Click::Cancel | Click::Reset) {
                            b.rect = old_rect;
                        }
                    }
                }
                if let Click::Row(row) = b.click {
                    if page == Page::Controls && row < BINDINGS.len() {
                        if let Some(label) = binding_command(BINDINGS[row].0)
                            .and_then(|c| self.binding_labels.get(c)) {
                            b.text = label.clone();
                        }
                    }
                }
            }
        }
        buttons
    }
    fn adjust(&mut self, page: Page, row: usize, delta: f32) {
        match (page, row) {
            (Page::Video, 0) => {
                self.draft.resolution = (self.draft.resolution as i32
                    + if delta < 0. { -1 } else { 1 })
                .rem_euclid(SIZES.len() as i32) as usize
            }
            (Page::Video, 1) => self.draft.fullscreen = !self.draft.fullscreen,
            (Page::Audio, 0) => {
                self.draft_audio.music = (self.draft_audio.music + delta * 0.05).clamp(0., 1.)
            }
            (Page::Audio, 1) => {
                self.draft_audio.effects = (self.draft_audio.effects + delta * 0.05).clamp(0., 1.)
            }
            (Page::Audio, 2) => self.draft_audio.muted = !self.draft_audio.muted,
            (Page::Game, 0) => {
                self.draft.sensitivity = (self.draft.sensitivity + delta * 0.1).clamp(0.2, 3.)
            }
            (Page::Game, 1) => self.draft.invert_mouse = !self.draft.invert_mouse,
            (Page::Game, 2) => {
                self.draft.camera_distance =
                    (self.draft.camera_distance + delta * 10.).clamp(80., 220.)
            }
            (Page::Game, 3) => self.draft.always_run = !self.draft.always_run,
            (Page::Game, 4) => self.draft.subtitles = !self.draft.subtitles,
            (Page::Controls, row) if row < BINDINGS.len() => self.binding = Some(row),
            (Page::Controls, row) if row == BINDINGS.len() => {
                self.draft.pad_sensitivity =
                    (self.draft.pad_sensitivity + delta * 0.1).clamp(0.2, 3.)
            }
            (Page::Controls, row) if row == BINDINGS.len() + 1 => {
                self.draft.invert_pad = !self.draft.invert_pad
            }
            (Page::Controls, _) => {
                self.draft.pad_deadzone = (self.draft.pad_deadzone + delta * 0.02).clamp(0.1, 0.4)
            }
            _ => (),
        }
    }
    fn toggle(&self, page: Page, row: usize) -> Option<bool> {
        match (page, row) {
            (Page::Audio, 2) => Some(self.draft_audio.muted),
            (Page::Video, 1) => Some(self.draft.fullscreen),
            (Page::Game, 1) => Some(self.draft.invert_mouse),
            (Page::Game, 3) => Some(self.draft.always_run),
            (Page::Game, 4) => Some(self.draft.subtitles),
            (Page::Controls, row) if row == BINDINGS.len() + 1 => Some(self.draft.invert_pad),
            _ => None,
        }
    }
    /// Original main artwork behind loading and chapter selection.
    pub fn backdrop(&mut self) {
        self.draw(Page::Main, &[]);
    }
    fn slider(&self, page: Page, row: usize) -> Option<f32> {
        match (page, row) {
            (Page::Audio, 0) => Some(self.draft_audio.music),
            (Page::Audio, 1) => Some(self.draft_audio.effects),
            (Page::Game, 0) => Some((self.draft.sensitivity - 0.2) / 2.8),
            (Page::Game, 2) => Some(((self.draft.camera_distance - 80.) / 140.).clamp(0., 1.)),
            (Page::Controls, row) if row == BINDINGS.len() => {
                Some((self.draft.pad_sensitivity - 0.2) / 2.8)
            }
            (Page::Controls, row) if row == BINDINGS.len() + 2 => {
                Some((self.draft.pad_deadzone - 0.1) / 0.3)
            }
            _ => None,
        }
    }
    fn drag_slider(&mut self, page: Page, row: usize, value: f32) {
        let v = value.clamp(0., 1.);
        match (page, row) {
            (Page::Audio, 0) => self.draft_audio.music = v,
            (Page::Audio, 1) => self.draft_audio.effects = v,
            (Page::Game, 0) => self.draft.sensitivity = 0.2 + v * 2.8,
            (Page::Game, 2) => self.draft.camera_distance = 80. + v * 140.,
            (Page::Controls, row) if row == BINDINGS.len() => {
                self.draft.pad_sensitivity = 0.2 + v * 2.8
            }
            (Page::Controls, row) if row == BINDINGS.len() + 2 => {
                self.draft.pad_deadzone = 0.1 + v * 0.3
            }
            _ => (),
        }
    }
    fn save_preview(&self, canvas: Canvas, index: usize, rect: Rect) {
        let r = canvas.rect(rect);
        if let Some(texture) = &self.save_previews[index] {
            // Crop to the frame without stretching a widescreen game view.
            let size = vec2(texture.width(), texture.height());
            let scale = (r.w / size.x).max(r.h / size.y);
            let source_size = r.size() / scale;
            let source = (size - source_size) * 0.5;
            draw_texture_ex(
                texture,
                r.x,
                r.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(r.size()),
                    source: Some(Rect::new(source.x, source.y, source_size.x, source_size.y)),
                    ..Default::default()
                },
            );
        } else {
            draw_rectangle(r.x, r.y, r.w, r.h, BLACK);
            self.ui.font.draw(
                canvas,
                if self.save_usable[index] {
                    "No preview"
                } else {
                    &self.save_labels[index]
                },
                rect,
                17.,
                crate::ui::PAPER_TEXT,
            );
        }
    }
    fn refresh_saves(&mut self, store: &Store) {
        for (index, slot) in Slot::ALL.into_iter().enumerate() {
            let loaded = store.read(slot);
            self.save_usable[index] = loaded.is_ok();
            self.save_previews[index] = loaded
                .as_ref()
                .ok()
                .and_then(|s| s.preview.as_ref())
                .and_then(|p| p.texture());
            self.save_dates[index] = loaded
                .as_ref()
                .map(|s| {
                    format!(
                        "{}{}",
                        save_screen::timestamp(s.saved_at),
                        if s.backup { " (backup)" } else { "" }
                    )
                })
                .unwrap_or_else(|_| slot.title().into());
            self.save_labels[index] = loaded
                .map(|s| {
                    format!(
                        "{}{}",
                        s.game
                            .level()
                            .map_or("Unknown", |l| self.map_titles.get(&l.map.to_ascii_lowercase())
                                .map(String::as_str).unwrap_or_else(|| crate::campaign::level_title(&l.map))),
                        if s.backup { " (backup)" } else { "" }
                    )
                })
                .unwrap_or_else(|_| {
                    if store.exists(slot) {
                        "Unreadable save"
                    } else {
                        "Empty slot"
                    }
                    .into()
                });
        }
    }
    pub async fn check_save_previews(&mut self, store: &Store) -> Result<()> {
        self.refresh_saves(store);
        ensure!(
            self.save_previews[0].is_some() && self.save_previews[1].is_some(),
            "Saved previews missing from menu"
        );
        ensure!(
            self.save_previews[2].is_none() && self.save_previews[3].is_none(),
            "Legacy/damaged image did not fall back"
        );
        for index in [0, 1, 2] {
            self.save_selected = index;
            self.selected = index;
            self.save_film.open(index, get_time() - 1.);
            let buttons = self.buttons(Page::LoadSave);
            self.draw(Page::LoadSave, &buttons);
            crate::viewer::save_capture(std::path::Path::new(&format!(
                "private/save-preview-menu-{index}.png"
            )))?;
            next_frame().await;
        }
        self.save_selected = 1;
        self.selected = 1;
        self.save_film.open(0, get_time() - 1.);
        self.save_film.select(1, get_time() - 0.23);
        self.draw(Page::LoadSave, &self.buttons(Page::LoadSave));
        crate::viewer::save_capture(std::path::Path::new("private/save-menu-shutters.png"))?;
        next_frame().await;
        self.save_film.open(1, get_time() - 1.);
        let confirm = Page::ConfirmSave(Slot::Two);
        self.draw(confirm, &self.buttons(confirm));
        crate::viewer::save_capture(std::path::Path::new("private/save-menu-confirm.png"))?;
        next_frame().await;
        Ok(())
    }
    fn controls_page(&mut self, delta: i32) {
        let count = BINDINGS.len().div_ceil(8) + usize::from(self.controls_pad);
        self.controls_page = (self.controls_page as i32 + delta).rem_euclid(count as i32) as usize;
        self.selected = 0;
    }
    fn back(&mut self, page: Page, prefs: &Preferences, audio: audio::Settings) -> Option<Action> {
        if self.post_game || self.frontend {
            if page == Page::Main {
                return None;
            }
            if page == Page::Credits {
                self.open(Page::Main, prefs, audio);
                return None;
            }
        }
        if let Some(parent) = page.parent() {
            self.open(parent, prefs, audio);
            None
        } else {
            self.close();
            Some(Action::Resume)
        }
    }
    fn draw_mirror(&mut self) {
        let eye = vec3(125., 0., 38.);
        set_camera(&Camera3D {
            position: eye,
            target: vec3(0., 0., 33.),
            up: Vec3::Z,
            fovy: 40f32.to_radians(),
            z_near: 1.,
            render_target: Some(self.mirror_target.clone()),
            aspect: Some(384. / 480.),
            ..Default::default()
        });
        clear_background(Color::new(0., 0., 0., 0.));
        self.material
            .atmosphere(&crate::environment::Atmosphere::default(), eye);
        self.material.bind();
        self.mirror.draw_filtered(
            "idle_stand_rocktoes",
            get_time() as f32,
            true,
            crate::skeletal::Transform {
                translation: Vec3::ZERO,
                rotation: Quat::from_rotation_z(-0.2),
            },
            1.,
            false,
            &["material15", "material16", "material17"],
        );
        gl_use_default_material();
        set_default_camera();
    }
    fn draw(&mut self, page: Page, buttons: &[Button]) {
        self.draw_into(page, buttons, None);
    }
    fn draw_into(&mut self, page: Page, buttons: &[Button], target: Option<RenderTarget>) {
        let canvas = Canvas::new(screen_width(), screen_height());
        let hit = buttons.iter().position(|b| b.rect.contains(canvas.pointer(Vec2::from(mouse_position()))));
        if page.layout() == "controls" {
            self.draw_mirror();
        }
        if let Some(target) = &target {
            set_camera(&Camera2D {
                render_target: Some(target.clone()),
                ..Camera2D::from_display_rect(Rect::new(0., 0., screen_width(), screen_height()))
            });
        }
        clear_background(BLACK);
        if page.layout() == "loadsave" {
            self.draw_save_underlay(canvas, get_time());
        }
        for widget in &self.layouts[page.layout()] {
            if widget.kind == "Alice3D" {
                let r = canvas.rect(Rect::new(356., 105., 231., 289.));
                draw_texture_ex(
                    &self.mirror_target.texture,
                    r.x,
                    r.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(r.size()),
                        flip_y: true,
                        ..Default::default()
                    },
                );
            }
            if widget.kind != "Label" {
                continue;
            }
            if page.layout() == "loadsave"
                && (widget.value("name").starts_with("LoadSaveShot")
                    || [
                        "BigSaveShot",
                        "sepia1",
                        "bigcover_black",
                        "bigcover_left",
                        "bigcover_right",
                    ]
                    .contains(&widget.value("name")))
            {
                continue;
            }
            // These original widgets are activated only on their error path.
            if ["DiskFull", "MsgBackground"].contains(&widget.value("name")) {
                continue;
            }
            let group = widget.value("groupid");
            if !(group.is_empty() || page == Page::Settings && group == "group_main"
                || page == Page::Credits && group == "group_page1") {
                continue;
            }
            let hovered_title = if page == Page::Settings && widget.value("name") == "TitleBar" {
                buttons.iter().enumerate().find(|(i,b)|
                    highlighted(self.selected, self.mouse_navigation, hit, *i) && matches!(b.click, Click::Page(_)))
                    .and_then(|(_,b)| self.layouts["controls"].iter().find(|w| w.kind == "Button" && w.rect == b.rect))
                    .and_then(|w| w.value("mouseentercmd").split_whitespace().collect::<Vec<_>>()
                        .windows(2).find(|p| p[0] == "shader").map(|p| p[1].trim_end_matches("\\n").to_owned()))
            } else { None };
            let art = hovered_title.as_deref().unwrap_or_else(|| widget.art());
            let alpha = if art.contains("/glow")
                || art.contains("wick")
                || page == Page::Main && main_background_glow(art)
            {
                0.15 + (get_time() as f32 * 3.).sin().abs() * 0.15
            } else {
                1.
            };
            self.art(canvas, art, widget.rect, alpha);
            if !widget.value("title").is_empty()
                && !(page.layout() == "loadsave" && ["MapName", "DateTime", "PageNum"].contains(&widget.value("name"))) {
                self.draw_lettering(canvas, widget, widget.value("title"), false);
            }
        }
        if page.layout() == "loadsave" {
            let focus = buttons.iter().enumerate().find(|(i, _)|
                highlighted(self.selected, self.mouse_navigation, hit, *i)).map(|(_, b)| b.click);
            self.draw_save_details(canvas, page, focus);
        }
        if matches!(
            page,
            Page::ConfirmNew(_) | Page::ConfirmLoad(_) | Page::ConfirmSave(_)
        ) {
            let r = canvas.rect(Rect::new(100., 165., 440., 153.));
            self.ui.panel(r);
            let title = match page {
                Page::ConfirmNew(d) => format!("Start a new {} game?", d.name()),
                Page::ConfirmLoad(_) => "Replace this session with the saved game?".into(),
                Page::ConfirmSave(s) => format!("Replace {} with this game?", s.title()),
                _ => unreachable!(),
            };
            self.ui.font.draw(
                canvas,
                &title,
                Rect::new(112., 182., 416., 36.),
                24.,
                crate::ui::INK,
            );
            self.ui.font.draw(
                canvas,
                if matches!(page, Page::ConfirmNew(_)) {
                    "Your current game will be saved automatically."
                } else {
                    "Choose No to return."
                },
                Rect::new(114., 222., 412., 25.),
                18.,
                crate::ui::INK,
            );
        }
        if page == Page::Credits {
            let widget = self.credits_widget();
            let (size, step, rows) = self.credits_geometry();
            for (i, line) in self
                .credits
                .iter()
                .skip(self.credit_offset)
                .take(rows)
                .enumerate()
            {
                let mut row = widget.clone();
                row.rect = Rect::new(widget.rect.x, widget.rect.y + i as f32 * step, widget.rect.w, size);
                self.draw_lettering(canvas, &row, line, false);
            }
        }
        for (i, b) in buttons.iter().enumerate() {
            let selected = highlighted(self.selected, self.mouse_navigation, hit, i);
            self.art(
                canvas,
                if selected && !b.hover.is_empty() {
                    &b.hover
                } else {
                    &b.art
                },
                b.rect,
                1.,
            );
            let dark = Color::from_hex(if selected { 0x831c12 } else { 0x330b0b });
            if let (Click::Row(row), Some(setting)) = (b.click, &b.setting) {
                self.draw_setting(canvas, page, row, setting, selected);
                continue;
            }
            if !b.text.is_empty() {
                if let Some(lettering) = &b.lettering {
                    self.draw_styled_text(canvas, b.rect, &b.art, &b.text, lettering, selected);
                    continue;
                }
                let r = if page == Page::Video && matches!(b.click, Click::Row(_)) {
                    for (name, x) in [("prev", b.rect.x), ("next", b.rect.right() - 18.)] {
                        self.art(
                            canvas,
                            &format!(
                                "ui/control/hslider_{name}{}",
                                if selected { "_pressed" } else { "" }
                            ),
                            Rect::new(x, b.rect.y + 5., 16., 16.),
                            1.,
                        );
                    }
                    Rect::new(b.rect.x + 24., b.rect.y, b.rect.w - 48., 26.)
                } else if matches!(b.click, Click::Row(_)) {
                    Rect::new(b.rect.x, b.rect.y, b.rect.w, 26.)
                } else if b.art == "ui/buttons/apply" {
                    Rect::new(b.rect.x + 24., b.rect.y, b.rect.w - 30., b.rect.h)
                } else {
                    b.rect
                };
                if let Click::Row(row) = b.click {
                    if page == Page::Controls && row < BINDINGS.len() {
                        let mut cell = self.layouts["controls"].iter()
                            .find(|w| w.kind == "FakkBindList").expect("validated binding list").clone();
                        cell.fields.entry("fontjustify".into()).or_insert_with(|| vec!["left".into(), "center".into()]);
                        let key_width = r.w / 3.;
                        cell.rect = Rect::new(r.x, r.y, r.w-key_width, r.h);
                        self.draw_lettering(canvas, &cell, &b.text, selected);
                        cell.rect = Rect::new(r.right()-key_width, r.y, key_width, r.h);
                        self.draw_lettering(canvas, &cell, &if self.controls_pad {
                            crate::input::pad_name(&self.draft.pad_bindings[row])
                        } else {self.draft.bindings[row].clone()}, selected);
                    } else if page != Page::Video {
                        let width = if self.toggle(page, row).is_some() {
                            r.w - 38.
                        } else {
                            r.w
                        };
                        self.ui.font.draw_left(
                            canvas,
                            &b.text,
                            Rect::new(r.x, r.y, width, r.h),
                            24.,
                            dark,
                        );
                    } else {
                        self.ui.font.draw(canvas, &b.text, r, 24., dark);
                    }
                } else {
                    let r = self.center_button_text(r, b.rect, &b.art);
                    self.ui.font.draw_button(canvas, &b.text, r, 24., dark);
                }
            }
            if let Click::Row(row) = b.click {
                if let Some(value) = self.toggle(page, row) {
                    self.art(
                        canvas,
                        if value {
                            "ui/control/checkbox_checked"
                        } else {
                            "ui/control/checkbox_unchecked"
                        },
                        Rect::new(260., b.rect.y + 5., 16., 16.),
                        1.,
                    );
                }
                if let Some(value) = self.slider(page, row) {
                    if let Some(rect) = b.slider_rect() {
                        self.draw_slider(canvas, rect, value);
                    }
                }
            }
        }
        if page == Page::Settings {
            self.ui.font.draw(
                canvas,
                "1 Video   2 Audio   3 Controls   4 Game",
                Rect::new(85., 441., 455., 24.),
                20.,
                WHITE,
            );
        }
        if page.settings() {
            self.ui.font.draw(
                canvas,
                &crate::input::pad_text(if self.binding.is_some() {
                    if self.controls_pad {
                        "Press a button   Start / Esc cancel   Delete clear"
                    } else {
                        "Press a key, mouse button or wheel   Esc cancels"
                    }
                } else if self.using_pad {
                    "D-pad / stick: browse   A: select   B: back"
                } else {
                    "Arrows adjust   Enter select   Esc cancel"
                }),
                Rect::new(80., 442., 478., 24.),
                19.,
                WHITE,
            );
            if page == Page::Controls {
                let area = self.binding_area();
                self.ui.font.draw(
                    canvas,
                    &format!(
                        "{} / {}",
                        self.controls_page + 1,
                        BINDINGS.len().div_ceil(8) + usize::from(self.controls_pad)
                    ),
                    Rect::new(area.center().x-22.5, area.bottom()-64., 45., 32.),
                    18.,
                    BLACK,
                );
            }
        }
        if page == Page::Main && !buttons.is_empty() {
            self.ui.font.draw(
                canvas,
                &crate::input::pad_text(if self.frontend || self.post_game {
                    "Tab: chapters"
                } else if self.using_pad {
                    "Tab: chapters   Start / B: back to game"
                } else {
                    "Tab: chapters   Esc: back to game"
                }),
                Rect::new(285., 451., 345., 25.),
                18.,
                WHITE,
            );
        }
        if !self.message.is_empty() {
            let r = canvas.rect(Rect::new(86., 10., 470., 48.));
            self.ui.panel(r);
            self.ui.font.draw(
                canvas,
                &self.message,
                Rect::new(102., 14., 438., 40.),
                22.,
                crate::ui::INK,
            );
        }
        if target.is_some() {
            unsafe {
                get_internal_gl().flush();
            }
            set_default_camera();
        }
    }
    /// The world loop does not advance while this modal loop is running.
    pub async fn run(
        &mut self,
        prefs: &mut Preferences,
        audio: &mut audio::Audio,
        store: &Store,
        listener: Vec3,
        yaw: f32,
        input: &mut crate::input::Input,
    ) -> Action {
        show_mouse(false);
        self.refresh_saves(store);
        loop {
            let page = self.page.unwrap_or(Page::Main);
            let mut action = None;
            let focused = crate::look::window_focused();
            input.update(prefs, focused);
            self.using_pad = input.using_pad;
            self.save_film.update_audio(audio.settings, focused);
            let previous_selected = self.selected;
            let buttons = self.buttons(page);
            self.selected = self.selected.min(buttons.len().saturating_sub(1));
            audio.update(0., listener, yaw, true, true);
            let canvas = Canvas::new(screen_width(), screen_height());
            let mouse = Vec2::from(mouse_position());
            let hit = buttons
                .iter()
                .position(|b| b.rect.contains(canvas.pointer(mouse)));
            if focused {
                if let Some(row) = self.binding {
                    if is_key_pressed(KeyCode::Escape) || input.pad_pressed("Start") {
                        self.binding = None;
                        self.message.clear();
                    } else if let Some(name) = if self.controls_pad {
                        if is_key_pressed(KeyCode::Delete) {
                            Some("None".into())
                        } else {
                            input.capture_pad().map(str::to_owned)
                        }
                    } else {
                        crate::input::capture_binding()
                    } {
                        if self.draft.bind(row, &name, self.controls_pad) {
                            self.binding = None;
                            self.message.clear();
                            input.suppress();
                        } else {
                            self.message = "That control is reserved. Choose another.".into();
                        }
                    }
                } else {
                    if self.using_pad {
                        self.mouse_navigation = false;
                    }
                    if mouse != self.previous_pointer {
                        self.mouse_navigation = true;
                        if let Some(i) = hit {
                            self.selected = i;
                        }
                    }
                    if is_mouse_button_down(MouseButton::Left) {
                        self.mouse_navigation = true;
                        if let Some(i) = hit {
                            if let Click::Row(row) = buttons[i].click {
                                let p = canvas.pointer(mouse);
                                if self.slider(page, row).is_some() {
                                    if let Some(rect) = buttons[i].slider_rect().filter(|r| r.contains(p)) {
                                        let thumb = self.slider_thumb_size(rect);
                                        self.drag_slider(page, row, (p.x - rect.x - thumb.x*0.5) / (rect.w-thumb.x).max(1.));
                                    }
                                }
                            }
                        }
                    }
                    if input.ui(KeyCode::Down) {
                        self.mouse_navigation = false;
                        self.selected = (self.selected + 1) % buttons.len();
                    }
                    if input.ui(KeyCode::Up) {
                        self.mouse_navigation = false;
                        self.selected = (self.selected + buttons.len() - 1) % buttons.len();
                    }
                    let click = if page == Page::Main && input.ui(KeyCode::Tab) {
                        Some(Click::Action(Action::Chapters))
                    } else if input.ui(KeyCode::Escape) {
                        Some(Click::Back)
                    } else if page == Page::Quit && is_key_pressed(KeyCode::Y) {
                        Some(Click::Action(Action::Quit))
                    } else if page == Page::Quit && is_key_pressed(KeyCode::N) {
                        Some(Click::Back)
                    } else if page == Page::Settings && is_key_pressed(KeyCode::Key1) {
                        Some(Click::Page(Page::Video))
                    } else if page == Page::Settings && is_key_pressed(KeyCode::Key2) {
                        Some(Click::Page(Page::Audio))
                    } else if page == Page::Settings && is_key_pressed(KeyCode::Key3) {
                        Some(Click::Page(Page::Controls))
                    } else if page == Page::Settings && is_key_pressed(KeyCode::Key4) {
                        Some(Click::Page(Page::Game))
                    } else if is_mouse_button_pressed(MouseButton::Left) {
                        hit.map(|i| buttons[i].click)
                    } else if input.ui(KeyCode::Enter) {
                        self.mouse_navigation = false;
                        Some(buttons[self.selected].click)
                    } else if page == Page::Controls
                        && (input.pad_pressed("LB")
                            || is_key_pressed(KeyCode::PageUp)
                            || mouse_wheel().1 > 0.)
                    {
                        Some(Click::ControlsPage(-1))
                    } else if page == Page::Controls
                        && (input.pad_pressed("RB")
                            || is_key_pressed(KeyCode::PageDown)
                            || mouse_wheel().1 < 0.)
                    {
                        Some(Click::ControlsPage(1))
                    } else {
                        None
                    };
                    if input.ui(KeyCode::Left) || input.ui(KeyCode::Right) {
                        self.mouse_navigation = false;
                        if let Click::Row(row) = buttons[self.selected].click {
                            self.adjust(page, row, if input.ui(KeyCode::Left) { -1. } else { 1. });
                        }
                    }
                    if page == Page::LoadSave && self.selected != previous_selected {
                        self.save_film.sound(audio, 1);
                    }
                    if let Some(click) = click {
                        if page.layout() == "loadsave" && !matches!(click, Click::Slot(_)) {
                            self.save_film.sound(audio, 2);
                        }
                        match click {
                            Click::Back | Click::Cancel => {
                                action = self.back(page, prefs, audio.settings)
                            }
                            Click::Page(p) => self.open(p, prefs, audio.settings),
                            Click::Action(a) => {
                                if a == Action::Chapters {
                                    // Keep the main menu underneath the modal chapter chooser.
                                } else if matches!(a, Action::Save(_) | Action::Load(_)) {
                                    self.open(Page::LoadSave, prefs, audio.settings);
                                } else {
                                    self.close();
                                }
                                action = Some(a);
                            }
                            Click::Row(row) => {
                                if !(is_mouse_button_pressed(MouseButton::Left)
                                    && self.slider(page, row).is_some()
                                    && buttons[hit.unwrap_or(self.selected)].slider_rect()
                                        .is_some_and(|r| r.contains(canvas.pointer(mouse))))
                                {
                                    let previous = page == Page::Video
                                        && row == 0
                                        && is_mouse_button_pressed(MouseButton::Left)
                                        && hit.is_some_and(|i| {
                                            canvas.pointer(mouse).x < buttons[i].setting.as_ref()
                                                .map_or(buttons[i].rect.center().x, |s| s.control.rect.center().x)
                                        });
                                    self.adjust(page, row, if previous { -1. } else { 1. });
                                }
                            }
                            Click::Reset => {
                                let defaults = Preferences::default();
                                if self.controls_pad {
                                    self.draft.pad_bindings = defaults.pad_bindings;
                                    self.draft.pad_sensitivity = defaults.pad_sensitivity;
                                    self.draft.pad_deadzone = defaults.pad_deadzone;
                                    self.draft.invert_pad = false;
                                } else {
                                    self.draft.bindings = defaults.bindings;
                                }
                            }
                            Click::ControlsPage(delta) => self.controls_page(delta),
                            Click::Device => {
                                self.controls_pad = !self.controls_pad;
                                self.controls_page = 0;
                                self.selected = 0;
                            }
                            Click::Slot(i) => {
                                self.save_selected = i;
                                self.save_film.select(i, get_time());
                                self.save_film.sound(audio, 0);
                                self.message.clear();
                            }
                            Click::LoadSelection => {
                                if self.save_usable[self.save_selected] {
                                    let slot = Slot::ALL[self.save_selected];
                                    if self.frontend {
                                        action = Some(Action::Load(slot));
                                    } else {
                                        self.open(Page::ConfirmLoad(slot), prefs, audio.settings);
                                    }
                                } else {
                                    self.message =
                                        if self.frontend { "This slot has no usable save." } else { "This slot has no usable save. Choose Save to create one." }
                                            .into();
                                }
                            }
                            Click::SaveSelection => {
                                if self.post_game {
                                    self.message =
                                        "Campaign complete. Load a checkpoint or start a new game."
                                            .into();
                                    continue;
                                }
                                let slot = Slot::ALL[self.save_selected];
                                if slot == Slot::Auto {
                                    self.message = "Choose a manual slot or the quick save.".into();
                                } else if store.exists(slot) {
                                    self.open(Page::ConfirmSave(slot), prefs, audio.settings);
                                } else {
                                    action = Some(Action::Save(slot));
                                }
                            }
                            Click::CreditScroll(delta) => {
                                let rows = self.credits_geometry().2;
                                self.credit_offset = (self.credit_offset as i32 + delta * rows.saturating_sub(1).max(1) as i32)
                                    .clamp(0, self.credits.len().saturating_sub(rows) as i32)
                                    as usize
                            }
                            Click::Apply => {
                                let display_changed = prefs.resolution != self.draft.resolution
                                    || prefs.fullscreen != self.draft.fullscreen;
                                match self.draft.save().and_then(|_| self.draft_audio.save()) {
                                    Ok(()) => {
                                        *prefs = self.draft.clone();
                                        audio.settings = self.draft_audio;
                                        if display_changed {
                                            prefs.display();
                                        }
                                        self.open(Page::Settings, prefs, audio.settings);
                                        self.message = "Settings saved".into();
                                    }
                                    Err(e) => {
                                        self.message = format!("Could not save settings: {e}")
                                    }
                                }
                            }
                        }
                    }
                }
            }
            self.previous_pointer = mouse;
            if is_quit_requested() {
                self.close();
                action = Some(Action::Quit);
            }
            // Consume the activating click/key before handing control back to gameplay.
            if let Some(action) = action {
                input.suppress();
                show_mouse(true);
                next_frame().await;
                return action;
            }
            let page = self.page.unwrap_or(Page::Main);
            let buttons = self.buttons(page);
            self.selected = self.selected.min(buttons.len().saturating_sub(1));
            self.draw(page, &buttons);
            if focused {
                self.ui.cursor();
            }
            if focused && is_key_pressed(KeyCode::F12) {
                let path = std::path::PathBuf::from(format!(
                    "private/screenshots/menu-{}.png",
                    (get_time() * 1000.) as u64
                ));
                if let Err(e) = crate::viewer::save_capture(&path) {
                    self.message = format!("Capture failed: {e}");
                }
            }
            next_frame().await;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires local game data and a native graphics window"]
    fn settings_render_with_local_data() {
        let data = std::env::var("LOOKING_GLASS_DATA")
            .unwrap_or_else(|_| "private/game-data/base".into());
        let mut assets = Assets::open(std::path::Path::new(&data)).unwrap();
        let selected_page = std::env::var("LOOKING_GLASS_MENU_RENDER_PAGE").ok();
        macroquad::Window::from_config(crate::viewer::config(), async move {
            let mut menu = Menu::load(&mut assets).unwrap();
            let target = render_target(screen_width() as u32, screen_height() as u32);
            for (page, name) in [(Page::Settings, "overview"), (Page::Video, "video"),
                (Page::Audio, "audio"), (Page::Controls, "controls"), (Page::Game, "game")] {
                if selected_page.as_deref().is_some_and(|s| s != name) {
                    continue;
                }
                let buttons = menu.buttons(page);
                assert!(buttons.iter().any(|b| matches!(b.click, Click::Back | Click::Apply)));
                for _ in 0..4 {
                    menu.draw_into(page, &buttons, Some(target.clone()));
                    next_frame().await;
                }
                menu.draw_into(page, &buttons, Some(target.clone()));
                // Read a separate framebuffer instead of the desktop window.
                let frame = target.texture.get_texture_data();
                let mut pixels = frame.bytes;
                let stride = frame.width as usize * 4;
                for y in 0..frame.height as usize / 2 {
                    let opposite = frame.height as usize - 1 - y;
                    let (top, bottom) = pixels.split_at_mut(opposite * stride);
                    top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
                }
                if page.settings() {
                    let r = Canvas::new(frame.width as f32, frame.height as f32)
                        .rect(Rect::new(50., 400., 250., 35.));
                    let mut button_pixels = 0;
                    for y in r.y.ceil() as usize..(r.bottom() as usize).min(frame.height as usize) {
                        for x in r.x.ceil() as usize..(r.right() as usize).min(frame.width as usize) {
                            let offset = y * stride + x * 4;
                            if pixels[offset + 1] > pixels[offset].saturating_add(10)
                                && pixels[offset + 2] > pixels[offset].saturating_add(10) {
                                button_pixels += 1;
                            }
                        }
                    }
                    assert!(button_pixels > 100, "Missing settings footer buttons on {page:?}");
                }
                std::fs::create_dir_all("private/dev-checks").unwrap();
                image::save_buffer(format!("private/dev-checks/settings-{name}.png"),
                    &pixels, frame.width as u32, frame.height as u32, image::ColorType::Rgba8).unwrap();
                next_frame().await;
            }
        });
    }
    #[test]
    fn localized_main_layout_preserves_umlauts_and_glow_layers() {
        let widgets = main_layout(b"resource\nButton\n{\nrect 0 455 75 25\nname backtogame\ntitle \"Zur\xfcck\"\n}\n").unwrap();
        assert_eq!(widgets[0].value("title"), "Zurück");
        assert!(main_background_glow("ui/main/maina_glow"));
        assert!(!main_background_glow("ui/main/newgame_glow"));
    }

    #[test]
    fn main_and_settings_menus_prefer_localized_ftx() -> Result<()> {
        use std::io::Write;
        let folder = std::env::temp_dir().join(format!("looking-glass-menu-{}", std::process::id()));
        std::fs::create_dir_all(&folder)?;
        let file = std::fs::File::create(folder.join("pak0.pk3"))?;
        let mut zip = zip::ZipWriter::new(file);
        for name in ["ui/main/maina.tga", "ui/main/maina.ftx",
            "ui/main/button_newgame_glow.tga", "ui/main/button_newgame_glow.ftx",
            "ui/control/head_2.tga", "ui/control/head_2.ftx"] {
            zip.start_file(name, zip::write::FileOptions::default())?;
            zip.write_all(b"resolution fixture")?;
        }
        zip.finish()?;
        let assets = Assets::open(&folder)?;
        let mut specs = BTreeMap::new();
        specs.insert("ui/main/newgame_glow".into(), texture::MaterialSpec {
            candidates: vec!["ui/main/button_newgame_glow.tga".into()],
            ..Default::default()
        });
        assert_eq!(menu_texture_path(&assets, &specs, "ui/main/maina").as_deref(), Some("ui/main/maina.ftx"));
        assert_eq!(menu_texture_path(&assets, &specs, "ui/main/newgame_glow").as_deref(), Some("ui/main/button_newgame_glow.ftx"));
        assert_eq!(menu_texture_path(&assets, &specs, "ui/control/head_2").as_deref(), Some("ui/control/head_2.ftx"));
        drop(assets);
        std::fs::remove_file(folder.join("pak0.pk3"))?;
        std::fs::remove_dir(folder)?;
        Ok(())
    }
    #[test]
    fn settings_labels_follow_cvars_and_bind_commands_in_the_archive() -> Result<()> {
        let widgets = main_layout(b"resource\nLabel\n{\nrect 0 0 10 10\ngroupid group_audio\ntitle \"Lautst\xe4rke Musik\"\n}\nresource\nSlider\n{\nrect 0 20 10 10\ngroupid group_audio\nlinkcvar s_musicvolume\n}\n")?;
        assert_eq!(setting_label(&widgets, "s_musicvolume", "Music Volume"), "Lautstärke Musik");
        assert_eq!(setting_label(&widgets, "missing", "Mute"), "Mute");
        let labels = binding_labels(b"// binditem ignored\nbinditem \"Vorw\xe4rts bewegen\" +forward run_nowep true\nbinditem \"Messer w\xe4hlen\" \"use knife\"\n")?;
        assert_eq!(labels.get(binding_command("Forward").unwrap()).map(String::as_str), Some("Vorwärts bewegen"));
        assert_eq!(labels.get(binding_command("Vorpal Blade").unwrap()).map(String::as_str), Some("Messer wählen"));
        assert!(binding_command("Choose level").is_none());
        Ok(())
    }
    #[test]
    fn layouts_are_data_and_bounds_are_checked() {
        let w=layout("menu test\nresource\nButton\n{\nrect 12 16 80 24\ntitle \"Back to game\"\nstuffcommand \"quit\"\n}\n").unwrap();
        assert_eq!(w[0].rect, Rect::new(12., 16., 80., 24.));
        assert_eq!(w[0].value("title"), "Back to game");
        assert!(layout("resource\nLabel\n{\nrect NaN 0 1 2\n}\n").is_err());
        assert!(layout("resource\nLabel\n{\nrect 0 0 -1 2\n}\n").is_err());
        assert!(layout("resource\nLabel\n{").is_err());
    }
    #[test]
    fn archive_back_button_preserves_art_and_lettering_bounds() -> Result<()> {
        let widgets = main_layout(b"resource\nButton\n{\nrect 0 455 75 25\ntitle \"Zur\xfcck\"\nshader ui/buttons/tab\nhovershader ui/buttons/tab_hover\nfontpos 25 5\nfontscale 0.8\nfontjustify center center\n}\n")?;
        let b = widgets[0].button(Click::Back);
        let lettering = Lettering::from_widget(&widgets[0]);
        assert_eq!(b.art, "ui/buttons/tab");
        assert_eq!(b.hover, "ui/buttons/tab_hover");
        assert_eq!(b.rect, Rect::new(0., 455., 75., 25.));
        assert_eq!(lettering.offset, vec2(25., 5.));
        assert_eq!(lettering.scale, 0.8);
        assert_eq!(widgets[0].value("title"), "Zurück");
        let mut plain = lettering.clone();
        plain.horizontal_start = false;
        let mut anchored = lettering.clone();
        anchored.justify = ("left".into(), "top".into());
        anchored.offset = vec2(3., 1.);
        let mut end = plain.clone();
        end.justify = ("right".into(), "bottom".into());
        end.offset = vec2(3., 1.);
        for style in [&lettering, &plain, &anchored, &end] {
            for text in ["Zurück", "Retour", "Volver al menú principal",
                "Zurück zu den vorherigen Spieleinstellungen"] {
                // Synthetic fixed-width metrics keep this test independent of GPU/assets.
                let width = text.chars().count() as f32 * 8.;
                for size in [20., 24., 40.] {
                    let rect = style.text_rect(b.rect, width, size);
                    assert_rect_inside(b.rect, rect);
                    assert!(rect.w <= width + 0.001 && rect.h <= size + 0.001);
                }
            }
        }
        Ok(())
    }
    fn assert_rect_inside(outer: Rect, inner: Rect) {
        const EPSILON: f32 = 0.001;
        assert!([inner.x, inner.y, inner.w, inner.h].into_iter().all(f32::is_finite));
        assert!(inner.w > 0. && inner.h > 0.);
        assert!(inner.x >= outer.x - EPSILON && inner.y >= outer.y - EPSILON,
            "{inner:?} starts outside {outer:?}");
        assert!(inner.right() <= outer.right() + EPSILON
            && inner.bottom() <= outer.bottom() + EPSILON,
            "{inner:?} ends outside {outer:?}");
    }
    #[test]
    fn settings_use_archive_label_control_and_hit_rectangles() -> Result<()> {
        let widgets = layout("resource\nLabel\n{\nrect 21 50 140 24\ntitle Music\ngroupid group_audio\nfontjustify left center\n}\nresource\nSlider\n{\nrect 35 69 123 16\nlinkcvar s_musicvolume\ngroupid group_audio\n}\n")?;
        let buttons = setting_buttons(&widgets, Page::Audio);
        assert_eq!(buttons.len(), 1);
        let b = &buttons[0];
        assert_eq!(b.setting.as_ref().unwrap().label.rect, Rect::new(21., 50., 140., 24.));
        assert_eq!(b.slider_rect(), Some(Rect::new(35., 69., 123., 16.)));
        assert_rect_inside(b.rect, b.setting.as_ref().unwrap().label.rect);
        assert_rect_inside(b.rect, b.slider_rect().unwrap());
        assert!(b.rect.contains(vec2(40., 80.)));
        Ok(())
    }
    #[test]
    fn mouse_leaving_button_clears_hover_without_losing_keyboard_selection() {
        assert!(highlighted(0, true, Some(0), 0));
        assert!(!highlighted(0, true, None, 0));
        assert!(!highlighted(0, true, Some(1), 0));
        assert!(highlighted(0, false, None, 0));
        assert!(!highlighted(0, false, Some(1), 1));
    }
    #[test]
    fn letterboxing_preserves_original_hit_regions() {
        let c = Canvas::new(1200., 680.);
        let original = Rect::new(61., 159., 256., 64.);
        let screen = c.rect(original);
        assert!(original.contains(c.pointer(screen.center())));
        assert!(!original.contains(c.pointer(vec2(0., 0.))));
        assert!((c.rect(Rect::new(0., 0., 640., 480.)).w / 680. - 4. / 3.).abs() < 0.001);
    }
    #[test]
    fn escape_returns_through_menu_hierarchy_never_quits() {
        assert_eq!(Page::Audio.parent(), Some(Page::Settings));
        assert_eq!(Page::Settings.parent(), Some(Page::Main));
        assert_eq!(Page::Quit.parent(), Some(Page::Main));
        assert_eq!(Page::Main.parent(), None);
        assert_eq!(
            Page::ConfirmLoad(Slot::Quick).parent(),
            Some(Page::LoadSave)
        );
    }
}
