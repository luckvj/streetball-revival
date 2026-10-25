//! Drawing the simulation: players, the ball, the camera and the score.

use std::f32::consts::PI;

use bevy::prelude::*;
use street_render::{
    animation::{Animator, Rig},
    court::DrawGroup,
};
use street_sim::{
    Action, BallState, Gait, PassStyle, Phase, ball, player::DunkStyle, records::Picture, street,
    tuning,
};

use crate::{
    Screen, channel, clip,
    controls::{Control, MAX_PEOPLE, SEAT_COLOURS},
    game::{Replay, Sim},
    setup::{self, Disc, GameClips, HandJoint, TeamNames},
};

/// The sideline camera: height above and distance from the court's long axis.
const CAMERA_HEIGHT: f32 = 24.0;
const CAMERA_DISTANCE: f32 = 52.0;
// FUN_003797e0 initializes the viewport zoom read as object +0x190.
#[cfg(test)]
const CAMERA_PROJECTION_ZOOM: f32 = 1.3;
/// The zoom the picture shows: the projection object's viewport scales
/// (`+0x120`/`+0x124`) undo `+0x190`'s 1.3, so a camera's lens `+0x98`
/// is the tangent of half the 4:3 picture's height. Measured on the
/// original's grabs (`docs/CAMERA_RECOVERY.md` "The picture's zoom"): the
/// rim under a staged camera and two players under the Street row land
/// within a few pixels only without the 1.3.
const SCREEN_ZOOM: f32 = 1.0;

/// The thirteen words of each 0x34-byte row at `0x498478` (ELF data):
/// x bound, x offset, eye y, focus-z scale, the three depth offsets (+0x10
/// the base, +0x14 and +0x18 the two the Street and Court depth pointer can
/// take), the lens, the screen thresholds, and Street's three recovery
/// targets (+0x28 .. +0x30).
const CAMERA_MODE_ROWS: [CameraModeRow; 6] = [
    CameraModeRow {
        x_bound: 4.0,
        x_offset: 5.0,
        eye_y: 12.0,
        focus_z_scale: 12.0,
        focus_z_offset: -30.0,
        low_depth: -30.0,
        high_depth: -40.0,
        row_word7: 0.33,
        horizontal_screen_threshold: 185,
        top_screen_threshold: 115,
        recover_low: 4.0,
        recover_high: 18.0,
        recover_lens: 0.18,
    },
    CameraModeRow {
        x_bound: 15.0,
        x_offset: 5.0,
        eye_y: 16.0,
        focus_z_scale: 12.0,
        focus_z_offset: -40.0,
        low_depth: -26.0,
        high_depth: -55.0,
        row_word7: 0.23,
        horizontal_screen_threshold: 185,
        top_screen_threshold: 115,
        recover_low: 0.0,
        recover_high: 0.0,
        recover_lens: 0.0,
    },
    CameraModeRow {
        x_bound: 18.0,
        x_offset: 12.0,
        eye_y: 22.0,
        focus_z_scale: 0.0,
        focus_z_offset: -65.0,
        low_depth: 0.0,
        high_depth: 0.0,
        row_word7: 0.18,
        horizontal_screen_threshold: 170,
        top_screen_threshold: 100,
        recover_low: 0.0,
        recover_high: 0.0,
        recover_lens: 0.0,
    },
    CameraModeRow {
        x_bound: 0.0,
        x_offset: 0.0,
        eye_y: 28.0,
        focus_z_scale: 0.0,
        focus_z_offset: -75.0,
        low_depth: 0.0,
        high_depth: 0.0,
        row_word7: 0.15,
        horizontal_screen_threshold: 230,
        top_screen_threshold: 90,
        recover_low: 0.0,
        recover_high: 0.0,
        recover_lens: 0.0,
    },
    CameraModeRow {
        x_bound: 0.0,
        x_offset: -60.0,
        eye_y: 19.0,
        focus_z_scale: 26.5,
        focus_z_offset: 0.0,
        low_depth: 0.0,
        high_depth: 0.0,
        row_word7: 0.281,
        horizontal_screen_threshold: 300,
        top_screen_threshold: 90,
        recover_low: 0.0,
        recover_high: 0.0,
        recover_lens: 0.0,
    },
    CameraModeRow {
        x_bound: 4.0,
        x_offset: 5.0,
        eye_y: 12.0,
        focus_z_scale: 8.0,
        focus_z_offset: -40.0,
        low_depth: 0.0,
        high_depth: 0.0,
        row_word7: 0.315,
        horizontal_screen_threshold: 185,
        top_screen_threshold: 115,
        recover_low: 0.0,
        recover_high: 0.0,
        recover_lens: 0.0,
    },
];

#[derive(Clone, Copy)]
struct CameraModeRow {
    x_bound: f32,
    x_offset: f32,
    eye_y: f32,
    focus_z_scale: f32,
    /// +0x10: the depth offset of the base pointer.
    focus_z_offset: f32,
    /// +0x14 (`DAT_0049848c` for Street): the depth of [`RowDepth::Low`].
    low_depth: f32,
    /// +0x18 (`UNK_00498490`): the depth of [`RowDepth::High`].
    high_depth: f32,
    /// +0x1c: the lens `FUN_002f8370` assigns to `+0x98` each update.
    row_word7: f32,
    /// Row word +0x20 enters the left/right threshold slots in FUN_002ff830.
    horizontal_screen_threshold: i32,
    /// Row word +0x24 enters the top threshold slot in FUN_002ff830.
    top_screen_threshold: i32,
    /// +0x28 .. +0x30: Street's base-pointer recovery (`FUN_002f8370`): the
    /// height comes back from below at `(eye_y - recover_low) / 50` an
    /// update, from above at `(recover_high - eye_y) / 50`, and the lens
    /// from below at `(row_word7 - recover_lens) / 50`.
    recover_low: f32,
    recover_high: f32,
    recover_lens: f32,
}

/// The rendered player for `World::players[index]`.
#[derive(Component)]
pub struct SimPlayer(pub usize);

#[derive(Component)]
pub struct BallView;

#[derive(Component)]
pub struct SidelineCamera;

#[derive(Component)]
pub struct ScoreText;

/// The trick points and the gamebreaker meters, under the score.
#[derive(Component)]
pub struct MeterText;

/// Pose from `FUN_002f7c38` and the mode row at `0x498478`.
/// Behind uses its native side-dependent yaw and one-sided x bound.
fn mode_camera_pose(focus: Vec3, side: f32, mode: usize) -> (Vec3, Vec3) {
    let mode = mode.min(CAMERA_MODE_ROWS.len() - 1);
    let row = CAMERA_MODE_ROWS[mode];
    if mode == 4 {
        let sign = if side == 0.0 { 1.0 } else { side.signum() };
        let mut x = focus.x + sign * row.x_offset;
        // The native Behind branch clamps only toward the court center:
        // positive side has an upper bound of zero, negative side a lower one.
        if sign > 0.0 {
            x = x.min(row.x_bound);
        } else {
            x = x.max(-row.x_bound);
        }
        let eye = Vec3::new(x, row.eye_y, focus.z);
        // The native Behind branch fixes yaw from `side`, derives pitch from
        // a ground-level target, and uses the unclamped 60-unit x offset for
        // horizontal range. Rebuild that direction from the clamped eye.
        let view_target = eye + Vec3::new(sign * -row.x_offset, -row.eye_y, 0.0);
        return (eye, view_target);
    }

    let x = (focus.x + side * row.x_offset).clamp(-row.x_bound, row.x_bound);
    let z = focus.z * row.focus_z_scale / 26.5 + row.focus_z_offset;
    (Vec3::new(x, row.eye_y, z), focus)
}

// Normal finite camera arithmetic follows the captured EE execution policy
// (`street_assets::ee`): ordinary operations chop, while division and square
// root round to nearest.
use street_assets::ee::{add as camera_add, chop as camera_chop, mul as camera_mul};

/// Native finite-scalar atan polynomial used by FUN_00300990.
fn native_camera_atan(x: f32) -> f32 {
    let square = camera_mul(x, x);
    let t = if x.abs() > 1.0 {
        1.0 / camera_add(square, 1.0)
    } else {
        square / camera_add(square, 1.0)
    };
    let mut polynomial = f32::from_bits(0x3d6137ab);
    for coefficient in [0x3d55033a, 0x3d8a908b, 0x3e2bba25] {
        polynomial = camera_add(f32::from_bits(coefficient), camera_mul(t, polynomial));
    }
    let result = camera_mul(t.sqrt(), camera_add(1.0, camera_mul(t, polynomial)));
    if x < -1.0 {
        camera_add(result, -std::f32::consts::FRAC_PI_2)
    } else if x > 1.0 {
        camera_add(std::f32::consts::FRAC_PI_2, -result)
    } else if x < 0.0 {
        -result
    } else {
        result
    }
}

/// Follow the native polynomial, degree conversion, and renderer half-angle
/// conversion before deriving Bevy's vertical FOV. Rust's tangent may differ
/// from the EE tangent kernel by an ulp. Street's conditional height/lens
/// offsets remain outside this base-row presentation fallback.
#[cfg(test)]
fn mode_vertical_fov(mode: usize, picture: Picture) -> f32 {
    let row = CAMERA_MODE_ROWS[mode.min(CAMERA_MODE_ROWS.len() - 1)];
    lens_vertical_fov(row.row_word7, picture)
}

/// Bevy's vertical FOV for a native lens scalar `+0x98` through the
/// renderer's three projections (`FUN_00323450`, by the wrapper's two
/// flags): the 4:3 picture; Widescreen's PS2 anamorphic picture (the first
/// flag, `wrapper+0xc`: the same horizontal angle at aspect 16/9, so the
/// vertical angle's tangent shrinks by 3/4); and the rewrite's Wide picture
/// (both flags: the horizontal half-angle's tangent widened by 4/3 before
/// the 16/9 projection, which keeps the 4:3 vertical angle), each at the
/// picture's own zoom ([`SCREEN_ZOOM`]). For the Street row's lens 0.33
/// that is 36.53 degrees of height at 4:3, 27.80 anamorphic and 36.53
/// wide, 47.50 degrees of width for the first two and 60.80 wide
/// (`docs/CAMERA_RECOVERY.md` "The three pictures", "The picture's zoom").
fn lens_vertical_fov(scalar: f32, picture: Picture) -> f32 {
    let (horizontal_degrees, vertical_4_3) =
        scalar_camera_projection(scalar, 4.0 / 3.0, SCREEN_ZOOM);
    match picture {
        Picture::Standard => vertical_4_3,
        Picture::Anamorphic => {
            consumed_camera_vertical_fov(horizontal_degrees, 16.0 / 9.0, SCREEN_ZOOM)
        }
        Picture::Wide => consumed_camera_vertical_fov(
            wide_horizontal_degrees(horizontal_degrees),
            16.0 / 9.0,
            SCREEN_ZOOM,
        ),
    }
}

/// `FUN_00323450`'s both-flags branch: the angle whose half-angle tangent
/// is 4/3 of the given angle's (`FUN_003af898` tangent, `FUN_003af3f8`
/// arctangent, the native constants `0.017453292`, `1.3333334` and
/// `57.295776`). Rust's tangent and arctangent may differ from the EE
/// kernels by an ulp.
fn wide_horizontal_degrees(horizontal_degrees: f32) -> f32 {
    let half_angle = camera_mul(camera_mul(horizontal_degrees, 0.017_453_292), 0.5);
    let widened = camera_mul(half_angle.tan(), 1.333_333_4).atan();
    camera_mul(camera_add(widened, widened), 57.295_776)
}

/// Shared production conversion, also accepting a native positioned-lens
/// scalar for an externally captured comparison. This does not select a
/// controller, generate its scalar, or reconstruct a native camera pose.
fn scalar_camera_projection(scalar: f32, aspect: f32, zoom: f32) -> (f32, f32) {
    let half_angle = native_camera_atan(camera_mul(scalar, 4.0 / 3.0));
    let horizontal_degrees = camera_mul(camera_add(half_angle, half_angle), 57.295_776);
    (
        horizontal_degrees,
        consumed_camera_vertical_fov(horizontal_degrees, aspect, zoom),
    )
}

/// Renderer-stage conversion from the native projection object's consumed
/// angle (+0x110), also shared by the production scalar conversion above.
/// Accept the observed degree word directly: no scalar or atan reconstruction.
fn consumed_camera_vertical_fov(horizontal_degrees: f32, aspect: f32, zoom: f32) -> f32 {
    let renderer_half_angle = camera_mul(camera_mul(horizontal_degrees, 0.5), 0.017_453_294);
    let projection_tangent = camera_mul(renderer_half_angle.tan(), zoom);
    2.0 * (projection_tangent / aspect).atan()
}

/// `FUN_002f8338`'s shake of the row's eye height: `left` of `updates`
/// updates, `cycles` half turns of a sine over them (`+0x204`), `amplitude`
/// court units (`+0x208`). `FUN_002f8370` assigns the row's height each
/// update and then lowers it by `amplitude * sin(cycles * pi * left /
/// updates)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowShake {
    pub left: u16,
    pub updates: u16,
    pub cycles: u16,
    pub amplitude: f32,
}

impl RowShake {
    /// One update's drop (raw `0x2f8a3c..`): the count goes down first, and
    /// its last update leaves the height alone.
    fn next(&mut self) -> f32 {
        if self.left == 0 {
            return 0.0;
        }
        self.left -= 1;
        if self.left == 0 {
            return 0.0;
        }
        let angle =
            f32::from(self.cycles) * PI * f32::from(self.left) * (1.0 / f32::from(self.updates));
        self.amplitude * native_sin(angle)
    }
}

/// `FUN_002f8370`'s inline sine: whole turns off, the nearest quarter
/// turn (`cvt.w.s` of `x * 2/pi +- 0.5`, taken as truncation), then the
/// single-precision sine or cosine polynomial.
fn native_sin(mut x: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let whole = (x / turn) as i32;
    if 0 < whole {
        x -= whole as f32 * turn;
    }
    let quarter = if x < 0.0 {
        x * std::f32::consts::FRAC_2_PI - 0.5
    } else {
        x * std::f32::consts::FRAC_2_PI + 0.5
    } as i32;
    let r = x - quarter as f32 * std::f32::consts::FRAC_PI_2;
    let r2 = r * r;
    let value = if quarter & 1 == 0 {
        (((r2 * 2.755_731_9e-6 + -0.000_198_412_7) * r2 + 0.008_333_334) * r2 + -0.166_666_67)
            * r2
            * r
            + r
    } else {
        (((r2 * 2.480_158_8e-5 + -0.001_388_888_9) * r2 + 0.041_666_668) * r2 + -0.5) * r2 + 1.0
    };
    if quarter & 2 != 0 { -value } else { value }
}

/// The row helper (manager `+0x28`, vtable `0x4985b0`): the camera of a
/// played match. `FUN_002f8370` runs it once an update: it assigns the
/// row's height and lens (Street adds its persistent offsets), shakes the
/// height, frames the focus `+0xd0` and the possessing side's rim `+0x100`
/// in screen windows (`FUN_002ff830`), turns those zones into flags, and
/// `FUN_002ff1c8` builds the command record (`FUN_002fb690`, `FUN_002fe780`,
/// `FUN_002fcd50`) that `FUN_002fda08` integrates. The eye does not aim at
/// the focus: the published pose is the integrated pitch and yaw.
/// Traced from the ELF and checked one update at a time against a played
/// match (`$EVIDENCE/street-camera-played-session*.jsonl`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraState {
    /// `+0x1f0`: the row.
    pub mode: usize,
    /// `+0x1f4`: the depth pointer.
    pub depth: RowDepth,
    /// `+0x84..+0x8c`.
    pub eye: [f32; 3],
    /// `+0x90` pitch and `+0x94` yaw.
    pub angles: [f32; 2],
    /// `+0x9c` x, `+0xa4` depth, `+0xa8` pitch and `+0xac` yaw rates, per
    /// update.
    pub velocity: [f32; 4],
    /// `+0x98`: the lens scalar.
    pub scalar: f32,
    /// Clear when a selection (`FUN_002fa4f8`) owes a settle
    /// (`FUN_002fabe8`) at the next update.
    pub initialized: bool,
    /// `+0x1fc..+0x208`: the eye height's shake (`FUN_002f8338`).
    pub shake: RowShake,
    /// `+0x12c`: a depth change under way: 1 or -1 while the depth steps
    /// at `DAT_004d8bd0` × (low - high depth) (rows 0 and 1), 0 once it
    /// arrives.
    pub transition: i32,
    /// `+0x15c`: the camera has come to rest (`FUN_002ff1c8`); a resting
    /// Street or Wide camera holds its x and yaw until the focus has
    /// moved `DAT_00400b14` (125) pixels across the screen (`+0x160`).
    pub still: bool,
    /// `+0x160`: that screen drift, pixels.
    pub drift: f32,
    /// `+0x164`: the framing yaw offset (Street and Wide, a held ball on
    /// the far side near the basket), radians; only the projection reads
    /// it, not the rendered view.
    pub yaw_offset: f32,
    /// `+0x148`: commands and rates all below 0.001 (the settle's exit).
    pub settled: bool,
    /// `+0x1f8`: the possession at the last live update.
    pub last_possession: i32,
    /// The globals the row keeps: Street's height and lens offsets
    /// (`DAT_00400b58`, `DAT_00400b5c`) and the depth scales
    /// (`DAT_004d8d04`, `DAT_004d8d08`, `DAT_004d8d0c`).
    pub offsets: StreetOffsets,
}

/// `+0x1f4`: which of the row's depth offsets the camera tracks. Rows 0
/// and 1 (Street and Court) move it in `FUN_002fb690`; the others keep the
/// base.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RowDepth {
    /// Row word `+0x10`.
    #[default]
    Base,
    /// Row word `+0x14` (`DAT_0049848c` for Street): an owner dunking,
    /// laying up or flying for an alley-oop, a pass to a flier, or a
    /// gamebreaker shot. Street lowers the eye toward 4 and the lens
    /// toward 0.18.
    Low,
    /// Row word `+0x18` (`UNK_00498490`): the possessing side has a flier
    /// in the air or a computer carrier while a person plays it
    /// (`FUN_00300b08`). Street raises the eye toward 18.
    High,
    /// `DAT_00401080` (`DAT_00400b20`, -30): a shot launched within 38.75
    /// of the basket (every row but Court, `DAT_00400b1c` set). Street
    /// eases the lens toward 0.25.
    Shot,
}

/// Globals `FUN_002f8370` and `FUN_002fb690` keep for the row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreetOffsets {
    /// `DAT_00400b58`: added to Street's eye height.
    pub height: f32,
    /// `DAT_00400b5c`: added to Street's lens.
    pub lens: f32,
    /// `DAT_004d8d04`: the depth command's scale for a pass to a flier
    /// (only read with `DAT_00400b48`, which is 0).
    pub pass_depth_scale: f32,
    /// `DAT_004d8d08`: the depth command's scale while the shot depth
    /// moves.
    pub shot_depth_scale: f32,
    /// `DAT_004d8d0c`: halves the yaw with the pass scale.
    pub pass_yaw_half: bool,
}

/// What `FUN_002f8370` reads from the game for one row update, after
/// `FUN_003003f8` has refreshed the focus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowInputs {
    /// `+0xd0`: the clamped focus.
    pub focus: Vec3,
    /// `+0xf0`: its motion this update.
    pub motion: Vec3,
    /// `+0x100`: the rim the possession attacks, `(side × 36.75, 12, 0)`.
    pub reference: Vec3,
    /// `FUN_00305428`'s margin: the owner's height (`actor+0x260`, feet)
    /// times 1.2, 0 for a free ball. Zones test the focus raised by it.
    pub margin: f32,
    /// A free ball on the default focus: `FUN_002fe780` aims at the ball.
    pub free_ball: Option<Vec3>,
    /// `+0x158` with a free ball: the vertical motion is dropped from the
    /// turn (not modelled by the rewrite's source adapter: always false).
    pub vertical_gate: bool,
    /// Game phase 1, the opening toss.
    pub opening: bool,
    /// `game+0x110`, -1 with nobody in possession.
    pub possession: i32,
    /// The possession's side byte (`FUN_001404c0`), 0 for none.
    pub side: f32,
    /// `FUN_00140558`: live play (game phases 2 to 5).
    pub live: bool,
    /// The ball's state `ball+0x180`: 3 held, 4 passed, 5 shot, 6 on the
    /// rim, else loose.
    pub ball_state: u32,
    /// `ball+0x178` on a shot: a gamebreaker shot.
    pub gamebreaker_shot: bool,
    /// `ball+0x154`: the ball's distance from the possession's rim when it
    /// last changed hands or left them (`FUN_00153790`).
    pub ball_distance: f32,
    /// The owner is dunking, laying up or flying for an alley-oop (actor
    /// `+0xa4` bits 7, 13 and 15).
    pub owner_in_air: bool,
    /// A pass to an alley-oop flier (`ball+0x100` with bit 15).
    pub pass_to_flier: bool,
    /// A shot whose shooter stands more than 42 from the possession's rim
    /// (`game+0x374`).
    pub far_shot: bool,
    /// `FUN_00300b08` (a computer carrier on a side a person plays) or a
    /// flier on the possessing side (`FUN_00140920`, bit 15).
    pub team_busy: bool,
}

impl Default for RowInputs {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            motion: Vec3::ZERO,
            reference: Vec3::new(ROW_RIM_X, ROW_RIM_Y, 0.0),
            margin: 0.0,
            free_ball: None,
            vertical_gate: false,
            opening: false,
            possession: 0,
            side: 1.0,
            live: true,
            ball_state: 3,
            gamebreaker_shot: false,
            ball_distance: 0.0,
            owner_in_air: false,
            pass_to_flier: false,
            far_shot: false,
            team_busy: false,
        }
    }
}

/// `FUN_001404e0`: the rim a side attacks, x `side × 36.75`, and the
/// height `FUN_003003f8` gives `+0x100`.
pub const ROW_RIM_X: f32 = 36.75;
pub const ROW_RIM_Y: f32 = 12.0;
/// The row descriptor `0x4983e8`'s framing words: left and right edges and
/// bands (+0x48, +0x4c), the top (+0x58, +0x5c) and the bottom (+0x68,
/// +0x6c), in `FUN_002ff830`'s order; +0x54 and +0x64 are the reference
/// window's side and top bands.
const ROW_WINDOW: [i32; 8] = [100, 80, 100, 80, 75, 80, 30, 80];
const ROW_REFERENCE_SIDE_BAND: i32 = 40;
const ROW_REFERENCE_TOP_BAND: i32 = 40;
/// `DAT_00400b54`: the low depth widens both sides of the windows by it.
const ROW_LOW_WIDEN: i32 = 150;
/// `DAT_004d8d00`: `5280 / (config[+0x28] × 3600) × 3.46` (raw
/// `0x300df8..0x300e28`), the vertical motion that lets the pitch target
/// wait (`FUN_002fe780`).
const ROW_PITCH_MOTION: f32 = 5280.0 / (60.0 * 3600.0) * 3.46;
/// `DAT_00400b14`: the screen drift a resting Street camera waits for.
const ROW_STILL_DRIFT: f32 = 125.0;
/// `DAT_00400b00`, `DAT_00400b0c` and `DAT_00400b10`: the far-side
/// framing yaw offset reaches 0.18 over distances 17.75 to 29.75 from the
/// basket.
const ROW_YAW_OFFSET: f32 = 0.18;
const ROW_YAW_BAND: [f32; 2] = [6.0, 6.0];
/// `DAT_00400b2c`, `DAT_00400b30`: the shot depth scale from 0.6 near the
/// camera's side to 0.125 at the far side.
const ROW_SHOT_SCALE: [f32; 2] = [0.6, 0.125];
/// `DAT_00400b4c`, `DAT_00400b50`: the pass depth scale's ends.
const ROW_PASS_SCALE: [f32; 2] = [0.115, 1.0];
/// `DAT_00400b20` (copied to `DAT_00401080` each update): the shot depth.
const ROW_SHOT_DEPTH: f32 = -30.0;
/// `DAT_00400b24`, `DAT_00400b28`: Street's shot lens and height targets.
const ROW_SHOT_LENS: f32 = 0.25;
const ROW_SHOT_HEIGHT: f32 = 12.0;

/// One update's command record (`FUN_002ff1c8`'s 13 floats): commands
/// 0..3 (x, depth, pitch, yaw), the lens step 4, steps 5..8 and the rate
/// corrections 9..12.
type RowRecord = [f32; 13];

/// `FUN_002ff1c8`'s flag block (its `param_2`).
#[derive(Clone, Copy, Debug)]
struct RowFlags {
    /// +0x10: the first window (focus).
    window: [i32; 8],
    /// +0x30: the focus margin.
    margin: f32,
    /// +0x34: the update flags.
    flags: u32,
    /// +0x38: the focus zones.
    focus_zones: u32,
    /// +0x3c: the reference zones (replaced by the near rim's for a held
    /// ball on Street and Wide).
    reference_zones: u32,
    /// +0x40, +0x44, +0x48: the yaw offset's ramp, recovery and limit.
    yaw_ramp: bool,
    yaw_recover: bool,
    yaw_limit: f32,
}

impl Default for NativeCameraState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl NativeCameraState {
    pub fn new(mode: usize) -> Self {
        let mode = mode.min(CAMERA_MODE_ROWS.len() - 1);
        let row = CAMERA_MODE_ROWS[mode];
        Self {
            mode,
            depth: RowDepth::Base,
            eye: [0.0, row.eye_y, row.focus_z_offset],
            angles: [0.0, 0.0],
            velocity: [0.0; 4],
            scalar: row.row_word7,
            initialized: false,
            shake: RowShake::default(),
            transition: 0,
            still: false,
            drift: 0.0,
            yaw_offset: 0.0,
            settled: false,
            last_possession: 0,
            offsets: StreetOffsets::default(),
        }
    }

    /// The published pose (`+0x84..+0x98`).
    pub fn pose(&self) -> CameraPose {
        CameraPose {
            eye: Vec3::from_array(self.eye),
            pitch: self.angles[0],
            yaw: self.angles[1],
            scalar: self.scalar,
        }
    }

    /// `FUN_002f7bb0`: a new row keeps a Street or Court depth variant
    /// when it is one of them too, and owes a settle.
    pub fn select_mode(&mut self, mode: usize) {
        let mode = mode.min(CAMERA_MODE_ROWS.len() - 1);
        if mode == self.mode {
            return;
        }
        let keeps =
            mode < 2 && self.mode < 2 && matches!(self.depth, RowDepth::Low | RowDepth::High);
        if !keeps {
            self.depth = RowDepth::Base;
        }
        self.mode = mode;
        self.initialized = false;
    }

    /// The depth offset the pointer selects.
    fn depth_offset(&self) -> f32 {
        let row = CAMERA_MODE_ROWS[self.mode];
        match self.depth {
            RowDepth::Base => row.focus_z_offset,
            RowDepth::Low => row.low_depth,
            RowDepth::High => row.high_depth,
            RowDepth::Shot => ROW_SHOT_DEPTH,
        }
    }

    /// One row update (`FUN_002f8370` for rows other than Behind). An
    /// owed settle runs instead of the ordinary update.
    pub fn update(&mut self, input: &RowInputs) {
        self.head(input);
        if self.initialized {
            self.frame(input, false);
        } else {
            self.settle(input);
        }
    }

    /// `FUN_002f8370`'s head: the row's height and lens (Street with its
    /// offsets, which then move toward the depth's targets), the shake and
    /// the live possession's bookkeeping.
    fn head(&mut self, input: &RowInputs) {
        let row = CAMERA_MODE_ROWS[self.mode];
        self.eye[1] = row.eye_y;
        self.scalar = row.row_word7;
        if self.mode == 0 {
            self.street_offsets(row);
        }
        self.eye[1] -= self.shake.next();
        if input.live {
            if self.last_possession != input.possession {
                self.still = false;
                self.drift = 0.0;
            }
            self.last_possession = input.possession;
        }
    }

    /// Street's offsets (`FUN_002f8370`, raw `0x2f86b8..0x2f8960`): applied
    /// first, then moved for the next update; the height offset stops at
    /// zero when it would cross it, the lens offset does not.
    fn street_offsets(&mut self, row: CameraModeRow) {
        let o = &mut self.offsets;
        let lens = camera_add(row.row_word7, o.lens);
        let height = camera_add(row.eye_y, o.height);
        self.scalar = lens;
        self.eye[1] = height;
        let (mut dy, mut dl) = (0.0_f32, 0.0_f32);
        match self.depth {
            RowDepth::Low => {
                if 4.0 < height {
                    dy = -0.16;
                }
                if 0.18 < lens {
                    dl = -0.15 / 50.0;
                }
            }
            RowDepth::High => {
                if height < 18.0 {
                    dy = 0.12;
                }
                if lens < 0.33 {
                    dl = (0.33 - 0.18) / 50.0;
                }
            }
            RowDepth::Shot => {
                if ROW_SHOT_HEIGHT < height {
                    dy = -(12.0 - ROW_SHOT_HEIGHT) / 50.0;
                }
                if ROW_SHOT_LENS < lens {
                    dl = -(0.33 - ROW_SHOT_LENS) / 50.0;
                }
            }
            RowDepth::Base => {
                if height < row.eye_y {
                    dy = (row.eye_y - row.recover_low) / 50.0;
                } else if row.eye_y < height {
                    dy = -(row.recover_high - row.eye_y) / 50.0;
                }
                if lens < row.row_word7 {
                    dl = (row.row_word7 - row.recover_lens) / 50.0;
                }
            }
        }
        let next = o.height + dy;
        o.height = if (o.height > 0.0 && next < 0.0) || (o.height < 0.0 && next > 0.0) {
            0.0
        } else {
            next
        };
        o.lens += dl;
    }

    /// The two framing windows (`FUN_002f8370`): the focus window, and the
    /// reference window with the row's side threshold on the attacking
    /// side and its top threshold. The low depth widens both and drops the
    /// top; a low Street base camera raises the bottom.
    fn windows(&self, input: &RowInputs) -> ([i32; 8], [i32; 8]) {
        let row = CAMERA_MODE_ROWS[self.mode];
        let mut w = ROW_WINDOW;
        let low_street = self.depth == RowDepth::Base && self.mode == 0 && self.eye[1] < 9.333_333;
        match self.depth {
            RowDepth::Low => {
                w[4] += 100;
                w[0] += ROW_LOW_WIDEN;
                w[2] += ROW_LOW_WIDEN;
            }
            _ if low_street => w[6] += 100,
            _ => {}
        }
        let first = w;
        if input.possession != -1 {
            if input.side * ROW_RIM_X > 0.0 {
                w[0] = row.horizontal_screen_threshold;
                w[1] = ROW_REFERENCE_SIDE_BAND;
            } else {
                w[2] = row.horizontal_screen_threshold;
                w[3] = ROW_REFERENCE_SIDE_BAND;
            }
        }
        w[4] = row.top_screen_threshold;
        match self.depth {
            RowDepth::Low => {
                w[4] += 100;
                w[0] += ROW_LOW_WIDEN;
                w[2] += ROW_LOW_WIDEN;
            }
            _ if low_street => w[6] += 100,
            _ => {}
        }
        w[5] = ROW_REFERENCE_TOP_BAND;
        (first, w)
    }

    /// `FUN_002ffac0` with the yaw offset (`FUN_002ff830` always passes 1).
    fn screen(&self, point: Vec3) -> Option<Vec2> {
        framing_projection(
            Vec3::from_array(self.eye),
            self.angles[0],
            // `add.s` (`0x2ffc78`).
            camera_add(self.angles[1], self.yaw_offset),
            self.scalar,
            point,
        )
    }

    /// `FUN_002ff830`: the zones of `point`, its top tests on the point
    /// raised by `margin`.
    fn zones(&self, point: Vec3, margin: f32, window: &[i32; 8]) -> u32 {
        let at = self.screen(point);
        let top = if margin == 0.0 {
            at
        } else {
            // The raise is an `add.s` (`0x2ff89c`).
            self.screen(Vec3::new(point.x, camera_add(point.y, margin), point.z))
                .or(at)
        };
        framing_zones_raised(at, top.map(|t| t.y), window)
    }

    /// The rest of `FUN_002f8370`: zones, flags and, unless `holding` (the
    /// settle's first update, `+0x144`), the controller.
    fn frame(&mut self, input: &RowInputs, holding: bool) {
        let (first, second) = self.windows(input);
        let focus_zones = self.zones(input.focus, input.margin, &first);
        let reference_zones = self.zones(input.reference, 0.0, &second);
        let wide = matches!(self.mode, 0 | 5);
        let held = input.ball_state == 3;
        let mut block = RowFlags {
            window: first,
            margin: input.margin,
            flags: 0,
            focus_zones,
            reference_zones,
            yaw_ramp: false,
            yaw_recover: true,
            yaw_limit: 0.0,
        };
        if wide && !held {
            self.drift = 0.0;
            self.yaw_offset = 0.0;
            self.still = false;
        }
        // The near rim (raw 0x2f91bc..0x2f9338): Street and Wide with a
        // held ball frame the basket on the focus's side; on the far side
        // near the basket the yaw offset may ramp toward it.
        let near_rim = Vec3::new(
            if input.focus.x < 0.0 {
                -ROW_RIM_X
            } else {
                ROW_RIM_X
            },
            ROW_RIM_Y,
            0.0,
        );
        if wide && held {
            block.reference_zones = self.zones(near_rim, 0.0, &second);
            if input.focus.z > 0.0 {
                let dx = near_rim.x - input.focus.x;
                let distance = (dx * dx + input.focus.z * input.focus.z).sqrt();
                let near = 23.75 - ROW_YAW_BAND[0];
                if input.focus.x.abs() < 23.0
                    && near < distance
                    && distance < ROW_YAW_BAND[1] + 23.75
                {
                    if reference_zones == block.reference_zones
                        && block.reference_zones & 0x300 != 0
                    {
                        self.drift = 0.0;
                        block.yaw_ramp = true;
                        self.still = false;
                    }
                    block.yaw_recover = false;
                    block.yaw_limit =
                        ROW_YAW_OFFSET * (distance - near) / (ROW_YAW_BAND[0] + ROW_YAW_BAND[1]);
                }
            }
        } else if self.mode != 4 && reference_zones & 0x7c00 != 0 {
            block.reference_zones = self.zones(near_rim, 0.0, &second);
        }
        block.flags = row_flags(input, focus_zones, reference_zones, block.reference_zones);
        if !holding {
            self.controller(input, &mut block);
        }
    }

    /// `FUN_002ff1c8` for the row: the record, `FUN_002fe780`'s pitch
    /// target, `FUN_002fcd50`, `FUN_002fda08`, and the rest test.
    fn controller(&mut self, input: &RowInputs, block: &mut RowFlags) {
        let fps = 60.0_f32;
        let mut rec: RowRecord = [0.0; 13];
        rec[7] = native_camera_atan(self.scalar) / fps * 0.5;
        rec[8] = rec[7] * (4.0 / 3.0);
        rec[5] = (42.0 / fps) * 0.5;
        rec[6] = (26.5 / fps) * 0.5;
        self.commands(input, &mut rec);
        let raised = input.focus + Vec3::Y * block.margin;
        let mut lift = false;
        if !(block.focus_zones & 4 != 0 && block.reference_zones & 4 != 0) {
            lift = if block.focus_zones & 0x44 != 0 {
                true
            } else {
                block.flags & 8 != 0
            };
        }
        let raise = lift
            && match (self.screen(raised), self.screen(input.reference)) {
                (Some(a), Some(b)) => a.y < b.y,
                _ => false,
            };
        let accel = self.pitch_target(input, block, &mut rec, raise);
        self.apply_flags(block, &mut rec);
        if accel {
            rec[11] = (rec[2] - self.velocity[2]) * 0.125;
        }
        self.integrate(input, block, &mut rec);
        let small = |v: f32| -0.001 < v && v < 0.001;
        self.settled = [
            rec[3],
            rec[4],
            rec[0],
            rec[1],
            self.velocity[0],
            self.velocity[1],
            self.velocity[3],
        ]
        .into_iter()
        .all(small);
        if self.settled && !self.still {
            self.drift = 0.0;
            self.still = true;
        }
    }

