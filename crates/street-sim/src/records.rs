//! Users, records and High Scores, and the saved options: what the game
//! keeps between sessions besides the story.
//!
//! - **Users** (`entername.txt`, "User Setup"): named people with a
//!   won-lost record (`%1dW - %2dL`, `WIN%`) and career totals: points,
//!   trick points, dunks, steals, blocks, gamebreakers. A name is picked or
//!   entered per controller when sides are chosen ("Choose sides and enter
//!   a name to track records."); names are unique ("The name already
//!   exists") and limited in number ("Maximum number of names has been
//!   reached").
//! - **High Scores** (`score.txt`, the game's ten tables at game data
//!   `+0x144`, `FUN_001d3d98`): Street Legends (five Be A Legend finishes by
//!   their won share), Win Percentage (ten user names with ten games or
//!   more, by their won share) and eight single-game tables of ten: trick
//!   points, level 1 and level 2 gamebreakers, alley-oops, blocks, steals,
//!   long range shots and dunks. A fresh profile starts with the
//!   executable's rows (`0x45e2d0`). The rewrite also keeps a pass outs
//!   table, which the original's screen never shows (`score.txt` 1 and 11,
//!   Fastest Rise To Fame and Pass Outs, are read by no code).

/// Most user names kept (the game's own limit is not traced).
pub const USER_SLOTS: usize = 32;
/// Rows in each single-game table and in the Street Legends table
/// (`DAT_0045e2a8`: 5, then nine of 10).
pub const TABLE_ROWS: usize = 10;
pub const LEGEND_ROWS: usize = 5;
/// Rows in the Win Percentage table.
pub const WIN_ROWS: usize = 10;
/// Games a name needs before it can enter Win Percentage (`FUN_001d4070`:
/// a record of nine games or fewer is never placed).
pub const WIN_GAMES: u32 = 10;
/// Most Street Legends rows a save may hold (an older save's finishes
/// list, before the table was the original's five).
const LEGENDS_KEPT: usize = 50;
/// The original keeps names of eight letters (`strncmp(.., 8)` when Win
/// Percentage looks a name up).
const NAME_KEY: usize = 8;

/// A user's career totals (`entername.txt` 15..21; the game's "Busted
/// Rims" is not counted here).
pub const CAREER: [&str; 6] = [
    "Points",
    "Trick Points",
    "Dunks",
    "Steals",
    "Blocks",
    "Gamebreakers",
];

/// The single-game records, in `score.txt`'s order (3..11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameRecord {
    TrickPoints,
    Gamebreakers1,
    Gamebreakers2,
    Steals,
    Blocks,
    AlleyOops,
    LongRange,
    Dunks,
    PassOuts,
}

impl GameRecord {
    pub const ALL: [GameRecord; 9] = [
        GameRecord::TrickPoints,
        GameRecord::Gamebreakers1,
        GameRecord::Gamebreakers2,
        GameRecord::Steals,
        GameRecord::Blocks,
        GameRecord::AlleyOops,
        GameRecord::LongRange,
        GameRecord::Dunks,
        GameRecord::PassOuts,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// League Challenge belongs to a named user (native user object +0x48..+0x50).
/// Rewards and the lifetime trophy belong to the shared game record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeagueProgress {
    pub played: u64,
    pub cleared: u8,
    pub lost: u8,
    /// Native +0x4a: selected team, automatically counted as beaten.
    pub team: Option<u16>,
    /// Original +0x48 low halfword, retained when importing a console profile.
    pub native_flags: Option<u16>,
    /// Rewrite's selected three-player lineup, independent for each name.
    pub roster: Vec<u16>,
}

/// Original user +0x54 paid flag, +0x58 five packed nibble words,
/// and +0x6c twenty-six signed lesson launch counters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchoolProgress {
    pub flags: u32,
    /// False for an original format whose compiler-paid flag is unmapped.
    pub payout_known: bool,
    pub challenges: [u8; 33],
    pub attempts: [i32; 26],
}

impl Default for SchoolProgress {
    fn default() -> Self {
        Self {
            flags: 0,
            payout_known: true,
            challenges: [0; 33],
            attempts: [0; 26],
        }
    }
}

impl SchoolProgress {
    pub fn completed(&self) -> u32 {
        self.challenges[..26]
            .iter()
            .enumerate()
            .fold(0, |bits, (lesson, &value)| {
                bits | (u32::from(value & 8 != 0) << lesson)
            })
    }

