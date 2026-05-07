//! Output encoding helpers for `klef_run`. Splits stdout/stderr into either
//! UTF-8 strings (preferred) or base64 (fallback for binary). Kept in a
//! sibling file via `#[path]` to keep `tools.rs` under the 300-line cap.

pub(super) fn encode_outputs(out: &[u8], err: &[u8]) -> (String, String, &'static str) {
    match (std::str::from_utf8(out), std::str::from_utf8(err)) {
        (Ok(o), Ok(e)) => (o.to_string(), e.to_string(), "utf8"),
        _ => (base64_encode(out), base64_encode(err), "base64"),
    }
}

fn base64_encode(b: &[u8]) -> String {
    use std::fmt::Write;
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= b.len() {
        let n = u32::from(b[i]) << 16 | u32::from(b[i + 1]) << 8 | u32::from(b[i + 2]);
        for shift in [18, 12, 6, 0] {
            out.push(A[((n >> shift) & 0x3F) as usize] as char);
        }
        i += 3;
    }
    if i < b.len() {
        let mut n: u32 = 0;
        for j in 0..3 {
            n <<= 8;
            if i + j < b.len() {
                n |= u32::from(b[i + j]);
            }
        }
        for shift in [18, 12, 6, 0] {
            let _ = write!(out, "{}", A[((n >> shift) & 0x3F) as usize] as char);
        }
        let pad = 3 - (b.len() - i);
        out.replace_range(out.len() - pad.., &"=".repeat(pad));
    }
    out
}