    /// `FUN_002fb690` for state `0x12`: Street and Court move the depth
    /// pointer; every row then steps its depth toward
    /// `focus.z × scale / 26.5 + depth` and its x toward `focus.x + side ×
    /// offset` within the bound; `FUN_002face8` turns to keep the focus.
    fn commands(&mut self, input: &RowInputs, rec: &mut RowRecord) {
        let row = CAMERA_MODE_ROWS[self.mode];
        if self.mode <= 1 {
            self.move_depth(input);
        }
        let mut lift = 0.0_f32;
        if self.mode == 0 {
            let y = self.eye[1];
            lift = match self.depth {
                RowDepth::Low if 4.0 < y => -(12.0 - 4.0) / 50.0,
                RowDepth::High if y < 18.0 => 6.0 / 50.0,
                RowDepth::Base | RowDepth::Shot if y < 12.0 => 8.0 / 50.0,
                RowDepth::Base | RowDepth::Shot if 12.0 < y => -(18.0 - 12.0) / 50.0,
                _ => 0.0,
            };
        }
        let step = if self.mode < 2 && self.transition != 0 {
            (1.0 / (60.0 * 0.75)) * (row.low_depth - row.high_depth)
        } else {
            rec[6].max(input.motion.z.abs())
        };
        let z = self.eye[2];
        let target = input.focus.z * (row.focus_z_scale / 26.5) + self.depth_offset();
        if z < target || target < z {
            let gap = (target - z).abs();
            let delta = if gap <= step {
                self.transition = 0;
                gap
            } else {
                step
            };
            rec[1] += if z < target { delta } else { -delta };
        } else {
            self.transition = 0;
        }
        let step = input.motion.x.abs().max(rec[5]);
        if input.side == 1.0 || input.side == -1.0 {
            let target = input.focus.x + input.side * row.x_offset;
            let x = self.eye[0];
            if x != target {
                let delta = (target - x).abs().min(step);
                rec[0] += if x < target { delta } else { -delta };
            }
        }
        let next = self.eye[0] + rec[0];
        if row.x_bound < next {
            rec[0] = row.x_bound - self.eye[0];
        } else if next < -row.x_bound {
            rec[0] = -row.x_bound - self.eye[0];
        }
        if input.motion.x == 0.0 {
            rec[0] = 0.0;
        }
        if self.transition == 0 && input.motion.z == 0.0 {
            rec[1] = 0.0;
        }
        if self.mode == 0 && input.far_shot {
            rec[0] = 0.0;
        }
        let mut motion = input.motion;
        if input.free_ball.is_some() && input.vertical_gate {
            motion.y = 0.0;
        }
        let (pitch, yaw) = face_turns(
            Vec3::from_array(self.eye),
            input.focus,
            motion,
            Vec3::new(rec[0], lift, rec[1]),
        );
        rec[2] = pitch;
        rec[3] = yaw;
    }

    /// `FUN_002fb690`'s depth pointer for Street and Court.
    fn move_depth(&mut self, input: &RowInputs) {
        let shot = input.ball_state == 5;
        let away = matches!(input.ball_state, 5 | 6);
        let lift = (shot && input.gamebreaker_shot) || input.owner_in_air || input.pass_to_flier;
        let to_low = |s: &mut Self| {
            s.transition = -1;
            s.depth = RowDepth::Low;
            if input.pass_to_flier {
                let o = &mut s.offsets;
                o.pass_depth_scale = ROW_PASS_SCALE[0]
                    + (ROW_PASS_SCALE[1] - ROW_PASS_SCALE[0]) * (input.focus.z + 26.5) / 53.0;
                o.pass_yaw_half = input.focus.z > -10.0 && input.ball_distance > 17.75;
            } else {
                s.offsets.pass_depth_scale = 0.0;
                s.offsets.pass_yaw_half = false;
            }
        };
        match self.depth {
            RowDepth::Low => {
                let back = if shot && input.gamebreaker_shot {
                    false
                } else if input.ball_state == 4 {
                    !input.pass_to_flier
                } else if input.owner_in_air {
                    false
                } else {
                    !away
                };
                if back {
                    self.transition = 1;
                    self.depth = RowDepth::Base;
                }
            }
            RowDepth::Base => {
                if lift {
                    to_low(self);
                } else if self.mode != 1 && shot && input.ball_distance < 38.75 {
                    self.depth = RowDepth::Shot;
                    self.offsets.shot_depth_scale = ROW_SHOT_SCALE[0]
                        - (ROW_SHOT_SCALE[0] - ROW_SHOT_SCALE[1]) * (input.focus.z + 26.5) / 53.0;
                    self.transition = -1;
                } else if input.possession != -1 && input.team_busy {
                    self.transition = 1;
                    self.depth = RowDepth::High;
                }
            }
            RowDepth::Shot if self.mode != 1 => {
                if away {
                    self.transition = -1;
                } else {
                    self.depth = RowDepth::Base;
                    self.transition = 1;
                }
            }
            RowDepth::High | RowDepth::Shot => {
                if lift {
                    to_low(self);
                } else if input.possession != -1 && !input.team_busy {
                    self.transition = -1;
                    self.depth = RowDepth::Base;
                }
            }
        }
    }

    /// `FUN_002fe780`: when the raised focus (or a free ball) sits away
    /// from the pitch that would put it in the middle of the top band, a
    /// pitch flag and a step growing with the squared screen error.
    fn pitch_target(
        &self,
        input: &RowInputs,
        block: &mut RowFlags,
        rec: &mut RowRecord,
        raise: bool,
    ) -> bool {
        let point = input
            .free_ball
            .unwrap_or(input.focus + Vec3::Y * block.margin);
        let eye = Vec3::from_array(self.eye);
        let (to, _) = native_angles((point - eye).to_array());
        let a = native_camera_atan(self.scalar);
        let middle = block.window[4] as f32 + block.window[5] as f32 * 0.5;
        let target = (to + a * 0.5) - a * 0.5 * (middle / 240.0);
        let pitch = self.angles[0];
        let rising = input.motion.y;
        let below = if block.focus_zones & 4 != 0 && block.reference_zones & 4 != 0 {
            if target < pitch && rising < ROW_PITCH_MOTION {
                return false;
            }
            if pitch < target && -ROW_PITCH_MOTION < rising {
                return false;
            }
            target < pitch
        } else if block.focus_zones & 0x40 == 0 {
            if pitch <= target {
                false
            } else if rising <= 0.0 {
                return false;
            } else {
                true
            }
        } else {
            target < pitch
        };
        if !(below || raise) {
            return false;
        }
        let y = self.screen(point).map_or(0.0, |at| at.y);
        let error = middle - y;
        if error.abs() < 5.0 {
            return false;
        }
        if error > 0.0 {
            if block.flags & 8 != 0 {
                return false;
            }
            block.flags = (block.flags & !0x20) | 4;
        } else {
            if block.flags & 4 != 0 {
                return false;
            }
            block.flags = (block.flags & !0x20) | 8;
        }
        let step = (a / (60.0 * 0.167)) * (error / 240.0) * (error / 240.0);
        if rec[7] < step {
            rec[7] = step;
        }
        true
    }

    /// `FUN_002fcd50` for state `0x12` (rows other than Behind): the flags
    /// turn the camera by the steps, an off-screen focus turns it harder,
    /// and the corrections are a quarter of each gap (an eighth for a turn
    /// that keeps its direction, unless the depth is still moving).
    fn apply_flags(&mut self, block: &RowFlags, rec: &mut RowRecord) {
        let flags = block.flags;
        if flags & 0x10 != 0 && self.transition == 0 {
            rec[3] = 0.0;
        }
        if flags & 0x20 != 0 && self.transition == 0 {
            rec[2] = 0.0;
        }
        if flags & 1 != 0 {
            rec[3] += rec[8];
        } else if flags & 2 != 0 {
            rec[3] -= rec[8];
        }
        if flags & 4 != 0 {
            rec[2] -= rec[7];
        } else if flags & 8 != 0 {
            rec[2] += rec[7];
        }
        let quarter = |command: f32, rate: f32| (command - rate) * 0.25;
        if block.focus_zones & 0x7c00 != 0 {
            // Off screen (FUN_00340010/FUN_0033f588: the pitch times 1.25
            // in double precision).
            rec[2] = (rec[2] as f64 * 1.25) as f32;
            rec[3] *= 1.125;
            rec[9] = quarter(rec[0], self.velocity[0]);
            rec[10] = quarter(rec[1], self.velocity[1]);
            rec[12] = quarter(rec[3], self.velocity[3]);
            rec[11] = quarter(rec[2], self.velocity[2]);
            return;
        }
        rec[9] = quarter(rec[0], self.velocity[0]);
        rec[10] = quarter(rec[1], self.velocity[1]);
        rec[12] = quarter(rec[3], self.velocity[3]);
        rec[11] = quarter(rec[2], self.velocity[2]);
        if self.transition != 0 {
            if self.mode != 1 && self.depth == RowDepth::Shot {
                rec[1] *= self.offsets.shot_depth_scale;
            }
            // The pass scale needs DAT_00400b48, which is 0.
            return;
        }
        let same = |rate: f32, correction: f32| (rate >= 0.0) == (correction >= 0.0) || rate == 0.0;
        if same(self.velocity[3], rec[12]) {
            rec[12] *= 0.5;
        }
        if same(self.velocity[2], rec[11]) {
            rec[11] *= 0.5;
        }
    }

    /// `FUN_002fda08` for state `0x12` with one substep: a resting Street
    /// or Wide camera damps its x and yaw by the screen drift, the rates
    /// step toward the commands, the yaw offset ramps or recovers by 0.03,
    /// and the pose integrates within the row's x bound.
    fn integrate(&mut self, input: &RowInputs, block: &mut RowFlags, rec: &mut RowRecord) {
        let wide = matches!(self.mode, 0 | 5);
        if self.depth != RowDepth::Base {
            block.yaw_ramp = false;
            block.yaw_recover = true;
        }
        if wide
            && self.depth != RowDepth::Low
            && self.still
            && !(rec[3] == 0.0 && rec[0] == 0.0 && rec[1] == 0.0)
        {
            let moved = match (
                self.screen(input.focus),
                self.screen(input.focus + input.motion),
            ) {
                (Some(a), Some(b)) => b.x - a.x,
                _ => 0.0,
            };
            self.drift += moved;
            if self.drift.abs() < ROW_STILL_DRIFT {
                let k = (self.drift / ROW_STILL_DRIFT) * (self.drift / ROW_STILL_DRIFT);
                let k = k * k;
                rec[3] *= k;
                rec[0] *= k;
            } else {
                self.drift = ROW_STILL_DRIFT.copysign(self.drift);
            }
        }
        self.velocity[3] = velocity_step(self.velocity[3], rec[3], rec[12], true);
        self.velocity[2] = velocity_step(self.velocity[2], rec[2], rec[11], false);
        self.velocity[0] = velocity_step(self.velocity[0], rec[0], rec[9], false);
        self.velocity[1] = velocity_step(self.velocity[1], rec[1], rec[10], false);
        if wide {
            if block.yaw_ramp {
                if block.reference_zones & 0x200 != 0 {
                    self.yaw_offset = (self.yaw_offset + 0.03).min(block.yaw_limit);
                } else {
                    self.yaw_offset = (self.yaw_offset - 0.03).max(-block.yaw_limit);
                }
            }
            if block.yaw_recover && self.yaw_offset != 0.0 {
                if self.yaw_offset < 0.0 {
                    self.yaw_offset = (self.yaw_offset + 0.03).min(0.0);
                }
                if self.yaw_offset > 0.0 {
                    self.yaw_offset = (self.yaw_offset - 0.03).max(0.0);
                }
            }
        }
        self.angles[1] = camera_add(self.angles[1], self.velocity[3]);
        self.angles[0] = camera_add(self.angles[0], self.velocity[2]);
        self.eye[0] = camera_add(self.eye[0], self.velocity[0]);
        self.eye[2] = camera_add(self.eye[2], self.velocity[1]);
        let bound = CAMERA_MODE_ROWS[self.mode].x_bound;
        if self.eye[0] > bound || self.eye[0] < -bound {
            self.eye[0] = self.eye[0].clamp(-bound, bound);
            rec[0] = 0.0;
            rec[9] = 0.0;
            self.velocity[0] = 0.0;
        }
        self.scalar += rec[4];
    }

    /// `FUN_002fabe8` on the row: `FUN_002f7c38`'s pose aimed at the focus,
    /// one update holding the controller (`+0x144`), the rates zeroed, then
    /// up to 501 updates with the world standing still until the commands
    /// and rates fall below 0.001. The camera comes to rest with the focus
    /// framed, not centred.
    fn settle(&mut self, input: &RowInputs) {
        let (eye, at) = mode_camera_pose(input.focus, input.side, self.mode);
        let row = CAMERA_MODE_ROWS[self.mode];
        self.eye = eye.to_array();
        self.eye[1] = row.eye_y;
        self.scalar = row.row_word7;
        let (pitch, yaw) = native_angles((at - eye).to_array());
        self.angles = [pitch, yaw];
        self.still = false;
        self.drift = 0.0;
        self.yaw_offset = 0.0;
        self.initialized = true;
        // Re-reading a world that has not moved gives no motion.
        let still = RowInputs {
            motion: Vec3::ZERO,
            ..*input
        };
        self.head(&still);
        self.frame(&still, true);
        self.velocity = [0.0; 4];
        self.settled = false;
        for _ in 0..501 {
            self.head(&still);
            self.frame(&still, false);
            if self.settled {
                break;
            }
        }
    }
}

/// `FUN_002f8370`'s flags for rows other than Behind: yaw toward a focus
/// past a side edge, else toward the reference past an edge or away from
/// the attacking side (`0x10` holds the yaw); pitch toward the top band
/// (`4` up, `8` down, `0x20` holds the pitch); always `0x40`.
fn row_flags(input: &RowInputs, focus: u32, reference: u32, alternate: u32) -> u32 {
    let mut flags = if focus & 0x100 != 0 {
        1
    } else if focus & 0x200 != 0 {
        2
    } else if input.opening {
        0
    } else if reference & 0x2200 != 0 || (reference & 0x4000 != 0 && input.reference.x < 0.0) {
        if focus & 0x10 == 0 { 2 } else { 0 }
    } else if reference & 0x1100 != 0 || (reference & 0x4000 != 0 && 0.0 < input.reference.x) {
        if focus & 0x20 == 0 { 1 } else { 0 }
    } else {
        let side = input.side * ROW_RIM_X;
        if side < 0.0 {
            if reference & 0x20 == 0 && focus & 0x20 == 0 {
                0x11
            } else {
                0x10
            }
        } else if side > 0.0 {
            if reference & 0x10 != 0 || focus & 0x10 != 0 {
                0x10
            } else {
                0x12
            }
        } else {
            0
        }
    };
    let pitch = if focus & 0x80 != 0 {
        8
    } else if input.opening {
        if focus & 0x48 != 0 {
            0
        } else if alternate & 0x40 != 0 {
            4
        } else if alternate & 4 == 0 {
            8
        } else {
            0x20
        }
    } else if alternate & 0x40 != 0 {
        if focus & 8 == 0 { 4 } else { 0 }
    } else if focus & 0x40 != 0 {
        4
    } else if alternate & 4 == 0 && focus & 4 == 0 {
        0x28
    } else {
        0x20
    };
    flags |= pitch;
    flags | 0x40
}

/// `FUN_002face8`: the pitch and yaw turns that keep `focus` where it is
/// on screen after it moves by `motion` while the eye moves by `command`.
fn face_turns(eye: Vec3, focus: Vec3, motion: Vec3, command: Vec3) -> (f32, f32) {
    let now = focus - eye;
    let (pitch, yaw) = native_angles(now.to_array());
    let (next_pitch, next_yaw) = native_angles((now + motion - command).to_array());
    (
        wrap_camera_angle(next_pitch - pitch),
        wrap_camera_angle(next_yaw - yaw),
    )
}

/// The four camera helpers `FUN_003038b0` builds; `DAT_00402020` names the
/// one on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraHelperKind {
    /// Manager `+0x24` (`FUN_00300e90`, vtable `0x498848`): authored script
    /// cameras, selected when a script starts (`FUN_00303370`).
    Staged,
    /// Manager `+0x28` (`FUN_002f7b00`, vtable `0x4985b0`): the six Pause rows.
    Row,
    /// Manager `+0x2c` (`FUN_003017c8`, vtable `0x498950`): a fixed eye whose
    /// lens holds the subject's size (`FUN_00301fa8`).
    Positioned,
    /// Manager `+0x30` (`FUN_003057a0`, vtable `0x498dd0`): a counted linear
    /// blend between two helpers' poses (`FUN_00305840` / `FUN_003058c8`).
    Blend,
}

/// `FUN_003a16b8`: six 32-bit words added down the chain with carries, the
/// last word counting up. The camera's placement draws use it.
#[derive(Debug, Clone)]
pub struct NativeWideRandom([u32; 6]);

impl Default for NativeWideRandom {
    fn default() -> Self {
        Self(Self::ELF_STATE)
    }
}

impl NativeWideRandom {
    /// The words at `0x4309f0` in the ELF, which the seeding routine at
    /// `0x3a1790` produces for seed zero. The original shares this generator
    /// with every other caller, so its in-match sequence is not reproduced.
    pub const ELF_STATE: [u32; 6] = [
        0xf22d_0e56,
        0x8831_26e9,
        0xc624_dd2f,
        0x0702_c49c,
        0x9e35_3f7d,
        0x6fdf_3b64,
    ];

    /// One draw, as the raw instructions at `0x3a16d8..0x3a1788` add.
    pub fn draw(&mut self) -> u32 {
        let [mut t0, mut t1, mut t2, mut t3, mut t4, mut t5] = self.0;
        t4 = t4.wrapping_add(t5);
        let mut carry = u32::from(t4 < t5);
        let add = |word: &mut u32, below: u32, carry: &mut u32| {
            let sum = word.wrapping_add(below);
            let first = u32::from(sum < below);
            *word = sum.wrapping_add(*carry);
            *carry = u32::from(*word < *carry) + first;
        };
        add(&mut t3, t4, &mut carry);
        add(&mut t2, t3, &mut carry);
        add(&mut t1, t2, &mut carry);
        t0 = t0.wrapping_add(t1).wrapping_add(carry);
        t5 = t5.wrapping_add(1);
        if t5 == 0 {
            t4 = t4.wrapping_add(1);
            if t4 == 0 {
                t3 = t3.wrapping_add(1);
                if t3 == 0 {
                    t2 = t2.wrapping_add(1);
                    if t2 == 0 {
                        t1 = t1.wrapping_add(1);
                        if t1 == 0 {
                            t0 = t0.wrapping_add(1);
                        }
                    }
                }
            }
        }
        self.0 = [t0, t1, t2, t3, t4, t5];
        t0
    }

    /// The callers' `(draw & 0x7fffffff) * 2^-31` (`0x30000000`).
    fn unit(&mut self) -> f32 {
        (self.draw() & 0x7fff_ffff) as f32 * f32::from_bits(0x3000_0000)
    }
}

/// A helper's focus descriptor (`+0x180`, set by `FUN_002fa210`): what
/// `FUN_003052a0` reads into `+0xc0` and `FUN_00305360` takes the source
/// vector from.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CameraFocus {
    /// Type 3 (`FUN_00305280`): the ball's owner, else the ball.
    #[default]
    Default,
    /// Type 2 (`FUN_00305258`): an actor's position `+0x30`.
    Actor(usize),
    /// Type 0 (`FUN_00305220`): a fixed point, with the global zero vector
    /// `0x4d9090` for its motion.
    Point(Vec3),
}

/// The referee's bones 21 and 10 from his root at the intro cut of the
/// retained attract capture (`street-native-full-rig-active.json` sample
/// 490: root (-0.9, 0, -3.0)). The camera reads these rather than the
/// drawn referee's rig (`crate::referee`), which it is not given.
pub const REFEREE_UPPER_BODY: [[f32; 3]; 2] = [[-0.067, 5.892, 0.448], [-0.014, 4.448, 0.067]];

/// The source entity the stationary gate compares for the referee, who
/// holds the ball through the opening toss (he is not one of the
/// simulation's players).
pub const REFEREE_ENTITY: usize = usize::MAX;

/// `FUN_003003f8`'s per-update inputs for a focus descriptor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSourceSample {
    /// The raw focus that `FUN_003052a0` copies to `+0xc0`.
    pub focus: Vec3,
    /// `FUN_003052a0`'s result (`+0x14c`): true on the ownerless-ball path.
    pub ownerless: bool,
    /// The actor whose identity the stationary gate compares (global
    /// `0x401098`); none for a free ball.
    pub entity: Option<usize>,
    /// `FUN_00305360`'s source vector (`object+0x40`), court units per
    /// update: a free ball's velocity times the tick (`FUN_0014d1f8`), an
    /// actor's physical vector as the simulation moves him
    /// ([`street_sim::World::actor_motion`]: his flight on a jump's script,
    /// none on a trick's or a reaction's, else his locomotion's).
    pub source: Vec3,
    /// The actor's script bits inside the mask `0x7ffdc` (actor `+0xa4`) as
    /// the simulation's actions show them
    /// ([`street_sim::World::actor_state_bits`]).
    pub entity_busy: bool,
    /// `FUN_00300b98`: channel 0 plays one of the stopping clips (records
    /// 46, 47, 174..177: `RunToStandDribble_L`, `jogdribblestop02_r`,
    /// `jogdribblestop_r`), which keeps the stationary gate off.
    pub stop_clip: bool,
    /// The ball's resting height (`ball+0x11c`), which the vertical gate
    /// tests against.
    pub rest: f32,
    /// The ball's state as it leaves its owner (`ball+0x180`): 5 a shot, 4
    /// a pass, anything else 0. Read on the update the flag turns on.
    pub release: u8,
}

/// `FUN_00300b98`'s stopping clips (channel 0's record `+0x50`).
pub const CAMERA_STOP_CLIPS: [u16; 6] = [46, 47, 174, 175, 176, 177];

impl CameraSourceSample {
    /// The default (type 3) focus.
    pub fn from_world(world: &street_sim::World) -> Self {
        Self::for_focus(world, CameraFocus::Default)
    }

    /// `FUN_003052a0` and `FUN_00305360` for one descriptor.
    pub fn for_focus(world: &street_sim::World, focus: CameraFocus) -> Self {
        let ball = &world.ball;
        let rest = ball.radius;
        let release = match ball.state {
            street_sim::BallState::Shot => 5,
            street_sim::BallState::Pass => 4,
            _ => 0,
        };
        let actor = |i: usize| {
            world.players.get(i).map(|player| Self {
                focus: Vec3::new(player.position[0], player.height, player.position[1]),
                ownerless: false,
                entity: Some(i),
                source: Vec3::from_array(world.actor_motion(i)),
                entity_busy: world.actor_state_bits(i) & 0x7ffdc != 0,
                stop_clip: player.action == Action::None
                    && CAMERA_STOP_CLIPS.contains(&player.gait_channel.row),
                rest,
                release,
            })
        };
        match focus {
            CameraFocus::Point(point) => {
                return Self {
                    focus: point,
                    ownerless: false,
                    entity: None,
                    source: Vec3::ZERO,
                    entity_busy: false,
                    stop_clip: false,
                    rest,
                    release,
                };
            }
            CameraFocus::Actor(i) => {
                if let Some(sample) = actor(i) {
                    return Self {
                        ownerless: false,
                        ..sample
                    };
                }
            }
            CameraFocus::Default => {}
        }
        // The referee holds the ball until his toss (`FUN_00141fd8` gives it
        // to him): the owner path reads his position, which stays put.
        let mut sample = if let street_sim::OpeningToss::Waiting(_) = world.opening_toss {
            Self {
                focus: Vec3::from_array(street_sim::toss::REFEREE),
                ownerless: false,
                entity: Some(REFEREE_ENTITY),
                source: Vec3::ZERO,
                entity_busy: false,
                stop_clip: false,
                rest,
                release,
            }
        } else if let street_sim::BallState::Held(owner) = ball.state
            && let Some(sample) = actor(owner)
        {
            sample
        } else {
            Self {
                focus: Vec3::from_array(ball.position),
                ownerless: true,
                entity: None,
                source: Vec3::from_array(ball.velocity) * street_sim::TICK_SECONDS,
                entity_busy: false,
                stop_clip: false,
                rest,
                release,
            }
        };
        // Game phase 1 (raw `0x300600..0x300618`): the default focus's x
        // and z are zeroed, so the opening frames the centre spot at the
        // referee's, then the ball's, height. The retained attract capture
        // has `+0xc0` at the origin through the midcourt cut.
        if world.opening_toss != street_sim::OpeningToss::None {
            sample.focus.x = 0.0;
            sample.focus.z = 0.0;
        }
        sample
    }
}

/// `5280 / (config[+0x28] * 3600) * 250` with the default 60: the largest
/// per-update focus difference `FUN_003003f8` keeps before it substitutes
/// the source vector (about 6.11 court units).
fn camera_source_jump_limit() -> f32 {
    5280.0 / (60.0_f32 * 3600.0) * 250.0
}

/// The native per-helper focus history `FUN_003003f8` keeps: raw `+0xc0`,
/// clamped `+0xd0`, motion `+0xf0`, the ownerless flag `+0x14c` and the
/// vertical gate `+0x158`/`+0x1b4`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CameraSourceState {
    /// `+0xc0`: the next update's difference reference (unclamped).
    pub raw: Option<Vec3>,
    /// `+0xd0`: `+0xc0` with z floored at -26.5.
    pub clamped: Vec3,
    /// `+0xf0`: source motion, court units per update.
    pub motion: Vec3,
    /// `+0x14c`.
    pub ownerless: bool,
    /// The row helper's copy (`FUN_003003f8`'s second argument 0): only it
    /// sets the vertical gate.
    pub row: bool,
    /// `+0x158`: a loose ball came up off the floor; its focus height
    /// reads 0 while it stays ownerless.
    pub vertical: bool,
    /// `+0x1b4`: the ball's last source rise while ownerless.
    pub last_rise: f32,
}

impl CameraSourceState {
    /// The row helper's history.
    pub fn row() -> Self {
        Self {
            row: true,
            ..Self::default()
        }
    }

    /// One update with a count of one. `settling` stands for `+0x13c` during
    /// a (re)selection, which skips the stationary gate and the substitution.
    /// `last_entity` is the shared global `0x401098`.
    pub fn update(
        &mut self,
        sample: &CameraSourceSample,
        settling: bool,
        last_entity: &mut Option<usize>,
    ) {
        let old = self.raw;
        // The flag is compared, not the owner (FUN_003052a0's boolean).
        let changed = old.is_none() || self.ownerless != sample.ownerless;
        let mut focus = sample.focus;
        if sample.ownerless {
            // 0x300520..0x300560: the ball leaving its owner as a shot (5)
            // or a pass (4) clears the gate (after FUN_00300150 or
            // FUN_003000b0); a ball knocked loose keeps it.
            if !self.ownerless && matches!(sample.release, 4 | 5) {
                self.vertical = false;
                self.last_rise = 0.0;
            }
            // 0x300564..0x3005c8: within 0.25 of its resting height, its
            // rise turning from falling to not sets it (the row's copy).
            if self.row
                && focus.y < sample.rest + 0.25
                && self.last_rise < 0.0
                && sample.source.y >= 0.0
            {
                self.vertical = true;
            }
            self.last_rise = sample.source.y;
            // 0x30061c..0x30062c: while it is set the focus has no height.
            if self.vertical {
                focus.y = 0.0;
            }
        }
        let stationary = !settling
            && !changed
            && sample.entity.is_some()
            && sample.entity == *last_entity
            && !sample.entity_busy
            && sample.source == Vec3::ZERO
            && !sample.stop_clip;
        if stationary {
            // 0x3007a4..0x3007b8: zero motion and keep the previous focus.
            self.motion = Vec3::ZERO;
            self.raw = Some(self.clamped);
        } else {
            self.motion = match old {
                Some(old) if !changed => focus - old,
                _ => Vec3::ZERO,
            };
            self.raw = Some(focus);
            // 0x300844..0x3008a0: a changed flag or a jump takes the source vector.
            if !settling && (changed || self.motion.length() > camera_source_jump_limit()) {
                self.motion = sample.source;
            }
        }
        *last_entity = sample.entity;
        self.ownerless = sample.ownerless;
        // 0x30091c..0x30094c: copy to +0xd0, floor z and drop the z motion.
        self.clamped = self.raw.unwrap_or(focus);
        if self.clamped.z < -26.5 {
            self.clamped.z = -26.5;
            self.motion.z = 0.0;
        }
    }
}

/// What the staged helper shows (`0x498848`, whose pose callback
/// `0x3010f8` aims from the eye at the point; its update `0x301598` is
/// empty): one of the script camera nodes `FUN_00331d08` fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StagedView {
    pub eye: Vec3,
    pub at: Vec3,
    /// `+0x98`, the lens scalar the renderer's projection reads like any
    /// helper's (zoom 1.3, measured in the Street School's Level 2 rerun:
    /// a field of 50 degrees shows 63.74 across).
    pub scalar: f32,
    /// `+0x1b0`, radians: the renderer turns the view by its negative about
    /// the view axis (`FUN_003046c0`'s third `FUN_00377b58`), so a positive
    /// roll leans the picture's up toward its right.
    pub roll: f32,
}

/// `FUN_00331d08`: script camera `camera` (its latest record with the keys
/// applied, as `FUN_00331968` and `FUN_00331a70` set the node) as the
/// staged helper's pose. A following camera (the record's byte `+0x23`,
/// node `+0x43`) aims at `follow(actor, part)`, the script actor its
/// record names (node `+0x40`, -1 the ball) at `part` (`+0x45`: 0 bone 0,
/// 1 bone 21, 2 the floor); its target's x is then a sideways offset (the
/// yaw turned by `asin(x / 10)`, the distance grown to `sqrt(h^2 + x^2)`)
/// and its y a height over the point. The lens is `tan(fov / 2)` for a
/// field of 1..180 degrees (`0x3327f8..0x33295c`), 1 for 0 (the
/// `0x332698` branch: a quarter turn's sine over its cosine) and the last
/// one otherwise; the roll `(heading - 180)` degrees (`0x332960..`).
pub fn staged_pose(
    camera: &street_assets::timeline::Camera,
    follow: impl Fn(i16, u8) -> Vec3,
    last_scalar: f32,
) -> StagedView {
    let eye = Vec3::from_array(camera.position);
    let mut at = Vec3::from_array(camera.target);
    if camera.flags[1] != 0 {
        let point = follow(camera.target_actor, camera.flags[3]);
        let (dx, dz) = (point.x - eye.x, point.z - eye.z);
        let flat = (dx * dx + dz * dz).sqrt();
        let side = camera.target[0];
        let reach = (flat * flat + side * side).sqrt();
        let yaw = dx.atan2(dz) - (side * flat / 10.0 / flat).asin();
        at = Vec3::new(
            eye.x + yaw.sin() * reach,
            point.y + camera.target[1],
            eye.z + yaw.cos() * reach,
        );
    }
    let fov = camera.fov;
    let scalar = if fov == 0.0 {
        1.0
    } else if (1.0..=180.0).contains(&fov) {
        (fov * 0.017_453_292 * 0.5).tan()
    } else {
        last_scalar
    };
    StagedView {
        eye,
        at,
        scalar,
        roll: (camera.heading - 180.0) * 0.017_453_292,
    }
}

/// A helper's published pose (`+0x84..+0x98`): eye, pitch, yaw and the lens
/// scalar `+0x98`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    pub eye: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub scalar: f32,
}

impl CameraPose {
    /// Aimed at `target`, with the native angle convention (`FUN_003018a0`).
    pub fn looking_at(eye: Vec3, target: Vec3, scalar: f32) -> Self {
        let (pitch, yaw) = native_angles((target - eye).to_array());
        Self {
            eye,
            pitch,
            yaw,
            scalar,
        }
    }

    /// The native basis forward vector for these angles.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            -self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// `FUN_003058c8`: `t * from + (1 - t) * to` on all six words, angles
    /// unwrapped.
    fn mix(from: &Self, to: &Self, t: f32) -> Self {
        let u = 1.0 - t;
        Self {
            eye: from.eye * t + to.eye * u,
            pitch: t * from.pitch + u * to.pitch,
            yaw: t * from.yaw + u * to.yaw,
            scalar: t * from.scalar + u * to.scalar,
        }
    }
}

/// VU0's add as PCSX2 runs it: the exact sum chopped (no guard bit).
fn vu_add(a: f32, b: f32) -> f32 {
    camera_chop(a as f64 + b as f64)
}