    pub fn failed(&self) -> bool {
        self.challenges[..26].iter().any(|&value| value & 7 != 0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub wins: u32,
    pub losses: u32,
    pub career: [u32; CAREER.len()],
    pub league: LeagueProgress,
    pub school: SchoolProgress,
}

impl User {
    /// Wins as a whole percentage of games (0 with none played).
    pub fn percent(&self) -> u32 {
        let games = self.wins + self.losses;
        (self.wins * 100 + games / 2)
            .checked_div(games)
            .unwrap_or(0)
    }
}

/// One row of a single-game table: who did it, for which user (empty for
/// none), and how many.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub user: String,
    pub value: u32,
}

/// A Street Legends row (`FUN_001d4438`'s mode-5 row): the star's first
/// and last name, the nickname the career earned (empty for none; the
/// table shows it in quotes between the names) and the career's wins and
/// losses (`+0xf38`, `+0xf3c`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Legend {
    pub name: String,
    pub nickname: String,
    pub wins: u32,
    pub losses: u32,
}

impl Legend {
    pub fn games(&self) -> u32 {
        self.wins.saturating_add(self.losses)
    }
}

/// A Win Percentage row: a user name and its record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WinRow {
    pub name: String,
    pub wins: u32,
    pub losses: u32,
}

/// The won share the two record tables sort and place by (`FUN_001d4070`,
/// the sort's 0x1d47b8): `cvt.s.w` of each figure (halved and doubled when
/// it reads negative, as the unsigned conversion goes), wins over games,
/// times 1000, truncated by `cvt.w.s`.
pub fn share(wins: u32, losses: u32) -> i32 {
    let float = |n: u32| {
        if (n as i32) < 0 {
            ((n >> 1 | (n & 1)) as i32 as f32) * 2.0
        } else {
            n as i32 as f32
        }
    };
    let games = wins.wrapping_add(losses);
    if games == 0 {
        // The EE's 0/0 is its largest float, which `cvt.w.s` saturates.
        return if wins == 0 { i32::MAX } else { 0 };
    }
    (float(wins) / float(games) * 1000.0) as i32
}

/// The C library's `qsort` (`FUN_003b3868`, BSD's: insertion sort below 7,
/// the median of three (of nine above 40), Bentley and McIlroy's split
/// around the pivot's equals, recursion on the smaller side), element for
/// element, so equal rows end where the original's end.
pub fn c_qsort<T>(a: &mut [T], cmp: &impl Fn(&T, &T) -> i32) {
    fn insertion<T>(a: &mut [T], cmp: &impl Fn(&T, &T) -> i32) {
        for m in 1..a.len() {
            let mut l = m;
            while l > 0 && cmp(&a[l - 1], &a[l]) > 0 {
                a.swap(l, l - 1);
                l -= 1;
            }
        }
    }
    fn med3<T>(a: &[T], i: usize, j: usize, k: usize, cmp: &impl Fn(&T, &T) -> i32) -> usize {
        if cmp(&a[i], &a[j]) < 0 {
            if cmp(&a[j], &a[k]) < 0 {
                j
            } else if cmp(&a[i], &a[k]) < 0 {
                k
            } else {
                i
            }
        } else if cmp(&a[j], &a[k]) > 0 {
            j
        } else if cmp(&a[i], &a[k]) < 0 {
            i
        } else {
            k
        }
    }
    let mut a = a;
    loop {
        let n = a.len();
        if n < 7 {
            insertion(a, cmp);
            return;
        }
        let mut pm = n / 2;
        if n > 7 {
            let (mut pl, mut pn) = (0, n - 1);
            if n > 40 {
                let d = n / 8;
                pl = med3(a, pl, pl + d, pl + 2 * d, cmp);
                pm = med3(a, pm - d, pm, pm + d, cmp);
                pn = med3(a, pn - 2 * d, pn - d, pn, cmp);
            }
            pm = med3(a, pl, pm, pn, cmp);
        }
        a.swap(0, pm);
        let (mut pa, mut pb) = (1, 1);
        let (mut pc, mut pd) = (n as isize - 1, n as isize - 1);
        let mut swapped = false;
        loop {
            while pb as isize <= pc {
                let r = cmp(&a[pb], &a[0]);
                if r > 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    a.swap(pa, pb);
                    pa += 1;
                }
                pb += 1;
            }
            while pb as isize <= pc {
                let r = cmp(&a[pc as usize], &a[0]);
                if r < 0 {
                    break;
                }
                if r == 0 {
                    swapped = true;
                    a.swap(pc as usize, pd as usize);
                    pd -= 1;
                }
                pc -= 1;
            }
            if pb as isize > pc {
                break;
            }
            a.swap(pb, pc as usize);
            swapped = true;
            pb += 1;
            pc -= 1;
        }
        if !swapped {
            insertion(a, cmp);
            return;
        }
        let r = pa.min(pb - pa);
        for k in 0..r {
            a.swap(k, pb - r + k);
        }
        let (pc, pd) = (pc as usize, pd as usize);
        let r = (pd - pc).min(n - pd - 1);
        for k in 0..r {
            a.swap(pb + k, n - r + k);
        }
        let left = pb - pa;
        if left > 1 {
            c_qsort(&mut a[..left], cmp);
        }
        let right = pd - pc;
        if right > 1 {
            let rest = std::mem::take(&mut a);
            a = &mut rest[n - right..];
            continue;
        }
        return;
    }
}

