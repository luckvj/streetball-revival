//! The screen's layers and blur over a match (docs/TRACE.md "The Level 2
//! look"), drawn by `street_render::screen_fx`:
//!
//! - **The staged effect layers**: a staged script's effect players
//!   (`FUN_00333298`, a full-screen quad each), played on the script
//!   manager's clock: the freeze's own frames while it holds, and the
//!   frames it held counted on after it (no grab shows a flash again after
//!   the freeze); the replay of the script points only its cameras, so the
//!   layers stand through it. Every Level 2 plan subtracts (0, 15, 60)
//!   from its first frame and flashes white at its freeze.
//! - **The gamebreaker overlay** (`FUN_003110e8`, a game listener): its
//!   ready tint (`+0x60`, a fader of queued requests, class `0x499758`),
//!   its flash (`+0x190`) and its blur (`+0x10`, class `0x499858`, the
//!   display's frame copy).
//!
//! Each update the listener's calls come first, then each quad draws and
//! steps its fade (`FUN_0030ff50`): staged layers, the tint, the flash.

use std::collections::VecDeque;

use bevy::prelude::*;
use street_assets::timeline::{EFFECT_PLAYERS, EffectLayer};
use street_render::screen_fx::{LAYERS, ScreenCopy, ScreenLayers};
use street_sim::{Basket, Event, World};

/// A screen quad (class `0x4997d8`): shown or not, its blend (1 a blend,
/// 2 an add, 3 a subtract, else not drawn), the colour it draws (`+0x70`,
/// whole numbers) and its fade (the colour `+0x40` stepping by `+0x60`
/// towards `+0x50` over `+0x38` updates, hiding at the end when `+0x34`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    pub shown: bool,
    pub blend: u8,
    pub colour: [i32; 4],
    current: [f32; 4],
    step: [f32; 4],
    target: [f32; 4],
    frames: i32,
    hide: bool,
}

impl Quad {
    /// `FUN_00310530`: steps from the current colour to the target over
    /// the fade's updates.
    fn aim(&mut self) {
        self.step = if self.frames > 0 {
            let n = self.frames as f32;
            std::array::from_fn(|k| (self.target[k] - self.current[k]) / n)
        } else {
            [0.0; 4]
        };
    }

    /// `FUN_0030ff50`: the colour drawn this update, if any, then the
    /// fade's step (the colour truncated, as the PS2's `cvt.w.s`).
    fn draw(&mut self) -> Option<(u8, [i32; 4])> {
        if !self.shown || !(1..=3).contains(&self.blend) {
            return None;
        }
        let drawn = (self.blend, self.colour);
        if self.frames != 0 {
            self.frames -= 1;
            if self.frames < 1 {
                self.frames = 0;
                if self.hide {
                    self.hide = false;
                    self.shown = false;
                }
                self.current = self.target;
            } else {
                for k in 0..4 {
                    self.current[k] += self.step[k];
                }
            }
            self.colour = self.current.map(|v| v as i32);
        }
        Some(drawn)
    }
}

/// A queued request (`FUN_00310810`): its updates, blend, whether the
/// quad hides at its end, and its target colour.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Request {
    frames: i32,
    blend: u8,
    hide: bool,
    target: [f32; 4],
}

/// The ready tint's rows (`0x499c98`): blend and colour; the requests
/// halve the colour. Row 0 for a side with a Level 1 ready, row 1 Level 2.
pub const TINT_ROWS: [(u8, [i32; 4]); 2] = [(2, [40, 60, 200, 100]), (3, [0, 25, 80, 255])];

/// The display's usual copy (0x7f, 0x7f).
const USUAL: ScreenCopy = ScreenCopy::USUAL;

/// The blur's copy at 60 updates a second: alpha 2880 / fps, colour 0xa0
/// (`FUN_0030fce0`).
const BLUR: ScreenCopy = ScreenCopy::new((2880 / street_sim::TICK_HZ) as u8, 0xa0);

/// Updates from a gamebreaker's start to its blur (`900 / fps`).
const BLUR_DELAY: i32 = 900 / street_sim::TICK_HZ as i32;

