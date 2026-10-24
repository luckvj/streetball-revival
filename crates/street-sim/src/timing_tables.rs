//! The original's move-script clocks, read from `SLUS_206.51` (generated
//! from the disc; `tests/script_timing_disc.rs` regenerates and compares).
//! Every tick is updates from the script's start along the path the VM
//! takes when its callbacks return 0 (`0x84` falls through, `0x85` jumps):
//! blends (`8e`) and fixed waits (`8d`) carry the ticks, callbacks are
//! instant. See docs/TRACE.md "Shots, passes and catches on the script
//! clock".

/// A row of the pass table (`0x3d65e8`, 93 x 28 bytes, `FUN_0015f930`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassRow {
    pub script: u32,
    /// The row script's callback 38 (the release); 0 when the script has
    /// none on its plain path (the passout rows).
    pub release: u8,
    /// +0x14: bits 0..7 the receiver's sector, 0x100 running, 0x200
    /// standing, 0x400 airborne, 0x800/0x1000 not with the left/right
    /// hand, 0x8000 alley-oop, 0x10000 phase 6, 0x80000 back court with
    /// 9 or more on the clock, 0x100000 diving, 0x200000/0x400000 to
    /// oneself.
    pub flags: u32,
    /// +0x0c/+0x10: the distance band, -1 unbounded.
    pub min_dist: f32,
    pub max_dist: f32,
    /// +0x04: a legend record id the passer must be, -1 for anyone.
    pub legend: i16,
    /// +0x08: the Passing rating a computer passer needs; a person never
    /// draws a rated row without L3 held.
    pub min_rating: i16,
    /// +0x18: the kind left on the passer (+0x268).
    pub kind: u8,
}