/// Whether two names are the same row's (`strncmp(a, b, 8)`).
fn same_name(a: &str, b: &str) -> bool {
    a.bytes().take(NAME_KEY).eq(b.bytes().take(NAME_KEY))
}

/// Why a name can't be added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// "No Name Entered".
    Empty,
    /// "The name already exists."
    Exists,
    /// "Maximum number of names has been reached."
    Full,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    pub users: Vec<User>,
    /// The single-game tables, by [`GameRecord`], best first.
    pub games: [Vec<Row>; 9],
    /// Street Legends, the best won share first.
    pub legends: Vec<Legend>,
    /// Win Percentage, the best won share first.
    pub win_rows: Vec<WinRow>,
}

impl Records {
    /// The user called `name` (ignoring case).
    pub fn user(&self, name: &str) -> Option<usize> {
        self.users
            .iter()
            .position(|u| u.name.eq_ignore_ascii_case(name.trim()))
    }

    /// A new user; their index.
    pub fn add_user(&mut self, name: &str) -> Result<usize, NameError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(NameError::Empty);
        }
        if self.user(name).is_some() {
            return Err(NameError::Exists);
        }
        if self.users.len() >= USER_SLOTS {
            return Err(NameError::Full);
        }
        self.users.push(User {
            name: name.to_owned(),
            ..Default::default()
        });
        Ok(self.users.len() - 1)
    }

    /// Forgets a user and their record ("All information for this user
    /// name will be lost.").
    pub fn delete_user(&mut self, index: usize) {
        if index < self.users.len() {
            self.users.remove(index);
        }
    }

    /// A game played by user `index`: the result and what they added.
    pub fn played(&mut self, index: usize, won: bool, career: [u32; CAREER.len()]) {
        if let Some(u) = self.users.get_mut(index) {
            if won {
                u.wins += 1;
            } else {
                u.losses += 1;
            }
            for (total, add) in u.career.iter_mut().zip(career) {
                *total = total.saturating_add(add);
            }
        }
    }

    /// Offers a row to a single-game table (`FUN_001d4070` for tables 2..9,
    /// then `FUN_001d3f90`): it goes in above the first row whose figure is
    /// no more than its own, so a tie puts the newer row first, and the
    /// last row drops off. Its place (0 the best) if it made the table.
    pub fn offer(&mut self, table: GameRecord, row: Row) -> Option<usize> {
        let rows = &mut self.games[table.index()];
        let at = rows
            .iter()
            .position(|r| r.value <= row.value)
            .or((rows.len() < TABLE_ROWS).then_some(rows.len()))?;
        rows.insert(at, row);
        rows.truncate(TABLE_ROWS);
        Some(at)
    }

    /// Offers a user's record to Win Percentage (`FUN_001d4070` table 1):
    /// a name already in the table has its row replaced, and the table
    /// sorted again by the C library's `qsort` (`FUN_001d43a0`), unless its
    /// wins are no more than the row's and it has played under ten games;
    /// a new name needs ten games and goes in above the first row whose
    /// share is no more than its own. Its place if it made the table.
    pub fn offer_win(&mut self, name: &str, wins: u32, losses: u32) -> Option<usize> {
        let games = wins.saturating_add(losses);
        let row = WinRow {
            name: name.to_owned(),
            wins,
            losses,
        };
        if let Some(at) = self.win_rows.iter().position(|r| same_name(&r.name, name)) {
            if wins <= self.win_rows[at].wins && games < WIN_GAMES {
                return None;
            }
            self.win_rows[at] = row;
            c_qsort(&mut self.win_rows, &|a: &WinRow, b: &WinRow| {
                share(b.wins, b.losses).wrapping_sub(share(a.wins, a.losses))
            });
            return Some(at);
        }
        if games < WIN_GAMES {
            return None;
        }
        let mine = share(wins, losses);
        let at = self
            .win_rows
            .iter()
            .position(|r| share(r.wins, r.losses) <= mine)
            .or((self.win_rows.len() < WIN_ROWS).then_some(self.win_rows.len()))?;
        self.win_rows.insert(at, row);
        self.win_rows.truncate(WIN_ROWS);
        Some(at)
    }

    /// A finish of Be A Legend offered to Street Legends (`FUN_001d4070`
    /// table 0): above the first row whose share is no more than its own.
    pub fn legend(&mut self, legend: Legend) -> Option<usize> {
        let mine = share(legend.wins, legend.losses);
        let at = self
            .legends
            .iter()
            .position(|r| share(r.wins, r.losses) <= mine)
            .or((self.legends.len() < LEGEND_ROWS).then_some(self.legends.len()))?;
        self.legends.insert(at, legend);
        self.legends.truncate(LEGEND_ROWS);
        Some(at)
    }

    /// The Street Legends table as the screen shows it.
    pub fn first_legends(&self) -> &[Legend] {
        &self.legends[..self.legends.len().min(LEGEND_ROWS)]
    }

    // -- persistence -------------------------------------------------------

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(self.users.len() as u32);
        for u in &self.users {
            w.name(&u.name);
            w.u32(u.wins);
            w.u32(u.losses);
            w.u8(u.career.len() as u8);
            for &c in &u.career {
                w.u32(c);
            }
        }
        w.u8(self.games.len() as u8);
        for rows in &self.games {
            w.u8(rows.len() as u8);
            for r in rows {
                w.name(&r.name);
                w.name(&r.user);
                w.u32(r.value);
            }
        }
        w.u32(self.legends.len() as u32);
        for l in &self.legends {
            w.name(&l.name);
            w.u32(l.games());
        }
        // Appended so pre-profile records still decode with fresh league runs.
        w.0.extend_from_slice(&LEAGUE_USERS);
        w.u8(1);
        w.u8(self.users.len() as u8);
        for user in &self.users {
            let league = &user.league;
            w.u32(league.played as u32);
            w.u32((league.played >> 32) as u32);
            w.u8(league.cleared);
            w.u8(league.lost);
            w.u32(league.team.map_or(u32::MAX, u32::from));
            w.u32(league.native_flags.map_or(u32::MAX, u32::from));
            w.u8(league.roster.len() as u8);
            for &record in &league.roster {
                w.u32(u32::from(record));
            }
        }
        w.0.extend_from_slice(b"SCHU");
        w.u8(1);
        w.u8(self.users.len() as u8);
        for user in &self.users {
            w.u32(user.school.flags);
            w.u8(u8::from(user.school.payout_known));
            w.0.extend_from_slice(&user.school.challenges);
            for &attempts in &user.school.attempts {
                w.u32(attempts as u32);
            }
        }
        // Street Legends' records and nicknames, and Win Percentage
        // (appended: older saves read without them).
        w.0.extend_from_slice(b"LGWL");
        w.u8(1);
        w.u8(self.users.len() as u8);
        w.u32(self.legends.len() as u32);
        for l in &self.legends {
            w.u32(l.wins);
            w.u32(l.losses);
            w.name(&l.nickname);
        }
        w.0.extend_from_slice(b"WINT");
        w.u8(1);
        w.u8(self.users.len() as u8);
        w.u8(self.win_rows.len() as u8);
        for row in &self.win_rows {
            w.name(&row.name);
            w.u32(row.wins);
            w.u32(row.losses);
        }
        w.0
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader(bytes);
        let mut out = Records::default();
        let users = r.u32()? as usize;
        if users > USER_SLOTS {
            return None;
        }
        for _ in 0..users {
            let name = r.name()?;
            let (wins, losses) = (r.u32()?, r.u32()?);
            let n = usize::from(r.u8()?);
            let mut career = [0u32; CAREER.len()];
            for k in 0..n {
                let v = r.u32()?;
                if let Some(c) = career.get_mut(k) {
                    *c = v;
                }
            }
            out.users.push(User {
                name,
                wins,
                losses,
                career,
                league: LeagueProgress::default(),
                school: SchoolProgress::default(),
            });
        }
        let tables = usize::from(r.u8()?);
        for t in 0..tables {
            let n = usize::from(r.u8()?);
            for _ in 0..n {
                let row = Row {
                    name: r.name()?,
                    user: r.name()?,
                    value: r.u32()?,
                };
                if let Some(rows) = out.games.get_mut(t)
                    && rows.len() < TABLE_ROWS
                {
                    rows.push(row);
                }
            }
        }
        let legends = r.u32()? as usize;
        if legends > LEGENDS_KEPT {
            return None;
        }
        for _ in 0..legends {
            // A save from before the records' split: its games as wins.
            out.legends.push(Legend {
                name: r.name()?,
                wins: r.u32()?,
                ..Default::default()
            });
        }
        let (mut league_profiles, mut school_profiles) = (false, false);
        let (mut legend_records, mut win_table) = (false, false);
        while !r.0.is_empty() {
            let tag: [u8; 4] = r.take(4)?.try_into().ok()?;
            if r.u8()? != 1 || usize::from(r.u8()?) != users {
                return None;
            }
            match &tag {
                t if (*t == LEAGUE_USERS || *t == LEGACY_LEAGUE_USERS) && !league_profiles => {
                    league_profiles = true;
                    for user in &mut out.users {
                        let played = u64::from(r.u32()?) | (u64::from(r.u32()?) << 32);
                        let cleared = r.u8()?;
                        let lost = r.u8()?;
                        let team = r.u32()?;
                        let flags = r.u32()?;
                        let count = usize::from(r.u8()?);
                        if count != 0 && count != 3 {
                            return None;
                        }
                        let mut roster = Vec::with_capacity(count);
                        for _ in 0..count {
                            roster.push(u16::try_from(r.u32()?).ok()?);
                        }
                        user.league = LeagueProgress {
                            played,
                            cleared,
                            lost,
                            team: if team == u32::MAX {
                                None
                            } else {
                                Some(u16::try_from(team).ok()?)
                            },
                            native_flags: if flags == u32::MAX {
                                None
                            } else {
                                Some(u16::try_from(flags).ok()?)
                            },
                            roster,
                        };
                    }
                }
                b"SCHU" if !school_profiles => {
                    school_profiles = true;
                    for user in &mut out.users {
                        user.school.flags = r.u32()?;
                        let known = r.u8()?;
                        if known > 1 {
                            return None;
                        }
                        user.school.payout_known = known != 0;
                        let values = r.take(33)?;
                        if values.iter().any(|&value| value > 15) {
                            return None;
                        }
                        user.school.challenges.copy_from_slice(values);
                        for attempts in &mut user.school.attempts {
                            *attempts = r.u32()? as i32;
                        }
                    }
                }
                b"LGWL" if !legend_records => {
                    legend_records = true;
                    if r.u32()? as usize != out.legends.len() {
                        return None;
                    }
                    for l in &mut out.legends {
                        l.wins = r.u32()?;
                        l.losses = r.u32()?;
                        l.nickname = r.name()?;
                    }
                }
                b"WINT" if !win_table => {
                    win_table = true;
                    let n = usize::from(r.u8()?);
                    if n > WIN_ROWS {
                        return None;
                    }
                    for _ in 0..n {
                        out.win_rows.push(WinRow {
                            name: r.name()?,
                            wins: r.u32()?,
                            losses: r.u32()?,
                        });
                    }
                }
                _ => return None,
            }
        }
        Some(out)
    }
}

