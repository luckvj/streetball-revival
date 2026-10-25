//! The EE's single-precision arithmetic, as PCSX2 runs it: ordinary FPU
//! multiplies and adds chop (round toward zero), the add keeping one guard
//! bit of the smaller operand; division and square root round to nearest
//! (the host's). Verified against PCSX2 2.8.2 captures, not a complete PS2
//! float emulator.

/// `value` rounded toward zero to single precision.
pub fn chop(value: f64) -> f32 {
    let rounded = value as f32;
    if rounded.is_finite() && (rounded.abs() as f64) > value.abs() {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    }
}

/// `mul.s`.
pub fn mul(a: f32, b: f32) -> f32 {
    chop(a as f64 * b as f64)
}

/// `add.s`: PCSX2's add path discards the smaller operand's alignment bits
/// but one guard bit, then chops. Its twin is `street_sim::math::ee_add`.
pub fn add(mut a: f32, mut b: f32) -> f32 {
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
    chop(a as f64 + b as f64)
}

/// `sub.s`: the add of the negation.
pub fn sub(a: f32, b: f32) -> f32 {
    add(a, -b)
}
