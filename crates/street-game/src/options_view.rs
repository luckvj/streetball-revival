//! The Options screen on its layout (`FUN_002a4c58`: class 0x28, layout 40,
//! backdrop 0x1c). List 55 holds the rows two widgets each: the label (a
//! kind-19 row, font 2, left on the even nodes 75..89 at x 92) and its value
//! widget (centred on the odd nodes 76..90 at x 446): an option widget's
//! words (the kind-19 choices 36..38, 29..31, 119..121, the On and Off
//! rows), lit gold with the list's orange arrows each side while focused,
//! or a volume's slider (`FUN_0021d148`). Eight rows show; the list scrolls
//! one row at a time and shows its arrows at (80, 101) and (80, 363) while
//! there is more above or below. The layout's legends: Credits on Circle
//! (class 9), Cancel on Triangle, Exit on Start, Help. Select asks "Exit to
//! Main Menu?" (`FUN_0024bfa0(.., 0x85)`, [`crate::exit_ask`]).
//! The help line (element 114) follows the focused row (`FUN_002a5150`).

use bevy::prelude::*;
use street_assets::font::Layout;
use street_sim::{Difficulty, records::Settings};

use crate::{
    audio::AudioSettings,
    choose::{place, spot},
    look::{self, Look, Piece, colours},
    options::{self, Value},
};

/// The screen's layout.
pub const CLASS: u32 = 40;
/// Rows the list shows.
pub const SHOWN: usize = 8;

/// The list arrows' middles (the screen's list 55, measured on
/// `options_scrolled.png`).
const ARROW_UP: Vec2 = Vec2::new(80.0, 101.0);
const ARROW_DOWN: Vec2 = Vec2::new(80.0, 363.0);
/// The option widget's arrows, each side of its value's middle (measured on
/// `options.png`: 117 either way).
const ARROW_REACH: f32 = 117.0;
/// A slider's middle below its node (measured on `options.png`).
const SLIDER_DROP: f32 = 10.0;

/// The kind-10 nodes of list 55 by their place (prop 34).
fn nodes(layout: &Layout) -> Vec<Vec2> {
    let mut rows: Vec<(u32, Vec2)> = layout
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&55))
        .filter_map(|e| Some((e.props.get(&34).copied().unwrap_or(0), spot(e)?)))
        .collect();
    rows.sort_by_key(|&(k, _)| k);
    rows.into_iter().map(|(_, at)| at).collect()
}

/// What the screen shows.
pub struct Shown<'a> {
    pub settings: &'a Settings,
    pub sound: &'a AudioSettings,
    pub difficulty: Difficulty,
    pub cursor: usize,
    pub top: usize,
    pub ticks: u64,
}

/// The rows' pieces.
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    s: &Shown,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let nodes = nodes(layout);
    let font = 2;
    // The focused row's words pulse as the row widgets' do (`look::row_pulse`).
    let lit_colour = {
        let mut c = colours::ROW_LIT;
        c[3] = look::row_pulse(s.ticks);
        c
    };
    for k in 0..SHOWN {
        let at = s.top + k;
        if at >= options::ROWS {
            break;
        }
        let (Some(&label_at), Some(&value_at)) = (nodes.get(2 * k), nodes.get(2 * k + 1)) else {
            break;
        };
        let lit = at == s.cursor;
        let colour = if lit { lit_colour } else { colours::ROW };
        let words = options::label(at)
            .and_then(|i| look.text("options.txt", i))
            .unwrap_or("Option");
        place(
            &mut out, labels, look, images, font, words, colour, label_at, 0,
        );
        match options::value(at, s.settings, s.sound, s.difficulty) {
            Some(Value::Words(i, plain)) => {
                let words = look.text("options.txt", i).unwrap_or(plain);
                place(
                    &mut out, labels, look, images, font, words, colour, value_at, 1,
                );
                if lit && let Some(pm) = look.prematch.as_ref() {
                    let middle = value_at.y
                        + look
                            .fonts
                            .get(font)
                            .map_or(14.0, |f| f.text_height as f32 / 2.0);
                    for (image, dx) in [
                        (&pm.arrow_left, -ARROW_REACH),
                        (&pm.arrow_right, ARROW_REACH),
                    ] {
                        out.push(Piece::new(
                            image.clone(),
                            Rect::from_center_size(
                                Vec2::new(value_at.x + dx, middle),
                                pm.arrow_side_size,
                            ),
                        ));
                    }
                }
            }
            Some(Value::Slider(level)) => {
                look.slider_at(
                    &mut out,
                    value_at + Vec2::new(0.0, SLIDER_DROP),
                    level,
                    10,
                    lit,
                );
            }
            None => {}
        }
    }
    if let Some(pm) = look.prematch.as_ref() {
        if s.top > 0 {
            out.push(Piece::new(
                pm.arrow_up.clone(),
                Rect::from_center_size(ARROW_UP, pm.arrow_size),
            ));
        }
        if s.top + SHOWN < options::ROWS {
            out.push(Piece::new(
                pm.arrow_down.clone(),
                Rect::from_center_size(ARROW_DOWN, pm.arrow_size),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_scrolls_one_row_at_a_time() {
        // Down past the eighth row moves the window by one
        // (`options_scrolled.png`: Replays lit at the bottom, Controller
        // Layout on top).
        assert_eq!(crate::cab_view::scrolled(0, 8, options::ROWS, SHOWN), 1);
        assert_eq!(crate::cab_view::scrolled(1, 0, options::ROWS, SHOWN), 0);
        assert_eq!(
            crate::cab_view::scrolled(0, options::row::EXIT, options::ROWS, SHOWN),
            options::ROWS - SHOWN
        );
    }
}
