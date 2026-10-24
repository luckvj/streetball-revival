//! A court's effects script, `/COURTFX/<COURT>.BIN`: plain text, one setting
//! per line, ended by `~`. The game reads it at 0x2f3b30 (`courtfx/%s.bin`).
//!
//! | Line | Meaning |
//! |---|---|
//! | `SUN_DIRECTION x y z` | the light the shadows fall along (court units) |
//! | `SHADOW_COLOUR r g b a` | the shadows' colour, 0..1 (`a` is the opacity) |
//! | `INDICATOR_CIRCLE a r g b`, `INDICATOR_STAR a r g b` | the player indicator's colours, 0..255 |
//! | `WIND speed x z` | wind for the drifting litter |
//! | `DRIFT n1 model1 n2 model2 x z` | litter blowing across ([`Drift`]) |
//! | `PS_STEAM x y z dx dy dz` | a steam vent and the way it blows |
//! | `COLD`, `HOT` | breath in the cold (`HOT` wins and turns steam off) |
//! | `PRECIPITATION`, `FOG` | snow or rain, fog (no court uses them) |
//!
//! Those are the keywords the PS2 game knows (`0x497f88..0x498078`, with
//! `PS_FOUNTAIN`, `PS_SPARKS` and `PS_SPLASH`). Lines it does not know
//! are left alone: Seattle's `PS_DUST`, and the `E STREET` and `E CAR`
//! traffic lines of three courts (the paths and cars are in
//! `/XF/XF000004`, but nothing in the PS2 executable reads them, so the
//! PS2 game has no traffic).
//!
//! Numbers may carry a trailing `f` (`0.6803f`); one file writes
//! `-21.7928.0f`, which the game reads as -21.7928.

use std::io::{Read, Seek};

use crate::{Result, iso::Iso};

#[derive(Clone, Debug, PartialEq)]
pub struct CourtFx {
    pub sun_direction: [f32; 3],
    pub shadow_colour: [f32; 4],
    pub indicator_circle: [u8; 4],
    pub indicator_star: [u8; 4],
    /// Every line, split into words, for the settings not parsed above.
    pub lines: Vec<Vec<String>>,
}

impl Default for CourtFx {
    fn default() -> Self {
        Self {
            sun_direction: [0.0, -20.0, -12.0],
            shadow_colour: [0.0, 0.0, 0.1, 0.6],
            indicator_circle: [146, 32, 32, 32],
            indicator_star: [200, 140, 153, 190],
            lines: Vec::new(),
        }
    }
}

/// Litter blowing across the court (`DRIFT`, built at 0x1f4248): `count`
/// pieces of each model from `/XF/XF000004` (`<model>.o` with its sheet),
/// scattered over x in -`half[0]`..`half[0]`, z in -`half[1]`..`half[1]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Drift {
    /// (model, count): the first lies at height 0.25 and is drawn at half
    /// size, the second at 0.375 and full size.
    pub kinds: [(String, u32); 2],
    pub half: [f32; 2],
}

/// The wind (`WIND speed x z`): its speed and its direction on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub speed: f32,
    pub direction: [f32; 2],
}

/// A steam vent (`PS_STEAM`, type 7 at 0x20a4d0): where it is and the way
/// its steam blows (normalised; zero for straight up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steam {
    pub at: [f32; 3],
    pub blow: [f32; 3],
}

/// A number the way the game's reader takes it: up to the first character
/// that cannot continue it.
fn number(word: &str) -> Option<f32> {
    let mut end = 0;
    let mut dot = false;
    for (i, c) in word.char_indices() {
        let ok = c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')) || (c == '.' && !dot);
        if !ok {
            break;
        }
        dot |= c == '.';
        end = i + c.len_utf8();
    }
    word[..end].parse().ok()
}

impl CourtFx {
    pub fn parse(text: &str) -> Self {
        let mut fx = CourtFx::default();
        for line in text.lines() {
            let words: Vec<String> = line.split_whitespace().map(String::from).collect();
            let Some(key) = words.first() else { continue };
            if key == "~" {
                break;
            }
            let floats: Vec<f32> = words[1..].iter().filter_map(|w| number(w)).collect();
            let bytes = || {
                let mut b = [0u8; 4];
                for (o, v) in b.iter_mut().zip(&floats) {
                    *o = v.clamp(0.0, 255.0) as u8;
                }
                b
            };
            match key.as_str() {
                "SUN_DIRECTION" if floats.len() >= 3 => {
                    fx.sun_direction = [floats[0], floats[1], floats[2]];
                }
                "SHADOW_COLOUR" if floats.len() >= 4 => {
                    fx.shadow_colour = [floats[0], floats[1], floats[2], floats[3]];
                }
                "INDICATOR_CIRCLE" => fx.indicator_circle = bytes(),
                "INDICATOR_STAR" => fx.indicator_star = bytes(),
                _ => {}
            }
            fx.lines.push(words);
        }
        fx
    }

