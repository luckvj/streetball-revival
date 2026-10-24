//! Private, test-staged numeric projection of the ordinary primary channel.
//!
//! This is not an actor update or a renderer clock. 1704e0/4574e2 and the
//! interior, forward AB loop are the admitted paths. Row handles, pose/root
//! evaluation, action callbacks and facial scheduling still need production
//! owners. Returned effects identify those obligations; they are not executed.
//! In particular the idle entry is rejected before its random row/blend draw.

use crate::rng::Rng;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pc: u32,
    wait: Option<i32>,
    flags: u32,
    state_flags: Option<[u32; 2]>,
    row: i32,
    alternate: i32,
    link: i32,
    integer: i32,
    target: i32,
    phase: u32,
    configured_step: u32,
    step: Option<u32>,
    duration: Option<i32>,
    counter: Option<i32>,
    held_offset: u32,
}

impl Channel {
    /// Only modeled stores actually present in 193130. Wait/step/duration
    /// and frame-state words are not initialized by that constructor.
    #[cfg(test)]
    fn constructed() -> Self {
        Self {
            pc: 0,
            wait: None,
            flags: 0,
            state_flags: None,
            row: -1,
            alternate: -1,
            link: -1,
            integer: -1,
            target: -2,
            phase: (-1.0_f32).to_bits(),
            configured_step: 0,
            step: None,
            duration: None,
            counter: None,
            held_offset: 0,
        }
    }
}

/// Raw native numeric fields, rather than an invented World/player mapping.
/// Quaternion/orientation 168ea8 is deliberately outside this projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    position: [u32; 4], // 30
    physical: [u32; 4], // 40
    elapsed: i32,       // 60
    flight: i32,        // 64
    flags: u32,         // a4
    heading: i32,       // a8
    heading_b0: i32,
    state_198: i32,
    state_1a0: i32,
    state_1a4: i32,
    state_1a8: i32,
    state_1b8: i32,
    state_1d4: i32,
    state_2b4: i32,
    placement: [u32; 4],    // 2c0
    placement_heading: i32, // 2d0
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    actor: Actor,
    primary: Channel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// 89 also resolves a live animation handle and selects pose buffers.
    SelectRow(i32),
    /// 19a658 continues into pose/root/rig evaluation after the modeled
    /// scalar clock. A future consumer must implement its complete effects.
    Sample {
        row: i32,
        phase: u32,
        integer: i32,
        state_flags: [u32; 2],
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Source(u32),
    Opcode { pc: u32, opcode: u8 },
    ChannelFlags(u32),
    UninitializedClock,
    DynamicRow(i32),
    MissingRow(i32),
    GroundedReset,
    Rate(u32),
    Transition,
    Boundary,
    NonFinite,
    Budget,
}

pub struct Command<'a> {
    opcode: u8,
    next: u32,
    operands: &'a [u8],
}

/// Implemented by a view of the already decoded immutable assets in tests.
/// The core does not rediscover branches or select a row from capture data.
pub trait Source {
    fn command(&self, pc: u32) -> Option<Command<'_>>;
    fn states(&self, row: i32) -> Option<&[u32]>;
}

fn operand(command: &Command<'_>, pc: u32) -> Result<u32, Unsupported> {
    Ok(u32::from_le_bytes(
        command
            .operands
            .try_into()
            .map_err(|_| Unsupported::Source(pc))?,
    ))
}

fn transaction<T>(
    state: &mut Projection,
    rng: &mut Rng,
    operation: impl FnOnce(&mut Projection, &mut Rng) -> Result<T, Unsupported>,
) -> Result<T, Unsupported> {
    let (mut candidate, mut candidate_rng) = (state.clone(), rng.clone());
    let result = operation(&mut candidate, &mut candidate_rng)?;
    (*state, *rng) = (candidate, candidate_rng);
    Ok(result)
}

