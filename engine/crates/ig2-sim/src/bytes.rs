//! Little-endian reads of the original's raw records (designs, fleets, planets, the battle's objects) and their
//! NUL-terminated names. Like slice indexing, a read past the record's end panics: the records' sizes are fixed.

/// The `i16` at byte `o`.
#[inline]
pub fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

/// The `u16` at byte `o`.
#[inline]
pub fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// The `i32` at byte `o`.
#[inline]
pub fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// The `u32` at byte `o`.
#[inline]
pub fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// The `f32` at byte `o`.
#[inline]
pub fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// A C string field: the bytes before the first NUL (all of them without one).
#[inline]
pub fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads() {
        let b = [0xfe, 0xff, 0x00, 0x00, 0x80, 0x3f, b'a', 0, b'b'];
        assert_eq!(i16_at(&b, 0), -2);
        assert_eq!(u16_at(&b, 0), 0xfffe);
        assert_eq!(i32_at(&b, 0), 0xfffe);
        assert_eq!(u32_at(&b, 2), 0x3f80_0000);
        assert_eq!(f32_at(&b, 2), 1.0);
        assert_eq!(cstr(&b[6..]), b"a");
        assert_eq!(cstr(b"ab"), b"ab");
    }
}
