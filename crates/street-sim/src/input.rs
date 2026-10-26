//! One player's controls for one tick: the whole PS2 pad, packed so that it
//! is identical on every machine and small enough to record or send (6 bytes).

/// Pad buttons, by what the game's input script (`CSCRIPTS/INPUT.CFG`)
/// binds them to.
pub mod button {
    /// Circle: shoot.
    pub const SHOOT: u16 = 1 << 0;
    /// Cross: pass.
    pub const PASS: u16 = 1 << 1;
    /// Square: juke (a trick move) with the ball, steal without it.
    pub const TRICK: u16 = 1 << 2;
    /// Triangle: rebound, block, or call a play.
    pub const JUMP: u16 = 1 << 3;
    /// The four turbo buttons. Moves care how many are held.
    pub const L1: u16 = 1 << 4;
    pub const R1: u16 = 1 << 5;
    pub const L2: u16 = 1 << 6;
    pub const R2: u16 = 1 << 7;
    /// Left stick click: kick pass.
    pub const L3: u16 = 1 << 8;
    /// Right stick click: strategy (call a pick or an alley-oop).
    pub const R3: u16 = 1 << 9;
    /// Select: pocket or cancel a gamebreaker.
    pub const SELECT: u16 = 1 << 10;
    /// Not a button: the dive (the pad's 0x1000000). A person's comes from
    /// tapping a turbo again ([`crate::World::dive_tap`]); a computer's pad
    /// sets it ([`crate::dive`]).
    pub const DIVE: u16 = 1 << 11;

    pub const TURBOS: u16 = L1 | R1 | L2 | R2;
    pub const ALL: u16 = (1 << 12) - 1;
    /// Not a button: a computer screener's pick (the pad's 0x4000000,
    /// [`crate::ai`]'s "set pick" as he steps into the defender), which
    /// [`crate::World`] plays as [`crate::trick::PICKS`]. No controller
    /// sets it.
    pub const PICK: u16 = 1 << 12;
    /// Not a button: a computer screener's stance (the pad's 0x400000,
    /// held while he waits at his spot), which [`crate::World`] plays as
    /// [`crate::stand::PICK_STANCE`]. No controller sets it.
    pub const PICK_STANCE: u16 = 1 << 13;
    /// Not a button: the computer drives this player this tick. The
    /// world replaces such an input with the pad its AI builds
    /// ([`crate::ai`]), as the original's computer players fill the same
    /// pad word people's controllers do.
    pub const COMPUTER: u16 = 1 << 15;
}

/// The controls for one tick. Sticks are in court axes (x, z), so clients
/// turn the physical stick into court directions before filling this in
/// (a person's as the original's pad builder does, [`person_words`] and
/// [`Input::set_words`]); each axis is quantised to -127..=127, and a
/// stick's length decodes to the original's strength exactly
/// ([`Input::strength`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Input {
    /// Left stick: movement.
    pub stick: [i8; 2],
    /// Right stick: give 'n go aim.
    pub aim: [i8; 2],
    /// Held buttons, a set of [`button`] bits.
    pub buttons: u16,
    /// The active camera's yaw word (`DAT_00402020`+0x94, `atan2` of its
    /// look's x and z, `trunc(yaw × 162.97 + 0.5)` units): the word that
    /// moves along the screen's *down* (a word `a` moves along `(-sin a,
    /// -cos a)`), so a pad's own word plus it is the court's
    /// (`FUN_001587d8`, `0x158d9c`), and the pass target adds it to the
    /// facing word of a pass at rest (`FUN_00146278`, `0x146510`). 0 where
    /// there is no camera (the screen's up along +z).
    pub view: u16,
    /// The actors off the screen the person sees, bit k for actor k
    /// (`FUN_00171920`: the actor's feet less 2, `(x, +0x260 - 2, z)`,
    /// through the active camera's projection `FUN_002ffe70` as the last
    /// frame left it, outside the screen `FUN_0030fa50` gives or behind
    /// the near plane). A switch's pick adds 20 to a teammate off it
    /// ([`World::switch_target`](crate::World::switch_target)). 0 where
    /// there is no camera: every actor on it. Carried in the view word's
    /// top six bits ([`Input::to_bytes`]).
    pub off_screen: u8,
}

