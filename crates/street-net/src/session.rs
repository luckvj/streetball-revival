//! Rollback: run ahead of the server on predicted inputs, and re-simulate
//! from the last confirmed state when a confirmation proves a guess wrong.
//!
//! The session keeps two [`Match`]es: the *confirmed* one, stepped only
//! with frames the server fixed, and the *predicted* one, the present the
//! game draws. Between them sit the guessed frames the prediction used. For
//! a guess, the local seats' own pads are known; every other seat repeats
//! its pad from the newest confirmed frame. When confirmed frames arrive the
//! confirmed match steps through them; if each equals the guess made for
//! its tick, the prediction stands (the simulation is deterministic),
//! otherwise the prediction is thrown away and re-run from the confirmed
//! match up to the present.
//!
//! The session does no networking; [`crate::Client`] feeds it.

use std::collections::{BTreeMap, VecDeque};

use street_sim::{Event, Input, World};

use crate::game::{Frame, MAX_SEATS, Match};

/// Tuning for a [`Session`].
#[derive(Clone, Copy, Debug)]
pub struct SessionConfig {
    /// Ticks between reading a local pad and the tick it drives. A tick or
    /// two hides most of the round trip from the other machines' guesses.
    pub input_delay: u32,
    /// The furthest the prediction may run past the confirmed state; past
    /// this the session waits for the server.
    pub max_prediction: u32,
    /// Confirmed states at ticks that are multiples of this are checksummed
    /// for the server.
    pub checksum_every: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            input_delay: 2,
            max_prediction: 120,
            checksum_every: 30,
        }
    }
}

/// What one [`Session::advance`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Advanced {
    /// The tick just simulated; the world now stands at `tick + 1`.
    pub tick: u64,
    /// Ticks re-simulated first because a guess was wrong (0 for none).
    pub rolled_back: u64,
}

/// Counters, for tests and a network overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Ticks predicted for the first time.
    pub predicted: u64,
    /// Times the prediction was re-run.
    pub rollbacks: u64,
    /// Ticks re-simulated by those re-runs.
    pub resimulated: u64,
    /// Full states taken from the server.
    pub resyncs: u64,
    /// Calls to `advance` that waited because the prediction was too far
    /// ahead.
    pub stalls: u64,
    /// Confirmed ticks where the server fixed another pad for one of our
    /// seats than the one played here: ours reached it late, and our own
    /// player was corrected.
    pub overridden: u64,
    /// Times confirmed frames reached past the prediction (the input delay
    /// longer than the round trip and lead): the present then jumps to
    /// them.
    pub overtaken: u64,
}

/// How far the rollbacks since the last look moved what is drawn: the
/// prediction's places before each re-run less those after it, at the same
/// tick (players on the floor, the ball in the air), summed. A renderer
/// eases these out rather than jump.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Correction {
    pub players: Vec<[f32; 2]>,
    pub ball: [f32; 3],
}

impl Correction {
    fn of(world: &World) -> Self {
        Self {
            players: world.players.iter().map(|p| p.position).collect(),
            ball: world.ball.position,
        }
    }

    /// Add `before - after` to the sum.
    fn add(&mut self, before: &Self, after: &Self) {
        self.players.resize(after.players.len(), [0.0; 2]);
        for ((sum, b), a) in self
            .players
            .iter_mut()
            .zip(&before.players)
            .zip(&after.players)
        {
            sum[0] += b[0] - a[0];
            sum[1] += b[1] - a[1];
        }
        for k in 0..3 {
            self.ball[k] += before.ball[k] - after.ball[k];
        }
    }
}

