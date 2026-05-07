//! Best-effort substitution of resolved `env_ref` values in captured output.
//!
//! Operates on raw bytes of stdout/stderr (binary-safe). Values <= 4 bytes
//! are skipped (false-positive risk too high — a `PORT=3000` value would
//! match every occurrence of `3000`).

const MIN_VALUE_LEN: usize = 5;

/// Replace every byte-occurrence of each resolved value in `buf` with
/// `[REDACTED:<name>]`. Mutates `buf` in place. Skips values <= 4 bytes.
pub fn redact(buf: &mut Vec<u8>, resolved: &[(String, String)]) {
    for (name, value) in resolved {
        if value.len() < MIN_VALUE_LEN {
            continue;
        }
        let needle = value.as_bytes();
        let placeholder = format!("[REDACTED:{name}]").into_bytes();
        replace_all(buf, needle, &placeholder);
    }
}

fn replace_all(haystack: &mut Vec<u8>, needle: &[u8], replacement: &[u8]) {
    if needle.is_empty() || needle.len() > haystack.len() {
        return;
    }
    let mut out: Vec<u8> = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            out.extend_from_slice(replacement);
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&haystack[i..]);
    *haystack = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_simple_occurrence() {
        let mut buf = b"key is sk_live_abcdef and stop".to_vec();
        redact(&mut buf, &[("stripe".into(), "sk_live_abcdef".into())]);
        assert_eq!(buf, b"key is [REDACTED:stripe] and stop");
    }

    #[test]
    fn redact_multi_occurrence() {
        let mut buf = b"AA-XYZ12-BB-XYZ12-CC".to_vec();
        redact(&mut buf, &[("k".into(), "XYZ12".into())]);
        assert_eq!(buf, b"AA-[REDACTED:k]-BB-[REDACTED:k]-CC");
    }

    #[test]
    fn redact_skips_values_below_min_len() {
        let mut buf = b"port is 3000 and 3000 again".to_vec();
        redact(&mut buf, &[("port".into(), "3000".into())]);
        assert_eq!(
            buf, b"port is 3000 and 3000 again",
            "<5-byte values must be skipped"
        );
    }

    #[test]
    fn redact_binary_safe() {
        let mut buf: Vec<u8> = vec![0x00, 0xFF, b's', b'k', b'_', b'a', b'b', b'c', 0x00];
        redact(&mut buf, &[("k".into(), "sk_abc".into())]);
        assert_eq!(
            buf,
            [
                0x00, 0xFF, b'[', b'R', b'E', b'D', b'A', b'C', b'T', b'E', b'D', b':', b'k', b']',
                0x00
            ]
        );
    }

    #[test]
    fn redact_no_op_when_value_absent() {
        let mut buf = b"nothing to see here".to_vec();
        redact(&mut buf, &[("k".into(), "missing".into())]);
        assert_eq!(buf, b"nothing to see here");
    }
}