fn quantise(v: f32) -> i8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8
}

impl Input {
    /// Movement toward `stick` (court axes, length up to 1), no buttons.
    pub fn moving(stick: [f32; 2]) -> Self {
        Self {
            stick: stick.map(quantise),
            ..Default::default()
        }
    }

    /// The marker input for a player the computer drives
    /// ([`button::COMPUTER`]).
    pub fn computer() -> Self {
        Self {
            buttons: button::COMPUTER,
            ..Default::default()
        }
    }

    /// True for [`Input::computer`].
    pub fn is_computer(&self) -> bool {
        self.buttons & button::COMPUTER != 0
    }

    /// This input with `buttons` held as well.
    pub fn with(mut self, buttons: u16) -> Self {
        self.buttons |= buttons & button::ALL;
        self
    }

    /// The left stick as a court direction, length up to about 1.
    pub fn stick(&self) -> [f32; 2] {
        self.stick.map(|v| f32::from(v) / 127.0)
    }

    pub fn aim(&self) -> [f32; 2] {
        self.aim.map(|v| f32::from(v) / 127.0)
    }

    pub fn set_aim(&mut self, aim: [f32; 2]) {
        self.aim = aim.map(quantise);
    }

    /// [`Input::view`] from the active camera's yaw, radians (`atan2` of
    /// its look's x and z, as the original's camera keeps it).
    pub fn set_yaw(&mut self, yaw: f32) {
        self.view = crate::timing::angle_units(yaw) as u16;
    }

    /// The left stick from the original's words: `word` the court word it
    /// moves along, `strength` 0..=90 ([`person_words`]).
    pub fn set_words(&mut self, word: i32, strength: i32) {
        self.stick = word_axes(word, strength);
    }

    /// The right stick from its words ([`Input::set_words`]).
    pub fn set_aim_words(&mut self, word: i32, strength: i32) {
        self.aim = word_axes(word, strength);
    }

    /// The left stick's strength, 0..=90 on the original's scale of 64 a
    /// full push (actor+0x90): the length of [`Input::stick`] times 64,
    /// rounded, which is exact for a stick [`Input::set_words`] filled.
    pub fn strength(&self) -> i32 {
        let [x, z] = self.stick();
        (crate::math::len(x, z) * 64.0).round() as i32
    }

    /// The stick asks the locomotion to move: a strength of 16 or more
    /// (`FUN_00163d40`; below it the stand path, no velocity).
    pub fn moves(&self) -> bool {
        self.strength() >= MOVES
    }

    pub fn held(&self, buttons: u16) -> bool {
        self.buttons & buttons != 0
    }

    /// How many of the four turbo buttons are held.
    pub fn turbos(&self) -> u32 {
        (self.buttons & button::TURBOS).count_ones()
    }

    pub fn turbo(&self) -> bool {
        self.turbos() > 0
    }

    /// Buttons held now that were not held in `before`.
    pub fn pressed_since(&self, before: &Input) -> u16 {
        self.buttons & !before.buttons
    }

    /// [`Input::to_bytes`]'s length.
    pub const BYTES: usize = 8;

    /// Eight bytes: stick x, z, aim x, z, buttons, view (little-endian; its
    /// top six bits the off-screen actors).
    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let b = self.buttons.to_le_bytes();
        let v = (self.view & 0x3ff | u16::from(self.off_screen & 0x3f) << 10).to_le_bytes();
        [
            self.stick[0] as u8,
            self.stick[1] as u8,
            self.aim[0] as u8,
            self.aim[1] as u8,
            b[0],
            b[1],
            v[0],
            v[1],
        ]
    }

    pub fn from_bytes(b: [u8; Self::BYTES]) -> Self {
        Self {
            stick: [b[0] as i8, b[1] as i8],
            aim: [b[2] as i8, b[3] as i8],
            buttons: u16::from_le_bytes([b[4], b[5]])
                & (button::ALL | button::PICK | button::PICK_STANCE | button::COMPUTER),
            view: u16::from_le_bytes([b[6], b[7]]) & 0x3ff,
            off_screen: b[7] >> 2,
        }
    }
}

