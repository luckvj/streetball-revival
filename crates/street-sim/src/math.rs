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
