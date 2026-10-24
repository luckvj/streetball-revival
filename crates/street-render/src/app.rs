//! Pieces shared by the Streetball Revival programs: finding the disc image and taking
//! a scripted screenshot.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};

pub const DEFAULT_ISO: &str = "game.iso";

/// The disc image: `$STREET_ISO`, else [`DEFAULT_ISO`] in the current
/// folder, the program's folder or a parent of either, else any disc image
/// there that is the US the original game (whatever its file is called).
pub fn find_iso() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("STREET_ISO") {
        return Ok(p.into());
    }
    let mut folders: Vec<PathBuf> = Vec::new();
    let starts = [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(PathBuf::from)),
    ];
    for start in starts.into_iter().flatten() {
        for d in start.ancestors().take(8) {
            if !folders.iter().any(|f| f == d) {
                folders.push(d.to_path_buf());
            }
        }
    }
    if let Some(p) = folders
        .iter()
        .map(|d| d.join(DEFAULT_ISO))
        .find(|p| p.is_file())
    {
        return Ok(p);
    }
    for d in &folders {
        let Ok(entries) = std::fs::read_dir(d) else {
            continue;
        };
        let mut isos: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("iso")))
            .collect();
        isos.sort();
        if let Some(p) = isos.into_iter().find(|p| is_the_game(p)) {
            return Ok(p);
        }
    }
    Err(
        "could not find the original game's disc image. Put your .iso \
         next to this program or in a folder above it (any name), or pass \
         --iso <path>, or set STREET_ISO"
            .into(),
    )
}

/// Whether `path` is a disc image of the US game (it holds SLUS-20651).
fn is_the_game(path: &std::path::Path) -> bool {
    street_assets::iso::Iso::open(path).is_ok_and(|mut iso| iso.read("/SLUS_206.51").is_ok())
}

/// Save one frame to `path` after `delay_secs` of real time and exit. Shader pipelines
/// compile in the background (several seconds on a cold DX12 cache) and meshes
/// are not drawn until they are ready, so a frame-count delay is not enough.
pub struct ScreenshotPlugin {
    pub path: PathBuf,
    pub delay_secs: f32,
}

/// Game seconds for [`ScreenshotPlugin`]. An app that keeps this resource
/// has its capture counted in game time instead of real time, so it lands on
/// the same simulated moment however long loading takes; `None` until the
/// game's clock is running.
#[derive(Resource, Default)]
pub struct ScreenshotClock(pub Option<f32>);

/// The screenshot's check, in `Update`: an app that keeps a
/// [`ScreenshotClock`] sets it before this set in the same frame, so the
/// frame captured is the first whose clock reaches the delay.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenshotSystems;

/// Whether a capture `delay_secs` in is due: by the game's clock when the app
/// keeps one, else by real time.
fn screenshot_due(delay_secs: f32, real_secs: f32, clock: Option<&ScreenshotClock>) -> bool {
    match clock {
        Some(clock) => clock.0.is_some_and(|secs| secs >= delay_secs),
        None => real_secs >= delay_secs,
    }
}

/// A series for [`ScreenshotPlugin`]: `count` frames `every` seconds apart
/// from its delay, each saved as `<stem>-NNN.<ext>` (by the same clock as
/// the one frame). Without it the plugin saves one frame at its path.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ScreenshotSeries {
    pub every: f32,
    pub count: usize,
}

#[derive(Resource)]
struct ScreenshotPlan {
    path: PathBuf,
    delay_secs: f32,
    requested: usize,
    saved: Arc<AtomicUsize>,
}

/// The series' frame `k`'s file: the path with `-NNN` before its extension.
fn series_path(path: &std::path::Path, k: usize) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map_or(String::new(), |e| format!(".{}", e.to_string_lossy()));
    path.with_file_name(format!("{stem}-{k:03}{ext}"))
}

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScreenshotPlan {
            path: self.path.clone(),
            delay_secs: self.delay_secs,
            requested: 0,
            saved: default(),
        })
        .add_systems(Update, take_screenshot.in_set(ScreenshotSystems));
    }
}

fn take_screenshot(
    mut commands: Commands,
    mut plan: ResMut<ScreenshotPlan>,
    time: Res<Time<Real>>,
    clock: Option<Res<ScreenshotClock>>,
    series: Option<Res<ScreenshotSeries>>,
    mut exit: MessageWriter<AppExit>,
) {
    let count = series.as_ref().map_or(1, |s| s.count.max(1));
    if plan.saved.load(Ordering::SeqCst) >= count {
        println!("saved {}", plan.path.display());
        exit.write(AppExit::Success);
        return;
    }
    let k = plan.requested;
    let at = plan.delay_secs + series.as_ref().map_or(0.0, |s| s.every * k as f32);
    if k >= count || !screenshot_due(at, time.elapsed_secs(), clock.as_deref()) {
        return;
    }
    plan.requested += 1;
    let path = if series.is_some() {
        series_path(&plan.path, k)
    } else {
        plan.path.clone()
    };
    let saved = plan.saved.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(move |_: On<ScreenshotCaptured>| {
            saved.fetch_add(1, Ordering::SeqCst);
        });
}

pub fn quit_on_escape(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

/// The window every program opens: 1280x720, or `STREET_WINDOW` (`WxH`,
/// e.g. `640x480`) for checking a layout at another size.
pub fn window_plugin(title: String) -> WindowPlugin {
    let size = std::env::var("STREET_WINDOW")
        .ok()
        .and_then(|s| window_size(&s))
        .unwrap_or((1280, 720));
    WindowPlugin {
        primary_window: Some(Window {
            title,
            resolution: size.into(),
            ..default()
        }),
        ..default()
    }
}

/// `WxH` as a window size.
fn window_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    (w >= 160 && h >= 120).then_some((w, h))
}

/// Marker for the directional light illuminating the court and players.
#[derive(Component, Debug, Default)]
pub struct CourtSun;

/// The court scene's light: a sun for the lit player models (the courts are
/// unlit and carry baked lighting).
pub fn spawn_sun(mut commands: Commands) {
    commands.spawn((
        CourtSun,
        DirectionalLight {
            illuminance: 6000.0,
            ..default()
        },
        Transform::from_xyz(30.0, 60.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Aligns the directional light with the court's authentic sun direction from `CourtFx`.
pub fn sync_court_sun(
    style: Option<Res<crate::shadow::ShadowStyle>>,
    mut query: Query<&mut Transform, With<CourtSun>>,
) {
    if let Some(style) = style {
        for mut transform in &mut query {
            let sun_pos = -style.direction.normalize() * 100.0;
            *transform = Transform::from_translation(sun_pos).looking_at(Vec3::ZERO, Vec3::Y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_clock_holds_the_capture_until_the_game_reaches_it() {
        // Without a clock: real time.
        assert!(!screenshot_due(5.0, 4.9, None));
        assert!(screenshot_due(5.0, 5.0, None));
        // With one: real time no longer counts, and a clock that has not
        // started holds the capture through loading.
        let loading = ScreenshotClock(None);
        assert!(!screenshot_due(5.0, 60.0, Some(&loading)));
        assert!(!screenshot_due(
            5.0,
            60.0,
            Some(&ScreenshotClock(Some(4.9)))
        ));
        assert!(screenshot_due(5.0, 0.0, Some(&ScreenshotClock(Some(5.0)))));
    }
}