/// The options kept between runs; original imports use the traced `GAMEDATA` fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Volumes 0..10.
    pub music: u8,
    pub effects: u8,
    pub speech: u8,
    pub vibration: bool,
    /// 0 easy, 1 medium, 2 hard.
    pub difficulty: u8,
    /// Replays (`options.txt` 47..49): 0 Always, 1 Automatic, 2 Manual.
    pub replays: u8,
    /// Audio Options: 0 Mono, 1 Stereo, 2 Dolby Pro Logic II.
    pub audio: u8,
    /// Onscreen Help: the in-game button prompts.
    pub help: bool,
    /// Autosave: progress and changes saved as they happen.
    pub autosave: bool,
    /// Widescreen (`FUN_001d7348`, bit 4): the PS2's anamorphic 16:9. The
    /// renderer (`FUN_00323450`) keeps the 4:3 picture's horizontal angle
    /// and projects it at 16:9, for a television that stretches the frame.
    pub widescreen: bool,
    /// The rewrite's own Wide picture, with Widescreen on. The original's
    /// renderer has a second flag (`wrapper+0x10`) that nothing in the game
    /// sets; with both set, `FUN_00323450`'s third branch keeps the 4:3
    /// picture's vertical angle and widens the horizontal by 4/3, so a 16:9
    /// window shows the whole 4:3 picture plus the sides.
    pub wide: bool,
    /// Visual Effects: the in-game effects (ball sparks and trails, the
    /// move glow, hoop fire, the courts' drifting leaves and steam).
    pub visual_effects: bool,
    /// Adjust Screen, on a PC: full screen rather than a window.
    pub full_screen: bool,
    /// The rewrite's Smooth Motion (PC Settings): the court drawn between
    /// the game's 60 updates a second on faster screens.
    pub smooth: bool,
    /// Controller Layout A, B or C (0..2) for 1P..4P.
    pub layouts: [u8; 4],
    /// Trax: the songs switched off, one bit each (the in-game songs
    /// from bit 0, the front end's from bit [`FRONT_END_BIT`]).
    pub songs_off: u32,
    /// Trax's Song Rotation: songs in order rather than at random.
    pub sequential: bool,
    /// Trax's Track Info: the song's name as a game starts.
    pub track_info: bool,
    /// Music Experience: Interactive (PathFinder) or Playlist (whole songs).
    pub interactive: bool,
    /// The pause menu's Camera Type: 0 Street, 1 Court, 2 Slide, 3 Solid,
    /// 4 Behind, 5 Wide (`pause.txt` 15..20).
    pub camera: u8,
}