/// `FUN_002f7700`'s sine and cosine: the nearest quarter turn
/// (`cvt.w.s(x × 2/π ± 0.5)`, no whole turns off first), the odd
/// polynomial on the rest, the cosine `sqrt(1 − s²)`, swapped and negated
/// by the quarter.
fn rotation_sin_cos(x: f32) -> (f32, f32) {
    let two_over_pi = f32::from_bits(0x3f22_f983);
    let k = camera_mul(x, two_over_pi);
    let k = if x < 0.0 {
        camera_add(k, -0.5)
    } else {
        camera_add(k, 0.5)
    };
    let quarter = k as i32;
    let r = camera_add(x, -camera_mul(quarter as f32, f32::from_bits(0x3fc9_0fdb)));
    let r2 = camera_mul(r, r);
    let mut p = camera_mul(r2, f32::from_bits(0x3638_ef1d));
    for c in [0xb950_0d01, 0x3c08_8889, 0xbe2a_aaab] {
        p = camera_mul(camera_add(p, f32::from_bits(c)), r2);
    }
    let s = camera_mul(camera_add(p, 1.0), r);
    let c = camera_add(1.0, -camera_mul(s, s)).sqrt();
    match quarter & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `FUN_002f7700(out, axis, angle)`: the rotation by `angle` about the
/// unit `axis` (rows, `w` last), its products in the original's order.
fn axis_rotation(axis: [f32; 3], angle: f32) -> [[f32; 4]; 4] {
    let (s, c) = rotation_sin_cos(angle);
    let [ax, ay, az] = axis;
    let m = camera_add(1.0, -c);
    let f2 = camera_mul(m, ax);
    let f9 = camera_mul(s, ay);
    let f8 = camera_mul(s, az);
    let f5 = camera_mul(s, ax);
    let f3 = camera_mul(m, ay);
    let f4 = camera_mul(m, az);
    [
        [
            camera_add(camera_mul(f2, ax), c),
            camera_add(camera_mul(f2, ay), f8),
            camera_add(camera_mul(f2, az), -f9),
            0.0,
        ],
        [
            camera_add(camera_mul(f3, ax), -f8),
            camera_add(camera_mul(f3, ay), c),
            camera_add(camera_mul(f3, az), f5),
            0.0,
        ],
        [
            camera_add(camera_mul(f4, ax), f9),
            camera_add(camera_mul(f4, ay), -f5),
            camera_add(camera_mul(f4, az), c),
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The VU0 row transform `vmulax`, `vmadday`, `vmaddaz`, `vmaddw`: `v`'s
/// components times the rows of `a`, summed in that order.
fn vu_transform(a: &[[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|k| {
        let mut acc = camera_mul(a[0][k], v[0]);
        for j in 1..4 {
            acc = vu_add(acc, camera_mul(a[j][k], v[j]));
        }
        acc
    })
}

/// `FUN_002ffac0`: a point's 640x480 framing position from the eye, the
/// two angles and the raw lens `+0x98` (no renderer zoom), or none at or
/// behind the 0.5 near plane. Screen x grows toward camera right, y down.
/// The view is `FUN_002f7700`'s rotations about z (the roll, 0 here), x
/// (−pitch) and y (−yaw) chained by VU0 onto the identity, its last row
/// the eye's translation; the point goes through it, `vdiv` and `vmulq`
/// take it to the screen plane, and the lens (`FUN_002ff818`: × 4/3 across)
/// to pixels.
fn framing_projection(eye: Vec3, pitch: f32, yaw: f32, scalar: f32, point: Vec3) -> Option<Vec2> {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut view = identity;
    for (axis, angle) in [
        ([0.0, 0.0, 1.0], -0.0_f32),
        ([1.0, 0.0, 0.0], -pitch),
        ([0.0, 1.0, 0.0], -yaw),
    ] {
        let r = axis_rotation(axis, angle);
        view = std::array::from_fn(|i| vu_transform(&view, r[i]));
    }
    view[3] = vu_transform(&view, [-eye.x, -eye.y, -eye.z, 1.0]);
    let p = vu_transform(&view, [point.x, point.y, point.z, 1.0]);
    if p[2] <= 0.5 {
        return None;
    }
    let q = camera_chop(1.0 / p[2] as f64);
    let tan_h = camera_mul(scalar, f32::from_bits(0x3faa_aaab));
    let across = camera_mul(640.0 / tan_h, -0.5);
    let down = camera_mul(480.0 / scalar, -0.5);
    let x = camera_mul(vu_add(camera_mul(p[0], q), -tan_h), across);
    let y = camera_mul(vu_add(camera_mul(p[1], q), -scalar), down);
    Some(Vec2::new(x, y))
}

/// `FUN_002ff830` with no top margin: the zone bits of a framing position
/// in a window `[left, left band, right, right band, top, top band,
/// bottom, bottom band]` (`0x4000` when the projection fails).
fn framing_zones(screen: Option<Vec2>, window: &[i32; 8]) -> u32 {
    framing_zones_raised(screen, screen.map(|at| at.y), window)
}

/// `FUN_002ff830` with a margin: the tests above the screen and against
/// the top edge and band read `top_y`, the point raised by the margin
/// (raw `0x2ff880..0x2ff8a4`); the bottom ones read the point itself.
fn framing_zones_raised(screen: Option<Vec2>, top_y: Option<f32>, window: &[i32; 8]) -> u32 {
    let Some(Vec2 { x, y }) = screen else {
        return 0x4000;
    };
    let top_y = top_y.unwrap_or(y);
    let w = window.map(|v| v as f32);
    let mut zones = if x < 0.0 {
        0x1000
    } else if x > 640.0 {
        0x2000
    } else {
        0
    };
    if top_y < 0.0 {
        zones |= 0x400;
    } else if y > 480.0 {
        zones |= 0x800;
    }
    zones |= if x < w[0] {
        0x100
    } else if x < w[0] + w[1] {
        0x10
    } else if 640.0 - w[2] < x {
        0x200
    } else if 640.0 - w[2] - w[3] < x {
        0x20
    } else {
        1
    };
    if top_y < w[4] {
        zones |= 0x40;
    } else if top_y < w[4] + w[5] {
        zones |= 4;
    }
    if 480.0 - w[6] < y {
        zones |= 0x80;
    } else if 480.0 - w[6] - w[7] < y {
        zones |= 8;
    }
    if zones & 0xcc == 0 { zones | 2 } else { zones }
}

/// `FUN_002face8` with no planar commands: the pitch and yaw turns that
/// keep the focus where it is on screen after it moves by `motion`.
fn face_commands(eye: Vec3, focus: Vec3, motion: Vec3) -> (f32, f32) {
    let now = focus - eye;
    let (pitch, yaw) = native_angles(now.to_array());
    let (next_pitch, next_yaw) = native_angles((now + motion).to_array());
    (
        wrap_camera_angle(next_pitch - pitch),
        wrap_camera_angle(next_yaw - yaw),
    )
}

/// The positioned helper's framing window (`FUN_00301fa8`, descriptor
/// `0x4988c0`): edges 110, 50 and 50 and bands 165, 110 and 210 pixels for
/// the sides, top and bottom.
const POSITIONED_WINDOW: [i32; 6] = [110, 165, 50, 110, 50, 210];

/// Helper 2 (`+0x2c`, vtable `0x498950`): an eye placed by `FUN_00301858`
/// that stays put, a lens that keeps the subject a set size, and an
/// angular controller that keeps it inside a framing window.
#[derive(Debug, Clone, Default)]
pub struct PositionedCameraHelper {
    /// `+0x84..+0x8c`, mirrored at `+0x00`, which the side alternation and
    /// the proximity test read.
    pub eye: Vec3,
    /// `+0x98`: the lens scalar.
    pub scalar: f32,
    /// `+0x1c0`: the subject-size scale.
    pub distance_scale: f32,
    /// `+0x1c4`: `distance_scale / |eye - focus|`.
    pub target_ratio: f32,
    /// `+0xe0`: the framed point.
    pub focus: Vec3,
    /// The focus descriptor (`+0x180`).
    pub focus_kind: CameraFocus,
    pub source: CameraSourceState,
    /// `+0x90` and `+0x94`.
    pub pitch: f32,
    pub yaw: f32,
    /// `+0xa8` and `+0xac`: the angular rates, radians per update.
    pub pitch_rate: f32,
    pub yaw_rate: f32,
    /// `+0x134`: 0 holding, 1 following, 2 bringing the subject back to
    /// the middle of the window.
    pub framing: u8,
    /// `+0x118` and `+0x11c`: how far the window leads a moving subject
    /// (pixels), and its rate.
    pub lead: f32,
    pub lead_rate: f32,
    settling: bool,
}

impl PositionedCameraHelper {
    /// `FUN_00303c80` with its last argument zero: a low eye beside the near
    /// sideline. `gamebreaker` is manager `+0x40`, which narrows it; `court`
    /// is the config's court (`+0xcc`), where courts 0 and 6 keep z at -35.
    pub fn place_courtside(
        &mut self,
        random: &mut NativeWideRandom,
        gamebreaker: bool,
        court: Option<usize>,
    ) {
        let (half_width, scale_base) = if gamebreaker {
            (23.0, 2.0)
        } else {
            (36.0, 3.5)
        };
        self.distance_scale = random.unit() * 3.0 + scale_base;
        let depth_spread = if matches!(court, Some(0 | 6)) {
            0.0
        } else {
            9.0
        };
        let previous_x = self.eye.x;
        let mut x = (half_width + half_width) * (random.unit() - 0.5);
        let y = random.unit() * 6.0 + 0.8;
        let z = -35.0 - random.unit() * depth_spread;
        // 0x303f2c..0x303f8c: never twice on the same side of centre.
        if (previous_x > 0.0 && x > 0.0) || (previous_x < 0.0 && x < 0.0) {
            x = -x;
        }
        self.eye = Vec3::new(x, y, z);
    }

    /// `FUN_00303c80` with its last argument one (raw `0x303f90..0x3041c4`),
    /// the replay's baseline view: an eye beyond the free throw line on the
    /// ball's side, low and near the sideline, scale 5.5..8. With a player
    /// within 3 of it (`FUN_00304d70`) it falls back to a courtside eye.
    pub fn place_baseline(
        &mut self,
        random: &mut NativeWideRandom,
        gamebreaker: bool,
        court: Option<usize>,
        world: &street_sim::World,
    ) {
        let mut x = 18.0 * random.unit() + 24.0;
        if world.ball.position[0] < 0.0 {
            x = -x;
        }
        let y = random.unit() * 1.5 + 0.5;
        let z = -8.0 - random.unit() * 4.0;
        self.eye = Vec3::new(x, y, z);
        self.distance_scale = random.unit() * 2.5 + 5.5;
        if players_near(world, self.eye) {
            self.place_courtside(random, gamebreaker, court);
        }
    }

    /// `FUN_00303458`, after the match intro: one of four eyes by the centre
    /// line, `rand() % 4` from `FUN_003b42a0`, and scale 6.75.
    pub fn place_midcourt(&mut self, draw: u32) {
        let (x, y) = match draw % 4 {
            0 => (4.1, 0.5),
            1 => (-4.1, 8.0),
            2 => (-4.1, 0.5),
            _ => (4.1, 8.0),
        };
        self.eye = Vec3::new(x, y, -15.0);
        self.distance_scale = 6.75;
    }

    /// `FUN_002fa4f8`: the next update settles (`FUN_002fabe8`), aiming at
    /// the focus through the pose callback `FUN_003018a0` with the target
    /// ratio for its lens and the rates at zero. (The original runs that
    /// settle at once and its ordinary update again later in the same
    /// frame; the rewrite folds them into the next update.)
    fn select(&mut self) {
        self.settling = true;
    }

    /// `FUN_00301fa8`'s window (`FUN_002ff830`'s eight thresholds): bands
    /// scaled by `2 / scale` beyond scale 2, halved again while recentring
    /// a small subject, and the window shifted by the lead.
    fn window(&self) -> [i32; 8] {
        let mut factor = 1.0_f32;
        if self.framing != 0 && self.distance_scale <= 4.0 {
            factor = 0.5;
        }
        if 2.0 < self.distance_scale {
            factor = camera_mul(factor, 2.0 / self.distance_scale);
        }
        let [side, side_band, top, top_band, bottom, bottom_band] = POSITIONED_WINDOW;
        // `cvt.w.s`, which truncates on the EE.
        let band = |b: i32| camera_mul(b as f32, factor) as i32;
        let (sb, tb, bb) = (band(side_band), band(top_band), band(bottom_band));
        let edge = side + (side_band - sb);
        [
            edge,
            sb,
            edge,
            sb,
            top + (top_band - tb),
            tb,
            bottom + (bottom_band - bb),
            bb,
        ]
    }

    /// `FUN_002fd970` and `FUN_00301e38`: the subject's screen motion leads
    /// the window by up to 40 pixels (15 per pixel of motion, less for small
    /// subjects), eased by a sixteenth (a quarter when it reverses); a
    /// settle takes the lead at once.
    fn lead_window(&mut self, window: &mut [i32; 8], settling: bool) {
        let scale = self.distance_scale;
        let at = |p: Vec3| framing_projection(self.eye, self.pitch, self.yaw, self.scalar, p);
        // Unprojectable points leave no motion (the original reads stale
        // stack values there).
        // `FUN_002fd970`: the focus and the moved focus (VU0's `vadd`)
        // projected, the motion their x apart.
        let (f, m) = (self.focus, self.source.motion);
        let moved = Vec3::new(vu_add(f.x, m.x), vu_add(f.y, m.y), vu_add(f.z, m.z));
        let motion = match (at(f), at(moved)) {
            (Some(a), Some(b)) => camera_add(b.x, -a.x),
            _ => 0.0,
        };
        let target = if 2.0 < scale {
            let mut lead = camera_mul(motion, 15.0);
            if scale < 6.0 {
                lead = camera_mul(lead, camera_mul(camera_add(scale, -2.0), 0.25));
            }
            lead.clamp(-40.0, 40.0)
        } else {
            0.0
        };
        if settling {
            self.lead = target;
            self.lead_rate = 0.0;
        } else {
            let gap = camera_add(camera_add(target, -self.lead), -self.lead_rate);
            let same = (gap >= 0.0) == (self.lead_rate >= 0.0);
            let rate = if same || self.lead_rate == 0.0 {
                0.0625
            } else {
                0.25
            };
            self.lead_rate = camera_add(self.lead_rate, camera_mul(gap, rate));
            self.lead = camera_add(self.lead, self.lead_rate);
        }
        let shift = self.lead as i32;
        window[0] -= shift;
        window[2] += shift;
    }

    /// `FUN_00301fa8`: the focus (`FUN_003003f8` with its alternate argument),
    /// the target ratio, `FUN_00301d50`'s easing, then the angular
    /// controller. `upper_body` is the subject's evaluated bones 21 and 10
    /// as offsets from the actor's position `+0x30`: `+0xe0` adds them,
    /// mixed by `clamp((scale - 4) / 15, 0, 1)` (`FUN_00305488`), to
    /// `+0xc0` (which game phase 1 zeroes in x and z).
    pub fn step(
        &mut self,
        sample: &CameraSourceSample,
        upper_body: Option<[Vec3; 2]>,
        last_entity: &mut Option<usize>,
    ) -> CameraPose {
        let settling = std::mem::take(&mut self.settling);
        self.source.update(sample, settling, last_entity);
        let raw = self.source.raw.unwrap_or(sample.focus);
        self.focus = match upper_body {
            Some([head, chest]) if !sample.ownerless => {
                let w = ((self.distance_scale - 4.0) / 15.0).clamp(0.0, 1.0);
                raw + (head * (1.0 - w) + chest * w)
            }
            _ => raw,
        };
        self.follow(settling, sample.source != Vec3::ZERO)
    }

    /// `FUN_00301fa8` once `+0xc0..+0xf0` and `+0xe0` are set: the window
    /// and its lead, the target ratio, the lens and the angular controller.
    /// `moving` is the descriptor's source vector (`FUN_00305360`) moving.
    fn follow(&mut self, settling: bool, moving: bool) -> CameraPose {
        let mut window = self.window();
        self.lead_window(&mut window, settling);
        // The VU0 length at 0x301ffc.. (vsub, vmul, three vadd broadcasts)
        // chops; the square root and the division round to nearest. The
        // retained capture's target is bit-exact only with the chopped sum.
        let d = [
            camera_add(self.eye.x, -self.focus.x),
            camera_add(self.eye.y, -self.focus.y),
            camera_add(self.eye.z, -self.focus.z),
        ];
        let square = camera_add(
            camera_add(camera_mul(d[0], d[0]), camera_mul(d[1], d[1])),
            camera_mul(d[2], d[2]),
        );
        self.target_ratio = self.distance_scale / square.sqrt();
        if settling {
            self.scalar = self.target_ratio;
            let (pitch, yaw) = native_angles((self.focus - self.eye).to_array());
            self.pitch = pitch;
            self.yaw = yaw;
            self.pitch_rate = 0.0;
            self.yaw_rate = 0.0;
            self.framing = 0;
        } else {
            self.scalar = positioned_lens_step(self.scalar, self.target_ratio);
            self.steer(&window, moving);
        }
        self.pose()
    }

    /// `FUN_002ff1c8` with flags `0xc0` for descriptor state 3: the turn
    /// that keeps the clamped focus `+0xd0` in line (`FUN_002face8`), the
    /// framing machine of `FUN_002fed00` on `+0xe0`, `FUN_002fc3f8`'s
    /// flags and corrections, and `FUN_002fda08`'s rates.
    fn steer(&mut self, window: &[i32; 8], moving: bool) {
        let rate = 60.0_f32;
        let (mut pitch_turn, mut yaw_turn) =
            face_commands(self.eye, self.source.clamped, self.source.motion);
        // FUN_002fed00's steps (the record's fields 7 and 8; 5 and 6 are
        // the eye's, which never moves here).
        let mut pitch_step = native_camera_atan(self.scalar) / (rate * 0.167);
        let mut yaw_step = pitch_step * (4.0 / 3.0);
        let screen = framing_projection(self.eye, self.pitch, self.yaw, self.scalar, self.focus);
        let zones = framing_zones(screen, window);
        let mut flags = 0xc0_u32;
        let recentre = if zones & 0x3c0 != 0 {
            self.framing = 2;
            true
        } else if self.framing == 0 {
            false
        } else {
            if zones & 3 == 3 {
                self.framing = u8::from(moving);
            }
            self.framing == 2
        };
        if recentre {
            if let Some(at) = screen {
                let centre_x =
                    ((window[0] + window[1] + ((640 - window[3]) - window[2])) >> 1) as f32;
                let centre_y =
                    ((window[4] + window[5] + ((480 - window[7]) - window[6])) >> 1) as f32;
                flags |= if centre_x - at.x < 0.0 { 2 } else { 1 };
                flags |= if centre_y - at.y < 0.0 { 8 } else { 4 };
                let mut across = ((centre_x - at.x) / 320.0).powi(2);
                let mut down = ((centre_y - at.y) / 240.0).powi(2);
                across = across.min(3.0);
                if flags & 4 != 0 {
                    down *= 4.0;
                }
                down = down.min(3.0);
                yaw_step *= across;
                pitch_step *= down;
            }
        } else if self.framing == 0 {
            if moving {
                self.framing = 1;
            } else {
                flags |= 0x30;
            }
        }
        // FUN_002fc3f8.
        if flags & 0x10 != 0 {
            yaw_turn = 0.0;
        }
        if flags & 0x20 != 0 {
            pitch_turn = 0.0;
        }
        if flags & 1 != 0 {
            yaw_turn += yaw_step;
        } else if flags & 2 != 0 {
            yaw_turn -= yaw_step;
        }
        if flags & 4 != 0 {
            pitch_turn -= pitch_step;
        } else if flags & 8 != 0 {
            pitch_turn += pitch_step;
        }
        let (pitch_correction, yaw_correction) = if zones & 0x7c00 != 0 {
            // Off screen: an eighth more turn, quarter corrections.
            pitch_turn += pitch_turn * 0.125;
            yaw_turn += yaw_turn * 0.125;
            (
                (pitch_turn - self.pitch_rate) * 0.25,
                (yaw_turn - self.yaw_rate) * 0.25,
            )
        } else {
            // Raw 0x2fc574..0x2fc684: a correction that agrees with its rate
            // (or starts it) is cut to a quarter again, an eighth inside
            // the window's bands; one against a running rate stays.
            let agrees =
                |rate: f32, correction: f32| (rate >= 0.0) == (correction >= 0.0) || rate == 0.0;
            let mut yaw_correction = (yaw_turn - self.yaw_rate) * 0.25;
            let mut pitch_correction = (pitch_turn - self.pitch_rate) * 0.25;
            if agrees(self.yaw_rate, yaw_correction) {
                yaw_correction *= if zones & 0x31 != 0 { 0.125 } else { 0.25 };
            }
            if agrees(self.pitch_rate, pitch_correction) {
                pitch_correction *= if zones & 0xe != 0 { 0.125 } else { 0.25 };
            }
            (pitch_correction, yaw_correction)
        };
        // FUN_002fda08: one update of the rates, then the angles.
        self.yaw_rate = velocity_step(self.yaw_rate, yaw_turn, yaw_correction, true);
        self.pitch_rate = velocity_step(self.pitch_rate, pitch_turn, pitch_correction, false);
        self.yaw += self.yaw_rate;
        self.pitch += self.pitch_rate;
    }

    /// The published pose (`+0x84..+0x98`).
    pub fn pose(&self) -> CameraPose {
        CameraPose {
            eye: self.eye,
            pitch: self.pitch,
            yaw: self.yaw,
            scalar: self.scalar,
        }
    }
}

/// `FUN_0018cdb0` from the rim the possession attacks (`FUN_00140518`):
/// the planar distance `game+0x374` keeps for each actor and
/// `FUN_00153790` stores in `ball+0x154`.
fn rim_distance(world: &street_sim::World, at: [f32; 2]) -> f32 {
    let x = street_sim::restart::side(world.possession) * ROW_RIM_X;
    ((at[0] - x) * (at[0] - x) + at[1] * at[1]).sqrt()
}

/// `FUN_00304d70`: any of the players within `3.0` of `eye`, in 3D.
fn players_near(world: &street_sim::World, eye: Vec3) -> bool {
    world
        .players
        .iter()
        .any(|p| Vec3::new(p.position[0], p.height, p.position[1]).distance(eye) < 3.0)
}

/// `FUN_00301d50` with the defaults read live from the original: global
/// `0x400b40 == 1` selects damping `0x400b44 == 0.8`, and the clamp is
/// `60 / fps` with fps 60. Rounded to nearest: all six retained original
/// updates match that way, two of them do not when chopped.
fn positioned_lens_step(scalar: f32, target: f32) -> f32 {
    let delta = (target - scalar) * 0.25;
    let correction = (delta * 0.8).clamp(-1.0, 1.0);
    scalar + (delta - correction)
}

/// Helper 3 (`+0x30`, vtable `0x498dd0`): `FUN_00305840` starts it with a
/// count; each update `FUN_003058c8` counts down and mixes the two poses.
#[derive(Debug, Clone)]
pub struct CameraBlendHelper {
    /// `+0x1c8`: the outgoing helper, still updated unless frozen.
    pub from: CameraHelperKind,
    /// `+0x1d0` set: the outgoing pose captured once (`+0x1d4..`).
    pub frozen: Option<CameraPose>,
    /// `+0x1cc`: the incoming helper, which becomes active at zero.
    pub to: CameraHelperKind,
    /// `+0x1c0`.
    pub counter: u32,
    /// `+0x1c4`.
    pub duration: f32,
    /// The published pose (`+0x84..`).
    pub pose: CameraPose,
}

/// The manager's inputs for one 60 Hz update.
#[derive(Clone, Copy)]
pub struct CameraTick<'a> {
    pub world: &'a street_sim::World,
    /// The simulation's events from this update (`TickEvents`); none while
    /// a replay holds the game.
    pub events: &'a [street_sim::Event],
    /// An authored camera on screen ([`StagedView`]).
    pub staged: Option<StagedView>,
    /// The pregame intro is still playing (`FUN_0013e358`).
    pub intro_playing: bool,
    /// A replay is playing (the replay manager's state 2, `crate::tape`).
    pub replay_playing: bool,
    /// The gamebreaker task posted its end this update (`FUN_00144ef8`'s
    /// `0x3c6880`, slot `+0x20c`), after its two replays
    /// ([`crate::tape::GamebreakerTask`]).
    pub gamebreaker_over: bool,
    /// The Pause row selector, already mapped to a row.
    pub camera_mode: usize,
    /// Config mode 0, the attract match: manager `+0x4c`.
    pub attract: bool,
    /// The config's court index (`+0xcc`).
    pub court: Option<usize>,
    /// Each player's evaluated bones 21 and 10, by player.
    pub bodies: &'a [Option<[Vec3; 2]>],
    /// A School lesson (game mode 4): no intro sequence, and the lesson's
    /// launch selects the row (slot `+0x274`).
    pub lesson: bool,
    /// The Explosive Rims cheat (`FUN_00131208() + 0x14`).
    pub explosive_rims: bool,
    /// Which players a person controls, by player (`game+0x24 + id*4`
    /// other than 4); a team with one has a person on it (`game+0x9a +
    /// team`). Empty in the attract match.
    pub seated: &'a [bool],
}

/// What the manager publishes for the renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraLens {
    /// A native lens scalar `+0x98`, converted with the renderer's zoom and
    /// the Widescreen row's picture.
    Scalar(f32),
    /// An authored script's vertical FOV.
    Fov(f32),
}

/// Ticks of `FUN_00304a38`'s blend from the intro cut to the row (`0x37`).
pub const INTRO_CUT_BLEND_TICKS: u32 = 55;
/// The cut holds while the global clock has advanced no more than one second
/// (`DAT_004b32e0` units per second); at 60 updates a second that is 60.
pub const INTRO_CUT_HOLD_TICKS: u32 = 60;
/// The gamebreaker blends (`FUN_00303650`, `FUN_003037b8`): `fps / 2`.
pub const GAMEBREAKER_BLEND_TICKS: u32 = 30;
/// The celebration countdowns (`+0x50`): `DAT_004d908c` (twice the frame
/// rate, written at `0x305174`) for a Level 1 gamebreaker basket, half of
/// it for the winning basket and the replay's celebration.
pub const CELEBRATION_TICKS: [u32; 2] = [120, 60];
/// The row's shake (`FUN_002f8338`): amplitude, cycles and updates. A made
/// dunk, a pocket or a cancel (`DAT_00402008`, `DAT_00402000`,
/// `DAT_00402004`) and a gamebreaker basket (0.75).
pub const ROW_SHAKE: (f32, u16, u16) = (0.5, 15, 40);
pub const BASKET_SHAKE: f32 = 0.75;
/// `FUN_00302c20`'s point on the basket: the rim's middle, x ±36.75.
pub const BASKET_POINT: [f32; 2] = [36.75, 11.956_775];

/// The camera manager (`FUN_003038b0`, vtable `0x498d10`) and its event
/// listener (`0x498a68`): which helper is on screen and how it changes.
pub struct CameraManager {
    /// `DAT_00402020`.
    pub active_helper: CameraHelperKind,
    pub row_helper: NativeCameraState,
    pub row_source: CameraSourceState,
    /// The row helper's focus descriptor.
    pub row_focus: CameraFocus,
    pub positioned_helper: PositionedCameraHelper,
    pub blend_helper: Option<CameraBlendHelper>,
    /// Global `0x401098`: the last source entity, shared by all helpers.
    last_entity: Option<usize>,
    /// Listener `+0x28`: the intro cut is still owed (set at construction).
    intro_cut_owed: bool,
    /// `DAT_00402010`/`DAT_00402014`: updates since the intro cut, until it
    /// blends to the row.
    intro_cut_age: Option<u32>,
    /// `DAT_0040200c`: set by the intro cut, cleared when the opening toss
    /// goes up (slot `+0x144`, `0x303640`). The cut's blend needs it set,
    /// the replay cuts clear.
    toss_pending: bool,
    /// Manager `+0x40`: a gamebreaker is on.
    pub gamebreaker: bool,
    /// A staged gamebreaker's script is the one on screen.
    staged_gamebreaker: bool,
    /// Listener `+0x18`: the first gamebreaker replay's eye was chosen.
    gamebreaker_replay_flags: u8,
    /// Manager `+0x50`: the celebration countdown.
    pub celebration: u32,
    /// Listener `+0x4`: a celebration camera is owed; the restart then
    /// selects the row even when it is on screen.
    celebrate: bool,
    /// Listener `+0x8`: the winning basket went in.
    won: bool,
    /// Listener `+0xc`: the side of the ball at the last replay request,
    /// 2 or 3 by `z > 0.793225` (1 from slot `+0x154`, not ported).
    replay_side: u8,
    /// Listener `+0x10`: the scorer of the last basket it noted.
    shooter: Option<usize>,
    /// Manager `+0x58`: this update's replay request, 1 for a release or
    /// the actor plus 3.
    pub replay_request: usize,
    /// Game phase 1, the opening toss (`game + 0xf4 == 1`).
    pub referee_phase: bool,
    /// `FUN_003a16b8` for placements.
    pub placement_random: NativeWideRandom,
    /// `FUN_003b42a0` for coin flips and the intro cut's choice.
    pub coin_random: crate::progression_trace::NativeRandom,
    /// The last authored camera, published while the staged helper is active.
    pub staged_pose: Option<StagedView>,
    previous: Option<CameraObservation>,
    /// `ball+0x154` (`FUN_00153790`): the ball's distance from the
    /// possession's rim when it last changed hands or left them.
    ball_distance: f32,
    /// The restart's walk to the inbound spot, for slot `+0x14c`.
    inbound: InboundWatch,
}

/// What the manager follows of a restart (game phase 6) to post the
/// inbound walk's message the way "throw inbound" does (`FUN_0010b820`'s
/// first update posts `0x3c6510` to slot `+0x14c`, `FUN_003025e8`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct InboundWatch {
    /// The restart plan's order as it stood before this restart: the
    /// spots are pushed by those players' heights (`DAT_004e7408`).
    pushers: [usize; 3],
    /// A restart is under way.
    restarting: bool,
    /// The inbounder held the ball at the last update.
    held: bool,
    /// The walk's message went out this restart.
    posted: bool,
}

/// What the manager remembers of the last update, to see game events.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CameraObservation {
    tick: u64,
    owner: Option<usize>,
    owner_height: f32,
    ball: street_sim::BallState,
    staged: bool,
    intro: bool,
    replay: bool,
    toss: street_sim::OpeningToss,
}

impl CameraManager {
    pub fn new(mode: usize) -> Self {
        Self {
            // FUN_003038b0 selects the row helper first.
            active_helper: CameraHelperKind::Row,
            row_helper: NativeCameraState::new(mode),
            row_source: CameraSourceState::row(),
            row_focus: CameraFocus::Default,
            positioned_helper: PositionedCameraHelper::default(),
            blend_helper: None,
            last_entity: None,
            intro_cut_owed: true,
            intro_cut_age: None,
            toss_pending: false,
            gamebreaker: false,
            staged_gamebreaker: false,
            gamebreaker_replay_flags: 0,
            celebration: 0,
            celebrate: false,
            won: false,
            replay_side: 0,
            shooter: None,
            replay_request: 0,
            referee_phase: false,
            placement_random: NativeWideRandom::default(),
            coin_random: crate::progression_trace::NativeRandom::new(1),
            staged_pose: None,
            previous: None,
            ball_distance: 0.0,
            inbound: InboundWatch {
                pushers: [0, 1, 2],
                ..InboundWatch::default()
            },
        }
    }

    /// A script's camera is on screen (`DAT_0040108c`, set by `FUN_00303370`
    /// and cleared by `FUN_003033a8`).
    pub fn script_on_screen(&self) -> bool {
        self.previous.is_some_and(|p| p.staged)
    }

    /// `FUN_002fa4f8` on the row helper: it settles on its next update.
    fn select_row(&mut self) {
        self.active_helper = CameraHelperKind::Row;
        self.blend_helper = None;
        self.row_helper.initialized = false;
    }

    /// `FUN_00303c10`: a new courtside eye (narrow while a gamebreaker is
    /// on) on `focus`, settled.
    fn select_courtside(&mut self, court: Option<usize>, focus: CameraFocus) {
        self.active_helper = CameraHelperKind::Positioned;
        self.blend_helper = None;
        self.positioned_helper
            .place_courtside(&mut self.placement_random, self.gamebreaker, court);
        self.positioned_helper.focus_kind = focus;
        self.positioned_helper.select();
    }

    /// `FUN_00303c10` with the default focus.
    fn select_positioned(&mut self, court: Option<usize>) {
        self.select_courtside(court, CameraFocus::Default);
    }

    /// `FUN_00305840(blend, from, to, count, freeze)`: the blend starts from
    /// the outgoing helper's pose and goes on screen. A blend from the blend
    /// itself holds its current pose (the original would run it into
    /// itself).
    fn start_blend(&mut self, to: CameraHelperKind, count: u32, freeze: bool) {
        let from = self.active_helper;
        let pose = self.helper_pose(from).unwrap_or_else(|| self.row_pose());
        let frozen = (freeze || from == CameraHelperKind::Blend).then_some(pose);
        self.blend_helper = Some(CameraBlendHelper {
            from,
            frozen,
            to,
            counter: count,
            duration: count as f32,
            pose,
        });
        self.active_helper = CameraHelperKind::Blend;
    }

    /// `FUN_002f8338` on the row helper.
    fn shake_row(&mut self, amplitude: f32) {
        let (_, cycles, updates) = ROW_SHAKE;
        self.row_helper.shake = RowShake {
            left: updates,
            updates,
            cycles,
            amplitude,
        };
    }

    /// The row is on screen, or the blend going to it is.
    fn row_or_blend_to_row(&self) -> bool {
        self.active_helper == CameraHelperKind::Row
            || (self.active_helper == CameraHelperKind::Blend
                && self
                    .blend_helper
                    .as_ref()
                    .is_some_and(|b| b.to == CameraHelperKind::Row))
    }

    /// `FUN_002fa210` on the helper on screen (a blend's incoming one).
    fn refocus_active(&mut self, focus: CameraFocus) {
        let helper = match (self.active_helper, &self.blend_helper) {
            (CameraHelperKind::Blend, Some(blend)) => blend.to,
            (helper, _) => helper,
        };
        match helper {
            CameraHelperKind::Row => self.row_focus = focus,
            CameraHelperKind::Positioned => self.positioned_helper.focus_kind = focus,
            _ => {}
        }
    }

    /// The focus descriptor of the helper on screen (`DAT_00402020+0x1a0`).
    fn active_focus(&self) -> CameraFocus {
        match self.active_helper {
            CameraHelperKind::Row => self.row_focus,
            CameraHelperKind::Positioned => self.positioned_helper.focus_kind,
            _ => CameraFocus::Default,
        }
    }

    /// `FUN_003b42a0() & 1`, drawn only on the attract path.
    fn coin(&mut self) -> bool {
        self.coin_random.draw() & 1 != 0
    }

    /// Slot `+0x204`, `FUN_00303650` (posted by `FUN_00159cf0`): a
    /// gamebreaker goes up. Skipped for a Level 2 one its spot stages
    /// (`FUN_00141298`), whose script films it. A narrower courtside camera
    /// blends in over half a second.
    fn gamebreaker_on(&mut self, tick: &CameraTick) {
        if self.gamebreaker {
            return;
        }
        self.gamebreaker_replay_flags = 0;
        self.gamebreaker = true;
        if tick.attract {
            self.select_positioned(tick.court);
            return;
        }
        self.positioned_helper
            .place_courtside(&mut self.placement_random, true, tick.court);
        self.positioned_helper.focus_kind = CameraFocus::Default;
        self.positioned_helper.select();
        self.start_blend(CameraHelperKind::Positioned, GAMEBREAKER_BLEND_TICKS, false);
    }

    /// Slot `+0x20c`, `FUN_003037b8` (the gamebreaker task `FUN_00144ef8`
    /// ends): the row settles and blends back over half a second (attract:
    /// another courtside eye).
    fn gamebreaker_off(&mut self, tick: &CameraTick) {
        self.gamebreaker = false;
        if tick.attract {
            self.select_positioned(tick.court);
            return;
        }
        self.row_helper.initialized = false;
        self.start_blend(CameraHelperKind::Row, GAMEBREAKER_BLEND_TICKS, false);
    }

    /// Slot `+0xd4`, `FUN_00302c20` (the basket publisher `FUN_001515a8`):
    /// a gamebreaker basket (or with Explosive Rims any basket) turns the
    /// camera to the basket and shakes the row; a Level 1 gamebreaker's
    /// owes a celebration camera on the scorer two seconds later, the
    /// winning basket one second later.
    fn basket(&mut self, tick: &CameraTick, player: usize, basket: street_sim::Basket) {
        let world = tick.world;
        let level = match basket {
            street_sim::Basket::Gamebreaker { level, .. } => level,
            _ => 0,
        };
        let won = matches!(world.phase, street_sim::Phase::Over { .. });
        if !self.gamebreaker && (level != 0 || self.won || tick.explosive_rims) {
            self.shooter = Some(player);
            if level != 0 || self.won {
                let side = if world.ball.position[0] > 0.0 {
                    1.0
                } else {
                    -1.0
                };
                self.refocus_active(CameraFocus::Point(Vec3::new(
                    side * BASKET_POINT[0],
                    BASKET_POINT[1],
                    0.0,
                )));
            }
            if self.row_or_blend_to_row() {
                self.shake_row(BASKET_SHAKE);
            }
            if level != 0 {
                self.celebrate = !tick.lesson && !won;
                if level == 1 {
                    self.celebration = CELEBRATION_TICKS[0];
                }
            }
        }
        // The tail runs whatever the gamebreaker: the winning basket.
        if won && level < 2 {
            self.celebration = CELEBRATION_TICKS[1];
            self.celebrate = true;
            self.won = true;
            self.replay_request = player + 3;
            self.shooter = Some(player);
        }
    }

    /// `FUN_003041f0`, while the courtside camera films a replay: a release
    /// re-places it; the owner's takeoff or an alley-oop catch either films
    /// from beside the basket (60 in 101) or from the baseline view.
    fn replay_cut(&mut self, tick: &CameraTick) {
        let request = self.replay_request;
        if request == 0 || self.toss_pending {
            return;
        }
        if request == 1 {
            self.select_positioned(tick.court);
            return;
        }
        if self.won {
            self.celebration = CELEBRATION_TICKS[1];
            self.celebrate = true;
            return;
        }
        if self.replay_side == 0 || tick.explosive_rims {
            return;
        }
        let roll = self.coin_random.draw() % 101 < 60;
        let helper = &mut self.positioned_helper;
        if roll && self.replay_side != 1 {
            let side = if tick.world.ball.position[0] > 0.0 {
                1.0
            } else {
                -1.0
            };
            let z = if self.replay_side == 3 { -3.25 } else { 3.25 };
            let y = self.placement_random.unit() * 6.0 + 12.0;
            helper.eye = Vec3::new(side * 37.75, y, z);
            helper.focus_kind = CameraFocus::Point(Vec3::new(side * 34.75, 12.0, 0.0));
            helper.distance_scale = 3.0;
        } else {
            helper.place_baseline(
                &mut self.placement_random,
                self.gamebreaker,
                tick.court,
                tick.world,
            );
            helper.focus_kind = CameraFocus::Default;
        }
        helper.select();
        self.active_helper = CameraHelperKind::Positioned;
        self.blend_helper = None;
        self.replay_side = 0;
    }

    /// Slot `+0x1d4`, `FUN_003030c0`: the replay starts on a courtside eye.
    /// A gamebreaker's first replay instead films from the end line (x
    /// -42, the side byte of team 1) or, half the time under an open sky,
    /// from 100 up.
    fn replay_started(&mut self, tick: &CameraTick) {
        if self.active_helper == CameraHelperKind::Blend {
            return;
        }
        let helper = &mut self.positioned_helper;
        helper.place_courtside(&mut self.placement_random, self.gamebreaker, tick.court);
        if self.gamebreaker && self.gamebreaker_replay_flags & 3 == 0 {
            let side = -1.0;
            let overhead = self.coin_random.draw() & 1 == 0 && tick.world.ball.ceiling <= 0.0;
            if overhead {
                helper.eye.y = 100.0;
                self.gamebreaker_replay_flags |= 1;
            } else {
                helper.eye.x = side * 42.0;
                self.gamebreaker_replay_flags |= 2;
            }
        }
        helper.focus_kind = CameraFocus::Default;
        helper.select();
        self.active_helper = CameraHelperKind::Positioned;
        self.blend_helper = None;
    }

    /// Slot `+0x14c`, `FUN_003025e8`: "throw inbound" (`FUN_0010b820`)
    /// posts `0x3c6510` with its spot and its receiver on the first update
    /// of its walk, the update after the inbounder has the ball. With a
    /// person on the possessing side, the Street row, no blend on screen
    /// and the spot within the sidelines, a courtside eye goes up beside
    /// the spot, `(x ± 7.5, 12, z ∓ 3)` (the signs by the side of the
    /// spot's x), scale 6.75, on the receiver. The spot is the restart
    /// strategy's first (`FUN_0012b8b8` through `restart::spot_at`), pushed
    /// by the heights of the previous restart's order.
    fn watch_inbound(&mut self, tick: &CameraTick) {
        let w = tick.world;
        if !w.restarting() {
            self.inbound = InboundWatch {
                pushers: w.restart_plan().order,
                ..InboundWatch::default()
            };
            return;
        }
        if !self.inbound.restarting {
            self.inbound.restarting = true;
            self.inbound.held = false;
            self.inbound.posted = false;
        }
        let team = w.possession;
        let squad: Vec<usize> = (0..w.players.len())
            .filter(|&i| w.players[i].team == team)
            .collect();
        if squad.len() != 3 {
            return;
        }
        let plan = w.restart_plan();
        let inbounder = squad[plan.order[0].min(2)];
        let receiver = squad[plan.order[1].min(2)];
        let walking = self.inbound.held && !self.inbound.posted;
        self.inbound.held = w.handler() == Some(inbounder);
        if !walking {
            return;
        }
        self.inbound.posted = true;
        let person = squad
            .iter()
            .any(|&i| tick.seated.get(i).copied().unwrap_or(false));
        if !person
            || self.active_helper == CameraHelperKind::Blend
            || self.row_helper.mode != 0
            || tick.lesson
        {
            return;
        }
        let heights: [[f32; 3]; 2] = std::array::from_fn(|t| {
            let side: Vec<usize> = (0..w.players.len())
                .filter(|&i| w.players[i].team == t)
                .collect();
            std::array::from_fn(|k| side.get(k).map_or(6.5, |&j| w.players[j].height_ft()))
        });
        let spot =
            street_sim::restart::spot_at(plan.row, self.inbound.pushers, team, team, 0, &heights)
                .position;
        let (x, z) = (spot[0], spot[2]);
        if (z as f64).abs() >= 26.5 {
            return;
        }
        let eye = if 0.0 < x {
            Vec3::new(x + 7.5, 12.0, z - 3.0)
        } else {
            Vec3::new(x - 7.5, 12.0, z + 3.0)
        };
        let helper = &mut self.positioned_helper;
        helper.eye = eye;
        helper.distance_scale = 6.75;
        helper.focus_kind = CameraFocus::Actor(receiver);
        helper.select();
        self.active_helper = CameraHelperKind::Positioned;
        self.blend_helper = None;
    }

    /// The listener's handlers for what changed since the last update.
    fn observe(&mut self, tick: &CameraTick) {
        let world = tick.world;
        let owner = match world.ball.state {
            street_sim::BallState::Held(owner) => Some(owner),
            _ => None,
        };
        let now = CameraObservation {
            tick: world.tick,
            owner,
            owner_height: owner
                .and_then(|o| world.players.get(o))
                .map_or(0.0, |p| p.height),
            ball: world.ball.state,
            staged: tick.staged.is_some(),
            intro: tick.intro_playing,
            replay: tick.replay_playing,
            toss: world.opening_toss,
        };
        // The first update sees every script or replay already running begin.
        let before = self.previous.replace(now).unwrap_or(CameraObservation {
            staged: false,
            intro: false,
            replay: false,
            ..now
        });
        self.replay_request = 0;
        self.referee_phase = world.opening_toss != street_sim::OpeningToss::None;
        let stepped = now.tick != before.tick;
        // ball+0x154: FUN_00153790 at each catch (FUN_0014cb28) and launch
        // (FUN_0014aaf8's shots, FUN_0014c408's passes).
        if stepped && now.ball != before.ball && !matches!(now.ball, street_sim::BallState::Loose) {
            let b = world.ball.position;
            self.ball_distance = rim_distance(world, [b[0], b[2]]);
        }
        if stepped && !now.replay && !before.replay {
            self.watch_inbound(tick);
        }

        // A script starts (slot 0x1ec, FUN_00303370): the staged helper.
        if now.staged && !before.staged {
            self.active_helper = CameraHelperKind::Staged;
            self.blend_helper = None;
        }
        // A script ends (slot 0x1f4, FUN_003033a8): the row settles, unless
        // the pregame intro sequence is still running (FUN_0013e358).
        if before.staged && !now.staged && !now.intro {
            // A gamebreaker's script (the end's word +4 of 1) owes the
            // celebration two seconds on (`DAT_004d908c` into +0x50, the
            // School's Level 2 capture: 120 the update the script ends).
            if self.staged_gamebreaker {
                self.celebration = CELEBRATION_TICKS[0];
            }
            self.select_row();
            // A staged gamebreaker's task ends once the script manager is
            // idle again (FUN_00144ef8, state 1 of its staged branch).
            if std::mem::take(&mut self.staged_gamebreaker) {
                self.gamebreaker_off(tick);
            }
        }

        // The simulation's events, in the order the game posts them.
        if stepped {
            for event in tick.events {
                match *event {
                    // Slot 0x204 (FUN_00159cf0's post): a staged Level 2
                    // gamebreaker is filmed by its script instead.
                    // The script starts two updates after the press
                    // (`World::staged_start`); the original's flag +0x40
                    // stays off throughout (the School's Level 2 capture).
                    street_sim::Event::Gamebreaker { level, .. } => {
                        if level == 2 && (world.sequence.is_some() || world.staged_start.is_some())
                        {
                            self.staged_gamebreaker = true;
                        } else {
                            self.gamebreaker_on(tick);
                        }
                    }
                    // Slot 0xd4: a basket went in.
                    street_sim::Event::Score { player, basket, .. } => {
                        self.basket(tick, player, basket);
                    }
                    // Slot 0x5c with ball+0x184 3 or 4 and the made flag
                    // (FUN_0014aaf8's a1): a made dunk shakes the row.
                    street_sim::Event::Slam { player, .. } => {
                        let made = tick.events.iter().any(|e| {
                            matches!(*e, street_sim::Event::Score { player: p, .. } if p == player)
                        });
                        if made
                            && self.active_helper == CameraHelperKind::Row
                            && !now.staged
                            && !now.intro
                        {
                            self.shake_row(ROW_SHAKE.0);
                        }
                    }
                    // Slot 0x234 (FUN_00159eb8, the pocket button): shakes
                    // the row on screen. Slot 0x23c (FUN_00159f40, a cancel):
                    // shakes it whatever is on screen.
                    street_sim::Event::Pocket { .. } => {
                        if self.active_helper == CameraHelperKind::Row {
                            self.shake_row(ROW_SHAKE.0);
                        }
                    }
                    street_sim::Event::Cancel { .. } => self.shake_row(ROW_SHAKE.0),
                    // Slot 0x1a4 (FUN_0013f1f0's violation post, FUN_00302488):
                    // a courtside camera on the team's last ball carrier.
                    street_sim::Event::ShotClockViolation { team } => {
                        let carrier = before
                            .owner
                            .filter(|&o| world.players.get(o).is_some_and(|p| p.team == team));
                        if !self.gamebreaker && self.active_helper != CameraHelperKind::Blend {
                            let focus = carrier.map_or(CameraFocus::Default, CameraFocus::Actor);
                            self.select_courtside(tick.court, focus);
                        }
                    }
                    // Slots 0xe4 (FUN_00302510, the restart after a basket)
                    // and 0x22c (FUN_003027f0, the dead-ball countdown at
                    // zero), which the rewrite's inbound folds together: the
                    // row with the default focus, unless it is on screen and
                    // no celebration is owed. The attract path re-places a
                    // courtside camera only off the default focus.
                    street_sim::Event::Inbound { .. } => {
                        if tick.attract {
                            if self.active_focus() != CameraFocus::Default {
                                self.select_positioned(tick.court);
                            }
                        } else if !self.gamebreaker
                            && !(self.active_helper == CameraHelperKind::Row && !self.celebrate)
                        {
                            self.row_focus = CameraFocus::Default;
                            self.select_row();
                        }
                        self.celebrate = false;
                    }
                    _ => {}
                }
            }
            // Slot 0x144 (0x303640): the opening toss goes up.
            if before.toss != street_sim::OpeningToss::Flight
                && now.toss == street_sim::OpeningToss::Flight
            {
                self.toss_pending = false;
            }
            // Slot 0xb4 (FUN_00303600): the toss is caught; the helper on
            // screen takes the default focus again.
            if before.toss == street_sim::OpeningToss::Flight
                && now.toss == street_sim::OpeningToss::None
            {
                self.refocus_active(CameraFocus::Default);
            }
        }

        // The replay starts (slot 0x1d4, FUN_003030c0).
        if now.replay && !before.replay {
            self.replay_started(tick);
        }
        // The replay ends (slot 0x1dc, FUN_00303248): the row, or in the
        // attract match another courtside eye; nothing while a gamebreaker
        // is on or a blend runs.
        if before.replay
            && !now.replay
            && !self.gamebreaker
            && self.active_helper != CameraHelperKind::Blend
        {
            if tick.attract {
                self.select_positioned(tick.court);
            } else {
                self.select_row();
            }
        }
        // The gamebreaker task's end (slot 0x20c, FUN_003037b8), posted
        // after the second replay's end in the same update.
        if tick.gamebreaker_over && self.gamebreaker && !self.staged_gamebreaker {
            self.gamebreaker_off(tick);
        }
        if now.replay != before.replay {
            return; // the world jumped to or from the replay's snapshot
        }
        // The game's own handlers below skip while the presentation
        // director is busy, and run only when the world ran.
        if now.staged || now.intro || !stepped {
            return;
        }
        // The owner leaves the floor (slot 0x24, FUN_003028c0).
        let takeoff = owner.is_some()
            && owner == before.owner
            && before.owner_height == 0.0
            && now.owner_height > 0.0;
        let side_of_ball = if world.ball.position[2] > 0.793_225 {
            3
        } else {
            2
        };
        if takeoff && let Some(o) = owner {
            self.replay_request = o + 3;
            self.replay_side = side_of_ball;
            if tick.attract && self.coin() {
                self.select_positioned(tick.court);
            }
        }
        // The shot leaves the hand (slot 0x5c, FUN_00302a78): kinds 3 and 4,
        // dunks and alley-oops, do not cut.
        let released = matches!(now.ball, street_sim::BallState::Shot)
            && !matches!(before.ball, street_sim::BallState::Shot);
        let dunk = before
            .owner
            .and_then(|o| world.players.get(o))
            .is_some_and(|p| matches!(p.action, Action::Dunking { .. }));
        if released && !dunk {
            self.replay_request = 1;
            if tick.attract && self.coin() {
                self.select_positioned(tick.court);
            }
        }
        // Someone gains the ball (slot 0xc4, FUN_00302f00): a catch in the
        // air for an alley-oop is a replay request; in play, an attract
        // courtside camera may move.
        let gained = owner.is_some() && owner != before.owner;
        let live = matches!(world.phase, street_sim::Phase::Live)
            && world.opening_toss == street_sim::OpeningToss::None;
        if gained
            && live
            && self.active_helper != CameraHelperKind::Staged
            && let Some(o) = owner
        {
            let flying = world
                .players
                .get(o)
                .is_some_and(|p| matches!(p.action, Action::Dunking { .. }));
            if flying {
                self.replay_request = o + 3;
                self.replay_side = side_of_ball;
            }
            if tick.attract && self.active_helper == CameraHelperKind::Positioned && self.coin() {
                self.select_positioned(tick.court);
            }
        }
    }

    /// One manager update (`FUN_00304548` state 2: `FUN_00304a38`, then the
    /// active helper's update `FUN_002fab68`).
    pub fn update(&mut self, tick: &CameraTick) -> (Vec3, Vec3, CameraLens) {
        let mode = tick.camera_mode.min(CAMERA_MODE_ROWS.len() - 1);
        // Slot 0x274 (FUN_00302798, the lesson's launch): the row. A
        // lesson has no intro sequence, so no intro cut either.
        if tick.lesson && self.intro_cut_owed {
            self.intro_cut_owed = false;
            self.row_focus = CameraFocus::Default;
            self.select_row();
        }
        self.observe(tick);
        if let Some(staged) = tick.staged {
            self.staged_pose = Some(staged);
        }

        // The intro sequence's end (slot 0x1fc, FUN_00303458): once a match.
        if self.intro_cut_owed && !tick.intro_playing && tick.staged.is_none() {
            self.intro_cut_owed = false;
            let draw = self.coin_random.draw();
            self.positioned_helper.place_midcourt(draw);
            self.positioned_helper.focus_kind = CameraFocus::Default;
            self.positioned_helper.select();
            self.active_helper = CameraHelperKind::Positioned;
            self.blend_helper = None;
            self.intro_cut_age = Some(0);
            self.toss_pending = true;
        }

        // FUN_00304a38: past one second, settle the row and blend to it,
        // unless the opening toss has gone up already.
        if let Some(age) = self.intro_cut_age {
            if age > INTRO_CUT_HOLD_TICKS && self.toss_pending {
                self.intro_cut_age = None;
                self.row_helper.initialized = false;
                // Slide and Solid freeze the outgoing pose.
                let freeze = matches!(self.row_helper.mode, 2 | 3);
                self.start_blend(CameraHelperKind::Row, INTRO_CUT_BLEND_TICKS, freeze);
            } else {
                self.intro_cut_age = Some(age + 1);
            }
        }
        let finished = self
            .blend_helper
            .as_ref()
            .filter(|b| b.counter == 0)
            .map(|b| b.to);
        if self.active_helper == CameraHelperKind::Blend
            && let Some(to) = finished
        {
            self.active_helper = to;
            self.blend_helper = None;
        }
        // The celebration countdown: while the game is won, or no replay or
        // script plays. At zero a courtside camera finds the scorer.
        if self.celebration > 0 {
            let won = matches!(tick.world.phase, street_sim::Phase::Over { .. });
            let busy = tick.replay_playing || tick.staged.is_some() || tick.intro_playing;
            if won || !busy {
                self.celebration -= 1;
                if self.celebration == 0 && self.celebrate {
                    let focus = self
                        .shooter
                        .map_or(CameraFocus::Default, CameraFocus::Actor);
                    self.select_courtside(tick.court, focus);
                }
            }
        }
        // FUN_002f7bb0 each update: a changed row settles.
        self.row_helper.select_mode(mode);
        if !self.gamebreaker && self.active_helper == CameraHelperKind::Positioned {
            // FUN_003041f0 during a replay.
            if tick.replay_playing {
                self.replay_cut(tick);
            }
            // FUN_00304d70: a player within 3 units of the courtside eye.
            if self.active_helper == CameraHelperKind::Positioned
                && players_near(tick.world, self.positioned_helper.eye)
            {
                self.select_positioned(tick.court);
            }
        }

        // The side byte of the possession (FUN_001404c0): the direction it
        // attacks, in the simulation's own frame.
        let side = street_sim::restart::side(tick.world.possession);
        match self.active_helper {
            CameraHelperKind::Staged => {
                let p = self.staged_pose.unwrap_or(StagedView {
                    eye: Vec3::ZERO,
                    at: Vec3::Z,
                    scalar: self.row_helper.scalar,
                    roll: 0.0,
                });
                (p.eye, p.at, CameraLens::Scalar(p.scalar))
            }
            CameraHelperKind::Row => {
                // The row's own pitch and yaw (FUN_002fa558): it frames the
                // focus, it does not aim at it.
                let pose = self.step_row(tick, side, mode);
                (
                    pose.eye,
                    pose.eye + pose.forward() * 10.0,
                    CameraLens::Scalar(pose.scalar),
                )
            }
            CameraHelperKind::Positioned => {
                let pose = self.step_positioned(tick);
                (
                    pose.eye,
                    pose.eye + pose.forward() * 10.0,
                    CameraLens::Scalar(pose.scalar),
                )
            }
            CameraHelperKind::Blend => {
                let Some(mut blend) = self.blend_helper.take() else {
                    self.select_row();
                    let pose = self.step_row(tick, side, mode);
                    return (
                        pose.eye,
                        pose.eye + pose.forward() * 10.0,
                        CameraLens::Scalar(pose.scalar),
                    );
                };
                if blend.counter > 0 {
                    blend.counter -= 1;
                    let from = match blend.frozen {
                        Some(pose) => pose,
                        None => self.step_helper(blend.from, tick, side, mode),
                    };
                    let to = self.step_helper(blend.to, tick, side, mode);
                    blend.pose = CameraPose::mix(&from, &to, blend.counter as f32 / blend.duration);
                }
                let pose = blend.pose;
                self.blend_helper = Some(blend);
                (
                    pose.eye,
                    pose.eye + pose.forward() * 10.0,
                    CameraLens::Scalar(pose.scalar),
                )
            }
        }
    }

    /// The bones `FUN_00305488` mixes in for a focus, as offsets from the
    /// actor's position: its actor's, or the ball owner's on the default
    /// path.
    fn upper_body(tick: &CameraTick, focus: CameraFocus) -> Option<[Vec3; 2]> {
        let actor = match (focus, tick.world.opening_toss, tick.world.ball.state) {
            (CameraFocus::Actor(i), _, _) => Some(i),
            // The referee holds it through the wait. The rewrite has no
            // referee rig: his bones stand where the capture has them.
            (CameraFocus::Default, street_sim::OpeningToss::Waiting(_), _) => {
                return Some(REFEREE_UPPER_BODY.map(Vec3::from_array));
            }
            (CameraFocus::Default, _, street_sim::BallState::Held(owner)) => Some(owner),
            _ => None,
        };
        let i = actor?;
        let p = tick.world.players.get(i)?;
        let at = Vec3::new(p.position[0], p.height, p.position[1]);
        tick.bodies
            .get(i)
            .copied()
            .flatten()
            .map(|bones| bones.map(|bone| bone - at))
    }

    /// The positioned helper's update (`FUN_00301fa8`).
    fn step_positioned(&mut self, tick: &CameraTick) -> CameraPose {
        let focus = self.positioned_helper.focus_kind;
        let sample = CameraSourceSample::for_focus(tick.world, focus);
        let body = Self::upper_body(tick, focus);
        self.positioned_helper
            .step(&sample, body, &mut self.last_entity)
    }

    /// The row helper's update: the source adapter (`FUN_003003f8`), then
    /// `FUN_002f8370`. In the opening phase it lifts the focus to at least
    /// 4.
    fn step_row(&mut self, tick: &CameraTick, side: f32, mode: usize) -> CameraPose {
        let sample = CameraSourceSample::for_focus(tick.world, self.row_focus);
        let settling = !self.row_helper.initialized;
        self.row_source
            .update(&sample, settling, &mut self.last_entity);
        let mut focus = self.row_source.clamped;
        if self.referee_phase && focus.y < 4.0 {
            focus.y = 4.0;
        }
        if mode == 4 {
            // Behind keeps the rewrite's fixed pose: FUN_002f7c38's side yaw
            // and ground-level pitch with the row's shake. The original
            // never selects it ([`native_camera_row`]), so its own update
            // (the +0x20c turn, its flags and x commands) is dead code there
            // and only tests reach this.
            let row = &mut self.row_helper;
            row.initialized = true;
            row.scalar = CAMERA_MODE_ROWS[4].row_word7;
            let (mut eye, mut at) = mode_camera_pose(focus, side, mode);
            let drop = row.shake.next();
            eye.y -= drop;
            at.y -= drop;
            let (pitch, yaw) = native_angles((at - eye).to_array());
            row.eye = eye.to_array();
            row.angles = [pitch, yaw];
            return row.pose();
        }
        let inputs = self.row_inputs(tick, &sample, focus, side);
        self.row_helper.update(&inputs);
        self.row_helper.pose()
    }

    /// What `FUN_002f8370` reads from the match, in the rewrite's terms.
    fn row_inputs(
        &self,
        tick: &CameraTick,
        sample: &CameraSourceSample,
        focus: Vec3,
        side: f32,
    ) -> RowInputs {
        use street_sim::BallState;
        let w = tick.world;
        let opening = w.opening_toss != street_sim::OpeningToss::None;
        // Nobody has the ball through the opening phase (game+0x110 -1):
        // FUN_003003f8 then takes the +x rim, as the original's frame has it.
        let possession = if opening { -1 } else { w.possession as i32 };
        let side = if possession < 0 { 0.0 } else { side };
        let reference = Vec3::new(
            if possession < 0 {
                ROW_RIM_X
            } else {
                side * ROW_RIM_X
            },
            ROW_RIM_Y,
            0.0,
        );
        let action = |i: usize| w.players.get(i).map(|p| p.action);
        let in_air = |i: usize| {
            matches!(
                action(i),
                Some(Action::Dunking { .. } | Action::Layup { .. })
            )
        };
        let holder = w.handler();
        let ball_state = match w.ball.state {
            BallState::Held(_) => 3,
            BallState::Pass => 4,
            BallState::Shot => 5,
            BallState::Loose => 1,
        };
        let shot = w.ball.shot.filter(|_| ball_state == 5);
        let far_shot = shot
            .and_then(|s| w.players.get(s.shooter))
            .is_some_and(|p| rim_distance(w, p.position) > 42.0);
        let pass_to_flier = ball_state == 4
            && w.ball
                .pass
                .is_some_and(|p| w.players.get(p.to).is_some_and(|q| q.leaping()));
        let seated = |i: usize| tick.seated.get(i).copied().unwrap_or(false);
        let people = |team: usize| {
            w.players
                .iter()
                .enumerate()
                .any(|(i, p)| p.team == team && seated(i))
        };
        // FUN_00300b08: a computer carrier on a side a person plays.
        let computer_carrier = holder.is_some_and(|h| people(w.players[h].team) && !seated(h));
        let flier = possession >= 0
            && w.players
                .iter()
                .any(|p| p.team == w.possession && p.leaping());
        // FUN_00305428: the focus actor's height (actor+0x260, feet) × 1.2.
        // The referee's is not known: 0.
        let actor = match self.row_focus {
            CameraFocus::Actor(i) => Some(i),
            CameraFocus::Default if !sample.ownerless => holder,
            _ => None,
        };
        let margin = actor
            .and_then(|i| w.players.get(i))
            .map_or(0.0, |p| p.height_ft() * 1.2);
        let free_ball = (self.row_focus == CameraFocus::Default && sample.ownerless)
            .then(|| Vec3::from_array(w.ball.position));
        RowInputs {
            focus,
            motion: self.row_source.motion,
            reference,
            margin,
            free_ball,
            vertical_gate: false,
            opening,
            possession,
            side,
            live: matches!(w.phase, street_sim::Phase::Live) && !opening,
            ball_state,
            gamebreaker_shot: shot.is_some_and(|s| s.gamebreaker != 0),
            ball_distance: self.ball_distance,
            owner_in_air: holder.is_some_and(in_air),
            pass_to_flier,
            far_shot,
            team_busy: computer_carrier || flier,
        }
    }

    fn row_pose(&self) -> CameraPose {
        self.row_helper.pose()
    }

    /// `FUN_002fa238`: a helper's current pose without updating it.
    fn helper_pose(&self, helper: CameraHelperKind) -> Option<CameraPose> {
        match helper {
            CameraHelperKind::Row => Some(self.row_pose()),
            CameraHelperKind::Positioned => Some(self.positioned_helper.pose()),
            CameraHelperKind::Staged => self
                .staged_pose
                .map(|p| CameraPose::looking_at(p.eye, p.at, p.scalar)),
            CameraHelperKind::Blend => self.blend_helper.as_ref().map(|b| b.pose),
        }
    }

    /// `FUN_002fab68` on a blend's helper, returning its pose.
    fn step_helper(
        &mut self,
        helper: CameraHelperKind,
        tick: &CameraTick,
        side: f32,
        mode: usize,
    ) -> CameraPose {
        match helper {
            CameraHelperKind::Row => self.step_row(tick, side, mode),
            CameraHelperKind::Positioned => self.step_positioned(tick),
            other => self.helper_pose(other).unwrap_or_else(|| self.row_pose()),
        }
    }
}

pub fn approach_step(current: f32, target: f32, limit: f32) -> f32 {
    let diff = target - current;
    if diff == 0.0 {
        0.0
    } else {
        diff.signum() * diff.abs().min(limit)
    }
}

pub fn velocity_step(current: f32, command: f32, correction: f32, yaw: bool) -> f32 {
    if correction == 0.0 || camera_add(current, -command).abs() < correction.abs() {
        if command == 0.0 || (yaw && camera_mul(current, command) < 0.0) {
            0.0
        } else {
            current
        }
    } else {
        camera_add(current, correction)
    }
}

pub fn native_angles(vector: [f32; 3]) -> (f32, f32) {
    let horizontal = (vector[0] * vector[0] + vector[2] * vector[2]).sqrt();
    (
        -native_camera_atan2(vector[1], horizontal),
        native_camera_atan2(vector[0], vector[2]),
    )
}

pub fn native_camera_atan2(y: f32, x: f32) -> f32 {
    if x == 0.0 {
        if y == 0.0 {
            0.0
        } else {
            y.signum() * std::f32::consts::FRAC_PI_2
        }
    } else {
        let mut result = native_camera_atan(y / x);
        if x < 0.0 {
            result += if y > 0.0 { PI } else { -PI };
        }
        result
    }
}

pub fn wrap_camera_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

/// The turbo bar and the latest play.
#[derive(Component)]
pub struct TurboText;

/// The ring under the player 1P..4P (by number) controls.
#[derive(Component)]
pub struct Indicator(pub usize);

/// 1P..4P's numbered badge over their player's head, when several play.
#[derive(Component)]
pub struct Badge(pub usize);

pub fn spawn_camera(commands: &mut Commands) {
    // FUN_003038b0 builds a new manager for every match.
    commands.insert_resource(ActiveCamera::default());
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 45f32.to_radians(),
            near: 0.5,
            far: 5000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, CAMERA_HEIGHT, -CAMERA_DISTANCE)
            .looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
        SidelineCamera,
        DespawnOnExit(Screen::Playing),
    ));
}