/// Projection of 1704e0, accepting the *resolved* 18ce78 integer heading.
/// The actual look-at producer, full constructor and RNG draw order have not
/// been connected to this type. In particular no captured gait row is a seed.
pub fn place(
    state: &mut Projection,
    rng: &mut Rng,
    source: &impl Source,
    position: [u32; 4],
    resolved_heading: i32,
) -> Result<Vec<Effect>, Unsupported> {
    transaction(state, rng, |state, rng| {
        if position
            .iter()
            .any(|bits| !f32::from_bits(*bits).is_finite())
        {
            return Err(Unsupported::NonFinite);
        }
        let actor = &mut state.actor;
        actor.placement = position;
        actor.placement_heading = resolved_heading;
        actor.position = position;
        actor.flags = 0;
        actor.heading = resolved_heading;
        actor.heading_b0 = resolved_heading;
        actor.physical = [0; 4];
        let effects = install(state, rng, source, 0x4574e2)?;
        // 170568..1705e0, including the reached post-install flag clears.
        state.primary.flags &= !(0x40000 | 0x80000 | 0x0200_0000);
        let actor = &mut state.actor;
        actor.position = actor.placement;
        actor.state_1b8 = -1;
        actor.state_2b4 = 1;
        actor.state_1a0 = 0;
        actor.state_1a8 = 0;
        actor.state_1d4 = 0;
        Ok(effects)
    })
}

/// 19c320 primary-channel numeric dispatch. The actor relation branches are
/// excluded, and buffer/handle effects of 89 are returned explicitly.
fn install(
    state: &mut Projection,
    _rng: &mut Rng,
    source: &impl Source,
    address: u32,
) -> Result<Vec<Effect>, Unsupported> {
    if state.actor.flags & 0x60000 != 0 {
        return Err(Unsupported::Transition);
    }
    let channel = &mut state.primary;
    // Admit only otherwise inert bits used by the placement caller. Other
    // high flags can alter handles, pose buffers, held motion or dispatch.
    if channel.flags & !(0x020c_0000 | 0x0002_2008) != 0 {
        return Err(Unsupported::ChannelFlags(channel.flags));
    }
    if channel.alternate != -1 || channel.link != -1 || channel.held_offset != 0 {
        return Err(Unsupported::Transition);
    }
    channel.pc = address;
    channel.wait = Some(1);
    channel.flags &= 0xffdf_0000;
    let effects = dispatch(state, source)?;
    // Set+8 is zero for this outside-update install. Do not advance again
    // until the following 193440 call has consumed this wait2 -> wait1.
    state.primary.wait = Some(2);
    Ok(effects)
}