/// The league profiles' section tag; the beta's first saves wrote
/// [`LEGACY_LEAGUE_USERS`], which is still read.
const LEAGUE_USERS: [u8; 4] = *b"LGUS";
const LEGACY_LEAGUE_USERS: [u8; 4] = [0x4e, 0x42, 0x41, 0x55];

/// [`Settings::songs_off`]'s first front-end song.
pub const FRONT_END_BIT: u32 = 20;
/// Nelly's front-end song (`NELLY.ASF`, the original's track 0), on from
/// the start.
pub const NELLY_SONG_BIT: u32 = FRONT_END_BIT + 1;
/// The Rucker Classic's song (in-game song 12, the original's track 20):
/// the one the reset leaves off (`FUN_001d7398(track, track !=
/// DAT_003e9ca8)`, `DAT_003e9ca8` being `audio_rucker_classic_track`,
/// registered with 20 at 0x1a2a2c). The Rucker Classic plays it on its
/// court whatever the rotation says; owning the court's card switches it
/// on (`FUN_001e0420`, [`crate::story::Story::court_granted`]; also
/// `FUN_002f2d58`, not traced).
pub const RUCKER_CLASSIC_SONG_BIT: u32 = 11;

/// What the renderer projects: the Widescreen row's three choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picture {
    /// Widescreen off: the 4:3 picture, in the middle of a wider window.
    Standard,
    /// Widescreen on: the PS2's anamorphic frame (`FUN_00323450`'s 16:9
    /// branch), the 4:3 horizontal angle projected at 16:9.
    Anamorphic,
    /// Wide (the rewrite's, through the renderer's both-flags branch): the
    /// 4:3 vertical angle with the horizontal widened by 4/3.
    Wide,
}