/// Native ordinary locomotion gait family flags (0x00163d40 / 0x00167480).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GaitFamily {
    /// Slow / Walk (record flags bit 0x400).
    Walk = 0x400,
    /// Medium / Jog (record flags bit 0x800).
    Jog = 0x800,
    /// Fast / Run (record flags bit 0x1000).
    Run = 0x1000,
}

/// D-pad button bits in the native pad input packet (FUN_001587d8).
pub mod dpad {
    pub const UP: u16 = 1 << 0;
    pub const DOWN: u16 = 1 << 1;
    pub const LEFT: u16 = 1 << 2;
    pub const RIGHT: u16 = 1 << 3;
}

/// Native controller button edge timing state (FUN_001587d8: actor+0x78 held, +0x7c pressed, +0x80 released).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonEdges {
    pub held: u16,
    pub pressed: u16,
    pub released: u16,
}

impl ButtonEdges {
    pub fn step(&mut self, current: u16) {
        let previous = self.held;
        self.held = current;
        self.pressed = current & !previous;
        self.released = previous & !current;
    }
}

/// Compute directional magnitude from D-pad button bits (FUN_001587d8).
///
/// In native PS2 pad processing, each axis contributes +-64.
/// Diagonal deflection yields trunc(sqrt(64^2 + 64^2)) = 90 (`cvt.w.s`
/// rounds toward zero: the played captures read 90 at actor+0x90 in every
/// one of the 1,180 updates a person held two arrows, never 91).
/// Single-axis deflection yields 64. No press yields 0.
pub fn dpad_magnitude(buttons: u16) -> u32 {
    let mut dx: i32 = 0x40;
    let mut dy: i32 = 0x40;
    if buttons & dpad::LEFT != 0 {
        dx = 0;
    }
    if buttons & dpad::UP != 0 {
        dy = 0;
    }
    if buttons & dpad::RIGHT == 0 {
        dx -= 0x40;
    }
    if buttons & dpad::DOWN == 0 {
        dy -= 0x40;
    }
    ((dx * dx + dy * dy) as f32).sqrt() as u32
}

/// The strength from which the locomotion moves (`FUN_00163d40`), the pass
/// target reads the stick's way (`FUN_00146278`) and the pad's +0xa0
/// counts (`FUN_001587d8`).
pub const MOVES: i32 = 0x10;

/// The pad builder's and the pad driver's arctangent of `x / y` with
/// their quadrants (`FUN_001587d8` `0x158b00..`, `FUN_00147700`; the EE's
/// polynomial, [`crate::ordinary::native_atan`]), as a word:
/// `trunc(a × 162.97466 + 0.5)` (toward zero) masked to the turn. Its
/// half-unit quirks are the original's: the D-pad's left is 769 and its
/// up 513 (the played captures' words, less the camera's yaw word of the
/// update before).
pub fn pad_word(x: f32, y: f32) -> i32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let a = if y == 0.0 {
        if x == 0.0 {
            x
        } else if 0.0 <= x {
            FRAC_PI_2
        } else {
            -FRAC_PI_2
        }
    } else {
        let a = crate::ordinary::native_atan(x / y);
        if y < 0.0 {
            if 0.0 < x { a + PI } else { a - PI }
        } else {
            a
        }
    };
    ((a * 162.974_66 + 0.5) as i32) & 0x3ff
}

