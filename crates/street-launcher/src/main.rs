//! Streetball Revival's launcher: find your own disc once, then play.
//!
//! One dark field of slow light (warm white, lavender, purple) with words
//! and thin lines over it, no boxes and no rounded panels. The first time,
//! it asks for the disc image and checks it is the USA PlayStation 2
//! release the game reads (SLUS-20651); the path is kept in
//! `Documents\Streetball Revival\launcher.json`, beside the game's save.
//! After that it opens on Play, which starts `street-game` next to it with
//! `--iso <disc>`.

#![cfg_attr(not(test), windows_subsystem = "windows")]

use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Key, Pos2, Rect, Sense,
    Stroke, Vec2,
    text::{LayoutJob, TextFormat},
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const BG: Color32 = Color32::from_rgb(0x07, 0x04, 0x0d);
const INK: Color32 = Color32::from_rgb(0xf4, 0xf1, 0xfa);
const MUTED: Color32 = Color32::from_rgb(0xa4, 0x9c, 0xb8);
const DIM: Color32 = Color32::from_rgb(0x6f, 0x67, 0x82);
const PURPLE: Color32 = Color32::from_rgb(0x9b, 0x30, 0xff);
const PURPLE_HI: Color32 = Color32::from_rgb(0xd2, 0x7b, 0xff);
const OK: Color32 = Color32::from_rgb(0x6e, 0xe7, 0xa0);
const ERR: Color32 = Color32::from_rgb(0xfc, 0xa5, 0xa5);

/// The executable the game's disc image must hold.
const BOOT_FILE: &str = "/SLUS_206.51";
const SERIAL: &str = "SLUS-20651";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Streetball Revival")
            .with_inner_size([1040.0, 620.0])
            .with_min_inner_size([760.0, 480.0])
            .with_icon(Arc::new(icon())),
        ..Default::default()
    };
    eframe::run_native(
        "Streetball Revival",
        options,
        Box::new(|cc| Ok(Box::new(Launcher::new(cc)))),
    )
}

/// Where the launcher keeps the disc's path.
fn config_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    let documents = home.and_then(|h| {
        [h.join("Documents"), h.join("OneDrive").join("Documents")]
            .into_iter()
            .find(|d| d.is_dir())
    });
    documents
        .map(|d| d.join("Streetball Revival"))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("launcher.json")
}

fn load_disc(config: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(config).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("disc")?.as_str().map(PathBuf::from)
}

fn save_disc(config: &Path, disc: &Path) -> Result<(), String> {
    if let Some(dir) = config.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let value = serde_json::json!({ "disc": disc.to_string_lossy() });
    std::fs::write(config, serde_json::to_string_pretty(&value).unwrap()).map_err(|e| e.to_string())
}

/// Whether `path` is a disc image of the release the game reads.
fn check_disc(path: &Path) -> Result<(), String> {
    let iso = street_assets::iso::Iso::open(path)
        .map_err(|_| "That file isn't a disc image the game can read (.iso).".to_string())?;
    if iso.stat(BOOT_FILE).is_none() {
        return Err(format!(
            "This disc isn't the USA PlayStation 2 release the game needs ({SERIAL})."
        ));
    }
    Ok(())
}

/// The game program beside the launcher.
fn game_program() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "street-game.exe"
    } else {
        "street-game"
    };
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let path = dir.join(name);
    path.is_file().then_some(path)
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Locate,
    Ready,
    Starting,
}

struct Particle {
    pos: Vec2,
    depth: f32,
    drift: Vec2,
    tint: Color32,
}

struct Launcher {
    config: PathBuf,
    disc: Option<PathBuf>,
    stage: Stage,
    since: Instant,
    message: Option<(String, Color32)>,
    particles: Vec<Particle>,
    started: Option<Instant>,
    /// `STREET_LAUNCHER_SHOT`: a PNG of the window two seconds in, then
    /// quit (for the README and checks without a person at the screen).
    shot: Option<PathBuf>,
    shot_asked: bool,
}

