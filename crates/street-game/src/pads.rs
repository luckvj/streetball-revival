//! Controllers, plugged in and out at any time: Xbox pads, PlayStation pads
//! (DualShock 4, DualSense) and anything else SDL's controller database
//! knows, through Bevy's gilrs backend (Windows.Gaming.Input on Windows,
//! evdev on Linux, IOKit on macOS), all mapped to one layout: the PS2's
//! buttons, Cross/A south, Circle/B east, Square/X west, Triangle/Y north.
//!
//! This module adds what the menus and a match need on top:
//!
//! - [`PadNav`]: menu directions from each pad's left stick and D-pad, held
//!   ones repeating, so a pad moves through every list.
//! - [`ButtonStyle`]: the buttons of whatever was touched last, for the
//!   prompts (PlayStation names for a Sony pad, Xbox names for any other,
//!   keys for the keyboard).
//! - Plug and play in a match ([`rebind`]): a seat whose pad was unplugged
//!   takes any free pad that is plugged in; with none, an offline match
//!   pauses on the game's own notice (`control_chk_ps2.txt`) until one is.
//! - Vibration ([`PadSettings`], the game's "Vibration" setting in
//!   `contconfig.txt`): the pads of the people in a play rumble on dunks,
//!   blocks, steals, fakeouts and gamebreakers.

use std::{collections::HashMap, time::Duration};

use bevy::{
    input::gamepad::{
        GamepadConnection, GamepadConnectionEvent, GamepadRumbleIntensity, GamepadRumbleRequest,
    },
    prelude::*,
};
use street_sim::{Basket, Event};

use crate::{
    Screen,
    controls::{Control, Device},
    game::{Paused, TickEvents},
};

/// Sony's USB vendor id: its pads get PlayStation names.
const SONY: u16 = 0x054c;

/// Whose buttons the prompts name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    #[default]
    Keyboard,
    Xbox,
    PlayStation,
}

impl ButtonStyle {
    pub fn of(pad: &Gamepad) -> Self {
        if pad.vendor_id() == Some(SONY) {
            ButtonStyle::PlayStation
        } else {
            ButtonStyle::Xbox
        }
    }

    /// The confirm and back buttons' names.
    pub fn confirm(self) -> &'static str {
        match self {
            ButtonStyle::Keyboard => "Enter",
            ButtonStyle::Xbox => "A",
            ButtonStyle::PlayStation => "Cross",
        }
    }

    pub fn back(self) -> &'static str {
        match self {
            ButtonStyle::Keyboard => "Esc",
            ButtonStyle::Xbox => "B",
            ButtonStyle::PlayStation => "Circle",
        }
    }

    /// The menus' key line: choose, change (when `change`), select, back.
    pub fn menu_keys(self, change: bool) -> String {
        let change = if change { "   Left/Right change" } else { "" };
        format!(
            "Up/Down choose{change}   {} select   {} back",
            self.confirm(),
            self.back()
        )
    }
}

/// The controls, by action: PlayStation, Xbox and keyboard names.
pub const BINDINGS: [(&str, &str, &str, &str); 11] = [
    ("Move", "Left stick", "Left stick", "WASD or arrows"),
    ("Turbo (four)", "L1 R1 L2 R2", "LB RB LT RT", "Shift Q E R"),
    ("Shoot", "Circle", "B", "J or Space"),
    ("Pass, switch", "Cross", "A", "K"),
    ("Steal", "Square", "X", "L"),
    ("Block, rebound", "Triangle", "Y", "I"),
    ("Trick move", "Square with the ball", "X with the ball", "L"),
    (
        "Gamebreaker",
        "Two turbos + Circle",
        "Two turbos + B",
        "Two turbos + J",
    ),
    ("Pocket, cancel", "Select", "View", "P"),
    ("Pick or alley-oop", "R3", "Right stick click", "O"),
    ("Pause", "Start", "Menu", "Esc"),
];

impl ButtonStyle {
    /// The Controls page's rows for this style.
    pub fn controls(self) -> Vec<String> {
        BINDINGS
            .iter()
            .map(|&(action, ps, xbox, keys)| {
                let how = match self {
                    ButtonStyle::PlayStation => ps,
                    ButtonStyle::Xbox => xbox,
                    ButtonStyle::Keyboard => keys,
                };
                format!("{action}  {how}")
            })
            .collect()
    }
}

