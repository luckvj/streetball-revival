//! Match-scoped presentation data. Only player definitions travel; local saves
//! and disc records stay local, including when a remote match ends.

use bevy::prelude::Resource;
use street_sim::story::{CreatedBaller, STAR_SLOTS, Story};

#[derive(Resource, Clone, Debug, Default)]
pub struct NetLooks {
    pub ballers: Vec<CreatedBaller>,
    /// Each side's city and nickname, as the match's creator's records
    /// have them: every machine shows the creator's (a created team is
    /// another disc's base team there).
    pub teams: [(String, String); 2],
    pub cheats: crate::cheats::Settings,
}

impl NetLooks {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = b"LOOK\x03".to_vec();
        out.extend(self.cheats.to_bytes());
        for name in self.teams.iter().flat_map(|(city, name)| [city, name]) {
            let name: String = name.chars().take(64).collect();
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
        }
        out.extend(
            Story {
                ballers: self.ballers.clone(),
                ..Default::default()
            }
            .write(),
        );
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let invalid = || "The match has invalid created-player data.".to_owned();
        if bytes.len() > 16_384 || !bytes.starts_with(b"LOOK\x03") {
            return Err(invalid());
        }
        let cheats = crate::cheats::Settings::from_bytes(bytes.get(5..145).ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let mut rest = &bytes[145..];
        let mut teams: [(String, String); 2] = Default::default();
        for name in teams.iter_mut().flat_map(|(city, name)| [city, name]) {
            let len = rest.get(..2).ok_or_else(invalid)?;
            let len = u16::from_le_bytes([len[0], len[1]]) as usize;
            rest = &rest[2..];
            if len > 256 {
                return Err(invalid());
            }
            *name = std::str::from_utf8(rest.get(..len).ok_or_else(invalid)?)
                .map_err(|_| invalid())?
                .to_owned();
            if name.chars().any(char::is_control) {
                return Err(invalid());
            }
            rest = &rest[len..];
        }
        let story = Story::read(rest).ok_or_else(invalid)?;
        let mut seen = [false; STAR_SLOTS];
        for b in &story.ballers {
            let slot = usize::from(b.slot);
            if slot >= seen.len()
                || seen[slot]
                || b.build > 5
                || b.female != (b.build >= 4)
                || !(60..=92).contains(&b.height_in)
                || !(100..=350).contains(&b.weight_lb)
                || b.skills.iter().any(|&s| s > 99)
                || b.clothing > 10
                || b.clothing_variant > 2
                || b.shoes >= street_assets::db::SHOE_STYLES as u8
                || [&b.first_name, &b.last_name]
                    .iter()
                    .any(|s| s.len() > 64 || s.chars().any(char::is_control))
            {
                return Err(invalid());
            }
            let (faces, hair, _) = street_assets::db::kit_limits(slot_build(b));
            if usize::from(b.look) >= faces
                || b.hair_style < -1
                || i32::from(b.hair_style) >= hair as i32
                || b.facial_hair_style < -1
                || b.facial_hair_style > 30
                || b.moves.iter().enumerate().any(|(k, m)| {
                    m.len() > street_sim::story::MOVE_SLOTS[k]
                        || m.iter()
                            .any(|&r| r >= 93 && r != street_sim::story::EMPTY_MOVE_SLOT)
                })
            {
                return Err(invalid());
            }
            seen[slot] = true;
        }
        Ok(Self {
            ballers: story.ballers,
            teams,
            cheats,
        })
    }
}

fn slot_build(b: &CreatedBaller) -> usize {
    usize::from(b.build)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_looks_round_trip_and_reject_invalid_builds() {
        let mut looks = NetLooks {
            cheats: crate::cheats::Settings::default(),
            teams: [
                ("Los Angeles".into(), "Lakers".into()),
                ("Philadelphia".into(), "76ers".into()),
            ],
            ballers: vec![CreatedBaller {
                height_in: 72,
                weight_lb: 200,
                hair_style: -1,
                facial_hair_style: -1,
                skills: [40; 7],
                moves: [vec![3], vec![10], vec![54]],
                ..Default::default()
            }],
        };
        let back = NetLooks::decode(&looks.encode()).unwrap();
        assert_eq!(back.ballers, looks.ballers);
        assert_eq!(back.teams, looks.teams);
        looks.ballers[0].build = 255;
        assert!(NetLooks::decode(&looks.encode()).is_err());
        assert!(NetLooks::decode(b"LOOK\x01\xff\xff").is_err());
        assert!(NetLooks::decode(b"LOOK\x02\xff\xff").is_err());
    }
}
