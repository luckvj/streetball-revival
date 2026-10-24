//! `street-export save-info`: what a street-rs save holds, for the builder
//! (its created ballers as `created:<slot>` characters), read with
//! street-sim's own save decoding.

use std::path::Path;

use serde_json::{Value, json};
use street_sim::story::{self, Story};

pub fn read(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let story =
        Story::read(&bytes).ok_or_else(|| format!("{}: not a street-rs save", path.display()))?;
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| crate::export::rfc3339(d.as_secs()));
    Ok(info(&story, &path.display().to_string(), modified))
}

/// The JSON for a decoded save.
pub fn info(story: &Story, path: &str, modified: Option<String>) -> Value {
    let users: Vec<Value> = story
        .records
        .users
        .iter()
        .map(|u| json!({ "name": u.name, "wins": u.wins, "losses": u.losses }))
        .collect();
    let ballers: Vec<Value> = story
        .ballers
        .iter()
        .map(|b| {
            let record = b.record();
            let rows = |k: usize| -> Vec<i64> {
                (0..story::MOVE_SLOTS[k])
                    .map(|s| match b.moves[k].get(s) {
                        Some(&r) if r != story::EMPTY_MOVE_SLOT => i64::from(r),
                        _ => -1,
                    })
                    .collect()
            };
            json!({
                "id": format!("created:{}", b.slot),
                "slot": b.slot,
                "record": record,
                "base": format!("vol2:{}", record.min(239)),
                "name": format!("{} {}", b.first_name, b.last_name).trim(),
                "fields": {
                    "firstName": b.first_name,
                    "lastName": b.last_name,
                    "female": b.female,
                    "build": b.build,
                    "heightIn": b.height_in,
                    "weightLb": b.weight_lb,
                    "look": b.look,
                    "hairStyle": b.hair_style,
                    "hairColor": b.hair_color,
                    "facialHairStyle": b.facial_hair_style,
                    "facialHairColor": b.facial_hair_color,
                    "clothing": b.clothing,
                    "clothingVariant": b.clothing_variant,
                    "shoes": b.shoes,
                    "skills": b.skills,
                    "skillNames": ["shots", "dunks", "power", "blocks", "steals", "handles", "rebounds"],
                },
                "moves": { "layups": rows(0), "dunks": rows(1), "tricks": rows(2) },
                "unlockedMoves": b.unlocked,
                "careerOnly": b.career_only(),
                "dp": story.points_for(record as u16),
            })
        })
        .collect();
    let teams: Vec<Value> = story
        .teams
        .iter()
        .map(|t| json!({ "city": t.city, "name": t.name, "logo": t.logo, "roster": t.roster }))
        .collect();
    json!({
        "path": path,
        "modified": modified,
        "users": users,
        "ballers": ballers,
        "teams": teams,
        "progress": {
            "games": story.games,
            "wins": story.wins,
            "losses": story.losses,
            "developmentPoints": story.development_points,
            "rewardPoints": story.reward_points,
            "reputation": story.reputation,
            "crew": story.crew,
            "unlockedPlayers": story.unlocked_players,
            "unlockedTeams": story.unlocked_teams,
            "ownedCards": story.owned,
            "activeCareer": story.active,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_baller_round_trips() {
        let mut story = Story::default();
        let mut b = story::CreatedBaller {
            slot: 2,
            first_name: "Ada".into(),
            last_name: "Hoops".into(),
            build: 1,
            height_in: 76,
            weight_lb: 220,
            ..Default::default()
        };
        b.moves[1] = vec![story::EMPTY_MOVE_SLOT, 7];
        story.ballers.push(b);
        let bytes = story.write();
        let back = Story::read(&bytes).unwrap();
        let v = info(&back, "save.dat", None);
        assert_eq!(v["ballers"][0]["id"], "created:2");
        assert_eq!(v["ballers"][0]["name"], "Ada Hoops");
        assert_eq!(v["ballers"][0]["moves"]["dunks"][1], 7);
        assert_eq!(v["ballers"][0]["moves"]["dunks"][0], -1);
        assert_eq!(
            v["ballers"][0]["moves"]["layups"].as_array().unwrap().len(),
            6
        );
    }
}
