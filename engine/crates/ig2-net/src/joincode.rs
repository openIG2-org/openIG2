//! Join codes (ADR 0005 §4, ADR 0008 §3.1): one pasteable code that carries what a joiner needs to reach and trust a
//! host without a LAN announcement or a directory: the session id (a private room's admission), the leading bytes of
//! the session certificate's fingerprint (`quic::Pin::Prefix`) and the host's IPv4 addresses (its LAN address, the
//! router's mapped one). Crockford base 32 in groups of five, a check byte against typos.
//!
//! ```text
//! byte 0         version << 4 | address count
//! 1..9           session id (LE)
//! 9..21          fingerprint prefix
//! 21..21+6n      address n: IPv4 octets, port (BE)
//! last           SHA-256(all before)[0]
//! ```

use std::net::{Ipv4Addr, SocketAddrV4};

/// The code's layout version.
pub const VERSION: u8 = 1;
/// The fingerprint bytes carried (96 bits: another certificate with the same prefix is out of reach).
pub const PIN_BYTES: usize = 12;
/// The addresses a code carries at most.
pub const MAX_ADDRS: usize = 4;
/// Crockford's alphabet (no I, L, O, U).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Characters between the dashes.
const GROUP: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinCode {
    pub session: u64,
    pub pin: [u8; PIN_BYTES],
    pub addrs: Vec<SocketAddrV4>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a join code")]
    Malformed,
    #[error("a join code from a newer game version")]
    Version,
    #[error("the join code is mistyped")]
    Check,
}

impl JoinCode {
    /// The code of session `session` whose certificate's fingerprint is `fingerprint`, reachable at `addrs` (the
    /// first [`MAX_ADDRS`] different ones).
    pub fn new(session: u64, fingerprint: &[u8; 32], addrs: &[SocketAddrV4]) -> Self {
        let mut pin = [0; PIN_BYTES];
        pin.copy_from_slice(&fingerprint[..PIN_BYTES]);
        let mut v: Vec<SocketAddrV4> = Vec::new();
        for a in addrs {
            if !v.contains(a) && v.len() < MAX_ADDRS {
                v.push(*a);
            }
        }
        Self { session, pin, addrs: v }
    }

    /// Whether the certificate whose fingerprint is `fingerprint` is the host's.
    pub fn pins(&self, fingerprint: &[u8; 32]) -> bool {
        fingerprint.starts_with(&self.pin)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![VERSION << 4 | self.addrs.len() as u8];
        b.extend_from_slice(&self.session.to_le_bytes());
        b.extend_from_slice(&self.pin);
        for a in &self.addrs {
            b.extend_from_slice(&a.ip().octets());
            b.extend_from_slice(&a.port().to_be_bytes());
        }
        b.push(check(&b));
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        let (&head, _) = b.split_first().ok_or(Error::Malformed)?;
        if head >> 4 != VERSION {
            return Err(if head >> 4 > VERSION { Error::Version } else { Error::Malformed });
        }
        let n = (head & 15) as usize;
        if n > MAX_ADDRS || b.len() != 1 + 8 + PIN_BYTES + 6 * n + 1 {
            return Err(Error::Malformed);
        }
        let (body, last) = b.split_at(b.len() - 1);
        if check(body) != last[0] {
            return Err(Error::Check);
        }
        let session = u64::from_le_bytes(body[1..9].try_into().unwrap());
        let pin = body[9..9 + PIN_BYTES].try_into().unwrap();
        let addrs = body[9 + PIN_BYTES..]
            .chunks_exact(6)
            .map(|a| SocketAddrV4::new(Ipv4Addr::new(a[0], a[1], a[2], a[3]), u16::from_be_bytes([a[4], a[5]])))
            .collect();
        Ok(Self { session, pin, addrs })
    }

    /// The code as players share it: base 32, groups of five joined by dashes.
    pub fn encode(&self) -> String {
        let chars = base32(&self.to_bytes());
        chars.chunks(GROUP).map(|g| String::from_utf8_lossy(g).into_owned()).collect::<Vec<_>>().join("-")
    }