/// The 24-byte options block the original's reset leaves (`FUN_001d6f20`,
/// copied to `config+0x12c`): the words its setters write, little endian,
/// as `docs/TRACE.md` "The original's default options" tables them. An
/// import of it is [`Settings::default`].
pub const ORIGINAL_DEFAULT_BLOCK: [u8; 24] = [
    0x2c, 0x10, 0, 0, // help, autosave, effects, Got Game, Street, Automatic
    0x07, 0x00, 0xaa, 0xaa, // 3, Stereo, four volumes of 10
    0xff, 0xff, 0x7f, 0xff, // interactive, random, Track Info, songs but 20
    0x49, 0x02, 0, 0, // four pads: vibration on, layout A
    0, 0, 0, 0, // Adjust Screen x
    0, 0, 0, 0, // Adjust Screen y
];

/// The original's defaults (`FUN_001d6f20`, the options object's reset,
/// traced in `docs/TRACE.md` "The original's default options"): every
/// volume 10, Got Game, Automatic replays, Stereo, help, autosave and
/// visual effects on, Widescreen off, the Street camera, layout A and
/// vibration on every pad, Trax interactive in random order with Track
/// Info on and every song on but the Rucker Classic's (as three save states
/// of two boots hold it, Trax word 0xff7fffff). The window's full screen and the
/// rewrite's Wide picture are off.
impl Default for Settings {
    fn default() -> Self {
        Self {
            music: 10,
            effects: 10,
            speech: 10,
            vibration: true,
            difficulty: 0,
            replays: 1,
            audio: 1,
            help: true,
            autosave: true,
            widescreen: false,
            wide: false,
            visual_effects: true,
            full_screen: false,
            smooth: true,
            layouts: [0; 4],
            songs_off: 1 << RUCKER_CLASSIC_SONG_BIT,
            sequential: false,
            track_info: true,
            interactive: true,
            camera: 0,
        }
    }
}

impl Settings {
    /// The Widescreen row's choice: Wide needs both flags, as the renderer's
    /// third branch does.
    pub fn picture(&self) -> Picture {
        match (self.widescreen, self.wide) {
            (false, _) => Picture::Standard,
            (true, false) => Picture::Anamorphic,
            (true, true) => Picture::Wide,
        }
    }

    pub fn set_picture(&mut self, picture: Picture) {
        self.widescreen = picture != Picture::Standard;
        self.wide = picture == Picture::Wide;
    }

    pub fn encode(&self) -> Vec<u8> {
        let flags = u8::from(self.help)
            | u8::from(self.autosave) << 1
            | u8::from(self.widescreen) << 2
            | u8::from(self.visual_effects) << 3
            | u8::from(self.full_screen) << 4
            | u8::from(self.sequential) << 5
            | u8::from(self.track_info) << 6
            | u8::from(self.wide) << 7;
        let mut out = vec![
            self.music,
            self.effects,
            self.speech,
            u8::from(self.vibration),
            self.difficulty,
            self.replays,
            self.audio,
            flags,
        ];
        out.extend_from_slice(&self.layouts);
        out.extend_from_slice(&self.songs_off.to_le_bytes());
        out.push(self.camera);
        out.push(u8::from(self.interactive));
        out.push(u8::from(self.smooth));
        out
    }

