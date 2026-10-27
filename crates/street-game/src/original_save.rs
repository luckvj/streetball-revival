//! Import original US PS2 saves into the native progression file.
//! Fields without an established native mapping remain in the original bytes.
use crate::setup::Disc;
use street_assets::save::{GciSave, Ps2Save, Section};
use street_sim::{
    bracket::{Bracket, Entrant},
    records::{
        GameRecord, LeagueProgress, Legend, Row, SchoolProgress, Settings, User, WinRow, c_qsort,
    },
    story::{Career, CreatedBaller, CreatedTeam, EVENTS, EventKind, Story},
};

#[derive(Debug)]
pub struct Import {
    pub story: Story,
    pub original: Original,
    pub notes: Vec<String>,
}

/// The complete source data retained alongside a converted native save.
#[derive(Debug)]
pub enum Original {
    Ps2(Ps2Save),
    GameCube(Vec<u8>),
}
impl Original {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Ps2(save) => save.to_bytes(),
            Self::GameCube(bytes) => bytes.clone(),
        }
    }
    fn suffix(&self) -> &'static str {
        match self {
            Self::Ps2(_) => ".original-ps2.dat",
            Self::GameCube(_) => ".original.gci",
        }
    }
}

impl Import {
    /// Preserve the original envelope alongside a new native file. Neither
    /// an existing save nor the input archive can be replaced by this import.
    pub fn store_new(&self, destination: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut original_name = destination.as_os_str().to_os_string();
        original_name.push(self.original.suffix());
        let original_path = std::path::PathBuf::from(original_name);
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut original = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&original_path)
        {
            Ok(file) => file,
            Err(error) => {
                drop(output);
                let _ = std::fs::remove_file(destination);
                return Err(error);
            }
        };
        let result = original
            .write_all(&self.original.to_bytes())
            .and_then(|()| original.sync_all())
            .and_then(|()| output.write_all(&self.story.write()))
            .and_then(|()| output.sync_all());
        if result.is_err() {
            drop(original);
            drop(output);
            let _ = std::fs::remove_file(&original_path);
            let _ = std::fs::remove_file(destination);
        }
        result
    }
}

pub fn import(bytes: &[u8], disc: &Disc) -> Result<Import, String> {
    if bytes.starts_with(b"GNZE69") {
        let gci = GciSave::parse(bytes).map_err(|e| e.to_string())?;
        gci.validate_sections().map_err(|e| e.to_string())?;
        let fields = gamecube_scalar_fields(&gci)?;
        let mut imported = import(&fields.to_bytes(), disc)?;
        imported.original = Original::GameCube(bytes.to_vec());
        imported.notes.push("GameCube scalar records, player fields, brackets, options, and ordinary unlock bitsets imported. Full original container preserved.".into());
        return Ok(imported);
    }
    let original = Ps2Save::read(bytes).map_err(|e| e.to_string())?;
    let mut story = Story::default();
    let mut notes = Vec::new();
    let globals = original.section(Section::Unlocks);
    story.reward_points = word(globals, 0);
    story.pickup_wins = word(globals, 4);
    story.milestones = word(globals, 0x1c) & ((1 << 19) - 1);
    story.school_trophy = word(globals, 0x18) & 1 != 0;
    if word(globals, 0x18) & 2 != 0 {
        story.league_rewarded = true;
    }
    for card in &disc.cards {
        let Ok(id) = usize::try_from(card.id) else {
            continue;
        };
        if id < 113 && word(globals, 8 + (id / 32) * 4) & (1 << (id % 32)) != 0 {
            story.owned.push(id as u16);
            for &record in crate::rewards::card_records(card) {
                story.unlock_player(record);
            }
        }
    }
    let options = original.section(Section::Options);
    story.settings = Some(options_from_original(options));
    for index in 0..32 {
        let user = original.user_info(index).expect("fixed section");
        if !user.is_used() {
            continue;
        }
        let count = |i| user.counter(i).unwrap_or(0).max(0) as u32;
        let name = user.name();
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            continue;
        }
        let flags = user.progress_flags();
        let region = flags & 0x3f;
        let mut league = LeagueProgress {
            cleared: (user.challenge_groups() & 0b1_1111) as u8,
            team: u16::try_from(user.current_court()).ok(),
            native_flags: Some(flags),
            // +0x48 bit6 is the pending region's unbeaten flag, not a
            // global career loss count. Other regions initialize on entry.
            lost: if region < 5 && flags & 0x40 == 0 {
                1 << region
            } else {
                0
            },
            ..Default::default()
        };
        if let Some(team) = league.team.and_then(|id| disc.teams.get(usize::from(id))) {
            league.roster = team.roster.iter().take(3).map(|&r| r as u16).collect();
        }
        // FUN_001ee548 flattens the first five region lists into one mask.
        let mut original_bit = 0;
        for (region, teams) in street_sim::story::REGION_MATCHES.iter().enumerate() {
            for game in 0..teams.len() {
                if region < 5
                    && original_bit < 32
                    && user.court_results() & (1 << original_bit) != 0
                {
                    league.played |= 1u64 << (region * 8 + game);
                }
                original_bit += 1;
            }
        }
        let mut school = SchoolProgress {
            flags: user.school_flags(),
            ..Default::default()
        };
        for (index, value) in school.challenges.iter_mut().enumerate() {
            let challenge = user.challenge(index).expect("all33 native nibbles");
            *value = challenge.result | (u8::from(challenge.completed) << 3);
        }
        for (index, attempts) in school.attempts.iter_mut().enumerate() {
            *attempts = user
                .challenge_attempts(index)
                .expect("all26 native counters");
        }
        story.records.users.push(User {
            name,
            wins: count(0),
            losses: count(1),
            career: [
                count(2),
                count(3),
                count(4),
                count(5),
                count(6),
                count(8),
                count(7),
            ],
            league,
            school,
        });
    }
    notes.push("League Challenge progress imported separately for each user name; guest progress is not stored in original user records. Original team IDs and packed region flags are retained with each profile.".into());
    notes.push("Street School's33 result nibbles,26 signed launch counters and completion payout flag imported separately for each user. Shared lifetime School trophy follows the original game record.".into());
    import_scores(original.section(Section::HighScores), &mut story);
    let mut mapped_stars = Vec::new();
    for career_index in 0..10 {
        let bytes = original.career(career_index).expect("fixed section");
        let player = &bytes[0x6d4..0x7f8];
        let first_name = text(&player[0x22..0x42]);
        let last_name = text(&player[2..0x22]);
        if first_name.trim().is_empty() && last_name.trim().is_empty() {
            continue;
        }
        // Each career keeps its own player (+0x6d4): the first six fill the
        // Create A Baller slots, the rest get career-only star slots, so ten
        // careers keep ten distinct stars.
        if story.ballers.len() >= street_sim::story::STAR_SLOTS {
            continue;
        }
        let slot = story.ballers.len() as u8;
        let mut baller = baller_from_original(player, slot);
        // FUN_001e7618: three ordinary bitset words at object+0x664.
        // These include purchased moves no longer assigned to a move slot.
        for id in 0..93 {
            if word(bytes, 0x64c + (id / 32) * 4) & (1 << (id % 32)) != 0 {
                baller.unlocked.push(id as u8);
            }
        }
        baller.unlocked.sort_unstable();
        baller.unlocked.dedup();
        // The wardrobe at object +0x670, +0x674, +0x678 (`FUN_001e7680`,
        // `FUN_001e7758`): outfit rows, rows by colour, shoe rows.
        baller.wardrobe = street_sim::cab::Wardrobe {
            outfits: word(bytes, 0x658) & 0x1ff,
            colours: word(bytes, 0x65c) & ((1 << 27) - 1),
            shoes: word(bytes, 0x660) & ((1 << 20) - 1),
        };
        let record = baller.record() as u16;
        story.ballers.push(baller);
        mapped_stars.push((245 + career_index as i16, record));
    }
    let resolve = |id: i16| {
        mapped_stars
            .iter()
            .find(|(original, _)| *original == id)
            .map(|(_, record)| *record)
            .or_else(|| (id >= 0 && (id as usize) < disc.players.len()).then_some(id as u16))
    };
    // The league teams' sixth men: the game data's +0x3c, this section's
    // +0x24, a player id or -1 for each of the 29 (`FUN_001e0000`); a
    // created baller's id is its career's (245 on).
    for team in 0..street_sim::story::LEAGUE_TEAMS {
        let id = half(globals, 0x24 + team * 2);
        if let Some(&(_, record)) = mapped_stars.iter().find(|(original, _)| *original == id) {
            story.join_team(team as u16, record);
        }
    }
    for index in 0..10 {
        let native = original.career_info(index).expect("fixed section");
        let Some((_, star)) = mapped_stars
            .iter()
            .find(|(id, _)| *id == 245 + index as i16)
        else {
            continue;
        };
        let raw = original.career(index).unwrap();
        let games = native.counter(0).unwrap_or(0).max(0) as u32;
        let wins = native.counter(1).unwrap_or(0).max(0) as u32;
        let losses = native.counter(2).unwrap_or(0).max(0) as u32;
        let mut career = Career {
            games,
            wins,
            losses,
            reputation: half(raw, 0x72e).max(0),
            nickname_id: half(raw, 0x71c),
            // Runtime profile+0x18 is the serialized payload base.
            nickname_pending: raw[1] & 1 != 0,
            crew: native
                .player_ids()
                .into_iter()
                .filter_map(resolve)
                .take(5)
                .collect(),
            // The career's own team record (+0x664): its city, name and
            // logo, as `FUN_00280cd0` named them.
            team: street_sim::story::CrewTeam {
                city: native.team().name_part(1).unwrap_or_default(),
                name: native.team().name_part(2).unwrap_or_default(),
                logo: native.team().field_10().clamp(0, 255) as u8,
            },
            ..Default::default()
        };
        career.nickname = career.nickname_id >= 0;
        for (ev, event) in EVENTS.iter().enumerate() {
            match event.kind {
                EventKind::Pickup => career.progress[ev] = native.event_tally(ev).unwrap_or(0),
                EventKind::StreetChallenge => {
                    let ordinal = EVENTS[..ev]
                        .iter()
                        .filter(|e| e.kind == EventKind::StreetChallenge)
                        .count();
                    if raw[0x648] & (1 << ordinal) != 0 {
                        career.street_cleared |= 1 << street_sim::story::street_ordinal(ev);
                        career.progress[ev] = event.target.max(0) as u8;
                    }
                }
                EventKind::Tournament => {
                    let ordinal = EVENTS[..ev]
                        .iter()
                        .filter(|e| e.kind == EventKind::Tournament)
                        .count();
                    let mut bracket = Story::default().tournament(ev);
                    fill_bracket(
                        &mut bracket,
                        &raw[0x34 + ordinal * 0x104..0x34 + (ordinal + 1) * 0x104],
                    );
                    if bracket.won() {
                        career.tournament_cleared |= 1 << ev;
                    }
                    career.brackets.push((ev as u8, bracket));
                }
            }
        }
        story.credit_baller(*star, native.counter(3).unwrap_or(0).max(0) as u32);
        story.careers[index] = Some(career);
    }
    for index in 0..10 {
        let team = original.team_info(index).unwrap();
        let roster: Vec<u16> = team.player_ids().into_iter().filter_map(resolve).collect();
        if roster.len() < 3 {
            continue;
        }
        let mut players = [roster[0]; 5];
        for (target, source) in players.iter_mut().zip(roster) {
            *target = source;
        }
        story.teams.push(CreatedTeam {
            name: team.name_part(2).unwrap_or_default(),
            city: team.name_part(1).unwrap_or_default(),
            logo: team.field_10().clamp(0, 255) as u8,
            roster: players,
        });
    }
    if let Some(index) = story.careers.iter().position(Option::is_some) {
        story.switch_career(index);
    }

    notes.push("Original bytes retained, including unused records, extra player fields and memory-card-only metadata.".into());
    Ok(Import {
        story,
        original: Original::Ps2(original),
        notes,
    })
}

