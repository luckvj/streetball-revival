// The trick and reaction scripts as the channel's clock reads them,
// generated from SLUS_206.51 (`tests/trick_disc.rs` reads them again from
// the disc and compares): each script followed along its plain path
// (`82` jumps, `85` jumps, `84` falls through), the play-outs' updates
// from their clips' frames, the waits' frames kept within their clips;
// and each clip's root change over the script (`root`: from the blend's
// frame to the last wait's, at scale 1, in the clip's own frame; the
// trick's for its right-hand and left-hand clip), which moves the player
// when the disc's animations are not loaded.

use Step::{
    Bits, Blend, Call, Clear, End, Flight, Hop, Land, PlayOut, Shove, Stop, Test, Wait, Window,
};

/// The 75 trick scripts (`0x3d7238`, 12 bytes a row: the script, a legend,
/// the selection flags), by id; then the clearout's ([`CLEAROUT`], never
/// drawn).
pub const SCRIPTS: [Script; 76] = [
    // 0: jogdribxover05_double_l
    Script {
        address: 0x44cc7b,
        flags: 0xf,
        mask: 1,
        root: [[f32::from_bits(0xbec073f9), f32::from_bits(0xc0c8ff43)], [f32::from_bits(0x3ec073f9), f32::from_bits(0xc0c8ff43)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [179, 178] },
            Stop,
            Window { row: 3, n: 39 },
            Wait { to: 37, n: 43 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 1: jogdribbehindback_l
    Script {
        address: 0x44cccc,
        flags: 0xa,
        mask: 0,
        root: [[f32::from_bits(0xbfa0cb74), f32::from_bits(0xc125d347)], [f32::from_bits(0x3fa0cb74), f32::from_bits(0xc125d347)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [181, 180] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 51, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 2: jogdribflair_spin45_r
    Script {
        address: 0x44cd1d,
        flags: 0x4a,
        mask: 1,
        root: [[f32::from_bits(0xc023d005), f32::from_bits(0xc0aea1ff)], [f32::from_bits(0x4023d005), f32::from_bits(0xc0aea1ff)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [182, 183] },
            Stop,
            Window { row: 3, n: 27 },
            Wait { to: 25, n: 30 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 3: jogdribinsideout01_r
    Script {
        address: 0x44cd6e,
        flags: 0x2f,
        mask: 1,
        root: [[f32::from_bits(0x3e654340), f32::from_bits(0xc0cce8ed)], [f32::from_bits(0xbe654340), f32::from_bits(0xc0cce8ed)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [184, 185] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 35, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 4: jogdribinsideout03_l
    Script {
        address: 0x44cdbf,
        flags: 0xf,
        mask: 1,
        root: [[f32::from_bits(0xbe190b9d), f32::from_bits(0xc0e2d0fe)], [f32::from_bits(0x3e190b9d), f32::from_bits(0xc0e2d0fe)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [187, 186] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 27, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 5: jogdribslipslide_l
    Script {
        address: 0x44ce10,
        flags: 0xd,
        mask: 0,
        root: [[f32::from_bits(0xbe9fa678), f32::from_bits(0xc119d4ed)], [f32::from_bits(0x3e9fa678), f32::from_bits(0xc119d4ed)]],
        steps: &[
            Bits(0x400),
            Blend { n: 10, start: 0, records: [189, 188] },
            Stop,
            Window { row: 3, n: 47 },
            Wait { to: 48, n: 52 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 6: jogdrib360spin_l
    Script {
        address: 0x44ce61,
        flags: 0x9,
        mask: 0,
        root: [[f32::from_bits(0x3f2f4793), f32::from_bits(0xc0d34bc4)], [f32::from_bits(0xbf2f4793), f32::from_bits(0xc0d34bc4)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [191, 190] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 33, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 7: jogdribxover17_overhead_l
    Script {
        address: 0x44ceb2,
        flags: 0xa,
        mask: 1,
        root: [[f32::from_bits(0x3fe783fc), f32::from_bits(0xc0f7a1ec)], [f32::from_bits(0xbfe783fc), f32::from_bits(0xc0f7a1ec)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [195, 194] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 26, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 8: jogdribxover18_bigxover_l
    Script {
        address: 0x44cf03,
        flags: 0x9,
        mask: 2,
        root: [[f32::from_bits(0xbf2e2c5e), f32::from_bits(0xc0bec7db)], [f32::from_bits(0x3f2e2c5e), f32::from_bits(0xc0bec7db)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [197, 196] },
            Stop,
            Window { row: 3, n: 27 },
            Wait { to: 22, n: 30 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 9: jogdribxover07_killer2_r
    Script {
        address: 0x44cf54,
        flags: 0x2a,
        mask: 1,
        root: [[f32::from_bits(0xbe9da52e), f32::from_bits(0xc025132a)], [f32::from_bits(0x3e9da52e), f32::from_bits(0xc025132a)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [192, 193] },
            Stop,
            Window { row: 3, n: 32 },
            Wait { to: 32, n: 35 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 10: DRIBBLE_FREESTYLE_01_7_L
    Script {
        address: 0x44d7ef,
        flags: 0xa,
        mask: 0,
        root: [[f32::from_bits(0xc021a4d2), f32::from_bits(0xc0de23ac)], [f32::from_bits(0x4021a4d2), f32::from_bits(0xc0de23ac)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [201, 200] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 25, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 11: DRIBBLE_FREESTYLE_03_5_L
    Script {
        address: 0x44d423,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0xc086519c), f32::from_bits(0xc1289b0c)], [f32::from_bits(0x4086519c), f32::from_bits(0xc1289b0c)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 9, records: [203, 202] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 31, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 12: DRIBBLE_FREESTYLE_04_4_L
    Script {
        address: 0x44d474,
        flags: 0x29,
        mask: 8,
        root: [[f32::from_bits(0x409edb84), f32::from_bits(0xc0d52b82)], [f32::from_bits(0xc09edb84), f32::from_bits(0xc0d52b82)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 10, records: [205, 204] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 40, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 13: TWOPERSON_FREESTYLE_06_1_SHANE_B_R
    Script {
        address: 0x44da4a,
        flags: 0x29,
        mask: 2,
        root: [[f32::from_bits(0x404f0f0e), f32::from_bits(0xc0b6cd3f)], [f32::from_bits(0xc04f0f0e), f32::from_bits(0xc0b6cd3f)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 9, records: [238, 239] },
            Stop,
            Window { row: 3, n: 48 },
            Wait { to: 31, n: 53 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 14: DRIBBLE_FREESTYLE_03_2_A_L
    Script {
        address: 0x44d79e,
        flags: 0x4a,
        mask: 0,
        root: [[f32::from_bits(0xc043e384), f32::from_bits(0xc0da0133)], [f32::from_bits(0x4043e384), f32::from_bits(0xc0da0133)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 7, records: [267, 266] },
            Stop,
            Window { row: 3, n: 56 },
            Wait { to: 38, n: 62 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 15: DRIBBLE_FREESTYLE_01_1_A_L
    Script {
        address: 0x44db8e,
        flags: 0x29,
        mask: 0,
        root: [[f32::from_bits(0x40596a6d), f32::from_bits(0xc098b671)], [f32::from_bits(0xc0596a6d), f32::from_bits(0xc098b671)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 4, records: [247, 246] },
            Stop,
            Window { row: 3, n: 45 },
            Wait { to: 33, n: 50 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 16: DRIBBLE_FREESTYLE_02_1_R
    Script {
        address: 0x44dbdf,
        flags: 0xa,
        mask: 4,
        root: [[f32::from_bits(0xbffa7f6b), f32::from_bits(0xc05143b4)], [f32::from_bits(0x3ffa7f6b), f32::from_bits(0xc05143b4)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [248, 249] },
            Stop,
            Window { row: 3, n: 38 },
            Wait { to: 21, n: 42 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 17: DRIBBLE_FREESTYLE_02_2_A_R
    Script {
        address: 0x44dc30,
        flags: 0x49,
        mask: 8,
        root: [[f32::from_bits(0xc0a2faa9), f32::from_bits(0xc0149cd5)], [f32::from_bits(0x40a2faa9), f32::from_bits(0xc0149cd5)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [250, 251] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 37, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 18: DRIBBLE_FREESTYLE_02_2_C_R
    Script {
        address: 0x44dcd2,
        flags: 0xb,
        mask: 0,
        root: [[f32::from_bits(0xbda1712c), f32::from_bits(0xc1078c8e)], [f32::from_bits(0x3da1712c), f32::from_bits(0xc1078c8e)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 5, records: [254, 255] },
            Stop,
            Window { row: 3, n: 61 },
            Wait { to: 50, n: 68 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 19: DRIBBLE_FREESTYLE_02_3_C_L
    Script {
        address: 0x44ddc5,
        flags: 0xa,
        mask: 8,
        root: [[f32::from_bits(0x3fda19b9), f32::from_bits(0xc11cdfa1)], [f32::from_bits(0xbfda19b9), f32::from_bits(0xc11cdfa1)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [261, 260] },
            Stop,
            Window { row: 3, n: 47 },
            Wait { to: 26, n: 52 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 20: DRIBBLE_FREESTYLE_03_1_A_R
    Script {
        address: 0x44de16,
        flags: 0xb,
        mask: 16,
        root: [[f32::from_bits(0x405e5193), f32::from_bits(0xc084179b)], [f32::from_bits(0xc05e5193), f32::from_bits(0xc084179b)]],
        steps: &[
            Bits(0x400),
            Blend { n: 10, start: 6, records: [262, 263] },
            Stop,
            Window { row: 3, n: 57 },
            Wait { to: 48, n: 63 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 21: DRIBBLE_FREESTYLE_03_1_B_R
    Script {
        address: 0x44de67,
        flags: 0xb,
        mask: 0,
        root: [[f32::from_bits(0x3e1c6878), f32::from_bits(0xc12b7420)], [f32::from_bits(0xbe1c6878), f32::from_bits(0xc12b7420)]],
        steps: &[
            Bits(0x410),
            Blend { n: 10, start: 3, records: [264, 265] },
            Stop,
            Window { row: 3, n: 79 },
            Wait { to: 40, n: 44 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 70 },
            Wait { to: 72, n: 2 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 22: DRIBBLE_FREESTYLE_03_2_B_R
    Script {
        address: 0x44df51,
        flags: 0xb,
        mask: 8,
        root: [[f32::from_bits(0x3f0ec253), f32::from_bits(0xc08866b9)], [f32::from_bits(0xbf0ec253), f32::from_bits(0xc08866b9)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 4, records: [268, 269] },
            Stop,
            Window { row: 3, n: 48 },
            Wait { to: 39, n: 53 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 23: DRIBBLE_FREESTYLE_03_6_L
    Script {
        address: 0x44dedc,
        flags: 0xa,
        mask: 0,
        root: [[f32::from_bits(0xbcf48efb), f32::from_bits(0xc1013a32)], [f32::from_bits(0x3cf48efb), f32::from_bits(0xc1013a32)]],
        steps: &[
            Bits(0x410),
            Blend { n: 6, start: 2, records: [273, 272] },
            Stop,
            Window { row: 3, n: 52 },
            Wait { to: 3, n: 2 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 30 },
            Wait { to: 31, n: 2 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 24: DRIBBLE_FREESTYLE_07_3_A_L
    Script {
        address: 0x44dff3,
        flags: 0xb,
        mask: 0,
        root: [[f32::from_bits(0xc00e40b5), f32::from_bits(0x3fc96a5b)], [f32::from_bits(0x400e40b5), f32::from_bits(0x3fc96a5b)]],
        steps: &[
            Bits(0x400),
            Blend { n: 10, start: 0, records: [283, 282] },
            Stop,
            Window { row: 3, n: 60 },
            Wait { to: 50, n: 67 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 25: standdrib_fakespin_l
    Script {
        address: 0x44cfa5,
        flags: 0x4b,
        mask: 2,
        root: [[f32::from_bits(0x3f99743c), f32::from_bits(0xc0a6df36)], [f32::from_bits(0xbf99743c), f32::from_bits(0xc0a6df36)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [683, 682] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 30, n: 45 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 26: standdrib_gregmove02_r
    Script {
        address: 0x44cff6,
        flags: 0xe,
        mask: 0,
        root: [[f32::from_bits(0x3ff8614f), f32::from_bits(0xc0553a33)], [f32::from_bits(0xbff8614f), f32::from_bits(0xc0553a33)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [684, 685] },
            Stop,
            Window { row: 3, n: 50 },
            Wait { to: 40, n: 55 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 27: standdrib_spin45_l
    Script {
        address: 0x44d047,
        flags: 0x4f,
        mask: 1,
        root: [[f32::from_bits(0xc035b6b9), f32::from_bits(0xc079f3ec)], [f32::from_bits(0x4035b6b9), f32::from_bits(0xc079f3ec)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [687, 686] },
            Stop,
            Window { row: 3, n: 32 },
            Wait { to: 19, n: 35 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 28: standdrib_xoveraroundhead_l
    Script {
        address: 0x44d098,
        flags: 0x5,
        mask: 2,
        root: [[f32::from_bits(0x3f670ea3), f32::from_bits(0xbee9c58a)], [f32::from_bits(0xbf670ea3), f32::from_bits(0xbee9c58a)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [691, 690] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 25, n: 45 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 29: standdrib_xoverbetweenlegs_l
    Script {
        address: 0x44d0e9,
        flags: 0x4f,
        mask: 2,
        root: [[f32::from_bits(0xbfeb986f), f32::from_bits(0xc08e301f)], [f32::from_bits(0x3feb986f), f32::from_bits(0xc08e301f)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [693, 692] },
            Stop,
            Window { row: 3, n: 59 },
            Wait { to: 48, n: 65 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 30: standdribflair_jumpballxover_l
    Script {
        address: 0x44d13a,
        flags: 0x5,
        mask: 2,
        root: [[f32::from_bits(0xbe753fb6), f32::from_bits(0xbf23d7da)], [f32::from_bits(0x3e753fb6), f32::from_bits(0xbf23d7da)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 4, records: [695, 694] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 56, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 31: standdrib360spinthrulegs_l
    Script {
        address: 0x44d18b,
        flags: 0x6,
        mask: 1,
        root: [[f32::from_bits(0xbea8ed93), f32::from_bits(0xbf8fe770)], [f32::from_bits(0x3ea8ed93), f32::from_bits(0xbf8fe770)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [697, 696] },
            Stop,
            Window { row: 3, n: 62 },
            Wait { to: 47, n: 65 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 32: standdrib_xoverbehindheadthru_l
    Script {
        address: 0x44d1dc,
        flags: 0x5,
        mask: 2,
        root: [[f32::from_bits(0x3e3474ab), f32::from_bits(0x3ea7e8af)], [f32::from_bits(0xbe3474ab), f32::from_bits(0x3ea7e8af)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [699, 698] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 29, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 33: standdrib_fakepassdrib_l
    Script {
        address: 0x44d22d,
        flags: 0x5,
        mask: 1,
        root: [[f32::from_bits(0xbe4ea9b2), f32::from_bits(0xbfeedee7)], [f32::from_bits(0x3e4ea9b2), f32::from_bits(0xbfeedee7)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [701, 700] },
            Stop,
            Window { row: 3, n: 59 },
            Wait { to: 40, n: 65 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 34: standdrib_xoverbacktofront_l
    Script {
        address: 0x44d27e,
        flags: 0x5,
        mask: 1,
        root: [[f32::from_bits(0xbebb3861), f32::from_bits(0x400a3e0f)], [f32::from_bits(0x3ebb3861), f32::from_bits(0x400a3e0f)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [703, 702] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 29, n: 45 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 35: standdrib_thrudefenderslegs_l
    Script {
        address: 0x44d2cf,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0x3fcef0bf), f32::from_bits(0xc0d23994)], [f32::from_bits(0xbfcef0bf), f32::from_bits(0xc0d23994)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [689, 688] },
            Stop,
            Window { row: 3, n: 50 },
            Wait { to: 33, n: 55 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 36: DRIBBLE_FREESTYLE_10_5_L
    Script {
        address: 0x44d840,
        flags: 0xf,
        mask: 0,
        root: [[f32::from_bits(0x3e8bf5b4), f32::from_bits(0xc0fefaac)], [f32::from_bits(0xbe8bf5b4), f32::from_bits(0xc0fefaac)]],
        steps: &[
            Bits(0x410),
            Blend { n: 6, start: 0, records: [229, 228] },
            Stop,
            Window { row: 3, n: 61 },
            Wait { to: 16, n: 32 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 32 },
            Wait { to: 34, n: 4 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 37: DRIBBLE_FREESTYLE_05_7_L
    Script {
        address: 0x44d4c5,
        flags: 0x7,
        mask: 0,
        root: [[f32::from_bits(0xbf4615be), f32::from_bits(0xbe8138f0)], [f32::from_bits(0x3f4615be), f32::from_bits(0xbe8138f0)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 8, records: [207, 206] },
            Stop,
            Window { row: 3, n: 39 },
            Wait { to: 26, n: 43 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 38: DRIBBLE_FREESTYLE_08_5_L
    Script {
        address: 0x44d516,
        flags: 0x4e,
        mask: 0,
        root: [[f32::from_bits(0x3f3386e4), f32::from_bits(0xc0220817)], [f32::from_bits(0xbf3386e4), f32::from_bits(0xc0220817)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 11, records: [209, 208] },
            Stop,
            Window { row: 3, n: 45 },
            Wait { to: 30, n: 50 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 39: DRIBBLE_FREESTYLE_11_3_R
    Script {
        address: 0x44d567,
        flags: 0x4d,
        mask: 0,
        root: [[f32::from_bits(0xbfc2f8e2), f32::from_bits(0xc08e9d39)], [f32::from_bits(0x3fc2f8e2), f32::from_bits(0xc08e9d39)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 18, records: [210, 211] },
            Stop,
            Window { row: 3, n: 36 },
            Wait { to: 34, n: 40 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 40: DRIBBLE_FREESTYLE_12_2_L
    Script {
        address: 0x44d5b8,
        flags: 0x5,
        mask: 0,
        root: [[f32::from_bits(0xbf8a68da), f32::from_bits(0xbfe73874)], [f32::from_bits(0x3f8a68da), f32::from_bits(0xbfe73874)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 12, records: [213, 212] },
            Stop,
            Window { row: 3, n: 45 },
            Wait { to: 31, n: 50 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 41: DRIBBLE_FREESTYLE_13_7_R
    Script {
        address: 0x44d609,
        flags: 0x7,
        mask: 0,
        root: [[f32::from_bits(0x3f37c082), f32::from_bits(0xc050b538)], [f32::from_bits(0xbf37c082), f32::from_bits(0xc050b538)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 13, records: [214, 215] },
            Stop,
            Window { row: 3, n: 39 },
            Wait { to: 31, n: 43 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 42: DRIBBLE_FREESTYLE_14_1_R
    Script {
        address: 0x44d65a,
        flags: 0x47,
        mask: 2,
        root: [[f32::from_bits(0xc00f1320), f32::from_bits(0xc0484a77)], [f32::from_bits(0x400f1320), f32::from_bits(0xc0484a77)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 13, records: [216, 217] },
            Stop,
            Window { row: 3, n: 44 },
            Wait { to: 31, n: 49 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 43: DRIBBLE_FREESTYLE_15_3_R
    Script {
        address: 0x44d6ab,
        flags: 0x7,
        mask: 2,
        root: [[f32::from_bits(0x3ea04d35), f32::from_bits(0xbfaeca01)], [f32::from_bits(0xbea04d35), f32::from_bits(0xbfaeca01)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 8, records: [218, 219] },
            Stop,
            Window { row: 3, n: 33 },
            Wait { to: 23, n: 37 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 44: STANDDRIB_FAKEPULLUP_2_L
    Script {
        address: 0x44d6fc,
        flags: 0x2e,
        mask: 2,
        root: [[f32::from_bits(0xbf83187e), f32::from_bits(0xbfd462d9)], [f32::from_bits(0x3f83187e), f32::from_bits(0xbfd462d9)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [221, 220] },
            Stop,
            Window { row: 3, n: 45 },
            Wait { to: 25, n: 50 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 45: STANDDRIB_KICKTHRULEGS_4_R
    Script {
        address: 0x44d381,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0x3f0f11db), f32::from_bits(0xc17e291a)], [f32::from_bits(0xbf0f11db), f32::from_bits(0xc17e291a)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [222, 223] },
            Stop,
            Window { row: 3, n: 97 },
            Wait { to: 54, n: 108 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 46: STANDDRIB_SHAMGOD_5_R
    Script {
        address: 0x44d74d,
        flags: 0xf,
        mask: 2,
        root: [[f32::from_bits(0xbe661209), f32::from_bits(0xbfcee718)], [f32::from_bits(0x3e661209), f32::from_bits(0xbfcee718)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [224, 225] },
            Stop,
            Window { row: 3, n: 48 },
            Wait { to: 35, n: 53 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 47: STANDDRIB_BALL_ON_FOOT_3_L_nocomp
    Script {
        address: 0x44d8b5,
        flags: 0x7,
        mask: 2,
        root: [[f32::from_bits(0x3e56fb49), f32::from_bits(0xbf513d07)], [f32::from_bits(0xbe56fb49), f32::from_bits(0xbf513d07)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [227, 226] },
            Stop,
            Window { row: 3, n: 62 },
            Wait { to: 40, n: 69 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 48: TWOPERSON_FREESTYLE_01_1_PHARMACIST_A_L
    Script {
        address: 0x44d906,
        flags: 0xf,
        mask: 1,
        root: [[f32::from_bits(0x3f3d4392), f32::from_bits(0xc08aca2b)], [f32::from_bits(0xbf3d4392), f32::from_bits(0xc08aca2b)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [230, 231] },
            Stop,
            Window { row: 3, n: 46 },
            Wait { to: 30, n: 51 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 49: TWOPERSON_FREESTYLE_01_1_PHARMACIST_B_L
    Script {
        address: 0x44d957,
        flags: 0xd,
        mask: 1,
        root: [[f32::from_bits(0xbfd45f68), f32::from_bits(0xc05a9ea1)], [f32::from_bits(0x3fd45f68), f32::from_bits(0xc05a9ea1)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 7, records: [233, 232] },
            Stop,
            Window { row: 3, n: 62 },
            Wait { to: 53, n: 69 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 50: TWOPERSON_FREESTYLE_03_1_PHAR_E_L
    Script {
        address: 0x44d9a8,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0xc0e2f741), f32::from_bits(0xc0949bf9)], [f32::from_bits(0x40e2f741), f32::from_bits(0xc0949bf9)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 17, records: [235, 234] },
            Stop,
            Window { row: 3, n: 50 },
            Wait { to: 54, n: 56 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 51: TWOPERSON_FREESTYLE_03_1_PHAR_F_R
    Script {
        address: 0x44d9f9,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0x4041d616), f32::from_bits(0xc06a4b37)], [f32::from_bits(0xc041d616), f32::from_bits(0xc06a4b37)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 13, records: [236, 237] },
            Stop,
            Window { row: 3, n: 32 },
            Wait { to: 34, n: 36 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 52: TWOPERSON_FREESTYLE_07_1_SHANE_A_L
    Script {
        address: 0x44daec,
        flags: 0x7,
        mask: 1,
        root: [[f32::from_bits(0x3fae7cfe), f32::from_bits(0xbe47e7f1)], [f32::from_bits(0xbfae7cfe), f32::from_bits(0xbe47e7f1)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 15, records: [241, 240] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 38, n: 46 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 53: TWOPERSON_FREESTYLE_07_1_SHANE_B_L
    Script {
        address: 0x44db3d,
        flags: 0x6,
        mask: 2,
        root: [[f32::from_bits(0xbf276e8a), f32::from_bits(0xc047aa63)], [f32::from_bits(0x3f276e8a), f32::from_bits(0xc047aa63)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 5, records: [243, 242] },
            Stop,
            Window { row: 3, n: 52 },
            Wait { to: 36, n: 58 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 54: TWOPERSON_FREESTYLE_07_2_SHANE_R
    Script {
        address: 0x44da9b,
        flags: 0xe,
        mask: 0,
        root: [[f32::from_bits(0x3fc84874), f32::from_bits(0xc08d04d4)], [f32::from_bits(0xbfc84874), f32::from_bits(0xc08d04d4)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 10, records: [244, 245] },
            Stop,
            Window { row: 3, n: 49 },
            Wait { to: 46, n: 54 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 55: DRIBBLE_FREESTYLE_02_2_B_R
    Script {
        address: 0x44dc81,
        flags: 0x7,
        mask: 0,
        root: [[f32::from_bits(0xbe0f066a), f32::from_bits(0xbe0e5ebe)], [f32::from_bits(0x3e0f066a), f32::from_bits(0xbe0e5ebe)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [252, 253] },
            Stop,
            Window { row: 3, n: 90 },
            Wait { to: 67, n: 100 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 56: DRIBBLE_FREESTYLE_02_3_A_R
    Script {
        address: 0x44dd23,
        flags: 0x4f,
        mask: 4,
        root: [[f32::from_bits(0xbef7e690), f32::from_bits(0xc00ed92c)], [f32::from_bits(0x3ef7e690), f32::from_bits(0xc00ed92c)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 5, records: [256, 257] },
            Stop,
            Window { row: 3, n: 50 },
            Wait { to: 42, n: 55 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 57: DRIBBLE_FREESTYLE_02_3_B_L
    Script {
        address: 0x44dd74,
        flags: 0x9,
        mask: 4,
        root: [[f32::from_bits(0x3dc775a1), f32::from_bits(0xbfe739dc)], [f32::from_bits(0xbdc775a1), f32::from_bits(0xbfe739dc)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [259, 258] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 35, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 58: DRIBBLE_FREESTYLE_03_3_R
    Script {
        address: 0x44d3d2,
        flags: 0x0,
        mask: 0,
        root: [[f32::from_bits(0xc0280e30), f32::from_bits(0xc10af413)], [f32::from_bits(0x40280e30), f32::from_bits(0xc10af413)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 2, records: [270, 271] },
            Stop,
            Window { row: 3, n: 77 },
            Wait { to: 52, n: 86 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 59: DRIBBLE_FREESTYLE_04_2_A_R
    Script {
        address: 0x44dfa2,
        flags: 0xd,
        mask: 2,
        root: [[f32::from_bits(0xbf825c18), f32::from_bits(0xc033b621)], [f32::from_bits(0x3f825c18), f32::from_bits(0xc033b621)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 3, records: [274, 275] },
            Stop,
            Window { row: 3, n: 38 },
            Wait { to: 24, n: 42 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 60: DRIBBLE_FREESTYLE_05_2_A_R
    Script {
        address: 0x44e044,
        flags: 0x4d,
        mask: 0,
        root: [[f32::from_bits(0xc0a94c40), f32::from_bits(0xc1203fec)], [f32::from_bits(0x40a94c40), f32::from_bits(0xc1203fec)]],
        steps: &[
            Bits(0x410),
            Blend { n: 6, start: 0, records: [276, 277] },
            Stop,
            Window { row: 3, n: 70 },
            Wait { to: 4, n: 6 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 50 },
            Wait { to: 52, n: 3 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 61: DRIBBLE_FREESTYLE_06_1_B_R
    Script {
        address: 0x44e0b9,
        flags: 0x5,
        mask: 2,
        root: [[f32::from_bits(0x3fb49eff), f32::from_bits(0xc00d9347)], [f32::from_bits(0xbfb49eff), f32::from_bits(0xc00d9347)]],
        steps: &[
            Bits(0x410),
            Blend { n: 6, start: 0, records: [278, 279] },
            Stop,
            Window { row: 3, n: 49 },
            Wait { to: 9, n: 14 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 34 },
            Wait { to: 35, n: 2 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 62: DRIBBLE_FREESTYLE_06_1_C_R
    Script {
        address: 0x44e12e,
        flags: 0x26,
        mask: 1,
        root: [[f32::from_bits(0x4055fe7e), f32::from_bits(0x3fda6ea7)], [f32::from_bits(0xc055fe7e), f32::from_bits(0x3fda6ea7)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [280, 281] },
            Stop,
            Window { row: 3, n: 54 },
            Wait { to: 40, n: 60 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 63: DRIBBLE_FREESTYLE_07_3_B_L
    Script {
        address: 0x44e17f,
        flags: 0x4f,
        mask: 4,
        root: [[f32::from_bits(0xc0567e66), f32::from_bits(0xc0978b36)], [f32::from_bits(0x40567e66), f32::from_bits(0xc0978b36)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [285, 284] },
            Stop,
            Window { row: 3, n: 60 },
            Wait { to: 39, n: 67 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 64: DRIBBLE_FREESTYLE_08_1_R
    Script {
        address: 0x44e1d0,
        flags: 0x2d,
        mask: 2,
        root: [[f32::from_bits(0xbfe84da1), f32::from_bits(0xc014cd72)], [f32::from_bits(0x3fe84da1), f32::from_bits(0xc014cd72)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 2, records: [286, 287] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 30, n: 45 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 65: DRIBBLE_FREESTYLE_10_1_R
    Script {
        address: 0x44e221,
        flags: 0x5,
        mask: 1,
        root: [[f32::from_bits(0xbf0ce277), f32::from_bits(0xbfe34051)], [f32::from_bits(0x3f0ce277), f32::from_bits(0xbfe34051)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [288, 289] },
            Stop,
            Window { row: 3, n: 43 },
            Wait { to: 28, n: 48 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 66: DRIBBLE_FREESTYLE_12_1_A_R
    Script {
        address: 0x44e272,
        flags: 0x26,
        mask: 4,
        root: [[f32::from_bits(0x3ff816f8), f32::from_bits(0xbe225247)], [f32::from_bits(0xbff816f8), f32::from_bits(0xbe225247)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [290, 291] },
            Stop,
            Window { row: 3, n: 46 },
            Wait { to: 30, n: 51 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 67: DRIBBLE_FREESTYLE_12_1_B_R
    Script {
        address: 0x44e2c3,
        flags: 0x6,
        mask: 2,
        root: [[f32::from_bits(0xbd647256), f32::from_bits(0xbf2ff80c)], [f32::from_bits(0x3d647256), f32::from_bits(0xbf2ff80c)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [292, 293] },
            Stop,
            Window { row: 3, n: 52 },
            Wait { to: 29, n: 58 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 68: DRIBBLE_FREESTYLE_12_2_A_R
    Script {
        address: 0x44e314,
        flags: 0x7,
        mask: 0,
        root: [[f32::from_bits(0x3e9501d4), f32::from_bits(0xbfdc17de)], [f32::from_bits(0xbe9501d4), f32::from_bits(0xbfdc17de)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [294, 295] },
            Stop,
            Window { row: 3, n: 41 },
            Wait { to: 23, n: 46 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 69: DRIBBLE_FREESTYLE_12_2_B_R
    Script {
        address: 0x44e365,
        flags: 0x2e,
        mask: 4,
        root: [[f32::from_bits(0x4017e0a4), f32::from_bits(0xbfa5a2d7)], [f32::from_bits(0xc017e0a4), f32::from_bits(0xbfa5a2d7)]],
        steps: &[
            Bits(0x410),
            Blend { n: 6, start: 0, records: [296, 297] },
            Stop,
            Window { row: 3, n: 43 },
            Wait { to: 7, n: 14 },
            Call(53),
            Clear(0x10),
            Bits(0x600),
            Flight { to: 21 },
            Wait { to: 25, n: 6 },
            Clear(0x600),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 70: DRIBBLE_FREESTYLE_12_2_C_R
    Script {
        address: 0x44e3da,
        flags: 0x26,
        mask: 1,
        root: [[f32::from_bits(0x4015c6a4), f32::from_bits(0xbfb4aa48)], [f32::from_bits(0xc015c6a4), f32::from_bits(0xbfb4aa48)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [298, 299] },
            Stop,
            Window { row: 3, n: 51 },
            Wait { to: 33, n: 57 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 71: DRIBBLE_FREESTYLE_12_2_D_R
    Script {
        address: 0x44e42b,
        flags: 0x7,
        mask: 0,
        root: [[f32::from_bits(0x4040e9c6), f32::from_bits(0x40571409)], [f32::from_bits(0xc040e9c6), f32::from_bits(0x40571409)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [300, 301] },
            Stop,
            Window { row: 3, n: 68 },
            Wait { to: 51, n: 76 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 72: DRIBBLE_FREESTYLE_13_1_R
    Script {
        address: 0x44e47c,
        flags: 0x5,
        mask: 0,
        root: [[f32::from_bits(0x3bc1dc2c), f32::from_bits(0xc022bc41)], [f32::from_bits(0xbbc1dc2c), f32::from_bits(0xc022bc41)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 0, records: [302, 303] },
            Stop,
            Window { row: 3, n: 32 },
            Wait { to: 23, n: 35 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 73: CONNIE_HAWKINS_SIGNATURE_MOVE_4_R
    Script {
        address: 0x44e4cd,
        flags: 0x6,
        mask: 0,
        root: [[f32::from_bits(0xbe30b4d8), f32::from_bits(0x3d15d1b4)], [f32::from_bits(0x3e30b4d8), f32::from_bits(0x3d15d1b4)]],
        steps: &[
            Bits(0x400),
            Blend { n: 6, start: 2, records: [304, 305] },
            Stop,
            Window { row: 3, n: 45 },
            Wait { to: 35, n: 50 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 74: post_wrappassdropspin_l
    Script {
        address: 0x44d320,
        flags: 0x10,
        mask: 3,
        root: [[f32::from_bits(0x4081d328), f32::from_bits(0x40a3c83c)], [f32::from_bits(0x4081d328), f32::from_bits(0x40a3c83c)]],
        steps: &[
            Stop,
            Bits(0x400),
            Call(4),
            Blend { n: 6, start: 0, records: [734, 734] },
            Window { row: 4, n: 50 },
            Wait { to: 30, n: 55 },
            Clear(0x400),
            Call(7),
            Call(1),
            End,
        ],
    },
    // 75: clearout01, `FUN_0015c778`'s (not of `0x3d7238`).
    Script {
        address: 0x456d02,
        flags: 0,
        mask: 0,
        root: [[f32::from_bits(0xbec86b64), f32::from_bits(0x3ecce406)], [f32::from_bits(0xbec86b64), f32::from_bits(0x3ecce406)]],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 6, start: 0, records: [658, 658] },
            Shove { row: 1, n: 40 },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 29, n: 58 },
            Call(1),
            End,
        ],
    },
];

/// The defenders' reaction scripts, by address: the lists of `0x3d0f18`
/// (not fooled) and `0x3d0f48` (faked) and the step-through `0x44c356`;
/// then the trick counter's two ([`crate::counter`]): the handler a
/// counter takes the ball from (`0x44bf61`) and the defender whose counter
/// misses (`0x44bf94`); then the clearout's knocks ([`CLEAROUT_BRACED`]);
/// then the freestyles' ([`FREESTYLES`]: `0x44c116`, `0x44c136`,
/// `0x44c156`); then the dunk's and lay-up's knocks ([`KNOCK_BEHIND`],
/// [`KNOCK_FLOOR`]); then the board passout's look ([`BOARD_LOOK`]), the
/// head passout's hit ([`HEAD_HIT`]) and catch ([`HEAD_CATCH`]); then
/// the celebrations ([`CELEBRATE_ROWS`]) and the
/// loser's reactions at the win ([`DEJECT_ROWS`]), each its row on the
/// shared script.
pub const REACTIONS: [Reaction; 69] = [
    // standdunkfallforward
    Reaction {
        address: 0x44bcb8,
        root: [f32::from_bits(0x3faa72c0), f32::from_bits(0x3ff3dcea)],
        steps: &[
            Bits(0x0),
            Hop { turn: 0, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [590, 590] },
            Land { to: 12 },
            Stop,
            Wait { to: 22, n: 20 },
            Test(3),
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [655, 655] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 53, n: 80 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // jogdribblefallforward_notrans
    Reaction {
        address: 0x44bcef,
        root: [f32::from_bits(0x3faaf66e), f32::from_bits(0x3eb9a2f0)],
        steps: &[
            Bits(0x0),
            Hop { turn: 0, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 3, start: 0, records: [587, 587] },
            Land { to: 12 },
            Stop,
            Wait { to: 35, n: 28 },
            Test(3),
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [655, 655] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 53, n: 80 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // standrestfallforward_notrans
    Reaction {
        address: 0x44bd26,
        root: [f32::from_bits(0x3fb19a3b), f32::from_bits(0x4037a45f)],
        steps: &[
            Bits(0x0),
            Hop { turn: 0, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [592, 592] },
            Land { to: 13 },
            Stop,
            Wait { to: 24, n: 17 },
            Test(3),
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [655, 655] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 53, n: 80 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // jogfallforward_notrans
    Reaction {
        address: 0x44bd5d,
        root: [f32::from_bits(0x3faaa334), f32::from_bits(0x3fca0634)],
        steps: &[
            Bits(0x0),
            Hop { turn: 0, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [594, 594] },
            Land { to: 16 },
            Stop,
            Wait { to: 23, n: 14 },
            Test(3),
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [655, 655] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 53, n: 80 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // standstumbleforward
    Reaction {
        address: 0x44bf2f,
        root: [f32::from_bits(0x3d760f25), f32::from_bits(0xc04e8aab)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [602, 602] },
            Wait { to: 12, n: 20 },
            Call(1),
            End,
        ],
    },
    // react_stumbleback
    Reaction {
        address: 0x44bfc6,
        root: [f32::from_bits(0xbed1b68f), f32::from_bits(0x3fab9b16)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [615, 615] },
            PlayOut { per_frame: f32::from_bits(0x3f99999a), to: 30, n: 36 },
            Call(1),
            End,
        ],
    },
    // react_stumbleback2
    Reaction {
        address: 0x44bff9,
        root: [f32::from_bits(0x3dd0d677), f32::from_bits(0x3f472508)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [616, 616] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 47, n: 94 },
            Call(1),
            End,
        ],
    },
    // standstumbleside_r
    Reaction {
        address: 0x44c07a,
        root: [f32::from_bits(0x405fd31e), f32::from_bits(0x3e95facf)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [600, 600] },
            Wait { to: 12, n: 20 },
            Call(1),
            End,
        ],
    },
    // standstumbleside_r
    Reaction {
        address: 0x44c096,
        root: [f32::from_bits(0xc05fd31e), f32::from_bits(0x3e95facf)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [601, 601] },
            Wait { to: 12, n: 20 },
            Call(1),
            End,
        ],
    },
    // react_sidelunge_r
    Reaction {
        address: 0x44c0c8,
        root: [f32::from_bits(0xbc2db403), f32::from_bits(0x3f05ef86)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 6, records: [611, 611] },
            Wait { to: 50, n: 80 },
            Call(1),
            End,
        ],
    },
    // react_sidelunge_r
    Reaction {
        address: 0x44c0e4,
        root: [f32::from_bits(0x3c2db403), f32::from_bits(0x3f05ef86)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 6, records: [612, 612] },
            Wait { to: 50, n: 80 },
            Call(1),
            End,
        ],
    },
    // REACT_SLIPNSLIDE_2_L
    Reaction {
        address: 0x44c356,
        root: [f32::from_bits(0x3f9907a3), f32::from_bits(0xc0e8ce1b)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [642, 642] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 39, n: 78 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // TWOPERSON_FREESTYLE_01_1_SHANE_A_L
    Reaction {
        address: 0x44c39f,
        root: [f32::from_bits(0xbf72b28c), f32::from_bits(0x3f9f9e63)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 12, records: [617, 617] },
            Wait { to: 24, n: 20 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // TWOPERSON_FREESTYLE_01_1_SHANE_B_L
    Reaction {
        address: 0x44c3e7,
        root: [f32::from_bits(0xbf8ee33e), f32::from_bits(0x401122c6)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 8, records: [619, 619] },
            Wait { to: 23, n: 24 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_DEF_01_2_L
    Reaction {
        address: 0x44c42f,
        root: [f32::from_bits(0xbf000410), f32::from_bits(0x4012ad61)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [626, 626] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 58, n: 116 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_DEF_03_2
    Reaction {
        address: 0x44c478,
        root: [f32::from_bits(0xbc3da3cd), f32::from_bits(0x4040bd72)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [630, 630] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 64, n: 128 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_DEF_04_2_R
    Reaction {
        address: 0x44c4bc,
        root: [f32::from_bits(0xbf3681aa), f32::from_bits(0x40514e8d)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [632, 632] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 57, n: 114 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // react_brokenankles
    Reaction {
        address: 0x44c505,
        root: [f32::from_bits(0x3fdac5e6), f32::from_bits(0x40044a29)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [604, 604] },
            PlayOut { per_frame: f32::from_bits(0x40133333), to: 33, n: 76 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_OFF_HAND_1_R
    Reaction {
        address: 0x44c549,
        root: [f32::from_bits(0xbf70f889), f32::from_bits(0xc002fd1c)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 6, records: [638, 638] },
            Wait { to: 18, n: 20 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_PULLUP_FAKE_1_R
    Reaction {
        address: 0x44c591,
        root: [f32::from_bits(0x3ebbd05e), f32::from_bits(0xc00830c6)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 6, records: [639, 639] },
            Wait { to: 20, n: 20 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_DEF_02_2_L
    Reaction {
        address: 0x44c5d9,
        root: [f32::from_bits(0xc035aa8e), f32::from_bits(0xbf94000a)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [628, 628] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 48, n: 96 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_DEF_02_2_L
    Reaction {
        address: 0x44c5f5,
        root: [f32::from_bits(0x4035aa8e), f32::from_bits(0xbf94000a)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [629, 629] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 48, n: 96 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_GOFORSTEAL_6_L
    Reaction {
        address: 0x44c639,
        root: [f32::from_bits(0xbf985908), f32::from_bits(0x3f054a72)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 4, records: [633, 633] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 48, n: 88 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_GOFORSTEAL_6_L
    Reaction {
        address: 0x44c655,
        root: [f32::from_bits(0x3f985908), f32::from_bits(0x3f054a72)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 4, records: [634, 634] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 48, n: 88 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_HEAD_LOOK_2_L
    Reaction {
        address: 0x44c699,
        root: [f32::from_bits(0xbf6cc1c6), f32::from_bits(0xbebabda9)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 9, records: [635, 635] },
            Wait { to: 24, n: 20 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACT_HEAD_LOOK_2_L
    Reaction {
        address: 0x44c6b5,
        root: [f32::from_bits(0x3f6cc1c6), f32::from_bits(0xbebabda9)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 9, records: [636, 636] },
            Wait { to: 24, n: 20 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // standstumbleback: the countered handler (`FUN_001343e8`, `0x134bfc`)
    Reaction {
        address: 0x44bf61,
        root: [f32::from_bits(0x3dd6ee71), f32::from_bits(0x40bddc60)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [603, 603] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 32, n: 64 },
            Call(1),
            End,
        ],
    },
    // standstumbleback: the counter that misses (`0x134ef8`)
    Reaction {
        address: 0x44bf94,
        root: [f32::from_bits(0x3c5ab196), f32::from_bits(0x409c22c8)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [603, 603] },
            Wait { to: 18, n: 23 },
            Call(1),
            End,
        ],
    },

    // The clearout's knocks of a defender at least as strong (`0x3d0de8`,
    // `0x43d0b0`'s quarter 2): records 591, 588, 589, 593.
    Reaction {
        address: 0x44baaf,
        root: [f32::from_bits(0xbed09d1b), f32::from_bits(0xbf945d69)],
        steps: &[
            Bits(0x0),
            Hop { turn: 512, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 3, records: [591, 591] },
            Land { to: 12 },
            Stop,
            PlayOut { per_frame: f32::from_bits(0x40200000), to: 34, n: 55 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44bb0a,
        root: [f32::from_bits(0xc07af54d), f32::from_bits(0x3f450f82)],
        steps: &[
            Bits(0x0),
            Hop { turn: 512, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [588, 588] },
            Land { to: 17 },
            Stop,
            PlayOut { per_frame: f32::from_bits(0x40200000), to: 44, n: 68 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44bb65,
        root: [f32::from_bits(0xbeb35f1c), f32::from_bits(0xbf8c16f8)],
        steps: &[
            Bits(0x0),
            Hop { turn: 512, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [589, 589] },
            Land { to: 11 },
            Stop,
            PlayOut { per_frame: f32::from_bits(0x40200000), to: 47, n: 90 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44bbc0,
        root: [f32::from_bits(0xbde02f07), f32::from_bits(0xc059125b)],
        steps: &[
            Bits(0x0),
            Hop { turn: 512, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
            Blend { n: 8, start: 0, records: [593, 593] },
            Land { to: 7 },
            Stop,
            PlayOut { per_frame: f32::from_bits(0x40200000), to: 29, n: 55 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44c116,
        root: [f32::from_bits(0xbedfcf38), f32::from_bits(0xbefd1115)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 12, start: 0, records: [69, 69] },
            Wait { to: 5, n: 13 },
            Blend { n: 4, start: 0, records: [605, 605] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 40, n: 60 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44c136,
        root: [f32::from_bits(0xbed63276), f32::from_bits(0xbf089b62)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 12, start: 0, records: [69, 69] },
            Wait { to: 15, n: 38 },
            Blend { n: 4, start: 0, records: [605, 605] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 40, n: 60 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44c156,
        root: [f32::from_bits(0xbf1a936a), f32::from_bits(0xbef6b11f)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 12, start: 0, records: [69, 69] },
            Wait { to: 25, n: 63 },
            Blend { n: 4, start: 0, records: [605, 605] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 40, n: 60 },
            Call(7),
            Call(1),
            End,
        ],
    },
    Reaction {
        address: 0x44bd94,
        root: [f32::from_bits(0x3f9991ba), f32::from_bits(0xbfbc7a86)],
        steps: &[
        Bits(0x0),
        Hop { turn: 0, speed: f32::from_bits(0x3e19999a), rise: f32::from_bits(0x3d23d70a) },
        Blend { n: 8, start: 0, records: [599, 599] },
        Land { to: 11 },
        Stop,
        Wait { to: 33, n: 33 },
        Test(3),
        Test(3),
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [655, 655] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 53, n: 80 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c85a,
        root: [f32::from_bits(0x3f1d78f1), f32::from_bits(0xbfda60c1)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [645, 645] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 52, n: 78 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c89e,
        root: [f32::from_bits(0xbbc35d5b), f32::from_bits(0xbec763f7)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [644, 644] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 50, n: 75 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c8fe,
        root: [f32::from_bits(0x40aeef01), f32::from_bits(0x3f9577b3)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [647, 647] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 59, n: 89 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c785,
        root: [f32::from_bits(0x3d88435a), f32::from_bits(0xbdbd27d8)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [608, 608] },
        Wait { to: 6, n: 45 },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 26, n: 30 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c7d2,
        root: [f32::from_bits(0x3e79c7eb), f32::from_bits(0x40ab1585)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [648, 648] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 46, n: 69 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c816,
        root: [f32::from_bits(0x3e3af5d0), f32::from_bits(0x3f8fd953)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [649, 649] },
        PlayOut { per_frame: f32::from_bits(0x40000000), to: 58, n: 116 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c8ba,
        root: [f32::from_bits(0x3bc35d5b), f32::from_bits(0xbec763f7)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [643, 643] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 50, n: 75 },
        Call(7),
        Call(1),
        End,
        ],
    },
    Reaction {
        address: 0x44c91a,
        root: [f32::from_bits(0xc0aeef01), f32::from_bits(0x3f9577b3)],
        steps: &[
        Stop,
        Bits(0x0),
        Blend { n: 4, start: 0, records: [646, 646] },
        PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 59, n: 89 },
        Call(7),
        Call(1),
        End,
        ],
    },
    // react_fakepasslookbehind_l
    Reaction {
        address: 0x44cac2,
        root: [f32::from_bits(0xbe6d6144), f32::from_bits(0x3f0d6b39)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 0, records: [610, 610] },
            PlayOut { per_frame: f32::from_bits(0x3fc00000), to: 37, n: 56 },
            Call(1),
            End,
        ],
    },
    // PASSOUT_DEFENDER_REACTION_3
    Reaction {
        address: 0x44c1ad,
        root: [f32::from_bits(0x3dcbd02c), f32::from_bits(0x3c25d9f0)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 4, records: [621, 621] },
            Wait { to: 29, n: 50 },
            Call(1),
            End,
        ],
    },
    // PASSOUT_DEFENDER_REACTION_3_2
    Reaction {
        address: 0x44c1df,
        root: [f32::from_bits(0x3deb1440), f32::from_bits(0xbe170fec)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 4, start: 7, records: [622, 622] },
            Wait { to: 37, n: 50 },
            Call(1),
            End,
        ],
    },
    // PASSOUT_DEFENDER_REACTION_5_R
    Reaction {
        address: 0x45799a,
        root: [f32::from_bits(0x3e8137ba), f32::from_bits(0xbe81d637)],
        steps: &[
            Stop,
            Bits(0x10040),
            Blend { n: 5, start: 9, records: [394, 394] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 21, n: 24 },
            Clear(0x10040),
            Call(1),
            End,
        ],
    },
    // celeb01_FINAL
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3c9e5b50), f32::from_bits(0x413328a6)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [933, 933] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 63, n: 126 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // celeb03_FINAL
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xbf842d82), f32::from_bits(0x40b89ffc)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [931, 931] },
            PlayOut { per_frame: f32::from_bits(0x40600000), to: 49, n: 172 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // celeb05_FINAL
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x40556dbc), f32::from_bits(0x41026ce7)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [932, 932] },
            PlayOut { per_frame: f32::from_bits(0x40200000), to: 50, n: 125 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // jogceleb_bringdownhouse_FINAL
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xc00134ba), f32::from_bits(0x4101fe31)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [934, 934] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 77, n: 154 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // jogceleb_sidehistep_FINAL
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xc062899b), f32::from_bits(0x410f7ffb)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [935, 935] },
            PlayOut { per_frame: f32::from_bits(0x40800000), to: 37, n: 148 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // stand_dance_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xbf6debe8), f32::from_bits(0x3fd8c5da)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [51, 51] },
            PlayOut { per_frame: f32::from_bits(0x41080000), to: 54, n: 459 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE_08_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3f8f61d9), f32::from_bits(0xc0e876a5)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [936, 936] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 60, n: 144 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE_14_2
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xc13d70d8), f32::from_bits(0xc1818c82)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [937, 937] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 69, n: 166 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE_16_2
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xc0d4a7b1), f32::from_bits(0xc1a2ae34)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [938, 938] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 65, n: 156 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE_16_3
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xc0c4f392), f32::from_bits(0xc16a8940)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [939, 939] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 64, n: 154 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE07_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3f4f0442), f32::from_bits(0xc10354fe)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [940, 940] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 60, n: 144 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE11_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3f3f4744), f32::from_bits(0xbf88efdb)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [941, 941] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 49, n: 118 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE02_4
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x40864511), f32::from_bits(0xc00862cf)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [942, 942] },
            PlayOut { per_frame: f32::from_bits(0x40400000), to: 69, n: 207 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE03_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x4077cd7c), f32::from_bits(0xbf37fac1)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [943, 943] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 68, n: 164 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE03_3
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3fff8c67), f32::from_bits(0xbfa3d704)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [944, 944] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 80, n: 192 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // CELEBRATE_FREESTYLE05_1
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0x3e32efbd), f32::from_bits(0xc0d8c528)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [945, 945] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 35, n: 84 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // SIG_MOVE_PICKUP_08_4
    Reaction {
        address: 0x44cbbf,
        root: [f32::from_bits(0xbd5bbd19), f32::from_bits(0xc0e74123)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [946, 946] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 79, n: 190 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACTIONS_FREESTYLE_01_2
    Reaction {
        address: 0x44cc03,
        root: [f32::from_bits(0xc010f82a), f32::from_bits(0xc132e420)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [958, 958] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 69, n: 138 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACTIONS_FREESTYLE_02_4
    Reaction {
        address: 0x44cc03,
        root: [f32::from_bits(0x3f5c6aaf), f32::from_bits(0xc13e58f7)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [959, 959] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 64, n: 154 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACTIONS_FREESTYLE_03_1
    Reaction {
        address: 0x44cc03,
        root: [f32::from_bits(0x3ec159fd), f32::from_bits(0x4131ac52)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [960, 960] },
            PlayOut { per_frame: f32::from_bits(0x40000000), to: 79, n: 158 },
            Call(7),
            Call(1),
            End,
        ],
    },
    // REACTIONS_FREESTYLE_04_2
    Reaction {
        address: 0x44cc03,
        root: [f32::from_bits(0xbe9f8df8), f32::from_bits(0x417c0673)],
        steps: &[
            Stop,
            Bits(0x0),
            Blend { n: 8, start: 0, records: [961, 961] },
            PlayOut { per_frame: f32::from_bits(0x4019999a), to: 93, n: 224 },
            Call(7),
            Call(1),
            End,
        ],
    },
];

/// `0x3d0f18`: the reactions of a defender who holds, by the quarter the
/// handler stands in about him (`FUN_0018d9f0(defender, handler)`'s third
/// output: 2 in front, 0 behind).
pub const HELD: [&[u8]; 4] = [
    &[4, 18, 19],
    &[7, 25],
    &[12, 13, 5],
    &[8, 24],
];

/// `0x3d0f48`: a faked defender's.
pub const FAKED: [&[u8]; 4] = [
    &[0, 1, 2, 3],
    &[21, 23, 9],
    &[14, 15, 16, 17, 6],
    &[20, 22, 10],
];

/// `0x44c356`: the reaction a fakeout from in front plays against the
/// [`STEP_THROUGH_RECORDS`].
pub const STEP_THROUGH: u8 = 11;

/// The two-person freestyles (`FUN_0015c8e8`): the trick script and the
/// opponent's reaction, within 4.75 of the holder's scale (`0x44d2cf`,
/// `0x44d9a8`, `0x44d9f9`: ids 35, 50, 51, each faking him with the first
/// reaction) and within 8 (`0x44d423`, `0x44d381`, `0x44d3d2`: 11, 45 and
/// 58, which the original keeps as id 45; the three reactions in turn).
pub const FREESTYLES: [[(u16, u8); 3]; 2] = [[(35, 32), (50, 32), (51, 32)], [(11, 32), (45, 33), (58, 34)]];

/// The knock of a man in the air by a dunk or lay-up (`FUN_001366d8`)
/// from behind him: `0x44bd94`, he turned away first; from his front or
/// sides [`REACTIONS`]' `0x44bb65` ([`CLEAROUT_BRACED`]'s third).
pub const KNOCK_BEHIND: u8 = 35;
pub const KNOCK_FRONT: u8 = 30;

/// A dunk's or lay-up's knock of a weaker man on the floor
/// (`0x3d0f58`'s four lists, by the quarter the dunker stands in about
/// him: 0 behind, 2 in front).
pub const KNOCK_FLOOR: [&[u8]; 4] = [&[36], &[37, 38], &[39, 40, 41], &[42, 43]];

/// The passer's defender looking behind him at a board passout
/// (callback 53, `0x174ac4..0x174ad0`: `react_fakepasslookbehind_l`).
pub const BOARD_LOOK: u8 = 44;

/// The defender a head passout meets (`FUN_00148378` kind 2,
/// `0x1485ec..0x14861c`): his reaction by `r & 1` (1 the first); in the
/// counter stance he catches it instead (`0x148594..0x1485dc`).
pub const HEAD_HIT: [u8; 2] = [45, 46];
pub const HEAD_CATCH: u8 = 47;

/// The celebrations (`FUN_00170e70`'s `r % 17` of `0x3d9078`, which the
/// boot's constructor fills at `0x17bcd0..0x17c1a8`): each row and the
/// updates its clip shows a frame for (fps / 60 times the constant there).
/// Script `0x44cbbf` plays the row from frame 0 at that rate; they are
/// [`REACTIONS`] from [`CELEBRATE_FIRST`] on, in this order.
pub const CELEBRATE_ROWS: [(u16, f32); 17] = [
    (933, 2.0),
    (931, 3.5),
    (932, 2.5),
    (934, 2.0),
    (935, 4.0),
    (51, 8.5),
    (936, f32::from_bits(0x4019_999a)),
    (937, f32::from_bits(0x4019_999a)),
    (938, f32::from_bits(0x4019_999a)),
    (939, f32::from_bits(0x4019_999a)),
    (940, f32::from_bits(0x4019_999a)),
    (941, f32::from_bits(0x4019_999a)),
    (942, 3.0),
    (943, f32::from_bits(0x4019_999a)),
    (944, f32::from_bits(0x4019_999a)),
    (945, f32::from_bits(0x4019_999a)),
    (946, f32::from_bits(0x4019_999a)),
];
pub const CELEBRATE_FIRST: u8 = 48;

/// The loser's reactions at the win (`FUN_00171160`'s `0x3d9138`, filled
/// at `0x17c3e8..0x17c4a0`): each row, the facings it is barred from (1
/// facing away from the court's centre, 2 facing it) and its rate, played
/// by script `0x44cc03` as [`CELEBRATE_ROWS`]'; [`REACTIONS`] from
/// [`DEJECT_FIRST`] on.
pub const DEJECT_ROWS: [(u16, u8, f32); 4] = [
    (958, 1, 2.0),
    (959, 1, f32::from_bits(0x4019_999a)),
    (960, 2, 2.0),
    (961, 2, f32::from_bits(0x4019_999a)),
];
pub const DEJECT_FIRST: u8 = 65;

/// The knock window's body, `0x3d0d90` row 2 (`0x3d0cd0`, the only row the
/// dunk and lay-up scripts' `a6 1` name): bones `a` to `b` pushed `reach`
/// past `b`, `radius` about them.
pub const KNOCK_BODY: [(usize, usize, f32, f32); 2] = [(20, 21, 3.0, 1.0), (20, 0, 3.0, 2.5)];

/// The held scripts' kind-2 windows' body, `0x3d0d90` row 4 (`0x3d0d30`:
/// the heave shot's and the gamebreaker shot `0x44f6c3`'s `a6 2 4 n`).
pub const SHOVE_BODY: [(usize, usize, f32, f32); 6] = [
    (20, 21, 3.0, f32::from_bits(0x3f59_999a)),
    (20, 0, 3.0, 1.0),
    (0, 8, 1.0, 0.5),
    (0, 4, 1.0, 0.5),
    (12, 15, f32::from_bits(0x4013_3333), 1.0),
    (16, 19, f32::from_bits(0x4013_3333), 1.0),
];

/// The clearout's script in [`SCRIPTS`] (`clearout01`, record 658, script
/// `0x456d02`, `FUN_0015c778`).
pub const CLEAROUT: u16 = 75;

/// The clearout's knock of a defender in front with at least its power:
/// one of `0x3d0de8` (`0x43d0b0`'s quarter 2).
pub const CLEAROUT_BRACED: [u8; 4] = [28, 29, 30, 31];

/// Of a weaker defender: `0x3d0e28` (`0x43d0c0`'s quarter 2),
/// `react_stumbleback2`.
pub const CLEAROUT_STUMBLE: [u8; 1] = [6];

/// The records a defender's power is weighed against the clearout's in
/// (`0x43d0d0`); in any other the roll is the odds (`FUN_00168e78`).
pub const CLEAROUT_STANCES: [u16; 3] = [650, 651, 652];

/// The clearout's arms, `0x3d0d90` row 1 (`0x3d0c70`): bones `a` to `b`
/// pushed `reach` past `b`, `radius` about them.
pub const CLEAROUT_ARMS: [(usize, usize, f32, f32); 6] = [
    (12, 13, f32::from_bits(0x3e99_999a), 0.0),
    (13, 14, f32::from_bits(0x3e99_999a), 0.0),
    (14, 15, f32::from_bits(0x3e99_999a), 1.0),
    (16, 17, f32::from_bits(0x3e99_999a), 0.0),
    (17, 18, f32::from_bits(0x3e99_999a), 0.0),
    (18, 19, f32::from_bits(0x3e4c_cccd), 1.0),
];