/// Menu directions this frame, per pad and from any, with held ones
/// repeating; and who touched the controls last.
#[derive(Resource, Default)]
pub struct PadNav {
    fired: HashMap<Entity, IVec2>,
    any: IVec2,
    held: HashMap<Entity, (IVec2, f32)>,
    pub style: ButtonStyle,
}

impl PadNav {
    /// A direction from any pad this frame (y up).
    pub fn any(&self) -> IVec2 {
        self.any
    }

    /// One pad's direction this frame.
    pub fn of(&self, pad: Entity) -> IVec2 {
        self.fired.get(&pad).copied().unwrap_or_default()
    }
}

/// The first repeat of a held direction, then the rest (seconds).
const REPEAT_AFTER: f32 = 0.38;
const REPEAT_EVERY: f32 = 0.11;
/// The stick counts past this, and lets go below the lower figure.
const STICK_ON: f32 = 0.55;
const STICK_OFF: f32 = 0.35;

/// A pad's held direction: the D-pad, else the left stick (its bigger
/// axis), with some give so a stick near the edge doesn't flicker.
fn held_direction(pad: &Gamepad, before: IVec2) -> IVec2 {
    let dpad = IVec2::new(
        i32::from(pad.pressed(GamepadButton::DPadRight))
            - i32::from(pad.pressed(GamepadButton::DPadLeft)),
        i32::from(pad.pressed(GamepadButton::DPadUp))
            - i32::from(pad.pressed(GamepadButton::DPadDown)),
    );
    if dpad != IVec2::ZERO {
        return if dpad.x != 0 && dpad.y != 0 {
            IVec2::new(0, dpad.y)
        } else {
            dpad
        };
    }
    let s = pad.left_stick();
    let threshold = if before == IVec2::ZERO {
        STICK_ON
    } else {
        STICK_OFF
    };
    if s.x.abs().max(s.y.abs()) < threshold {
        return IVec2::ZERO;
    }
    if s.y.abs() >= s.x.abs() {
        IVec2::new(0, s.y.signum() as i32)
    } else {
        IVec2::new(s.x.signum() as i32, 0)
    }
}

fn nav(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut nav: ResMut<PadNav>,
) {
    let dt = time.delta_secs();
    let nav = &mut *nav;
    nav.fired.clear();
    nav.any = IVec2::ZERO;
    nav.held.retain(|e, _| gamepads.contains(*e));
    if keys.get_just_pressed().next().is_some() {
        nav.style = ButtonStyle::Keyboard;
    }
    for (e, pad) in &gamepads {
        let (before, timer) = nav.held.get(&e).copied().unwrap_or_default();
        let dir = held_direction(pad, before);
        let (fire, timer) = if dir == IVec2::ZERO {
            (false, 0.0)
        } else if dir != before {
            (true, REPEAT_AFTER)
        } else if timer - dt <= 0.0 {
            (true, REPEAT_EVERY)
        } else {
            (false, timer - dt)
        };
        nav.held.insert(e, (dir, timer));
        if fire {
            nav.fired.insert(e, dir);
            if nav.any == IVec2::ZERO {
                nav.any = dir;
            }
        }
        if fire || pad.get_just_pressed().next().is_some() {
            nav.style = ButtonStyle::of(pad);
        }
    }
}

/// Says which controllers come and go, and whose buttons they get.
fn announce(mut events: MessageReader<GamepadConnectionEvent>) {
    for e in events.read() {
        match &e.connection {
            GamepadConnection::Connected {
                name, vendor_id, ..
            } => {
                let style = if *vendor_id == Some(SONY) {
                    ButtonStyle::PlayStation
                } else {
                    ButtonStyle::Xbox
                };
                info!("controller connected: {name} ({style:?} buttons)");
            }
            GamepadConnection::Disconnected => info!("controller disconnected"),
        }
    }
}

/// Settings for the pads (Options).
#[derive(Resource)]
pub struct PadSettings {
    pub vibration: bool,
}

impl Default for PadSettings {
    fn default() -> Self {
        Self {
            vibration: street_sim::records::Settings::default().vibration,
        }
    }
}

/// [`PadNav`]'s update, which reads the pads after the input's own systems:
/// anything that adds presses for the menus runs before it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NavSystems;

pub struct PadsPlugin;

impl Plugin for PadsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PadNav>()
            .init_resource::<PadSettings>()
            .add_systems(
                PreUpdate,
                nav.in_set(NavSystems).after(bevy::input::InputSystems),
            )
            .add_systems(Update, announce)
            .add_systems(
                Update,
                rebind
                    .before(crate::controls::read)
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(
                FixedUpdate,
                rumble
                    .after(crate::game::SimStep)
                    .run_if(in_state(Screen::Playing)),
            );
    }
}