/// Normalize only fields with traced scalar/explicit-mask access. This
/// temporary six-section image is consumed by our importer, never written as
/// a replacement GameCube/PS2 save. The full GCI is the preservation artifact.
fn gamecube_scalar_fields(gci: &GciSave) -> Result<Ps2Save, String> {
    let mut envelope = Vec::new();
    for length in street_assets::save::SECTION_LENGTHS {
        envelope.extend_from_slice(&(length as u32).to_le_bytes());
        envelope.resize(envelope.len() + length, 0);
    }
    let mut result = Ps2Save::parse(&envelope).map_err(|e| e.to_string())?;
    let source = |section| gci.section(section).map_err(|e| e.to_string());
    let unlocks = source(Section::Unlocks)?;
    let target = result.section_mut(Section::Unlocks);
    for at in [0, 4, 8, 12, 16, 20, 0x18, 0x1c] {
        swap32(target, unlocks, at);
    }
    let options = source(Section::Options)?;
    let target = result.section_mut(Section::Options);
    swap16(target, options, 0);
    for at in [4, 8, 12] {
        swap32(target, options, at);
    }
    let scores = source(Section::HighScores)?;
    let target = result.section_mut(Section::HighScores);
    target.copy_from_slice(scores);
    for row in 0..95 {
        for field in [20, 24] {
            swap32(target, scores, row * 28 + field);
        }
    }
    let users = source(Section::Users)?;
    let target = result.section_mut(Section::Users);
    for row in 0..32 {
        let at = row * 0xbc;
        target[at..at + 10].copy_from_slice(&users[at..at + 10]);
        for field in (0xc..0x30)
            .step_by(4)
            .chain([0x34, 0x38, 0x3c])
            .chain((0x40..0xbc).step_by(4))
        {
            swap32(target, users, at + field);
        }
        swap16(target, users, at + 0x32);
    }
    let careers = source(Section::Careers)?;
    let target = result.section_mut(Section::Careers);
    for row in 0..10 {
        let at = row * 0xc88;
        for field in (0xc..0x1c).step_by(4).chain((0x64c..0x658).step_by(4)) {
            swap32(target, careers, at + field);
        }
        target[at + 0x1c..at + 0x2d].copy_from_slice(&careers[at + 0x1c..at + 0x2d]);
        target[at + 0x648] = careers[at + 0x648];
        // FUN_001e8608 / 001e8790: two i16 IDs, two i32 crew keys.
        // FUN_001e8d60: the two result masks are bytes at +12 and +13.
        for tournament in 0..6 {
            for row in 0..16 {
                let offset = at + 0x34 + tournament * 0x104 + row * 16;
                for field in [0, 2] {
                    swap16(target, careers, offset + field);
                }
                for field in [4, 8] {
                    swap32(target, careers, offset + field);
                }
                target[offset + 12..offset + 14]
                    .copy_from_slice(&careers[offset + 12..offset + 14]);
            }
        }
        team_scalars(target, careers, at + 0x664);
        for player in 0..5 {
            let offset = at + 0x6d4 + player * 0x124;
            target[offset..offset + 0x124].copy_from_slice(&careers[offset..offset + 0x124]);
            // FUN_001d89d0: two name strings and code text stay byte strings;
            // the remaining serialized player properties are i16 fields.
            for field in std::iter::once(0)
                .chain((0x42..0x76).step_by(2))
                .chain((0x96..0x124).step_by(2))
            {
                swap16(target, careers, offset + field);
            }
        }
    }
    let teams = source(Section::Teams)?;
    let target = result.section_mut(Section::Teams);
    for row in 0..10 {
        team_scalars(target, teams, row * 0x70);
    }
    Ok(result)
}
fn team_scalars(target: &mut [u8], source: &[u8], at: usize) {
    target[at..at + 0x70].copy_from_slice(&source[at..at + 0x70]);
    for field in std::iter::once(0).chain((0x62..0x70).step_by(2)) {
        swap16(target, source, at + field);
    }
}
fn swap16(target: &mut [u8], source: &[u8], at: usize) {
    target[at..at + 2]
        .copy_from_slice(&u16::from_be_bytes(source[at..at + 2].try_into().unwrap()).to_le_bytes());
}
fn swap32(target: &mut [u8], source: &[u8], at: usize) {
    target[at..at + 4]
        .copy_from_slice(&u32::from_be_bytes(source[at..at + 4].try_into().unwrap()).to_le_bytes());
}

