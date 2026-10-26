//! Turning gamepads and the keyboard into simulation input for the players
//! people control, and filling in everyone else's from the computer.
//!
//! Up to four people play, each on a controller (the keyboard or a pad) and a
//! side, picked on Choose Sides ([`MatchSetup::seats`](crate::setup::MatchSetup));
//! they are 1P..4P in the order they joined, coloured as the game's own
//! off-screen markers (`/FE/OFFIND.SSH`): red, yellow, blue, green. With
//! nobody seated (the story modes, the command line) one person plays the
//! home team on every controller at once.
//!
//! The simulation moves a person's controller where the original moves it
//! ([`street_sim::control`]): on a team with one person, to a pass's
//! receiver as the ball leaves, to whoever is given the ball, and on
//! defence (or a loose ball) to the teammate Cross switches to, unless a
//! give 'n go keeps the passer; each seat alone on its side drives the
//! player [`World::person_on`] names after the last step. On a team with
//! several, each keeps a player of their own and nothing switches, as the
//! original keeps a side two or three pads play.

use bevy::prelude::*;
use street_sim::{Input, World, ai, button};

use crate::{
    game::Sim,
    view::{SidelineCamera, native_angles},
};

/// A controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Device {
    Keyboard,
    Pad(Entity),
}

/// The most people at once, as on the PS2 with a multitap.
pub const MAX_SEATS: usize = 4;

/// The most people in a match online: three a side, 1P..6P.
pub const MAX_PEOPLE: usize = street_net::MAX_SEATS;

/// 1P..6P's colours: the off-screen markers' red, yellow, blue and green,
/// then the rewrite's purple and orange for 5P and 6P online (the art's
/// [`crate::art::PEOPLE_SHIFTS`]).
pub const SEAT_COLOURS: [[f32; 3]; MAX_PEOPLE] = [
    [0.86, 0.14, 0.12],
    [0.93, 0.76, 0.12],
    [0.22, 0.28, 0.90],
    [0.55, 0.80, 0.14],
    [0.58, 0.20, 0.88],
    [0.94, 0.50, 0.10],
];

/// Someone playing from another machine online.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remote {
    /// Their seat in the match, 0..5 for 1P..6P.
    pub number: usize,
    pub team: usize,
    /// The player they drive now.
    pub player: usize,
}

/// One person at the controls.
#[derive(Clone, Debug)]
pub struct Seat {
    /// 0..3 for 1P..4P; online, the seat in the match (0..5 for 1P..6P).
    pub number: usize,
    /// The controller, or `None` for every controller at once.
    pub device: Option<Device>,
    pub team: usize,
    /// The player driven now.
    pub player: usize,
    /// The pad as last read, in court axes, the pass button included (the
    /// simulation turns a pass press on defence into the switch).
    pub pad: Input,
    /// The same, as online play sends it.
    pub raw: Input,
    /// Native controller button edge timing state (FUN_001587d8).
    pub edges: street_sim::input::ButtonEdges,
}

/// Who is at the controls and what they drive.
#[derive(Resource)]
pub struct Control {
    pub seats: Vec<Seat>,
    /// The computer plays every player.
    pub demo: bool,
    /// Online, the people at the other machines, as the match shown seats
    /// them.
    pub remote: Vec<Remote>,
}

impl Control {
    /// The seats for a match: one per (controller, side) chosen, each on a
    /// player of its own; or, with none chosen, one person on the home team
    /// on every controller.
    pub fn new(chosen: &[(Device, usize)], demo: bool, world: &World) -> Self {
        let mut seats: Vec<Seat> = Vec::new();
        let picks: Vec<(Option<Device>, usize)> = if chosen.is_empty() {
            vec![(None, 0)]
        } else {
            chosen
                .iter()
                .take(MAX_SEATS)
                .map(|&(d, side)| (Some(d), side))
                .collect()
        };
        for (number, (device, team)) in picks.into_iter().enumerate() {
            let before = seats.iter().filter(|s| s.team == team).count();
            let Some(player) = (0..world.players.len())
                .filter(|&j| world.team_of(j) == team)
                .nth(before)
            else {
                continue; // more people than players on that side
            };
            seats.push(Seat {
                number,
                device,
                team,
                player,
                pad: Input::default(),
                raw: Input::default(),
                edges: street_sim::input::ButtonEdges::default(),
            });
        }
        Self {
            seats,
            demo,
            remote: Vec::new(),
        }
    }