/// `FUN_002f9ab0`: the sides a court part or crowd seat belongs to, from
/// the tags in its name (`strstr`): `clef` 1, `crit` 2, `ctop` 4, `cbot` 8.
fn scenery_mask(name: &str) -> u32 {
    [("clef", 1), ("crit", 2), ("cbot", 8), ("ctop", 4)]
        .into_iter()
        .filter(|(tag, _)| name.contains(tag))
        .fold(0, |mask, (_, bit)| mask | bit)
}

/// `FUN_00304e38`: the sides the active camera's eye stands beyond: past
/// x ±44.1 (1 for +x, 2 for -x) and z ±27.825 (4 for +z, 8 for -z). The
/// original also sets 8 in its opening referee phase (`game + 0xf4 == 1`),
/// which [`cull_scenery`] adds.
pub fn scenery_side_flags(eye: Vec3) -> u32 {
    let x = if eye.x > 44.1 {
        1
    } else if eye.x < -44.1 {
        2
    } else {
        0
    };
    let z = if eye.z > 27.824_999 {
        4
    } else if eye.z < -27.824_999 {
        8
    } else {
        0
    };
    x | z
}

/// `FUN_002f9d10`, after every camera update: a tagged court part or crowd
/// seat (registered by `FUN_0030b708`) is hidden when it is on a side the
/// camera stands beyond, else shown; while a script plays (`DAT_0040108c`,
/// set by `FUN_00303370`) every one is shown.
pub fn cull_scenery(
    active_camera: Res<ActiveCamera>,
    mut groups: Query<(&DrawGroup, &mut Visibility)>,
    mut seats: Query<(&Name, &ChildOf, &mut Visibility), Without<DrawGroup>>,
    crowds: Query<(), With<street_render::crowd::CrowdRig>>,
) {
    let flags = if active_camera.manager.script_on_screen() {
        0
    } else if active_camera.manager.referee_phase {
        // FUN_00304e38 adds the near side through the opening toss.
        scenery_side_flags(active_camera.eye) | 8
    } else {
        scenery_side_flags(active_camera.eye)
    };
    let visibility = |mask: u32| {
        if mask & flags != 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        }
    };
    for (group, mut shown) in &mut groups {
        let mask = scenery_mask(&group.0);
        if mask != 0 {
            shown.set_if_neq(visibility(mask));
        }
    }
    for (name, parent, mut shown) in &mut seats {
        let mask = scenery_mask(name.as_str());
        if mask != 0 && crowds.contains(parent.parent()) {
            shown.set_if_neq(visibility(mask));
        }
    }
}