    fn line(&self, key: &str) -> impl Iterator<Item = &[String]> {
        self.lines
            .iter()
            .filter(move |l| l.first().is_some_and(|k| k == key))
            .map(|l| &l[1..])
    }

    fn floats(words: &[String]) -> Vec<f32> {
        words.iter().filter_map(|w| number(w)).collect()
    }

    pub fn wind(&self) -> Option<Wind> {
        let f = Self::floats(self.line("WIND").next()?);
        let [speed, x, z] = f[..] else { return None };
        Some(Wind {
            speed,
            direction: [x, z],
        })
    }

    pub fn drift(&self) -> Option<Drift> {
        let w = self.line("DRIFT").next()?;
        let [n1, m1, n2, m2, x, z] = w else {
            return None;
        };
        let count = |n: &String| number(n).map_or(0, |v| v.max(0.0) as u32);
        Some(Drift {
            kinds: [(m1.clone(), count(n1)), (m2.clone(), count(n2))],
            half: [number(x)?, number(z)?],
        })
    }

    /// `HOT` turns steam off (0x2f3b30 skips `PS_STEAM` then).
    pub fn steam(&self) -> Vec<Steam> {
        if self.hot() {
            return Vec::new();
        }
        self.line("PS_STEAM")
            .filter_map(|w| {
                let f = Self::floats(w);
                let [x, y, z, dx, dy, dz] = f[..] else {
                    return None;
                };
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                let blow = if len > 0.0 {
                    [dx / len, dy / len, dz / len]
                } else {
                    [0.0; 3]
                };
                Some(Steam {
                    at: [x, y, z],
                    blow,
                })
            })
            .collect()
    }

    /// Breath shows in the cold: `COLD`, unless `HOT` came first.
    pub fn cold(&self) -> bool {
        self.climate() == Some("COLD")
    }

    fn hot(&self) -> bool {
        self.climate() == Some("HOT")
    }

    /// The first of `COLD` and `HOT`: the parser keeps whichever came first.
    fn climate(&self) -> Option<&str> {
        self.lines
            .iter()
            .filter_map(|l| l.first().map(String::as_str))
            .find(|k| matches!(*k, "COLD" | "HOT"))
    }

    /// The effects script of the court whose archive is `/XF/<id>.VIV`.
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>, court_id: &str) -> Result<Self> {
        let bytes = iso.read(&format!("/COURTFX/{}.BIN", court_id.to_ascii_uppercase()))?;
        Ok(Self::parse(&String::from_utf8_lossy(&bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_lines_the_game_reads() {
        let fx = CourtFx::parse(
            "SUN_DIRECTION -21.7928.0f -19.8029f 10.3571f\r\nSHADOW_COLOUR 0.0f 0.04f 0.1f 0.53f\r\n\
             INDICATOR_STAR 200 140 153 190\r\nWIND 15.0f -1.0f 1.0f\r\n~\r\nSUN_DIRECTION 1 2 3\r\n",
        );
        assert_eq!(fx.sun_direction, [-21.7928, -19.8029, 10.3571]);
        assert_eq!(fx.shadow_colour, [0.0, 0.04, 0.1, 0.53]);
        assert_eq!(fx.indicator_star, [200, 140, 153, 190]);
        assert_eq!(fx.lines.len(), 4);
    }

    #[test]
    fn reads_the_ambient_effects() {
        let fx = CourtFx::parse(
            "WIND 15.0f -1.0f 1.0f\r\nDRIFT 0 0 5 trash 55.0f 32.8f\r\n\
             PS_STEAM -61.808f 6.2257f 10.181f 0.1f 0.0f 0.0f\r\nPS_DUST 0\r\n~\r\n",
        );
        assert_eq!(
            fx.wind(),
            Some(Wind {
                speed: 15.0,
                direction: [-1.0, 1.0]
            })
        );
        let drift = fx.drift().unwrap();
        assert_eq!(drift.kinds, [("0".into(), 0), ("trash".into(), 5)]);
        assert_eq!(drift.half, [55.0, 32.8]);
        assert_eq!(
            fx.steam(),
            [Steam {
                at: [-61.808, 6.2257, 10.181],
                blow: [1.0, 0.0, 0.0]
            }]
        );
        assert!(!fx.cold());
        assert!(CourtFx::parse("COLD 0\r\n~").cold());
    }
}
