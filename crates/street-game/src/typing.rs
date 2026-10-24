//! Typing names: a baller's first and last name, a team's city and name,
//! and online, the server's address and the player's name. Confirm on such
//! a row starts typing. On the keyboard letters, digits, spaces and
//! punctuation go in, Backspace takes one out, and Enter or Esc finishes.
//! With a pad, an on-screen keyboard ([`KEYBOARD`]) opens: the stick or
//! D-pad picks a key, Cross/A types it, Square/X deletes, Start or
//! Circle/B finishes. (The name rows also cycle the game's own names with
//! left and right.)

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};

use crate::{ballers, teams};

/// The longest name.
pub const MAX_LEN: usize = 16;

/// What is being typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    BallerFirst,
    BallerLast,
    TeamCity,
    TeamName,
    /// The server to join (`host[:port]`).
    Server,
    /// The name shown in an online lobby.
    PlayerName,
    /// A new user name ("Enter Name").
    UserName,
    /// The crew's team's city and name (Create Team's keyboards,
    /// `FUN_0024d8a8`).
    CrewCity,
    CrewName,
    /// Create Baller's name panel (`FUN_0027c160`): the first name, then
    /// the last, on its keyboard with RANDOM; Cancel steps back a panel.
    CabFirst,
    CabLast,
}

impl Field {
    /// Typed on User Setup's keyboard (`FUN_0023c7b8`), which the game's
    /// look draws where the field is.
    fn on_name_keyboard(self) -> bool {
        matches!(
            self,
            Field::UserName | Field::CrewCity | Field::CrewName | Field::CabFirst | Field::CabLast
        )
    }

    /// Create Baller's name panel's fields.
    pub fn on_create_baller(self) -> bool {
        matches!(self, Field::CabFirst | Field::CabLast)
    }
}

#[derive(Resource, Default)]
pub struct Typing {
    pub field: Option<Field>,
    /// Typing ended this frame: the menus ignore its Enter.
    pub ended: bool,
    /// The on-screen keyboard's key under the cursor: (row, column).
    pub key: (usize, usize),
    /// RANDOM's presses so far.
    pub rolls: u32,
}

/// The on-screen keyboard: rows of keys; `SPACE`, `DEL` and `DONE` are
/// whole keys.
pub const KEYBOARD: [&[&str]; 5] = [
    &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"],
    &["K", "L", "M", "N", "O", "P", "Q", "R", "S", "T"],
    &["U", "V", "W", "X", "Y", "Z", ".", "-", "'", ":"],
    &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
    &["SPACE", "DEL", "DONE"],
];

/// User Setup's own keyboard (`FUN_0023c7b8`, the keys 0x41..0x5a, 0x5b
/// SPC, 0x5c DEL, 0x5d DONE): six letters a row, SPC and DEL after Y and
/// Z, DONE below; no digits. A new user name is typed on it.
pub const NAME_KEYBOARD: [&[&str]; 6] = [
    &["A", "B", "C", "D", "E", "F"],
    &["G", "H", "I", "J", "K", "L"],
    &["M", "N", "O", "P", "Q", "R"],
    &["S", "T", "U", "V", "W", "X"],
    &["Y", "Z", "SPC", "DEL"],
    &["DONE"],
];

/// Create Baller's keyboard: User Setup's with RANDOM under DONE
/// (`entername.txt` 8), which picks one of the game's own names
/// (`random_first.txt`, `random_first_female.txt`, `random_last.txt`).
pub const CAB_KEYBOARD: [&[&str]; 7] = [
    NAME_KEYBOARD[0],
    NAME_KEYBOARD[1],
    NAME_KEYBOARD[2],
    NAME_KEYBOARD[3],
    NAME_KEYBOARD[4],
    NAME_KEYBOARD[5],
    &["RANDOM"],
];

/// The on-screen keyboard a field is typed on.
pub fn board(field: Field) -> &'static [&'static [&'static str]] {
    if field.on_create_baller() {
        &CAB_KEYBOARD
    } else if field.on_name_keyboard() {
        &NAME_KEYBOARD
    } else {
        &KEYBOARD
    }
}

