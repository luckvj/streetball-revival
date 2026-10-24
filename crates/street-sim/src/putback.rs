//! The putback (`FUN_00169c98`, raw `0x169c98..0x16b1e0`): the side with
//! the ball's rebound press with two turbos held, at a shot, a ball on
//! the rim or a rebound coming off, slams it back from the air.
//!
//! Its call (`FUN_0015a060`, `0x15b4c0..0x15b53c`): the rebound press by
//! the side with the ball off it, the ball not a pass to himself, on the
//! floor in live play and not in a rebound jump; when the putback starts
//! nothing, the rebound jump does (`FUN_0015ce50`).
//!
//! The test: two turbos held (+0x78 & 0x40000), no teammate dunking,
//! laying up, hanging or leaping (`0xe080`), the ball no gamebreaker's
//! (ball+0x178, `FUN_00141018`), and the ball a shot, on the rim or coming
//! off (states 5, 6, 0); on the court's side of the rim he attacks, 3 to
//! 12.5 from it (game+0x374, `DAT_003d82e4`), his wanted facing within 228
//! of his way to it, and no opponent in his cone (`FUN_00170840` at
//! `(1 − Dunks / 99) × 200 + 100`) within 4.5 of him. Then each of the
//! eleven records of `0x3d8300` (0x120 bytes, built at the boot; read from
//! the save states' RAM, the same in all three) on either side, whose mask
//! holds his sector about the rim (`FUN_0018d768`, word 0x100 or 0x300),
//! unless his Dunks is under 25: the record's points posed at his place,
//! turned to his way to the rim (a yaw quaternion, `FUN_002f7430`) and
//! scaled by his model (the height scale, and the larger of it and the
//! width for the hands), mirrored on the left. The hand at the slam is
//! aimed at a point by the rim (11.67 up, or 11.6 for a two-handed hang;
//! turned by the record's +0xf8 and offset as its hands are spread), the
//! jump's velocity reaching it in the record's +0xf0 updates under the
//! actor's gravity (32.15 / fps² × −1.258); the record takes when the ball
//! (`FUN_001537d0`, its own path stepped ahead) is within the larger of 1.5
//! and the ball's +0x11c of the hand at either of its catch moments
//! (+0xe0 and +0xe8 updates into the jump, after the +0xd0 blend and +0xd8
//! wait). One of those that take is drawn (`rand() % n`) and its side's
//! script started on his channel; actor+0x1e4 is the record's kind (1, a
//! tip slam). Not carried: +0x35c (cleared without the turbos, set when
//! nothing takes for a person), `DAT_003d658c` (3 out of the dunking area,
//! else 2) and +0x1e0.

use crate::{Input, World, distance, pose, timing};

/// One of `0x3d8300`'s records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Putback {
    /// +0x00: the jump's root from his place.
    pub base: [f32; 3],
    /// +0x10..+0xc0, in pairs (a hand offset at the larger scale, the bone
    /// at the height scale): the first catch's two hands (+0x10/+0x20,
    /// +0x30/+0x40), the second's (+0x50..+0x80), the slam's (+0x90..+0xc0).
    pub points: [[f32; 3]; 12],
    /// +0xd0 the blend, +0xd8 the wait, +0xe0 and +0xe8 the catches, +0xf0
    /// the slam, in updates at 60.
    pub counts: [i32; 5],
    /// +0xf8: the slam's turn from his way to the rim.
    pub turn: i32,
    /// +0x100 and +0x108: the right and the left side's scripts, as
    /// [`timing::PUTBACK_ROWS`] indices.
    pub rows: [usize; 2],
    /// +0x104 and +0x10c: the sectors each side takes.
    pub masks: [u32; 2],
    /// +0x110: actor+0x1e4 at the start (1, a tip slam).
    pub kind: i32,
    /// +0x114 and +0x118: the hands of the first catch, and of the second
    /// and the slam (1 the first pair, 2 the second, 3 their middle).
    pub catch_hands: i32,
    pub slam_hands: i32,
}

