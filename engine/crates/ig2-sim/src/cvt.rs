//! The SSE/x87 float-to-integer truncations the original's code relies on (`cvttss2si`, `cvttsd2si`, `ftol`).
//!
//! Rust's `as i32` saturates and maps NaN to 0; the exe's instructions give the "integer indefinite" value
//! `i32::MIN` (`0x80000000`) for NaN and every value outside the `i32` range.

/// `cvttss2si`: f32 → i32 by truncation; NaN and out of range → `i32::MIN`.
#[inline]
pub(crate) fn cvtt_f32(x: f32) -> i32 {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) { x as i32 } else { i32::MIN }
}

/// `cvttsd2si` / x87 `ftol`: f64 → i32 by truncation; NaN and out of range → `i32::MIN`.
#[inline]
pub(crate) fn cvtt_f64(x: f64) -> i32 {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) { x as i32 } else { i32::MIN }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indefinite_outside_the_range() {
        assert_eq!(cvtt_f32(f32::NAN), i32::MIN);
        assert_eq!(cvtt_f32(2_147_483_648.0), i32::MIN);
        assert_eq!(cvtt_f32(-2_147_483_648.0), i32::MIN);
        assert_eq!(cvtt_f32(-1.9), -1);
        assert_eq!(cvtt_f64(f64::NAN), i32::MIN);
        assert_eq!(cvtt_f64(2_147_483_647.9), i32::MAX);
        assert_eq!(cvtt_f64(-2_147_483_648.5), i32::MIN);
        assert_eq!(cvtt_f64(-2_147_483_649.0), i32::MIN);
        assert_eq!(cvtt_f64(1e300), i32::MIN);
        assert_eq!(cvtt_f64(7.99), 7);
    }
}