/// The overlay's flash: white to nothing over 60 updates (`FUN_00311810`).
const FLASH_FRAMES: i32 = 60;

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ScreenFx {
    /// The tint (`+0x60`) and its queue (`+0x90`).
    tint: Quad,
    queue: VecDeque<Request>,
    /// The request being built (`+0x100`): its fields stand between calls.
    request: Request,
    /// Each side's ready state (`+0x220`, `+0x224`): -1 none, 0 a Level 1,
    /// 1 a Level 2 (`FUN_00311328`).
    sides: [i32; 2],
    flash: Quad,
    /// A Level 2 basket's flash waits for the script's end (`+0x228`).
    flash_at_script_end: bool,
    /// The blur (`+0x10`): on, its delay (`+0x44`) and the copy it set.
    blur_on: bool,
    blur_delay: i32,
    copy: ScreenCopy,
    /// The staged script's effect layers and the frame its manager reached.
    staged: [Option<EffectLayer>; EFFECT_PLAYERS],
    script: Option<usize>,
    task: bool,
    /// This update's quads and copy.
    pub layers: ScreenLayers,
}

impl Default for ScreenFx {
    /// `FUN_00311260`: everything off, no side ready.
    fn default() -> Self {
        Self {
            tint: Quad::default(),
            queue: VecDeque::new(),
            request: Request {
                frames: 0,
                blend: 0,
                hide: false,
                target: [0.0; 4],
            },
            sides: [-1; 2],
            flash: Quad::default(),
            flash_at_script_end: false,
            blur_on: false,
            blur_delay: 0,
            copy: USUAL,
            staged: [None; EFFECT_PLAYERS],
            script: None,
            task: false,
            layers: ScreenLayers::default(),
        }
    }
}

impl ScreenFx {
    pub fn copy(&self) -> ScreenCopy {
        self.copy
    }

    /// `FUN_00310810`: the request queued, the quad shown.
    fn push(&mut self, frames: i32, hide: bool, target: [f32; 4]) {
        self.request.frames = frames;
        self.request.hide = hide;
        self.request.target = target;
        self.queue.push_back(self.request);
        self.tint.shown = true;
    }

    /// A tint row's request: its blend and its colour halved.
    fn push_row(&mut self, row: usize) {
        let (blend, colour) = TINT_ROWS[row];
        self.request.blend = blend;
        self.push(30, false, colour.map(|c| (c >> 1) as f32));
    }

    /// `FUN_00311328`: each side's ready state.
    fn read_sides(&mut self, w: &World) {
        for (side, state) in self.sides.iter_mut().enumerate() {
            *state = match w.teams.get(side) {
                Some(t) if t.meter_full() => i32::from(t.pocketed),
                _ => -1,
            };
        }
    }

    fn ready(&self) -> i32 {
        self.sides[0].max(self.sides[1])
    }

    /// `FUN_00311390`: the tint picked again: out over 60 with no side
    /// ready (hiding), else out over 60 and the row in over 30.
    fn repick(&mut self, w: &World) {
        self.read_sides(w);
        let ready = self.ready();
        if ready < 0 {
            self.push(60, true, [0.0; 4]);
        } else {
            self.push(60, false, [0.0; 4]);
            self.push_row(ready as usize);
        }
    }

    /// `FUN_003115f8` (slot 66): a meter full; from a Level 1's row to a
    /// Level 2's it goes out over 30 first.
    fn meter_full(&mut self, w: &World) {
        let before = self.ready();
        self.read_sides(w);
        let now = self.ready();
        if now == 1 && before == 0 {
            self.push(30, false, [0.0; 4]);
        }
        self.push_row(now.max(0) as usize);
    }

    /// `FUN_00311708` (slot 64): a gamebreaker's start: the blur, and for a
    /// Level 1 its own row at full, after the Level 2 row's going out.
    fn gamebreaker(&mut self, level: u8) {
        if !self.blur_on {
            self.blur_delay = BLUR_DELAY;
        }
        self.blur_on = true;
        if level == 1 {
            if self.ready() == 1 {
                self.push(30, false, [0.0; 4]);
            }
            self.request.blend = TINT_ROWS[0].0;
            self.push(30, false, TINT_ROWS[0].1.map(|c| c as f32));
        }
    }

    /// `FUN_0030fc98` (slots 65 and 67): the copy as usual at once.
    fn stop_blur(&mut self) {
        self.blur_on = false;
        self.copy = USUAL;
    }

    /// The flash (`FUN_00311810`, `FUN_00311508`): white, fading to
    /// nothing, hiding at the end.
    fn flash(&mut self) {
        let q = &mut self.flash;
        q.blend = 1;
        q.hide = true;
        q.frames = FLASH_FRAMES;
        q.shown = true;
        q.colour = [255; 4];
        q.current = [255.0; 4];
        q.target = [0.0; 4];
        q.aim();
    }