/// A rollback session over one match (see the module docs).
pub struct Session {
    cfg: SessionConfig,
    local_seats: Vec<u8>,
    confirmed: Match,
    /// Confirmed frames at or after the confirmed state's tick, not applied yet.
    arrived: BTreeMap<u64, Frame>,
    /// The newest confirmed frame seen, for guessing other seats.
    latest: Option<(u64, Frame)>,
    predicted: Match,
    /// The frames guessed for ticks `confirmed.tick() .. predicted.tick()`.
    guesses: VecDeque<Frame>,
    /// Local pads from tick `local_base`, one per local seat each tick.
    local: VecDeque<Vec<Input>>,
    local_base: u64,
    /// The pads to commit at the next advance.
    pads: Vec<Input>,
    /// The first tick whose local pads the server still wants.
    input_ack: u64,
    reports: Vec<(u64, u64)>,
    /// What happened in confirmed ticks since the last look: `(tick,
    /// events)` for ticks with any.
    confirmed_events: Vec<(u64, Vec<Event>)>,
    /// The rollbacks' corrections since the last look.
    correction: Option<Correction>,
    stats: SessionStats,
}

/// Confirmed events kept for a game that does not take them.
const MAX_CONFIRMED_EVENTS: usize = 600;

impl Session {
    /// A session starting from `start`, with these seats played here.
    pub fn new(start: Match, local_seats: Vec<u8>, cfg: SessionConfig) -> Self {
        let tick = start.tick();
        Self {
            cfg,
            pads: vec![Input::default(); local_seats.len()],
            local_seats,
            confirmed: start.clone(),
            arrived: BTreeMap::new(),
            latest: None,
            predicted: start,
            guesses: VecDeque::new(),
            local: VecDeque::new(),
            local_base: tick,
            input_ack: tick,
            reports: Vec::new(),
            confirmed_events: Vec::new(),
            correction: None,
            stats: SessionStats::default(),
        }
    }

    /// Set a local seat's pad for the next [`Session::advance`]. A pad not
    /// set again stays held.
    pub fn add_local_input(&mut self, seat: u8, input: Input) {
        if let Some(j) = self.local_seats.iter().position(|&s| s == seat) {
            self.pads[j] = input;
        }
    }

    /// Simulate the next tick: first fold in any confirmed frames (rolling
    /// back if a guess was wrong), then predict one tick on. `None` when the
    /// prediction is already [`SessionConfig::max_prediction`] ahead.
    pub fn advance(&mut self) -> Option<Advanced> {
        let rolled_back = self.settle();
        let ahead = self.predicted.tick() - self.confirmed.tick();
        if ahead >= u64::from(self.cfg.max_prediction) {
            self.stats.stalls += 1;
            return None;
        }
        let tick = self.predicted.tick();
        self.commit_pads(tick + u64::from(self.cfg.input_delay));
        let guess = self.guess(tick);
        self.predicted.step(&guess);
        self.guesses.push_back(guess);
        self.stats.predicted += 1;
        Some(Advanced { tick, rolled_back })
    }

    /// Frames the server fixed, the first for tick `first`.
    pub fn confirm(&mut self, first: u64, frames: &[Frame]) {
        let floor = self.confirmed.tick();
        for (n, f) in frames.iter().enumerate() {
            let t = first + n as u64;
            if t < floor || t > floor + 100_000 {
                continue;
            }
            self.arrived.insert(t, *f);
            if self.latest.is_none_or(|(l, _)| t >= l) {
                self.latest = Some((t, *f));
            }
        }
    }

    /// Replace the confirmed state with the server's (`state` is the match at
    /// the start of `state.tick()`), and re-run the prediction from it.
    pub fn resync(&mut self, state: Match) {
        let target = self.predicted.tick();
        self.confirmed = state;
        let floor = self.confirmed.tick();
        self.arrived.retain(|&t, _| t >= floor);
        self.reports.retain(|&(t, _)| t >= floor);
        self.predicted = self.confirmed.clone();
        self.guesses.clear();
        self.stats.resyncs += 1;
        self.resimulate(target);
        if self.local_base > floor {
            // Nothing local that old: start the record at the new state.
            self.local.clear();
            self.local_base = floor;
        }
    }