fn options_from_original(bytes: &[u8]) -> Settings {
    let flags = u16::from_le_bytes(bytes[..2].try_into().unwrap());
    let volumes = word(bytes, 4);
    let pad = word(bytes, 12);
    let trax = word(bytes, 8);
    // FUN_00219588 / tables 0x467710 and 0x4678d0: the first nine
    // enabled tracks are front-end songs, followed by PATHSONG.CFG's 15.
    // The original orders Nelly before Pete Rock; our FE catalog reverses them.
    let mut songs_off = 0;
    for original in 0..24 {
        let native = match original {
            0 => street_sim::records::FRONT_END_BIT + 1,
            1 => street_sim::records::FRONT_END_BIT,
            2..=8 => street_sim::records::FRONT_END_BIT + original,
            _ => original - 9,
        };
        if trax & (1 << (original + 3)) == 0 {
            songs_off |= 1 << native;
        }
    }
    Settings {
        difficulty: (flags & 3).min(2) as u8,
        help: flags & 4 != 0,
        autosave: flags & 8 != 0,
        visual_effects: flags & 32 != 0,
        widescreen: flags & 16 != 0,
        // Original selector values 0..4 correspond to modes 0,1,2,3,5.
        camera: match (flags >> 6) & 7 {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 5,
            _ => 0,
        },
        sequential: trax & 2 == 0,
        interactive: trax & 1 != 0,
        track_info: trax & 4 != 0,
        songs_off,
        replays: ((flags >> 12) & 3).min(2) as u8,
        audio: ((volumes >> 2) & 7).min(2) as u8,
        effects: ((volumes >> 20) & 15).min(10) as u8,
        music: ((volumes >> 24) & 15).min(10) as u8,
        speech: ((volumes >> 28) & 15).min(10) as u8,
        vibration: pad & 1 != 0,
        layouts: std::array::from_fn(|i| ((pad >> (i * 3 + 1)) & 3).min(2) as u8),
        ..Default::default()
    }
}

fn baller_from_original(bytes: &[u8], slot: u8) -> CreatedBaller {
    let clothing = half(bytes, 0xa2) as u16;
    let list = |start: usize, count: usize| {
        (0..count)
            .map(|i| half(bytes, start + i * 2))
            .map(|i| {
                if (0..93).contains(&i) {
                    i as u8
                } else {
                    street_sim::story::EMPTY_MOVE_SLOT
                }
            })
            .collect::<Vec<_>>()
    };
    let moves: [Vec<u8>; 3] = [list(0xe2, 6), list(0xf8, 11), list(0x10e, 11)];
    let moves = moves.map(|rows| {
        if rows
            .iter()
            .all(|&r| r == street_sim::story::EMPTY_MOVE_SLOT)
        {
            Vec::new()
        } else {
            rows
        }
    });
    let mut unlocked: Vec<u8> = moves
        .iter()
        .flatten()
        .copied()
        .filter(|&r| r != street_sim::story::EMPTY_MOVE_SLOT)
        .collect();
    unlocked.sort_unstable();
    unlocked.dedup();
    CreatedBaller {
        slot,
        first_name: text(&bytes[0x22..0x42]),
        last_name: text(&bytes[2..0x22]),
        female: half(bytes, 0x54) != 0,
        build: half(bytes, 0x96).clamp(0, 5) as u8,
        height_in: half(bytes, 0x4c).clamp(60, 92) as u8,
        weight_lb: half(bytes, 0x4e).clamp(100, 350) as u16,
        look: half(bytes, 0x9a).max(0) as u8,
        hair_style: half(bytes, 0xa8) as i8,
        hair_color: half(bytes, 0xaa).max(0) as u8,
        facial_hair_style: half(bytes, 0xac) as i8,
        facial_hair_color: half(bytes, 0xae).max(0) as u8,
        clothing: (clothing & 0x3fff) as u8,
        clothing_variant: (clothing >> 14) as u8,
        shoes: half(bytes, 0xa6).max(0) as u8,
        skills: [0, 2, 4, 5, 6, 7, 3]
            .map(|rating| half(bytes, 0x5c + rating * 2).clamp(0, 99) as u8),
        moves,
        unlocked,
        wardrobe: Default::default(),
        // Field 14 (the record's +0x58).
        icon: half(bytes, 0x58).clamp(0, 6) as u8,
    }
}

fn fill_bracket(bracket: &mut Bracket, bytes: &[u8]) {
    for (i, row) in bracket.rows.iter_mut().enumerate() {
        let at = i * 16;
        row.sides = std::array::from_fn(|side| match half(bytes, at + side * 2) {
            -1 => Entrant::Empty,
            37 => Entrant::You,
            36 => Entrant::Crew(word(bytes, at + 4 + side * 4)),
            team if team >= 0 => Entrant::Boss(team as u16),
            _ => Entrant::Empty,
        });
        let bits = word(bytes, at + 12);
        row.wins = [bits as u8, (bits >> 8) as u8];
    }
}

fn import_scores(bytes: &[u8], story: &mut Story) {
    for index in 0..5 {
        let row = &bytes[index * 28..(index + 1) * 28];
        let name = [text(&row[..9]), text(&row[9..18])]
            .join(" ")
            .trim()
            .to_owned();
        if !name.is_empty() {
            story.records.legends.push(Legend {
                name,
                // The nickname's id (+0x12, -1 for none) needs the disc's
                // names; the rows the executable starts with have none.
                nickname: String::new(),
                wins: word(row, 20),
                losses: word(row, 24),
            });
        }
    }
    for index in 5..15 {
        let row = &bytes[index * 28..(index + 1) * 28];
        let name = text(&row[..9]);
        if !name.is_empty() {
            story.records.win_rows.push(WinRow {
                name,
                wins: word(row, 20),
                losses: word(row, 24),
            });
        }
    }
    for (group, kind) in [
        GameRecord::TrickPoints,
        GameRecord::Gamebreakers1,
        GameRecord::Gamebreakers2,
        GameRecord::AlleyOops,
        GameRecord::Blocks,
        GameRecord::Steals,
        GameRecord::LongRange,
        GameRecord::Dunks,
    ]
    .into_iter()
    .enumerate()
    {
        for index in 0..10 {
            let at = (15 + group * 10 + index) * 28;
            let row = &bytes[at..at + 28];
            let name = [text(&row[..9]), text(&row[9..18])]
                .join(" ")
                .trim()
                .to_owned();
            if !name.is_empty() {
                story.records.games[kind.index()].push(Row {
                    name,
                    user: String::new(),
                    value: word(row, 20),
                });
            }
        }
    }
}

/// The high scores a fresh profile starts with (`FUN_001d3d98(scores, 0)`):
/// the executable's 95 rows at `0x45e2d0` (28 bytes each: two names, the
/// figures at +0x14 and +0x18), each of the ten tables (5 rows, then nine of
/// 10, `DAT_0045e2a8`) sorted by the C library's `qsort` (`FUN_003b3868`,
/// [`c_qsort`](street_sim::records::c_qsort)) as `FUN_001d43a0` asks: tables 0 and 1 by the share
/// `trunc(1000 * a / (a + b))` (0x1d47b8), the rest by +0x14 (0x1d47a8), the
/// larger first. The save states of two boots hold exactly this.
pub fn default_scores(elf: &[u8]) -> Option<Vec<u8>> {
    let words = street_assets::scripts::executable_words(elf, 0x45e2d0, 0xa64 / 4).ok()?;
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let mut rows: Vec<[u8; 28]> = bytes.chunks(28).map(|r| r.try_into().unwrap()).collect();
    let figure = |r: &[u8; 28], at: usize| word(r, at);
    let share = |r: &[u8; 28]| street_sim::records::share(figure(r, 0x14), figure(r, 0x18));
    let mut at = 0;
    for (table, n) in std::iter::once(5)
        .chain(std::iter::repeat_n(10, 9))
        .enumerate()
    {
        let part = &mut rows[at..at + n];
        if table < 2 {
            c_qsort(part, &|a, b| share(b) - share(a));
        } else {
            c_qsort(part, &|a, b| {
                figure(b, 0x14).wrapping_sub(figure(a, 0x14)) as i32
            });
        }
        at += n;
    }
    Some(rows.concat())
}

