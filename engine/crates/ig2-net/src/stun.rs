//! The least of STUN (RFC 8489) the project needs (ADR 0047 part 4): the rendezvous service answers Binding requests
//! on its UDP port, so browsers and native WebRTC ends learn their public address (a server-reflexive candidate)
//! from the project's own relay instead of a third party's STUN server.

use std::net::{Ipv4Addr, SocketAddrV4};

const COOKIE: [u8; 4] = [0x21, 0x12, 0xa4, 0x42];
const BINDING_REQUEST: [u8; 2] = [0x00, 0x01];
const BINDING_SUCCESS: [u8; 2] = [0x01, 0x01];
const XOR_MAPPED_ADDRESS: [u8; 2] = [0x00, 0x20];

/// A Binding request with transaction id `id`, no attributes.
pub fn request(id: [u8; 12]) -> Vec<u8> {
    let mut b = Vec::with_capacity(20);
    b.extend_from_slice(&BINDING_REQUEST);
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&COOKIE);
    b.extend_from_slice(&id);
    b
}

/// The service's answer to a Binding request `b` from `from`: a success with `from` as XOR-MAPPED-ADDRESS (32
/// bytes, never more than the 20-byte request: no amplification). `None`: not a Binding request.
pub fn answer(b: &[u8], from: SocketAddrV4) -> Option<Vec<u8>> {
    if b.len() < 20 || b[..2] != BINDING_REQUEST || b[4..8] != COOKIE || u16::from_be_bytes([b[2], b[3]]) as usize != b.len() - 20 {
        return None;
    }
    let mut r = Vec::with_capacity(32);
    r.extend_from_slice(&BINDING_SUCCESS);
    r.extend_from_slice(&12u16.to_be_bytes());
    r.extend_from_slice(&b[4..20]);
    r.extend_from_slice(&XOR_MAPPED_ADDRESS);
    r.extend_from_slice(&8u16.to_be_bytes());
    r.extend_from_slice(&[0, 1]);
    r.extend_from_slice(&(from.port() ^ 0x2112).to_be_bytes());
    r.extend(from.ip().octets().iter().zip(COOKIE).map(|(a, c)| a ^ c));
    Some(r)
}

/// The mapped address of a Binding success `b` for transaction `id` (`None`: something else).
pub fn mapped(b: &[u8], id: [u8; 12]) -> Option<SocketAddrV4> {
    if b.len() < 20 || b[..2] != BINDING_SUCCESS || b[4..8] != COOKIE || b[8..20] != id {
        return None;
    }
    let mut attrs = &b[20..];
    while attrs.len() >= 4 {
        let n = u16::from_be_bytes([attrs[2], attrs[3]]) as usize;
        let v = attrs.get(4..4 + n)?;
        if attrs[..2] == XOR_MAPPED_ADDRESS && n == 8 && v[1] == 1 {
            let port = u16::from_be_bytes([v[2], v[3]]) ^ 0x2112;
            let ip: Vec<u8> = v[4..8].iter().zip(COOKIE).map(|(a, c)| a ^ c).collect();
            return Some(SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port));
        }
        // Attributes are padded to four bytes.
        attrs = attrs.get(4 + n.div_ceil(4) * 4..)?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_request_gets_its_source_back() {
        let from: SocketAddrV4 = "198.51.100.7:40123".parse().unwrap();
        let id = [7; 12];
        let r = answer(&request(id), from).unwrap();
        assert_eq!(r.len(), 32);
        assert_eq!(mapped(&r, id), Some(from));
        assert_eq!(mapped(&r, [8; 12]), None, "another transaction");
        // RFC 5769 §2.2's IPv4 response: 192.0.2.1:32853.
        let mut sample = vec![0x01, 0x01, 0x00, 0x0c, 0x21, 0x12, 0xa4, 0x42];
        sample.extend_from_slice(&[0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae]);
        sample.extend_from_slice(&[0x00, 0x20, 0x00, 0x08, 0x00, 0x01, 0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43]);
        let id: [u8; 12] = sample[8..20].try_into().unwrap();
        assert_eq!(mapped(&sample, id), Some("192.0.2.1:32853".parse().unwrap()));
        // Not requests: the service's own packets, a wrong length.
        assert_eq!(answer(b"IG2R\x01\x07", from), None);
        let mut long = request(id);
        long.push(0);
        assert_eq!(answer(&long, from), None);
    }
}