    /// The server has every local pad before `ack`.
    pub fn acknowledge_inputs(&mut self, ack: u64) {
        self.input_ack = self.input_ack.max(ack);
    }

    /// The first tick whose local pads the server still wants, as far as we
    /// know.
    pub fn input_ack(&self) -> u64 {
        self.input_ack
    }

    /// Local pads from tick `from` (or the oldest kept), at most `max` ticks,
    /// oldest first: `(first tick, pads per tick)`.
    pub fn local_inputs(&self, from: u64, max: usize) -> (u64, Vec<Vec<Input>>) {
        let start = from.max(self.local_base);
        let skip = (start - self.local_base) as usize;
        let run: Vec<Vec<Input>> = self.local.iter().skip(skip).take(max).cloned().collect();
        (start, run)
    }

    /// The first confirmed tick this session lacks.
    pub fn need(&self) -> u64 {
        let mut t = self.confirmed.tick();
        while self.arrived.contains_key(&t) {
            t += 1;
        }
        t
    }

    /// Checksums of confirmed states made since the last call, `(tick, sum)`.
    pub fn take_reports(&mut self) -> Vec<(u64, u64)> {
        std::mem::take(&mut self.reports)
    }

    /// The events of the ticks confirmed since the last call, `(tick,
    /// events)`, oldest first: what really happened, where the predicted
    /// world's events may be undone by a rollback (a box score wants these;
    /// sounds can't wait for them).
    pub fn take_confirmed_events(&mut self) -> Vec<(u64, Vec<Event>)> {
        std::mem::take(&mut self.confirmed_events)
    }

    /// How far the rollbacks since the last call moved the drawn world
    /// ([`Correction`]); `None` if none did. A resync's jump is not one.
    pub fn take_correction(&mut self) -> Option<Correction> {
        self.correction.take()
    }

    /// The present, to draw: the predicted world.
    pub fn world(&self) -> &World {
        &self.predicted.world
    }

    /// The predicted match (the world plus who each seat drives).
    pub fn current(&self) -> &Match {
        &self.predicted
    }

    /// The last state every machine agrees on.
    pub fn confirmed(&self) -> &Match {
        &self.confirmed
    }

    /// The tick the present stands at.
    pub fn tick(&self) -> u64 {
        self.predicted.tick()
    }

    pub fn local_seats(&self) -> &[u8] {
        &self.local_seats
    }

    /// Change the input delay from the next tick's pads on: a longer one
    /// holds the last pad over the ticks it skips, a shorter one drops the
    /// pads already set for the ticks it overlaps.
    pub fn set_input_delay(&mut self, delay: u32) {
        self.cfg.input_delay = delay;
    }

    pub fn config(&self) -> &SessionConfig {
        &self.cfg
    }

    pub fn stats(&self) -> SessionStats {
        self.stats
    }

    /// Change the confirmed state behind the server's back. Only for tests
    /// of the resync path.
    #[doc(hidden)]
    pub fn tamper(&mut self, f: impl FnOnce(&mut Match)) {
        f(&mut self.confirmed);
    }