/// `FUN_00147700` (the left stick, `FUN_001479f8` the right): the raw
/// axes 0..=255 as the pad sends them (127 the middle; both 0, no
/// analog data, is strength 0), the strength `trunc((√(x² + y²) + 0.5) ×
/// 64 / 127)` of the offsets from 127, at most 64, no dead zone, and the
/// word of the offsets ([`pad_word`]). `(word, strength)`.
pub fn analog_words(raw: [u8; 2]) -> (i32, i32) {
    if raw == [0, 0] {
        return (0, 0);
    }
    let x = f32::from(raw[0]) - 127.0;
    let y = f32::from(raw[1]) - 127.0;
    let s = ((libm::sqrtf(x * x + y * y) + 0.5) * 64.0 / 127.0) as i32;
    (pad_word(x, y), s.min(0x40))
}

/// The D-pad's words (`FUN_001587d8`, `0x158a1c..0x158c40`): 64 a side
/// toward the arrows held (up −y, down +y, left −x, right +x), the
/// strength `trunc(√(x² + y²))` (64, or 90 on a diagonal) and its word.
pub fn dpad_words(buttons: u16) -> (i32, i32) {
    let mut x: i32 = 0x40;
    let mut y: i32 = 0x40;
    if buttons & dpad::LEFT != 0 {
        x = 0;
    }
    if buttons & dpad::UP != 0 {
        y = 0;
    }
    if buttons & dpad::RIGHT == 0 {
        x -= 0x40;
    }
    if buttons & dpad::DOWN == 0 {
        y -= 0x40;
    }
    let s = libm::sqrtf((x * x + y * y) as f32) as i32;
    (pad_word(x as f32, y as f32), s)
}

/// A person's stick words as the pad builder leaves them (`FUN_001587d8`,
/// `0x158c40..0x158dc0`): the larger strength of the stick's (`analog`,
/// its raw axes) and the D-pad's, the stick's word when the stick's is
/// at least the D-pad's, else the D-pad's; then the camera's yaw word
/// added (`yaw`, [`Input::view`]). `(court word, strength)`.
pub fn person_words(analog: Option<[u8; 2]>, dpad_buttons: u16, yaw: u16) -> (i32, i32) {
    let (aw, a) = analog.map_or((0, 0), analog_words);
    let (dw, d) = dpad_words(dpad_buttons);
    let (word, strength) = if a >= d { (aw, a) } else { (dw, d) };
    ((word + i32::from(yaw)) & 0x3ff, strength)
}

/// The axes a stick of `strength` along court word `word` takes: the
/// word's way (`(-sin, -cos)`, the game's table) at a length of
/// `strength × 127 / 64`, so [`Input::strength`] reads the strength back
/// (the quantised length is within 0.71 of it, under half a step of 64).
/// The word itself comes back only to within the axes' resolution.
pub fn word_axes(word: i32, strength: i32) -> [i8; 2] {
    let len = strength.clamp(0, 90) as f32 * 127.0 / 64.0;
    let x = -native_sin_1024(word) * len;
    let z = -native_cos_1024(word) * len;
    [
        x.round().clamp(-127.0, 127.0) as i8,
        z.round().clamp(-127.0, 127.0) as i8,
    ]
}

/// Combine analog stick magnitude and directional magnitude (FUN_001587d8),
/// returning `(requested_intensity, capped_intensity)`.
///
/// FUN_001587d8 writes `actor+0x90 = max(analog_mag, dpad_mag)`.
/// FUN_00163d40 caps it at 64 (`0x40`).
pub fn combine_input_intensity(analog_magnitude: u32, dpad_magnitude: u32) -> (u32, u32) {
    let requested = analog_magnitude.max(dpad_magnitude);
    let capped = requested.min(64);
    (requested, capped)
}

