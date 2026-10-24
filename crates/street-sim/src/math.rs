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