    /// Reads the options back; a save from before the later options keeps
    /// their defaults.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let b = bytes.get(..5)?;
        let mut s = Self {
            music: b[0].min(10),
            effects: b[1].min(10),
            speech: b[2].min(10),
            vibration: b[3] != 0,
            difficulty: b[4].min(2),
            ..Default::default()
        };
        if let Some(rest) = bytes.get(5..16) {
            s.replays = rest[0].min(2);
            s.audio = rest[1].min(2);
            let flags = rest[2];
            s.help = flags & 1 != 0;
            s.autosave = flags & 2 != 0;
            s.widescreen = flags & 4 != 0;
            s.visual_effects = flags & 8 != 0;
            s.full_screen = flags & 16 != 0;
            s.sequential = flags & 32 != 0;
            s.track_info = flags & 64 != 0;
            s.wide = flags & 128 != 0;
            for (k, l) in s.layouts.iter_mut().enumerate() {
                *l = rest[3 + k].min(2);
            }
            s.songs_off = u32::from_le_bytes(rest[7..11].try_into().ok()?);
            if let Some(&camera) = bytes.get(16) {
                s.camera = camera.min(5);
            }
            if let Some(&interactive) = bytes.get(17) {
                s.interactive = interactive != 0;
            }
            if let Some(&smooth) = bytes.get(18) {
                s.smooth = smooth != 0;
            }
        }
        Some(s)
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }

    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    /// A length byte, then up to 255 bytes of UTF-8 cut at a character.
    fn name(&mut self, s: &str) {
        let mut end = s.len().min(255);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push(end as u8);
        self.0.extend_from_slice(&s.as_bytes()[..end]);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn name(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_limited() {
        let mut r = Records::default();
        assert_eq!(r.add_user("JAY"), Ok(0));
        assert_eq!(r.add_user(" jay "), Err(NameError::Exists));
        assert_eq!(r.add_user("  "), Err(NameError::Empty));
        for k in 1..USER_SLOTS {
            r.add_user(&format!("U{k}")).unwrap();
        }
        assert_eq!(r.add_user("ONE MORE"), Err(NameError::Full));
    }

    #[test]
    fn tables_keep_the_best_ten() {
        let mut r = Records::default();
        let row = |v: u32| Row {
            name: format!("P{v}"),
            user: String::new(),
            value: v,
        };
        for v in 1..=12 {
            r.offer(GameRecord::Dunks, row(v));
        }
        let got: Vec<u32> = r.games[GameRecord::Dunks.index()]
            .iter()
            .map(|x| x.value)
            .collect();
        assert_eq!(got, vec![12, 11, 10, 9, 8, 7, 6, 5, 4, 3]);
        assert_eq!(r.offer(GameRecord::Dunks, row(2)), None);
        // A tie goes in above the row it ties (`FUN_001d4070`).
        assert_eq!(r.offer(GameRecord::Dunks, row(8)), Some(4));
        // A table with room takes a nought (the original's empty rows hold
        // noughts, and the first row no better takes the new one).
        assert_eq!(r.offer(GameRecord::Steals, row(0)), Some(0));
    }

    #[test]
    fn users_keep_their_records() {
        let mut r = Records::default();
        for name in ["A", "B", "C"] {
            r.add_user(name).unwrap();
        }
        r.played(0, true, [10, 0, 1, 0, 0, 0]);
        r.played(0, false, [0; 6]);
        r.played(1, true, [0; 6]);
        assert_eq!((r.users[1].wins, r.users[1].losses), (1, 0));
        assert_eq!(r.users[0].percent(), 50);
        assert_eq!(r.users[0].career[0], 10);
    }

    #[test]
    fn school_record_extension_keeps_all_native_values_and_rejects_bad_bounds() {
        let mut records = Records::default();
        records.add_user("A").unwrap();
        let school = &mut records.users[0].school;
        school.flags = 0x80000001;
        school.challenges[32] = 15;
        school.attempts[25] = i32::MIN;
        let bytes = records.encode();
        assert_eq!(Records::decode(&bytes), Some(records.clone()));
        let at = bytes.windows(4).rposition(|w| w == b"SCHU").unwrap();
        let old = Records::decode(&bytes[..at]).unwrap();
        assert_eq!(old.users[0].school, SchoolProgress::default());
        let mut bad = bytes.clone();
        bad[at + 5] = 2;
        assert!(Records::decode(&bad).is_none());
        let mut bad = bytes.clone();
        bad[at + 11] = 16;
        assert!(Records::decode(&bad).is_none());
        assert!(Records::decode(&bytes[..bytes.len() - 1]).is_none());
        let mut duplicate = bytes.clone();
        duplicate.extend_from_slice(&bytes[at..]);
        assert!(Records::decode(&duplicate).is_none());
    }

    #[test]
    fn older_records_have_fresh_unassigned_league_profiles() {
        let mut r = Records::default();
        r.add_user("JAY").unwrap();
        let mut bytes = r.encode();
        let at = bytes.windows(4).rposition(|w| w == LEAGUE_USERS).unwrap();
        bytes.truncate(at);
        let old = Records::decode(&bytes).unwrap();
        assert_eq!(old.users[0].name, "JAY");
        assert_eq!(old.users[0].league, LeagueProgress::default());
        let mut named = old;
        named.users[0].league = LeagueProgress {
            played: 1 << 32,
            cleared: 4,
            lost: 2,
            team: Some(0),
            native_flags: Some(0x81),
            roster: vec![1, 2, 3],
        };
        assert_eq!(Records::decode(&named.encode()), Some(named));
    }

    #[test]
    fn the_tables_place_rows_as_the_original_does() {
        let mut r = Records::default();
        let row = |name: &str, value| Row {
            name: name.into(),
            user: String::new(),
            value,
        };
        for (k, v) in [5, 4, 4, 0].into_iter().enumerate() {
            r.offer(GameRecord::Dunks, row(&format!("R{k}"), v));
        }
        // `FUN_001d4070`: above the first row no better, so a tie goes in
        // first, and a nought still makes a table that has room.
        assert_eq!(r.offer(GameRecord::Dunks, row("NEW", 4)), Some(1));
        assert_eq!(r.offer(GameRecord::Dunks, row("ZERO", 0)), Some(4));
        let names: Vec<&str> = r.games[GameRecord::Dunks.index()]
            .iter()
            .map(|x| x.name.as_str())
            .collect();
        assert_eq!(names, ["R0", "NEW", "R2", "R1", "ZERO", "R3"]);
        // Win Percentage: ten games or more to come in; a name in the
        // table is replaced and the table sorted again.
        assert_eq!(r.offer_win("JAY", 9, 0), None);
        assert_eq!(r.offer_win("JAY", 6, 4), Some(0));
        assert_eq!(r.offer_win("KAY", 9, 1), Some(0));
        assert_eq!(r.offer_win("JAY", 9, 1), Some(1));
        assert_eq!(r.win_rows[0].name, "KAY");
        assert_eq!(
            (r.win_rows[1].name.as_str(), r.win_rows[1].wins),
            ("JAY", 9)
        );
        // A name in the table with no more wins and under ten games stays.
        let mut few = Records::default();
        few.win_rows.push(WinRow {
            name: "LOW".into(),
            wins: 3,
            losses: 9,
        });
        assert_eq!(few.offer_win("LOW", 2, 1), None);
        assert_eq!(few.offer_win("LOW", 4, 1), Some(0));
        // The share: wins over games, in thousandths, truncated.
        assert_eq!((share(95, 5), share(2, 1), share(1, 2)), (950, 666, 333));
        // Street Legends keeps five, best share first.
        for (k, (w, l)) in [(10, 5), (12, 1), (5, 5), (9, 1), (1, 9), (3, 1)]
            .into_iter()
            .enumerate()
        {
            r.legend(Legend {
                name: format!("L{k}"),
                wins: w,
                losses: l,
                ..Default::default()
            });
        }
        let order: Vec<&str> = r.legends.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(order, ["L1", "L3", "L5", "L0", "L2"]);
    }

    #[test]
    fn records_and_settings_survive_a_save() {
        let mut r = Records::default();
        r.add_user("JAY").unwrap();
        r.played(0, true, [21, 5000, 3, 2, 1, 1]);
        r.offer(
            GameRecord::TrickPoints,
            Row {
                name: "Lakers".into(),
                user: "JAY".into(),
                value: 250_000,
            },
        );
        r.legend(Legend {
            name: "STRETCH MONROE".into(),
            nickname: "Slick".into(),
            wins: 38,
            losses: 3,
        });
        r.offer_win("JAY", 12, 3);
        assert_eq!(r.win_rows.len(), 1);
        assert_eq!(Records::decode(&r.encode()), Some(r));
        let s = Settings {
            music: 3,
            vibration: false,
            replays: 2,
            autosave: false,
            widescreen: true,
            wide: true,
            layouts: [1, 2, 0, 1],
            songs_off: 0b101 | 1 << FRONT_END_BIT,
            interactive: false,
            ..Default::default()
        };
        assert_eq!(Settings::decode(&s.encode()), Some(s));
        assert_eq!(s.picture(), Picture::Wide);
        // A save from before the later options keeps their defaults.
        let old = Settings::decode(&[2, 3, 4, 1, 2]).expect("old options");
        assert_eq!((old.music, old.difficulty), (2, 2));
        assert!(old.autosave && !old.widescreen && !old.wide && old.help);
        assert_eq!(old.songs_off, 1 << RUCKER_CLASSIC_SONG_BIT);
        assert!(Settings::decode(&s.encode()[..17]).unwrap().interactive);
        // A save from before Wide: Widescreen on stays the PS2's picture.
        let mut bytes = s.encode();
        bytes[7] &= !128;
        assert_eq!(
            Settings::decode(&bytes).unwrap().picture(),
            Picture::Anamorphic
        );
        let mut off = Settings::default();
        for picture in [Picture::Anamorphic, Picture::Wide, Picture::Standard] {
            off.set_picture(picture);
            assert_eq!(off.picture(), picture);
        }
        assert_eq!(Records::decode(&[1, 0]), None);
    }
}