/// The game films from the -z sideline and hides the scenery between that
/// camera and the court: draw groups ending in `cbot` ("camera bottom": the
/// near bleachers, rails and fence, all at z < -33 in Rucker Park).
pub fn hide_near_side(mut groups: Query<(&DrawGroup, &mut Visibility)>) {
    for (group, mut visibility) in &mut groups {
        if group.0.ends_with("cbot") {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Place each rendered player where the simulation has it and pick the
/// animation for what it is doing.
/// The game's end (game vtable `+0x17c`, `FUN_0013f0b0`): once the game is
/// won (phase 9) and no replay is asked for, delayed or playing (the phase
/// machine's case 9 in `FUN_0013e4c8`, or the replay's end in phase 9,
/// `FUN_00142648`), the referee, the ball and the six actors are hidden
/// (each object's vtable `+0x3c` with 0) and so are the people's rings
/// (`FUN_00314298`): the boards and the post-game dialog stand over an
/// empty court.
pub fn game_ended(world: &street_sim::World, tape: Option<&crate::tape::Tape>) -> bool {
    matches!(world.phase, street_sim::Phase::Over { .. }) && !tape.is_some_and(|t| t.holding())
}

pub fn sync_players(
    sim: Res<Sim>,
    clips: Res<GameClips>,
    moves: Res<clip::PlayerMoves>,
    disc: Res<Disc>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    tape: Option<Res<crate::tape::Tape>>,
    mut players: Query<(&SimPlayer, &mut Transform, &mut Animator, &mut Visibility)>,
) {
    // A replay (crate::tape): the recorded poses either side of the
    // replay's position, blended by its fraction (FUN_00329718 and the
    // actors' FUN_003275d8): the place lerped, the rotation slerped, and
    // each recorded joint slerped by crate::tape::pose_bones over the
    // animator's pose. The clip's frame, blended here, shows only for a
    // player recorded without a rig.
    if let Some((a, b, t)) = tape.as_deref().and_then(|tape| tape.shown()) {
        for (index, mut transform, mut animator, mut visibility) in &mut players {
            let Some(Some(from)) = a.poses.get(index.0) else {
                continue;
            };
            let to = b
                .poses
                .get(index.0)
                .and_then(Option::as_ref)
                .unwrap_or(from);
            visibility.set_if_neq(Visibility::Inherited);
            transform.translation = from.translation.lerp(to.translation, t);
            transform.rotation = from.rotation.slerp(to.rotation, t);
            let Some(mut shown) = from.animator.clone() else {
                continue;
            };
            let x = &shown.current;
            let y = &to.animator.as_ref().unwrap_or(&shown).current;
            if std::sync::Arc::ptr_eq(&x.clip, &y.clip) && x.mirrored == y.mirrored {
                let n = x.clip.frames().max(1) as f32;
                let mut end = y.frame;
                if x.looping && end < x.frame {
                    end += n;
                }
                let frame = x.frame + (end - x.frame) * t;
                shown.current.frame = frame;
            }
            shown.hold();
            *animator = shown;
        }
        return;
    }
    let w = &sim.world;
    let staging = staging(w, intro.as_deref());
    let ended = game_ended(w, tape.as_deref());
    for (index, mut transform, mut animator, mut visibility) in &mut players {
        let i = index.0;
        let Some(p) = w.players.get(i) else {
            continue;
        };
        // An intro shows only its own cast; the game's end none.
        let shown = !ended
            && staging
                .as_ref()
                .is_none_or(|s| !s.cast_only || s.pose(i, &clips).is_some());
        visibility.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        transform.translation = Vec3::new(p.position[0], p.height, p.position[1]);
        // A staged script (a Level 2 gamebreaker or the intro): its own
        // place, heading and animation record, at its frame (the script
        // moves the player itself).
        if let Some(staged) = staging.as_ref().and_then(|s| s.pose(i, &clips)) {
            transform.translation = staged.at;
            transform.rotation = Quat::from_rotation_y(staged.heading + PI);
            if let Some(clip) = clips.clips.get(&staged.name) {
                // The actor VM passes channel +64 as a float to the pose
                // samplers (0019add0..0019adf8). Keep that fractional phase;
                // 00330f98's integer hand endpoint is a separate consumer.
                let looping = staged.looping.unwrap_or(false);
                // In place: the script's curves already move the actor
                // along his clip's travel (the original's proxy, as the
                // School's Level 2 capture shows: script 3's passer moves
                // 11.6 by frame 108 while his clip's root has gone 9.1
                // at his 1.27 scale), so only the root's rise and fall
                // is drawn; adding the travel again put him at the rim.
                animator.play_with_mirror_and_heading(
                    &clip,
                    0.1,
                    looping,
                    true,
                    staged.mirrored,
                    Some(staged.heading),
                );
                animator.current.looping = looping;
                animator.current.frame = match staged.looping {
                    Some(false) => staged
                        .frame
                        .clamp(0.0, clip.frames().saturating_sub(1) as f32),
                    _ => staged.frame.rem_euclid(clip.frames().max(1) as f32),
                };
                animator.current.speed = 0.0;
            }
            continue;
        }
        // Animations face -z, the simulation's heading 0 faces +z. Each is
        // drawn on his own facing, as the original draws the actor's:
        // standing, its played captures have him 26 degrees off the ball at
        // the median and past 90 a quarter of the time (the simulation's
        // 39 and 26%). Turning a standing player to the ball here had
        // snapped him back to his facing whenever he moved.
        let heading = p.heading;
        let with_ball = w.ball.state == BallState::Held(i);
        transform.rotation = Quat::from_rotation_y(heading + PI);

        // The move script the sim chose, on the VM's own clock
        // (crate::channel): a shot, dunk, lay-up, pump fake, pass or
        // catch shows the script's clip at the script's frame, with its
        // blends. Once the script has ended the ordinary animation is
        // back, whatever the sim's action still says.
        let mut script_over = false;
        // A block jump's script on the sim's own clock of it.
        if let Action::Jumping { block: Some(j), .. } = p.action
            && let Some(shown) = channel::block_shown(&clips, &j)
        {
            channel::apply(&mut animator, &shown, heading);
            continue;
        }
        // A trick's or a reaction's script on the sim's own clock of it.
        if let Some(shown) = channel::script_or_stand_shown(&clips, &p.action, p.stand.as_ref()) {
            channel::apply(&mut animator, &shown, heading);
            continue;
        }
        // A shot's, dunk's, lay-up's, pass's or catch row's script as the
        // simulation runs it (the clip it holds the ball on).
        if let Some(shown) = channel::sim_shown(&clips, w, i) {
            channel::apply(&mut animator, &shown, heading);
            continue;
        }
        if let Some(s) = channel::scripted(w, i) {
            match channel::shown(&clips, &s, p.gait_channel.trick_side == 1) {
                Some(channel::Channel::Pose(shown)) => {
                    channel::apply(&mut animator, &shown, heading);
                    continue;
                }
                Some(channel::Channel::Over) => script_over = true,
                None => {}
            }
        }
        // Other actions: their clips play in place, sped up to the
        // action's length (`Some`), or at the clip's own rate for a script
        // the disc or the executable does not carry.
        let mut mirrored = false;
        let action = match p.action {
            _ if script_over => None,
            Action::Shooting {
                gamebreaker: 0,
                fade: true,
                ..
            } => Some((clip::FADEAWAY, None, 1.0)),
            Action::Shooting { gamebreaker: 0, .. } => {
                // Each move plays one of its own clip names: the shot slot
                // routes through the shared shot set's script id
                // (`0x43d6c0`, `moves::SHOTS_SET`) and then into the 27
                // shot-script bodies read by `scripts::SCRIPT_BLENDS`. The
                // variant picks among the script's own blend targets.
                Some((
                    clip::move_selection(
                        &clips,
                        &moves,
                        &disc,
                        i,
                        street_assets::moves::Kind::Shot,
                        p.action_buttons,
                        2,
                        0,
                    )
                    .map(|selected| {
                        mirrored = selected.mirrored;
                        selected.name
                    })
                    .unwrap_or(clip::SHOT),
                    None,
                    1.0,
                ))
            }
            Action::Shooting {
                gamebreaker: 1,
                hoop,
                ..
            } => Some(if w.is_long_range(p.position, hoop) {
                (clip::GAMEBREAKER_LONG, None, 1.0)
            } else {
                (clip::GAMEBREAKER_SHORT, None, 1.0)
            }),
            Action::Shooting { .. } => Some((clip::GAMEBREAKER_2, None, 1.0)),
            // The putback's clock is the sim's own (no script row).
            Action::Dunking {
                style: DunkStyle::Putback,
                ..
            } => Some((clip::PUTBACK, Some(tuning::PUTBACK_TIME), 1.5)),
            Action::Dunking { .. } => {
                // The shared set's eleven dunks (`0x43d5d0`, keyed on the
                // two-turbo pair) each play their own script body's blend
                // targets (`scripts::DUNK_BLENDS`); slot per player, variant
                // among the script's own blend targets.
                let level = (p.action_buttons & street_sim::button::TURBOS)
                    .count_ones()
                    .max(1) as u8
                    + 1;
                Some((
                    clip::move_selection(
                        &clips,
                        &moves,
                        &disc,
                        i,
                        street_assets::moves::Kind::Dunk,
                        p.action_buttons,
                        level,
                        0,
                    )
                    .map(|selected| {
                        mirrored = selected.mirrored;
                        selected.name
                    })
                    .unwrap_or(clip::DUNK),
                    None,
                    1.0,
                ))
            }
            Action::Layup { .. } => Some((clip::LAYUP, None, 1.0)),
            Action::PumpFake { .. } => Some((clip::PUMP_FAKE, None, 1.0)),
            // A dive plays on its script's clock ([`channel::script_shown`]);
            // the clip stands in only without the programs.
            Action::Dive { .. } => Some((clip::DIVE, None, 1.0)),
            Action::Jumping { high: true, .. } => {
                Some((clip::TURBO_BLOCK, Some(tuning::JUMP_TIME), 1.5))
            }
            Action::Passing { .. } => {
                let name = match sim.pass_style[i] {
                    PassStyle::Kick => clip::KICK_PASS,
                    PassStyle::Heezay => clip::HEEZAY,
                    PassStyle::Glass => clip::GLASS,
                    PassStyle::Oop => clip::LOB,
                    PassStyle::GiveAndGo => clip::GIVE_AND_GO,
                    PassStyle::Normal => clip::PASS,
                };
                // A pass without a row (a handoff, a lob to a leaper,
                // the trick passes): the view's own clip.
                Some((name, Some(tuning::PASS_TIME), 2.5))
            }
            // Shown on its script's clock above ([`channel::script_shown`]).
            Action::Scoop { .. } => None,
            // The counter stance plays on its script's clock above
            // ([`channel::script_shown`]); the clip stands in only without
            // the programs.
            Action::Stealing { counter: true, .. } => {
                Some((clip::COUNTER, Some(tuning::STEAL_TIME), 2.5))
            }
            Action::Stealing { swipe: Some(s), .. } => {
                // The swipe's own clip and side (`street_sim::swipe`), over
                // its script's swipe.
                let span = f32::from(s.span()) * street_sim::TICK_SECONDS;
                match clips
                    .timeline
                    .animations
                    .get(usize::from(s.record))
                    .and_then(|a| Some((a.name.as_deref()?, a.mirrored)))
                {
                    Some((name, side)) => {
                        mirrored = side;
                        Some((name, Some(span), 2.5))
                    }
                    None => Some((clip::STEAL, Some(span), 2.5)),
                }
            }
            Action::Stealing { .. } => Some((clip::STEAL, Some(tuning::STEAL_TIME), 2.5)),
            Action::Trick { id, .. } => {
                // A clip the disc does not carry: the script's target by
                // name, over the script's own length.
                let value_class = usize::from(id.level).min(4);
                let name = clip::trick_target(&clips, id.script, p.gait_channel.trick_side)
                    .and_then(|target| {
                        mirrored = target.mirrored;
                        target.name.as_deref()
                    })
                    .unwrap_or(clip::TRICKS[value_class][0]);
                Some((name, Some(street_sim::player::trick_seconds(id)), 2.0))
            }
            Action::Stumbling { duration, .. } => Some((
                clip::STUMBLES[i % clip::STUMBLES.len()],
                Some(duration),
                1.5,
            )),
            Action::Jumping { .. } if w.ball.state == BallState::Loose => {
                Some((clip::REBOUND, Some(tuning::JUMP_TIME), 2.5))
            }
            Action::Jumping { .. } => Some((clip::BLOCK, Some(tuning::JUMP_TIME), 2.5)),
            // A tip-off or rebound script the channel cannot show (no
            // program or clip).
            Action::TipJump { .. } | Action::ReboundJump { .. } => {
                Some((clip::REBOUND, Some(tuning::JUMP_TIME), 2.5))
            }
            Action::None => None,
        };
        let just_started = p.action != Action::None && p.action.elapsed() <= 0.02;
        if let Some((name, seconds, max_speed)) = action {
            let clip = clips.get(name);
            animator.play_with_mirror_and_heading_restart(
                &clip,
                0.1,
                false,
                true,
                mirrored,
                Some(heading),
                just_started,
            );
            animator.current.speed = match seconds {
                Some(seconds) => (clip.seconds() / seconds).clamp(1.0, max_speed),
                None => 1.0,
            };
            continue;
        }
        // The winners' outros at the buzzer, one each.
        if let Phase::Over { winner } = w.phase
            && winner == p.team
            && p.gait == Gait::Stand
        {
            animator.play_with_mirror_and_heading(
                &clips.get(clip::OUTROS[i % clip::OUTROS.len()]),
                0.3,
                false,
                false,
                false,
                Some(heading),
            );
            continue;
        }
        let defending = match w.ball.state {
            BallState::Held(h) => w.team_of(h) != p.team,
            _ => false,
        };
        let (name, top_speed) = match (p.gait, with_ball) {
            (Gait::Stand, false) if defending => (clip::DEFEND, 0.0),
            (Gait::Stand, false) => (clip::STAND, 0.0),
            (Gait::Walk, false) => (clip::WALK, tuning::WALK_SPEED),
            (Gait::Run, false) => (clip::RUN, tuning::RUN_SPEED),
            (Gait::Turbo, false) => (clip::TURBO, tuning::TURBO_SPEED),
            (Gait::Stand, true) => (clip::STAND_BALL, 0.0),
            (Gait::Walk, true) => (clip::WALK_BALL, tuning::WALK_SPEED),
            (Gait::Run, true) => (clip::RUN_BALL, tuning::RUN_SPEED),
            (Gait::Turbo, true) => (clip::TURBO_BALL, tuning::TURBO_SPEED),
        };
        // Moving, the original's gait record picks the clip (its primary
        // row, or on the live left hand its 0xf8 alternate); the generic
        // walk and run stand in
        // without the records or for a row missing from the disc.
        let native = p
            .gait_channel
            .record
            .filter(|_| p.gait != Gait::Stand)
            .and_then(|_| {
                let row = clips
                    .timeline
                    .animations
                    .get(usize::from(p.gait_channel.record_row()))?;
                Some((clips.clips.get(row.name.as_deref()?)?, row.mirrored))
            });
        let (clip, mirrored) = native.clone().unwrap_or_else(|| (clips.get(name), false));
        // A change from one gait record's row to another blends as the
        // simulation does (`street_sim::pose::GAIT_BLEND` updates, the old
        // row's clip held at its frame); the stand-ins fade over the same
        // span.
        let g = &p.gait_channel;
        let held_from = native
            .as_ref()
            .filter(|_| (1..=street_sim::pose::GAIT_BLEND).contains(&g.blend_counter))
            .and(g.blend_from)
            .and_then(|(row, time)| {
                let row = clips.timeline.animations.get(usize::from(row))?;
                let from = clips.clips.get(row.name.as_deref()?)?.clone();
                let total = from.frames() as f32;
                let frame = if total > 0.0 { time % total } else { 0.0 };
                Some((from, row.mirrored, frame))
            });
        if let Some(from) = held_from {
            let blend = street_sim::pose::GAIT_BLEND;
            let total = clip.frames() as f32;
            let shown = channel::Shown {
                clip: clip.clone(),
                mirrored,
                frame: if total > 0.0 { g.phase % total } else { 0.0 },
                blend: Some(channel::ShownBlend {
                    from: Some(from),
                    weight: (blend + 1 - g.blend_counter) as f32 / blend as f32,
                }),
                callbacks: Vec::new(),
            };
            channel::apply(&mut animator, &shown, heading);
            animator.current.looping = true;
            animator.current.in_place = false;
            continue;
        }
        animator.play_with_mirror_and_heading(
            &clip,
            street_sim::pose::GAIT_BLEND as f32 / 60.0,
            true,
            false,
            mirrored,
            Some(heading),
        );
        if top_speed > 0.0 && p.gait != Gait::Stand {
            let total = clip.frames() as f32;
            if total > 0.0 {
                animator.current.frame = p.gait_channel.phase % total;
            }
            // The simulation's channel owns the frame; the render clock
            // must not advance it a second time.
            animator.current.speed = 0.0;
        } else {
            animator.current.speed = 1.0;
        }
    }
}

/// A staged script on screen: a Level 2 gamebreaker the simulation plays,
/// or the match intro before it starts.
pub(crate) struct Staging<'a> {
    pub sequence: Option<&'a street_sim::gamebreaker::Sequence>,
    pub plan: &'a street_sim::gamebreaker::SequencePlan,
    pub script: usize,
    pub frame: f32,
    pub actors: [Option<usize>; 7],
    pub mirrored: bool,
    /// The basket the script plays toward.
    pub hoop: usize,
    /// Only the cast is on screen (an intro).
    pub cast_only: bool,
    /// The script camera on screen: its id and the frame of its track.
    pub camera: (u8, f32),
}

/// A staged actor this frame: where, facing which way, and the clip of its
/// script's animation record (the handed one when mirrored) at its frame.
pub(crate) struct StagedPose {
    pub at: Vec3,
    pub heading: f32,
    pub name: String,
    pub mirrored: bool,
    pub frame: f32,
    /// Native finite/loop choice; synthetic metadata keeps the older fallback.
    pub looping: Option<bool>,
}

pub(crate) fn staging<'a>(
    w: &'a street_sim::World,
    intro: Option<&'a crate::intro::MatchIntro>,
) -> Option<Staging<'a>> {
    if let Some(s) = w.sequence {
        let plan = w.staged.as_ref()?.plan(s.script)?;
        return Some(Staging {
            sequence: w.sequence.as_ref(),
            plan,
            script: s.script,
            frame: s.pose_frame() as f32,
            actors: s.actors,
            mirrored: s.mirrored(),
            hoop: street_sim::target_hoop(s.team),
            cast_only: false,
            camera: plan.camera_at(s.frame as f32, s.held as f32, s.resume_pending),
        });
    }
    let intro = intro?;
    let part = intro.current.as_ref()?;
    let (frame, camera, camera_frame) = intro.shown()?;
    Some(Staging {
        sequence: None,
        plan: &part.plan,
        script: part.script,
        frame,
        actors: part.actors,
        mirrored: part.mirrored,
        hoop: part.hoop,
        cast_only: part.cast_only,
        camera: (camera, camera_frame),
    })
}

impl Staging<'_> {
    pub(crate) fn pose(&self, player: usize, clips: &GameClips) -> Option<StagedPose> {
        let role = self.actors.iter().position(|a| *a == Some(player))?;
        // A role on its finishing body is the simulation's dunker: drawn
        // on his own place and his body's clip (`channel::sim_shown`).
        if self
            .sequence
            .is_some_and(|s| s.finishing & (1 << role) != 0)
        {
            return None;
        }
        let pose = self.plan.pose_at(self.frame)[role]?;
        let anim = clips.timeline.animation(pose.animation, self.mirrored)?;
        let phase = self
            .sequence
            .and_then(|sequence| self.plan.runtime_display_phase(sequence, role))
            .or_else(|| self.plan.display_phase_at(role, self.frame));
        let x = if self.mirrored {
            -pose.position[0]
        } else {
            pose.position[0]
        };
        Some(StagedPose {
            at: Vec3::new(x, pose.position[1].max(0.0), pose.position[2]),
            heading: street_sim::gamebreaker::script_heading(pose.heading, self.mirrored),
            name: anim.name.clone()?,
            mirrored: anim.mirrored,
            frame: phase.map_or(pose.animation_frame, |phase| phase.display_frame),
            looping: phase.map(|phase| phase.looping),
        })
    }

    /// Where the script has the ball: in a role's hand, or between two
    /// ends of a throw (hands, the rim, or a toss up high).
    fn ball(&self, w: &street_sim::World, hand_of: impl Fn(usize) -> Option<Vec3>) -> Vec3 {
        use street_sim::gamebreaker::{BallEnd, BallPlace};
        let player = |role: u8| self.actors.get(usize::from(role)).copied().flatten();
        let rim = Vec3::from_array(w.hoops[self.hoop].rim);
        let end = |e: BallEnd| match e {
            BallEnd::Actor(r) => player(r).and_then(&hand_of).unwrap_or(rim),
            BallEnd::Rim => rim + Vec3::Y * 1.0,
            BallEnd::Above(r) => player(r).and_then(&hand_of).unwrap_or(rim) + Vec3::Y * 12.0,
        };
        match self.plan.ball_at(self.frame as u32) {
            BallPlace::Held(r) => end(BallEnd::Actor(r)),
            BallPlace::Flight { from, to, t } => {
                let (a, b) = (end(from), end(to));
                let arc = 4.0 * t * (1.0 - t) * (2.0 + 0.1 * a.distance(b));
                a.lerp(b, t) + Vec3::Y * arc
            }
        }
    }
}

/// Native held-ball attachment geometry (`FUN_0014da20` branch 0).
///
/// Computes the live attachment position from the evaluated hand bone translation and
/// rotation, body scale x, ball radius, and hand mode (1 for right hand, 2 for left hand).
/// The ball in the referee's hands: bones 15 and 19 by his clip's hand
/// flags (bits 2 and 3 of the state word), placed as a player's held ball
/// ([`native_held_attachment`]).
fn referee_hold(
    animator: &Animator,
    rig: &Rig,
    joints: &Query<&GlobalTransform>,
    hand: usize,
    radius: f32,
) -> Option<Vec3> {
    let current = &animator.current;
    let mask = current
        .clip
        .state_at_mirrored(current.frame as usize, current.mirrored)
        & 0xc;
    let joint = |bone: usize| {
        let entity = rig.joints.get(bone).or_else(|| rig.joints.get(hand))?;
        Some(joints.get(*entity).ok()?.to_scale_rotation_translation())
    };
    let (at, rotation, scale, mode) = if mask == 12 {
        let (sl, rl, pl) = joint(15)?;
        let (sr, rr, pr) = joint(19)?;
        ((pl + pr) * 0.5, rl.slerp(rr, 0.5), (sl.x + sr.x) * 0.5, 3)
    } else if mask == 4 {
        let (s, r, p) = joint(15)?;
        (p, r, s.x, 2)
    } else {
        let (s, r, p) = joint(19)?;
        (p, r, s.x, 1)
    };
    Some(native_held_attachment(
        at,
        rotation,
        scale.max(0.1),
        radius,
        mode,
    ))
}

pub fn native_held_attachment(
    hand_translation: Vec3,
    hand_rotation: Quat,
    body_scale_x: f32,
    ball_radius: f32,
    mode: u8,
) -> Vec3 {
    let z = if mode == 2 {
        -ball_radius
    } else if mode == 3 {
        0.0
    } else {
        ball_radius
    };
    let local = Vec3::new(0.4 * body_scale_x, 0.0, z);
    hand_translation + hand_rotation * local
}

/// Put the ball where the simulation has it. While it is held the renderer
/// decides where exactly: in the hand during a shot or dunk, otherwise
/// dribbled from the active hand, bouncing once per stride cycle.
pub fn sync_ball(
    sim: Res<Sim>,
    time: Res<Time>,
    hand: Res<HandJoint>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    players: Query<(&SimPlayer, &Animator, &Rig)>,
    (tape, referee): (
        Option<Res<crate::tape::Tape>>,
        Query<(&Animator, &Rig), With<crate::referee::RefereeView>>,
    ),
    joints: Query<&GlobalTransform>,
    mut balls: Query<&mut Transform, With<BallView>>,
    mut ball_shown: Query<&mut Visibility, With<BallView>>,
) {
    let _ = &time;
    let shown = if game_ended(&sim.world, tape.as_deref()) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut ball_shown {
        visibility.set_if_neq(shown);
    }
    // A replay: the recorded ball, blended.
    if let Some((a, b, t)) = tape.as_deref().and_then(|tape| tape.shown()) {
        if let Some(from) = a.ball {
            let at = from.lerp(b.ball.unwrap_or(from), t);
            for mut transform in &mut balls {
                transform.translation = at;
            }
        }
        return;
    }
    let ball = &sim.world.ball;
    // The referee holds it until his toss (FUN_00141fd8's FUN_0014cb28):
    // in his hands as his clip's flags say, as a player's held ball.
    if matches!(sim.world.opening_toss, street_sim::OpeningToss::Waiting(_))
        && let Ok((animator, rig)) = referee.single()
        && let Some(at) = referee_hold(animator, rig, &joints, hand.0, ball.radius)
    {
        for mut transform in &mut balls {
            transform.translation = at;
        }
        return;
    }
    if sim.world.sequence.is_some() {
        for mut transform in &mut balls {
            transform.translation = Vec3::from_array(ball.position);
        }
        return;
    }
    let hand_of = |i: usize| {
        players
            .iter()
            .find(|(p, _, _)| p.0 == i)
            .and_then(|(_, _, rig)| Some(joints.get(rig.joints[hand.0]).ok()?.translation()))
    };
    if let Some(at) = staging(&sim.world, intro.as_deref()).map(|s| s.ball(&sim.world, hand_of)) {
        for mut transform in &mut balls {
            transform.translation = at;
        }
        return;
    }
    let position = match ball.state {
        BallState::Held(i) => players
            .iter()
            .find(|(p, _, _)| p.0 == i)
            .and_then(|(_, animator, rig)| {
                let p = &sim.world.players[i];
                let clip_state = animator
                    .current
                    .clip
                    .state_at_mirrored(animator.current.frame as usize, animator.current.mirrored);
                let mask = clip_state & 0xc;
                let (hand_at, hand_rot, scale_x, mode) = if mask == 12 {
                    let bone_l = rig.joints.get(15).copied().unwrap_or(rig.joints[hand.0]);
                    let bone_r = rig.joints.get(19).copied().unwrap_or(rig.joints[hand.0]);
                    let jl = joints.get(bone_l).ok()?;
                    let jr = joints.get(bone_r).ok()?;
                    let (sl, rl, pl) = jl.to_scale_rotation_translation();
                    let (sr, rr, pr) = jr.to_scale_rotation_translation();
                    ((pl + pr) * 0.5, rl.slerp(rr, 0.5), (sl.x + sr.x) * 0.5, 3u8)
                } else {
                    let mode = if mask == 4 { 2u8 } else { 1u8 };
                    let bone = if mode == 2 {
                        rig.joints.get(15).copied().unwrap_or(rig.joints[hand.0])
                    } else {
                        rig.joints.get(19).copied().unwrap_or(rig.joints[hand.0])
                    };
                    let joint_transform = joints.get(bone).ok()?;
                    let (scale, hand_rot, hand_at) =
                        joint_transform.to_scale_rotation_translation();
                    (hand_at, hand_rot, scale.x, mode)
                };
                // The simulation holds the ball as the original's ball
                // update does (`FUN_0014e060`, [`street_sim::held`]): in the
                // hand a script's frame holds out, eased through a blend,
                // and on a gait clip the dribble between its release and
                // catch frames (`street_sim::dribble`, checked against the
                // played captures). The bounce below stands in only
                // without the disc's animations.
                if sim.world.channel(i).is_some() {
                    return Some(Vec3::from_array(ball.position));
                }
                if p.action != Action::None {
                    return Some(native_held_attachment(
                        hand_at,
                        hand_rot,
                        scale_x.max(0.1),
                        ball.radius,
                        mode,
                    ));
                }
                // One dribble per animation cycle, or synchronized gait phase
                let phase = if p.gait == Gait::Stand {
                    (p.gait_channel.phase / 6.0).fract()
                } else {
                    animator.current.frame / animator.current.clip.frames().max(1) as f32
                };
                let lift = (PI * phase).cos().abs();
                let low = ball::RADIUS;
                let high = (hand_at.y - 0.3).max(low);
                Some(Vec3::new(hand_at.x, low + (high - low) * lift, hand_at.z))
            })
            .unwrap_or(Vec3::from_array(ball.position)),
        _ => Vec3::from_array(ball.position),
    };
    for mut transform in &mut balls {
        transform.translation = position;
    }
}

pub fn spawn_indicator(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let ring = meshes.add(Annulus::new(1.3, 1.7));
    for (k, [r, g, b]) in SEAT_COLOURS.into_iter().enumerate() {
        commands.spawn((
            Mesh3d(ring.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(r, g, b, 0.85),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_rotation(Quat::from_rotation_x(-PI / 2.0)),
            Visibility::Hidden,
            Indicator(k),
            DespawnOnExit(Screen::Playing),
        ));
    }
}

/// How high over the floor the badges float (court units; the rim is at
/// 11.8).
const BADGE_HEIGHT: f32 = 10.5;
/// The badges' size on screen, pixels.
const BADGE_SIZE: Vec2 = Vec2::new(25.0, 26.0);

/// Keep each person's ring under their player, and with several people
/// their numbered badge over the player's head (all hidden in demo mode).
pub fn sync_indicator(
    sim: Res<Sim>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    tape: Option<Res<crate::tape::Tape>>,
    control: Res<Control>,
    mut rings: Query<(&Indicator, &mut Transform, &mut Visibility)>,
    mut badges: Query<(&Badge, &mut Node, &mut Visibility), Without<Indicator>>,
    cameras: Query<(&Camera, &GlobalTransform), With<SidelineCamera>>,
) {
    // Whom person `k` (1P = 0) drives, here or online elsewhere.
    let people = control.people();
    let seat = |k: usize| people.iter().find(|p| p.0 == k).map(|p| p.2);
    let ended = game_ended(&sim.world, tape.as_deref());
    let staged = sim.world.sequence.is_some() || !crate::intro::finished(intro) || ended;
    for (Indicator(k), mut transform, mut visibility) in &mut rings {
        *visibility = match seat(*k).filter(|_| !staged) {
            Some(s) => {
                let p = &sim.world.players[s];
                transform.translation = Vec3::new(p.position[0], 0.05, p.position[1]);
                Visibility::Visible
            }
            None => Visibility::Hidden,
        };
    }
    let camera = cameras.single().ok();
    for (Badge(k), mut node, mut visibility) in &mut badges {
        let at = seat(*k)
            .filter(|_| control.several() && !ended)
            .zip(camera)
            .and_then(|(s, (camera, eye))| {
                let p = &sim.world.players[s];
                let head = Vec3::new(p.position[0], BADGE_HEIGHT, p.position[1]);
                camera.world_to_viewport(eye, head).ok()
            });
        *visibility = match at {
            Some(at) => {
                node.left = Val::Px(at.x - BADGE_SIZE.x / 2.0);
                node.top = Val::Px(at.y - BADGE_SIZE.y);
                Visibility::Visible
            }
            None => Visibility::Hidden,
        };
    }
}

/// The camera between 60 Hz updates: the manager and the pose it published,
/// which the renderer reads every frame without advancing it.
#[derive(Resource)]
pub struct ActiveCamera {
    pub eye: Vec3,
    pub target_focus: Vec3,
    pub lens: CameraLens,
    /// The active helper's `+0x1b0` ([`StagedView::roll`]; 0 but for the
    /// staged helper).
    pub roll: f32,
    pub manager: CameraManager,
}

impl Default for ActiveCamera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 16.0, -38.0),
            target_focus: Vec3::ZERO,
            lens: CameraLens::Scalar(CAMERA_MODE_ROWS[0].row_word7),
            roll: 0.0,
            manager: CameraManager::new(0),
        }
    }
}

/// One manager update and its published pose.
pub fn update_camera_step(tick: &CameraTick, active_camera: &mut ActiveCamera) {
    let before = active_camera.manager.active_helper;
    let (eye, target_focus, lens) = active_camera.manager.update(tick);
    if active_camera.manager.active_helper != before {
        debug!(
            "camera helper {:?} -> {:?} at tick {}, eye {eye}",
            before, active_camera.manager.active_helper, tick.world.tick
        );
    }
    trace!(
        "camera tick {} {:?} eye {eye} at {target_focus} lens {lens:?} ball {:?} {:?}",
        tick.world.tick,
        active_camera.manager.active_helper,
        tick.world.ball.position,
        tick.world.opening_toss,
    );
    active_camera.eye = eye;
    active_camera.target_focus = target_focus;
    active_camera.lens = lens;
    active_camera.roll = match (
        active_camera.manager.active_helper,
        active_camera.manager.staged_pose,
    ) {
        (CameraHelperKind::Staged, Some(p)) => p.roll,
        _ => 0.0,
    };
}

/// The row the camera manager puts on (`FUN_003038b0` at the match's
/// start and `FUN_00304a38` each update, through `FUN_002f7bb0`): Wide (5)
/// for the front end's mode 3 and for mode 4 with more than one user
/// (`FUN_001db248`), else the saved Camera Type (`config +0x12c >> 6 & 7`:
/// Street, Court, Slide, Solid, Wide as rows 0, 1, 2, 3, 5), any other
/// value Street. Those are the only calls of `FUN_002f7bb0` and the row
/// word `+0x1f0` has no other writer (the constructor's 0 at `0x2f7b68`):
/// the original never shows Behind (row 4), so a saved 4 is Street here.
pub fn native_camera_row(front_end_mode: u32, users: usize, saved: u8) -> usize {
    if front_end_mode == 3 || (front_end_mode == 4 && users > 1) {
        return 5;
    }
    match saved {
        1 | 2 | 3 | 5 => usize::from(saved),
        _ => 0,
    }
}

/// The 60 Hz camera update: gathers the manager's inputs from the match.
#[allow(clippy::too_many_arguments)]
pub fn step_camera(
    sim: Res<Sim>,
    options: Option<Res<crate::options::Options>>,
    clips: Res<GameClips>,
    intro: Option<Res<crate::intro::MatchIntro>>,
    replay: Option<Res<Replay>>,
    tape: Option<Res<crate::tape::Tape>>,
    gamebreaker_task: Option<Res<crate::tape::GamebreakerTask>>,
    control: Option<Res<Control>>,
    setup: Option<Res<setup::MatchSetup>>,
    disc: Option<Res<Disc>>,
    (players, balls): (
        Query<(&SimPlayer, &Transform, &Rig)>,
        Query<&Transform, With<BallView>>,
    ),
    joints: Query<&GlobalTransform>,
    events: Option<Res<crate::game::TickEvents>>,
    running: Option<Res<crate::story::Running>>,
    cheats: Option<Res<setup::MatchCheats>>,
    mut active_camera: ResMut<ActiveCamera>,
) {
    // A staged script's replay (mode 7) stays on the script's own cameras:
    // the command's, at the script manager's replay frame (`FUN_00336868`);
    // the camera's replay start and end (`FUN_003030c0`, `FUN_00303248`)
    // return for it.
    let rerun = tape
        .as_deref()
        .filter(|t| t.replaying() && t.mode == crate::tape::mode::STAGED)
        .and_then(|t| t.staged);
    let script_camera = match rerun {
        Some(r) => Some((r.script, r.mirrored, r.camera, r.shown)),
        None => staging(&sim.world, intro.as_deref())
            .map(|s| (s.script, s.mirrored, s.camera.0, s.camera.1)),
    };
    // The script's actors as the game binds them (`FUN_00337588`), for a
    // following camera: the drawn player's root across, its bone 0 or 21
    // up (`FUN_00331d08`), or the ball's place.
    let bound = staging(&sim.world, intro.as_deref()).map(|s| s.actors);
    let follow = |actor: i16, part: u8| -> Vec3 {
        if actor < 0 {
            return balls.iter().next().map_or(Vec3::ZERO, |t| t.translation);
        }
        let player = bound.and_then(|b| b.get(actor as usize).copied().flatten());
        let Some((root, rig)) = player
            .and_then(|p| players.iter().find(|(sp, _, _)| sp.0 == p))
            .map(|(_, t, r)| (t.translation, r))
        else {
            return Vec3::ZERO;
        };
        let bone = |i: usize| {
            rig.joints
                .get(i)
                .and_then(|&j| joints.get(j).ok())
                .map(|g| g.translation().y)
        };
        let y = match part {
            0 => bone(0).unwrap_or(root.y),
            1 => bone(21).unwrap_or(root.y),
            _ => 0.0,
        };
        Vec3::new(root.x, y, root.z)
    };
    let last_scalar = active_camera
        .manager
        .staged_pose
        .map_or(CAMERA_MODE_ROWS[0].row_word7, |p| p.scalar);
    let staged = if let Some((script, mirrored, id, frame)) = script_camera
        && let Some(timeline) = clips.timeline.scripts.get(script)
        && let Some(camera) = timeline.camera(frame, mirrored, id)
    {
        Some(staged_pose(&camera, follow, last_scalar))
    } else {
        None
    };
    let court = setup.as_ref().zip(disc.as_ref()).and_then(|(setup, disc)| {
        disc.courts
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(&setup.court))
            .map(|c| c.index)
    });
    // Each player's evaluated bones 21 and 10 (FUN_00305488's 0x15 and 10).
    let mut bodies = vec![None; sim.world.players.len()];
    for (player, _, rig) in &players {
        let bone = |i: usize| Some(joints.get(*rig.joints.get(i)?).ok()?.translation());
        if let Some(slot) = bodies.get_mut(player.0) {
            *slot = bone(21).zip(bone(10)).map(|(head, chest)| [head, chest]);
        }
    }
    let seated: Vec<bool> = (0..sim.world.players.len())
        .map(|i| control.as_ref().is_some_and(|c| c.seat_of(i).is_some()))
        .collect();
    let tick = CameraTick {
        world: &sim.world,
        events: events.as_ref().map_or(&sim.world.events, |e| &e.events),
        staged,
        intro_playing: intro.as_ref().is_some_and(|i| i.current.is_some()),
        replay_playing: rerun.is_none()
            && (replay.as_ref().is_some_and(|r| r.playing.is_some())
                || tape.as_ref().is_some_and(|t| t.replaying())),
        gamebreaker_over: gamebreaker_task.as_ref().is_some_and(|t| t.ended),
        camera_mode: native_camera_row(
            running.as_deref().map_or(2, crate::game::front_end_mode),
            setup.as_ref().map_or(1, |s| s.seats.len().max(1)),
            options.as_ref().map_or(0, |o| o.0.camera),
        ),
        attract: control.as_ref().is_some_and(|c| c.demo),
        court,
        bodies: &bodies,
        lesson: running
            .as_ref()
            .is_some_and(|r| matches!(r.0, Some(crate::story::Run::School(_)))),
        explosive_rims: cheats.as_ref().is_some_and(|c| c.0.explosive_rims()),
        seated: &seated,
    };
    update_camera_step(&tick, &mut active_camera);
}

/// The renderer's half of the camera: the published pose, with the lens
/// scalar converted through the native projection and the Widescreen row's
/// picture (or the run's `--widescreen`).
pub fn follow_camera(
    options: Option<Res<crate::options::Options>>,
    over: Option<Res<crate::options::PictureOverride>>,
    active_camera: Res<ActiveCamera>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<SidelineCamera>>,
) {
    let picture = crate::options::picture(options.as_deref(), over.as_deref());
    let fov = match active_camera.lens {
        CameraLens::Scalar(scalar) => lens_vertical_fov(scalar, picture),
        CameraLens::Fov(fov) => fov,
    };
    for (mut transform, mut projection) in &mut cameras {
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = fov;
        }
        *transform = Transform::from_translation(active_camera.eye)
            .looking_at(active_camera.target_focus, Vec3::Y);
        // `FUN_003046c0` turns the view by -roll about its axis: the
        // camera turns by +roll, its up toward its right.
        transform.rotate_local_z(-active_camera.roll);
    }
}

pub fn spawn_hud(
    mut commands: Commands,
    art: Option<Res<crate::art::Art>>,
    setup: Res<setup::MatchSetup>,
    disc: Res<Disc>,
    look: Option<Res<crate::look::Look>>,
    cheats: Res<crate::setup::MatchCheats>,
) {
    if cheats.0.no_hud() {
        return;
    }
    // 1P..6P's badges, placed over their players by `sync_indicator`.
    if let Some(art) = art.as_ref() {
        for (k, badge) in art.badges.iter().take(MAX_PEOPLE).enumerate() {
            commands.spawn((
                ImageNode::new(badge.clone()).with_mode(NodeImageMode::Stretch),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(BADGE_SIZE.x),
                    height: Val::Px(BADGE_SIZE.y),
                    ..default()
                },
                Visibility::Hidden,
                Badge(k),
                DespawnOnExit(Screen::Playing),
            ));
        }
    }
    // With the game's fonts and HUD art, `hud::draw` draws the rest.
    if look.is_some() && art.as_ref().is_some_and(|a| a.hud.is_some()) {
        return;
    }
    let logo = |side: usize| {
        art.as_ref().and_then(|art| {
            let team = disc.teams.iter().find(|t| t.index == setup.teams[side])?;
            art.logos
                .get(usize::try_from(team.logo).ok().unwrap_or(0))
                .cloned()
        })
    };
    let left = logo(0);
    let right = logo(1);
    // The score bug: logos either side of the score, meters below.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            DespawnOnExit(Screen::Playing),
        ))
        .with_children(|bug| {
            bug.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            ))
            .with_children(|row| {
                if let Some(handle) = &left {
                    row.spawn((
                        ImageNode::new(handle.clone()),
                        Node {
                            height: Val::Px(34.0),
                            ..default()
                        },
                    ));
                }
                row.spawn((
                    Text::new(""),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(24.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    ScoreText,
                ));
                if let Some(handle) = &right {
                    row.spawn((
                        ImageNode::new(handle.clone()),
                        Node {
                            height: Val::Px(34.0),
                            margin: UiRect::left(Val::Px(8.0)),
                            ..default()
                        },
                    ));
                }
            });
            bug.spawn((
                Text::new(""),
                TextFont {
                    font_size: bevy::text::FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.85)),
                MeterText,
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            ));
        });
    // The turbo bar and the play call, under the bug.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(112.0),
            left: Val::Px(16.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        TurboText,
        DespawnOnExit(Screen::Playing),
    ));
}