/// The keys line while a pad types.
pub fn pad_keys(style: crate::pads::ButtonStyle) -> String {
    use crate::pads::ButtonStyle::*;
    let (press, delete, done) = match style {
        PlayStation => ("Cross", "Square", "Start"),
        _ => ("A", "X", "Menu"),
    };
    format!("Pick a key   {press} type   {delete} delete   {done} done")
}

/// Moves the keyboard's cursor by `step`, wrapping, and keeps it on a key
/// of a shorter row.
fn move_key(board: &[&[&str]], key: (usize, usize), step: IVec2) -> (usize, usize) {
    let rows = board.len() as i32;
    let key = (key.0.min(board.len() - 1), key.1);
    let key = (key.0, key.1.min(board[key.0].len() - 1));
    let row = (key.0 as i32 - step.y).rem_euclid(rows) as usize;
    let len = board[row].len() as i32;
    // Into a shorter row: the key under the same share of the width.
    let col = if row != key.0 {
        let from = board[key.0].len() as f32;
        ((key.1 as f32 + 0.5) / from * len as f32) as i32
    } else {
        key.1 as i32
    };
    let col = (col + step.x).rem_euclid(len) as usize;
    (row, col)
}

/// The keyboard drawn as text, the key under the cursor in brackets.
pub fn keyboard_text(board: &[&[&str]], key: (usize, usize)) -> String {
    board
        .iter()
        .enumerate()
        .map(|(r, row)| {
            row.iter()
                .enumerate()
                .map(|(c, k)| {
                    if (r, c) == key {
                        format!("[{k}]")
                    } else {
                        format!(" {k} ")
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The on-screen keyboard's panel, shown while a pad types.
#[derive(Component)]
pub struct KeyboardPanel;

impl Typing {
    /// Whether the menus should leave the keys alone this frame.
    pub fn busy(&self) -> bool {
        self.field.is_some() || self.ended
    }
}

/// Adds one typed character: the database spells ballers' names in
/// capitals.
fn push_up_to(text: &mut String, c: char, capitals: bool, max: usize) {
    if c.is_control() || text.chars().count() >= max {
        return;
    }
    text.push(if capitals { c.to_ascii_uppercase() } else { c });
}

pub fn type_text(
    mut typing: ResMut<Typing>,
    mut keys: MessageReader<KeyboardInput>,
    gamepads: Query<&Gamepad>,
    mut baller: ResMut<ballers::Editing>,
    mut team: ResMut<teams::TeamEditing>,
    mut menus: ResMut<crate::menu::Menus>,
    nav: Res<crate::pads::PadNav>,
    names: Option<Res<ballers::Names>>,
) {
    typing.ended = false;
    let Some(field) = typing.field else {
        keys.clear();
        return;
    };
    let female = baller.baller.as_ref().is_some_and(|b| b.female);
    let text = match field {
        Field::BallerFirst | Field::CabFirst => baller.baller.as_mut().map(|b| &mut b.first_name),
        Field::BallerLast | Field::CabLast => baller.baller.as_mut().map(|b| &mut b.last_name),
        Field::TeamCity => team.team.as_mut().map(|t| &mut t.city),
        Field::TeamName => team.team.as_mut().map(|t| &mut t.name),
        Field::Server => Some(&mut menus.online.server),
        Field::PlayerName => Some(&mut menus.online.name),
        Field::UserName => Some(&mut menus.new_name),
        Field::CrewCity => Some(&mut menus.crew_city),
        Field::CrewName => Some(&mut menus.crew_name),
    };
    let Some(text) = text else {
        typing.field = None;
        return;
    };
    let capitals = matches!(
        field,
        Field::BallerFirst
            | Field::BallerLast
            | Field::CabFirst
            | Field::CabLast
            | Field::UserName
            | Field::CrewCity
            | Field::CrewName
    );
    // An address runs longer than a name.
    let max = if field == Field::Server { 64 } else { MAX_LEN };
    // The on-screen keyboard, from any pad.
    let mut done = false;
    // Create Baller's Cancel (Triangle, L1, Esc) steps back a panel.
    let mut cancel = false;
    let pressed = |b: GamepadButton| gamepads.iter().any(|g| g.just_pressed(b));
    let board = board(field);
    if nav.any() != IVec2::ZERO {
        typing.key = move_key(board, typing.key, nav.any());
    }
    if field.on_create_baller()
        && (pressed(GamepadButton::North) || pressed(GamepadButton::LeftTrigger))
    {
        cancel = true;
    } else if pressed(GamepadButton::Start) || pressed(GamepadButton::East) {
        done = true;
    } else if pressed(GamepadButton::West) {
        text.pop();
    } else if pressed(GamepadButton::South) {
        let (r, c) = typing.key;
        let key = board
            .get(r)
            .and_then(|row| row.get(c))
            .copied()
            .unwrap_or("");
        match key {
            "DONE" => done = true,
            "DEL" => {
                text.pop();
            }
            "RANDOM" => {
                if let Some(names) = names.as_deref() {
                    let list = match (field, female) {
                        (Field::CabLast, _) => &names.last,
                        (_, true) => &names.first_female,
                        _ => &names.first,
                    };
                    // The C library's rand() picks the original's; the
                    // rewrite rolls on the name and the time typed.
                    typing.rolls = typing.rolls.wrapping_add(1);
                    let roll = text
                        .bytes()
                        .fold(typing.rolls.wrapping_mul(2_654_435_761), |h, c| {
                            h.rotate_left(5) ^ u32::from(c)
                        });
                    if !list.is_empty() {
                        *text = list[roll as usize % list.len()].to_uppercase();
                        text.truncate(max);
                    }
                }
            }
            "SPACE" | "SPC" if field != Field::Server => push_up_to(text, ' ', capitals, max),
            "SPACE" | "SPC" => {}
            k => {
                for ch in k.chars() {
                    push_up_to(text, ch, capitals, max);
                }
            }
        }
    }
    for key in keys.read() {
        if key.state != ButtonState::Pressed || done {
            continue;
        }
        match &key.logical_key {
            Key::Escape if field.on_create_baller() => cancel = true,
            Key::Enter | Key::Escape => done = true,
            Key::Backspace => {
                text.pop();
            }
            Key::Space if field != Field::Server => push_up_to(text, ' ', capitals, max),
            Key::Character(s) => {
                for c in s.chars() {
                    push_up_to(text, c, capitals, max);
                }
            }
            _ => {}
        }
    }
    if cancel {
        // Back a panel: from the first name to Skills, from the last to
        // the first name (`FUN_0027c0c8`).
        typing.key = (0, 0);
        typing.ended = true;
        typing.field = if field == Field::CabLast {
            typing.key = (5, 0);
            Some(Field::CabFirst)
        } else {
            None
        };
        menus.wizard_name_step(field, false);
        return;
    }
    if done {
        typing.key = (0, 0);
        // A name can't be empty (an address can: nothing to join yet).
        // (Create Baller's last name may stay empty, as the premades' do.)
        if text.trim().is_empty()
            && !matches!(field, Field::Server | Field::UserName | Field::CabLast)
        {
            *text = if field == Field::PlayerName {
                "Player".into()
            } else {
                "BALLER".into()
            };
        }
        *text = text.trim().to_owned();
        typing.field = None;
        typing.ended = true;
        // Create Baller: the first name's DONE opens the last name's
        // keyboard, the last name's the icons.
        if field.on_create_baller() {
            if field == Field::CabFirst {
                typing.field = Some(Field::CabLast);
                typing.key = (5, 0);
            }
            menus.wizard_name_step(field, true);
        }
        // A join code (or address) typed: join it.
        if field == Field::Server && !menus.online.server.is_empty() {
            menus.go_online = Some(menus.online.args(false));
        }
    }
}

/// Shows the on-screen keyboard while a pad is the last thing touched and
/// a name is being typed. A user name with the game's look is typed on
/// User Setup's own keyboard, drawn in its column (`users.rs`).
pub fn show_keyboard(
    mut commands: Commands,
    typing: Res<Typing>,
    nav: Res<crate::pads::PadNav>,
    look: Option<Res<crate::look::Look>>,
    mut panel: Query<(Entity, &mut Text), With<KeyboardPanel>>,
) {
    let in_column = typing.field.is_some_and(Field::on_name_keyboard) && look.is_some();
    let want =
        typing.field.is_some() && nav.style != crate::pads::ButtonStyle::Keyboard && !in_column;
    let board = typing.field.map_or(&KEYBOARD[..], board);
    match (want, panel.single_mut()) {
        (true, Ok((_, mut t))) => {
            let s = keyboard_text(board, typing.key);
            if t.0 != s {
                t.0 = s;
            }
        }
        (true, Err(_)) => {
            commands.spawn((
                Text::new(keyboard_text(board, typing.key)),
                TextFont {
                    font_size: bevy::text::FontSize::Px(26.0),
                    ..default()
                },
                TextLayout::justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(14.0),
                    left: Val::Percent(14.0),
                    width: Val::Percent(72.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.03, 0.12, 0.92)),
                GlobalZIndex(1000),
                KeyboardPanel,
                DespawnOnExit(crate::Screen::Menu),
            ));
        }
        (false, Ok((e, _))) => {
            commands.entity(e).despawn();
        }
        (false, Err(_)) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyboard_cursor_wraps_and_lands_on_short_rows() {
        assert_eq!(move_key(&KEYBOARD, (0, 0), IVec2::new(-1, 0)), (0, 9));
        assert_eq!(move_key(&KEYBOARD, (0, 0), IVec2::new(0, 1)), (4, 0));
        // From the right of a full row down onto DONE.
        assert_eq!(move_key(&KEYBOARD, (3, 9), IVec2::new(0, -1)), (4, 2));
        assert!(keyboard_text(&KEYBOARD, (4, 1)).contains("[DEL]"));
        // User Setup's: X down onto DEL, then DONE; up from A wraps there.
        assert_eq!(move_key(&NAME_KEYBOARD, (3, 5), IVec2::new(0, -1)), (4, 3));
        assert_eq!(move_key(&NAME_KEYBOARD, (4, 3), IVec2::new(0, -1)), (5, 0));
        assert_eq!(move_key(&NAME_KEYBOARD, (0, 0), IVec2::new(0, 1)), (5, 0));
        // A cursor left past the other keyboard's rows stays in range.
        assert_eq!(move_key(&NAME_KEYBOARD, (4, 9), IVec2::new(1, 0)), (4, 0));
    }

    #[test]
    fn create_ballers_keyboard_has_random_under_done() {
        assert_eq!(board(Field::CabFirst).len(), 7);
        assert_eq!(board(Field::CabLast)[6], ["RANDOM"]);
        assert_eq!(board(Field::UserName).len(), 6);
        // Down from DONE onto RANDOM, and on round to the letters.
        assert_eq!(move_key(&CAB_KEYBOARD, (5, 0), IVec2::new(0, -1)), (6, 0));
        assert_eq!(move_key(&CAB_KEYBOARD, (6, 0), IVec2::new(0, -1)), (0, 3));
        assert!(Field::CabFirst.on_name_keyboard() && Field::CabLast.on_create_baller());
        assert!(!Field::BallerFirst.on_create_baller());
    }

    #[test]
    fn names_stay_short_and_printable() {
        let mut s = String::new();
        for c in "jay\u{7}streets of harlem nyc".chars() {
            push_up_to(&mut s, c, true, MAX_LEN);
        }
        assert_eq!(s, "JAYSTREETS OF HA");
        let mut t = String::new();
        push_up_to(&mut t, 'b', false, MAX_LEN);
        assert_eq!(t, "b");
    }
}