    /// Step the confirmed match through the frames that have arrived, and
    /// re-run the prediction if one of them differs from its guess. Returns
    /// the ticks re-simulated.
    fn settle(&mut self) -> u64 {
        let mut wrong = false;
        while let Some(f) = self.arrived.remove(&self.confirmed.tick()) {
            match self.guesses.pop_front() {
                Some(g) if g == f => {}
                Some(g) => {
                    wrong = true;
                    if self.local_seats.iter().any(|&s| {
                        usize::from(s) < MAX_SEATS
                            && g.seats[usize::from(s)] != f.seats[usize::from(s)]
                    }) {
                        self.stats.overridden += 1;
                    }
                }
                None => {}
            }
            self.confirmed.step(&f);
            let events = &self.confirmed.world.events;
            if !events.is_empty() {
                if self.confirmed_events.len() >= MAX_CONFIRMED_EVENTS {
                    self.confirmed_events.remove(0);
                }
                self.confirmed_events
                    .push((self.confirmed.tick() - 1, events.clone()));
            }
            let t = self.confirmed.tick();
            if t.is_multiple_of(self.cfg.checksum_every) {
                self.reports.push((t, self.confirmed.checksum()));
            }
        }
        self.prune_local();
        if self.confirmed.tick() >= self.predicted.tick() {
            // The confirmations overtook the prediction.
            if self.confirmed.tick() > self.predicted.tick() {
                self.stats.overtaken += 1;
            }
            if self.confirmed.tick() > self.predicted.tick() || wrong {
                let before = Correction::of(&self.predicted.world);
                let same = self.confirmed.tick() == self.predicted.tick();
                self.predicted = self.confirmed.clone();
                if same {
                    self.note_correction(&before);
                }
            }
            self.guesses.clear();
            return 0;
        }
        if !wrong {
            return 0;
        }
        let target = self.predicted.tick();
        let before = Correction::of(&self.predicted.world);
        self.predicted = self.confirmed.clone();
        self.guesses.clear();
        self.stats.rollbacks += 1;
        let n = self.resimulate(target);
        self.note_correction(&before);
        n
    }

    /// Add the move from `before` to the prediction now (the same tick) to
    /// the corrections not taken yet.
    fn note_correction(&mut self, before: &Correction) {
        let after = Correction::of(&self.predicted.world);
        self.correction
            .get_or_insert_with(Correction::default)
            .add(before, &after);
    }

    /// Predict from the predicted match up to tick `target`.
    fn resimulate(&mut self, target: u64) -> u64 {
        let mut n = 0;
        while self.predicted.tick() < target {
            let guess = self.guess(self.predicted.tick());
            self.predicted.step(&guess);
            self.guesses.push_back(guess);
            n += 1;
        }
        self.stats.resimulated += n;
        n
    }

    /// The frame to predict tick `t` with.
    fn guess(&self, t: u64) -> Frame {
        let mut f = self.latest.map(|(_, f)| f).unwrap_or_default();
        for (j, &seat) in self.local_seats.iter().enumerate() {
            if usize::from(seat) < MAX_SEATS {
                f.seats[usize::from(seat)] = Some(self.local_pad(t, j));
            }
        }
        f
    }

    /// Local seat `j`'s pad at tick `t`: as recorded, else the newest held.
    fn local_pad(&self, t: u64, j: usize) -> Input {
        let entry = t
            .checked_sub(self.local_base)
            .and_then(|k| self.local.get(k as usize))
            .or(self.local.back());
        entry.map_or(Input::default(), |pads| pads[j])
    }

    /// Record the pads set for tick `t`, holding the last ones over any gap.
    fn commit_pads(&mut self, t: u64) {
        let mut end = self.local_base + self.local.len() as u64;
        if end > t {
            return;
        }
        let held = self
            .local
            .back()
            .cloned()
            .unwrap_or_else(|| vec![Input::default(); self.local_seats.len()]);
        while end < t {
            self.local.push_back(held.clone());
            end += 1;
        }
        self.local.push_back(self.pads.clone());
    }