/// A bar of `width` characters, `fill` (0..1) of it full.
fn bar(fill: f32, width: usize) -> String {
    let full = ((fill.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("[{}{}]", "#".repeat(full), "-".repeat(width - full))
}

/// Score, shot clock, each team's trick points and gamebreaker meter, and
/// the latest play.
pub fn update_hud(
    sim: Res<Sim>,
    teams: Res<TeamNames>,
    control: Res<Control>,
    play_by_play: Option<Res<crate::hud::PlayByPlay>>,
    mut score: Query<&mut Text, (With<ScoreText>, Without<MeterText>, Without<TurboText>)>,
    mut meters: Query<&mut Text, (With<MeterText>, Without<ScoreText>, Without<TurboText>)>,
    mut turbo: Query<&mut Text, (With<TurboText>, Without<ScoreText>, Without<MeterText>)>,
) {
    let w = &sim.world;
    let clock = match (w.rules.shot_clock, w.phase) {
        (Some(_), Phase::Live) => format!("   shot clock {:.0}", w.shot_clock.ceil()),
        _ => String::new(),
    };
    let ball = |team: usize| if w.possession == team { "* " } else { "  " };
    for mut t in &mut score {
        t.0 = format!(
            "{ball0}{name0}  {score0}  -  {score1}  {name1}{ball1}{clock}",
            ball0 = ball(0),
            name0 = teams.0[0],
            score0 = w.score[0],
            score1 = w.score[1],
            name1 = teams.0[1],
            ball1 = ball(1),
            clock = clock,
        );
    }
    let meter = |team: usize| {
        let t = &w.teams[team];
        let state = match (t.gamebreaker(), t.pocketed) {
            (0, true) => " (pocketed)".to_string(),
            (0, false) => String::new(),
            (level, _) => format!(" level {level} ready"),
        };
        format!(
            "{:<12} {:>7} pts  GB {}{state}",
            teams.0[team],
            t.trick_points,
            bar(t.meter as f32 / street::METER_FULL as f32, 12)
        )
    };
    for mut t in &mut meters {
        t.0 = format!("{}\n{}", meter(0), meter(1));
    }
    // The play-by-play is the rewrite's own (`--play-by-play`).
    let call = if sim.call.is_empty() || play_by_play.is_none() {
        String::new()
    } else {
        format!("  {}", sim.call)
    };
    for mut t in &mut turbo {
        t.0 = if control.demo || control.seats.is_empty() {
            call.trim_start().to_string()
        } else if control.several() {
            // Each person's turbo, numbered.
            let bars: Vec<String> = control
                .seats
                .iter()
                .map(|s| format!("{}P {}", s.number + 1, bar(w.players[s.player].turbo, 6)))
                .collect();
            format!("{}{}", bars.join("  "), call)
        } else {
            let player = control.seats[0].player;
            format!("turbo {}{}", bar(w.players[player].turbo, 10), call)
        };
    }
}

#[cfg(test)]
mod staged_display_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use std::sync::Arc;

    #[test]
    fn disc_staged_fractional_phase_reaches_the_displayed_hands() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = setup::load_clips(&mut disc.iso).unwrap();
        let skeleton = street_assets::player::skeleton(&mut disc.iso).unwrap();
        let mut config = crate::staged::config(&clips.timeline).unwrap();
        for plan in &mut config.plans {
            crate::staged::attach_playback(plan, &clips.timeline, &clips.clips).unwrap();
        }
        let config = Arc::new(config);
        // Use phases produced by actual disc plans, away from terminal wrap.
        // Seed an already-playing clip to isolate fractional display from
        // crossfade. Native VM startup and raw skeleton arithmetic stay open.
        let mut samples = Vec::new();
        'plans: for plan in &config.plans {
            for frame in 7..plan.frame_count {
                for (slot, pose) in plan.pose_at(frame as f32).into_iter().enumerate().take(6) {
                    let Some(pose) = pose else { continue };
                    let record = clips.timeline.animation(pose.animation, false).unwrap();
                    let clip = clips.clips.get(record.name.as_deref().unwrap()).unwrap();
                    let phase = pose.animation_frame.rem_euclid(clip.frames() as f32);
                    if phase.fract() != 0.0 && phase > 1.0 && phase < (clip.frames() - 2) as f32 {
                        samples.push((plan.script, frame, slot));
                        if samples.len() == 3 {
                            break 'plans;
                        }
                    }
                }
            }
        }
        assert_eq!(
            samples.len(),
            3,
            "disc plans must exercise fractional playback"
        );
        let mut display = World::new();
        display.insert_resource(Time::<()>::default());
        display.insert_resource(clips);
        display.insert_resource(disc);
        display.insert_resource(Replay::default());
        display.insert_resource(clip::PlayerMoves(vec![(0, Default::default())]));
        let joints: Vec<Entity> = skeleton
            .bones
            .iter()
            .map(|bone| {
                display
                    .spawn(
                        Transform::from_scale(Vec3::from_array(bone.scale))
                            .with_rotation(Quat::from_array(bone.rotation))
                            .with_translation(Vec3::from_array(bone.translation)),
                    )
                    .id()
            })
            .collect();
        let mut distinct_from_nearest = 0;
        let mut max_display_error = 0.0f32;
        let mut max_nearest_difference = 0.0f32;
        for (script, frame, slot) in samples {
            for team in [0, 1] {
                let mut sim = street_sim::World::new(
                    [
                        street_sim::Hoop {
                            rim: [36.95, 11.75, 0.0],
                            rim_radius: 0.75,
                        },
                        street_sim::Hoop {
                            rim: [-36.96, 11.75, 0.0],
                            rim_radius: 0.75,
                        },
                    ],
                    vec![street_sim::Player::new(team, [0.0, 0.0], 0.0)],
                    street_sim::Rules::default(),
                    1,
                );
                let mut actors = [None; 7];
                actors[slot] = Some(0);
                sim.staged = Some(config.clone());
                sim.sequence = Some(street_sim::gamebreaker::Sequence {
                    script,
                    shooter: 0,
                    team,
                    level: 2,
                    frame,
                    held: 0,
                    actors,
                    native_clocks: [None; 7],
                    resume_pending: false,
                    finishing: 0,
                });
                let staged = staging(&sim, None)
                    .unwrap()
                    .pose(0, display.resource::<GameClips>())
                    .unwrap();
                let clip = display
                    .resource::<GameClips>()
                    .clips
                    .get(&staged.name)
                    .unwrap();
                let phase = staged.frame.rem_euclid(clip.frames() as f32);
                eprintln!(
                    "staged display script={script} tick={frame} slot={slot} team={team} clip={} phase={phase}",
                    staged.name
                );
                let mut animator = Animator::new(clip.clone());
                animator.current.mirrored = staged.mirrored;
                let actor = display
                    .spawn((
                        SimPlayer(0),
                        Transform::default(),
                        Visibility::Inherited,
                        animator,
                        Rig::new(joints.clone(), &skeleton),
                    ))
                    .id();
                display.insert_resource(Sim {
                    world: sim,
                    inputs: vec![],
                    call: String::new(),
                    pass_style: vec![],
                });
                display.run_system_once(sync_players).unwrap();
                assert_eq!(display.get::<Animator>(actor).unwrap().current.frame, phase);
                display
                    .run_system_once(street_render::animation::pose_skeletons)
                    .unwrap();
                let mut matrices: Vec<Mat4> = Vec::new();
                for (index, bone) in skeleton.bones.iter().enumerate() {
                    let local = display.get::<Transform>(joints[index]).unwrap().to_matrix();
                    matrices.push(bone.parent.map_or(local, |parent| matrices[parent] * local));
                }
                for hand in [15, 19] {
                    let expected = clip
                        .joint_position(&skeleton, hand, phase, staged.mirrored, false)
                        .unwrap();
                    let nearest = clip
                        .joint_position(
                            &skeleton,
                            hand,
                            phase.round_ties_even(),
                            staged.mirrored,
                            false,
                        )
                        .unwrap();
                    let visible = matrices[hand].transform_point3(Vec3::ZERO);
                    max_display_error = max_display_error.max((visible - expected).length());
                    max_nearest_difference =
                        max_nearest_difference.max((visible - nearest).length());
                    assert!((visible - expected).length() < 1e-5);
                    distinct_from_nearest += usize::from((visible - nearest).length() > 1e-5);
                }
                display.despawn(actor);
            }
        }
        assert!(
            distinct_from_nearest > 0,
            "the regression must distinguish rounded display"
        );
        eprintln!(
            "staged display hands=12 max_fractional_error={max_display_error} max_nearest_difference={max_nearest_difference} distinct_from_nearest={distinct_from_nearest}"
        );
    }
}

#[cfg(test)]
mod camera_pose_tests {
    use super::*;