    /// Everyone playing, here and online elsewhere, by number: (number,
    /// team, player).
    pub fn people(&self) -> Vec<(usize, usize, usize)> {
        if self.demo {
            return Vec::new();
        }
        let mut all: Vec<(usize, usize, usize)> = self
            .seats
            .iter()
            .map(|s| (s.number, s.team, s.player))
            .chain(self.remote.iter().map(|r| (r.number, r.team, r.player)))
            .collect();
        all.sort_unstable();
        all
    }

    /// The first person's player (the story modes and lessons have one).
    pub fn first_player(&self) -> Option<usize> {
        self.seats.first().map(|s| s.player)
    }

    /// The person driving `player`, if any.
    pub fn seat_of(&self, player: usize) -> Option<&Seat> {
        if self.demo {
            return None;
        }
        self.seats.iter().find(|s| s.player == player)
    }

    /// Whether more than one person plays: the HUD numbers them then.
    pub fn several(&self) -> bool {
        self.seats.len() + self.remote.len() > 1
    }
}

/// Keyboard keys for each pad button. The four turbo buttons are separate,
/// as on the PS2 pad, because moves care how many are held.
const KEYS: [(u16, &[KeyCode]); 11] = [
    (button::SHOOT, &[KeyCode::KeyJ, KeyCode::Space]),
    (button::PASS, &[KeyCode::KeyK]),
    (button::TRICK, &[KeyCode::KeyL]),
    (button::JUMP, &[KeyCode::KeyI]),
    (button::R1, &[KeyCode::ShiftLeft, KeyCode::ShiftRight]),
    (button::L1, &[KeyCode::KeyQ]),
    (button::R2, &[KeyCode::KeyE]),
    (button::L2, &[KeyCode::KeyR]),
    (button::L3, &[KeyCode::KeyU]),
    (button::R3, &[KeyCode::KeyO]),
    (button::SELECT, &[KeyCode::KeyP]),
];

/// Gamepad buttons for each pad button, laid out like the PS2 pad.
const PAD: [(u16, GamepadButton); 11] = [
    (button::SHOOT, GamepadButton::East),
    (button::PASS, GamepadButton::South),
    (button::TRICK, GamepadButton::West),
    (button::JUMP, GamepadButton::North),
    (button::L1, GamepadButton::LeftTrigger),
    (button::R1, GamepadButton::RightTrigger),
    (button::L2, GamepadButton::LeftTrigger2),
    (button::R2, GamepadButton::RightTrigger2),
    (button::L3, GamepadButton::LeftThumb),
    (button::R3, GamepadButton::RightThumb),
    (button::SELECT, GamepadButton::Select),
];

/// One controller's state, or every controller's merged.
#[derive(Default)]
struct Sample {
    /// The D-pad's arrows ([`street_sim::dpad`]): the keyboard's arrows
    /// and WASD, a pad's D-pad.
    dpad: u16,
    /// The left stick's raw axes as the PS2 pad sends them (0..=255, 127
    /// the middle, up 0), the farthest pushed of the pads.
    analog: Option<[u8; 2]>,
    /// The right stick's, likewise.
    aim: Option<[u8; 2]>,
    held: u16,
    /// The buttons pressed this frame (as layout A names them).
    just: u16,
}

/// A pad's stick as the PS2 pad's raw axes (x right, y down, 127 the
/// middle).
fn raw_axes(v: Vec2) -> [u8; 2] {
    let axis = |a: f32| (127.0 + a * 128.0).round().clamp(0.0, 255.0) as u8;
    [axis(v.x), axis(-v.y)]
}

/// The pad's D-pad buttons and their arrows.
const ARROWS: [(GamepadButton, u16); 4] = [
    (GamepadButton::DPadUp, street_sim::dpad::UP),
    (GamepadButton::DPadDown, street_sim::dpad::DOWN),
    (GamepadButton::DPadLeft, street_sim::dpad::LEFT),
    (GamepadButton::DPadRight, street_sim::dpad::RIGHT),
];