fn dispatch(state: &mut Projection, source: &impl Source) -> Result<Vec<Effect>, Unsupported> {
    let mut effects = Vec::new();
    // 193440 decrements the installed wait before reading the first opcode.
    state.primary.wait = Some(0);
    for _ in 0..32 {
        let pc = state.primary.pc;
        let command = source.command(pc).ok_or(Unsupported::Source(pc))?;
        if command.next <= pc || command.next - pc != command.operands.len() as u32 + 1 {
            return Err(Unsupported::Source(pc));
        }
        state.primary.pc = command.next; // native stores PC before dispatch
        match command.opcode {
            0xa7 if command.operands.is_empty() => {
                state.actor.state_198 = 0;
                state.actor.state_1a4 = 0;
            }
            0x96 if command.operands.is_empty() => state.primary.flags |= 0x0200_0000,
            0x97 if command.operands.is_empty() => {
                // 1947a0 clears physical40..4c, then 19bed8 evaluates the
                // grounded case to its minimum1; it also resets actor60.
                // Nonzero height needs the full time/FPU/ballistic producer.
                if state.actor.position[1] != 0 {
                    return Err(Unsupported::GroundedReset);
                }
                state.actor.physical = [0; 4];
                state.actor.flight = 1;
                state.actor.elapsed = 0;
            }
            0x91 => {
                state.primary.flags = (state.primary.flags & 0xffff_0000) | operand(&command, pc)?;
            }
            0x9b => state.actor.flags = operand(&command, pc)?,
            0x89 => {
                let row = operand(&command, pc)? as i32;
                if row != 65 {
                    return Err(Unsupported::DynamicRow(row));
                }
                let states = source.states(row).ok_or(Unsupported::MissingRow(row))?;
                let flags = *states.first().ok_or(Unsupported::MissingRow(row))?;
                let channel = &mut state.primary;
                if channel.flags & !0x020e_0000 != 0 {
                    return Err(Unsupported::ChannelFlags(channel.flags));
                }
                channel.row = row;
                channel.state_flags = Some([flags; 2]);
                channel.target = -2;
                channel.phase = (-1.0_f32).to_bits();
                channel.integer = -1;
                channel.wait = Some(0);
                channel.flags &= !0x1000;
                channel.link = -1;
                channel.alternate = -1;
                channel.step = Some(0);
                channel.held_offset = 0;
                effects.push(Effect::SelectRow(row));
            }
            0xab => {
                let rate = operand(&command, pc)?;
                let step = if rate == (-1.0_f32).to_bits() {
                    state.primary.configured_step
                } else if rate == 6.0_f32.to_bits() {
                    // Source DIV.S197234, scoped AB6 result3e2aaaab is
                    // independently retained. No global FCSR/divide claim.
                    (1.0_f32 / 6.0).to_bits()
                } else {
                    return Err(Unsupported::Rate(rate));
                };
                let channel = &mut state.primary;
                if channel.integer == -1 {
                    channel.integer = 0;
                }
                channel.target = -1;
                channel.duration = Some(-1);
                channel.counter = Some(-1);
                channel.step = Some(step);
                channel.held_offset = 0;
                channel.alternate = -1;
                channel.phase = (channel.integer as f32).to_bits();
                channel.flags |= 0x2008;
                effects.push(advance_clock(channel, source)?);
                return Ok(effects);
            }
            opcode => return Err(Unsupported::Opcode { pc, opcode }),
        }
    }
    Err(Unsupported::Budget)
}

/// Numeric 193440 call only. This deliberately does not call it an actor
/// tick: action/facial callbacks, actor2a4 gating and other channels remain
/// outside the admitted entry. No RNG can be skipped by a live caller here.
pub fn update_primary(
    state: &mut Projection,
    rng: &mut Rng,
    source: &impl Source,
) -> Result<Vec<Effect>, Unsupported> {
    transaction(state, rng, |state, _| {
        let channel = &mut state.primary;
        if channel.flags & 1 != 0 {
            return Ok(Vec::new());
        }
        let wait = channel.wait.ok_or(Unsupported::UninitializedClock)?;
        if wait < 1 {
            return Err(Unsupported::Transition);
        }
        channel.wait = Some(wait - 1);
        if wait > 1 {
            return Ok(Vec::new());
        }
        if channel.flags & 0x200008 == 0 {
            return Err(Unsupported::Transition);
        }
        Ok(vec![advance_clock(channel, source)?])
    })
}

fn advance_clock(channel: &mut Channel, source: &impl Source) -> Result<Effect, Unsupported> {
    if channel.flags & !0x020e_2008 != 0 || channel.flags & 0x2008 != 0x2008 {
        return Err(Unsupported::ChannelFlags(channel.flags));
    }
    if channel.alternate != -1
        || channel.link != -1
        || channel.target != -1
        || channel.duration != Some(-1)
        || channel.counter != Some(-1)
        || channel.held_offset != 0
    {
        return Err(Unsupported::Transition);
    }
    let (phase, step) = (
        f32::from_bits(channel.phase),
        f32::from_bits(channel.step.ok_or(Unsupported::UninitializedClock)?),
    );
    if !phase.is_finite() || !step.is_finite() {
        return Err(Unsupported::NonFinite);
    }
    if phase < 0.0
        || (phase != 0.0 && !phase.is_normal())
        || !step.is_normal()
        || step <= 0.0
        || channel.integer < 0
    {
        return Err(Unsupported::Transition);
    }
    let states = source
        .states(channel.row)
        .filter(|states| (2..=65536).contains(&states.len()))
        .ok_or(Unsupported::MissingRow(channel.row))?;
    // Accumulate positive operands in binary64, then chop the binary32
    // result. This is a bounded clock operation, not a global FCSR model.
    let exact = f64::from(phase) + f64::from(step);
    let rounded = exact as f32;
    let next = if f64::from(rounded) > exact {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    };
    // Wrapping and the last-frame self blend carry further buffer/root
    // effects. Reject before committing instead of clamping or resetting.
    if !next.is_finite() || next >= (states.len() - 1) as f32 {
        return Err(Unsupported::Boundary);
    }
    channel.phase = next.to_bits(); // ADD.S19a8ec -> store19a9cc
    channel.integer = next as i32; // nonnegative CVT.W.S19acbc
    channel.wait = Some(1);
    let flags = states[channel.integer as usize];
    channel.state_flags = Some([flags; 2]);
    // Reached scalar tail19b618..19b62c: with alternate==-1, the
    // placement bit20000 is cleared even though root/rig math is pending.
    channel.flags &= !0x20000;
    let state_flags = channel.state_flags.unwrap_or([0; 2]);
    Ok(Effect::Sample {
        row: channel.row,
        phase: channel.phase,
        integer: channel.integer,
        state_flags,
    })
}

