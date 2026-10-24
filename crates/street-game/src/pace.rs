//! How long the loading screens and the save's waits stand.
//!
//! The original's holds were measured to the hundredth (the boot's loading
//! screen 13.62 s, the load back to the front end 3.27 s, the memory card's
//! checks and writes 2 to 4.4 s, its result messages 2 s). Vj asked on
//! 2026-10-21 for the loading screens to be much faster, so by default each
//! stands only a moment, long enough for its picture to be seen, and the
//! save's waits as long as the PC needs. `--original-load-times` brings the
//! measured holds back; the parity tests run with them.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the original's measured holds are in use (tests use them, so
/// the parity checks keep comparing the original's times).
static ORIGINAL: AtomicBool = AtomicBool::new(cfg!(test));

/// A loading screen's picture: the boot's, the load into a game, the load
/// back to the menus.
pub const PICTURE: f64 = 0.6;
/// The save's busy waits (checking, saving, loading).
pub const WAIT: f64 = 0.4;
/// A result the player reads ("Save successful.").
pub const NOTICE: f64 = 1.0;
/// The boot's loading screen, its bar filling across it.
pub const BOOT: f64 = 1.0;
/// From Start in the idle demo to the load back.
pub const DEMO_EXIT: f64 = 0.3;
/// The least a load into a game shows its picture before loading.
pub const LOAD_FLOOR: f64 = 0.3;

/// Use the original's measured holds (`--original-load-times`).
pub fn set_original(on: bool) {
    ORIGINAL.store(on, Ordering::Relaxed);
}

/// Whether the original's measured holds are in use.
pub fn original() -> bool {
    ORIGINAL.load(Ordering::Relaxed)
}

/// The hold to use: the original's `measured` one, or the fast one.
pub fn hold(measured: f64, fast: f64) -> f64 {
    pick(original(), measured, fast)
}

fn pick(original: bool, measured: f64, fast: f64) -> f64 {
    if original { measured } else { fast }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fast_holds_are_short_and_the_original_ones_stay() {
        assert_eq!(pick(false, 13.62, BOOT), 1.0);
        assert_eq!(pick(true, 13.62, BOOT), 13.62);
        const { assert!(PICTURE < 1.0 && WAIT < 1.0 && NOTICE <= 1.0) };
        // Tests run on the original's holds.
        assert!(original());
    }
}