/// The records, in the table's order.
pub const PUTBACKS: [Putback; 11] = [
    // putback_1h_2f_tomahawk_r
    Putback {
        base: [
            f32::from_bits(0x3ebd6c2f),
            f32::from_bits(0x40793b3a),
            f32::from_bits(0xbfb507a7),
        ],
        points: [
            [
                f32::from_bits(0xbef66d9c),
                f32::from_bits(0x3e0a9542),
                f32::from_bits(0x3d9042d9),
            ],
            [
                f32::from_bits(0xbf5d0614),
                f32::from_bits(0xbebaa0a9),
                f32::from_bits(0x3fd7630b),
            ],
            [
                f32::from_bits(0x3ef6a833),
                f32::from_bits(0x3cefaa47),
                f32::from_bits(0x3e170ef5),
            ],
            [
                f32::from_bits(0x400d10d8),
                f32::from_bits(0x3fd89907),
                f32::from_bits(0x3f5b631b),
            ],
            [
                f32::from_bits(0xbea90300),
                f32::from_bits(0xbe83e857),
                f32::from_bits(0x3e904945),
            ],
            [
                f32::from_bits(0xbfe4ac4b),
                f32::from_bits(0xbecfd7c3),
                f32::from_bits(0xbf5ccee6),
            ],
            [
                f32::from_bits(0x3e8ce1c6),
                f32::from_bits(0x3ec596c8),
                f32::from_bits(0xbe30daa1),
            ],
            [
                f32::from_bits(0x3d8651f4),
                f32::from_bits(0x405e704c),
                f32::from_bits(0xbf4eda88),
            ],
            [
                f32::from_bits(0xbed5eb10),
                f32::from_bits(0xbe544ae8),
                f32::from_bits(0x3e48c6d6),
            ],
            [
                f32::from_bits(0xbfafe8f7),
                f32::from_bits(0xbd464ecf),
                f32::from_bits(0xbf9b80cb),
            ],
            [
                f32::from_bits(0x3e42ce46),
                f32::from_bits(0x3ed27371),
                f32::from_bits(0xbe6451b9),
            ],
            [
                f32::from_bits(0xbf6b530d),
                f32::from_bits(0x403e10cf),
                f32::from_bits(0xbf947504),
            ],
        ],
        counts: [8, 6, 35, 43, 45],
        turn: 128,
        rows: [0, 1],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 1,
        slam_hands: 1,
    },
    // putback_2h_2f_hang_l
    Putback {
        base: [
            f32::from_bits(0x3edd3cbc),
            f32::from_bits(0x4072445f),
            f32::from_bits(0xbfa86b6a),
        ],
        points: [
            [
                f32::from_bits(0xbea25cc4),
                f32::from_bits(0x3ec94e3c),
                f32::from_bits(0x3bcb1034),
            ],
            [
                f32::from_bits(0x3f86fa9c),
                f32::from_bits(0x4042b713),
                f32::from_bits(0xbfaf8b2f),
            ],
            [
                f32::from_bits(0x3efe17e3),
                f32::from_bits(0x3a917d6b),
                f32::from_bits(0xbda4b557),
            ],
            [
                f32::from_bits(0x3f70c9eb),
                f32::from_bits(0x403ff10f),
                f32::from_bits(0xbf9d2514),
            ],
            [
                f32::from_bits(0xbec3c1ce),
                f32::from_bits(0x3e9cd76b),
                f32::from_bits(0x3dfc5ac4),
            ],
            [
                f32::from_bits(0xbf91c793),
                f32::from_bits(0x4047de44),
                f32::from_bits(0xbfe25da7),
            ],
            [
                f32::from_bits(0x3e9fe026),
                f32::from_bits(0x3e963da3),
                f32::from_bits(0xbe8780dc),
            ],
            [
                f32::from_bits(0xbf83ca19),
                f32::from_bits(0x404b70e3),
                f32::from_bits(0xbfd4e936),
            ],
            [
                f32::from_bits(0xbed6e0b1),
                f32::from_bits(0x3e85c098),
                f32::from_bits(0x3dda176e),
            ],
            [
                f32::from_bits(0xbfd6b285),
                f32::from_bits(0x40154be8),
                f32::from_bits(0xbfe4c95c),
            ],
            [
                f32::from_bits(0x3e0cf399),
                f32::from_bits(0x3e95cd70),
                f32::from_bits(0xbec6c6bd),
            ],
            [
                f32::from_bits(0xbfc1e28b),
                f32::from_bits(0x40157c95),
                f32::from_bits(0xbfeeae6d),
            ],
        ],
        counts: [8, 5, 37, 43, 45],
        turn: 128,
        rows: [2, 3],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 3,
    },
    // putback_2h_2f_windmill_hang
    Putback {
        base: [
            f32::from_bits(0x3e16c54c),
            f32::from_bits(0x408608a2),
            f32::from_bits(0xbfdbacf7),
        ],
        points: [
            [
                f32::from_bits(0xbe2d2bf5),
                f32::from_bits(0x3e06e7a3),
                f32::from_bits(0x3ee9e53b),
            ],
            [
                f32::from_bits(0xbf94e621),
                f32::from_bits(0x3fd5e072),
                f32::from_bits(0xbf39610b),
            ],
            [
                f32::from_bits(0x3e4b8b69),
                f32::from_bits(0xbbe94680),
                f32::from_bits(0xbeec1872),
            ],
            [
                f32::from_bits(0xbfab48dc),
                f32::from_bits(0x3fc803f7),
                f32::from_bits(0xbf3650a4),
            ],
            [
                f32::from_bits(0x3d5f03d1),
                f32::from_bits(0x3e390364),
                f32::from_bits(0x3eef1173),
            ],
            [
                f32::from_bits(0x3effb246),
                f32::from_bits(0x4054628d),
                f32::from_bits(0xbedd043e),
            ],
            [
                f32::from_bits(0x3e3ab9b2),
                f32::from_bits(0x3e364f98),
                f32::from_bits(0xbedd2aa6),
            ],
            [
                f32::from_bits(0x3e2ce63a),
                f32::from_bits(0x40538544),
                f32::from_bits(0xbeafebaf),
            ],
            [
                f32::from_bits(0x3da424a2),
                f32::from_bits(0x3e44e5d6),
                f32::from_bits(0x3eeb42cc),
            ],
            [
                f32::from_bits(0xbecb8a1a),
                f32::from_bits(0x40403c97),
                f32::from_bits(0x3f019168),
            ],
            [
                f32::from_bits(0x3c65c4eb),
                f32::from_bits(0x3ea4ad15),
                f32::from_bits(0xbec63a3b),
            ],
            [
                f32::from_bits(0xbed7bc3c),
                f32::from_bits(0x40447454),
                f32::from_bits(0x3ec2cc0c),
            ],
        ],
        counts: [8, 8, 16, 36, 40],
        turn: 256,
        rows: [4, 4],
        masks: [0x60, 0xc],
        kind: 1,
        catch_hands: 3,
        slam_hands: 3,
    },
    // putback_1h_2f_scoop
    Putback {
        base: [
            f32::from_bits(0x3e14e7ab),
            f32::from_bits(0x40887a8b),
            f32::from_bits(0xbfa6a50d),
        ],
        points: [
            [
                f32::from_bits(0xbe84404a),
                f32::from_bits(0xbe3783e0),
                f32::from_bits(0x3ec9bfbe),
            ],
            [
                f32::from_bits(0xc0082b5a),
                f32::from_bits(0x3fde89ca),
                f32::from_bits(0xbd2d96a7),
            ],
            [
                f32::from_bits(0x3c6afab5),
                f32::from_bits(0x3ee85a4f),
                f32::from_bits(0xbe5f7404),
            ],
            [
                f32::from_bits(0xbfd76fd2),
                f32::from_bits(0x4047fa44),
                f32::from_bits(0x3f224ba9),
            ],
            [
                f32::from_bits(0xbd285986),
                f32::from_bits(0xbe6a64c3),
                f32::from_bits(0x3ee5859d),
            ],
            [
                f32::from_bits(0xbf43de94),
                f32::from_bits(0xbf7b7729),
                f32::from_bits(0x3f8cc543),
            ],
            [
                f32::from_bits(0x3e4f8b9c),
                f32::from_bits(0x3ea6dd05),
                f32::from_bits(0xbea812be),
            ],
            [
                f32::from_bits(0xbe2b9b67),
                f32::from_bits(0x405671f8),
                f32::from_bits(0xbdeeab7a),
            ],
            [
                f32::from_bits(0xbe16da45),
                f32::from_bits(0xbe4ecdd1),
                f32::from_bits(0x3ee026cc),
            ],
            [
                f32::from_bits(0xbf5dd851),
                f32::from_bits(0xbf326eeb),
                f32::from_bits(0x3f0a3f14),
            ],
            [
                f32::from_bits(0x3e06a6a0),
                f32::from_bits(0x3ebc1bfc),
                f32::from_bits(0xbea3317f),
            ],
            [
                f32::from_bits(0xbfacbb45),
                f32::from_bits(0x40375b9f),
                f32::from_bits(0xbeda1965),
            ],
        ],
        counts: [8, 6, 14, 37, 40],
        turn: 256,
        rows: [5, 5],
        masks: [0x60, 0xc],
        kind: 1,
        catch_hands: 1,
        slam_hands: 1,
    },
    // putback_1h_2f_taprim
    Putback {
        base: [
            f32::from_bits(0xbd62ad3f),
            f32::from_bits(0x4089095b),
            f32::from_bits(0xbfc2b9d4),
        ],
        points: [
            [
                f32::from_bits(0xbeeaf2fa),
                f32::from_bits(0x3e4ffa7f),
                f32::from_bits(0x3d8625ea),
            ],
            [
                f32::from_bits(0xbf187831),
                f32::from_bits(0x4048ad9f),
                f32::from_bits(0xbf8217b1),
            ],
            [
                f32::from_bits(0x3ef9b1b8),
                f32::from_bits(0x3e0343fa),
                f32::from_bits(0xbca7418d),
            ],
            [
                f32::from_bits(0xbe67d806),
                f32::from_bits(0x402a2a41),
                f32::from_bits(0x3ef2871a),
            ],
            [
                f32::from_bits(0xbeb0867b),
                f32::from_bits(0xbdd21f6d),
                f32::from_bits(0x3eb541fa),
            ],
            [
                f32::from_bits(0xbf6214f0),
                f32::from_bits(0x4019aa00),
                f32::from_bits(0xbe1155d6),
            ],
            [
                f32::from_bits(0x3e802752),
                f32::from_bits(0x3ec0a45d),
                f32::from_bits(0xbe63afb8),
            ],
            [
                f32::from_bits(0xbf6171d2),
                f32::from_bits(0x40559ec7),
                f32::from_bits(0xbe7b5244),
            ],
            [
                f32::from_bits(0xbea3482c),
                f32::from_bits(0xbdfd33d3),
                f32::from_bits(0x3ebe32e4),
            ],
            [
                f32::from_bits(0xbf78f67b),
                f32::from_bits(0x3fe68733),
                f32::from_bits(0xbe2b039f),
            ],
            [
                f32::from_bits(0x3e6176de),
                f32::from_bits(0x3ec230db),
                f32::from_bits(0xbe7e5711),
            ],
            [
                f32::from_bits(0xbf56b4ea),
                f32::from_bits(0x403b128c),
                f32::from_bits(0xbe80294a),
            ],
        ],
        counts: [8, 8, 18, 38, 40],
        turn: 256,
        rows: [6, 6],
        masks: [0x60, 0xc],
        kind: 1,
        catch_hands: 1,
        slam_hands: 1,
    },
    // putback_1h_2f_2handcatch_r
    Putback {
        base: [
            f32::from_bits(0xbf015726),
            f32::from_bits(0x40861655),
            f32::from_bits(0xbfd4ee52),
        ],
        points: [
            [
                f32::from_bits(0xbed3acd6),
                f32::from_bits(0x3e83c2fc),
                f32::from_bits(0xbe0ac9b0),
            ],
            [
                f32::from_bits(0x3fa36ce8),
                f32::from_bits(0x40270874),
                f32::from_bits(0xbef097ea),
            ],
            [
                f32::from_bits(0x3eee0fe4),
                f32::from_bits(0x3c42656b),
                f32::from_bits(0x3e48b9bb),
            ],
            [
                f32::from_bits(0x3f89c433),
                f32::from_bits(0x403112ae),
                f32::from_bits(0xbe4d1aeb),
            ],
            [
                f32::from_bits(0xbec4db59),
                f32::from_bits(0xbe8c219f),
                f32::from_bits(0xbe3b24ea),
            ],
            [
                f32::from_bits(0xbe4608d1),
                f32::from_bits(0xbec37a3e),
                f32::from_bits(0xbff1a9ca),
            ],
            [
                f32::from_bits(0x3e983409),
                f32::from_bits(0x3ebab9d4),
                f32::from_bits(0x3e39dece),
            ],
            [
                f32::from_bits(0xbc106467),
                f32::from_bits(0x406b410b),
                f32::from_bits(0xbd9f0564),
            ],
            [
                f32::from_bits(0xbece742a),
                f32::from_bits(0xbe89a976),
                f32::from_bits(0xbe107358),
            ],
            [
                f32::from_bits(0xbea1bf16),
                f32::from_bits(0xbf04996b),
                f32::from_bits(0xbf946890),
            ],
            [
                f32::from_bits(0x3e8fe005),
                f32::from_bits(0x3ed1348b),
                f32::from_bits(0x3dbc4b90),
            ],
            [
                f32::from_bits(0xbe804dfd),
                f32::from_bits(0x404f3598),
                f32::from_bits(0xbf4740b3),
            ],
        ],
        counts: [8, 6, 7, 37, 40],
        turn: 0,
        rows: [7, 8],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 1,
    },
    // putback_1h_2f_2handcatch_hang_r
    Putback {
        base: [
            f32::from_bits(0x3e86e6b8),
            f32::from_bits(0x407a3366),
            f32::from_bits(0xbfd470e3),
        ],
        points: [
            [
                f32::from_bits(0xbee2f252),
                f32::from_bits(0x3e75d420),
                f32::from_bits(0x3d0dd2f2),
            ],
            [
                f32::from_bits(0x3f401398),
                f32::from_bits(0x40579cb7),
                f32::from_bits(0x3f13033a),
            ],
            [
                f32::from_bits(0x3eeda837),
                f32::from_bits(0x3d4ad688),
                f32::from_bits(0x3e439fba),
            ],
            [
                f32::from_bits(0x3f1e974a),
                f32::from_bits(0x405be182),
                f32::from_bits(0x3f0711b6),
            ],
            [
                f32::from_bits(0xbedf3d5c),
                f32::from_bits(0xbe46e6da),
                f32::from_bits(0x3e2cf851),
            ],
            [
                f32::from_bits(0xbfd100ce),
                f32::from_bits(0x3f23be34),
                f32::from_bits(0xbfd6c67a),
            ],
            [
                f32::from_bits(0x3e97e953),
                f32::from_bits(0x3ecb0f8c),
                f32::from_bits(0xbdc369fd),
            ],
            [
                f32::from_bits(0xbebd9019),
                f32::from_bits(0x405ee70c),
                f32::from_bits(0xbf8f95d5),
            ],
            [
                f32::from_bits(0xbedfcf3e),
                f32::from_bits(0xbe6f61ed),
                f32::from_bits(0x3dd58e22),
            ],
            [
                f32::from_bits(0xbf461d65),
                f32::from_bits(0xbddc01a3),
                f32::from_bits(0xbfbb35a0),
            ],
            [
                f32::from_bits(0x3e92e3e7),
                f32::from_bits(0x3ec905c0),
                f32::from_bits(0xbe0ec354),
            ],
            [
                f32::from_bits(0xbe884ee8),
                f32::from_bits(0x403d8beb),
                f32::from_bits(0xbfb42da5),
            ],
        ],
        counts: [8, 6, 25, 43, 45],
        turn: 128,
        rows: [9, 10],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 1,
    },
    // putback_1h_2f_hang
    Putback {
        base: [
            f32::from_bits(0x3eaf00ef),
            f32::from_bits(0x40877176),
            f32::from_bits(0xbfb8582e),
        ],
        points: [
            [
                f32::from_bits(0xbede0cbf),
                f32::from_bits(0x3d87075b),
                f32::from_bits(0x3e81a75d),
            ],
            [
                f32::from_bits(0xbfbe4f33),
                f32::from_bits(0x4006013f),
                f32::from_bits(0xbe54e7ef),
            ],
            [
                f32::from_bits(0x3eed0ebb),
                f32::from_bits(0x3e32877f),
                f32::from_bits(0xbdd39219),
            ],
            [
                f32::from_bits(0x3f8ec78f),
                f32::from_bits(0x404c0a85),
                f32::from_bits(0xbf6bec89),
            ],
            [
                f32::from_bits(0xbea93b5c),
                f32::from_bits(0xbde894c4),
                f32::from_bits(0x3ebb9ae1),
            ],
            [
                f32::from_bits(0xc009454a),
                f32::from_bits(0x3ea90c4e),
                f32::from_bits(0x3e994079),
            ],
            [
                f32::from_bits(0x3ea8def4),
                f32::from_bits(0x3e90198f),
                f32::from_bits(0xbe851d04),
            ],
            [
                f32::from_bits(0xbf17a5a0),
                f32::from_bits(0x40564a1b),
                f32::from_bits(0xbf7f27ee),
            ],
            [
                f32::from_bits(0xbeb81a9c),
                f32::from_bits(0xbdede871),
                f32::from_bits(0x3eaa9414),
            ],
            [
                f32::from_bits(0xbfc1ea92),
                f32::from_bits(0xbe1e5abc),
                f32::from_bits(0x3f337186),
            ],
            [
                f32::from_bits(0x3e99f080),
                f32::from_bits(0x3ea23b58),
                f32::from_bits(0xbe800f77),
            ],
            [
                f32::from_bits(0xbf6a1f3a),
                f32::from_bits(0x4039bf34),
                f32::from_bits(0xbf7ba4a5),
            ],
        ],
        counts: [8, 6, 32, 38, 40],
        turn: 256,
        rows: [11, 11],
        masks: [0x60, 0xc],
        kind: 1,
        catch_hands: 1,
        slam_hands: 1,
    },
    // putback_2h_2f_hang_2_l
    Putback {
        base: [
            f32::from_bits(0x3ed9b3d0),
            f32::from_bits(0x40828948),
            f32::from_bits(0xbfe3919b),
        ],
        points: [
            [
                f32::from_bits(0xbe9623d1),
                f32::from_bits(0x3d9db984),
                f32::from_bits(0x3ecd629e),
            ],
            [
                f32::from_bits(0x3f4dbf27),
                f32::from_bits(0x40100dd8),
                f32::from_bits(0x3f8217a9),
            ],
            [
                f32::from_bits(0x3ee8d10f),
                f32::from_bits(0x3e04c1a9),
                f32::from_bits(0xbe337d20),
            ],
            [
                f32::from_bits(0x3f4c9fd8),
                f32::from_bits(0x40054fc6),
                f32::from_bits(0x3f9365a1),
            ],
            [
                f32::from_bits(0xbe6e9857),
                f32::from_bits(0x3e3a39c5),
                f32::from_bits(0x3ecff25e),
            ],
            [
                f32::from_bits(0xbf6202eb),
                f32::from_bits(0x4052ca92),
                f32::from_bits(0xbeb2f3e4),
            ],
            [
                f32::from_bits(0x3e36a876),
                f32::from_bits(0x3ea1bbcf),
                f32::from_bits(0xbeb29eb2),
            ],
            [
                f32::from_bits(0xbf8895af),
                f32::from_bits(0x404ea9bd),
                f32::from_bits(0xbe868295),
            ],
            [
                f32::from_bits(0xbe613986),
                f32::from_bits(0x3e297fe9),
                f32::from_bits(0x3ed74b84),
            ],
            [
                f32::from_bits(0xbf784ae8),
                f32::from_bits(0x4036b4dd),
                f32::from_bits(0xbd91cf35),
            ],
            [
                f32::from_bits(0x3e0846e9),
                f32::from_bits(0x3ea7179c),
                f32::from_bits(0xbeb85d74),
            ],
            [
                f32::from_bits(0xbf78f00f),
                f32::from_bits(0x403be0d6),
                f32::from_bits(0xbdfa9c13),
            ],
        ],
        counts: [8, 6, 37, 43, 45],
        turn: 128,
        rows: [12, 13],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 3,
    },
    // putback_2h_2f_hang_swing_l
    Putback {
        base: [
            f32::from_bits(0x3e7e64b2),
            f32::from_bits(0x40869462),
            f32::from_bits(0xbfcedb1f),
        ],
        points: [
            [
                f32::from_bits(0xbed78162),
                f32::from_bits(0x3e0ff1d8),
                f32::from_bits(0x3e77ff9b),
            ],
            [
                f32::from_bits(0xbe04aa97),
                f32::from_bits(0x405c815e),
                f32::from_bits(0x3f258d58),
            ],
            [
                f32::from_bits(0x3ef4825a),
                f32::from_bits(0x3e24fe37),
                f32::from_bits(0x3ce52589),
            ],
            [
                f32::from_bits(0xbea2749f),
                f32::from_bits(0x405b4b49),
                f32::from_bits(0x3f4ef7d3),
            ],
            [
                f32::from_bits(0xbed52e73),
                f32::from_bits(0x3e77caa7),
                f32::from_bits(0x3e17342f),
            ],
            [
                f32::from_bits(0xbf134b30),
                f32::from_bits(0x402ae8cd),
                f32::from_bits(0xbff3ee24),
            ],
            [
                f32::from_bits(0x3ec46de7),
                f32::from_bits(0x3e7a3726),
                f32::from_bits(0xbe5d4888),
            ],
            [
                f32::from_bits(0xbf868ff3),
                f32::from_bits(0x4035ae21),
                f32::from_bits(0xbfdec20d),
            ],
            [
                f32::from_bits(0xbed3eb5b),
                f32::from_bits(0x3e7080b6),
                f32::from_bits(0x3e2c3dab),
            ],
            [
                f32::from_bits(0xbf2cfbec),
                f32::from_bits(0x402ec6ce),
                f32::from_bits(0xbfb999dd),
            ],
            [
                f32::from_bits(0x3ec602c9),
                f32::from_bits(0x3e6e3194),
                f32::from_bits(0xbe65a78f),
            ],
            [
                f32::from_bits(0xbf3e424a),
                f32::from_bits(0x402b4428),
                f32::from_bits(0xbfd2e32e),
            ],
        ],
        counts: [8, 6, 31, 43, 45],
        turn: 128,
        rows: [14, 15],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 3,
    },
    // putback_2h_2f_hang_3_l
    Putback {
        base: [
            f32::from_bits(0x3e29df98),
            f32::from_bits(0x407c2bae),
            f32::from_bits(0xbf9d5c10),
        ],
        points: [
            [
                f32::from_bits(0xbebecf42),
                f32::from_bits(0x3e809acc),
                f32::from_bits(0x3e6bf3bf),
            ],
            [
                f32::from_bits(0x3ef8a47f),
                f32::from_bits(0x406950db),
                f32::from_bits(0x3e8a73b4),
            ],
            [
                f32::from_bits(0x3ef059ea),
                f32::from_bits(0x3e38b652),
                f32::from_bits(0x3cb22292),
            ],
            [
                f32::from_bits(0x3ee2ba7c),
                f32::from_bits(0x406c7b63),
                f32::from_bits(0x3ec9f9f0),
            ],
            [
                f32::from_bits(0xbebd74b4),
                f32::from_bits(0x3e9d89ce),
                f32::from_bits(0x3e189df1),
            ],
            [
                f32::from_bits(0xbf639fcb),
                f32::from_bits(0x404b6fdf),
                f32::from_bits(0xbfca4acf),
            ],
            [
                f32::from_bits(0x3ea6427c),
                f32::from_bits(0x3e99d580),
                f32::from_bits(0xbe7681aa),
            ],
            [
                f32::from_bits(0xbf5ac2bd),
                f32::from_bits(0x4052b50f),
                f32::from_bits(0xbfc1822c),
            ],
            [
                f32::from_bits(0xbed019d2),
                f32::from_bits(0x3e93484d),
                f32::from_bits(0x3daa411c),
            ],
            [
                f32::from_bits(0xbf9cff22),
                f32::from_bits(0x4025efe5),
                f32::from_bits(0xbfcebf98),
            ],
            [
                f32::from_bits(0x3e8c3e75),
                f32::from_bits(0x3e86049f),
                f32::from_bits(0xbeaa21a7),
            ],
            [
                f32::from_bits(0xbf821a72),
                f32::from_bits(0x4024cc3a),
                f32::from_bits(0xbfe24c38),
            ],
        ],
        counts: [8, 5, 31, 43, 45],
        turn: 128,
        rows: [16, 17],
        masks: [0x70, 0x1c],
        kind: 1,
        catch_hands: 3,
        slam_hands: 3,
    },
];