fn sample(
    keys: &ButtonInput<KeyCode>,
    gamepads: &Query<(Entity, &Gamepad)>,
    device: Option<Device>,
) -> Sample {
    let mut s = Sample::default();
    if matches!(device, None | Some(Device::Keyboard)) {
        // The keys are the D-pad, as the project's PCSX2 binds them.
        for (codes, arrow) in [
            ([KeyCode::KeyW, KeyCode::ArrowUp], street_sim::dpad::UP),
            ([KeyCode::KeyS, KeyCode::ArrowDown], street_sim::dpad::DOWN),
            (
                [KeyCode::KeyD, KeyCode::ArrowRight],
                street_sim::dpad::RIGHT,
            ),
            ([KeyCode::KeyA, KeyCode::ArrowLeft], street_sim::dpad::LEFT),
        ] {
            if keys.any_pressed(codes) {
                s.dpad |= arrow;
            }
        }
        for (b, codes) in KEYS {
            if keys.any_pressed(codes.iter().copied()) {
                s.held |= b;
            }
            if keys.any_just_pressed(codes.iter().copied()) {
                s.just |= b;
            }
        }
    }
    for (entity, gamepad) in gamepads {
        if device.is_some_and(|d| d != Device::Pad(entity)) {
            continue;
        }
        let farther = |now: Option<[u8; 2]>, v: Vec2| {
            let raw = raw_axes(v);
            let strength =
                |r: Option<[u8; 2]>| r.map_or(-1, |r| street_sim::input::analog_words(r).1);
            if strength(Some(raw)) > strength(now) {
                Some(raw)
            } else {
                now
            }
        };
        s.analog = farther(s.analog, gamepad.left_stick());
        s.aim = farther(s.aim, gamepad.right_stick());
        for (pad_button, arrow) in ARROWS {
            if gamepad.pressed(pad_button) {
                s.dpad |= arrow;
            }
        }
        for (b, pad_button) in PAD {
            if gamepad.pressed(pad_button) {
                s.held |= b;
            }
            if gamepad.just_pressed(pad_button) {
                s.just |= b;
            }
        }
    }
    s
}

/// `FUN_00171920` for each actor: off the screen when his point, his
/// feet less 2 (`(x, +0x260 - 2, z)`), projects (`project`, the shown
/// camera's) outside the screen of `size` or not at all (behind the near
/// plane, `FUN_002ffe70`); bit k for actor k ([`Input::off_screen`]).
pub fn off_screen(world: &World, project: impl Fn(Vec3) -> Option<Vec2>, size: Vec2) -> u8 {
    let mut mask = 0;
    for (k, p) in world.players.iter().enumerate().take(6) {
        let at = Vec3::new(p.position[0], p.height - 2.0, p.position[1]);
        let on = project(at)
            .is_some_and(|s| (0.0..=size.x).contains(&s.x) && (0.0..=size.y).contains(&s.y));
        if !on {
            mask |= 1 << k;
        }
    }
    mask
}

/// Sample each person's controller. The sticks and the D-pad become the
/// original's words as its pad builder makes them, the camera's yaw word
/// added (`street_sim::input::person_words`).
///
/// PS2 layout (Xbox in brackets): Circle (B) shoot, Cross (A) pass or switch,
/// Square (X) trick move or steal, Triangle (Y) block or rebound, L1 R1 L2
/// R2 (LB RB LT RT) the four turbos, L3 kick pass, R3 (RS) strategy, Select
/// (Back) pocket or cancel a gamebreaker, right stick give 'n go.
/// Keyboard: WASD or arrows, J or Space shoot, K pass or switch, L trick or
/// steal, I block or rebound, Shift Q E R the turbos, U kick pass, O
/// strategy, P pocket or cancel.
pub fn read(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    cameras: Query<(&Transform, &Camera, &GlobalTransform), With<SidelineCamera>>,
    sim: Res<Sim>,
    mut control: ResMut<Control>,
    options: Option<Res<crate::options::Options>>,
) {
    let layouts = options.map(|o| o.0.layouts).unwrap_or_default();
    // The active camera's yaw (+0x94): `atan2` of its look's x and z; and
    // the actors off its screen, as the frame shown left it.
    let (yaw, hidden) = match cameras.single() {
        Ok((cam, camera, eye)) => (
            native_angles(cam.forward().to_array()).1,
            camera.logical_viewport_size().map_or(0, |size| {
                // The screen the original shows is 16:9 at most: an
                // ultrawide window's wider court leaves its sides out.
                let width = size.x.min(size.y * 16.0 / 9.0);
                let left = (size.x - width) / 2.0;
                off_screen(
                    &sim.world,
                    |at| {
                        camera
                            .world_to_viewport(eye, at)
                            .ok()
                            .map(|s| Vec2::new(s.x - left, s.y))
                    },
                    Vec2::new(width, size.y),
                )
            }),
        ),
        Err(_) => (0.0, 0),
    };
    let holder_team = sim.world.handler().map(|h| sim.world.team_of(h));
    for seat in &mut control.seats {
        let s = sample(&keys, &gamepads, seat.device);
        // The player's controller layout (Options → Controller Layout).
        let layout = layouts[seat.number.min(MAX_SEATS - 1)];
        let offence = holder_team == Some(seat.team);
        let held = crate::options::remap(layout, s.held, offence);
        let mut pad = Input::default().with(held);
        // The camera's yaw word, which the pad builder adds to the pad's
        // own and the pass target to the facing at rest (`Input::view`).
        pad.set_yaw(yaw);
        // Who a switch would find off the screen (`Input::off_screen`).
        pad.off_screen = hidden;
        let (word, strength) = street_sim::input::person_words(s.analog, s.dpad, pad.view);
        pad.set_words(word, strength);
        if let Some(raw) = s.aim {
            let (word, strength) = street_sim::input::analog_words(raw);
            pad.set_aim_words(word + i32::from(pad.view), strength);
        }
        seat.raw = pad;
        seat.pad = pad;
        seat.edges.step(pad.buttons);
    }
}