pub const PASS_ROWS: [PassRow; 93] = [
    // 0: pass_bird
    PassRow {
        script: 0x454dfa,
        release: 16,
        flags: 0x201,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: 172,
        min_rating: -1,
        kind: 2,
    },
    // 1: pass_magic
    PassRow {
        script: 0x454ea6,
        release: 22,
        flags: 0x31e0,
        min_dist: 2.0,
        max_dist: 35.0,
        legend: 166,
        min_rating: -1,
        kind: 2,
    },
    // 2: pass_magic
    PassRow {
        script: 0x454ea6,
        release: 22,
        flags: 0x290e,
        min_dist: 2.0,
        max_dist: 35.0,
        legend: 166,
        min_rating: -1,
        kind: 2,
    },
    // 3: pass_pistol
    PassRow {
        script: 0x454f01,
        release: 22,
        flags: 0x3106,
        min_dist: 2.0,
        max_dist: 35.0,
        legend: 169,
        min_rating: -1,
        kind: 2,
    },
    // 4: pass_pistol
    PassRow {
        script: 0x454f01,
        release: 22,
        flags: 0x29c0,
        min_dist: 2.0,
        max_dist: 35.0,
        legend: 169,
        min_rating: -1,
        kind: 2,
    },
    // 5: ?
    PassRow {
        script: 0x454e50,
        release: 24,
        flags: 0x4210,
        min_dist: 8.0,
        max_dist: 30.0,
        legend: 160,
        min_rating: -1,
        kind: 4,
    },
    // 6: pass_dropkick
    PassRow {
        script: 0x454cd6,
        release: 26,
        flags: 0x238,
        min_dist: 19.5,
        max_dist: -1.0,
        legend: -1,
        min_rating: 80,
        kind: 5,
    },
    // 7: pass_knee
    PassRow {
        script: 0x454d3d,
        release: 32,
        flags: 0x238,
        min_dist: 8.0,
        max_dist: 20.5,
        legend: -1,
        min_rating: 80,
        kind: 5,
    },
    // 8: pass_sidekick
    PassRow {
        script: 0x454da4,
        release: 26,
        flags: 0x12c1,
        min_dist: 7.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: 80,
        kind: 5,
    },
    // 9: pass_sidekick
    PassRow {
        script: 0x454da4,
        release: 26,
        flags: 0xa07,
        min_dist: 7.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: 80,
        kind: 5,
    },
    // 10: pass_fwd_1
    PassRow {
        script: 0x4548b8,
        release: 18,
        flags: 0x102ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 11: pass_fwd_1
    PassRow {
        script: 0x4548b8,
        release: 18,
        flags: 0x238,
        min_dist: 5.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 12: pass_fwd_run_1
    PassRow {
        script: 0x45490e,
        release: 18,
        flags: 0x138,
        min_dist: 5.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 13: pass_fwd_2
    PassRow {
        script: 0x45496c,
        release: 20,
        flags: 0x238,
        min_dist: 15.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 14: pass_fwd_run_2
    PassRow {
        script: 0x4549c6,
        release: 15,
        flags: 0x138,
        min_dist: 5.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 15: pass_fwd_run_3
    PassRow {
        script: 0x454a25,
        release: 20,
        flags: 0x2138,
        min_dist: 20.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 16: pass_fwd_4
    PassRow {
        script: 0x454a75,
        release: 16,
        flags: 0xa20,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 0,
    },
    // 17: pass_fwd_4
    PassRow {
        script: 0x454a75,
        release: 16,
        flags: 0x1208,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 0,
    },
    // 18: pass_fwd_run_4
    PassRow {
        script: 0x454ae5,
        release: 16,
        flags: 0x940,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 0,
    },
    // 19: pass_fwd_run_4
    PassRow {
        script: 0x454ae5,
        release: 16,
        flags: 0x1104,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 0,
    },
    // 20: pass_fwd_5
    PassRow {
        script: 0x454b43,
        release: 21,
        flags: 0xa20,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 21: pass_fwd_5
    PassRow {
        script: 0x454b43,
        release: 21,
        flags: 0x1208,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 22: pass_fwd_run_5
    PassRow {
        script: 0x454baa,
        release: 21,
        flags: 0x920,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 23: pass_fwd_run_5
    PassRow {
        script: 0x454baa,
        release: 21,
        flags: 0x1108,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 24: pass_fwd_right_run_1
    PassRow {
        script: 0x454f5c,
        release: 46,
        flags: 0x829e0,
        min_dist: 9.0,
        max_dist: 45.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 25: pass_fwd_right_run_1
    PassRow {
        script: 0x454f5c,
        release: 46,
        flags: 0x8310e,
        min_dist: 9.0,
        max_dist: 45.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 26: pass_fwd_6
    PassRow {
        script: 0x454c08,
        release: 10,
        flags: 0x238,
        min_dist: 30.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 27: pass_fwd_7
    PassRow {
        script: 0x454c6f,
        release: 14,
        flags: 0x238,
        min_dist: 30.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 28: pass_left_or_right_1
    PassRow {
        script: 0x454fad,
        release: 17,
        flags: 0xa40,
        min_dist: -1.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 29: pass_left_or_right_run_1
    PassRow {
        script: 0x45500c,
        release: 17,
        flags: 0x940,
        min_dist: -1.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 30: pass_left_or_right_2
    PassRow {
        script: 0x45506a,
        release: 18,
        flags: 0xa40,
        min_dist: -1.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 31: pass_left_or_right_run_2
    PassRow {
        script: 0x4550ca,
        release: 23,
        flags: 0x940,
        min_dist: -1.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 32: pass_left_or_right_3
    PassRow {
        script: 0x455128,
        release: 21,
        flags: 0xa40,
        min_dist: 7.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 33: pass_left_or_right_run_3
    PassRow {
        script: 0x45517e,
        release: 21,
        flags: 0x940,
        min_dist: 7.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 34: pass_left_or_right_4
    PassRow {
        script: 0x4551dc,
        release: 31,
        flags: 0x1280,
        min_dist: -1.0,
        max_dist: 55.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 35: pass_left_or_right_run_4
    PassRow {
        script: 0x45523b,
        release: 31,
        flags: 0x1180,
        min_dist: -1.0,
        max_dist: 55.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 36: pass_left_or_right_5
    PassRow {
        script: 0x455299,
        release: 24,
        flags: 0x7106,
        min_dist: -1.0,
        max_dist: 30.0,
        legend: -1,
        min_rating: -1,
        kind: 3,
    },
    // 37: pass_left_or_right_run_6
    PassRow {
        script: 0x4552f8,
        release: 31,
        flags: 0x11c0,
        min_dist: -1.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 38: pass_left_or_right_run_6a
    PassRow {
        script: 0x455356,
        release: 18,
        flags: 0x1106,
        min_dist: -1.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 39: pass_left_or_right_1
    PassRow {
        script: 0x454fad,
        release: 17,
        flags: 0x1206,
        min_dist: -1.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 40: pass_left_or_right_run_1
    PassRow {
        script: 0x45500c,
        release: 17,
        flags: 0x1106,
        min_dist: -1.0,
        max_dist: 20.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 41: pass_left_or_right_2
    PassRow {
        script: 0x45506a,
        release: 18,
        flags: 0x1204,
        min_dist: -1.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 42: pass_left_or_right_run_2
    PassRow {
        script: 0x4550ca,
        release: 23,
        flags: 0x1104,
        min_dist: -1.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 43: pass_left_or_right_3
    PassRow {
        script: 0x455128,
        release: 21,
        flags: 0x1204,
        min_dist: 7.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 44: pass_left_or_right_run_3
    PassRow {
        script: 0x45517e,
        release: 21,
        flags: 0x1104,
        min_dist: 7.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 45: pass_left_or_right_4
    PassRow {
        script: 0x4551dc,
        release: 31,
        flags: 0xa02,
        min_dist: -1.0,
        max_dist: 55.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 46: pass_left_or_right_run_4
    PassRow {
        script: 0x45523b,
        release: 31,
        flags: 0x902,
        min_dist: -1.0,
        max_dist: 55.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 47: pass_left_or_right_5
    PassRow {
        script: 0x455299,
        release: 24,
        flags: 0x69c0,
        min_dist: -1.0,
        max_dist: 30.0,
        legend: -1,
        min_rating: -1,
        kind: 3,
    },
    // 48: pass_left_or_right_run_6
    PassRow {
        script: 0x4552f8,
        release: 31,
        flags: 0x906,
        min_dist: -1.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 49: pass_left_or_right_run_6a
    PassRow {
        script: 0x455356,
        release: 18,
        flags: 0x9c0,
        min_dist: -1.0,
        max_dist: 35.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 50: pass_behind_1
    PassRow {
        script: 0x4553b4,
        release: 20,
        flags: 0xa81,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 51: pass_behind_1
    PassRow {
        script: 0x4553b4,
        release: 20,
        flags: 0x1203,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 52: pass_behind_run_1
    PassRow {
        script: 0x455413,
        release: 20,
        flags: 0x981,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 53: pass_behind_run_1
    PassRow {
        script: 0x455413,
        release: 20,
        flags: 0x1103,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 54: pass_behind_2
    PassRow {
        script: 0x455471,
        release: 24,
        flags: 0x4201,
        min_dist: 23.0,
        max_dist: 30.0,
        legend: -1,
        min_rating: -1,
        kind: 3,
    },
    // 55: pass_behind_3
    PassRow {
        script: 0x4554cd,
        release: 24,
        flags: 0x6101,
        min_dist: 2.0,
        max_dist: 15.0,
        legend: -1,
        min_rating: -1,
        kind: 3,
    },
    // 56: pass_behind_5
    PassRow {
        script: 0x455523,
        release: 28,
        flags: 0x201,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 57: pass_behind_run_5
    PassRow {
        script: 0x455583,
        release: 26,
        flags: 0x101,
        min_dist: -1.0,
        max_dist: 50.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 58: pass_allyoop_1
    PassRow {
        script: 0x4555e1,
        release: 12,
        flags: 0x82ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 59: pass_allyoop_run_1
    PassRow {
        script: 0x45564d,
        release: 12,
        flags: 0x81ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 60: pass_post_1
    PassRow {
        script: 0x4556ca,
        release: 20,
        flags: 0x238,
        min_dist: 10.0,
        max_dist: 25.0,
        legend: -1,
        min_rating: -1,
        kind: 1,
    },
    // 61: pass_air_1
    PassRow {
        script: 0x455740,
        release: 14,
        flags: 0x8444,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 62: pass_air_2
    PassRow {
        script: 0x4557d5,
        release: 15,
        flags: 0x8482,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 63: pass_air_4
    PassRow {
        script: 0x45586a,
        release: 18,
        flags: 0x8482,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 64: pass_air_5
    PassRow {
        script: 0x4558ff,
        release: 23,
        flags: 0x8444,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 65: pass_air_6
    PassRow {
        script: 0x455994,
        release: 22,
        flags: 0x8428,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 66: pass_air_7
    PassRow {
        script: 0x455a29,
        release: 21,
        flags: 0x8410,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 67: pass_air_8
    PassRow {
        script: 0x455a9f,
        release: 19,
        flags: 0x8410,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 68: pass_air_9
    PassRow {
        script: 0x455b21,
        release: 22,
        flags: 0x8401,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 69: pass_air_1
    PassRow {
        script: 0x455740,
        release: 14,
        flags: 0x444,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 70: pass_air_2
    PassRow {
        script: 0x4557d5,
        release: 15,
        flags: 0x482,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 71: pass_air_4
    PassRow {
        script: 0x45586a,
        release: 18,
        flags: 0x482,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 72: pass_air_5
    PassRow {
        script: 0x4558ff,
        release: 23,
        flags: 0x444,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 73: pass_air_6
    PassRow {
        script: 0x455994,
        release: 22,
        flags: 0x428,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 74: pass_air_7
    PassRow {
        script: 0x455a29,
        release: 21,
        flags: 0x410,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 75: pass_air_8
    PassRow {
        script: 0x455a9f,
        release: 19,
        flags: 0x410,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 76: pass_air_9
    PassRow {
        script: 0x455b21,
        release: 22,
        flags: 0x401,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 2,
    },
    // 77: divepass_air_1
    PassRow {
        script: 0x455bb6,
        release: 6,
        flags: 0x10046c,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 78: divepass_air_2
    PassRow {
        script: 0x455c46,
        release: 4,
        flags: 0x100410,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 79: divepass_air_3
    PassRow {
        script: 0x455cb7,
        release: 6,
        flags: 0x100483,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 80: divepass_butt_1
    PassRow {
        script: 0x455d59,
        release: 12,
        flags: 0x1000ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 81: divepass_butt_2
    PassRow {
        script: 0x455da5,
        release: 10,
        flags: 0x1000ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 82: divepass_butt_3
    PassRow {
        script: 0x455df1,
        release: 12,
        flags: 0x1000ff,
        min_dist: -1.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 83: passout_overshoulder
    PassRow {
        script: 0x455e3d,
        release: 0,
        flags: 0x2000ff,
        min_dist: -1.0,
        max_dist: 8.5,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 84: passout_overshoulder
    PassRow {
        script: 0x455e3d,
        release: 0,
        flags: 0x2000ff,
        min_dist: -1.0,
        max_dist: 8.5,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 85: passout_close
    PassRow {
        script: 0x456031,
        release: 0,
        flags: 0x2000ff,
        min_dist: 7.5,
        max_dist: 24.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 86: passout_travelling
    PassRow {
        script: 0x4560ae,
        release: 0,
        flags: 0x2000ff,
        min_dist: 7.5,
        max_dist: 24.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 87: passout_overshoulder
    PassRow {
        script: 0x455e3d,
        release: 0,
        flags: 0x2000ff,
        min_dist: 23.5,
        max_dist: 35.5,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 88: passout_spin_1
    PassRow {
        script: 0x45612b,
        release: 0,
        flags: 0x2040ff,
        min_dist: 34.5,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 89: passout_spin_2
    PassRow {
        script: 0x4561a8,
        release: 0,
        flags: 0x2040ff,
        min_dist: 34.5,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 90: passout_bounce
    PassRow {
        script: 0x455eba,
        release: 0,
        flags: 0x2040ff,
        min_dist: 60.0,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 91: passout_1
    PassRow {
        script: 0x456214,
        release: 14,
        flags: 0x4000ff,
        min_dist: -1.0,
        max_dist: 25.5,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
    // 92: passout_2
    PassRow {
        script: 0x45627b,
        release: 26,
        flags: 0x4000ff,
        min_dist: 24.5,
        max_dist: -1.0,
        legend: -1,
        min_rating: -1,
        kind: 4,
    },
];

/// A shot script: `release` its callback 21 (the ball leaves, `FUN_001729f8`),
/// `check` its callbacks 17 and 20 (the fadeaway and the pump fake are
/// decided there, `FUN_001726c0`, `FUN_00172348`; 0 when it has none),
/// `end` its final opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotScript {
    pub name: &'static str,
    pub script: u32,
    /// Callback 21 on the held path: the ball leaves (a computer's shot,
    /// or a person's with the button up by the gate).
    pub release: u8,
    /// The `84`/`85` of callbacks 17 and 20 (the fadeaway and the pump
    /// fake are decided there); 0 when the script has none.
    pub check: u8,
    pub end: u8,
    /// The release gate `86 19` (callback 19, `FUN_00174c10`): from this
    /// tick a person's script waits, an update at a time, while the shoot
    /// button is still held; 0 when the script has none. The ball leaves
    /// `release - gate` updates after the gate opens.
    pub gate: u8,
    /// The update after the press the gate opens by itself: the script's
    /// jump (`9f`, `a1`, or game callback 18 for the side fades,
    /// `FUN_00172788`: 15 / fps up) lands `FUN_0019bed8` updates later and
    /// the gate stops waiting 15 updates before the landing for the 26
    /// jump-shot clips of `0x3d9218`, 5 for any other clip. 0 for a script
    /// without a gate.
    pub latest: u8,
    /// The check carries callback 17 (`84 17`): the stick away from the
    /// basket turns the shot into `jumpshot_1_fadeaway` there.
    pub fades: bool,
}

/// The 27 shot scripts of `0x3d8fd8` (ids 0..26), then the side fades
/// `0x44facb`, `0x44fb78`, `0x44fba5`, `0x44fc52` (27..30), the heave
/// `0x44f622` (31) and the gamebreaker shots `0x44f785`, `0x44f6c3`,
/// `0x44f7f8`, `0x44f89e` (32..35), all picked by `FUN_0016b4b8`.
pub const SHOT_SCRIPTS: [ShotScript; 36] = [
    ShotScript {
        name: "jumpshot_1",
        script: 0x44e51e,
        release: 30,
        check: 15,
        end: 40,
        gate: 26,
        latest: 47,
        fades: true,
    }, // 0
    ShotScript {
        name: "jumpshot_2",
        script: 0x44e605,
        release: 41,
        check: 0,
        end: 49,
        gate: 35,
        latest: 41,
        fades: false,
    }, // 1
    ShotScript {
        name: "jumpshot_360",
        script: 0x44e6a7,
        release: 41,
        check: 0,
        end: 50,
        gate: 35,
        latest: 43,
        fades: false,
    }, // 2
    ShotScript {
        name: "jumpshot_4",
        script: 0x44e763,
        release: 34,
        check: 14,
        end: 46,
        gate: 30,
        latest: 44,
        fades: true,
    }, // 3
    ShotScript {
        name: "jumpshot_5",
        script: 0x44ed73,
        release: 42,
        check: 28,
        end: 49,
        gate: 38,
        latest: 54,
        fades: true,
    }, // 4
    ShotScript {
        name: "jumpshot_6",
        script: 0x44ee5a,
        release: 37,
        check: 12,
        end: 50,
        gate: 33,
        latest: 45,
        fades: true,
    }, // 5
    ShotScript {
        name: "jumpshot_7",
        script: 0x44ef45,
        release: 38,
        check: 15,
        end: 54,
        gate: 33,
        latest: 41,
        fades: true,
    }, // 6
    ShotScript {
        name: "jumpshot_8",
        script: 0x44f02c,
        release: 31,
        check: 15,
        end: 42,
        gate: 26,
        latest: 40,
        fades: true,
    }, // 7
    ShotScript {
        name: "jumpshot_hook_1",
        script: 0x44f138,
        release: 32,
        check: 0,
        end: 46,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 8
    ShotScript {
        name: "jumpshot_hook_2",
        script: 0x44f1de,
        release: 27,
        check: 0,
        end: 40,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 9
    ShotScript {
        name: "jumpshot_hook_3",
        script: 0x44f284,
        release: 29,
        check: 0,
        end: 42,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 10
    ShotScript {
        name: "jumpshot_hook_bishop",
        script: 0x44f346,
        release: 30,
        check: 0,
        end: 37,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 11
    ShotScript {
        name: "jumpshot_hook_bishop_2",
        script: 0x44f3e8,
        release: 32,
        check: 0,
        end: 41,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 12
    ShotScript {
        name: "jumpshot_hook_magic",
        script: 0x44f564,
        release: 32,
        check: 0,
        end: 41,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 13
    ShotScript {
        name: "jumpshot_hook_wilt",
        script: 0x44f4a6,
        release: 32,
        check: 0,
        end: 41,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 14
    ShotScript {
        name: "jumpshot_1_fadeaway",
        script: 0x44f945,
        release: 19,
        check: 0,
        end: 35,
        gate: 15,
        latest: 30,
        fades: false,
    }, // 15
    ShotScript {
        name: "jumpshot_2_fadeaway",
        script: 0x44f9ec,
        release: 44,
        check: 24,
        end: 59,
        gate: 41,
        latest: 69,
        fades: false,
    }, // 16
    ShotScript {
        name: "jumpshot_runner_1",
        script: 0x44fc7f,
        release: 24,
        check: 0,
        end: 31,
        gate: 20,
        latest: 40,
        fades: false,
    }, // 17
    ShotScript {
        name: "jumpshot_2hand_legkick_1",
        script: 0x44e846,
        release: 34,
        check: 14,
        end: 45,
        gate: 30,
        latest: 44,
        fades: true,
    }, // 18
    ShotScript {
        name: "jumpshot_2hand_legkick_2",
        script: 0x44e92d,
        release: 34,
        check: 13,
        end: 49,
        gate: 32,
        latest: 46,
        fades: true,
    }, // 19
    ShotScript {
        name: "jumpshot_stepback_2hand_legkick",
        script: 0x44ea18,
        release: 33,
        check: 12,
        end: 50,
        gate: 31,
        latest: 45,
        fades: true,
    }, // 20
    ShotScript {
        name: "jumpshot_female_2hand_set",
        script: 0x44eba5,
        release: 15,
        check: 12,
        end: 26,
        gate: 0,
        latest: 0,
        fades: true,
    }, // 21
    ShotScript {
        name: "jumpshot_female_2hand_stepforward_set",
        script: 0x44eb03,
        release: 28,
        check: 0,
        end: 43,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 22
    ShotScript {
        name: "jumpshot_bird",
        script: 0x44ec83,
        release: 28,
        check: 11,
        end: 41,
        gate: 24,
        latest: 47,
        fades: true,
    }, // 23
    ShotScript {
        name: "jumpshot_hook_1",
        script: 0x44f138,
        release: 32,
        check: 0,
        end: 46,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 24
    ShotScript {
        name: "jumpshot_hook_2",
        script: 0x44f1de,
        release: 27,
        check: 0,
        end: 40,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 25
    ShotScript {
        name: "jumpshot_hook_3",
        script: 0x44f284,
        release: 29,
        check: 0,
        end: 42,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 26
    ShotScript {
        name: "jumpshot_sidefade_right_r",
        script: 0x44facb,
        release: 39,
        check: 0,
        end: 58,
        gate: 33,
        latest: 45,
        fades: false,
    }, // 27
    ShotScript {
        name: "jumpshot_sidefade_left_l",
        script: 0x44fb78,
        release: 39,
        check: 0,
        end: 58,
        gate: 33,
        latest: 45,
        fades: false,
    }, // 28
    ShotScript {
        name: "jumpshot_sidefade_right_l",
        script: 0x44fba5,
        release: 32,
        check: 0,
        end: 49,
        gate: 28,
        latest: 50,
        fades: false,
    }, // 29
    ShotScript {
        name: "jumpshot_sidefade_left_r",
        script: 0x44fc52,
        release: 32,
        check: 0,
        end: 49,
        gate: 28,
        latest: 50,
        fades: false,
    }, // 30
    ShotScript {
        name: "heaveshot_1",
        script: 0x44f622,
        release: 19,
        check: 8,
        end: 70,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 31
    ShotScript {
        name: "baseballshot_gb",
        script: 0x44f785,
        release: 48,
        check: 0,
        end: 98,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 32
    ShotScript {
        name: "heaveshot_gb",
        script: 0x44f6c3,
        release: 24,
        check: 0,
        end: 40,
        gate: 0,
        latest: 0,
        fades: false,
    }, // 33
    ShotScript {
        name: "jumpshot_gb_longrange",
        script: 0x44f7f8,
        release: 33,
        check: 0,
        end: 46,
        gate: 29,
        latest: 49,
        fades: false,
    }, // 34
    ShotScript {
        name: "jumpshot_gb_shortrange",
        script: 0x44f89e,
        release: 33,
        check: 0,
        end: 44,
        gate: 29,
        latest: 49,
        fades: false,
    }, // 35
];

/// A row of the dunk and lay-up candidate table (`0x3d7b28`, 99 x 20
/// bytes, walked by `FUN_001693b8`): `family` 0 a dunk, 2 a lay-up; `id`
/// the dunk id (`player+0x1e8`) or the lay-up kind (`player+0x26c`);
/// `mask` bits 0..7 the approach sector seen from the rim, 0x100/0x200/
/// 0x400/0x800 the distance bands (near, mid, far), 0x20000/0x40000 not
/// with the right/left hand, 0x80000 and 0x400000 the Dunks-rating gates,
/// 0x100000 an alley-oop finish; `release` callback 21, `end` the last
/// opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveRow {
    pub name: &'static str,
    pub script: u32,
    pub legend: i16,
    pub mask: u32,
    pub family: u8,
    pub id: u8,
    pub release: u8,
    pub end: u8,
}

pub const MOVE_ROWS: [MoveRow; 99] = [
    MoveRow {
        name: "dunk_32",
        script: 0x44960a,
        legend: 175,
        mask: 0x3ff,
        family: 0,
        id: 29,
        release: 54,
        end: 68,
    }, // 0
    MoveRow {
        name: "dunk_1h_1f_drexler",
        script: 0x44a05f,
        legend: 173,
        mask: 0x438,
        family: 0,
        id: 43,
        release: 54,
        end: 63,
    }, // 1
    MoveRow {
        name: "dunk_1h_1f_hawkins",
        script: 0x449d18,
        legend: 159,
        mask: 0x6ff,
        family: 0,
        id: 44,
        release: 59,
        end: 70,
    }, // 2
    MoveRow {
        name: "dunk_1h_2f_dawkins",
        script: 0x449dee,
        legend: 181,
        mask: 0x3ff,
        family: 0,
        id: 45,
        release: 57,
        end: 70,
    }, // 3
    MoveRow {
        name: "dunk_1h_2f_jordan",
        script: 0x449ec8,
        legend: 167,
        mask: 0x6ff,
        family: 0,
        id: 46,
        release: 56,
        end: 68,
    }, // 4
    MoveRow {
        name: "dunk_1h_2f_jordan",
        script: 0x449ec8,
        legend: 168,
        mask: 0x6ff,
        family: 0,
        id: 46,
        release: 56,
        end: 68,
    }, // 5
    MoveRow {
        name: "dunk_1h_2f_wilt",
        script: 0x449f89,
        legend: 178,
        mask: 0x1ff,
        family: 0,
        id: 47,
        release: 57,
        end: 73,
    }, // 6
    MoveRow {
        name: "layup_drj",
        script: 0x44718e,
        legend: 158,
        mask: 0xc040c,
        family: 2,
        id: 23,
        release: 46,
        end: 55,
    }, // 7
    MoveRow {
        name: "layup_drj",
        script: 0x44718e,
        legend: 158,
        mask: 0xa0460,
        family: 2,
        id: 23,
        release: 46,
        end: 55,
    }, // 8
    MoveRow {
        name: "layup_barry",
        script: 0x4474ba,
        legend: 162,
        mask: 0x80438,
        family: 2,
        id: 24,
        release: 34,
        end: 41,
    }, // 9
    MoveRow {
        name: "layup_iceman",
        script: 0x447266,
        legend: 170,
        mask: 0xc060c,
        family: 2,
        id: 25,
        release: 46,
        end: 55,
    }, // 10
    MoveRow {
        name: "layup_iceman",
        script: 0x447266,
        legend: 170,
        mask: 0xa0660,
        family: 2,
        id: 25,
        release: 46,
        end: 55,
    }, // 11
    MoveRow {
        name: "layup_pee_wee",
        script: 0x44733e,
        legend: 174,
        mask: 0x804ff,
        family: 2,
        id: 26,
        release: 47,
        end: 54,
    }, // 12
    MoveRow {
        name: "layup_tiny",
        script: 0x44740a,
        legend: 180,
        mask: 0x80438,
        family: 2,
        id: 27,
        release: 46,
        end: 53,
    }, // 13
    MoveRow {
        name: "dunk_34",
        script: 0x4496e6,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 25,
        release: 59,
        end: 68,
    }, // 14
    MoveRow {
        name: "dunk_35",
        script: 0x4497e6,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 26,
        release: 63,
        end: 82,
    }, // 15
    MoveRow {
        name: "dunk_36",
        script: 0x4498a7,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 27,
        release: 57,
        end: 74,
    }, // 16
    MoveRow {
        name: "dunk_37",
        script: 0x449964,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 28,
        release: 56,
        end: 102,
    }, // 17
    MoveRow {
        name: "dunk_26",
        script: 0x4493f6,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 8,
        release: 59,
        end: 68,
    }, // 18
    MoveRow {
        name: "dunk_25",
        script: 0x449320,
        legend: -1,
        mask: 0x16c,
        family: 0,
        id: 2,
        release: 59,
        end: 74,
    }, // 19
    MoveRow {
        name: "dunk_24",
        script: 0x44924a,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 17,
        release: 57,
        end: 70,
    }, // 20
    MoveRow {
        name: "dunk_17",
        script: 0x448cfc,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 7,
        release: 60,
        end: 83,
    }, // 21
    MoveRow {
        name: "dunk_13",
        script: 0x448b09,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 10,
        release: 57,
        end: 77,
    }, // 22
    MoveRow {
        name: "dunk_11",
        script: 0x448900,
        legend: -1,
        mask: 0x1ff,
        family: 0,
        id: 13,
        release: 47,
        end: 74,
    }, // 23
    MoveRow {
        name: "dunk_10",
        script: 0x44880a,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 12,
        release: 57,
        end: 119,
    }, // 24
    MoveRow {
        name: "dunk_9",
        script: 0x44871f,
        legend: -1,
        mask: 0x2ff,
        family: 0,
        id: 14,
        release: 52,
        end: 66,
    }, // 25
    MoveRow {
        name: "dunk_7",
        script: 0x448562,
        legend: -1,
        mask: 0x3ff,
        family: 0,
        id: 18,
        release: 45,
        end: 58,
    }, // 26
    MoveRow {
        name: "dunk_8",
        script: 0x44861f,
        legend: -1,
        mask: 0x2ff,
        family: 0,
        id: 16,
        release: 56,
        end: 78,
    }, // 27
    MoveRow {
        name: "dunk_6",
        script: 0x448486,
        legend: -1,
        mask: 0x638,
        family: 0,
        id: 19,
        release: 60,
        end: 72,
    }, // 28
    MoveRow {
        name: "dunk_5",
        script: 0x448377,
        legend: -1,
        mask: 0x16c,
        family: 0,
        id: 22,
        release: 42,
        end: 58,
    }, // 29
    MoveRow {
        name: "dunk_4",
        script: 0x4482a1,
        legend: -1,
        mask: 0x16c,
        family: 0,
        id: 20,
        release: 55,
        end: 70,
    }, // 30
    MoveRow {
        name: "dunk_3",
        script: 0x4481ac,
        legend: -1,
        mask: 0x138,
        family: 0,
        id: 21,
        release: 55,
        end: 84,
    }, // 31
    MoveRow {
        name: "dunk_2",
        script: 0x4480e6,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 23,
        release: 51,
        end: 77,
    }, // 32
    MoveRow {
        name: "dunk_1",
        script: 0x448025,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 24,
        release: 59,
        end: 78,
    }, // 33
    MoveRow {
        name: "dunk_28",
        script: 0x4494d0,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 1,
        release: 62,
        end: 75,
    }, // 34
    MoveRow {
        name: "dunk_23",
        script: 0x449189,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 3,
        release: 49,
        end: 77,
    }, // 35
    MoveRow {
        name: "dunk_18",
        script: 0x448e2c,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 6,
        release: 66,
        end: 93,
    }, // 36
    MoveRow {
        name: "dunk_12",
        script: 0x448a05,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 11,
        release: 54,
        end: 116,
    }, // 37
    MoveRow {
        name: "bbpass_1",
        script: 0x447b53,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 30,
        release: 0,
        end: 10,
    }, // 38
    MoveRow {
        name: "dunk_30",
        script: 0x44956f,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 0,
        release: 61,
        end: 74,
    }, // 39
    MoveRow {
        name: "dunk_14",
        script: 0x448be3,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 9,
        release: 53,
        end: 69,
    }, // 40
    MoveRow {
        name: "dunk_19",
        script: 0x448eed,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 15,
        release: 57,
        end: 68,
    }, // 41
    MoveRow {
        name: "dunk_20",
        script: 0x448faa,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 5,
        release: 53,
        end: 78,
    }, // 42
    MoveRow {
        name: "dunk_21",
        script: 0x4490c8,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 4,
        release: 61,
        end: 76,
    }, // 43
    MoveRow {
        name: "dunk_1h_2f_tomahawk",
        script: 0x449a39,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 31,
        release: 57,
        end: 68,
    }, // 44
    MoveRow {
        name: "dunk_2h_2f_rev_hang_tapglass",
        script: 0x449afa,
        legend: -1,
        mask: 0x144,
        family: 0,
        id: 32,
        release: 51,
        end: 77,
    }, // 45
    MoveRow {
        name: "dunk_1h_1f_spread_eagle",
        script: 0x449c5b,
        legend: -1,
        mask: 0x638,
        family: 0,
        id: 33,
        release: 57,
        end: 70,
    }, // 46
    MoveRow {
        name: "dunk_1h_2f_180_tapglass",
        script: 0x44a870,
        legend: -1,
        mask: 0x144,
        family: 0,
        id: 34,
        release: 59,
        end: 70,
    }, // 47
    MoveRow {
        name: "dunk_2h_2f_windmill_hang",
        script: 0x44a95d,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 35,
        release: 57,
        end: 119,
    }, // 48
    MoveRow {
        name: "dunk_2h_2f_360_hang",
        script: 0x44aa5c,
        legend: -1,
        mask: 0x844,
        family: 0,
        id: 36,
        release: 59,
        end: 121,
    }, // 49
    MoveRow {
        name: "dunk_1h_2f_spike",
        script: 0x44ab5f,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 37,
        release: 57,
        end: 73,
    }, // 50
    MoveRow {
        name: "dunk_2h_2f_180_hang",
        script: 0x44ac3e,
        legend: -1,
        mask: 0x844,
        family: 0,
        id: 38,
        release: 57,
        end: 74,
    }, // 51
    MoveRow {
        name: "dunk_1h_2f_spike_hang",
        script: 0x44ad51,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 39,
        release: 57,
        end: 69,
    }, // 52
    MoveRow {
        name: "dunk_2h_2f_hang",
        script: 0x44ae55,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 40,
        release: 57,
        end: 76,
    }, // 53
    MoveRow {
        name: "dunk_2h_2f_hang_swing",
        script: 0x44af68,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 41,
        release: 57,
        end: 119,
    }, // 54
    MoveRow {
        name: "dunk_2h_2f_rev_hang",
        script: 0x44b070,
        legend: -1,
        mask: 0x1ff,
        family: 0,
        id: 42,
        release: 57,
        end: 119,
    }, // 55
    MoveRow {
        name: "bbpass_1",
        script: 0x447b53,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 48,
        release: 0,
        end: 10,
    }, // 56
    MoveRow {
        name: "bbpass_1",
        script: 0x447b53,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 49,
        release: 0,
        end: 10,
    }, // 57
    MoveRow {
        name: "bbpass_1",
        script: 0x447b53,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 50,
        release: 0,
        end: 10,
    }, // 58
    MoveRow {
        name: "bbpass_1",
        script: 0x447b53,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 51,
        release: 0,
        end: 10,
    }, // 59
    MoveRow {
        name: "dunk_1h_2f_360_spread_eagle",
        script: 0x44a11c,
        legend: -1,
        mask: 0x638,
        family: 0,
        id: 52,
        release: 65,
        end: 83,
    }, // 60
    MoveRow {
        name: "dunk_1h_2f_360_thrulegs",
        script: 0x44a1f2,
        legend: -1,
        mask: 0x144,
        family: 0,
        id: 53,
        release: 65,
        end: 76,
    }, // 61
    MoveRow {
        name: "dunk_1h_1f_tomahawk",
        script: 0x44a2f2,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 54,
        release: 57,
        end: 71,
    }, // 62
    MoveRow {
        name: "dunk_2h_1f_hang",
        script: 0x44a3c8,
        legend: -1,
        mask: 0x6ff,
        family: 0,
        id: 55,
        release: 57,
        end: 119,
    }, // 63
    MoveRow {
        name: "dunk_1h_2f_thrulegs_elbowhang",
        script: 0x44a4c7,
        legend: -1,
        mask: 0x138,
        family: 0,
        id: 56,
        release: 57,
        end: 83,
    }, // 64
    MoveRow {
        name: "dunk_2h_2f_thrulegs_revhang",
        script: 0x44a5cd,
        legend: -1,
        mask: 0x3ff,
        family: 0,
        id: 57,
        release: 54,
        end: 75,
    }, // 65
    MoveRow {
        name: "dunk_1h_2f_thrulegs_windmill",
        script: 0x44a6dd,
        legend: -1,
        mask: 0x3ff,
        family: 0,
        id: 58,
        release: 60,
        end: 74,
    }, // 66
    MoveRow {
        name: "dunk_2h_2f_touchheel_spread_eagle",
        script: 0x44a7b3,
        legend: -1,
        mask: 0x8ff,
        family: 0,
        id: 59,
        release: 61,
        end: 75,
    }, // 67
    MoveRow {
        name: "layup_1",
        script: 0x4461d1,
        legend: -1,
        mask: 0x4c063c,
        family: 2,
        id: 0,
        release: 54,
        end: 65,
    }, // 68
    MoveRow {
        name: "layup_1",
        script: 0x4461d1,
        legend: -1,
        mask: 0x4a0678,
        family: 2,
        id: 0,
        release: 54,
        end: 65,
    }, // 69
    MoveRow {
        name: "layup_2",
        script: 0x446285,
        legend: -1,
        mask: 0x4806ff,
        family: 2,
        id: 3,
        release: 57,
        end: 67,
    }, // 70
    MoveRow {
        name: "layup_3",
        script: 0x446339,
        legend: -1,
        mask: 0x48046c,
        family: 2,
        id: 5,
        release: 49,
        end: 62,
    }, // 71
    MoveRow {
        name: "layup_4",
        script: 0x446488,
        legend: -1,
        mask: 0x4c0640,
        family: 2,
        id: 7,
        release: 50,
        end: 61,
    }, // 72
    MoveRow {
        name: "layup_4",
        script: 0x446488,
        legend: -1,
        mask: 0x4a0604,
        family: 2,
        id: 7,
        release: 50,
        end: 61,
    }, // 73
    MoveRow {
        name: "layup_5",
        script: 0x446565,
        legend: -1,
        mask: 0x4c060c,
        family: 2,
        id: 11,
        release: 45,
        end: 56,
    }, // 74
    MoveRow {
        name: "layup_5",
        script: 0x446565,
        legend: -1,
        mask: 0x4a0660,
        family: 2,
        id: 11,
        release: 45,
        end: 56,
    }, // 75
    MoveRow {
        name: "layup_6",
        script: 0x44663e,
        legend: -1,
        mask: 0x4a060c,
        family: 2,
        id: 12,
        release: 49,
        end: 56,
    }, // 76
    MoveRow {
        name: "layup_6",
        script: 0x44663e,
        legend: -1,
        mask: 0x4c0660,
        family: 2,
        id: 12,
        release: 49,
        end: 56,
    }, // 77
    MoveRow {
        name: "layup_7",
        script: 0x44671b,
        legend: -1,
        mask: 0x4802ff,
        family: 2,
        id: 8,
        release: 39,
        end: 46,
    }, // 78
    MoveRow {
        name: "layup_8",
        script: 0x4467f4,
        legend: -1,
        mask: 0x4806ff,
        family: 2,
        id: 14,
        release: 43,
        end: 52,
    }, // 79
    MoveRow {
        name: "layup_9",
        script: 0x4468a5,
        legend: -1,
        mask: 0x808ff,
        family: 2,
        id: 17,
        release: 41,
        end: 44,
    }, // 80
    MoveRow {
        name: "layup_10",
        script: 0x446956,
        legend: -1,
        mask: 0x808ff,
        family: 2,
        id: 1,
        release: 42,
        end: 60,
    }, // 81
    MoveRow {
        name: "layup_11",
        script: 0x446a0a,
        legend: -1,
        mask: 0x808ff,
        family: 2,
        id: 16,
        release: 39,
        end: 60,
    }, // 82
    MoveRow {
        name: "layup_12",
        script: 0x446ada,
        legend: -1,
        mask: 0x4c043c,
        family: 2,
        id: 2,
        release: 69,
        end: 84,
    }, // 83
    MoveRow {
        name: "layup_12",
        script: 0x446ada,
        legend: -1,
        mask: 0x4a0478,
        family: 2,
        id: 2,
        release: 69,
        end: 84,
    }, // 84
    MoveRow {
        name: "layup_13",
        script: 0x446bba,
        legend: -1,
        mask: 0x806ff,
        family: 2,
        id: 6,
        release: 44,
        end: 53,
    }, // 85
    MoveRow {
        name: "layup_14",
        script: 0x446c97,
        legend: -1,
        mask: 0x46c,
        family: 2,
        id: 18,
        release: 56,
        end: 63,
    }, // 86
    MoveRow {
        name: "layup_15",
        script: 0x446d47,
        legend: -1,
        mask: 0x480438,
        family: 2,
        id: 15,
        release: 75,
        end: 82,
    }, // 87
    MoveRow {
        name: "layup_boardslap",
        script: 0x446dea,
        legend: -1,
        mask: 0x4c0304,
        family: 2,
        id: 4,
        release: 31,
        end: 44,
    }, // 88
    MoveRow {
        name: "layup_boardslap",
        script: 0x446dea,
        legend: -1,
        mask: 0x4a0340,
        family: 2,
        id: 4,
        release: 31,
        end: 44,
    }, // 89
    MoveRow {
        name: "layup_fingerroll",
        script: 0x446e96,
        legend: -1,
        mask: 0x4806ff,
        family: 2,
        id: 13,
        release: 44,
        end: 55,
    }, // 90
    MoveRow {
        name: "layup_upandunder_1",
        script: 0x446f46,
        legend: -1,
        mask: 0x48036c,
        family: 2,
        id: 9,
        release: 41,
        end: 51,
    }, // 91
    MoveRow {
        name: "layup_upandunder_2",
        script: 0x447012,
        legend: -1,
        mask: 0x48036c,
        family: 2,
        id: 10,
        release: 42,
        end: 51,
    }, // 92
    MoveRow {
        name: "layup_runner",
        script: 0x4470de,
        legend: -1,
        mask: 0x804ff,
        family: 2,
        id: 19,
        release: 37,
        end: 48,
    }, // 93
    MoveRow {
        name: "allydunk_2",
        script: 0x447900,
        legend: -1,
        mask: 0x1008c0,
        family: 0,
        id: 7,
        release: 89,
        end: 112,
    }, // 94
    MoveRow {
        name: "allydunk_2",
        script: 0x447900,
        legend: -1,
        mask: 0x100806,
        family: 0,
        id: 7,
        release: 89,
        end: 112,
    }, // 95
    MoveRow {
        name: "allydunk_1",
        script: 0x447969,
        legend: -1,
        mask: 0x10087c,
        family: 0,
        id: 24,
        release: 91,
        end: 117,
    }, // 96
    MoveRow {
        name: "allydunk_1h_2f_spike",
        script: 0x447a85,
        legend: -1,
        mask: 0x10087c,
        family: 0,
        id: 37,
        release: 92,
        end: 108,
    }, // 97
    MoveRow {
        name: "allydunk_2h_2f_hang_swing",
        script: 0x447aec,
        legend: -1,
        mask: 0x100883,
        family: 0,
        id: 41,
        release: 92,
        end: 154,
    }, // 98
];

/// A row of the catch table (`0x3d75c8`, 40 x 20 bytes, drawn by
/// `FUN_001611d8` at the release): `flags` bits 0..7 the ball's sector
/// seen by the receiver, 0x100 either hand, 0x200/0x400 sets the left/
/// right hand, 0x2000 running, 0x4000 standing, 0x800 moving on the
/// ground, 0x8000 with state bit 11, 0x10000/0x20000/0x40000 the
/// chooser's other gates; `min_flight` the shortest flight (+0x04);
/// `tail` the updates bit 9 (the catch) stays set after the flight's
/// nominal end: the script's fixed waits after its flight wait, or
/// [`CLIP_TAIL`] where it plays the clip out (`8c`); `record` and `frame`
/// the clip and its frame the hands meet the ball on (+0x0c, +0x10: the
/// release poses the receiver there to aim the pass, `0x162518`, and the
/// script's flight wait ends on that frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatchRow {
    pub name: &'static str,
    pub script: u32,
    pub min_flight: i16,
    pub flags: u32,
    pub tail: u8,
    pub record: u16,
    pub frame: u8,
}

/// Updates a catch that plays its clip out (`8c 1.5`: the rest of the
/// clip at two thirds speed) keeps bit 9 after the flight's end: 9 in
/// every one of the 53 `catch_stand_1` catches of the attract captures
/// (the clip's length is not in the executable).
pub const CLIP_TAIL: u8 = 9;

pub const CATCH_ROWS: [CatchRow; 39] = [
    CatchRow {
        name: "catch_stand_1",
        script: 0x44319a,
        min_flight: -1,
        flags: 0x14110,
        tail: CLIP_TAIL,
        record: 565,
        frame: 10,
    }, // 0
    CatchRow {
        name: "catch_run_1",
        script: 0x4431e7,
        min_flight: -1,
        flags: 0x2110,
        tail: 0,
        record: 565,
        frame: 10,
    }, // 1
    CatchRow {
        name: "catch_stand_2r",
        script: 0x443240,
        min_flight: -1,
        flags: 0x4420,
        tail: 15,
        record: 567,
        frame: 11,
    }, // 2
    CatchRow {
        name: "catch_run_2r",
        script: 0x443298,
        min_flight: -1,
        flags: 0x2420,
        tail: 0,
        record: 567,
        frame: 11,
    }, // 3
    CatchRow {
        name: "catch_stand_2l",
        script: 0x44327c,
        min_flight: -1,
        flags: 0x4208,
        tail: 15,
        record: 566,
        frame: 11,
    }, // 4
    CatchRow {
        name: "catch_run_2l",
        script: 0x4432f1,
        min_flight: -1,
        flags: 0x2208,
        tail: 0,
        record: 566,
        frame: 11,
    }, // 5
    CatchRow {
        name: "catch_stand_3r",
        script: 0x44331b,
        min_flight: -1,
        flags: 0x24410,
        tail: 15,
        record: 569,
        frame: 15,
    }, // 6
    CatchRow {
        name: "catch_run_3r",
        script: 0x443372,
        min_flight: -1,
        flags: 0x2410,
        tail: 0,
        record: 569,
        frame: 15,
    }, // 7
    CatchRow {
        name: "catch_stand_3l",
        script: 0x443356,
        min_flight: -1,
        flags: 0x24210,
        tail: 15,
        record: 568,
        frame: 15,
    }, // 8
    CatchRow {
        name: "catch_run_3l",
        script: 0x4433cb,
        min_flight: -1,
        flags: 0x2210,
        tail: 0,
        record: 568,
        frame: 15,
    }, // 9
    CatchRow {
        name: "catch_stand_4r",
        script: 0x4433f5,
        min_flight: 20,
        flags: 0x4420,
        tail: 20,
        record: 571,
        frame: 11,
    }, // 10
    CatchRow {
        name: "catch_run_4r",
        script: 0x44344c,
        min_flight: 20,
        flags: 0x2420,
        tail: 0,
        record: 571,
        frame: 11,
    }, // 11
    CatchRow {
        name: "catch_stand_4l",
        script: 0x443430,
        min_flight: 20,
        flags: 0x4208,
        tail: 20,
        record: 570,
        frame: 11,
    }, // 12
    CatchRow {
        name: "catch_run_4l",
        script: 0x4434a5,
        min_flight: 20,
        flags: 0x2208,
        tail: 0,
        record: 570,
        frame: 11,
    }, // 13
    CatchRow {
        name: "catch_stand_5",
        script: 0x4434cf,
        min_flight: 15,
        flags: 0x110,
        tail: CLIP_TAIL,
        record: 572,
        frame: 6,
    }, // 14
    CatchRow {
        name: "catch_stand_6r",
        script: 0x44350b,
        min_flight: -1,
        flags: 0x4181,
        tail: 15,
        record: 573,
        frame: 17,
    }, // 15
    CatchRow {
        name: "catch_run_6r",
        script: 0x443562,
        min_flight: -1,
        flags: 0x2181,
        tail: 9,
        record: 573,
        frame: 17,
    }, // 16
    CatchRow {
        name: "catch_stand_6l",
        script: 0x443546,
        min_flight: -1,
        flags: 0x4103,
        tail: 15,
        record: 574,
        frame: 17,
    }, // 17
    CatchRow {
        name: "catch_run_6l",
        script: 0x4435bf,
        min_flight: -1,
        flags: 0x2103,
        tail: 9,
        record: 574,
        frame: 17,
    }, // 18
    CatchRow {
        name: "catch_stand_7r",
        script: 0x4435e9,
        min_flight: -1,
        flags: 0x4404,
        tail: 10,
        record: 576,
        frame: 17,
    }, // 19
    CatchRow {
        name: "catch_run_7r",
        script: 0x443640,
        min_flight: -1,
        flags: 0x2404,
        tail: 9,
        record: 576,
        frame: 17,
    }, // 20
    CatchRow {
        name: "catch_stand_7l",
        script: 0x443624,
        min_flight: -1,
        flags: 0x4260,
        tail: 10,
        record: 575,
        frame: 17,
    }, // 21
    CatchRow {
        name: "catch_run_7l",
        script: 0x44369d,
        min_flight: -1,
        flags: 0x2260,
        tail: 9,
        record: 575,
        frame: 17,
    }, // 22
    CatchRow {
        name: "catch_stand_8r",
        script: 0x4436c7,
        min_flight: -1,
        flags: 0x4404,
        tail: 10,
        record: 578,
        frame: 17,
    }, // 23
    CatchRow {
        name: "catch_run_8r",
        script: 0x44371e,
        min_flight: -1,
        flags: 0x2404,
        tail: 0,
        record: 578,
        frame: 17,
    }, // 24
    CatchRow {
        name: "catch_stand_8l",
        script: 0x443702,
        min_flight: -1,
        flags: 0x4240,
        tail: 10,
        record: 577,
        frame: 17,
    }, // 25
    CatchRow {
        name: "catch_run_8l",
        script: 0x443777,
        min_flight: -1,
        flags: 0x2240,
        tail: 0,
        record: 577,
        frame: 17,
    }, // 26
    CatchRow {
        name: "catch_post_1",
        script: 0x442f92,
        min_flight: -1,
        flags: 0x1c1ff,
        tail: CLIP_TAIL,
        record: 582,
        frame: 7,
    }, // 27
    CatchRow {
        name: "catch_post_2",
        script: 0x442fdf,
        min_flight: 10,
        flags: 0x1c4ff,
        tail: CLIP_TAIL,
        record: 581,
        frame: 11,
    }, // 28
    CatchRow {
        name: "catch_post_3r",
        script: 0x44302c,
        min_flight: -1,
        flags: 0x1c4ff,
        tail: CLIP_TAIL,
        record: 584,
        frame: 12,
    }, // 29
    CatchRow {
        name: "catch_post_3l",
        script: 0x443079,
        min_flight: -1,
        flags: 0x1c2ff,
        tail: CLIP_TAIL,
        record: 583,
        frame: 12,
    }, // 30
    CatchRow {
        name: "catch_post_5r",
        script: 0x4430a6,
        min_flight: -1,
        flags: 0x1c4ff,
        tail: CLIP_TAIL,
        record: 586,
        frame: 7,
    }, // 31
    CatchRow {
        name: "catch_post_5l",
        script: 0x4430f3,
        min_flight: -1,
        flags: 0x1c2ff,
        tail: CLIP_TAIL,
        record: 585,
        frame: 7,
    }, // 32
    CatchRow {
        name: "catch_post_7r",
        script: 0x443120,
        min_flight: -1,
        flags: 0x1c4ff,
        tail: CLIP_TAIL,
        record: 580,
        frame: 16,
    }, // 33
    CatchRow {
        name: "catch_post_7l",
        script: 0x44316d,
        min_flight: -1,
        flags: 0x1c2ff,
        tail: CLIP_TAIL,
        record: 579,
        frame: 16,
    }, // 34
    CatchRow {
        name: "catch_passout_1l",
        script: 0x4437f2,
        min_flight: -1,
        flags: 0x502ff,
        tail: 6,
        record: 554,
        frame: 5,
    }, // 35
    CatchRow {
        name: "catch_passout_1r",
        script: 0x4437a1,
        min_flight: -1,
        flags: 0x504ff,
        tail: 6,
        record: 553,
        frame: 5,
    }, // 36
    CatchRow {
        name: "catch_passout_2l",
        script: 0x443870,
        min_flight: -1,
        flags: 0x502ff,
        tail: 6,
        record: 560,
        frame: 7,
    }, // 37
    CatchRow {
        name: "catch_passout_2r",
        script: 0x44381f,
        min_flight: -1,
        flags: 0x504ff,
        tail: 6,
        record: 559,
        frame: 7,
    }, // 38
];
