use std::borrow::Cow;

/// Which flavour of "From " escaping to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MboxFormat {
    /// mboxo – only bare `From ` lines are escaped with a single `>`.
    Mboxo,
    /// mboxrd – every leading `>*From ` is escaped by prepending one more `>`.
    /// This is what b4 prefers.
    #[default]
    Mboxrd,
}

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

const FROM_PREFIX: &[u8] = b"From ";

/// Check whether a byte-line is an mbox `From ` separator.
#[inline]
pub fn is_from_line(line: &[u8]) -> bool {
    line.starts_with(FROM_PREFIX) && !line.starts_with(b"From:")
}

/// Does `line` match `>+From ` (one or more `>` followed by `From `)?
#[inline]
pub fn is_escaped_from(line: &[u8]) -> bool {
    // Walk past leading `>` bytes, then check for `From `.
    let rest = line.iter().position(|&b| b != b'>');
    match rest {
        Some(pos) if pos > 0 => line[pos..].starts_with(FROM_PREFIX),
        _ => false,
    }
}

/// Strip exactly one leading `>` from an mboxrd-escaped `>+From ` line.
/// For mboxo this is a no-op (mboxo never escapes with `>`).
#[inline]
pub fn unescape_from_line(line: &[u8], format: MboxFormat) -> &[u8] {
    match format {
        MboxFormat::Mboxrd => {
            if line.starts_with(b">") {
                let rest = &line[1..];
                if rest.starts_with(FROM_PREFIX) || is_escaped_from(rest) {
                    return rest;
                }
            }
            line
        }
        MboxFormat::Mboxo => {
            // mboxo: strip a single `>` only if the remainder is bare `From `.
            if line.starts_with(b">") && line[1..].starts_with(FROM_PREFIX) {
                return &line[1..];
            }
            line
        }
    }
}

/// Escape a line for mboxrd: if the line starts with `>*From `, prepend `>`.
pub fn escape_from_line(line: &[u8], format: MboxFormat) -> Option<u8> {
    match format {
        MboxFormat::Mboxrd => {
            if line.starts_with(FROM_PREFIX) || is_escaped_from(line) {
                Some(b'>')
            } else {
                None
            }
        }
        MboxFormat::Mboxo => {
            if line.starts_with(FROM_PREFIX) {
                Some(b'>')
            } else {
                None
            }
        }
    }
}

/// Normalize CRLF to LF in a byte slice.
pub fn normalize_line_endings(data: &[u8]) -> Cow<'_, [u8]> {
    if !data.contains(&b'\r') {
        return Cow::Borrowed(data);
    }
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\r' && data.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// Extract the bare email address from a `From:` header value.
pub fn extract_email_address(from: &str) -> Option<String> {
    // Try `Name <addr>` format
    if let Some(start) = from.find('<') {
        if let Some(end) = from[start..].find('>') {
            return Some(from[start + 1..start + end].to_string());
        }
    }
    // Bare email
    let trimmed = from.trim();
    if trimmed.contains('@') {
        return Some(trimmed.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_from_line() {
        assert!(is_from_line(b"From sender@x Thu Jan 1 00:00:00 2015"));
        assert!(!is_from_line(b"From: sender@x"));
        assert!(!is_from_line(b">From escaped"));
        assert!(!is_from_line(b""));
    }

    #[test]
    fn test_is_escaped_from() {
        assert!(is_escaped_from(b">From something"));
        assert!(is_escaped_from(b">>From something"));
        assert!(is_escaped_from(b">>>From something"));
        assert!(!is_escaped_from(b"From something"));
        assert!(!is_escaped_from(b">Not from"));
        assert!(!is_escaped_from(b""));
    }

    #[test]
    fn test_unescape_from_line() {
        assert_eq!(unescape_from_line(b">From x", MboxFormat::Mboxrd), b"From x");
        assert_eq!(unescape_from_line(b">>From x", MboxFormat::Mboxrd), b">From x");
        assert_eq!(unescape_from_line(b">>>From x", MboxFormat::Mboxrd), b">>From x");
        assert_eq!(unescape_from_line(b"normal line", MboxFormat::Mboxrd), b"normal line");
        assert_eq!(unescape_from_line(b">Not from", MboxFormat::Mboxrd), b">Not from");
    }

    #[test]
    fn test_unescape_from_line_mboxo() {
        // mboxo only unescapes >From (one level)
        assert_eq!(unescape_from_line(b">From x", MboxFormat::Mboxo), b"From x");
        // >>From stays as-is in mboxo (it was never escaped by mboxo)
        assert_eq!(unescape_from_line(b">>From x", MboxFormat::Mboxo), b">>From x");
        assert_eq!(unescape_from_line(b"normal line", MboxFormat::Mboxo), b"normal line");
    }

    #[test]
    fn test_escape_from_line_mboxrd() {
        assert_eq!(escape_from_line(b"From x", MboxFormat::Mboxrd), Some(b'>'));
        assert_eq!(escape_from_line(b">From x", MboxFormat::Mboxrd), Some(b'>'));
        assert_eq!(escape_from_line(b">>From x", MboxFormat::Mboxrd), Some(b'>'));
        assert_eq!(escape_from_line(b"normal", MboxFormat::Mboxrd), None);
    }

    #[test]
    fn test_escape_from_line_mboxo() {
        assert_eq!(escape_from_line(b"From x", MboxFormat::Mboxo), Some(b'>'));
        assert_eq!(escape_from_line(b">From x", MboxFormat::Mboxo), None);
        assert_eq!(escape_from_line(b"normal", MboxFormat::Mboxo), None);
    }

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(normalize_line_endings(b"a\r\nb\r\n").as_ref(), b"a\nb\n");
        assert_eq!(normalize_line_endings(b"a\nb\n").as_ref(), b"a\nb\n");
        assert_eq!(normalize_line_endings(b"no newline").as_ref(), b"no newline");
    }

    #[test]
    fn test_extract_email_address() {
        assert_eq!(
            extract_email_address("Alice <alice@example.com>"),
            Some("alice@example.com".into())
        );
        assert_eq!(
            extract_email_address("bare@example.com"),
            Some("bare@example.com".into())
        );
        assert_eq!(extract_email_address("noaddr"), None);
    }
}