    /// The listener's calls for this update's events: a basket (26), a
    /// gamebreaker's start (64), a meter full (66), dropping (67),
    /// pocketed (70) or cancelled (71).
    pub fn hear(&mut self, w: &World, events: &[Event]) {
        for e in events {
            match *e {
                Event::Score { basket, .. } => {
                    let won = matches!(w.phase, street_sim::Phase::Over { .. });
                    match basket {
                        _ if won => self.flash(),
                        Basket::Gamebreaker { level: 1, .. } => self.flash(),
                        Basket::Gamebreaker { level: 2, .. } => self.flash_at_script_end = true,
                        _ => {}
                    }
                }
                Event::Gamebreaker { level, .. } => self.gamebreaker(level),
                Event::GamebreakerReady { .. } => self.meter_full(w),
                Event::MeterDropped { .. } => {
                    self.stop_blur();
                    self.repick(w);
                }
                Event::Pocket { .. } | Event::Cancel { .. } => self.repick(w),
                _ => {}
            }
        }
        // Slot 65: the gamebreaker task's end.
        let task = w.gamebreaker_task.is_some();
        if self.task && !task {
            self.stop_blur();
            self.repick(w);
        }
        self.task = task;
    }

    /// The staged script's effect players on its manager's clock; at its
    /// end (slot 62) a waiting flash goes off.
    pub fn stage(&mut self, w: &World, clips: &crate::setup::GameClips) {
        let Some(seq) = w.sequence.as_ref() else {
            if self.script.take().is_some() {
                self.staged = [None; EFFECT_PLAYERS];
                if std::mem::take(&mut self.flash_at_script_end) {
                    self.flash();
                }
            }
            return;
        };
        self.script = Some(seq.script);
        let hold = w
            .staged
            .as_ref()
            .and_then(|c| c.plan(seq.script))
            .and_then(|p| p.controls.hold);
        let clock = match hold {
            Some(h) if seq.frame == h.frame && seq.held > 0 => h.frame + seq.held,
            Some(h) if seq.frame > h.frame => seq.frame + h.duration,
            _ => seq.frame,
        };
        if let Some(timeline) = clips.timeline.scripts.get(seq.script) {
            self.staged = timeline.effect_layers(clock as f32);
        }
    }

    /// Each quad's draw and step, then the blur's copy (`FUN_0030fce0`).
    pub fn draw(&mut self) {
        let mut layers = ScreenLayers::default();
        let mut k = 0;
        let mut put = |drawn: Option<(u8, [i32; 4])>| {
            if let Some((blend, c)) = drawn
                && k < LAYERS
            {
                layers.colour[k] = Vec4::new(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32);
                layers.blend[k] = f32::from(blend);
                k += 1;
            }
        };
        for layer in self.staged.iter().flatten() {
            put((1..=3)
                .contains(&layer.blend)
                .then_some((layer.blend, layer.rgba)));
        }
        if self.tint.frames == 0
            && let Some(r) = self.queue.pop_front()
        {
            // `FUN_00310940`: the next request once the last is done.
            self.tint.frames = r.frames;
            self.tint.hide = r.hide;
            self.tint.blend = r.blend;
            self.tint.target = r.target;
            self.tint.aim();
            self.tint.shown = true;
        }
        put(self.tint.draw());
        put(self.flash.draw());
        self.layers = layers;
        if self.blur_delay > 0 {
            self.blur_delay -= 1;
            self.copy = USUAL;
        } else if self.blur_on {
            self.copy = BLUR;
        }
    }
}

/// The fixed step's update, after the simulation's and the replays'.
pub fn step(
    mut fx: ResMut<ScreenFx>,
    sim: Res<crate::game::Sim>,
    tick: Res<crate::game::TickEvents>,
    clips: Res<crate::setup::GameClips>,
) {
    // While a replay holds the game its world is a recorded one: the
    // listener hears nothing and the script's players stand.
    if !tick.replay {
        fx.hear(&sim.world, &tick.events);
        fx.stage(&sim.world, &clips);
    }
    fx.draw();
}