/// Production-facing callback sink for the native ordinary-channel effects.
///
/// Returned effects describe obligations the caller must execute. The state
/// flags are included so a row consumer can apply them without re-reading the
/// source's staged state table.
pub trait OrdinarySink {
    fn select_row(&mut self, row: i32) -> Result<(), Unsupported>;
    fn sample(
        &mut self,
        row: i32,
        phase: u32,
        integer: i32,
        state_flags: [u32; 2],
    ) -> Result<(), Unsupported>;
}

/// Apply a native ordinary-channel effect stream to a sink. All effects are
/// rejected atomically by the caller's sink returning an error; this helper
/// simply stops at the first unsupported effect.
pub fn apply_effects(sink: &mut impl OrdinarySink, effects: &[Effect]) -> Result<(), Unsupported> {
    for effect in effects {
        match effect {
            Effect::SelectRow(row) => sink.select_row(*row)?,
            Effect::Sample {
                row,
                phase,
                integer,
                state_flags,
            } => sink.sample(*row, *phase, *integer, *state_flags)?,
        }
    }
    Ok(())
}

/// Minimal sink that records the state flags of the most recent native sample.
/// It exists to expose the callback that used to be only internal to the
/// numeric projection.
pub struct StateFlagsCallback {
    pub row: i32,
    pub state_flags: [u32; 2],
}

impl Default for StateFlagsCallback {
    fn default() -> Self {
        Self {
            row: 65,
            state_flags: [0; 2],
        }
    }
}

impl OrdinarySink for StateFlagsCallback {
    fn select_row(&mut self, row: i32) -> Result<(), Unsupported> {
        self.row = row;
        Ok(())
    }

    fn sample(
        &mut self,
        row: i32,
        _phase: u32,
        _integer: i32,
        state_flags: [u32; 2],
    ) -> Result<(), Unsupported> {
        self.row = row;
        self.state_flags = state_flags;
        Ok(())
    }
}

/// Canonical ordinary gait rows.
pub const HELD_GAIT_ROW: u16 = 88;
pub const UNHELD_GAIT_ROW: u16 = 89;
pub const PLACEMENT_ROW: u16 = 65;

/// Select native ordinary gait animation row based on ball possession and movement.
pub fn select_gait_row(held: bool, moving: bool) -> u16 {
    if !moving {
        PLACEMENT_ROW
    } else if held {
        HELD_GAIT_ROW
    } else {
        UNHELD_GAIT_ROW
    }
}

/// Calculate the native gait frame step from root distance, actor speed, and body scale X.
/// Matches raw MIPS 167cd0..167ce8: (record+18 / actor+190) * bodyScale.x, stored at actor+32c as reciprocal.
/// Actor+0x190 is the displacement per native update, so `speed` (court
/// units per second) is divided down to one 60 Hz update first.
pub fn calculate_gait_frame_step(root_distance: f32, speed: f32, body_scale_x: f32) -> f32 {
    let denom = root_distance * body_scale_x;
    let per_update = speed * crate::TICK_SECONDS;
    if denom <= 1e-4 || per_update <= 1e-6 {
        1.0 / 6.0
    } else {
        per_update / denom
    }
}