/// Evaluate the native gait family hysteresis thresholds (0x00167480..0x00167554).
///
/// Thresholds based on current gait flags:
/// - Flags containing 0x1000 (Run): lower = 40, upper = 59.
/// - Flags containing 0x800 (Jog): lower = 39, upper = 61.
/// - Flags containing 0x400 (Walk): lower = 41, upper = 60.
/// - Default: lower = 40, upper = 60.
///
/// Selection:
/// - intensity >= upper: Run (0x1000)
/// - intensity < lower: Walk (0x400)
/// - otherwise: Jog (0x800)
pub fn evaluate_gait_family_hysteresis(current_flags: u32, intensity: u32) -> GaitFamily {
    let (lower, upper) = if current_flags & (GaitFamily::Run as u32) != 0 {
        (40, 59)
    } else if current_flags & (GaitFamily::Jog as u32) != 0 {
        (39, 61)
    } else if current_flags & (GaitFamily::Walk as u32) != 0 {
        (41, 60)
    } else {
        (40, 60)
    };

    let capped = intensity.min(64);
    if capped >= upper {
        GaitFamily::Run
    } else if capped < lower {
        GaitFamily::Walk
    } else {
        GaitFamily::Jog
    }
}

/// Build the native gait search mask for FUN_00168b30 combining family and ball possession.
/// Bit 0 (0x001) = unheld, Bit 1 (0x002) = held.
pub fn gait_query_mask(family: GaitFamily, ball_held: bool) -> u32 {
    (family as u32) | if ball_held { 0x002 } else { 0x001 }
}

/// Compute native horizontal physical motion (FUN_00163d40 / actor+0x190).
///
/// Speed rating `s` in 0..99.
/// If gait family is Run: `f = min(speed_rating / 99.0, 1.0)`.
/// Else: `f = min(capped_intensity / 64.0, 1.0)`.
/// `base_speed = min_motion + (max_motion - min_motion) * f`.
/// If turbo button (actor+0x78 & 0x2000) is active and turbo meter > 0:
///   `speed = base_speed * (1.25 - min(speed_rating / 99.0, 1.0) * 0.1)`.
pub fn compute_physical_motion(
    family: GaitFamily,
    capped_intensity: u32,
    speed_rating: u32,
    min_motion: f32,
    max_motion: f32,
    turbo_active: bool,
) -> f32 {
    let s = (speed_rating as f32 / 99.0).min(1.0);
    let f = if family == GaitFamily::Run {
        s
    } else {
        (capped_intensity as f32 / 64.0).min(1.0)
    };
    let base = min_motion + (max_motion - min_motion) * f;
    if turbo_active {
        base * (1.25 - s * 0.1)
    } else {
        base
    }
}

/// Quarter-wave cosine table at 0x00440ea0 for 257 angles (0..=256), step = 2*PI / 1024.
/// Reflects the quarter-wave into all four quadrants matching FUN_0018ccc0.
pub fn native_cos_1024(angle: i32) -> f32 {
    let a = (angle as u32) & 0x3ff;
    if a < 0x100 {
        ((a as f64) * std::f64::consts::PI / 512.0).cos() as f32
    } else if a < 0x200 {
        -(((0x200 - a) as f64) * std::f64::consts::PI / 512.0).cos() as f32
    } else if a > 0x2ff {
        (((0x400 - a) as f64) * std::f64::consts::PI / 512.0).cos() as f32
    } else {
        -(((a - 0x200) as f64) * std::f64::consts::PI / 512.0).cos() as f32
    }
}

/// Native sine via cosine quarter-wave reflection (FUN_0018cd58: FUN_0018ccc0(angle - 0x100)).
pub fn native_sin_1024(angle: i32) -> f32 {
    native_cos_1024(angle.wrapping_sub(0x100))
}