    /// Forget local pads that neither a guess nor the server needs.
    fn prune_local(&mut self) {
        let keep_from = self.confirmed.tick().min(self.input_ack);
        while self.local_base < keep_from && self.local.len() > 1 {
            self.local.pop_front();
            self.local_base += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MatchSetup;
    use street_sim::{button, math};

    fn start() -> Match {
        let s = MatchSetup::standard(Default::default(), 4);
        Match::new(s.world, s.seat_teams)
    }

    fn pad(t: u64, salt: u64) -> Input {
        let a = (t + salt * 37) as f32 * 0.05;
        let buttons = if (t / 7 + salt).is_multiple_of(5) {
            button::SHOOT | button::PASS
        } else {
            0
        };
        Input::moving([math::sin(a), math::cos(a)]).with(buttons)
    }

    /// The server's view: seats 0 and 1 driven with known pads.
    fn frame(t: u64) -> Frame {
        let mut f = Frame::default();
        f.seats[0] = Some(pad(t, 0));
        f.seats[1] = Some(pad(t, 1));
        f
    }

    #[test]
    fn late_confirmations_roll_back_to_the_truth() {
        let cfg = SessionConfig {
            input_delay: 0,
            ..Default::default()
        };
        let mut s = Session::new(start(), vec![0], cfg);
        let mut truth = start();
        for t in 0..3000u64 {
            s.add_local_input(0, pad(t, 0));
            s.advance().expect("not stalled");
            // Confirmations arrive 8 ticks late, in bursts of 4.
            if t >= 8 && t % 4 == 0 {
                let from = t - 8;
                let frames: Vec<Frame> = (from..from + 4).map(frame).collect();
                s.confirm(from, &frames);
            }
        }
        s.advance();
        for t in 0..s.confirmed().tick() {
            truth.step(&frame(t));
        }
        assert_eq!(s.confirmed().checksum(), truth.checksum());
        assert!(
            s.stats().rollbacks > 0,
            "seat 1's pads change, so guesses miss"
        );
        // Catch the prediction up with every frame: it must equal the truth.
        let end = s.tick();
        let rest: Vec<Frame> = (s.confirmed().tick()..end).map(frame).collect();
        s.confirm(s.confirmed().tick(), &rest);
        s.advance();
        for t in truth.tick()..end {
            truth.step(&frame(t));
        }
        assert_eq!(s.confirmed().checksum(), truth.checksum());
    }

    /// The confirmed events are the truth's, tick for tick, however the
    /// prediction guessed.
    #[test]
    fn confirmed_events_are_what_really_happened() {
        let cfg = SessionConfig {
            input_delay: 0,
            ..Default::default()
        };
        let mut s = Session::new(start(), vec![0], cfg);
        let mut got = Vec::new();
        for t in 0..3000u64 {
            s.add_local_input(0, pad(t, 0));
            s.advance().expect("not stalled");
            if t >= 8 && t % 4 == 0 {
                let from = t - 8;
                let frames: Vec<Frame> = (from..from + 4).map(frame).collect();
                s.confirm(from, &frames);
            }
            got.extend(s.take_confirmed_events());
        }
        let mut truth = start();
        let mut want = Vec::new();
        for t in 0..s.confirmed().tick() {
            truth.step(&frame(t));
            if !truth.world.events.is_empty() {
                want.push((t, truth.world.events.clone()));
            }
        }
        assert!(!want.is_empty(), "something happens in 3000 ticks");
        assert_eq!(got, want);
    }

    #[test]
    fn prediction_stops_at_the_limit() {
        let cfg = SessionConfig {
            max_prediction: 10,
            ..Default::default()
        };
        let mut s = Session::new(start(), vec![0], cfg);
        for _ in 0..10 {
            assert!(s.advance().is_some());
        }
        assert!(s.advance().is_none());
        s.confirm(0, &[Frame::default(); 3]);
        assert!(s.advance().is_some());
    }

    #[test]
    fn local_pads_are_kept_until_acknowledged() {
        let mut s = Session::new(start(), vec![2], SessionConfig::default());
        for t in 0..20 {
            s.add_local_input(2, pad(t, 0));
            s.advance();
        }
        // Delay 2: ticks 0 and 1 hold the default, then the pads from tick 2.
        let (first, run) = s.local_inputs(0, 64);
        assert_eq!(first, 0);
        assert_eq!(run.len(), 22);
        assert_eq!(run[0][0], Input::default());
        assert_eq!(run[2][0], pad(0, 0));
        s.acknowledge_inputs(15);
        s.confirm(0, &[Frame::default(); 18]);
        s.advance();
        let (first, _) = s.local_inputs(0, 64);
        assert_eq!(first, 15);
    }
}