/// Actor sink that receives effects from the ordinary primary channel and drives actor state.
#[derive(Clone, Debug, PartialEq)]
pub struct OrdinaryActorSink {
    pub row: i32,
    pub phase: f32,
    pub integer: i32,
    pub state_flags: [u32; 2],
    pub position: [f32; 2],
    pub heading: i32,
}

impl Default for OrdinaryActorSink {
    fn default() -> Self {
        Self {
            row: PLACEMENT_ROW as i32,
            phase: 0.0,
            integer: 0,
            state_flags: [0; 2],
            position: [0.0; 2],
            heading: 0,
        }
    }
}

impl OrdinarySink for OrdinaryActorSink {
    fn select_row(&mut self, row: i32) -> Result<(), Unsupported> {
        self.row = row;
        Ok(())
    }

    fn sample(
        &mut self,
        row: i32,
        phase: u32,
        integer: i32,
        state_flags: [u32; 2],
    ) -> Result<(), Unsupported> {
        self.row = row;
        self.phase = f32::from_bits(phase);
        self.integer = integer;
        self.state_flags = state_flags;
        Ok(())
    }
}

/// Continuous ordinary locomotion channel state for an actor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerGaitChannel {
    pub row: u16,
    pub phase: f32,
    pub integer: i32,
    pub frame_step: f32,
    pub blend_counter: i32,
    pub held: bool,
    pub moving: bool,
    pub heading: i32,
    /// The native gait family flags (`GaitFamily` bits, 0 standing), kept
    /// as they are: Jog shares `Gait::Walk`'s clip but not its hysteresis.
    pub family: u32,
    /// The original gait record playing (`crate::gait_select`), if the
    /// match has the records: its rows, root distance and motion range.
    pub record: Option<u16>,
    pub rows: [u16; 2],
    pub root_distance: f32,
    /// Motion per native update at the slow and fast ends.
    pub motion: [f32; 2],
    /// The live ball hand (`+0x25c`): 1 right, 2 left. The constructor
    /// (`156638`) starts it at 2.
    pub hand: u8,
    /// The trick under way plays its 0xf8 alternate (1) or primary (0),
    /// latched from the live hand when it started.
    pub trick_side: u8,
    /// The row a change of row blends from, held at the phase it had
    /// reached (channel+0x50 and its frame while flag 0x200000 is set: the
    /// played captures' gait changes hold the old clip's frame through a
    /// blend of 6 updates), while [`PlayerGaitChannel::blend_counter`]
    /// runs ([`crate::World::actor_pose`]).
    pub blend_from: Option<(u16, f32)>,
}

impl PlayerGaitChannel {
    /// The row the record plays: its 0xf8 alternate on hand 2
    /// (`FUN_001978b0`), else its primary.
    pub fn record_row(&self) -> u16 {
        self.rows[usize::from(self.hand == 2)]
    }
}

impl Default for PlayerGaitChannel {
    fn default() -> Self {
        Self {
            row: PLACEMENT_ROW,
            phase: 0.0,
            integer: 0,
            frame_step: 1.0 / 6.0,
            blend_counter: 0,
            held: false,
            moving: false,
            heading: 0,
            family: 0,
            record: None,
            rows: [PLACEMENT_ROW; 2],
            root_distance: 0.0,
            motion: [0.0; 2],
            hand: 2,
            trick_side: 0,
            blend_from: None,
        }
    }
}