/// The jump's gravity factor (`0x16a910..0x16a944`): 32.15 / fps² times
/// −1.258.
fn gravity() -> f32 {
    let fps = crate::TICK_HZ as f32;
    f32::from_bits(0x4200_9be3) / (fps * fps) * f32::from_bits(0xbfa1_3ba4)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

impl World {
    /// `FUN_00169c98(actor)`: whether `i`'s rebound press starts a
    /// putback ([`crate::putback`]); true when it did.
    pub(crate) fn putback(&mut self, i: usize, input: &Input) -> bool {
        if input.turbos() < 2 {
            return false;
        }
        let team = self.players[i].team;
        let busy = (0..self.players.len()).any(|j| {
            j != i
                && self.players[j].team == team
                && (crate::block::holder_bits(&self.players[j].action) & 0xe080 != 0
                    || self.players[j].leaping())
        });
        if busy || self.ball.shot.is_some_and(|s| s.gamebreaker != 0) {
            return false;
        }
        let Some(candidates) = self.putback_candidates(i) else {
            return false;
        };
        if candidates.is_empty() {
            return false;
        }
        let r = self.rng.next_u32();
        let (k, m) = candidates[(r % candidates.len() as u32) as usize];
        let row = timing::PUTBACK_FIRST + PUTBACKS[k].rows[m] as u8;
        let Some(r) = timing::move_row(row) else {
            return false;
        };
        let hoop = crate::target_hoop(team);
        let spot = self.rim_side(i, hoop, 1.2);
        self.begin_move(i, row);
        self.players[i].start_dunk(
            hoop,
            spot,
            crate::player::DunkStyle::Putback,
            (row, r.release, r.end),
        );
        self.events.push(crate::Event::Dunk { player: i, hoop });
        true
    }

    /// The records that take for `i` (`0x169d8c..0x16b064`), each as
    /// (record, side); none when the place or the ball refuses them all.
    fn putback_candidates(&self, i: usize) -> Option<Vec<(usize, usize)>> {
        let p = &self.players[i];
        let state = self.native_ball_state();
        if !matches!(state, 6 | 0 | 5) {
            return None;
        }
        let toward = self.toward_rim(i);
        let x = p.position[0];
        let front =
            (x > 0.0 && (512..=1024).contains(&toward)) || (x < 0.0 && (0..=512).contains(&toward));
        if !front {
            return None;
        }
        let d = self.offence_rim_distance(i);
        if !(3.0..=f32::from_bits(0x4148_0000)).contains(&d) {
            return None;
        }
        let facing = timing::facing_word(p.wanted);
        if timing::angle_diff(toward, facing).abs() >= f32::from_bits(0x4364_0e0f) as i32 {
            return None;
        }
        let dunks = p.ratings.native(2);
        let width = ((1.0 - dunks as f32 / f32::from_bits(0x42c6_0000)) * 200.0 + 100.0) as i32;
        if let Some(o) = self.nearest_opponent(i)
            && self.in_cone(i, o, width)
            && distance(self.players[o].position, p.position) <= 4.5
        {
            return None;
        }
        let rim = self.rim_spot(crate::target_hoop(p.team));
        let rim3 = [rim[0], 0.0, rim[1]];
        let body = p.body();
        let (f22, f23) = (body.height, body.height.max(body.width));
        let tol = self.ball.radius.max(1.5);
        let word = if rim[0] >= 0.0 { 0x100 } else { 0x300 };
        let bit = 1u32 << timing::sector(word, rim, p.position);
        let low = dunks < 25;
        let half = toward / 2;
        let q = [
            0.0,
            crate::input::native_sin_1024(half),
            0.0,
            crate::input::native_cos_1024(half),
        ];
        let me = [p.position[0], p.height, p.position[1]];
        let g = gravity();
        let ball_at = |n: i32| {
            let n = n.max(1) as usize;
            crate::rebound::ball_path(self, n)[n - 1].0
        };
        let mut out = Vec::new();
        for fp in 0..22 {
            let (k, m) = (fp / 2, fp % 2);
            let rec = &PUTBACKS[k];
            if low && rec.kind == 1 {
                continue;
            }
            if rec.masks[m] & bit == 0 {
                continue;
            }
            let mirror = |mut v: [f32; 3]| {
                if m == 1 {
                    v[0] = -v[0];
                }
                v
            };
            let pair = |a: usize| {
                let v = add(scale(rec.points[a], f23), scale(rec.points[a + 1], f22));
                pose::rotate(q, mirror(v))
            };
            let base = add(pose::rotate(q, mirror(scale(rec.base, f22))), me);
            // The slam's hands.
            let (hand, spread) = match rec.slam_hands {
                3 => {
                    let (a, b) = (pair(8), pair(10));
                    let s = [b[0] - a[0], b[2] - a[2]];
                    (scale(add(a, b), 0.5), Some(s))
                }
                2 => (pair(8), None),
                _ => (pair(10), None),
            };
            let hand = add(hand, base);
            let mut s3 = (toward + 512) & 0x3ff;
            s3 = if m == 0 { s3 + rec.turn } else { s3 - rec.turn } & 0x3ff;
            let two = rec.kind == 1 && rec.slam_hands == 3;
            if two && let Some(s) = spread {
                // Square to the hands' line (`0x16a4ac..0x16a71c`).
                let a = timing::angle_units(crate::math::atan2(s[0], s[1])) & 0x3ff;
                let (lo, hi) = ((a - 256) & 0x3ff, (a + 256) & 0x3ff);
                s3 = if timing::angle_diff(lo, s3).abs() < timing::angle_diff(hi, s3).abs() {
                    lo
                } else {
                    hi
                };
            }
            let height = if two {
                f32::from_bits(0x4139_999a)
            } else {
                f32::from_bits(0x413a_aaae)
            };
            let off = if rec.kind == 1 {
                if two {
                    let half = spread.map_or(0.0, |s| crate::math::len(s[0], s[1]) * 0.5);
                    let o = if half < f32::from_bits(0x3f64_aa65) {
                        (f32::from_bits(0x3f4c_3ff5) - half * half).sqrt()
                    } else {
                        0.0
                    };
                    o.max(f32::from_bits(0x3e22_73d6))
                } else {
                    f32::from_bits(0x3f82_5532)
                }
            } else {
                self.ball.radius + f32::from_bits(0x4001_2a99)
            };
            let at = [
                rim3[0] - off * crate::input::native_sin_1024(s3),
                height,
                rim3[2] - off * crate::input::native_cos_1024(s3),
            ];
            let t = rec.counts[4] as f32;
            let mut v = scale([at[0] - hand[0], at[1] - hand[1], at[2] - hand[2]], 1.0 / t);
            v[1] -= g * 0.5 * t;
            let reaches = |hands: i32, first: usize, frames: i32| {
                let c = match hands {
                    3 => scale(add(pair(first), pair(first + 2)), 0.5),
                    2 => pair(first),
                    _ => pair(first + 2),
                };
                let tf = frames as f32;
                let mut c = add(add(c, base), scale(v, tf));
                c[1] += g * 0.5 * tf * tf;
                let n = rec.counts[0] + rec.counts[1] + frames;
                let b = ball_at(n);
                crate::math::len3(b[0] - c[0], b[1] - c[1], b[2] - c[2]) < tol
            };
            if reaches(rec.catch_hands, 0, rec.counts[2])
                || reaches(rec.slam_hands, 4, rec.counts[3])
            {
                out.push((k, m));
            }
        }
        Some(out)
    }
}