/// The high score tables of a fresh profile ([`default_scores`]) as the
/// rewrite keeps them.
pub fn default_records(elf: &[u8]) -> Option<street_sim::records::Records> {
    let mut story = Story::default();
    import_scores(&default_scores(elf)?, &mut story);
    Some(story.records)
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn half(bytes: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .take_while(|&b| b != 0)
        .map(char::from)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_equipped_move_slots_keep_native_holes() {
        let mut bytes = vec![0; 0x130];
        for (start, count) in [(0xe2, 6), (0xf8, 11), (0x10e, 11)] {
            for slot in 0..count {
                bytes[start + slot * 2..start + slot * 2 + 2]
                    .copy_from_slice(&(-1i16).to_le_bytes());
            }
        }
        bytes[0x122..0x124].copy_from_slice(&69i16.to_le_bytes());
        let b = baller_from_original(&bytes, 0);
        assert_eq!(b.moves[2].len(), 11);
        assert_eq!(b.moves[2][5], street_sim::story::EMPTY_MOVE_SLOT);
        assert_eq!(b.moves[2][10], 69);
        assert_eq!(b.unlocked, vec![69]);
        assert!(b.moves[0].is_empty());
    }

    #[test]
    fn imported_and_legacy_dunk_slots_keep_the_correct_native_script_body() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut disc =
            crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = crate::setup::load_clips(&mut disc.iso).unwrap();
        for (id, body) in [(47, 0x449f89), (59, 0x44a7b3)] {
            let row = disc
                .moves
                .iter()
                .find(|row| row.kind == 1 && clips.scripts.row_script(row.index) == Some(id))
                .unwrap()
                .index as u8;
            let mut bytes = vec![0; 0x130];
            for (start, count) in [(0xe2, 6), (0xf8, 11), (0x10e, 11)] {
                for slot in 0..count {
                    bytes[start + slot * 2..start + slot * 2 + 2]
                        .copy_from_slice(&(-1i16).to_le_bytes());
                }
            }
            bytes[0x102..0x104].copy_from_slice(&i16::from(row).to_le_bytes());
            let imported = baller_from_original(&bytes, 0);
            assert_eq!(imported.moves[1][5], row);
            let mut legacy: [Vec<u8>; 3] = Default::default();
            legacy[1] = vec![row];
            for equipment in [imported.moves, legacy] {
                let slots = crate::clip::native_move_slots(&disc, 1, &equipment[1]);
                let slot = slots.iter().position(|&equipped| equipped == row).unwrap();
                let moves = crate::clip::PlayerMoves(vec![(0, equipment)]);
                let actual = crate::clip::move_script_id(
                    &clips,
                    &moves,
                    &disc,
                    0,
                    street_assets::moves::Kind::Dunk,
                    slot,
                )
                .unwrap();
                assert_eq!(actual, id);
                assert_eq!(
                    clips
                        .scripts
                        .script(street_assets::moves::Kind::Dunk, usize::from(actual))
                        .unwrap()
                        .address,
                    body
                );
                // The renderer consumes that same direct id after import or
                // compact-slot migration; these formerly missing ids render.
                assert!(
                    crate::clip::script_selection(&clips, street_assets::moves::Kind::Dunk, actual)
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn original_camera_selector_has_five_choices_and_defaults_invalid_values() {
        for (value, expected) in [0, 1, 2, 3, 5, 0, 0, 0].into_iter().enumerate() {
            let mut bytes = [0; 24];
            bytes[..2].copy_from_slice(&((value as u16) << 6).to_le_bytes());
            assert_eq!(options_from_original(&bytes).camera, expected);
        }
    }
    #[test]
    fn original_option_bits_and_bracket_wins_are_mapped_independently() {
        let mut options = vec![0; 24];
        options[..2].copy_from_slice(&(2u16 | 4 | 8 | 32 | (4 << 6) | (2 << 12)).to_le_bytes());
        options[4..8]
            .copy_from_slice(&((1u32 << 2) | (8 << 20) | (6 << 24) | (10 << 28)).to_le_bytes());
        options[12..16].copy_from_slice(&(1u32 | (2 << 1) | (1 << 4)).to_le_bytes());
        let parsed = options_from_original(&options);
        assert_eq!(
            (parsed.difficulty, parsed.camera, parsed.replays),
            (2, 5, 2)
        );
        assert_eq!((parsed.effects, parsed.music, parsed.speech), (8, 6, 10));
        assert_eq!(parsed.layouts, [2, 1, 0, 0]);
        let mut raw = vec![0xff; 0x104];
        raw[..4].copy_from_slice(&[37, 0, 40, 0]);
        raw[12..16].copy_from_slice(&0x00000201u32.to_le_bytes());
        let mut bracket = Bracket::new(3, Some(40), 1);
        fill_bracket(&mut bracket, &raw);
        assert_eq!(bracket.rows[0].sides, [Entrant::You, Entrant::Boss(40)]);
        assert_eq!(bracket.rows[0].wins, [1, 2]);
    }

    /// The original's reset (`FUN_001d6f20`) leaves the traced block; an
    /// import of it is the rewrite's `Settings::default`.
    #[test]
    fn the_defaults_are_the_originals_reset_words() {
        let defaults = options_from_original(&street_sim::records::ORIGINAL_DEFAULT_BLOCK);
        assert_eq!(defaults, Settings::default());
        assert_eq!(
            (
                defaults.difficulty,
                defaults.music,
                defaults.audio,
                defaults.replays
            ),
            (0, 10, 1, 1)
        );
        assert!(!defaults.widescreen && !defaults.wide && defaults.help);
        assert_eq!(
            defaults.songs_off,
            1 << street_sim::records::RUCKER_CLASSIC_SONG_BIT
        );
    }

    #[test]
    fn original_trax_order_and_negative_rotation_flag_are_converted() {
        let mut options = [0; 24];
        options[..2].copy_from_slice(&16u16.to_le_bytes());
        // Only Nelly, the first interactive track, and React are enabled.
        options[8..12].copy_from_slice(&(4u32 | (1 << 3) | (1 << 12) | (1 << 26)).to_le_bytes());
        let parsed = options_from_original(&options);
        assert!(parsed.widescreen && parsed.sequential && parsed.track_info);
        assert!(!parsed.interactive);
        assert_eq!(parsed.songs_off & (1 << 20), 1 << 20); // Pete Rock disabled.
        assert_eq!(parsed.songs_off & (1 << 21), 0); // Nelly enabled.
        assert_eq!(parsed.songs_off & 1, 0); // Bleek enabled.
        assert_eq!(parsed.songs_off & (1 << 14), 0); // React enabled.
        assert_eq!(parsed.songs_off & (1 << 13), 1 << 13);
        options[8..12].copy_from_slice(&3u32.to_le_bytes());
        let parsed = options_from_original(&options);
        assert!(!parsed.sequential && !parsed.track_info);
        assert!(parsed.interactive);
    }

    #[test]
    fn supplied_gamecube_save_imports_known_fields_and_retains_original_container() {
        let (Some(iso), Some(save)) = (
            std::env::var_os("STREET_ISO"),
            std::env::var_os("STREET_GCI_SAVE"),
        ) else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(iso).unwrap()).unwrap();
        let bytes = std::fs::read(save).unwrap();
        let imported = import(&bytes, &disc).unwrap();
        assert_eq!(imported.original.to_bytes(), bytes);
        assert_eq!(imported.story.records.users[0].name, "QUENTIN");
        assert!(
            imported
                .story
                .records
                .users
                .iter()
                .all(|user| !user.school.payout_known)
        );
        assert_eq!(
            (
                imported.story.records.users[0].wins,
                imported.story.records.users[0].losses
            ),
            (105, 0)
        );
        assert_eq!(imported.story.ballers[0].first_name, "GILBERT");
        assert_eq!(imported.story.ballers[0].last_name, "CURRY");
        assert_eq!((imported.story.wins, imported.story.losses), (48, 0));
        assert_eq!(imported.story.star_points(), 1695);
        assert_eq!(imported.story.reputation, 55);
        assert_eq!(imported.story.street_cleared, 127);
        assert!(imported.story.settings.is_none());
        assert_eq!(
            Story::read(&imported.story.write()).unwrap(),
            imported.story
        );
        let directory =
            std::env::temp_dir().join(format!("street-gci-import-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let destination = directory.join("native.dat");
        imported.store_new(&destination).unwrap();
        assert_eq!(
            std::fs::read(directory.join("native.dat.original.gci")).unwrap(),
            bytes
        );
        assert!(imported.store_new(&destination).is_err());
        std::fs::remove_file(directory.join("native.dat.original.gci")).unwrap();
        std::fs::remove_file(destination).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn supplied_original_save_imports_and_native_format_reads_back() {
        let (Some(iso), Some(save)) = (
            std::env::var_os("STREET_ISO"),
            std::env::var_os("STREET_MAX_SAVE"),
        ) else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(iso).unwrap()).unwrap();
        let bytes = std::fs::read(save).unwrap();
        let imported = import(&bytes, &disc).unwrap();
        assert_eq!(imported.story.reward_points, 250);
        let raw = match &imported.original {
            Original::Ps2(save) => save.section(Section::Unlocks),
            _ => unreachable!(),
        };
        assert_eq!(imported.story.pickup_wins, word(raw, 4));
        let boss_flags = match &imported.original {
            Original::Ps2(save) => {
                (0..32)
                    .filter_map(|i| save.user_info(i))
                    .filter(|user| user.is_used())
                    .fold(0, |flags, user| flags | user.challenge_groups())
                    & 0b1_1111
            }
            _ => unreachable!(),
        };
        assert_eq!(
            imported
                .story
                .records
                .users
                .iter()
                .fold(0u32, |bits, user| bits | u32::from(user.league.cleared)),
            boss_flags
        );
        assert_eq!(imported.story.region_cleared, 0);
        assert_eq!(imported.story.league_played >> 40, 0);
        assert_eq!(imported.story.records.users[0].name, "ALLSTAR");
        assert_eq!(
            (
                imported.story.records.users[0].wins,
                imported.story.records.users[0].losses
            ),
            (33, 0)
        );
        assert_eq!(imported.story.crew, [234, 158, 56, 95, 150]);
        assert_eq!((imported.story.wins, imported.story.losses), (49, 2));
        assert_eq!(imported.story.reputation, 55);
        assert_eq!(imported.story.star_points(), 675);
        assert_eq!(
            imported.story.records.games[GameRecord::Dunks.index()].len(),
            10
        );
        assert_eq!(
            Story::read(&imported.story.write()).unwrap(),
            imported.story
        );
        // Exercise fields that this downloaded save's all-cleared mask hides.
        let mut fixture = Ps2Save::parse(&imported.original.to_bytes()).unwrap();
        let raw = fixture.section_mut(Section::Careers);
        raw[0x648] = 1; // Original first street challenge = Legendary Challenge.
        raw[0x64c..0x658].fill(0);
        raw[0x654..0x658].copy_from_slice(&(1u32 << (92 - 64)).to_le_bytes());
        raw[0x6d4 + 0xe2..0x6d4 + 0x124].fill(0xff); // No assigned move.
        let team = fixture.section_mut(Section::Teams);
        team.fill(0xff);
        team[2..0x62].fill(0);
        team[2..2 + 16].copy_from_slice(b"New York Legends");
        team[0x22..0x22 + 8].copy_from_slice(b"New York");
        team[0x42..0x42 + 7].copy_from_slice(b"Legends");
        for (i, player) in [0i16, 1, 2, 3, 4, 5].into_iter().enumerate() {
            team[0x62 + i * 2..0x64 + i * 2].copy_from_slice(&player.to_le_bytes());
        }
        team[0x6e..0x70].copy_from_slice(&2i16.to_le_bytes());
        let converted = import(&fixture.to_bytes(), &disc).unwrap();
        assert_eq!(converted.story.street_cleared, 1 << 6);
        assert_eq!(converted.story.ballers[0].unlocked, [92]);
        assert!(converted.story.ballers[0].moves.iter().all(Vec::is_empty));
        assert_eq!(converted.story.teams[0].name, "Legends");
        assert_eq!(converted.story.teams[0].city, "New York");
        assert_eq!(
            Story::read(&converted.story.write()).unwrap(),
            converted.story
        );
        assert_eq!(
            imported.original.to_bytes(),
            Ps2Save::read(&bytes).unwrap().to_bytes()
        );
    }

    /// The game data's sixth men (`+0x3c`, this section's `+0x24`): a
    /// career's baller (id 245 on) on a league team comes over as that
    /// baller's record; a database player there does not (the table
    /// holds created ballers only, `FUN_00271cb8`).
    #[test]
    fn original_sixth_men_import_as_the_ballers_records() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        for length in street_assets::save::SECTION_LENGTHS {
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
            bytes.resize(bytes.len() + length, 0);
        }
        let mut original = Ps2Save::parse(&bytes).unwrap();
        let globals = original.section_mut(Section::Unlocks);
        for team in 0..29 {
            globals[0x24 + team * 2..0x26 + team * 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
        globals[0x24 + 5 * 2..0x26 + 5 * 2].copy_from_slice(&245i16.to_le_bytes());
        globals[0x24 + 7 * 2..0x26 + 7 * 2].copy_from_slice(&3i16.to_le_bytes());
        let careers = original.section_mut(Section::Careers);
        careers[0x6d4 + 0x22..0x6d4 + 0x22 + 6].copy_from_slice(b"PLAYER");
        let imported = import(&original.to_bytes(), &disc).unwrap();
        let record = imported.story.ballers[0].record() as u16;
        assert_eq!(imported.story.sixth_men, [(5, record)]);
        assert_eq!(
            Story::read(&imported.story.write()).unwrap().sixth_men,
            [(5, record)]
        );
    }

    #[test]
    fn original_school_payloads_import_per_user_without_combining_results() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        for length in street_assets::save::SECTION_LENGTHS {
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
            bytes.resize(bytes.len() + length, 0);
        }
        let mut original = Ps2Save::parse(&bytes).unwrap();
        original.section_mut(Section::Unlocks)[0x18..0x1c].copy_from_slice(&1u32.to_le_bytes());
        let users = original.section_mut(Section::Users);
        let put_challenge = |bytes: &mut [u8], lesson: usize, value: u32| {
            let at = 0x40 + (4 - lesson / 8) * 4;
            let shift = (7 - lesson % 8) * 4;
            let packed = word(bytes, at) | value << shift;
            bytes[at..at + 4].copy_from_slice(&packed.to_le_bytes());
        };
        let first = &mut users[..0xbc];
        first[..1].copy_from_slice(b"A");
        first[0x3c..0x40].copy_from_slice(&0x80000001u32.to_le_bytes());
        put_challenge(first, 0, 12);
        put_challenge(first, 25, 8);
        put_challenge(first, 32, 15);
        first[0x54..0x58].copy_from_slice(&i32::MAX.to_le_bytes());
        first[0xb8..0xbc].copy_from_slice(&(-7i32).to_le_bytes());
        let second = &mut users[0xbc..0x178];
        second[..1].copy_from_slice(b"B");
        put_challenge(second, 1, 8);
        put_challenge(second, 26, 8);
        let mut imported = import(&original.to_bytes(), &disc).unwrap();
        assert_eq!(imported.story.school, 0);
        assert!(imported.story.school_trophy);
        imported.story.select_school_user(Some(0));
        assert_eq!(imported.story.school, 1 | 1 << 25);
        assert_eq!(imported.story.school_profile.flags, 0x80000001);
        assert_eq!(imported.story.school_profile.attempts[0], i32::MAX);
        assert_eq!(imported.story.school_profile.attempts[25], -7);
        assert_eq!(imported.story.school_profile.challenges[32], 15);
        assert!(imported.story.school_failed);
        let mut loaded = Story::read(&imported.story.write()).unwrap();
        loaded.select_school_user(Some(1));
        assert_eq!(loaded.school, 2);
        assert_eq!(loaded.school_profile.challenges[26], 8);
        assert!(!loaded.school_failed);
        assert!(loaded.trophies()[0]);
        assert_eq!(imported.original.to_bytes(), original.to_bytes());
    }

    #[test]
    fn original_league_ordinary_mask_and_region_boss_bits_import_independently() {
        let Some(iso) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(iso).unwrap()).unwrap();
        let mut bytes = Vec::new();
        for length in street_assets::save::SECTION_LENGTHS {
            bytes.extend_from_slice(&(length as u32).to_le_bytes());
            bytes.resize(bytes.len() + length, 0);
        }
        let mut original = Ps2Save::parse(&bytes).unwrap();
        let users = original.section_mut(Section::Users);
        users[..4].copy_from_slice(b"TEST");
        // Object +0x4c is ordinary wins; +0x50 independently marks bosses.
        users[0x34..0x38].copy_from_slice(&0x3fu32.to_le_bytes());
        users[0x38..0x3c].copy_from_slice(&4u32.to_le_bytes());
        users[0x30..0x32].copy_from_slice(&0xc0u16.to_le_bytes());
        users[0x32..0x34].copy_from_slice(&(-1i16).to_le_bytes());
        let second = &mut users[0xbc..0x178];
        second[..4].copy_from_slice(b"NEXT");
        second[0x30..0x32].copy_from_slice(&0x81u16.to_le_bytes());
        second[0x32..0x34].copy_from_slice(&7i16.to_le_bytes());
        second[0x34..0x38].copy_from_slice(&(1u32 << 6).to_le_bytes());
        second[0x38..0x3c].copy_from_slice(&8u32.to_le_bytes());
        let mut imported = import(&original.to_bytes(), &disc).unwrap();
        assert_eq!(imported.story.league_played, 0);
        assert!(imported.story.select_league_user(Some(0)));
        assert!(imported.story.league_boss_open(0));
        assert!(!imported.story.league_done(0, 6));
        assert!(imported.story.league_done(2, 6));
        assert_eq!(imported.story.region_cleared, 4);
        assert!(!imported.story.league_rewarded && !imported.story.complete_league());
        let mut reloaded = Story::read(&imported.story.write()).unwrap();
        assert!(reloaded.select_league_user(Some(0)));
        assert_eq!(reloaded.league_progress(), imported.story.league_progress());
        assert!(!reloaded.league_done(1, 0));
        reloaded.select_league_user(Some(1));
        assert_eq!(reloaded.league_user.as_deref(), Some("NEXT"));
        assert_eq!(reloaded.region_cleared, 8);
        assert_eq!(reloaded.league_native_flags, Some(0x81));
        assert_eq!(reloaded.league_lost, 2);
        assert_eq!(reloaded.league_team, Some(7));
        assert!(reloaded.league_done(1, 0));
        assert!(
            reloaded.league_done(0, 0),
            "own team is automatically counted"
        );
        assert!(
            !reloaded.league_done(0, 1),
            "no union with TEST's ordinary wins"
        );
        assert!(
            !reloaded.league_done(2, 6),
            "no union with TEST's boss bits"
        );
        assert_eq!(imported.original.to_bytes(), original.to_bytes());
    }

    #[test]
    fn gamecube_save_imports_options_and_scalars_losslessly() {
        let Some(iso) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(iso).unwrap()).unwrap();
        let mut bytes = vec![0u8; 64 + 7 * 8192];
        bytes[..4].copy_from_slice(b"GNZE");
        bytes[4..6].copy_from_slice(b"69");
        let name = &street_assets::save::GCI_FILENAME;
        bytes[8..8 + name.len()].copy_from_slice(name);
        bytes[0x38..0x3a].copy_from_slice(&7u16.to_be_bytes());

        const OFFSETS: [usize; 6] = [0x3200, 0x3600, 0x3800, 0x4400, 0x5c00, 0xda00];
        for (i, (&offset, &len)) in OFFSETS
            .iter()
            .zip(street_assets::save::SECTION_LENGTHS.iter())
            .enumerate()
        {
            let payload_at = 64 + offset;
            let footer = 0xdeadfadeu32 + len as u32;
            bytes[payload_at + len..payload_at + len + 4].copy_from_slice(&footer.to_be_bytes());
            if i == 0 {
                // Unlocks: reward_points = 5000, pickup_wins = 12
                bytes[payload_at..payload_at + 4].copy_from_slice(&5000u32.to_be_bytes());
                bytes[payload_at + 4..payload_at + 8].copy_from_slice(&12u32.to_be_bytes());
            } else if i == 1 {
                // Options: flags = 0x000c (difficulty 0, help, autosave), volumes = (10 << 24) | (8 << 20)
                let flags = 0x000cu16;
                bytes[payload_at..payload_at + 2].copy_from_slice(&flags.to_be_bytes());
                let volumes = (10u32 << 24) | (8u32 << 20);
                bytes[payload_at + 4..payload_at + 8].copy_from_slice(&volumes.to_be_bytes());
            }
        }

        let imported = import(&bytes, &disc).unwrap();
        assert_eq!(imported.story.reward_points, 5000);
        assert_eq!(imported.story.pickup_wins, 12);
        let settings = imported.story.settings.expect("GameCube options imported");
        assert!(settings.help);
        assert!(settings.autosave);
        assert_eq!(settings.music, 10);
        assert_eq!(settings.effects, 8);
    }

    #[test]
    fn all_ten_original_careers_import_and_restore_playable_state() {
        let Some(iso) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(iso).unwrap()).unwrap();
        let mut envelope = Vec::new();
        for length in street_assets::save::SECTION_LENGTHS {
            envelope.extend_from_slice(&(length as u32).to_le_bytes());
            envelope.resize(envelope.len() + length, 0);
        }
        let mut save = Ps2Save::parse(&envelope).unwrap();
        let careers_bytes = save.section_mut(Section::Careers);
        for i in 0..10 {
            let offset = i * 0xc88;
            // First/last name in player sub-block at 0x6d4
            let player_offset = offset + 0x6d4;
            let last = format!("LAST{i}");
            let first = format!("FIRST{i}");
            careers_bytes[player_offset + 2..player_offset + 2 + last.len()]
                .copy_from_slice(last.as_bytes());
            careers_bytes[player_offset + 0x22..player_offset + 0x22 + first.len()]
                .copy_from_slice(first.as_bytes());
            // Counter 0: games (10 + i), counter 1: wins (8 + i), counter 2: losses 2
            careers_bytes[offset + 0x0c..offset + 0x10]
                .copy_from_slice(&((10 + i) as u32).to_le_bytes());
            careers_bytes[offset + 0x10..offset + 0x14]
                .copy_from_slice(&((8 + i) as u32).to_le_bytes());
            careers_bytes[offset + 0x14..offset + 0x18].copy_from_slice(&2u32.to_le_bytes());
            // The crew's player ids at +0x6c6 (`Career::player_ids`): the
            // career's own star (245 + career), then four generic ballers.
            let star_id = 245 + i as i16;
            for (slot, &id) in [star_id, 158, 56, 95, 150].iter().enumerate() {
                let at = offset + 0x6c6 + slot * 2;
                careers_bytes[at..at + 2].copy_from_slice(&id.to_le_bytes());
            }
        }
        let imported = import(&save.to_bytes(), &disc).unwrap();
        let populated_careers = imported
            .story
            .careers
            .iter()
            .filter(|c| c.is_some())
            .count();
        assert_eq!(
            populated_careers, 10,
            "all ten original careers must be restored into playable story.careers"
        );
        for i in 0..10 {
            let c = imported.story.careers[i].as_ref().unwrap();
            assert_eq!(c.games, (10 + i) as u32);
            assert_eq!(c.wins, (8 + i) as u32);
            assert_eq!(c.losses, 2);
            assert_eq!(c.crew.len(), 5);
        }
        // G01: ten careers keep ten distinct stars, each with its own name,
        // before and after a save and reload onto a fresh disc.
        let check = |story: &Story, disc: &Disc| {
            let stars: Vec<u16> = story.careers.iter().flatten().map(|c| c.crew[0]).collect();
            let mut distinct = stars.clone();
            distinct.sort_unstable();
            distinct.dedup();
            assert_eq!(distinct.len(), 10, "stars {stars:?}");
            for (i, &star) in stars.iter().enumerate() {
                let p = &disc.players[usize::from(star)];
                assert_eq!(
                    (p.first_name.as_str(), p.last_name.as_str()),
                    (format!("FIRST{i}").as_str(), format!("LAST{i}").as_str())
                );
            }
        };
        let mut first = Disc::load(
            street_assets::iso::Iso::open(std::env::var_os("STREET_ISO").unwrap()).unwrap(),
        )
        .unwrap();
        crate::ballers::apply_all(&mut first, &imported.story);
        check(&imported.story, &first);
        let mut back = Story::read(&imported.story.write()).expect("reads back");
        let mut fresh = Disc::load(
            street_assets::iso::Iso::open(std::env::var_os("STREET_ISO").unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            back.sanitize(fresh.players.len(), fresh.teams.len(), fresh.moves.len()),
            0
        );
        crate::ballers::apply_all(&mut fresh, &back);
        check(&back, &fresh);
    }

    /// The C library's `qsort` sorts like any sort, and leaves equal rows
    /// where BSD's leaves them: the defaults' fifth table with its ties
    /// (3, 3, 2, 2, 2, 2, 1, ..) comes out in the save states' order.
    #[test]
    fn the_c_librarys_qsort_sorts_and_keeps_its_own_tie_order() {
        let mut seed = 7u32;
        for n in 0..60 {
            let mut v: Vec<i32> = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    (seed >> 16) as i32 % 9
                })
                .collect();
            let mut expect = v.clone();
            expect.sort_by(|a, b| b.cmp(a));
            c_qsort(&mut v, &|a, b| b - a);
            assert_eq!(v, expect, "n {n}");
        }
        // The defaults' fifth table (rows 35..44) as the executable lists it.
        let names = [
            "BIRD", "DIME", "PISTOL", "WILT", "OSMOSIS", "MAGIC", "ICEMAN", "JORDAN", "THEPEARL",
            "BONAFIDE",
        ];
        let values = [3, 3, 2, 2, 2, 2, 1, 1, 1, 1];
        let mut rows: Vec<(&str, i32)> = names.into_iter().zip(values).collect();
        c_qsort(&mut rows, &|a, b| b.1 - a.1);
        let order: Vec<&str> = rows.iter().map(|r| r.0).collect();
        assert_eq!(
            order,
            [
                "DIME", "BIRD", "WILT", "OSMOSIS", "MAGIC", "PISTOL", "ICEMAN", "JORDAN",
                "THEPEARL", "BONAFIDE"
            ]
        );
    }

    /// A fresh profile's high scores: the executable's tables sorted as the
    /// game sorts them, which is what every fresh save state holds byte for
    /// byte (STREET_ORIGINAL_SECTIONS: the decoded saves
    /// `tools/save_state_sections.py` cuts; one whose rows are only the
    /// defaults' rows must hold them in this order).
    #[test]
    fn the_fresh_high_scores_are_the_executables_tables_sorted_as_the_game_sorts() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(&path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let scores = default_scores(&elf).unwrap();
        assert_eq!(scores.len(), 0xa64);
        let name = |row: usize| text(&scores[row * 28..row * 28 + 9]);
        assert_eq!(
            (name(0), name(1), name(15)),
            ("DR".into(), "BOBBITO".into(), "BIGGIE L".into())
        );
        let records = default_records(&elf).unwrap();
        assert_eq!(records.legends.len(), 5);
        assert_eq!(
            records.games[GameRecord::TrickPoints.index()][0].value,
            500_000
        );
        let Ok(list) = std::env::var("STREET_ORIGINAL_SECTIONS") else {
            return;
        };
        let mut fresh = 0;
        for file in list.split(';').filter(|f| !f.is_empty()) {
            let save = Ps2Save::parse(&std::fs::read(file).unwrap()).unwrap();
            let held = save.section(Section::HighScores);
            let mut a: Vec<&[u8]> = held.chunks(28).collect();
            let mut b: Vec<&[u8]> = scores.chunks(28).collect();
            a.sort();
            b.sort();
            if a == b {
                assert_eq!(held, &scores[..], "{file}");
                fresh += 1;
            }
        }
        assert!(fresh > 0, "a fresh profile among {list}");
    }

    /// The original's own saves, cut from the save states' RAM
    /// (STREET_ORIGINAL_SECTIONS), through the import and the native save:
    /// the users the original made (User Setup's "PLAYER1"), its options,
    /// its owned cards and its high scores come in; a fresh original
    /// profile matches a fresh rewrite one card for card and table for
    /// table; and the native save reads back to the same story.
    #[test]
    fn the_originals_own_save_round_trips_through_the_native_one() {
        let (Some(iso), Ok(list)) = (
            std::env::var_os("STREET_ISO"),
            std::env::var("STREET_ORIGINAL_SECTIONS"),
        ) else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(&iso).unwrap()).unwrap();
        let elf = street_assets::iso::Iso::open(&iso)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let defaults = default_scores(&elf).unwrap();
        let fresh_rewrite = Story::default();
        for file in list.split(';').filter(|f| !f.is_empty()) {
            let bytes = std::fs::read(file).unwrap();
            let save = Ps2Save::parse(&bytes).unwrap();
            let imported = import(&bytes, &disc).unwrap();
            let story = &imported.story;
            // Every user the original holds, by name.
            let names: Vec<String> = (0..32)
                .filter_map(|i| save.user_info(i))
                .filter(|u| u.is_used() && !u.name().trim().is_empty())
                .map(|u| u.name())
                .collect();
            let imported_names: Vec<String> =
                story.records.users.iter().map(|u| u.name.clone()).collect();
            assert_eq!(imported_names, names, "{file}");
            // A fresh profile: cards and careers as a fresh rewrite save.
            if save.section(Section::HighScores) == &defaults[..] {
                for card in &disc.cards {
                    assert_eq!(
                        crate::rewards::card_owned(story, card),
                        crate::rewards::card_owned(&fresh_rewrite, card),
                        "{file}: card {}",
                        card.id
                    );
                }
                assert_eq!(story.records.games, default_records(&elf).unwrap().games);
                assert!(story.careers.iter().all(Option::is_none));
                assert_eq!(story.reward_points, 0);
                // The options are the reset's (`FUN_001d7348(opts, 0)`), the
                // rewrite's defaults; the attract boot's difficulty reads 2.
                let reset = Settings {
                    difficulty: story.settings.map_or(0, |s| s.difficulty),
                    ..Default::default()
                };
                assert_eq!(story.settings, Some(reset), "{file}");
            }
            // The native save keeps all of it.
            let back = Story::read(&story.write()).expect("reads back");
            assert_eq!(back.records, story.records, "{file}");
            assert_eq!(back.settings, story.settings, "{file}");
            assert_eq!(back.owned, story.owned, "{file}");
        }
    }

    /// The wardrobe's words as the executable reads and writes them
    /// (`FUN_001e7680` 0x1e76e8/0x1e7728, `FUN_001e7758` 0x1e779c,
    /// `FUN_001e7940` 0x1e796c; the free row's colour 50 at 0x1e4618 in
    /// `FUN_001e4580`), imported from a career laid out by them: object
    /// +0x670 is the serialized +0x658.
    #[test]
    fn an_original_careers_wardrobe_imports_from_the_executables_words() {
        use street_assets::scripts::executable_words;
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(&path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let low = |a: u32| (executable_words(&elf, a, 1).unwrap()[0] & 0xffff) as usize;
        let offsets = [low(0x1e76e8), low(0x1e7728), low(0x1e779c), low(0x1e796c)];
        assert_eq!(offsets, [0x670, 0x674, 0x678, 0x678]);
        assert_eq!(
            low(0x1e4618) as u32,
            street_sim::cab::FREE_OUTFIT_COLOUR,
            "addiu s3, zero, 0x32"
        );
        let disc = Disc::load(street_assets::iso::Iso::open(&path).unwrap()).unwrap();
        let mut envelope = Vec::new();
        for length in street_assets::save::SECTION_LENGTHS {
            envelope.extend_from_slice(&(length as u32).to_le_bytes());
            envelope.resize(envelope.len() + length, 0);
        }
        let mut save = Ps2Save::parse(&envelope).unwrap();
        let career = &mut save.section_mut(Section::Careers)[..0xc88];
        career[0x6d4 + 2..0x6d4 + 6].copy_from_slice(b"LAST");
        // Outfit rows 0 and 7, row 7 in colour 2, shoe rows 0, 10 and 14.
        let base = 0x18;
        for (offset, word) in [
            (0x670, 0b1000_0001u32),
            (0x674, 1 << (7 * 3 + 2) | 1),
            (0x678, 1 | 1 << 10 | 1 << 14),
        ] {
            career[offset - base..offset - base + 4].copy_from_slice(&word.to_le_bytes());
        }
        let imported = import(&save.to_bytes(), &disc).unwrap();
        let w = imported.story.ballers[0].wardrobe;
        assert!(w.owns_outfit(7, 2) && w.owns_outfit(0, 0) && !w.owns_outfit(7, 0));
        assert!(w.owns_shoes(10) && w.owns_shoes(14) && !w.owns_shoes(2));
        // The native save keeps it.
        let back = Story::read(&imported.story.write()).unwrap();
        assert_eq!(back.ballers[0].wardrobe, w);
    }

    /// The save's layout as the executable builds it: the six sections'
    /// constructors, the career's payload base, its counters, its created
    /// player block and move slots, and the nickname-pending bit; then a
    /// save laid out by those offsets through `import` and back through
    /// the rewrite's own save.
    #[test]
    fn native_save_layout_is_the_executables() {
        use street_assets::scripts::{executable_constant, executable_words};
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let elf = street_assets::iso::Iso::open(&path)
            .unwrap()
            .read("/SLUS_206.51")
            .unwrap();
        let c = |a: u32| executable_constant(&elf, a).unwrap() as usize;
        let low = |a: u32| (executable_words(&elf, a, 1).unwrap()[0] & 0xffff) as usize;
        // Section constructors: 0x1dadb4, 0x1d6e94, 0x1d3d0c (one block
        // each); 0x1ef02c × 0x1ef034, 0x1eb194 × 0x1eb19c, 0x1edd44 ×
        // 0x1edd4c (records × count).
        let lengths = [
            c(0x1dadb4),
            c(0x1d6e94),
            c(0x1d3d0c),
            c(0x1ef02c) * c(0x1ef034),
            c(0x1eb194) * c(0x1eb19c),
            c(0x1edd44) * c(0x1edd4c),
        ];
        assert_eq!(lengths, street_assets::save::SECTION_LENGTHS);
        assert_eq!(
            street_assets::save::PS2_SAVE_LENGTH,
            lengths.iter().sum::<usize>() + 6 * 4
        );
        // The career (0x1e693c: 0xc88 bytes from object +0x18, 0x1e6958);
        // its counters at +0x24 (`FUN_001e70a8`, 0x1e70b8); the created
        // player block of team slot 0 at +0x6ec (0x1e7e58) and the team's
        // ids at +0x6de (0x1e7eb4); the move slots at block +0xe2, +0xf8,
        // +0x10e (0x1d8a70, 0x1d8a74, 0x1d8a64) for 6, 11 and 11 slots
        // (0x1d87c0, 0x1d87d4, 0x1d87e8); the pending bit, profile +0x19
        // bit 0 (set 0x1e7394..0x1e739c, cleared 0x1a854c..0x1a8560).
        assert_eq!(c(0x1e693c), 0xc88);
        let base = low(0x1e6958);
        let games = low(0x1e70b8) - base;
        let block = low(0x1e7e58) - base;
        let ids = low(0x1e7eb4) - base;
        let slots = [low(0x1d8a70), low(0x1d8a74), low(0x1d8a64)];
        let counts = [low(0x1d87c0), low(0x1d87d4), low(0x1d87e8)];
        assert_eq!((base, games, block, ids), (0x18, 0x0c, 0x6d4, 0x6c6));
        assert_eq!((slots, counts), ([0xe2, 0xf8, 0x10e], [6, 11, 11]));
        assert_eq!(low(0x1e7394), 0x19);
        assert_eq!(low(0x1a854c), 0x19);
        assert_eq!(low(0x1a8558), 0xfe);
        let pending = 0x19 - base;

        let disc = Disc::load(street_assets::iso::Iso::open(&path).unwrap()).unwrap();
        let mut envelope = Vec::new();
        for length in lengths {
            envelope.extend_from_slice(&(length as u32).to_le_bytes());
            envelope.resize(envelope.len() + length, 0);
        }
        let mut save = Ps2Save::parse(&envelope).unwrap();
        let career = &mut save.section_mut(Section::Careers)[..0xc88];
        career[games..games + 4].copy_from_slice(&7u32.to_le_bytes());
        career[pending] |= 1;
        let player = block;
        career[player + 2..player + 6].copy_from_slice(b"LAST");
        career[player + 0x22..player + 0x27].copy_from_slice(b"FIRST");
        for (start, count) in slots.into_iter().zip(counts) {
            for slot in 0..count {
                let at = player + start + slot * 2;
                career[at..at + 2].copy_from_slice(&(-1i16).to_le_bytes());
            }
        }
        // Dribble slot 3 holds row 40 behind two holes, dunk slot 10 row 7.
        let at = player + slots[2] + 3 * 2;
        career[at..at + 2].copy_from_slice(&40i16.to_le_bytes());
        let at = player + slots[1] + 10 * 2;
        career[at..at + 2].copy_from_slice(&7i16.to_le_bytes());
        for (slot, &id) in [245i16, 158, 56].iter().enumerate() {
            let at = ids + slot * 2;
            career[at..at + 2].copy_from_slice(&id.to_le_bytes());
        }
        let bytes = save.to_bytes();
        assert_eq!(Ps2Save::parse(&bytes).unwrap().to_bytes(), bytes);
        let imported = import(&bytes, &disc).unwrap();
        let career = imported.story.careers[0].as_ref().unwrap();
        assert_eq!(career.games, 7);
        assert!(career.nickname_pending);
        let baller = &imported.story.ballers[0];
        let empty = street_sim::story::EMPTY_MOVE_SLOT;
        assert_eq!(baller.moves[2][..4], [empty, empty, empty, 40]);
        assert_eq!(baller.moves[1][10], 7);
        assert!(baller.moves[0].is_empty());
        // The rewrite's save keeps both.
        let back = Story::read(&imported.story.write()).unwrap();
        assert!(back.careers[0].as_ref().unwrap().nickname_pending);
        assert_eq!(back.ballers[0].moves, baller.moves);
    }
}