impl PlayerGaitChannel {
    /// Update the locomotion channel for one tick of movement: `ways` is
    /// the facing and the course (the velocity's direction), radians.
    pub fn update(
        &mut self,
        held: bool,
        moving: bool,
        speed: f32,
        position: &mut [f32; 2],
        ways: [f32; 2],
        body_scale_x: f32,
    ) {
        let [heading_rad, course_rad] = ways;
        let wanted_row = match self.record {
            Some(_) if moving => self.record_row(),
            _ => select_gait_row(held, moving),
        };
        if wanted_row != self.row {
            self.blend_from = Some((self.row, self.phase));
            self.row = wanted_row;
            self.blend_counter = 6;
            self.phase = 0.0;
            self.integer = 0;
        } else if self.blend_counter > 0 {
            self.blend_counter -= 1;
        }

        self.held = held;
        self.moving = moving;
        // The playing record's root distance; without the records, record
        // 0x4b97d0's 0.590390 held and an unheld stand-in.
        let root_distance = match self.record {
            Some(_) if moving => self.root_distance,
            _ if held => 0.590390,
            _ => 0.582450,
        };
        self.frame_step = calculate_gait_frame_step(root_distance, speed, body_scale_x);

        // The channel's clock runs standing too: the idle row's cycle (and
        // the standing dribble drawn from it) keeps going.
        self.phase += self.frame_step;
        self.integer = self.phase as i32;
        if moving {
            // Along the velocity, which the stick points (`0x167d88`),
            // whatever the facing.
            let fx = course_rad.sin();
            let fz = course_rad.cos();
            let step = speed * (1.0 / 60.0);
            position[0] += fx * step;
            position[1] += fz * step;
        }

        // Convert radians heading to native 10-bit integer heading
        let scale = 162.97466_f32;
        let scaled = heading_rad * scale + 0.5;
        self.heading = (scaled as i32) & 0x3ff;
    }
}

#[cfg(test)]
mod callback_effects_tests {
    use super::*;

    #[test]
    fn apply_effects_records_the_native_sample_state_flags() {
        let mut sink = StateFlagsCallback::default();
        let effects = [
            Effect::SelectRow(65),
            Effect::Sample {
                row: 65,
                phase: 1.5_f32.to_bits(),
                integer: 1,
                state_flags: [17, 17],
            },
        ];

        apply_effects(&mut sink, &effects).unwrap();

        assert_eq!(sink.row, 65);
        assert_eq!(sink.state_flags, [17, 17]);
    }
}

/// Native polynomial approximation of atan(x), using the 4 single-precision
/// coefficients at 0x004b3160: [0x3e2bba25, 0x3d8a908b, 0x3d55033a, 0x3d6137ab].
pub fn native_atan(x: f32) -> f32 {
    let c0 = f32::from_bits(0x3e2bba25);
    let c1 = f32::from_bits(0x3d8a908b);
    let c2 = f32::from_bits(0x3d55033a);
    let c3 = f32::from_bits(0x3d6137ab);

    let x_abs = x.abs();
    let x2 = x * x;
    let t = if x_abs > 1.0 {
        1.0 / (x2 + 1.0)
    } else {
        x2 / (x2 + 1.0)
    };
    let poly = c0 + t * (c1 + t * (c2 + t * c3));
    let term = t.sqrt() * (1.0 + t * poly);
    let pi_half = f32::from_bits(0x3fc90fdb); // 1.5707964

    if x < -1.0 {
        term - pi_half
    } else if x > 1.0 {
        pi_half - term
    } else if x < 0.0 {
        -term
    } else {
        term
    }
}

/// Native look-angle / heading calculation FUN_0018ce78.
///
/// Computes integer heading in 0..1024 (10-bit wrap) from position and look target.
/// The angle in radians is scaled by 162.97466 (1024 / (2*PI)), offset by 0.5,
/// and converted to integer (truncation / cvt.w.s) before 10-bit masking.
pub fn native_heading(position: [f32; 3], target: [f32; 3]) -> i32 {
    let dx = position[0] - target[0];
    let dz = position[2] - target[2];
    let pi = f32::from_bits(0x40490fdb); // 3.1415927
    let pi_half = f32::from_bits(0x3fc90fdb); // 1.5707964

    let angle = if dz == 0.0 {
        if dx == 0.0 {
            0.0
        } else if dx >= 0.0 {
            pi_half
        } else {
            -pi_half
        }
    } else {
        let r = dx / dz;
        let mut a = native_atan(r);
        if dz < 0.0 {
            if dx > 0.0 {
                a += pi;
            } else {
                a -= pi;
            }
        }
        a
    };

    let scale = f32::from_bits(0x4322f983); // 162.97466
    let scaled = angle * scale + 0.5;
    (scaled as i32) & 0x3ff
}