/// Calculate native horizontal velocity step from physical speed and 10-bit integer heading.
/// Matches raw 167d88..167db4:
/// vx = -speed * Sin(heading)
/// vz = -speed * Cos(heading)
pub fn native_velocity_step(speed: f32, heading: i32) -> [f32; 2] {
    let vx = -speed * native_sin_1024(heading);
    let vz = -speed * native_cos_1024(heading);
    [vx, vz]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_eight_bytes() {
        let mut i = Input::moving([0.5, -1.0]).with(button::SHOOT | button::R2);
        i.set_aim([-0.25, 0.75]);
        // A camera looking along -x: yaw -pi/2, word 769 (the screen's down
        // along +x).
        i.set_yaw(-std::f32::consts::FRAC_PI_2);
        assert_eq!(i.view, 769);
        assert_eq!(Input::from_bytes(i.to_bytes()), i);
        // Actors 0 and 5 off the screen, in the view word's top bits.
        i.off_screen = 0b10_0001;
        assert_eq!(Input::from_bytes(i.to_bytes()), i);
        assert_eq!(i.to_bytes()[6..], [0x01, 0x87]);
        assert_eq!(i.stick, [64, -127]);
        assert_eq!(i.turbos(), 1);
        assert!(i.held(button::SHOOT) && !i.held(button::PASS));
    }

    #[test]
    fn presses_are_new_buttons_only() {
        let before = Input::default().with(button::SHOOT);
        let now = Input::default().with(button::SHOOT | button::PASS);
        assert_eq!(now.pressed_since(&before), button::PASS);
    }

    #[test]
    fn the_pad_words_are_the_pad_builders() {
        // The D-pad's eight words, with the arctangent's half-unit quirks
        // the played captures show (their words less the yaw word).
        let words: Vec<(i32, i32)> = [
            dpad::DOWN,
            dpad::DOWN | dpad::RIGHT,
            dpad::RIGHT,
            dpad::UP | dpad::RIGHT,
            dpad::UP,
            dpad::UP | dpad::LEFT,
            dpad::LEFT,
            dpad::DOWN | dpad::LEFT,
        ]
        .into_iter()
        .map(dpad_words)
        .collect();
        assert_eq!(
            words,
            vec![
                (0, 64),
                (128, 90),
                (256, 64),
                (384, 90),
                (513, 64),
                (641, 90),
                (769, 64),
                (897, 90)
            ]
        );
        assert_eq!(dpad_words(0), (0, 0));
        // The stick: no dead zone, 127 the middle, at most 64.
        assert_eq!(analog_words([127, 127]), (0, 0));
        assert_eq!(analog_words([128, 128]).1, 0);
        assert_eq!(analog_words([255, 127]), (256, 64));
        assert_eq!(analog_words([127, 0]), (513, 64));
        assert_eq!(analog_words([255, 0]).1, 64);
        assert_eq!(analog_words([127 + 20, 127]).1, 10);
        assert_eq!(analog_words([0, 0]), (0, 0));
        // The larger strength's word, then the yaw: a diagonal D-pad
        // outweighs a full stick, and the stick wins a tie.
        assert_eq!(
            person_words(Some([255, 127]), dpad::UP | dpad::LEFT, 3),
            (644, 90)
        );
        assert_eq!(person_words(Some([255, 127]), dpad::UP, 0), (256, 64));
        // Encoded on the court's axes, the strength comes back exactly.
        for s in 0..=64 {
            for w in (0..1024).step_by(37) {
                let mut i = Input::default();
                i.set_words(w, s);
                assert_eq!(i.strength(), s, "{w} {s}");
            }
        }
        let mut i = Input::default();
        i.set_words(128, 90);
        assert_eq!(i.strength(), 90);
        assert!(i.moves());
        i.set_words(0, 15);
        assert!(!i.moves());
    }

    #[test]
    fn native_dpad_magnitude_and_intensity_combination() {
        // No buttons pressed -> 0
        assert_eq!(dpad_magnitude(0), 0);
        // Single axis: UP -> 64
        assert_eq!(dpad_magnitude(dpad::UP), 64);
        // Single axis: RIGHT -> 64
        assert_eq!(dpad_magnitude(dpad::RIGHT), 64);
        // Diagonal: UP + RIGHT -> 90 (sqrt(64^2 + 64^2) = 90.50966, truncated
        // as the played captures show at actor+0x90)
        assert_eq!(dpad_magnitude(dpad::UP | dpad::RIGHT), 90);
        // Diagonal: DOWN + LEFT -> 90
        assert_eq!(dpad_magnitude(dpad::DOWN | dpad::LEFT), 90);

        // Intensity combination and capping
        let (req, capped) = combine_input_intensity(50, 90);
        assert_eq!(req, 90);
        assert_eq!(capped, 64); // capped at 64 by FUN_00163d40

        let (req2, capped2) = combine_input_intensity(35, 0);
        assert_eq!(req2, 35);
        assert_eq!(capped2, 35);
    }

    #[test]
    fn native_gait_family_hysteresis_state_transitions() {
        // 1. Starting from Run state (flags bit 0x1000): lower 40, upper 59
        let run_flags = GaitFamily::Run as u32;
        assert_eq!(
            evaluate_gait_family_hysteresis(run_flags, 64),
            GaitFamily::Run
        );
        assert_eq!(
            evaluate_gait_family_hysteresis(run_flags, 59),
            GaitFamily::Run
        );
        // Dropping below 59 drops into Jog
        assert_eq!(
            evaluate_gait_family_hysteresis(run_flags, 58),
            GaitFamily::Jog
        );
        assert_eq!(
            evaluate_gait_family_hysteresis(run_flags, 40),
            GaitFamily::Jog
        );
        // Dropping below 40 drops into Walk
        assert_eq!(
            evaluate_gait_family_hysteresis(run_flags, 39),
            GaitFamily::Walk
        );

        // 2. Starting from Jog state (flags bit 0x800): lower 39, upper 61
        let jog_flags = GaitFamily::Jog as u32;
        assert_eq!(
            evaluate_gait_family_hysteresis(jog_flags, 60),
            GaitFamily::Jog
        );
        // Exceeding 61 promotes to Run
        assert_eq!(
            evaluate_gait_family_hysteresis(jog_flags, 61),
            GaitFamily::Run
        );
        assert_eq!(
            evaluate_gait_family_hysteresis(jog_flags, 39),
            GaitFamily::Jog
        );
        // Dropping below 39 drops to Walk
        assert_eq!(
            evaluate_gait_family_hysteresis(jog_flags, 38),
            GaitFamily::Walk
        );

        // 3. Starting from Walk state (flags bit 0x400): lower 41, upper 60
        let walk_flags = GaitFamily::Walk as u32;
        assert_eq!(
            evaluate_gait_family_hysteresis(walk_flags, 40),
            GaitFamily::Walk
        );
        // Reaching 41 promotes to Jog
        assert_eq!(
            evaluate_gait_family_hysteresis(walk_flags, 41),
            GaitFamily::Jog
        );
        assert_eq!(
            evaluate_gait_family_hysteresis(walk_flags, 59),
            GaitFamily::Jog
        );
        // Reaching 60 promotes to Run
        assert_eq!(
            evaluate_gait_family_hysteresis(walk_flags, 60),
            GaitFamily::Run
        );

        // 4. Default / unclassified: lower 40, upper 60
        assert_eq!(evaluate_gait_family_hysteresis(0, 39), GaitFamily::Walk);
        assert_eq!(evaluate_gait_family_hysteresis(0, 40), GaitFamily::Jog);
        assert_eq!(evaluate_gait_family_hysteresis(0, 59), GaitFamily::Jog);
        assert_eq!(evaluate_gait_family_hysteresis(0, 60), GaitFamily::Run);
    }

    #[test]
    fn native_gait_query_mask_held_modifiers() {
        assert_eq!(gait_query_mask(GaitFamily::Walk, false), 0x401);
        assert_eq!(gait_query_mask(GaitFamily::Walk, true), 0x402);
        assert_eq!(gait_query_mask(GaitFamily::Jog, false), 0x801);
        assert_eq!(gait_query_mask(GaitFamily::Jog, true), 0x802);
        assert_eq!(gait_query_mask(GaitFamily::Run, false), 0x1001);
        assert_eq!(gait_query_mask(GaitFamily::Run, true), 0x1002);
    }

    #[test]
    fn native_button_edge_timing() {
        let mut edges = ButtonEdges::default();
        // Initial press
        edges.step(button::SHOOT);
        assert_eq!(edges.held, button::SHOOT);
        assert_eq!(edges.pressed, button::SHOOT);
        assert_eq!(edges.released, 0);

        // Hold through next tick
        edges.step(button::SHOOT);
        assert_eq!(edges.held, button::SHOOT);
        assert_eq!(edges.pressed, 0);
        assert_eq!(edges.released, 0);

        // Press additional button
        edges.step(button::SHOOT | button::PASS);
        assert_eq!(edges.held, button::SHOOT | button::PASS);
        assert_eq!(edges.pressed, button::PASS);
        assert_eq!(edges.released, 0);

        // Release first button
        edges.step(button::PASS);
        assert_eq!(edges.held, button::PASS);
        assert_eq!(edges.pressed, 0);
        assert_eq!(edges.released, button::SHOOT);
    }

    #[test]
    fn native_physical_motion_and_turbo_speed() {
        let min_m = 3.0_f32;
        let max_m = 8.0_f32;
        // Walk at half intensity (32/64): motion = 3.0 + 5.0 * 0.5 = 5.5
        let walk_speed = compute_physical_motion(GaitFamily::Walk, 32, 50, min_m, max_m, false);
        assert!((walk_speed - 5.5).abs() < 1e-5);

        // Run with speed rating 99 (s = 1.0): motion = 3.0 + 5.0 * 1.0 = 8.0
        let run_speed = compute_physical_motion(GaitFamily::Run, 64, 99, min_m, max_m, false);
        assert!((run_speed - 8.0).abs() < 1e-5);

        // Turbo multiplier for s=1.0: 1.25 - 1.0 * 0.1 = 1.15
        let turbo_speed = compute_physical_motion(GaitFamily::Run, 64, 99, min_m, max_m, true);
        assert!((turbo_speed - 8.0 * 1.15).abs() < 1e-5);

        // Turbo multiplier for s=0.0: 1.25 - 0.0 = 1.25
        let turbo_speed_s0 = compute_physical_motion(GaitFamily::Run, 64, 0, min_m, max_m, true);
        assert!((turbo_speed_s0 - 3.0 * 1.25).abs() < 1e-5);
    }

    #[test]
    fn native_1024_trig_quarter_wave_reflection() {
        // angle 0: cos = 1.0, sin = 0.0
        assert!((native_cos_1024(0) - 1.0).abs() < 1e-5);
        assert!((native_sin_1024(0) - 0.0).abs() < 1e-5);

        // angle 256 (pi/2): cos = 0.0, sin = 1.0
        assert!((native_cos_1024(256) - 0.0).abs() < 1e-5);
        assert!((native_sin_1024(256) - 1.0).abs() < 1e-5);

        // angle 512 (pi): cos = -1.0, sin = 0.0
        assert!((native_cos_1024(512) - (-1.0)).abs() < 1e-5);
        assert!((native_sin_1024(512) - 0.0).abs() < 1e-5);

        // angle 768 (3pi/2): cos = 0.0, sin = -1.0
        assert!((native_cos_1024(768) - 0.0).abs() < 1e-5);
        assert!((native_sin_1024(768) - (-1.0)).abs() < 1e-5);

        // Angle wrapping: angle 1024 wraps to 0
        assert_eq!(native_cos_1024(1024), native_cos_1024(0));
        assert_eq!(native_cos_1024(-256), native_cos_1024(768));

        // Velocity step along heading 0: vx = -speed * sin(0) = 0, vz = -speed * cos(0) = -speed
        let v0 = native_velocity_step(10.0, 0);
        assert!((v0[0] - 0.0).abs() < 1e-5);
        assert!((v0[1] - (-10.0)).abs() < 1e-5);
    }
}