/// The match camera's passes from this update's.
pub fn show(
    mut commands: Commands,
    fx: Res<ScreenFx>,
    cameras: Query<(
        Entity,
        &Camera,
        Option<&bevy::camera::RenderTarget>,
        Option<&ScreenCopy>,
        Option<&ScreenLayers>,
        Has<crate::view::SidelineCamera>,
    )>,
) {
    // The shown buffer's copy goes on the camera drawn last to the window
    // (the cameras under it share its picture): a match's sideline camera
    // wears this update's copy, any other screen's last camera the usual
    // one. Only one, or the dither would go over the picture twice.
    let last = cameras
        .iter()
        .filter(|(_, c, target, ..)| {
            c.is_active && !matches!(target, Some(bevy::camera::RenderTarget::Image(_)))
        })
        .max_by_key(|(e, c, ..)| (c.order, *e))
        .map(|(e, ..)| e);
    for (entity, _, _, copy, layers, sideline) in &cameras {
        let mut camera = commands.entity(entity);
        let want = (Some(entity) == last).then(|| if sideline { fx.copy() } else { USUAL });
        match want {
            Some(want) if copy != Some(&want) => {
                camera.insert(want);
            }
            None if copy.is_some() => {
                camera.remove::<ScreenCopy>();
            }
            _ => {}
        }
        if !sideline {
            continue;
        }
        if fx.layers.any() {
            if layers != Some(&fx.layers) {
                camera.insert(fx.layers);
            }
        } else if layers.is_some() {
            camera.remove::<ScreenLayers>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shown buffer's copy (and so its dither) goes on one camera: the
    /// window's last, not one under it nor one drawing into a picture.
    #[test]
    fn the_copy_goes_on_the_windows_last_camera_only() {
        let mut app = App::new();
        app.init_resource::<ScreenFx>().add_systems(Update, show);
        let world = app.world_mut();
        let under = world
            .spawn(Camera {
                order: -1,
                ..default()
            })
            .id();
        let top = world.spawn(Camera::default()).id();
        let picture = world
            .spawn((
                Camera {
                    order: 5,
                    ..default()
                },
                bevy::camera::RenderTarget::from(Handle::<Image>::default()),
            ))
            .id();
        app.update();
        let copy = |app: &App, e: Entity| app.world().get::<ScreenCopy>(e).copied();
        assert_eq!(copy(&app, top), Some(USUAL));
        assert_eq!(copy(&app, under), None);
        assert_eq!(copy(&app, picture), None);
        // With the top one gone the one under it takes the copy.
        app.world_mut().despawn(top);
        app.update();
        assert_eq!(copy(&app, under), Some(USUAL));
    }

    #[test]
    fn a_quad_draws_its_colour_then_steps_and_hides_at_the_end() {
        let mut fx = ScreenFx::default();
        fx.flash();
        // The first update draws white, the fade then steps.
        assert_eq!(fx.flash.draw(), Some((1, [255; 4])));
        assert_eq!(fx.flash.draw(), Some((1, [250; 4])));
        for _ in 0..57 {
            fx.flash.draw();
        }
        // The 60th draw shows the last step; the quad hides after it.
        assert!(fx.flash.draw().is_some());
        assert_eq!(fx.flash.draw(), None);
    }

    #[test]
    fn the_ready_tint_fades_out_before_a_new_row_fades_in() {
        let mut fx = ScreenFx {
            sides: [1, -1],
            ..default()
        };
        // A row before (the first request is always a meter's row): its
        // blend stands in the request.
        fx.request.blend = 2;
        fx.push(60, false, [0.0; 4]);
        fx.push_row(1);
        fx.draw();
        // The first request: 60 updates out.
        assert_eq!(fx.tint.frames, 59);
        for _ in 0..59 {
            fx.draw();
        }
        assert_eq!(fx.tint.frames, 0);
        fx.draw();
        // Then 30 updates to (0, 12, 40) at 127 as a subtract.
        assert_eq!(fx.tint.blend, 3);
        for _ in 0..30 {
            fx.draw();
        }
        assert_eq!(fx.tint.colour, [0, 12, 40, 127]);
        assert_eq!(fx.layers.blend.x, 3.0);
    }

    #[test]
    fn the_blur_starts_15_updates_after_a_gamebreaker_and_stops_at_once() {
        let mut fx = ScreenFx::default();
        fx.gamebreaker(2);
        for _ in 0..15 {
            fx.draw();
            assert_eq!(fx.copy(), USUAL);
        }
        fx.draw();
        assert_eq!(fx.copy(), ScreenCopy::new(48, 0xa0));
        fx.stop_blur();
        assert_eq!(fx.copy(), USUAL);
        fx.draw();
        assert_eq!(fx.copy(), USUAL);
    }
}