    /// A typed or pasted code: case, dashes and blanks don't matter; O reads as 0, I and L as 1.
    pub fn decode(s: &str) -> Result<Self, Error> {
        let mut bits = 0u32;
        let mut n = 0;
        let mut out = Vec::new();
        for c in s.bytes() {
            let c = match c.to_ascii_uppercase() {
                b'-' | b' ' | b'\t' | b'\r' | b'\n' => continue,
                b'O' => b'0',
                b'I' | b'L' => b'1',
                c => c,
            };
            let v = ALPHABET.iter().position(|&a| a == c).ok_or(Error::Malformed)? as u32;
            bits = bits << 5 | v;
            n += 5;
            if n >= 8 {
                n -= 8;
                out.push((bits >> n) as u8);
                bits &= (1 << n) - 1;
            }
        }
        if bits != 0 {
            return Err(Error::Malformed);
        }
        Self::from_bytes(&out)
    }
}

fn check(b: &[u8]) -> u8 {
    <sha2::Sha256 as sha2::Digest>::digest(b)[0]
}

fn base32(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut bits, mut n) = (0u32, 0);
    for &x in b {
        bits = bits << 8 | x as u32;
        n += 8;
        while n >= 5 {
            n -= 5;
            out.push(ALPHABET[(bits >> n) as usize & 31]);
        }
        bits &= (1 << n) - 1;
    }
    if n > 0 {
        out.push(ALPHABET[(bits << (5 - n)) as usize & 31]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code() -> JoinCode {
        let fp: [u8; 32] = std::array::from_fn(|i| i as u8 * 7 + 3);
        JoinCode::new(0x1234_5678_9abc_def0, &fp, &["192.168.1.20:5011".parse().unwrap(), "203.0.113.7:5011".parse().unwrap(), "192.168.1.20:5011".parse().unwrap()])
    }

    #[test]
    fn codes_round_trip_through_their_text() {
        let c = code();
        assert_eq!(c.addrs.len(), 2, "the same address once");
        let s = c.encode();
        assert_eq!(s.len(), 55 + 10, "34 bytes: 55 characters in 11 groups");
        assert!(s.bytes().all(|b| b == b'-' || ALPHABET.contains(&b)));
        assert_eq!(JoinCode::decode(&s), Ok(c.clone()));
        // Pasted in lower case, without dashes, with O/I/L for 0/1, surrounded by blanks.
        let loose = format!("  {}\n", s.replace('-', "").to_ascii_lowercase().replace('0', "o").replace('1', "l"));
        assert_eq!(JoinCode::decode(&loose), Ok(c.clone()));
        let fp: [u8; 32] = std::array::from_fn(|i| i as u8 * 7 + 3);
        assert!(c.pins(&fp));
        let mut other = fp;
        other[11] ^= 1;
        assert!(!c.pins(&other));
        // One address: 28 bytes.
        let one = JoinCode::new(7, &fp, &["10.0.0.2:5011".parse().unwrap()]);
        assert_eq!(JoinCode::decode(&one.encode()), Ok(one));
    }

    #[test]
    fn typos_and_addresses_are_not_codes() {
        let s = code().encode();
        // One character changed: the check byte (or the length) catches it.
        let mut b = s.clone().into_bytes();
        b[7] = if b[7] == b'A' { b'B' } else { b'A' };
        assert!(JoinCode::decode(std::str::from_utf8(&b).unwrap()).is_err());
        assert!(JoinCode::decode(&s[..s.len() - 6]).is_err(), "cut short");
        for addr in ["192.168.1.20:5011", "192.168.1.20", "localhost", "my-pc", ""] {
            assert!(JoinCode::decode(addr).is_err(), "{addr}");
        }
        // A newer layout says so.
        let mut bytes = code().to_bytes();
        bytes[0] = 2 << 4 | 2;
        let n = bytes.len();
        bytes[n - 1] = check(&bytes[..n - 1]);
        assert_eq!(JoinCode::from_bytes(&bytes), Err(Error::Version));
    }
}
