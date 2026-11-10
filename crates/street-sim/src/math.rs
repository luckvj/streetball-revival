//! The maths the simulation uses, from `libm` rather than the platform's
//! library, so every machine computes the same results bit for bit. Basic
//! arithmetic and `sqrt` are exact in IEEE 754 everywhere; `sin`, `cos` and
//! `atan2` from the C runtime are not, and would let online games drift apart.

pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}

pub fn cos(x: f32) -> f32 {
    libm::cosf(x)
}

pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

/// Length of (x, z).
pub fn len(x: f32, z: f32) -> f32 {
    (x * x + z * z).sqrt()
}

/// Length of (x, y, z), as VU0 takes it (the squares summed, then the
/// root).
pub fn len3(x: f32, y: f32, z: f32) -> f32 {
    (x * x + y * y + z * z).sqrt()
}

/// The EE's `add.s` as PCSX2 runs it: the smaller operand keeps one guard
/// bit past the larger's, and the sum chops (rounds toward zero). The
/// twin of `street_assets::ee::add` (the simulation reads no disc crate).
pub fn ee_add(mut a: f32, mut b: f32) -> f32 {
    let difference = ((a.to_bits() >> 23) & 255) as i32 - ((b.to_bits() >> 23) & 255) as i32;
    if difference >= 25 {
        b = f32::from_bits(b.to_bits() & 0x8000_0000);
    } else if difference <= -25 {
        a = f32::from_bits(a.to_bits() & 0x8000_0000);
    } else if difference > 0 {
        b = f32::from_bits(b.to_bits() & (u32::MAX << (difference - 1)));
    } else if difference < 0 {
        a = f32::from_bits(a.to_bits() & (u32::MAX << (-difference - 1)));
    }
    let exact = a as f64 + b as f64;
    let rounded = exact as f32;
    if rounded.is_finite() && (rounded.abs() as f64) > exact.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

/// An exact result chopped to f32 (rounded toward zero), as the EE's FPU
/// rounds.
fn chop(exact: f64) -> f32 {
    let rounded = exact as f32;
    if rounded.is_finite() && (rounded.abs() as f64) > exact.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

/// VU0's `vadd` lane as PCSX2 runs it: the exact sum chopped, no guard
/// bit.
pub fn vu_add(a: f32, b: f32) -> f32 {
    chop(a as f64 + b as f64)
}

/// VU0's `vrsqrt` of a lane (`1 / sqrt(x)`), chopped.
pub fn vu_rsqrt(x: f32) -> f32 {
    chop(1.0 / (x as f64).sqrt())
}

/// VU0's `vmul` lane: the exact product chopped.
pub fn vu_mul(a: f32, b: f32) -> f32 {
    chop(a as f64 * b as f64)
}

/// The EE's `mul.s`: the exact product chopped.
pub fn ee_mul(a: f32, b: f32) -> f32 {
    chop(a as f64 * b as f64)
}

/// The EE's `div.s`, chopped.
pub fn ee_div(a: f32, b: f32) -> f32 {
    chop(a as f64 / b as f64)
}

/// The EE's `sqrt.s`, chopped (of the magnitude: it takes no sign test).
pub fn ee_sqrt(a: f32) -> f32 {
    chop((a.abs() as f64).sqrt())
}

/// The shot code's sine and cosine (`FUN_001505a0` raw `0x1505b4..0x150718`,
/// inlined again in `FUN_0014fc00`): the angle's nearest quarter turn
/// (`× 2/π`, ± 0.5, truncated), a degree-9 sine polynomial of what is
/// left, the cosine as `√(1 − s²)`, the pair swapped and signed by the
/// quadrant; in the EE's chopped arithmetic.
pub fn ee_sincos(angle: f32) -> (f32, f32) {
    let mut quarter = ee_mul(angle, f32::from_bits(0x3f22_f983));
    quarter = ee_add(quarter, if angle < 0.0 { -0.5 } else { 0.5 });
    let q = quarter as i32;
    let x = ee_add(angle, -ee_mul(q as f32, f32::from_bits(0x3fc9_0fdb)));
    let x2 = ee_mul(x, x);
    let mut p = ee_add(
        ee_mul(x2, f32::from_bits(0x3638_ef1d)),
        f32::from_bits(0xb950_0d01),
    );
    p = ee_add(ee_mul(p, x2), f32::from_bits(0x3c08_8889));
    p = ee_add(ee_mul(p, x2), f32::from_bits(0xbe2a_aaab));
    p = ee_add(ee_mul(p, x2), 1.0);
    let s = ee_mul(p, x);
    let c = ee_sqrt(ee_add(1.0, -ee_mul(s, s)));
    match q & 3 {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// The shot launcher's tangent: [`ee_sincos`]'s sine over its cosine.
pub fn ee_tan(angle: f32) -> f32 {
    let (sin, cos) = ee_sincos(angle);
    ee_div(sin, cos)
}

/// `t·(c0 + t·(c1 + t·(c2 + t·c3)))` on the arcsine table at `0x4b3100`
/// (`c0` also read as `-0x7ef0(gp)`).
fn arc_series(t: f32) -> f32 {
    let c = [0x3e2b_ba25, 0x3d8a_908b, 0x3d55_033a, 0x3d61_37ab].map(f32::from_bits);
    let mut p = ee_add(c[2], ee_mul(t, c[3]));
    p = ee_add(c[1], ee_mul(t, p));
    p = ee_add(c[0], ee_mul(t, p));
    ee_mul(t, p)
}

/// The game's arcsine (`FUN_0014fc00` raw `0x14fc78..0x14fdbc`): past
/// ±1/√2 from the cosine side, `±(π/2 − √t·(1 + series(t)))` with
/// `t = 1 − x²`; within, `x + x·series(x²)`.
pub fn ee_asin(x: f32) -> f32 {
    let half_pi = f32::from_bits(0x3fc9_0fdb);
    let edge = f32::from_bits(0x3f35_04f3);
    if x < -edge || edge < x {
        let t = ee_add(1.0, -ee_mul(x, x));
        let r = ee_mul(ee_sqrt(t), ee_add(arc_series(t), 1.0));
        if x < -edge {
            ee_add(r, -half_pi)
        } else {
            ee_add(half_pi, -r)
        }
    } else {
        ee_add(ee_mul(arc_series(ee_mul(x, x)), x), x)
    }
}

/// The game's arctangent (`FUN_0014fc00` raw `0x14ff4c..0x1500a0` and
/// `0x150420..0x150574`): the arcsine series of its sine squared,
/// `t = x²/(x² + 1)` within ±1 (its sign the argument's), past ±1 from
/// the other side with `t = 1/(x² + 1)`.
pub fn ee_atan(x: f32) -> f32 {
    let half_pi = f32::from_bits(0x3fc9_0fdb);
    let x2 = ee_mul(x, x);
    let one_more = ee_add(x2, 1.0);
    if x < -1.0 || 1.0 < x {
        let t = ee_div(1.0, one_more);
        let r = ee_mul(ee_sqrt(t), ee_add(arc_series(t), 1.0));
        if x < -1.0 {
            ee_add(r, -half_pi)
        } else {
            ee_add(half_pi, -r)
        }
    } else {
        let t = ee_div(x2, one_more);
        let r = ee_mul(ee_sqrt(t), ee_add(arc_series(t), 1.0));
        if x < 0.0 { -r } else { r }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_launchers_tangent_follows_tan() {
        for k in -30..=30 {
            let a = k as f32 * 0.05;
            let ours = super::ee_tan(a);
            let exact = (a as f64).tan() as f32;
            assert!(
                (ours - exact).abs() <= 2e-5 * exact.abs().max(1.0),
                "{a}: {ours} {exact}"
            );
        }
    }

    #[test]
    fn the_games_arcsine_and_arctangent_follow_libm() {
        for k in -20..=20 {
            let x = k as f32 * 0.05;
            let (ours, exact) = (super::ee_asin(x), (x as f64).asin() as f32);
            assert!((ours - exact).abs() <= 2e-4, "asin {x}: {ours} {exact}");
        }
        for k in -40..=40 {
            let x = k as f32 * 0.25;
            let (ours, exact) = (super::ee_atan(x), (x as f64).atan() as f32);
            assert!((ours - exact).abs() <= 2e-4, "atan {x}: {ours} {exact}");
        }
    }
}