    /// Opt-in check of a staged Level 2's script cameras against the
    /// original's (STREET_L2_CAMPROBE: the driver's `camprobe` lines, the
    /// script manager's words +0x30.. with its state at word 8, its frame
    /// at word 10 and its replay clock at word 12, and the ten camera
    /// nodes' +0x20..: position, target, fov, heading). Each state's camera
    /// on screen: in play (3) camera 0 at the frame before the manager's;
    /// in the freeze (7) the freeze's camera, one more frame each update
    /// from the freeze's own; in the replay (5) the replay's camera at the
    /// clock before the update's step.
    #[test]
    fn staged_level2_cameras_follow_the_original() {
        use street_assets::timeline::Payload;
        let (Some(path), Some(iso)) = (
            std::env::var_os("STREET_L2_CAMPROBE"),
            std::env::var_os("STREET_ISO"),
        ) else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(iso).unwrap();
        let bank = street_assets::timeline::ScriptBank::read(&mut iso).unwrap();
        let word = |w: &serde_json::Value| w.as_u64().unwrap() as u32;
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        // The script: the one whose camera 0 record sits where node 0 does
        // when the first frame is set.
        let first = lines
            .iter()
            .find(|l| word(&l["script"][8]) == 3 && word(&l["script"][10]) == 1)
            .unwrap();
        let node0: Vec<f32> = (0..3)
            .map(|k| f32::from_bits(word(&first["nodes"][0][k])))
            .collect();
        let script = (0..bank.scripts.len())
            .find(|&s| {
                bank.scripts[s]
                    .camera(0.0, false, 0)
                    .is_some_and(|c| c.position == [node0[0], node0[1], node0[2]])
            })
            .expect("the probe's script");
        let timeline = &bank.scripts[script];
        // The replay command (op 0) and the freeze (op 2): camera at word 2.
        let command = |op: u16| {
            timeline.records.iter().find_map(|r| match &r.payload {
                Payload::Command(c) if c.dispatch == 1 && c.opcode == op => Some((
                    r.frame,
                    r.duration,
                    u32::from_le_bytes(c.bytes[8..12].try_into().unwrap()) as u8,
                    u32::from_le_bytes(c.bytes[16..20].try_into().unwrap()),
                )),
                _ => None,
            })
        };
        let hold = command(2).unwrap();
        let rerun = command(0).unwrap();
        let mut replay =
            street_sim::replay::StagedReplay::new(script, false, rerun.0, rerun.3 as u8, rerun.2);
        let fields = |c: &street_assets::timeline::Camera| {
            [
                c.position[0],
                c.position[1],
                c.position[2],
                c.target[0],
                c.target[1],
                c.target[2],
                c.fov,
                c.heading,
            ]
        };
        let (mut samples, mut exact, mut torn) = (0, 0, 0);
        let mut worst = (0.0_f32, String::new());
        let mut freeze = std::collections::BTreeSet::new();
        for l in &lines {
            let (state, frame) = (word(&l["script"][8]), word(&l["script"][10]));
            let clock = f32::from_bits(word(&l["script"][12]));
            let (id, time) = match state {
                3 if frame > 0 => (0, (frame - 1) as f32),
                7 => (hold.2, f32::NAN),
                5 => {
                    while replay.clock < clock {
                        replay.step();
                    }
                    if replay.clock != clock || clock == rerun.0 as f32 {
                        continue;
                    }
                    (rerun.2, replay.shown)
                }
                _ => continue,
            };
            let node = &l["nodes"][usize::from(id)];
            let want: Vec<f32> = (0..8).map(|k| f32::from_bits(word(&node[k]))).collect();
            let time = if time.is_nan() {
                // The freeze's count isn't in the probe: the frame whose
                // camera the node holds.
                let at = |f: u32| fields(&timeline.camera(f as f32, false, id).unwrap());
                if let Some(f) = (hold.0..hold.0 + hold.1).find(|&f| at(f)[..] == want[..]) {
                    freeze.insert(f);
                    f as f32
                } else if let Some(f) = (hold.0..hold.0 + hold.1)
                    .find(|&f| (0..8).all(|k| want[k] == at(f)[k] || want[k] == at(f + 1)[k]))
                {
                    f as f32
                } else {
                    samples += 1;
                    eprintln!("freeze node matches no frame: {want:?}");
                    continue;
                }
            } else {
                time
            };
            let camera = timeline.camera(time, false, id).unwrap();
            let got = fields(&camera);
            let error = got
                .iter()
                .zip(&want)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            // The probe's reads aren't atomic: a field read after the next
            // update has that update's value.
            let after = if state == 5 { replay.clock } else { time + 1.0 };
            let next = fields(&timeline.camera(after, false, id).unwrap());
            samples += 1;
            exact += usize::from(got[..] == want[..]);
            if (0..8).all(|k| want[k] == got[k] || want[k] == next[k]) {
                torn += usize::from(got[..] != want[..]);
            } else if error > worst.0 || worst.1.is_empty() {
                worst = (
                    error,
                    format!(
                        "state {state} frame {frame} time {time} camera {id}: ours {got:?} original {want:?}"
                    ),
                );
            }
        }
        eprintln!(
            "script {script}: {samples} samples, {exact} exact, {torn} torn reads, worst {worst:?}, freeze frames {}..={}",
            freeze.first().unwrap_or(&0),
            freeze.last().unwrap_or(&0)
        );
        assert!(samples > 0 && exact > samples * 3 / 4);
        assert_eq!(exact + torn, samples, "{worst:?}");
        // The freeze shows its camera from its own frame through its last
        // update's.
        assert_eq!(freeze.first(), Some(&hold.0));
        assert_eq!(freeze.last(), Some(&(hold.0 + hold.1 - 1)));
    }
    #[test]
    fn original_capture_matches_production_consumed_projection() {
        use bevy::camera::CameraProjection;

        let Some(capture_path) = std::env::var_os("STREET_CONSUMED_CAMERA_CAPTURE") else {
            return;
        };
        let report_path = std::env::var_os("STREET_CONSUMED_CAMERA_REPORT")
            .expect("capture comparisons require STREET_CONSUMED_CAMERA_REPORT");
        let capture =
            std::fs::read_to_string(capture_path).expect("read consumed camera bit capture");
        let words: Vec<u32> = capture
            .split_whitespace()
            .map(|word| u32::from_str_radix(word, 16).expect("float32 hex word"))
            .collect();
        assert_eq!(
            words.len(),
            5,
            "exactly one original consumed projection observation"
        );
        let values: Vec<f32> = words.iter().copied().map(f32::from_bits).collect();
        assert!(values.iter().all(|value| value.is_finite()));
        let [native_angle, aspect, zoom, native_m00, native_m11] = values[..] else {
            unreachable!();
        };
        assert!(native_angle > 0.0 && native_angle < 180.0 && aspect > 0.0 && zoom > 0.0);
        let fov = consumed_camera_vertical_fov(native_angle, aspect, zoom);
        let projection = PerspectiveProjection {
            fov,
            aspect_ratio: aspect,
            ..default()
        };
        let matrix = projection.get_clip_from_view();
        // Native viewport flips Y; depth and pose entries are not compared.
        let actual = [matrix.x_axis.x, -matrix.y_axis.y];
        let native = [native_m00, native_m11];
        let ulps: Vec<u32> = actual
            .iter()
            .zip(native)
            .map(|(a, b)| {
                let ordered = |value: f32| {
                    let bits = value.to_bits();
                    if bits & 0x8000_0000 == 0 {
                        bits | 0x8000_0000
                    } else {
                        !bits
                    }
                };
                ordered(*a).abs_diff(ordered(b))
            })
            .collect();
        let source = include_str!("view.rs");
        let source_fnv1a = source.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        });
        let compiled_source_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/view.rs");
        assert_eq!(
            source,
            std::fs::read_to_string(compiled_source_path).unwrap(),
            "rebuild the test: current production source differs from compiled source"
        );
        let metrics: Vec<String> = ["m00", "native_y_m11"]
            .iter().enumerate().map(|(index, name)| format!(
                "\"{name}\":{{\"native\":{},\"actual\":{},\"native_bits\":\"{:08x}\",\"actual_bits\":\"{:08x}\",\"absolute_error\":{},\"ulp_error\":{}}}",
                native[index] as f64, actual[index] as f64, native[index].to_bits(), actual[index].to_bits(),
                (native[index] as f64 - actual[index] as f64).abs(), ulps[index]
            )).collect();
        let passed = ulps.iter().all(|error| *error <= 4);
        let input_bits = words
            .iter()
            .map(|word| format!("\"{word:08x}\""))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(report_path, format!(
            "{{\"schema\":1,\"component\":\"production_consumed_camera_projection\",\"compiled_view_rs_fnv1a64\":\"{source_fnv1a:016x}\",\"input_bits\":[{input_bits}],\"observations\":1,\"bit_exact\":{},\"within_declared_tolerance\":{passed},\"matrix_max_ulps\":4,\"vertical_fov_radians\":{fov},\"metrics\":{{{}}}}}\n",
            ulps.iter().all(|error| *error == 0), metrics.join(",")
        )).expect("write consumed camera comparison report");
        // Write every comparison before failing; never filter by its residual.
        assert!(
            passed,
            "original consumed projection exceeds component tolerance: {ulps:?}"
        );
    }

    /// Opt-in original-runtime component check. The adapter contains six
    /// float32 bit words: scalar, aspect, zoom, angle-degrees, m00, m11.
    /// `tools/compare_camera_projection.py` retains and hashes the original
    /// JSON capture around this report; no generated mode cases are accepted.
    #[test]
    fn original_capture_matches_production_scalar_projection() {
        use bevy::camera::CameraProjection;

        let Some(capture_path) = std::env::var_os("STREET_CAMERA_PARITY_CAPTURE") else {
            return;
        };
        let report_path = std::env::var_os("STREET_CAMERA_PARITY_REPORT")
            .expect("capture comparisons require STREET_CAMERA_PARITY_REPORT");
        let capture = std::fs::read_to_string(capture_path).expect("read camera bit capture");
        let words: Vec<u32> = capture
            .split_whitespace()
            .map(|word| u32::from_str_radix(word, 16).expect("float32 hex word"))
            .collect();
        assert_eq!(
            words.len(),
            6,
            "exactly one original projection observation"
        );
        let values: Vec<f32> = words.iter().copied().map(f32::from_bits).collect();
        assert!(values.iter().all(|value| value.is_finite()));
        let [scalar, aspect, zoom, native_angle, native_m00, native_m11] = values[..] else {
            unreachable!();
        };
        assert!(scalar > 0.0 && aspect > 0.0 && zoom > 0.0);
        let (angle, fov) = scalar_camera_projection(scalar, aspect, zoom);
        let projection = PerspectiveProjection {
            fov,
            aspect_ratio: aspect,
            ..default()
        };
        let matrix = projection.get_clip_from_view();
        // Native renderer's viewport flips Y. Bevy's positive Y coefficient
        // is mapped to that convention; depth/pose entries are not compared.
        let actual = [angle, matrix.x_axis.x, -matrix.y_axis.y];
        let native = [native_angle, native_m00, native_m11];
        let ulps: Vec<u32> = actual
            .iter()
            .zip(native)
            .map(|(a, b)| {
                let ordered = |value: f32| {
                    let bits = value.to_bits();
                    if bits & 0x8000_0000 == 0 {
                        bits | 0x8000_0000
                    } else {
                        !bits
                    }
                };
                ordered(*a).abs_diff(ordered(b))
            })
            .collect();
        let source = include_str!("view.rs");
        let source_fnv1a = source.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        });
        let compiled_source_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/view.rs");
        assert_eq!(
            source,
            std::fs::read_to_string(compiled_source_path).unwrap(),
            "rebuild the test: current production source differs from compiled source"
        );
        let metrics: Vec<String> = ["horizontal_degrees", "m00", "native_y_m11"]
            .iter().enumerate().map(|(index, name)| format!(
                "\"{name}\":{{\"native\":{},\"actual\":{},\"native_bits\":\"{:08x}\",\"actual_bits\":\"{:08x}\",\"absolute_error\":{},\"ulp_error\":{}}}",
                native[index] as f64, actual[index] as f64, native[index].to_bits(), actual[index].to_bits(),
                (native[index] as f64 - actual[index] as f64).abs(), ulps[index]
            )).collect();
        let passed = ulps[0] == 0 && ulps[1] <= 4 && ulps[2] <= 4;
        let input_bits = words
            .iter()
            .map(|word| format!("\"{word:08x}\""))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(report_path, format!(
            "{{\"schema\":1,\"component\":\"production_scalar_camera_projection\",\"compiled_view_rs_fnv1a64\":\"{source_fnv1a:016x}\",\"input_bits\":[{input_bits}],\"observations\":1,\"bit_exact\":{},\"within_declared_tolerance\":{passed},\"angle_max_ulps\":0,\"matrix_max_ulps\":4,\"vertical_fov_radians\":{fov},\"metrics\":{{{}}}}}\n",
            ulps.iter().all(|error| *error == 0), metrics.join(",")
        )).expect("write camera comparison report");
        assert!(
            passed,
            "original captured projection exceeds component tolerance: {ulps:?}"
        );
    }

    // Selected pose and screen-zone words from each row at 0x498478.
    const CAMERA_MODE_ROWS_RAW: [[u32; 8]; 6] = [
        [
            0x4080_0000,
            0x40a0_0000,
            0x4140_0000,
            0x4140_0000,
            0xc1f0_0000,
            0x3ea8_f5c3,
            185,
            115,
        ],
        [
            0x4170_0000,
            0x40a0_0000,
            0x4180_0000,
            0x4140_0000,
            0xc220_0000,
            0x3e6b_851f,
            185,
            115,
        ],
        [
            0x4190_0000,
            0x4140_0000,
            0x41b0_0000,
            0x0000_0000,
            0xc282_0000,
            0x3e38_51ec,
            170,
            100,
        ],
        [
            0x0000_0000,
            0x0000_0000,
            0x41e0_0000,
            0x0000_0000,
            0xc296_0000,
            0x3e19_999a,
            230,
            90,
        ],
        [
            0x0000_0000,
            0xc270_0000,
            0x4198_0000,
            0x41d4_0000,
            0x0000_0000,
            0x3e8f_df3b,
            300,
            90,
        ],
        [
            0x4080_0000,
            0x40a0_0000,
            0x4140_0000,
            0x4100_0000,
            0xc220_0000,
            0x3ea1_47ae,
            185,
            115,
        ],
    ];

    #[test]
    fn camera_mode_rows_match_native_pose_and_screen_zone_words() {
        let implemented = CAMERA_MODE_ROWS.map(|row| {
            [
                row.x_bound.to_bits(),
                row.x_offset.to_bits(),
                row.eye_y.to_bits(),
                row.focus_z_scale.to_bits(),
                row.focus_z_offset.to_bits(),
                row.row_word7.to_bits(),
                row.horizontal_screen_threshold as u32,
                row.top_screen_threshold as u32,
            ]
        });
        assert_eq!(CAMERA_MODE_ROWS_RAW, implemented);
    }

    #[test]
    fn camera_scalar_rounding_matches_retained_native_inputs() {
        // Actual restored-state9 blend-helper observations, retained outside
        // Git. The first failed by three angle ULPs with ordinary host math.
        for (scalar, degrees) in [(0x3d7f5c91, 0x41180f22), (0x3dc2575e, 0x4166c148)] {
            let actual = scalar_camera_projection(f32::from_bits(scalar), 4.0 / 3.0, 1.3).0;
            assert_eq!(actual.to_bits(), degrees);
        }
    }

    #[test]
    fn the_saved_camera_type_picks_the_originals_rows_and_never_behind() {
        // Pick Up, one person: the saved type; Behind's 4 and 6, 7 read
        // Street as FUN_003038b0's switch has them.
        let rows: Vec<usize> = (0..8).map(|t| native_camera_row(2, 1, t)).collect();
        assert_eq!(rows, [0, 1, 2, 3, 0, 5, 0, 0]);
        // Mode 3, and the League Challenge with two users: Wide whatever is
        // saved.
        assert_eq!(native_camera_row(3, 1, 1), 5);
        assert_eq!(native_camera_row(4, 2, 1), 5);
        assert_eq!(native_camera_row(4, 1, 1), 1);
    }

    #[test]
    fn mode_fov_matches_native_projection_for_4_3_and_16_9() {
        // Independently rounded float32 native-polynomial/renderer oracle values.
        let expected_degrees_4_3 = [46.43959, 33.29466, 26.34143, 22.06907, 40.13587, 44.53931];
        let expected_degrees_16_9 = [35.67191, 25.27999, 19.908_8, 16.64154, 30.64436, 34.14695];
        // The projection matrix's angles carry +0x190's 1.3; the picture
        // shows them with it undone (the viewport scales, SCREEN_ZOOM).
        let shown = |native: f32| {
            2.0 * ((native.to_radians() * 0.5).tan() / CAMERA_PROJECTION_ZOOM)
                .atan()
                .to_degrees()
        };
        for mode in 0..CAMERA_MODE_ROWS.len() {
            let scalar = CAMERA_MODE_ROWS[mode].row_word7;
            let (horizontal, native_4_3) =
                scalar_camera_projection(scalar, 4.0 / 3.0, CAMERA_PROJECTION_ZOOM);
            let native_16_9 =
                consumed_camera_vertical_fov(horizontal, 16.0 / 9.0, CAMERA_PROJECTION_ZOOM);
            assert!((native_4_3.to_degrees() - expected_degrees_4_3[mode]).abs() < 1e-4);
            assert!((native_16_9.to_degrees() - expected_degrees_16_9[mode]).abs() < 1e-4);
            assert!(
                (mode_vertical_fov(mode, Picture::Standard).to_degrees()
                    - shown(expected_degrees_4_3[mode]))
                .abs()
                    < 1e-3
            );
            assert!(
                (mode_vertical_fov(mode, Picture::Anamorphic).to_degrees()
                    - shown(expected_degrees_16_9[mode]))
                .abs()
                    < 1e-3
            );
            // Wide keeps the 4:3 vertical angle (the renderer's third branch
            // through its own tangent and arctangent, an ulp or two apart).
            assert!(
                (mode_vertical_fov(mode, Picture::Wide).to_degrees()
                    - shown(expected_degrees_4_3[mode]))
                .abs()
                    < 1e-3
            );
        }
        assert_eq!(
            mode_vertical_fov(99, Picture::Standard),
            mode_vertical_fov(5, Picture::Standard)
        );
    }

    /// The Street row's lens 0.33 through the three pictures: the angles
    /// `docs/CAMERA_RECOVERY.md` tables, and Wide's horizontal half-angle
    /// tangent 4/3 of the 4:3 picture's (`FUN_00323450`'s third branch).
    #[test]
    fn the_three_pictures_of_the_street_lens() {
        let scalar = CAMERA_MODE_ROWS[0].row_word7;
        assert!((scalar - 0.33).abs() < 1e-6);
        let (horizontal, _) = scalar_camera_projection(scalar, 4.0 / 3.0, SCREEN_ZOOM);
        let degrees = |p| lens_vertical_fov(scalar, p).to_degrees();
        assert!((degrees(Picture::Standard) - 36.5258).abs() < 1e-3);
        assert!((degrees(Picture::Anamorphic) - 27.8027).abs() < 1e-3);
        assert!((degrees(Picture::Wide) - 36.5258).abs() < 1e-3);
        let wide = wide_horizontal_degrees(horizontal);
        let half = |d: f32| (d.to_radians() * 0.5).tan();
        assert!((half(wide) - half(horizontal) * 4.0 / 3.0).abs() < 1e-5);
        // The pictures' widths: the half-angle tangent at the picture's zoom.
        let shown = |d: f32| 2.0 * (half(d) * SCREEN_ZOOM).atan().to_degrees();
        assert!((shown(horizontal) - 47.50).abs() < 0.01);
        assert!((shown(wide) - 60.80).abs() < 0.01);
    }

    #[test]
    fn normal_mode_rows_apply_offset_clamp_and_z_mapping() {
        assert_eq!(
            mode_camera_pose(Vec3::ZERO, 1.0, 0),
            (Vec3::new(4.0, 12.0, -30.0), Vec3::ZERO)
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(10.0, 3.0, -26.5), 1.0, 1),
            (Vec3::new(15.0, 16.0, -52.0), Vec3::new(10.0, 3.0, -26.5))
        );
        assert_eq!(
            mode_camera_pose(Vec3::ZERO, 1.0, 2),
            (Vec3::new(12.0, 22.0, -65.0), Vec3::ZERO)
        );
        assert_eq!(
            mode_camera_pose(Vec3::ZERO, -1.0, 3),
            (Vec3::new(0.0, 28.0, -75.0), Vec3::ZERO)
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(0.0, 3.0, 26.5), -1.0, 5),
            (Vec3::new(-4.0, 12.0, -32.0), Vec3::new(0.0, 3.0, 26.5))
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(50.0, 3.0, 53.0), 1.0, 0).0,
            Vec3::new(4.0, 12.0, -6.0)
        );
    }

    #[test]
    fn behind_mode_keeps_target_z_and_uses_asymmetric_x_bound() {
        assert_eq!(
            mode_camera_pose(Vec3::new(10.0, 0.0, 8.0), 1.0, 4),
            (Vec3::new(-50.0, 19.0, 8.0), Vec3::new(10.0, 0.0, 8.0))
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(10.0, 0.0, 8.0), -1.0, 4).0,
            Vec3::new(70.0, 19.0, 8.0)
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(100.0, 4.25, 8.0), 1.0, 4),
            (Vec3::new(0.0, 19.0, 8.0), Vec3::new(60.0, 0.0, 8.0))
        );
        assert_eq!(
            mode_camera_pose(Vec3::new(-100.0, 4.25, 8.0), -1.0, 4),
            (Vec3::new(0.0, 19.0, 8.0), Vec3::new(-60.0, 0.0, 8.0))
        );
    }

    #[test]
    fn native_focus_uses_owner_xyz_and_ball_xyz_when_free() {
        let mut w = camera_world();
        w.players[0].position = [7.0, -3.0];
        w.players[0].height = 4.25;
        w.ball.state = street_sim::BallState::Held(0);
        w.ball.position = [20.0, 5.0, 11.0];
        let sample = CameraSourceSample::from_world(&w);
        assert_eq!(sample.focus, Vec3::new(7.0, 4.25, -3.0));
        assert!(!sample.ownerless);
        assert_eq!(sample.entity, Some(0));
        for state in [
            street_sim::BallState::Shot,
            street_sim::BallState::Pass,
            street_sim::BallState::Loose,
        ] {
            w.ball.state = state;
            let sample = CameraSourceSample::from_world(&w);
            assert_eq!(sample.focus, Vec3::new(20.0, 5.0, 11.0));
            assert!(sample.ownerless);
            assert_eq!(sample.entity, None);
        }
        // +0xd0 floors z at -26.5; +0xc0 keeps the raw value.
        w.ball.position = [20.0, 5.0, -40.0];
        let mut source = CameraSourceState::default();
        source.update(&CameraSourceSample::from_world(&w), false, &mut None);
        assert_eq!(source.raw, Some(Vec3::new(20.0, 5.0, -40.0)));
        assert_eq!(source.clamped, Vec3::new(20.0, 5.0, -26.5));
        assert_eq!(mode_camera_pose(source.clamped, 1.0, 1).0.z, -52.0);
    }

    #[test]
    fn native_camera_controller_velocity_step_and_integration() {
        assert_eq!(approach_step(0.0, 10.0, 2.0), 2.0);
        assert_eq!(approach_step(0.0, -10.0, 2.0), -2.0);
        assert_eq!(approach_step(5.0, 6.0, 2.0), 1.0);
        assert_eq!(approach_step(5.0, 5.0, 2.0), 0.0);

        // Velocity step overshooting check
        assert_eq!(velocity_step(1.0, 2.0, 0.5, false), 1.5);
        assert_eq!(velocity_step(1.0, 1.2, 0.5, false), 1.0);
        // The EE's add (its guard bit, then chopped): -0.90000004.
        assert_eq!(
            velocity_step(-1.0, 1.0, 0.1, true),
            f32::from_bits(0xbf66_6667)
        );

        // Native camera state initialization and stepping
        let mut state = NativeCameraState::new(0);
        assert_eq!(state.mode, 0);
        assert!(!state.initialized);

        state.update(&RowInputs::default());
        assert!(state.initialized);
        assert_eq!(state.eye[1], 12.0); // mode 0 eye_y
        assert_eq!(state.eye[2], -30.0); // mode 0 focus_z_offset

        // Step with motion: the depth command follows the focus.
        let moving = RowInputs {
            focus: Vec3::new(1.0, 0.0, 0.5),
            motion: Vec3::new(1.0, 0.0, 0.5),
            ..RowInputs::default()
        };
        state.update(&moving);
        assert!(state.velocity[1] > 0.0);
    }

    fn camera_world() -> street_sim::World {
        let players = (0..6)
            .map(|k| street_sim::Player::new(k / 3, [10.0 - k as f32 * 4.0, 5.0], 0.0))
            .collect();
        let hoop = |x: f32| street_sim::Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        let mut w = street_sim::World::new(
            [hoop(37.0), hoop(-37.0)],
            players,
            street_sim::Rules::default(),
            1,
        );
        w.ball.state = street_sim::BallState::Held(0);
        w
    }

    fn attract(world: &street_sim::World) -> CameraTick<'_> {
        CameraTick {
            attract: true,
            ..tick(world)
        }
    }

    fn tick<'a>(world: &'a street_sim::World) -> CameraTick<'a> {
        CameraTick {
            world,
            staged: None,
            intro_playing: false,
            replay_playing: false,
            gamebreaker_over: false,
            camera_mode: 0,
            attract: false,
            court: Some(3),
            events: &world.events,
            bodies: &[],
            lesson: false,
            explosive_rims: false,
            seated: &[],
        }
    }

    /// A manager past its intro cut, on the settled row.
    fn on_the_row(w: &street_sim::World) -> CameraManager {
        let mut mgr = CameraManager::new(0);
        mgr.update(&tick(w));
        mgr.intro_cut_age = None;
        mgr.select_row();
        mgr.update(&tick(w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        mgr
    }

    /// One stepped update with these events.
    fn stepped(
        mgr: &mut CameraManager,
        w: &mut street_sim::World,
        events: Vec<street_sim::Event>,
    ) -> (Vec3, Vec3, CameraLens) {
        w.tick += 1;
        w.events = events;
        mgr.update(&tick(w))
    }

    #[test]
    fn a_staged_level_two_keeps_the_row_until_its_script_and_owes_a_celebration() {
        // The School's Level 2 capture (manager +0x4c 0): +0x40 stays off
        // from the press through the script, the row stays on screen until
        // the script's camera takes it two updates after the press, and
        // the script's end (FUN_003033a8, word +4 of 1) sets +0x50 to two
        // seconds, settles the row and the task's end blends to it.
        let mut w = camera_world();
        let mut mgr = on_the_row(&w);
        w.staged_start = Some((
            street_sim::gamebreaker::Sequence {
                script: 8,
                shooter: 0,
                team: 0,
                level: 2,
                frame: 0,
                held: 0,
                actors: [None; 7],
                native_clocks: [None; 7],
                resume_pending: false,
                finishing: 0,
            },
            2,
        ));
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Gamebreaker {
                player: 0,
                level: 2,
            }],
        );
        assert!(!mgr.gamebreaker);
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        w.tick += 1;
        let script = CameraTick {
            staged: Some(StagedView {
                eye: Vec3::new(1.0, 2.0, 3.0),
                at: Vec3::ZERO,
                scalar: 0.7,
                roll: 0.0,
            }),
            ..tick(&w)
        };
        mgr.update(&script);
        assert_eq!(mgr.active_helper, CameraHelperKind::Staged);
        assert!(!mgr.gamebreaker);
        stepped(&mut mgr, &mut w, vec![]);
        assert_eq!(mgr.celebration, CELEBRATION_TICKS[0] - 1);
        assert_eq!(mgr.active_helper, CameraHelperKind::Blend);
        assert!(!mgr.gamebreaker);
    }

    #[test]
    fn gamebreakers_blend_to_a_narrow_courtside_camera_and_back_like_fun_00303650() {
        let mut w = camera_world();
        let mut mgr = on_the_row(&w);
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Gamebreaker {
                player: 0,
                level: 1,
            }],
        );
        // +0x40, and a blend from the row over fps / 2 updates.
        assert!(mgr.gamebreaker);
        assert_eq!(mgr.active_helper, CameraHelperKind::Blend);
        let blend = mgr.blend_helper.as_ref().unwrap();
        assert_eq!(
            (blend.from, blend.to),
            (CameraHelperKind::Row, CameraHelperKind::Positioned)
        );
        assert_eq!((blend.counter, blend.duration), (29, 30.0));
        // The narrow courtside placement: half width 23, scale 2..5.
        let helper = &mgr.positioned_helper;
        assert!(helper.eye.x.abs() <= 23.0, "{}", helper.eye);
        assert!((2.0..=5.0).contains(&helper.distance_scale));
        for _ in 0..29 {
            stepped(&mut mgr, &mut w, vec![]);
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Blend);
        stepped(&mut mgr, &mut w, vec![]);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        // A restart while it is on keeps the camera (FUN_00302510), and the
        // proximity re-placement is off (FUN_00304a38).
        let eye = mgr.positioned_helper.eye;
        w.players[4].position = [eye.x + 1.0, eye.z];
        w.players[4].height = eye.y;
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Inbound { player: 3 }],
        );
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert_eq!(mgr.positioned_helper.eye, eye);
        w.players[4].position = [0.0, 0.0];
        w.players[4].height = 0.0;
        // Through FUN_00144ef8's wait and its two replays (the task, in
        // crate::tape) the courtside camera stays: a replay's start and end
        // leave it while +0x40 is set.
        for _ in 32..crate::tape::GAMEBREAKER_DELAY[0] {
            stepped(&mut mgr, &mut w, vec![]);
            assert!(mgr.gamebreaker);
        }
        let replaying = CameraTick {
            replay_playing: true,
            ..tick(&w)
        };
        mgr.update(&replaying);
        assert!(mgr.gamebreaker);
        mgr.update(&tick(&w));
        assert!(mgr.gamebreaker);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        // The task's end (FUN_003037b8): the row settles and blends back
        // over 30.
        mgr.update(&CameraTick {
            gamebreaker_over: true,
            ..tick(&w)
        });
        assert!(!mgr.gamebreaker);
        let blend = mgr.blend_helper.as_ref().unwrap();
        assert_eq!(
            (blend.from, blend.to),
            (CameraHelperKind::Positioned, CameraHelperKind::Row)
        );
        for _ in 0..30 {
            stepped(&mut mgr, &mut w, vec![]);
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // The attract match cuts straight to the courtside camera.
        let mut mgr = CameraManager::new(0);
        mgr.update(&attract(&w));
        w.tick += 1;
        w.events = vec![street_sim::Event::Gamebreaker {
            player: 0,
            level: 2,
        }];
        mgr.update(&attract(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert!(mgr.positioned_helper.eye.x.abs() <= 23.0);
    }

    #[test]
    fn a_gamebreaker_basket_frames_the_rim_and_owes_a_celebration_like_fun_00302c20() {
        let mut w = camera_world();
        let mut mgr = on_the_row(&w);
        w.ball.state = street_sim::BallState::Loose;
        w.ball.position = [35.0, 10.0, 0.5];
        let basket = street_sim::Event::Score {
            team: 0,
            player: 1,
            points: 3,
            basket: street_sim::Basket::Gamebreaker {
                level: 1,
                long: false,
                dunk: false,
            },
        };
        stepped(&mut mgr, &mut w, vec![basket]);
        // The row turns to the basket's middle and shakes (0.75).
        assert_eq!(
            mgr.row_focus,
            CameraFocus::Point(Vec3::new(36.75, 11.956_775, 0.0))
        );
        assert_eq!(mgr.row_helper.shake.amplitude, BASKET_SHAKE);
        assert_eq!(mgr.row_helper.shake.left, 39);
        // Two seconds later (counted while no replay or script plays) a
        // courtside camera finds the scorer.
        assert_eq!(mgr.celebration, CELEBRATION_TICKS[0] - 1);
        let replay = CameraTick {
            replay_playing: true,
            ..tick(&w)
        };
        // Not while the replay plays; again once it is over.
        mgr.update(&replay);
        assert_eq!(mgr.celebration, CELEBRATION_TICKS[0] - 1);
        mgr.update(&tick(&w));
        assert_eq!(mgr.celebration, CELEBRATION_TICKS[0] - 2);
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        for _ in 1..CELEBRATION_TICKS[0] - 2 {
            stepped(&mut mgr, &mut w, vec![]);
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        stepped(&mut mgr, &mut w, vec![]);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert_eq!(mgr.positioned_helper.focus_kind, CameraFocus::Actor(1));
        // The restart brings the row back with the default focus.
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Inbound { player: 3 }],
        );
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        assert_eq!(mgr.row_focus, CameraFocus::Default);
        // A restart before the countdown ends cancels the celebration.
        let mut mgr = on_the_row(&w);
        stepped(&mut mgr, &mut w, vec![basket]);
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Inbound { player: 3 }],
        );
        for _ in 0..CELEBRATION_TICKS[0] {
            stepped(&mut mgr, &mut w, vec![]);
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // An ordinary basket does nothing, unless Explosive Rims is on.
        let mut mgr = on_the_row(&w);
        let jumper = street_sim::Event::Score {
            team: 0,
            player: 1,
            points: 1,
            basket: street_sim::Basket::Jumper { long: false },
        };
        stepped(&mut mgr, &mut w, vec![jumper]);
        assert_eq!(mgr.row_helper.shake.left, 0);
        w.tick += 1;
        w.events = vec![jumper];
        mgr.update(&CameraTick {
            explosive_rims: true,
            ..tick(&w)
        });
        assert_eq!(mgr.row_helper.shake.left, 39);
        assert_eq!(mgr.row_focus, CameraFocus::Default);
        assert_eq!(mgr.celebration, 0);
    }

    #[test]
    fn replay_cuts_follow_fun_003041f0_once_the_toss_is_up() {
        // Coin seeds whose first FUN_003b42a0 draw falls either side of 60/101.
        let seed = |beside: bool| {
            (0..)
                .find(|&s| {
                    (crate::progression_trace::NativeRandom::new(s).draw() % 101 < 60) == beside
                })
                .unwrap()
        };
        let mut w = camera_world();
        fn replay(w: &street_sim::World) -> CameraTick<'_> {
            CameraTick {
                replay_playing: true,
                ..tick(w)
            }
        }
        for (pending, beside) in [(true, true), (false, true), (false, false)] {
            let mut mgr = on_the_row(&w);
            mgr.toss_pending = pending;
            mgr.update(&replay(&w));
            assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
            // The owner leaves the floor in the replay.
            mgr.coin_random = crate::progression_trace::NativeRandom::new(seed(beside));
            let eye = mgr.positioned_helper.eye;
            w.tick += 1;
            w.ball.position = [12.0, 3.0, 2.0];
            w.players[0].height = 0.5;
            mgr.update(&replay(&w));
            w.players[0].height = 0.0;
            let helper = &mgr.positioned_helper;
            if pending {
                // DAT_0040200c still set: no cut.
                assert_eq!(helper.eye, eye);
            } else if beside {
                // Beside the basket on the ball's side, z away from it.
                assert_eq!((helper.eye.x, helper.eye.z), (37.75, -3.25));
                assert!((12.0..=18.0).contains(&helper.eye.y));
                assert_eq!(
                    helper.focus_kind,
                    CameraFocus::Point(Vec3::new(34.75, 12.0, 0.0))
                );
                assert_eq!(helper.distance_scale, 3.0);
            } else {
                // FUN_00303c80's baseline view.
                assert!((24.0..=42.0).contains(&helper.eye.x), "{}", helper.eye);
                assert!((0.5..=2.0).contains(&helper.eye.y));
                assert!((-12.0..=-8.0).contains(&helper.eye.z));
                assert!((5.5..=8.0).contains(&helper.distance_scale));
                assert_eq!(helper.focus_kind, CameraFocus::Default);
            }
            // A release re-places the courtside eye.
            if !pending {
                let eye = mgr.positioned_helper.eye;
                w.tick += 1;
                w.ball.state = street_sim::BallState::Shot;
                mgr.update(&replay(&w));
                w.ball.state = street_sim::BallState::Held(0);
                assert_ne!(mgr.positioned_helper.eye, eye);
                assert!(mgr.positioned_helper.eye.z <= -35.0);
            }
        }
    }

    #[test]
    fn the_opening_toss_holds_the_intro_cut_on_the_referee() {
        let mut w = camera_world();
        w.start_opening_toss();
        let mut mgr = CameraManager::new(0);
        mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert!(mgr.toss_pending && mgr.referee_phase);
        // The referee holds the ball: phase 1 zeroes the focus's x and z
        // (raw 0x300600..0x300618), and his upper body's offsets are added
        // (the retained capture's +0xc0 is the origin).
        let referee = Vec3::from_array(street_sim::toss::REFEREE);
        let [head, chest] = REFEREE_UPPER_BODY.map(Vec3::from_array);
        let w_mix = ((6.75_f32 - 4.0) / 15.0).clamp(0.0, 1.0);
        let framed = Vec3::new(0.0, referee.y, 0.0) + head * (1.0 - w_mix) + chest * w_mix;
        assert!(mgr.positioned_helper.focus.distance(framed) < 1e-4);
        // The world waits, so the cut holds and then blends to the row,
        // whose focus is lifted to 4 through the opening phase.
        for _ in 0..INTRO_CUT_HOLD_TICKS + INTRO_CUT_BLEND_TICKS + 2 {
            w.step(&[]);
            mgr.update(&tick(&w));
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        assert_eq!(mgr.row_source.clamped, Vec3::ZERO);
        // Nobody has the ball (side 0): the Street eye stands over the
        // centre line, settled on the lifted focus.
        let (eye, _, _) = mgr.update(&tick(&w));
        assert_eq!(eye, Vec3::new(0.0, 12.0, -30.0));
        // The played match's opening (street-camera-played-opening1.jsonl,
        // the user's slot 1, manager +0x4c 0): the row settles at the
        // blend's start aimed at (0, 4, 0) (pitch 0.2603: nobody has the
        // ball, so no rim to frame), then its updates under the blend
        // pitch it up toward the top band, 0.1623 when the blend hands
        // over (row +0x90 at update 116).
        assert_eq!(mgr.row_helper.angles[1], 0.0);
        let pitch = mgr.row_helper.angles[0];
        assert!(0.155 < pitch && pitch < 0.17, "{pitch}");
        // The toss goes up (slot 0x144): DAT_0040200c clears.
        while w.opening_toss != street_sim::OpeningToss::Flight {
            w.step(&[]);
            mgr.update(&tick(&w));
        }
        assert!(!mgr.toss_pending);
        assert!(mgr.referee_phase);
        // A toss that goes up inside the hold leaves the cut on screen.
        let mut w = camera_world();
        w.start_opening_toss();
        w.opening_toss = street_sim::OpeningToss::Waiting(5);
        let mut mgr = CameraManager::new(0);
        mgr.update(&tick(&w));
        for _ in 0..INTRO_CUT_HOLD_TICKS + INTRO_CUT_BLEND_TICKS + 10 {
            w.step(&[]);
            mgr.update(&tick(&w));
        }
        assert!(!mgr.toss_pending);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
    }

    #[test]
    fn the_courtside_camera_holds_a_still_subject_and_turns_with_a_moving_one() {
        let mut helper = PositionedCameraHelper::default();
        helper.place_midcourt(1);
        helper.select();
        let mut at = Vec3::new(0.0, 4.0, 0.0);
        let sample = |focus: Vec3, source: Vec3| CameraSourceSample {
            focus,
            ownerless: true,
            entity: None,
            source,
            entity_busy: false,
            stop_clip: false,
            rest: 0.0,
            release: 0,
        };
        // The settle aims straight at the subject.
        helper.step(&sample(at, Vec3::ZERO), None, &mut None);
        let aimed = native_angles((at - helper.eye).to_array());
        assert_eq!((helper.pitch, helper.yaw), aimed);
        // Inside the window and still: FUN_002fed00's 0x30 holds the angles.
        for _ in 0..30 {
            helper.step(&sample(at, Vec3::ZERO), None, &mut None);
        }
        assert_eq!((helper.pitch, helper.yaw), aimed);
        assert_eq!(
            (helper.pitch_rate, helper.yaw_rate, helper.framing),
            (0.0, 0.0, 0)
        );
        // A subject running across: the rates build and the camera keeps
        // it on screen, following (1) or recentring (2).
        let step = Vec3::new(0.25, 0.0, 0.0);
        for _ in 0..90 {
            at += step;
            helper.step(&sample(at, step), None, &mut None);
        }
        assert!(helper.yaw_rate != 0.0);
        assert!(helper.framing != 0);
        let to = (at - helper.eye).normalize();
        assert!(
            helper.pose().forward().dot(to) > 0.99,
            "{}",
            helper.pose().forward()
        );
        let window = helper.window();
        let zones = framing_zones(
            framing_projection(
                helper.eye,
                helper.pitch,
                helper.yaw,
                helper.scalar,
                helper.focus,
            ),
            &window,
        );
        assert_eq!(zones & 0x7c00, 0, "{zones:#x}");
    }

    #[test]
    fn framing_follows_fun_002ffac0_and_fun_002ff830() {
        // Straight ahead is the middle of the 640x480 frame (within the
        // native arctangent's approximation).
        let eye = Vec3::new(0.0, 5.0, -20.0);
        let (pitch, yaw) = native_angles([3.0, -5.0, 20.0]);
        let middle = framing_projection(eye, pitch, yaw, 0.3, Vec3::new(3.0, 0.0, 0.0)).unwrap();
        assert!(
            (middle - Vec2::new(320.0, 240.0)).length() < 0.02,
            "{middle}"
        );
        // Behind the near plane it fails.
        assert_eq!(
            framing_projection(eye, 0.0, 0.0, 0.3, Vec3::new(0.0, 5.0, -19.6)),
            None
        );
        // Screen x grows toward camera right (-x looking down +z).
        let right = framing_projection(eye, 0.0, 0.0, 0.3, Vec3::new(-1.0, 5.0, 0.0)).unwrap();
        assert!(right.x > 320.0);
        // The window at scale 6.75 (2 / 6.75 of the bands, truncated as
        // `cvt.w.s` does).
        let mut helper = PositionedCameraHelper {
            distance_scale: 6.75,
            ..Default::default()
        };
        assert_eq!(helper.window(), [227, 48, 227, 48, 128, 32, 198, 62]);
        helper.framing = 2;
        helper.distance_scale = 3.0;
        assert_eq!(helper.window(), [220, 55, 220, 55, 124, 36, 190, 70]);
        let window = [226, 49, 226, 49, 127, 33, 198, 62];
        let zone = |x: f32, y: f32| framing_zones(Some(Vec2::new(x, y)), &window);
        assert_eq!(zone(320.0, 200.0), 1 | 2);
        assert_eq!(zone(250.0, 140.0), 0x10 | 4);
        assert_eq!(zone(100.0, 300.0), 0x100 | 0x80);
        assert_eq!(zone(700.0, 500.0), 0x2000 | 0x800 | 0x200 | 0x80);
        assert_eq!(framing_zones(None, &window), 0x4000);
    }

    #[test]
    fn the_row_shakes_like_fun_002f8338() {
        let mut state = NativeCameraState::new(0);
        let still = RowInputs::default();
        state.update(&still);
        state.shake = RowShake {
            left: 40,
            updates: 40,
            cycles: 15,
            amplitude: 0.5,
        };
        for n in (1..40).rev() {
            state.update(&still);
            let drop = 0.5 * (15.0 * PI * n as f32 / 40.0).sin();
            assert!((state.eye[1] - (12.0 - drop)).abs() < 1e-5, "{n}");
        }
        state.update(&still);
        assert_eq!(state.eye[1], 12.0);
        for k in 0..2000 {
            let x = k as f32 * 0.03;
            assert!(
                (native_sin(x) - (x as f64).sin() as f32).abs() < 1e-5,
                "{x}"
            );
        }
    }

    #[test]
    fn pockets_cancels_dunks_and_violations_reach_their_listener_slots() {
        let mut w = camera_world();
        // Slot 0x234: a pocket shakes the row on screen.
        let mut mgr = on_the_row(&w);
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Pocket { team: 0 }],
        );
        assert_eq!(
            (mgr.row_helper.shake.left, mgr.row_helper.shake.amplitude),
            (39, 0.5)
        );
        // Slot 0x23c: a cancel shakes it even off screen.
        let mut mgr = on_the_row(&w);
        mgr.select_positioned(Some(3));
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::Cancel { team: 1 }],
        );
        assert_eq!(mgr.row_helper.shake.left, 40);
        // Slot 0x5c: a made dunk shakes the row.
        let mut mgr = on_the_row(&w);
        stepped(
            &mut mgr,
            &mut w,
            vec![
                street_sim::Event::Slam { player: 0, hoop: 0 },
                street_sim::Event::Score {
                    team: 0,
                    player: 0,
                    points: 2,
                    basket: street_sim::Basket::Dunk { advanced: false },
                },
            ],
        );
        assert_eq!(mgr.row_helper.shake.left, 39);
        // Slot 0x1a4: a shot-clock violation films the carrier.
        let mut mgr = on_the_row(&w);
        w.ball.state = street_sim::BallState::Loose;
        stepped(
            &mut mgr,
            &mut w,
            vec![street_sim::Event::ShotClockViolation { team: 0 }],
        );
        w.ball.state = street_sim::BallState::Held(0);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert_eq!(mgr.positioned_helper.focus_kind, CameraFocus::Actor(0));
        // Slot 0x274: a lesson starts on the row, with no intro cut.
        let mut mgr = CameraManager::new(0);
        mgr.update(&CameraTick {
            lesson: true,
            ..tick(&w)
        });
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        for _ in 0..200 {
            mgr.update(&CameraTick {
                lesson: true,
                ..tick(&w)
            });
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
    }

    #[test]
    fn native_wide_random_matches_the_raw_generator() {
        // Independent transcription of 0x3a16d8..0x3a1788 from the ELF state.
        let mut random = NativeWideRandom::default();
        let draws: Vec<u32> = (0..6).map(|_| random.draw()).collect();
        assert_eq!(
            draws,
            [
                0x559a_51ed,
                0x274e_a7f5,
                0xe827_f7e0,
                0x76b6_5632,
                0x7f1b_3feb,
                0xeae8_d5ee
            ]
        );
        // The counting word carries into the chain when it wraps.
        let mut random = NativeWideRandom([0, 0, 0, 0, 0, u32::MAX]);
        assert_eq!(random.draw(), 0);
        assert_eq!(random.0, [0; 6]);
    }

    #[test]
    fn courtside_placement_follows_fun_00303c80() {
        let mut random = NativeWideRandom::default();
        for (gamebreaker, court, half, scale, depth) in [
            (false, Some(3), 36.0, (3.5, 6.5), (-44.0, -35.0)),
            (false, Some(0), 36.0, (3.5, 6.5), (-35.0, -35.0)),
            (false, Some(6), 36.0, (3.5, 6.5), (-35.0, -35.0)),
            (true, Some(1), 23.0, (2.0, 5.0), (-44.0, -35.0)),
        ] {
            let mut helper = PositionedCameraHelper::default();
            let mut previous: f32 = 0.0;
            for _ in 0..200 {
                helper.place_courtside(&mut random, gamebreaker, court);
                let eye = helper.eye;
                assert!(eye.x.abs() <= half, "{eye}");
                assert!((0.8..=6.8).contains(&eye.y), "{eye}");
                assert!((depth.0..=depth.1).contains(&eye.z), "{eye}");
                assert!((scale.0..=scale.1).contains(&helper.distance_scale));
                // Never twice on one side of centre.
                assert!(previous * eye.x <= 0.0, "{previous} then {}", eye.x);
                previous = eye.x;
            }
        }
        // The retained original attract capture's three courtside eyes
        // (court 0 or 6) alternate sides inside these ranges.
        let observed: [[f32; 3]; 3] = [
            [-17.073_036, 4.917_22, -35.0],
            [28.487_4, 4.657_4, -35.0],
            [-19.209_6, 3.753_7, -35.0],
        ];
        for pair in observed.windows(2) {
            assert!(pair[0][0] * pair[1][0] < 0.0);
        }
        for eye in observed {
            assert!(eye[0].abs() <= 36.0 && (0.8..=6.8).contains(&eye[1]) && eye[2] == -35.0);
        }
    }

    #[test]
    fn positioned_lens_matches_retained_original_updates() {
        // street-camera-positioned-lens.json, samples 0 -> 1 (helper 0x498950).
        let mut helper = PositionedCameraHelper {
            eye: Vec3::new(
                f32::from_bits(0xc1b0_1833),
                f32::from_bits(0x4063_f9c7),
                f32::from_bits(0xc20c_0000),
            ),
            scalar: f32::from_bits(0x3e11_4796),
            distance_scale: f32::from_bits(0x40b6_bd36),
            ..Default::default()
        };
        let focus = Vec3::new(
            f32::from_bits(0xc21e_9dc7),
            f32::from_bits(0x3fd5_d49f),
            f32::from_bits(0x406c_c474),
        );
        let sample = CameraSourceSample {
            focus,
            ownerless: true,
            entity: None,
            source: Vec3::ZERO,
            entity_busy: false,
            stop_clip: false,
            rest: 0.0,
            release: 0,
        };
        let pose = helper.step(&sample, None, &mut None);
        assert_eq!(helper.focus, focus);
        assert_eq!(helper.target_ratio.to_bits(), 0x3e09_5aef);
        assert_eq!(pose.scalar.to_bits(), 0x3e10_e227);
        // A (re)selection settles: the lens takes the target directly.
        helper.select();
        let pose = helper.step(&sample, None, &mut None);
        assert_eq!(pose.scalar, helper.target_ratio);
    }

    #[test]
    fn positioned_focus_adds_the_owners_upper_body_like_the_original() {
        // street-native-full-rig-active.json sample 500: the midcourt cut
        // (scale 6.75) on the referee holding the ball in game phase 1,
        // which zeroes +0xc0's x and z (the origin).
        let bits = |b: [u32; 3]| {
            Vec3::new(
                f32::from_bits(b[0]),
                f32::from_bits(b[1]),
                f32::from_bits(b[2]),
            )
        };
        let actor = bits([0xbf3d_7b93, 0, 0xc041_7bc7]);
        let head = bits([0xbf30_62e9, 0x40bd_1d3d, 0xc031_249b]);
        let chest = bits([0xbf3e_2382, 0x408d_edaa, 0xc03f_df2a]);
        let mut helper = PositionedCameraHelper::default();
        helper.place_midcourt(1);
        assert_eq!(helper.eye, Vec3::new(-4.1, 8.0, -15.0));
        assert_eq!(helper.distance_scale, 6.75);
        helper.source = CameraSourceState {
            raw: Some(Vec3::ZERO),
            clamped: Vec3::ZERO,
            motion: Vec3::ZERO,
            ownerless: false,
            ..Default::default()
        };
        let sample = CameraSourceSample {
            focus: Vec3::new(0.0, actor.y, 0.0),
            ownerless: false,
            entity: Some(2),
            source: Vec3::ZERO,
            entity_busy: false,
            stop_clip: false,
            rest: 0.0,
            release: 0,
        };
        helper.step(&sample, Some([head - actor, chest - actor]), &mut Some(2));
        let observed = bits([0x3d29_3389, 0x40b4_76a6, 0x3e5a_3e71]);
        assert!(helper.focus.distance(observed) < 1e-5, "{}", helper.focus);
        assert!((helper.target_ratio - f32::from_bits(0x3ed8_c79d)).abs() < 1e-6);
    }

    #[test]
    fn a_loose_balls_bounce_flattens_the_rows_focus_until_a_shot_or_pass() {
        let ball = |y: f32, rise: f32, release: u8| CameraSourceSample {
            focus: Vec3::new(1.0, y, 2.0),
            ownerless: true,
            entity: None,
            source: Vec3::new(0.0, rise, 0.0),
            entity_busy: false,
            stop_clip: false,
            rest: 0.39,
            release,
        };
        let held = CameraSourceSample {
            focus: Vec3::new(1.0, 0.0, 2.0),
            ownerless: false,
            entity: Some(3),
            source: Vec3::ZERO,
            entity_busy: true,
            stop_clip: false,
            rest: 0.39,
            release: 0,
        };
        let mut last = None;
        let mut row = CameraSourceState::row();
        let mut other = CameraSourceState::default();
        for s in [&mut row, &mut other] {
            // Knocked loose (state 0), falling, then off the floor.
            s.update(&ball(3.0, -0.2, 0), false, &mut last);
            s.update(&ball(0.5, -0.1, 0), false, &mut last);
            assert!(!s.vertical);
            s.update(&ball(0.4, 0.05, 0), false, &mut last);
        }
        // Only the row's copy (FUN_003003f8's argument 0) sets the gate.
        assert!(row.vertical && !other.vertical);
        assert_eq!(row.raw.unwrap().y, 0.0);
        // High in the air it still reads 0 while the ball is loose.
        row.update(&ball(5.0, 0.1, 0), false, &mut last);
        assert_eq!(row.raw.unwrap().y, 0.0);
        // Picked up: the owner's focus, the gate kept.
        row.update(&held, false, &mut last);
        assert!(row.vertical);
        // Knocked loose again: still flat. A shot clears it.
        row.update(&ball(4.0, 0.1, 0), false, &mut last);
        assert_eq!(row.raw.unwrap().y, 0.0);
        row.update(&held, false, &mut last);
        row.update(&ball(7.0, 0.3, 5), false, &mut last);
        assert!(!row.vertical);
        assert_eq!(row.raw.unwrap().y, 7.0);
    }

    #[test]
    fn a_stopping_clip_keeps_the_stationary_gate_off_like_fun_00300b98() {
        let held = |x: f32, stop_clip: bool| CameraSourceSample {
            focus: Vec3::new(x, 0.0, 2.0),
            ownerless: false,
            entity: Some(1),
            source: Vec3::ZERO,
            entity_busy: false,
            stop_clip,
            rest: 0.39,
            release: 0,
        };
        let mut last = None;
        let mut s = CameraSourceState::row();
        s.update(&held(1.0, false), false, &mut last);
        s.update(&held(1.5, false), false, &mut last);
        assert_eq!(s.raw.unwrap().x, 1.0);
        // RunToStandDribble_L's root travel: the camera follows it.
        s.update(&held(2.0, true), false, &mut last);
        assert_eq!(s.raw.unwrap().x, 2.0);
        assert_eq!(s.motion.x, 1.0);
        assert_eq!(CAMERA_STOP_CLIPS, [46, 47, 174, 175, 176, 177]);
    }

    #[test]
    fn source_adapter_follows_fun_003003f8() {
        let ball = |focus: Vec3, source: Vec3| CameraSourceSample {
            focus,
            ownerless: true,
            entity: None,
            source,
            entity_busy: false,
            stop_clip: false,
            rest: 0.0,
            release: 0,
        };
        let held = |focus: Vec3, source: Vec3, busy: bool| CameraSourceSample {
            focus,
            ownerless: false,
            entity: Some(4),
            source,
            entity_busy: busy,
            stop_clip: false,
            rest: 0.0,
            release: 0,
        };
        let mut last = None;
        let mut s = CameraSourceState::default();
        // The first update has no history: the source vector.
        s.update(&ball(Vec3::new(1.0, 2.0, 3.0), Vec3::X), false, &mut last);
        assert_eq!(s.motion, Vec3::X);
        // Ordinary updates difference the raw focus.
        s.update(&ball(Vec3::new(1.5, 2.0, 2.0), Vec3::X), false, &mut last);
        assert_eq!(s.motion, Vec3::new(0.5, 0.0, -1.0));
        // A jump beyond 5280/(60*3600)*250 takes the source vector instead.
        s.update(&ball(Vec3::new(9.0, 2.0, 2.0), Vec3::Y), false, &mut last);
        assert_eq!(s.motion, Vec3::Y);
        // So does a change of the ownerless flag, even for a small step.
        s.update(
            &held(Vec3::new(9.0, 0.0, 2.0), Vec3::Z, true),
            false,
            &mut last,
        );
        assert_eq!(s.motion, Vec3::Z);
        // The same idle owner with a zero source vector holds the old focus.
        s.update(
            &held(Vec3::new(9.5, 0.0, 2.0), Vec3::ZERO, false),
            false,
            &mut last,
        );
        assert_eq!(s.motion, Vec3::ZERO);
        assert_eq!(s.raw, Some(Vec3::new(9.0, 0.0, 2.0)));
        // A busy owner follows its own difference.
        s.update(
            &held(Vec3::new(9.5, 1.0, 2.0), Vec3::ZERO, true),
            false,
            &mut last,
        );
        assert_eq!(s.motion, Vec3::new(0.5, 1.0, 0.0));
        // Settling skips both the gate and the substitution.
        s.update(&ball(Vec3::new(30.0, 1.0, 2.0), Vec3::Y), true, &mut last);
        assert_eq!(s.motion, Vec3::ZERO);
        // Past the -26.5 floor the clamped copy stops and drops its z motion.
        s.update(
            &ball(Vec3::new(30.0, 1.0, -26.0), Vec3::Y),
            false,
            &mut last,
        );
        s.update(
            &ball(Vec3::new(30.0, 1.0, -27.0), Vec3::Y),
            false,
            &mut last,
        );
        assert_eq!(s.clamped.z, -26.5);
        assert_eq!(s.motion, Vec3::ZERO);
        assert_eq!(s.raw, Some(Vec3::new(30.0, 1.0, -27.0)));
    }

    /// The Street School's Level 2 (GamLR2) rerun in the original, its
    /// replay frame 45: camera 3's node (`FUN_00331d08`) and the staged
    /// helper's pose, its `+0x98` and `+0x1b0`, and the view matrix the
    /// renderer built from them (`FUN_003046c0`), read through PINE.
    #[test]
    fn the_staged_camera_has_the_originals_lens_roll_and_view() {
        let camera = street_assets::timeline::Camera {
            position: [-10.809_503, 4.954_774, -9.080_201],
            target: [-13.640_046, 6.0, 14.531_385],
            fov: 50.0,
            heading: 189.55,
            target_actor: -1,
            flags: [0, 0, 3, 0, 22, 1],
        };
        let p = staged_pose(&camera, |_, _| panic!("not a following camera"), 0.33);
        assert!((p.scalar - 0.466_307_6).abs() < 1e-6, "{}", p.scalar);
        assert!((p.roll - 0.166_679).abs() < 1e-5, "{}", p.roll);
        let pose = CameraPose::looking_at(p.eye, p.at, p.scalar);
        assert!((pose.pitch + 0.043_924_62).abs() < 1e-5);
        assert!((pose.yaw + 0.119_311_63).abs() < 1e-5);
        // The renderer's view: its x and y columns (screen right and up).
        let mut active = ActiveCamera {
            eye: p.eye,
            target_focus: p.at,
            lens: CameraLens::Scalar(p.scalar),
            roll: p.roll,
            ..ActiveCamera::default()
        };
        active.manager = CameraManager::new(0);
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(active);
        let camera = world
            .spawn((
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                SidelineCamera,
            ))
            .id();
        world.run_system_once(follow_camera).unwrap();
        let t = *world.get::<Transform>(camera).unwrap();
        let near = |a: Vec3, b: [f32; 3]| (a - Vec3::from_array(b)).length() < 2e-4;
        assert!(
            near(
                t.right().into(),
                [-0.979_997_6, -0.165_748_27, -0.110_145_92]
            ),
            "{:?}",
            t.right()
        );
        assert!(
            near(t.up().into(), [-0.159_574_64, 0.985_190_03, -0.062_741_97]),
            "{:?}",
            t.up()
        );
        // The lens through the shared projection (zoom 1.3): 63.74 degrees
        // across at 4:3, the projection object's +0x110.
        let (across, _) = scalar_camera_projection(p.scalar, 4.0 / 3.0, CAMERA_PROJECTION_ZOOM);
        assert!((across - 63.740_67).abs() < 1e-3, "{across}");
    }

    /// The freeze's camera 1 follows script actor 1 (`+0x43` set): in the
    /// same run the original aimed it at the jumper bound to that slot,
    /// (-5.25, 1.502, 18.6), from (-20.628, 4.223, 7.031): yaw 0.9258.
    /// A sideways offset turns the yaw by `asin(x / 10)` and keeps the
    /// distance `sqrt(h^2 + x^2)`; the height offset rises over the point.
    #[test]
    fn a_following_staged_camera_aims_at_its_actor() {
        let mut camera = street_assets::timeline::Camera {
            position: [-20.628, 4.223, 7.031],
            target: [0.0, 0.0, 0.0],
            fov: 45.0,
            heading: 170.0,
            target_actor: 1,
            flags: [0, 1, 1, 0, 22, 1],
        };
        let point = Vec3::new(-5.25, 5.0, 18.6);
        let follow = |actor: i16, part: u8| {
            assert_eq!((actor, part), (1, 0));
            point
        };
        let p = staged_pose(&camera, follow, 0.33);
        assert!((p.at - point).length() < 1e-4, "{}", p.at);
        let pose = CameraPose::looking_at(p.eye, p.at, p.scalar);
        assert!((pose.yaw - 0.9258).abs() < 1e-3, "{}", pose.yaw);
        assert!((p.scalar - (22.5f32).to_radians().tan()).abs() < 1e-6);
        assert!((p.roll + 10f32.to_radians()).abs() < 1e-5);
        camera.target = [2.0, -1.0, 0.0];
        let p = staged_pose(&camera, |_, _| point, 0.33);
        let (dx, dz) = (point.x - p.eye.x, point.z - p.eye.z);
        let flat = (dx * dx + dz * dz).sqrt();
        let yaw = dx.atan2(dz) - 0.2f32.asin();
        let reach = (flat * flat + 4.0).sqrt();
        assert!(
            (p.at
                - Vec3::new(
                    p.eye.x + yaw.sin() * reach,
                    4.0,
                    p.eye.z + yaw.cos() * reach
                ))
            .length()
                < 1e-4
        );
        // A field outside 1..180 keeps the last lens; 0 is a quarter turn's.
        camera.fov = 0.5;
        assert_eq!(staged_pose(&camera, |_, _| point, 0.42).scalar, 0.42);
        camera.fov = 0.0;
        assert_eq!(staged_pose(&camera, |_, _| point, 0.42).scalar, 1.0);
    }

    #[test]
    fn intro_cut_holds_one_second_then_blends_to_the_row() {
        let w = camera_world();
        let mut mgr = CameraManager::new(0);
        let intro = CameraTick {
            staged: Some(StagedView {
                eye: Vec3::new(1.0, 2.0, 3.0),
                at: Vec3::ZERO,
                scalar: 0.7,
                roll: 0.0,
            }),
            intro_playing: true,
            ..tick(&w)
        };
        mgr.update(&intro);
        assert_eq!(mgr.active_helper, CameraHelperKind::Staged);
        let (eye2, _, lens2) = mgr.update(&intro);
        assert_eq!(mgr.active_helper, CameraHelperKind::Staged);
        assert_eq!(
            (eye2, lens2),
            (Vec3::new(1.0, 2.0, 3.0), CameraLens::Scalar(0.7))
        );
        // The intro's end: one of FUN_00303458's four eyes, scale 6.75.
        let (eye, _, _) = mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert!(eye.x.abs() == 4.1 && (eye.y == 0.5 || eye.y == 8.0) && eye.z == -15.0);
        assert_eq!(mgr.positioned_helper.distance_scale, 6.75);
        for _ in 0..INTRO_CUT_HOLD_TICKS {
            mgr.update(&tick(&w));
            assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        }
        // The next update starts the 55-update blend to a settled row.
        let mut counters = vec![];
        loop {
            mgr.update(&tick(&w));
            if mgr.active_helper != CameraHelperKind::Blend {
                break;
            }
            counters.push(mgr.blend_helper.as_ref().unwrap().counter);
        }
        assert_eq!(
            counters,
            (0..INTRO_CUT_BLEND_TICKS).rev().collect::<Vec<_>>()
        );
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // Only once a match.
        for _ in 0..200 {
            mgr.update(&tick(&w));
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
    }

    #[test]
    fn blend_mixes_poses_like_the_retained_original() {
        // street-native-full-rig-active.json sample 539: counter 53 of 55
        // between the midcourt cut and the settled Street row.
        let from = CameraPose {
            eye: Vec3::new(-4.1, 8.0, -15.0),
            pitch: 0.1482,
            yaw: 0.2571,
            scalar: 0.420_08,
        };
        let to = CameraPose {
            eye: Vec3::new(0.0, 12.0, -30.0),
            pitch: 0.2588,
            yaw: 0.0,
            scalar: 0.33,
        };
        let pose = CameraPose::mix(&from, &to, 53.0 / 55.0);
        assert!(pose.eye.distance(Vec3::new(-3.9509, 8.1455, -15.5455)) < 1e-4);
        assert!((pose.pitch - 0.1522).abs() < 1e-4);
        assert!((pose.yaw - 0.2477).abs() < 1e-4);
        assert!((pose.scalar - 0.4168).abs() < 1e-4);
        // The forward vector agrees with the native angle convention.
        let aimed = CameraPose::looking_at(Vec3::ZERO, Vec3::new(3.0, -4.0, 12.0), 0.3);
        assert!(aimed.forward().distance(Vec3::new(3.0, -4.0, 12.0) / 13.0) < 1e-3);
    }

    #[test]
    fn scripts_and_replays_select_helpers_like_the_listener() {
        let mut w = camera_world();
        let mut mgr = CameraManager::new(0);
        mgr.update(&tick(&w));
        mgr.intro_cut_age = None; // past the intro cut
        mgr.select_row();
        mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // A gamebreaker script: staged, then straight back to a settled row.
        let staged = CameraTick {
            staged: Some(StagedView {
                eye: Vec3::new(5.0, 9.0, -20.0),
                at: Vec3::ZERO,
                scalar: 0.9,
                roll: 0.0,
            }),
            ..tick(&w)
        };
        mgr.update(&staged);
        assert_eq!(mgr.active_helper, CameraHelperKind::Staged);
        let (eye, _, lens) = mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        assert!(mgr.blend_helper.is_none());
        assert_eq!(lens, CameraLens::Scalar(0.33));
        assert_eq!(eye.y, 12.0);
        // The instant replay: a courtside eye, then the row again.
        let replay = CameraTick {
            replay_playing: true,
            ..tick(&w)
        };
        let (eye, _, _) = mgr.update(&replay);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert!(eye.z <= -35.0 && (0.8..=6.8).contains(&eye.y));
        mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // In the attract match the replay's end places another courtside eye.
        mgr.update(&CameraTick {
            replay_playing: true,
            ..attract(&w)
        });
        let first = mgr.positioned_helper.eye;
        mgr.update(&attract(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert!(first.x * mgr.positioned_helper.eye.x <= 0.0);
        // A possession restart brings an ordinary match back to the row.
        w.tick += 1;
        mgr.update(&tick(&w));
        w.tick += 1;
        w.events = vec![street_sim::Event::Inbound { player: 3 }];
        mgr.select_positioned(Some(3));
        mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
    }

    #[test]
    fn attract_cuts_on_shot_releases_and_players_near_the_eye() {
        let mut w = camera_world();
        let mut mgr = CameraManager::new(0);
        mgr.update(&attract(&w));
        mgr.intro_cut_age = None;
        mgr.select_row();
        // FUN_003b42a0 from state 0 draws 12345: odd, so the coin cuts.
        mgr.coin_random = crate::progression_trace::NativeRandom::new(0);
        mgr.update(&attract(&w));
        w.tick += 1;
        w.ball.state = street_sim::BallState::Shot;
        mgr.update(&attract(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        // The same release in an ordinary match never cuts.
        let mut normal = CameraManager::new(0);
        w.ball.state = street_sim::BallState::Held(0);
        normal.update(&tick(&w));
        normal.intro_cut_age = None;
        normal.select_row();
        normal.update(&tick(&w));
        w.tick += 1;
        w.ball.state = street_sim::BallState::Shot;
        normal.update(&tick(&w));
        assert_eq!(normal.active_helper, CameraHelperKind::Row);
        // FUN_00304d70: a player within 3 units of the eye moves the camera.
        let mut mgr = CameraManager::new(0);
        mgr.update(&tick(&w));
        mgr.select_positioned(Some(3));
        mgr.update(&tick(&w));
        let eye = mgr.positioned_helper.eye;
        w.players[5].position = [eye.x + 1.0, eye.z];
        w.players[5].height = eye.y;
        mgr.update(&tick(&w));
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        assert_ne!(mgr.positioned_helper.eye, eye);
    }

    #[test]
    fn row_changes_settle_and_follow_their_lens() {
        let mut state = NativeCameraState::new(0);
        state.update(&RowInputs::default());
        assert!(state.initialized);
        assert_eq!(state.scalar, 0.33);
        state.select_mode(2);
        state.update(&RowInputs {
            focus: Vec3::X,
            motion: Vec3::X,
            ..RowInputs::default()
        });
        // FUN_002f7bb0: a new row settles onto its pose and lens.
        assert_eq!(state.mode, 2);
        assert_eq!(state.scalar, 0.18);
        assert_eq!(state.eye, [13.0, 22.0, -65.0]);
    }

    #[test]
    fn scenery_culls_the_sides_behind_the_camera_like_fun_002f9d10() {
        use bevy::ecs::system::RunSystemOnce;
        // FUN_002f9ab0's tags (strstr, so "_cleft" carries "clef").
        assert_eq!(scenery_mask("AHD_FNC1_cleft"), 1);
        assert_eq!(scenery_mask("AIC_Glow_crit"), 2);
        assert_eq!(scenery_mask("opaque_rail_cbot"), 8);
        assert_eq!(scenery_mask("Q_ST175_cbot"), 8);
        assert_eq!(scenery_mask("A_ST231"), 0);
        // FUN_00304e38's thresholds.
        assert_eq!(scenery_side_flags(Vec3::new(0.0, 12.0, -30.0)), 8);
        assert_eq!(scenery_side_flags(Vec3::new(0.0, 12.0, -27.8)), 0);
        assert_eq!(scenery_side_flags(Vec3::new(45.0, 3.0, 30.0)), 5);
        assert_eq!(scenery_side_flags(Vec3::new(-45.0, 3.0, -40.0)), 10);

        let mut world = World::new();
        let near = world
            .spawn((DrawGroup("opaque_cbot".into()), Visibility::Inherited))
            .id();
        let left = world
            .spawn((DrawGroup("opaque_cleft".into()), Visibility::Inherited))
            .id();
        let plain = world
            .spawn((DrawGroup("opaque".into()), Visibility::Hidden))
            .id();
        let w = camera_world();
        let mut active = ActiveCamera::default();
        update_camera_step(&tick(&w), &mut active);
        active.eye = Vec3::new(13.9, 5.7, -43.3);
        world.insert_resource(active);
        world.run_system_once(cull_scenery).unwrap();
        assert_eq!(world.get::<Visibility>(near), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Visibility>(left), Some(&Visibility::Inherited));
        assert_eq!(world.get::<Visibility>(plain), Some(&Visibility::Hidden));
        // Beyond +x the near side comes back and the +x side goes.
        world.resource_mut::<ActiveCamera>().eye = Vec3::new(50.0, 4.0, 0.0);
        world.run_system_once(cull_scenery).unwrap();
        assert_eq!(world.get::<Visibility>(near), Some(&Visibility::Inherited));
        assert_eq!(world.get::<Visibility>(left), Some(&Visibility::Hidden));
        // While a script's camera is on screen everything is shown.
        let staged = CameraTick {
            staged: Some(StagedView {
                eye: Vec3::new(0.0, 5.0, -60.0),
                at: Vec3::ZERO,
                scalar: 0.7,
                roll: 0.0,
            }),
            ..tick(&w)
        };
        update_camera_step(&staged, &mut world.resource_mut::<ActiveCamera>());
        world.run_system_once(cull_scenery).unwrap();
        assert_eq!(world.get::<Visibility>(near), Some(&Visibility::Inherited));
        assert_eq!(world.get::<Visibility>(left), Some(&Visibility::Inherited));
    }

    /// A row state from a played-match case (`tools/compare_camera_played.py`).
    fn played_case_state(w: &[u32]) -> NativeCameraState {
        let f = f32::from_bits;
        let mode = w[15] as usize;
        let row = 0x0049_8478 + 0x34 * w[15];
        let depth = match w[16] {
            p if p == row + 0x10 => RowDepth::Base,
            p if p == row + 0x14 => RowDepth::Low,
            p if p == row + 0x18 => RowDepth::High,
            0x0040_1080 => RowDepth::Shot,
            p => panic!("depth pointer {p:#x}"),
        };
        let left = w[21] as u16;
        NativeCameraState {
            mode,
            depth,
            eye: [f(w[0]), f(w[1]), f(w[2])],
            angles: [f(w[3]), f(w[4])],
            scalar: f(w[5]),
            velocity: [f(w[6]), f(w[7]), f(w[8]), f(w[9])],
            initialized: true,
            shake: RowShake {
                left,
                updates: if f(w[12]) > 0.0 {
                    (1.0 / f(w[12])).round() as u16
                } else {
                    1
                },
                cycles: f(w[13]) as u16,
                amplitude: f(w[14]),
            },
            transition: w[17] as i32,
            still: w[18] != 0,
            drift: f(w[10]),
            yaw_offset: f(w[11]),
            settled: w[19] != 0,
            last_possession: w[20] as i32,
            offsets: StreetOffsets {
                height: f(w[22]),
                lens: f(w[23]),
                pass_depth_scale: f(w[24]),
                shot_depth_scale: f(w[25]),
                pass_yaw_half: w[26] != 0,
            },
        }
    }

    /// A row update's reads from a played-match case.
    fn played_case_inputs(w: &[u32]) -> RowInputs {
        let f = f32::from_bits;
        let v = |i: usize| Vec3::new(f(w[i]), f(w[i + 1]), f(w[i + 2]));
        RowInputs {
            focus: v(0),
            motion: v(3),
            reference: v(6),
            margin: f(w[9]),
            free_ball: (w[10] != 0).then(|| v(11)),
            vertical_gate: w[14] != 0,
            opening: w[15] != 0,
            possession: w[16] as i32,
            side: w[17] as i32 as f32,
            live: w[18] != 0,
            ball_state: w[19],
            gamebreaker_shot: w[20] != 0,
            ball_distance: f(w[21]),
            owner_in_air: w[22] != 0,
            pass_to_flier: w[23] != 0,
            far_shot: w[24] != 0,
            team_busy: w[25] != 0,
        }
    }

    /// Words of one played-match case (`compare_camera_played.py` format,
    /// without the kind and frame).
    fn case_words(text: &str) -> Vec<u32> {
        text.split_whitespace()
            .map(|t| u32::from_str_radix(t, 16).unwrap())
            .collect()
    }

    #[test]
    fn the_row_settles_framing_its_focus_like_fun_002fabe8() {
        // Retained from street-camera-played-session1.jsonl frames 3727 and
        // 3728 (a played Pick Up match, manager +0x4c 0): the restart's
        // inbound pass selects the Street row (FUN_00302510); the original
        // poses it aimed at the focus, runs the settle loop with the world
        // still, and its next update lands here. The camera does not end
        // up aimed at the focus: the settle turns until the possession's
        // rim enters its window.
        let w = case_words(
            "c0800000 41400000 c21ab56b 3dfc9083 bf5732ef 3ea8f5cd 00000000 00000000 \
             00000000 00000000 00000000 00000000 3ccccccd 41700000 3f000000 00000000 \
             00498488 00000000 00000001 00000001 00000000 00000000 00000000 34a0c000 \
             00000000 00000000 00000000 \
             c22a08fc 40c92b0a c19d16f5 00000000 00000000 00000000 c2130000 41400000 \
             00000000 00000000 00000001 c22a08fc 40c92b0a c19d16f5 00000000 00000000 \
             00000001 ffffffff 00000000 00000004 00000000 41a3b4b7 00000000 00000000 \
             00000000 00000000 \
             c2297084 40cf7609 c19eb999 3e187800 3e495fe0 be515200 c2130000 41400000 \
             00000000 00000000 00000001 c2297084 40cf7609 c19eb999 00000000 00000000 \
             00000001 ffffffff 00000001 00000004 00000000 41a4f29d 00000000 00000000 \
             00000000 00000000",
        );
        let mut state = played_case_state(&w[..27]);
        state.initialized = false;
        let selected = played_case_inputs(&w[27..53]);
        state.update(&selected);
        // The settle's pose aims at the focus; the loop turns the yaw by
        // about 0.2 to frame the rim at -36.75.
        let (_, aimed) =
            native_angles((selected.focus - Vec3::new(-4.0, 12.0, state.eye[2])).to_array());
        assert!((state.angles[1] - aimed).abs() > 0.15);
        assert!(state.settled);
        state.update(&played_case_inputs(&w[53..79]));
        let f = f32::from_bits;
        let want = [
            f(0xc080_0000),
            12.0,
            f(0xc21b_a8f8),
            f(0x3e07_19d3),
            f(0xbf67_5158),
        ];
        let got = [
            state.eye[0],
            state.eye[1],
            state.eye[2],
            state.angles[0],
            state.angles[1],
        ];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-5, "{got:?} {want:?}");
        }
        assert_eq!(state.velocity[1], f(0xbcbd_9200));
        assert!((state.velocity[3] - f(0x3a25_3410)).abs() < 1e-7);
    }

    #[test]
    fn street_drops_for_a_dunk_and_climbs_back_like_fun_002f8370() {
        let mut state = NativeCameraState::new(0);
        let held = RowInputs {
            focus: Vec3::new(30.0, 0.0, 0.0),
            ..RowInputs::default()
        };
        state.update(&held);
        // FUN_002fb690: an owner dunking takes the low depth; Street's
        // offsets then lower the eye 0.16 and the lens 0.003 an update,
        // to 4 and 0.18.
        let dunking = RowInputs {
            owner_in_air: true,
            ..held
        };
        state.update(&dunking);
        // Street's low and base depths are both -30: the change arrives at once.
        assert_eq!((state.depth, state.transition), (RowDepth::Low, 0));
        assert_eq!(state.eye[1], 12.0);
        // Each update applies the offsets before moving them.
        state.update(&dunking);
        assert_eq!(state.eye[1], 12.0);
        state.update(&dunking);
        assert!((state.eye[1] - 11.84).abs() < 1e-5);
        assert!((state.scalar - (0.33 - 0.003)).abs() < 1e-6);
        for _ in 0..80 {
            state.update(&dunking);
        }
        assert!((state.eye[1] - 4.0).abs() < 0.16);
        assert!((state.scalar - 0.18).abs() < 0.003);
        // Back on the floor the base pointer brings it up at
        // (12 - 4) / 50 an update, the height offset stopping at zero and
        // the lens offset carrying past it.
        state.update(&held);
        assert_eq!((state.depth, state.transition), (RowDepth::Base, 0));
        for _ in 0..120 {
            state.update(&held);
        }
        assert_eq!(state.eye[1], 12.0);
        // A residue below an ulp of 12 stays: 12 + it is 12.
        assert!(state.offsets.height.abs() < 1e-6);
        assert!(state.scalar >= 0.33 && state.scalar < 0.333 + 1e-6);
    }

    #[test]
    fn court_steps_between_its_depths_like_fun_002fb690() {
        // Court's depths: base -40, low -26, high -55. While +0x12c is set
        // the depth steps by DAT_004d8bd0 (1 / 45) × (-26 - -55) an update
        // and keeps stepping with the focus still.
        let mut state = NativeCameraState::new(1);
        let held = RowInputs::default();
        state.update(&held);
        assert_eq!(state.eye[2], -40.0);
        let dunking = RowInputs {
            owner_in_air: true,
            ..held
        };
        state.update(&dunking);
        assert_eq!((state.depth, state.transition), (RowDepth::Low, -1));
        let step = (1.0 / 45.0) * 29.0;
        let mut z = state.eye[2];
        for _ in 0..40 {
            state.update(&dunking);
            assert!(state.eye[2] >= z && state.velocity[1] <= step + 1e-6);
            z = state.eye[2];
        }
        // The rate runs on past the target before it decays.
        assert!((state.eye[2] + 26.0).abs() < 2.0, "{:?}", state);
        // A side with a flier or a computer carrier takes the high depth.
        let mut busy = NativeCameraState::new(1);
        busy.update(&held);
        busy.update(&RowInputs {
            team_busy: true,
            ..held
        });
        assert_eq!((busy.depth, busy.transition), (RowDepth::High, 1));
    }

    #[test]
    fn a_close_shot_takes_the_shot_depth_like_fun_002fb690() {
        let mut state = NativeCameraState::new(0);
        let held = RowInputs {
            focus: Vec3::new(20.0, 0.0, 5.0),
            ..RowInputs::default()
        };
        state.update(&held);
        // A shot launched 38.75 or closer (ball+0x154) from the rim: the
        // shot depth, its scale from the focus's depth, the lens toward 0.25.
        let shot = RowInputs {
            ball_state: 5,
            ball_distance: 20.0,
            focus: Vec3::new(20.0, 8.0, 5.0),
            ..held
        };
        state.update(&shot);
        assert_eq!((state.depth, state.transition), (RowDepth::Shot, 0));
        let scale = 0.6 - (0.6 - 0.125) * (5.0 + 26.5) / 53.0;
        assert!((state.offsets.shot_depth_scale - scale).abs() < 1e-6);
        state.update(&shot);
        state.update(&shot);
        assert!((state.scalar - (0.33 - 0.08 / 50.0)).abs() < 1e-6);
        // A long shot keeps the base; the shot depth ends with the ball loose.
        let mut long = NativeCameraState::new(0);
        long.update(&held);
        long.update(&RowInputs {
            ball_distance: 40.0,
            ..shot
        });
        assert_eq!(long.depth, RowDepth::Base);
        state.update(&RowInputs {
            ball_state: 1,
            ..shot
        });
        assert_eq!((state.depth, state.transition), (RowDepth::Base, 0));
    }

    #[test]
    fn a_resting_street_camera_waits_for_the_focus_to_move_like_fun_002fda08() {
        let mut state = NativeCameraState::new(0);
        // Nobody in possession (no side): the yaw only keeps the focus.
        let still = RowInputs {
            focus: Vec3::new(0.0, 0.0, 0.0),
            side: 0.0,
            possession: -1,
            ..RowInputs::default()
        };
        state.update(&still);
        state.update(&still);
        assert!(state.settled && state.still && state.drift == 0.0);
        // A small move: x and yaw are damped by (drift / 125)^4, so the
        // eye does not follow a step of a few pixels.
        let step = RowInputs {
            focus: still.focus + Vec3::new(0.3, 0.0, 0.0),
            motion: Vec3::new(0.3, 0.0, 0.0),
            ..still
        };
        let yaw = state.angles[1];
        state.update(&step);
        assert!(state.drift != 0.0 && state.drift.abs() < ROW_STILL_DRIFT);
        assert!(
            (state.angles[1] - yaw).abs() < 1e-3,
            "{} {yaw}",
            state.angles[1]
        );
        assert_eq!(state.velocity[0], 0.0);
    }

    #[test]
    fn a_close_far_side_holder_turns_the_framing_yaw_like_fun_002f8370() {
        // A held ball on the far half (z > 0) between 17.75 and 29.75 of
        // the near rim, |x| < 23: the near rim's zones replace the
        // reference's; with it at the right edge the framing yaw ramps by
        // 0.03 an update up to 0.18 x (distance - 17.75) / 12.
        let row = NativeCameraState::new(0);
        let input = RowInputs {
            focus: Vec3::new(14.0, 0.0, 5.0),
            ..RowInputs::default()
        };
        let mut s = row;
        s.update(&input);
        let distance = ((36.75_f32 - 14.0).powi(2) + 25.0).sqrt();
        let limit = 0.18 * (distance - 17.75) / 12.0;
        for _ in 0..20 {
            s.update(&input);
        }
        assert!(s.yaw_offset.abs() <= limit + 1e-6);
        // Any other ball state clears it.
        s.update(&RowInputs {
            ball_state: 1,
            ..input
        });
        assert_eq!(s.yaw_offset, 0.0);
    }

    #[test]
    fn the_row_leads_toward_the_rim_its_side_attacks() {
        // The side byte is the direction the possession attacks
        // (FUN_001404c0); team 0 attacks +x (side +1, the original's byte),
        // so its Street eye leads to +x of the focus, within the bound.
        assert_eq!(street_sim::restart::side(0), 1.0);
        let mut w = camera_world();
        w.possession = 0;
        w.players[0].position = [1.0, 0.0];
        w.ball.state = street_sim::BallState::Held(0);
        let mut mgr = on_the_row(&w);
        let (eye, _, _) = mgr.update(&tick(&w));
        assert_eq!(eye.x, 4.0);
        w.possession = 1;
        w.players[3].position = [-1.0, 0.0];
        w.ball.state = street_sim::BallState::Held(3);
        mgr.select_row();
        let (eye, _, _) = mgr.update(&tick(&w));
        assert_eq!(eye.x, -4.0);
    }

    #[test]
    fn the_inbound_walk_puts_a_courtside_eye_by_the_spot_like_fun_003025e8() {
        let mut w = camera_world();
        let seated = [true, false, false, false, false, false];
        let mut mgr = on_the_row(&w);
        // A restart for team 0, whose inbounder (the plan's first slot)
        // has just picked the ball up.
        w.possession = 0;
        w.phase = street_sim::Phase::DeadBall {
            left: 20.0,
            team: 0,
        };
        assert!(w.restarting());
        let plan = w.restart_plan();
        let inbounder = plan.order[0];
        w.ball.state = street_sim::BallState::Loose;
        stepped_with(&mut mgr, &mut w, &seated);
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        w.ball.state = street_sim::BallState::Held(inbounder);
        stepped_with(&mut mgr, &mut w, &seated);
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
        // The next update is the walk's first: the eye goes up beside the
        // spot, on the receiver.
        stepped_with(&mut mgr, &mut w, &seated);
        assert_eq!(mgr.active_helper, CameraHelperKind::Positioned);
        let heights: [[f32; 3]; 2] =
            std::array::from_fn(|t| std::array::from_fn(|k| w.players[3 * t + k].height_ft()));
        let spot = street_sim::restart::spot_at(plan.row, plan.order, 0, 0, 0, &heights).position;
        let eye = if 0.0 < spot[0] {
            Vec3::new(spot[0] + 7.5, 12.0, spot[2] - 3.0)
        } else {
            Vec3::new(spot[0] - 7.5, 12.0, spot[2] + 3.0)
        };
        assert_eq!(mgr.positioned_helper.eye, eye);
        assert_eq!(mgr.positioned_helper.distance_scale, 6.75);
        assert_eq!(
            mgr.positioned_helper.focus_kind,
            CameraFocus::Actor(plan.order[1])
        );
        // Once a restart; and never for a side nobody plays.
        stepped_with(&mut mgr, &mut w, &seated);
        let mut w = camera_world();
        let mut mgr = on_the_row(&w);
        w.possession = 1;
        w.phase = street_sim::Phase::DeadBall {
            left: 20.0,
            team: 1,
        };
        let inbounder = 3 + w.restart_plan().order[0];
        w.ball.state = street_sim::BallState::Held(inbounder);
        for _ in 0..3 {
            stepped_with(&mut mgr, &mut w, &seated);
        }
        assert_eq!(mgr.active_helper, CameraHelperKind::Row);
    }

    /// One stepped update with these players seated.
    fn stepped_with(mgr: &mut CameraManager, w: &mut street_sim::World, seated: &[bool]) {
        w.tick += 1;
        mgr.update(&CameraTick { seated, ..tick(w) });
    }

    /// Opt-in original-runtime check of the courtside (positioned) helper's
    /// angular controller in a played match: its "P" cases (the
    /// shot-clock violation's camera, the inbound walk's and the replays')
    /// seed `PositionedCameraHelper` with the original's state and its
    /// `+0xc0..+0xf0`/`+0xe0` reads and run `follow` once.
    #[test]
    fn played_match_courtside_follows_the_original() {
        let Some(path) = std::env::var_os("STREET_CAMERA_PLAYED_CASES") else {
            return;
        };
        let text = std::fs::read_to_string(path).expect("read played camera cases");
        let f = f32::from_bits;
        let (mut cases, mut close, mut tight, mut framing) = (0, 0, 0, 0);
        let mut worst = (0.0_f32, String::new());
        for line in text.lines().filter(|l| l.starts_with("P ")) {
            let w: Vec<u32> = line
                .split_whitespace()
                .skip(2)
                .map(|t| u32::from_str_radix(t, 16).expect("hex word"))
                .collect();
            let v = |i: usize| Vec3::new(f(w[i]), f(w[i + 1]), f(w[i + 2]));
            let mut helper = PositionedCameraHelper {
                eye: v(0),
                pitch: f(w[3]),
                yaw: f(w[4]),
                scalar: f(w[5]),
                pitch_rate: f(w[6]),
                yaw_rate: f(w[7]),
                lead: f(w[8]),
                lead_rate: f(w[9]),
                distance_scale: f(w[10]),
                framing: w[11] as u8,
                ..PositionedCameraHelper::default()
            };
            helper.source = CameraSourceState {
                raw: Some(v(12)),
                clamped: v(15),
                motion: v(18),
                ownerless: false,
                ..Default::default()
            };
            helper.focus = v(21);
            helper.follow(false, w[24] != 0);
            let e = &w[25..];
            let got = [
                helper.pitch,
                helper.yaw,
                helper.scalar,
                helper.pitch_rate,
                helper.yaw_rate,
                helper.lead,
                helper.lead_rate,
                helper.target_ratio,
            ];
            let want: Vec<f32> = e[..8].iter().map(|&x| f(x)).collect();
            let error = got
                .iter()
                .zip(&want)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            cases += 1;
            close += usize::from(error < 1e-4);
            tight += usize::from(error < 1e-6);
            framing += usize::from(helper.framing as u32 == e[8]);
            if error > worst.0 {
                worst = (
                    error,
                    format!("{got:?} {want:?} framing {} {}", helper.framing, e[8]),
                );
            }
        }
        eprintln!(
            "played courtside: {cases} updates, {tight} within 1e-6, {close} within 1e-4, +0x134 equal in {framing}, worst {worst:?}"
        );
        assert!(cases > 0);
        // Since 2026-10-24 (the projection's VU0 chain, the lead's and the
        // window's arithmetic and `cvt.w.s` truncation): every update.
        assert_eq!(tight, cases, "courtside updates within 1e-6");
        assert_eq!(framing, cases, "courtside +0x134");
        assert!(close >= tight);
    }

    /// Opt-in original-runtime check of the row helper in a match a person
    /// plays (manager `+0x4c == 0`, front-end mode 2): each case is one
    /// update of the original's row, or a selection's settle and the next
    /// update, read from a capture (`tools/capture_camera_played.py`, then
    /// `tools/compare_camera_played.py`; STREET_CAMERA_PLAYED_CASES). The
    /// production `NativeCameraState::update` starts from the original's
    /// state and reads, and must land on the original's next state.
    #[test]
    fn played_match_row_follows_the_original() {
        let Some(path) = std::env::var_os("STREET_CAMERA_PLAYED_CASES") else {
            return;
        };
        let text = std::fs::read_to_string(path).expect("read played camera cases");
        let (mut cases, mut settles, mut close, mut exact_depth) = (0, 0, 0, 0);
        let (mut settles_close, mut tight) = (0, 0);
        let mut worst = (0.0_f32, String::new());
        // The free run: the production row carried from update to update
        // on the capture's reads alone, seeded at each break in the chain.
        let mut running: Option<(u32, NativeCameraState)> = None;
        let (mut chained, mut chained_close, mut chained_worst) = (0, 0, 0.0_f32);
        for line in text.lines() {
            let mut tokens = line.split_whitespace();
            let kind = tokens.next().expect("case kind");
            if kind == "P" {
                continue;
            }
            let mut words = tokens.map(|t| u32::from_str_radix(t, 16).expect("hex word"));
            let frame = words.next().expect("case frame");
            let w: Vec<u32> = words.collect();
            let free = running
                .take()
                .filter(|(at, _)| kind == "S" && *at + 1 == frame)
                .map(|(_, state)| state);
            let mut state = played_case_state(&w[..27]);
            let first = played_case_inputs(&w[27..53]);
            let expected = if kind == "T" {
                settles += 1;
                state.initialized = false;
                state.update(&first);
                state.update(&played_case_inputs(&w[53..79]));
                &w[79..94]
            } else {
                state.update(&first);
                &w[53..68]
            };
            cases += 1;
            let f = f32::from_bits;
            let got = [
                state.eye[0],
                state.eye[1],
                state.eye[2],
                state.angles[0],
                state.angles[1],
                state.scalar,
                state.velocity[0],
                state.velocity[1],
                state.velocity[2],
                state.velocity[3],
                state.yaw_offset,
            ];
            let want = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11].map(|i| f(expected[i]));
            let error = got
                .iter()
                .zip(want)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            let depth = played_case_state(&{
                let mut words = w[..27].to_vec();
                words[16] = expected[13];
                words
            })
            .depth;
            if depth == state.depth && expected[12] as i32 == state.transition {
                exact_depth += 1;
            } else if cases - exact_depth < 4 {
                eprintln!(
                    "depth {:?}/{} want {depth:?}/{}: {line}",
                    state.depth, state.transition, expected[12] as i32
                );
            }
            tight += usize::from(error < 1e-6);
            if error < 1e-4 {
                settles_close += usize::from(kind == "T");
                close += 1;
            } else if kind == "T" || cases - close <= 4 {
                eprintln!("off by {error} at update {frame}: {kind} {got:?} {want:?}");
            }
            if error > worst.0 {
                worst = (error, format!("{kind} {got:?} {want:?}"));
            }
            let next = match free {
                Some(mut carried) => {
                    // A shake starts from an event in the game update
                    // (FUN_002f8338): an input, as the reads are.
                    let shake = played_case_state(&w[..27]).shake;
                    if shake.left > carried.shake.left {
                        carried.shake = shake;
                    }
                    carried.update(&first);
                    let pose = [
                        carried.eye[0],
                        carried.eye[1],
                        carried.eye[2],
                        carried.angles[0],
                        carried.angles[1],
                        carried.scalar,
                    ];
                    let drift = pose
                        .iter()
                        .zip(&want[..6])
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0_f32, f32::max);
                    chained += 1;
                    chained_close += usize::from(drift < 1e-3);
                    chained_worst = chained_worst.max(drift);
                    carried
                }
                None => state,
            };
            // A settle's case ends on a read inside its update; the chain
            // starts again at the next ordinary case.
            running = (kind == "S").then_some((frame, next));
        }
        eprintln!(
            "played row: {cases} cases ({settles} settles, {settles_close} of them within 1e-4), {tight} within 1e-6, {close} within 1e-4, depth and transition exact in {exact_depth}, worst {worst:?}"
        );
        eprintln!(
            "free run: {chained} carried updates, {chained_close} within 1e-3 of the original's pose, worst {chained_worst}"
        );
        assert!(cases > 0);
        assert!(
            chained_close * 100 >= chained * 99,
            "the carried row drifts from the original: {chained_close} of {chained}"
        );
        assert_eq!(
            exact_depth, cases,
            "the depth pointer and +0x12c follow the original"
        );
        assert!(
            close * 1000 >= cases * 995,
            "fewer than 99.5% of updates within 1e-4: {close} of {cases}"
        );
    }

    #[test]
    fn follow_camera_converts_the_published_lens() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let camera = world
            .spawn((
                Transform::default(),
                Projection::Perspective(PerspectiveProjection::default()),
                SidelineCamera,
            ))
            .id();
        let w = camera_world();
        let mut active = ActiveCamera::default();
        update_camera_step(&tick(&w), &mut active);
        active.lens = CameraLens::Scalar(0.15);
        world.insert_resource(active);
        world.run_system_once(follow_camera).unwrap();
        let fov = |world: &World| match world.get::<Projection>(camera) {
            Some(Projection::Perspective(p)) => p.fov,
            _ => f32::NAN,
        };
        assert_eq!(fov(&world), mode_vertical_fov(3, Picture::Standard));
        // The saved Widescreen choice, and the run's --widescreen over it.
        let mut saved = crate::options::Options::default();
        saved.0.set_picture(Picture::Anamorphic);
        world.insert_resource(saved);
        world.run_system_once(follow_camera).unwrap();
        assert_eq!(fov(&world), mode_vertical_fov(3, Picture::Anamorphic));
        world.insert_resource(crate::options::PictureOverride(Picture::Wide));
        world.run_system_once(follow_camera).unwrap();
        assert_eq!(fov(&world), mode_vertical_fov(3, Picture::Wide));
        world.remove_resource::<crate::options::PictureOverride>();
        world.resource_mut::<ActiveCamera>().lens = CameraLens::Fov(0.8);
        world.run_system_once(follow_camera).unwrap();
        assert_eq!(fov(&world), 0.8);
    }

    #[test]
    fn native_held_attachment_matches_captured_pine_sample() {
        // Animation 78, frame 9, mode 2 from street-internal-flight.json sample 0
        let hand_t = Vec3::new(
            f32::from_bits(0xc0f3_c1f5),
            f32::from_bits(0x4098_b8d4),
            f32::from_bits(0xc0b0_4cb9),
        );
        let hand_r = Quat::from_xyzw(
            f32::from_bits(0x3f53_3f25),
            f32::from_bits(0xbd0d_6d88),
            f32::from_bits(0xbeab_6142),
            f32::from_bits(0xbee8_736d),
        );
        let body_scale_x = f32::from_bits(0x3f99_9999);
        let ball_r = 0.46875;
        let observed_ball = Vec3::new(
            f32::from_bits(0xc0e0_0e4c),
            f32::from_bits(0x4090_ede7),
            f32::from_bits(0xc0b3_ce9f),
        );

        let pred = super::native_held_attachment(hand_t, hand_r, body_scale_x, ball_r, 2);
        let err = (pred - observed_ball).length();
        assert!(err < 1e-6, "attachment error {err} exceeds tolerance");
    }
}
