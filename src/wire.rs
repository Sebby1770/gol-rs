//! `wire` — the from-scratch transport primitives behind `gol serve`.
//!
//! A WebSocket handshake needs SHA-1 + base64, and a frame needs the RFC 6455
//! bit-twiddling. Rather than pull in a crate for 120 lines of well-specified
//! code, we implement them here so the whole project stays dependency-free and
//! builds offline. Everything here is covered by the RFC's own test vectors.

// ---------------------------------------------------------------------------
// base64 (standard alphabet, with padding)
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// SHA-1 (FIPS 180-1) — used only for the WebSocket accept key
// ---------------------------------------------------------------------------

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];

    let ml = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());

    for block in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Compute the `Sec-WebSocket-Accept` value for a client key (RFC 6455 §4.2.2).
pub fn ws_accept_key(client_key: &str) -> String {
    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined = format!("{client_key}{MAGIC}");
    base64_encode(&sha1(combined.as_bytes()))
}

// ---------------------------------------------------------------------------
// WebSocket framing (RFC 6455 §5)
// ---------------------------------------------------------------------------

/// Build a server→client text frame (FIN=1, opcode=0x1, unmasked).
pub fn ws_text_frame(payload: &[u8]) -> Vec<u8> {
    ws_frame(0x1, payload)
}

/// Build a server→client close frame.
pub fn ws_close_frame() -> Vec<u8> {
    ws_frame(0x8, &[])
}

fn ws_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x80 | opcode); // FIN + opcode
    let len = payload.len();
    if len < 126 {
        frame.push(len as u8);
    } else if len <= u16::MAX as usize {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    frame
}

/// A decoded inbound WebSocket frame (client→server frames are always masked).
pub struct InboundFrame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// Parse a single client frame from `buf`, returning the frame and the number
/// of bytes consumed, or `None` if `buf` does not yet hold a full frame.
pub fn parse_client_frame(buf: &[u8]) -> Option<(InboundFrame, usize)> {
    if buf.len() < 2 {
        return None;
    }
    let opcode = buf[0] & 0x0F;
    let masked = buf[1] & 0x80 != 0;
    let mut len = (buf[1] & 0x7F) as usize;
    let mut cursor = 2;

    if len == 126 {
        if buf.len() < cursor + 2 {
            return None;
        }
        len = u16::from_be_bytes([buf[cursor], buf[cursor + 1]]) as usize;
        cursor += 2;
    } else if len == 127 {
        if buf.len() < cursor + 8 {
            return None;
        }
        let mut l = [0u8; 8];
        l.copy_from_slice(&buf[cursor..cursor + 8]);
        len = u64::from_be_bytes(l) as usize;
        cursor += 8;
    }

    let mask = if masked {
        if buf.len() < cursor + 4 {
            return None;
        }
        let m = [
            buf[cursor],
            buf[cursor + 1],
            buf[cursor + 2],
            buf[cursor + 3],
        ];
        cursor += 4;
        Some(m)
    } else {
        None
    };

    if buf.len() < cursor + len {
        return None;
    }

    let mut payload = buf[cursor..cursor + len].to_vec();
    if let Some(m) = mask {
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= m[i % 4];
        }
    }
    cursor += len;

    Some((InboundFrame { opcode, payload }, cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn sha1_known_vectors() {
        // FIPS 180-1 sample messages.
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn websocket_accept_key_rfc_vector() {
        // The canonical example from RFC 6455 §1.3.
        assert_eq!(
            ws_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn text_frame_small_payload_header() {
        let f = ws_text_frame(b"hi");
        assert_eq!(f[0], 0x81); // FIN + text
        assert_eq!(f[1], 2); // unmasked length 2
        assert_eq!(&f[2..], b"hi");
    }

    #[test]
    fn text_frame_extended_length() {
        let payload = vec![0u8; 200];
        let f = ws_text_frame(&payload);
        assert_eq!(f[1], 126);
        assert_eq!(u16::from_be_bytes([f[2], f[3]]), 200);
    }

    #[test]
    fn parse_masked_client_frame() {
        // A masked "Hi" text frame the way a browser would send it.
        let mask = [0x37, 0xfa, 0x21, 0x3d];
        let data = b"Hi";
        let mut frame = vec![0x81, 0x80 | data.len() as u8];
        frame.extend_from_slice(&mask);
        for (i, b) in data.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        let (parsed, used) = parse_client_frame(&frame).unwrap();
        assert_eq!(used, frame.len());
        assert_eq!(parsed.opcode, 0x1);
        assert_eq!(parsed.payload, b"Hi");
    }

    #[test]
    fn parse_returns_none_on_partial() {
        assert!(parse_client_frame(&[0x81]).is_none());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