impl Launcher {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        let config = config_path();
        let disc = load_disc(&config).filter(|d| d.is_file());
        let stage = if disc.as_deref().is_some_and(|d| check_disc(d).is_ok()) {
            Stage::Ready
        } else {
            Stage::Locate
        };
        Self {
            config,
            disc,
            stage,
            since: Instant::now(),
            message: None,
            particles: particles(170),
            started: None,
            shot: std::env::var_os("STREET_LAUNCHER_SHOT").map(PathBuf::from),
            shot_asked: false,
        }
    }

    fn go(&mut self, stage: Stage) {
        self.stage = stage;
        self.since = Instant::now();
    }

    fn take_disc(&mut self, path: PathBuf) {
        match check_disc(&path) {
            Ok(()) => {
                if let Err(e) = save_disc(&self.config, &path) {
                    self.message = Some((format!("Found it, but couldn't remember it: {e}"), ERR));
                } else {
                    self.message = None;
                }
                self.disc = Some(path);
                self.go(Stage::Ready);
            }
            Err(e) => self.message = Some((e, ERR)),
        }
    }

    fn browse(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Locate your disc image")
            .add_filter("Disc image", &["iso"])
            .add_filter("All files", &["*"]);
        if let Some(dir) = self.disc.as_deref().and_then(Path::parent) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.take_disc(path);
        }
    }

    fn play(&mut self) {
        let Some(disc) = self.disc.clone() else {
            self.go(Stage::Locate);
            return;
        };
        let Some(game) = game_program() else {
            self.message = Some((
                "street-game wasn't found next to the launcher. Unzip the whole release into one folder."
                    .into(),
                ERR,
            ));
            return;
        };
        let mut command = std::process::Command::new(&game);
        command.arg("--iso").arg(&disc);
        if let Some(dir) = game.parent() {
            command.current_dir(dir);
        }
        match command.spawn() {
            Ok(_) => {
                self.message = Some(("Starting the game…".into(), OK));
                self.started = Some(Instant::now());
                self.go(Stage::Starting);
            }
            Err(e) => self.message = Some((format!("The game didn't start: {e}"), ERR)),
        }
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        // A disc image dropped on the window.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.take_disc(path);
        }
        if self
            .started
            .is_some_and(|t| t.elapsed().as_secs_f32() > 1.6)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(path) = self.shot.clone() {
            if !self.shot_asked && ctx.input(|i| i.time) > 2.0 {
                self.shot_asked = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let image = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(image) = image {
                let [w, h] = image.size;
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(png) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    let _ = png.save(&path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG))
            .show(ctx, |ui| {
                let screen = ui.max_rect();
                let painter = ui.painter_at(screen);
                let time = ctx.input(|i| i.time) as f32;
                self.draw_field(&painter, screen, time);

                let t = self.since.elapsed().as_secs_f32();
                let fade = ease((t / 0.9).min(1.0));
                let rise = (1.0 - fade) * 14.0;
                let center = screen.center() + Vec2::new(0.0, rise);

                // The mark, top left.
                let mark = Pos2::new(screen.left() + 34.0, screen.top() + 30.0);
                words(&painter, mark, Align2::LEFT_TOP, "STREETBALL REVIVAL", 15.0, INK, 0.30, true);
                words(
                    &painter,
                    mark + Vec2::new(0.0, 22.0),
                    Align2::LEFT_TOP,
                    "BETA",
                    11.0,
                    PURPLE_HI,
                    0.40,
                    true,
                );

                match self.stage {
                    Stage::Locate => {
                        words(
                            &painter,
                            center + Vec2::new(0.0, -92.0),
                            Align2::CENTER_CENTER,
                            "Locate your disc",
                            52.0,
                            alpha(INK, fade),
                            0.02,
                            true,
                        );
                        words(
                            &painter,
                            center + Vec2::new(0.0, -34.0),
                            Align2::CENTER_CENTER,
                            &format!(
                                "Choose the .iso disc image of your own PlayStation 2 copy (USA, {SERIAL})."
                            ),
                            16.0,
                            alpha(MUTED, fade),
                            0.02,
                            false,
                        );
                        words(
                            &painter,
                            center + Vec2::new(0.0, -10.0),
                            Align2::CENTER_CENTER,
                            "Nothing from the disc is copied or sent anywhere. You can also drop it here.",
                            14.0,
                            alpha(DIM, fade),
                            0.02,
                            false,
                        );
                        let browse = line_button(ui, center + Vec2::new(0.0, 46.0), "Browse", true);
                        if browse || ctx.input(|i| i.key_pressed(Key::Enter)) {
                            self.browse();
                        }
                        if self.disc.is_some()
                            && line_button(ui, center + Vec2::new(0.0, 96.0), "Back", false)
                        {
                            self.message = None;
                            self.go(Stage::Ready);
                        }
                    }
                    Stage::Ready | Stage::Starting => {
                        let name = self
                            .disc
                            .as_deref()
                            .and_then(Path::file_name)
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        words(
                            &painter,
                            center + Vec2::new(0.0, -96.0),
                            Align2::CENTER_CENTER,
                            "Ready to play",
                            52.0,
                            alpha(INK, fade),
                            0.02,
                            true,
                        );
                        words(
                            &painter,
                            center + Vec2::new(0.0, -40.0),
                            Align2::CENTER_CENTER,
                            &format!("{name}  ·  USA  ·  {SERIAL}"),
                            15.0,
                            alpha(MUTED, fade),
                            0.06,
                            false,
                        );
                        let line_y = center.y - 14.0;
                        painter.line_segment(
                            [
                                Pos2::new(center.x - 160.0 * fade, line_y),
                                Pos2::new(center.x + 160.0 * fade, line_y),
                            ],
                            Stroke::new(1.0_f32, alpha(PURPLE_HI, 0.5 * fade)),
                        );
                        if self.stage == Stage::Ready {
                            let play = line_button(ui, center + Vec2::new(0.0, 36.0), "Play", true);
                            if play || ctx.input(|i| i.key_pressed(Key::Enter)) {
                                self.play();
                            }
                            if line_button(ui, center + Vec2::new(-90.0, 96.0), "Change disc", false) {
                                self.message = None;
                                self.go(Stage::Locate);
                            }
                            if line_button(ui, center + Vec2::new(90.0, 96.0), "Quit", false) {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                }

                if let Some((text, color)) = &self.message {
                    words(
                        &painter,
                        center + Vec2::new(0.0, 150.0),
                        Align2::CENTER_CENTER,
                        text,
                        14.0,
                        *color,
                        0.02,
                        false,
                    );
                }

                // The fine print.
                words(
                    &painter,
                    Pos2::new(screen.center().x, screen.bottom() - 26.0),
                    Align2::CENTER_CENTER,
                    "Unofficial fan project. Not affiliated with or endorsed by the original game's makers or its league. You need your own disc.",
                    11.5,
                    DIM,
                    0.02,
                    false,
                );
                words(
                    &painter,
                    Pos2::new(screen.right() - 30.0, screen.top() + 30.0),
                    Align2::RIGHT_TOP,
                    &format!("v{VERSION}"),
                    11.0,
                    DIM,
                    0.2,
                    false,
                );
                if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
}

impl Launcher {
    /// The field: slow motes of light drifting up, nearer ones larger,
    /// brighter and quicker, with a purple glow under the middle.
    fn draw_field(&mut self, painter: &egui::Painter, screen: Rect, time: f32) {
        // The court's light: a wide, soft purple pool low in the middle,
        // breathing slowly.
        let breath = 0.9 + 0.1 * (time * 0.5).sin();
        glow(
            painter,
            screen.center() + Vec2::new(0.0, screen.height() * 0.32),
            Vec2::new(screen.width() * 0.55, screen.height() * 0.42),
            PURPLE,
            0.22 * breath,
        );
        glow(
            painter,
            screen.center() + Vec2::new(0.0, screen.height() * 0.30),
            Vec2::new(screen.width() * 0.22, screen.height() * 0.14),
            PURPLE_HI,
            0.10 * breath,
        );
        for p in &mut self.particles {
            p.pos += p.drift * (0.4 + p.depth);
            if p.pos.y < -0.05 {
                p.pos.y += 1.1;
            }
            if p.pos.x < -0.05 {
                p.pos.x += 1.1;
            } else if p.pos.x > 1.05 {
                p.pos.x -= 1.1;
            }
            let twinkle = 0.65 + 0.35 * (time * (0.6 + p.depth) + p.pos.x * 40.0).sin();
            let at = Pos2::new(
                screen.left() + p.pos.x * screen.width(),
                screen.top() + p.pos.y * screen.height(),
            );
            let radius = 0.6 + p.depth * 2.2;
            painter.circle_filled(at, radius * 3.0, alpha(p.tint, 0.05 * twinkle * p.depth));
            painter.circle_filled(at, radius, alpha(p.tint, (0.25 + 0.6 * p.depth) * twinkle));
        }
    }
}

/// Words, optionally bold (the condensed face's heavy cut when Windows
/// has one), letter-spaced by `spacing` ems.
#[allow(clippy::too_many_arguments)]
fn words(
    painter: &egui::Painter,
    at: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
    spacing: f32,
    bold: bool,
) -> Rect {
    let family = if bold {
        FontFamily::Name("heavy".into())
    } else {
        FontFamily::Proportional
    };
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::new(size, family),
            color,
            extra_letter_spacing: size * spacing,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let rect = anchor.anchor_size(at, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

/// A word button: capitals, wide-spaced, a thin line under it on hover
/// (purple for the main one). True when clicked.
fn line_button(ui: &mut egui::Ui, at: Pos2, label: &str, primary: bool) -> bool {
    let size = if primary { 22.0 } else { 14.0 };
    let painter = ui.painter().clone();
    let text = label.to_uppercase();
    // Measure first, then claim the area.
    let probe = words(
        &painter,
        Pos2::new(-10_000.0, -10_000.0),
        Align2::LEFT_TOP,
        &text,
        size,
        Color32::TRANSPARENT,
        0.24,
        true,
    );
    let rect = Align2::CENTER_CENTER.anchor_size(at, probe.size() + Vec2::new(16.0, 14.0));
    let response = ui.interact(rect, ui.id().with(label), Sense::click());
    let hot = response.hovered();
    let color = if hot {
        Color32::WHITE
    } else if primary {
        INK
    } else {
        MUTED
    };
    let spacing = if hot { 0.28 } else { 0.24 };
    let drawn = words(
        &painter,
        at,
        Align2::CENTER_CENTER,
        &text,
        size,
        color,
        spacing,
        true,
    );
    let line = if primary {
        alpha(PURPLE_HI, if hot { 1.0 } else { 0.55 })
    } else if hot {
        alpha(INK, 0.7)
    } else {
        Color32::TRANSPARENT
    };
    painter.line_segment(
        [
            Pos2::new(drawn.left(), drawn.bottom() + 6.0),
            Pos2::new(drawn.right(), drawn.bottom() + 6.0),
        ],
        Stroke::new(1.0_f32, line),
    );
    if hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// A soft elliptical pool of `color`, `peak` opaque in the middle and
/// fading to nothing at its edge.
fn glow(painter: &egui::Painter, center: Pos2, radius: Vec2, color: Color32, peak: f32) {
    const RINGS: u32 = 20;
    const SEGMENTS: u32 = 72;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, alpha(color, peak));
    for ring in 1..=RINGS {
        let r = ring as f32 / RINGS as f32;
        let a = peak * (1.0 - r).powf(2.2);
        for seg in 0..SEGMENTS {
            let t = seg as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let at = center + Vec2::new(t.cos() * radius.x * r, t.sin() * radius.y * r);
            mesh.colored_vertex(at, alpha(color, a));
        }
    }
    let at = |ring: u32, seg: u32| 1 + (ring - 1) * SEGMENTS + seg % SEGMENTS;
    for seg in 0..SEGMENTS {
        mesh.add_triangle(0, at(1, seg), at(1, seg + 1));
    }
    for ring in 1..RINGS {
        for seg in 0..SEGMENTS {
            let (a, b) = (at(ring, seg), at(ring, seg + 1));
            let (c, d) = (at(ring + 1, seg), at(ring + 1, seg + 1));
            mesh.add_triangle(a, c, b);
            mesh.add_triangle(b, c, d);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn alpha(c: Color32, a: f32) -> Color32 {
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// The motes, from a fixed seed: warm white, lavender and purple.
fn particles(n: usize) -> Vec<Particle> {
    let mut seed: u32 = 0x5eed_1234;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32) / (u32::MAX as f32)
    };
    let tints = [
        Color32::from_rgb(0xff, 0xf3, 0xe0),
        Color32::from_rgb(0xc9, 0xb6, 0xff),
        PURPLE_HI,
    ];
    (0..n)
        .map(|_| {
            let depth = next().powi(2);
            Particle {
                pos: Vec2::new(next(), next()),
                depth,
                drift: Vec2::new((next() - 0.5) * 0.0002, -(0.00015 + next() * 0.0004)),
                tint: tints[(next() * 3.0) as usize % 3],
            }
        })
        .collect()
}

/// Windows' condensed face when it has one (the look of the game's own
/// menus), else egui's.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let windir = std::env::var_os("WINDIR").map(PathBuf::from);
    let face = windir
        .map(|w| w.join("Fonts").join("bahnschrift.ttf"))
        .and_then(|p| std::fs::read(p).ok());
    let mut heavy = Vec::new();
    if let Some(bytes) = face {
        fonts
            .font_data
            .insert("condensed".into(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "condensed".into());
        heavy.push("condensed".to_string());
    }
    heavy.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| f != "condensed"),
    );
    fonts
        .families
        .insert(FontFamily::Name("heavy".into()), heavy);
    ctx.set_fonts(fonts);
}

/// The window's icon: a purple square with a light slash.
fn icon() -> egui::IconData {
    let n = 64u32;
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let t = (x + y) as f32 / (2 * n) as f32;
            let slash = ((x as i32 - (n as i32 - 1 - y as i32)).abs() as f32) < 7.0;
            let (r, g, b) = if slash {
                (0xf4, 0xf1, 0xfa)
            } else {
                let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
                (mix(0x9b, 0x3a), mix(0x30, 0x10), mix(0xff, 0x70))
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    egui::IconData {
        rgba,
        width: n,
        height: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_no_disc_image_says_so() {
        let dir = std::env::temp_dir().join("street-launcher-test.iso");
        std::fs::write(&dir, b"not a disc").unwrap();
        assert!(check_disc(&dir).is_err());
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn the_disc_path_is_remembered() {
        let config = std::env::temp_dir()
            .join("street-launcher-test")
            .join("launcher.json");
        save_disc(&config, Path::new("C:/games/disc.iso")).unwrap();
        assert_eq!(load_disc(&config), Some(PathBuf::from("C:/games/disc.iso")));
        let _ = std::fs::remove_dir_all(config.parent().unwrap());
    }

    #[test]
    fn the_real_disc_passes_when_there_is_one() {
        if let Some(path) = std::env::var_os("STREET_ISO") {
            assert_eq!(check_disc(Path::new(&path)), Ok(()));
        }
    }
}