/// Permutation table at 0x003cebd0 mapping slot index 0..3 to permutation [0, 2, 1].
pub const SLOT_PERMUTATION: [usize; 3] = [0, 2, 1];

/// Offense formation base table at 0x003cea20.
/// Slots are [X, Z] court coordinates:
/// - slot 0: [47.0, 16.0]
/// - slot 1: [35.0, 8.0]
/// - slot 2: [32.0, -10.0]
pub const OFFENSE_FORMATION: [[f32; 2]; 3] = [[47.0, 16.0], [35.0, 8.0], [32.0, -10.0]];

/// Defense formation base table at 0x003ceaf8.
/// Slots are [X, Z] court coordinates:
/// - slot 0: [12.0, 10.0]
/// - slot 1: [22.0, 12.0]
/// - slot 2: [-13.0, -10.0]
pub const DEFENSE_FORMATION: [[f32; 2]; 3] = [[12.0, 10.0], [22.0, 12.0], [-13.0, -10.0]];

/// A computed formation spot including 4D position, 4D look target, and 10-bit native heading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormationSpot {
    pub position: [f32; 4],
    pub target: [f32; 4],
    pub heading: i32,
}

/// Compute the formation spot for a player according to native FUN_0012c748.
pub fn formation_spot(possessing_team: usize, team: usize, slot: usize) -> FormationSpot {
    let perm = SLOT_PERMUTATION[slot % 3];
    let team_sign = if team == 0 { 1.0_f32 } else { -1.0_f32 };
    let is_offense = team == possessing_team;
    let (pos_xy, tgt_xy) = if is_offense {
        let sign = -team_sign;
        let pos = OFFENSE_FORMATION[perm];
        let tgt = if perm == 0 {
            OFFENSE_FORMATION[1]
        } else {
            OFFENSE_FORMATION[0]
        };
        ([sign * pos[0], pos[1]], [sign * tgt[0], tgt[1]])
    } else {
        let sign = team_sign;
        let pos = DEFENSE_FORMATION[perm];
        let tgt = OFFENSE_FORMATION[0];
        ([sign * pos[0], pos[1]], [sign * tgt[0], tgt[1]])
    };
    let position = [pos_xy[0], 0.0, pos_xy[1], 1.0];
    let target = [tgt_xy[0], 0.0, tgt_xy[1], 1.0];
    let heading = native_heading(
        [position[0], position[1], position[2]],
        [target[0], target[1], target[2]],
    );
    FormationSpot {
        position,
        target,
        heading,
    }
}

/// Returns the native FUN_001420e8 restart order as 6 entries:
/// `(actor_index, team, slot, spot)`.
///
/// Order is interleaved by slot: (0,0), (1,0), (0,1), (1,1), (0,2), (1,2).
pub fn restart_order(possessing_team: usize) -> [(usize, usize, usize, FormationSpot); 6] {
    let mut order = Vec::with_capacity(6);
    for slot in 0..3 {
        for team in [0, 1] {
            let actor_idx = team * 3 + slot;
            let spot = formation_spot(possessing_team, team, slot);
            order.push((actor_idx, team, slot, spot));
        }
    }
    order.try_into().expect("exactly 6 restart entries")
}

/// Reset and place all 6 actor projections in native FUN_001420e8 order,
/// returning the ball receiver actor index (`possessing_team * 3`).
pub fn place_restart(
    projections: &mut [Projection; 6],
    rng: &mut Rng,
    source: &impl Source,
    possessing_team: usize,
) -> Result<usize, Unsupported> {
    let order = restart_order(possessing_team);
    for (actor_idx, _team, _slot, spot) in order {
        let pos_u32 = spot.position.map(f32::to_bits);
        place(
            &mut projections[actor_idx],
            rng,
            source,
            pos_u32,
            spot.heading,
        )?;
    }
    Ok(possessing_team * 3)
}

#[cfg(test)]
#[path = "ordinary_tests.rs"]
mod tests;