/// Before each tick: settle who each person controls, then give every other
/// player the computer's inputs. A seat alone on its side drives whom the
/// last step left its pad on ([`World::person_on`]).
pub fn fill_inputs(mut control: ResMut<Control>, mut sim: ResMut<Sim>) {
    let w = &sim.world;
    for k in 0..control.seats.len() {
        let team = control.seats[k].team;
        let alone = control.seats.iter().filter(|s| s.team == team).count() == 1;
        if let (true, Some(p)) = (alone, w.person_on(team)) {
            control.seats[k].player = p;
        }
    }
    let inputs: Vec<Input> = (0..w.players.len())
        .map(|i| match control.seat_of(i) {
            Some(seat) => seat.pad,
            None => ai::input(w, i),
        })
        .collect();
    sim.inputs = inputs;
}

#[cfg(test)]
mod tests {
    use street_sim::{Hoop, Player, Rules};

    use super::*;

    fn world() -> World {
        let players = (0..6)
            .map(|k| Player::new(k / 3, [0.0, k as f32], 0.0))
            .collect();
        let hoop = |x: f32| Hoop {
            rim: [x, 11.75, 0.0],
            rim_radius: 0.75,
        };
        World::new([hoop(37.0), hoop(-37.0)], players, Rules::default(), 1)
    }

    /// `FUN_00171920`'s screen test: the point under each actor's feet,
    /// inside the screen or not.
    #[test]
    fn the_actors_off_the_screen_are_the_switchs_twenty() {
        let mut w = world();
        // A camera showing x in -20..20 across 640 pixels and z in -10..10
        // down 480, nothing behind it at z > 30.
        let project =
            |at: Vec3| (at.z < 30.0).then(|| Vec2::new((at.x + 20.0) * 16.0, (at.z + 10.0) * 24.0));
        let size = Vec2::new(640.0, 480.0);
        for (k, p) in w.players.iter_mut().enumerate() {
            p.position = [0.0, 0.0];
            p.height = 0.0;
            if k == 2 {
                p.position = [25.0, 0.0];
            }
            if k == 4 {
                p.position = [0.0, 40.0];
            }
        }
        assert_eq!(off_screen(&w, project, size), 1 << 2 | 1 << 4);
    }

    /// Nobody chosen: one person on the home team, on every controller.
    #[test]
    fn one_person_by_default() {
        let c = Control::new(&[], false, &world());
        assert_eq!(c.seats.len(), 1);
        assert_eq!(
            (c.seats[0].team, c.seats[0].player, c.seats[0].device),
            (0, 0, None)
        );
    }

    /// People on the same side get players of their own, numbered in the
    /// order they joined.
    #[test]
    fn seats_take_their_own_players() {
        let pad = |n: u32| Device::Pad(Entity::from_raw_u32(n).unwrap());
        let chosen = [(Device::Keyboard, 1), (pad(7), 0), (pad(8), 1), (pad(9), 1)];
        let c = Control::new(&chosen, false, &world());
        let got: Vec<(usize, usize, usize)> = c
            .seats
            .iter()
            .map(|s| (s.number, s.team, s.player))
            .collect();
        assert_eq!(got, vec![(0, 1, 3), (1, 0, 0), (2, 1, 4), (3, 1, 5)]);
        assert!(c.several());
        assert_eq!(c.seat_of(4).map(|s| s.number), Some(2));
        assert!(Control::new(&chosen, true, &world()).seat_of(4).is_none());
    }
}