/// The notice while a seat has no pad (`control_chk_ps2.txt` 0, without
/// the PS2's port numbers).
pub const REMOVED: &str =
    "A controller has been removed!\nPlease plug it back in,\nor plug in another.";

/// Plug and play: seats whose pad went away take free pads; with none
/// free, an offline match pauses until one is plugged in.
fn rebind(
    control: Option<ResMut<Control>>,
    gamepads: Query<Entity, With<Gamepad>>,
    mut paused: ResMut<Paused>,
    online: Option<Res<crate::online::Online>>,
) {
    let Some(mut control) = control else { return };
    let connected: Vec<Entity> = gamepads.iter().collect();
    let in_use: Vec<Entity> = control
        .seats
        .iter()
        .filter_map(|s| match s.device {
            Some(Device::Pad(e)) if connected.contains(&e) => Some(e),
            _ => None,
        })
        .collect();
    let mut free = connected.iter().filter(|e| !in_use.contains(e)).copied();
    let mut lost = false;
    for seat in &mut control.seats {
        if let Some(Device::Pad(e)) = seat.device
            && !connected.contains(&e)
        {
            match free.next() {
                Some(pad) => {
                    info!(
                        "controller: {}P moves to a newly plugged-in pad",
                        seat.number + 1
                    );
                    seat.device = Some(Device::Pad(pad));
                }
                None => lost = true,
            }
        }
    }
    if lost && online.is_none() {
        paused.hold(REMOVED);
    } else {
        paused.release(REMOVED);
    }
}

/// How hard and how long a play shakes the pads of the people in it.
fn shake(event: &Event) -> Vec<(usize, f32, f32)> {
    match *event {
        Event::Score {
            player,
            basket: Basket::Gamebreaker { .. },
            ..
        } => vec![(player, 1.0, 0.6)],
        Event::Score {
            player,
            basket: Basket::Dunk { .. } | Basket::AlleyOop { .. },
            ..
        } => vec![(player, 0.7, 0.25)],
        Event::Block { player, shooter } | Event::Goaltend { player, shooter } => {
            vec![(player, 0.5, 0.2), (shooter, 0.6, 0.25)]
        }
        Event::Steal { player, from, .. } => vec![(player, 0.3, 0.12), (from, 0.5, 0.2)],
        Event::Trick {
            fakeout: Some(victim),
            ..
        } => vec![(victim, 0.6, 0.3)],
        Event::Gamebreaker { player, .. } => vec![(player, 0.8, 0.4)],
        _ => Vec::new(),
    }
}

fn rumble(
    tick: Res<TickEvents>,
    settings: Res<PadSettings>,
    control: Option<Res<Control>>,
    gamepads: Query<Entity, With<Gamepad>>,
    mut requests: MessageWriter<GamepadRumbleRequest>,
) {
    if !settings.vibration || tick.replay {
        return;
    }
    let Some(control) = control else { return };
    for event in &tick.events {
        for (player, strength, secs) in shake(event) {
            let Some(seat) = control.seat_of(player) else {
                continue;
            };
            let pads: Vec<Entity> = match seat.device {
                Some(Device::Pad(e)) => vec![e],
                Some(Device::Keyboard) => Vec::new(),
                // One person on every controller.
                None => gamepads.iter().collect(),
            };
            for gamepad in pads {
                requests.write(GamepadRumbleRequest::Add {
                    gamepad,
                    duration: Duration::from_secs_f32(secs),
                    intensity: GamepadRumbleIntensity {
                        strong_motor: strength,
                        weak_motor: strength * 0.6,
                    },
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_follow_the_pad() {
        assert_eq!(
            ButtonStyle::PlayStation.menu_keys(false),
            "Up/Down choose   Cross select   Circle back"
        );
        assert!(ButtonStyle::Xbox.controls()[2].ends_with("  B"));
        assert!(ButtonStyle::Keyboard.controls()[0].ends_with("WASD or arrows"));
    }

    #[test]
    fn big_plays_shake_both_players() {
        let block = Event::Block {
            player: 1,
            shooter: 4,
        };
        let who: Vec<usize> = shake(&block).iter().map(|s| s.0).collect();
        assert_eq!(who, vec![1, 4]);
        assert!(shake(&Event::RimOut).is_empty());
    }
}
