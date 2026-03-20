use chrono::{DateTime, Utc};
use mailparse::parse_mail;
use mailparse::ParsedMail;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::attachment::Attachment;
use crate::error::MailError;
use crate::format::normalize_line_endings;

/// Maximum allowed size of a single message body (excluding attachments).
///
/// Constraint: body size must be strictly less than 150 KiB.
pub const MAX_MESSAGE_BODY_BYTES: usize = 150 * 1024;

/// An owned RFC 5322 message with cached header access.
///
/// Header lookups and body text are cached on first access via `OnceLock`,
/// making `MailMessage` both `Send` and `Sync`.
#[derive(Debug)]
pub struct MailMessage {
    /// The complete raw message bytes (headers + body), LF line endings.
    raw: Vec<u8>,
    /// The sender from the mbox envelope `From ` line (if loaded from mbox).
    envelope_from: Option<String>,
    /// Lazily-populated header cache (lowercased key → all values).
    headers: OnceLock<HashMap<String, Vec<String>>>,
    /// Cached decoded body text.
    body_cache: OnceLock<String>,
}

impl Clone for MailMessage {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            envelope_from: self.envelope_from.clone(),
            headers: OnceLock::new(),
            body_cache: OnceLock::new(),
        }
    }
}

impl MailMessage {
    /// Build a `MailMessage` from raw RFC 5322 bytes.
    /// Automatically generates a Message-ID if missing.
    pub fn from_raw(raw: Vec<u8>) -> Self {
        let normalized = normalize_line_endings(&raw);

        // Check if Message-ID exists, generate if missing
        let has_message_id = parse_mail(&normalized)
            .ok()
            .and_then(|parsed| {
                parsed
                    .headers
                    .iter()
                    .find(|h| h.get_key().eq_ignore_ascii_case("message-id"))
                    .map(|_| true)
            })
            .unwrap_or(false);

        let final_raw = if has_message_id {
            normalized.into_owned()
        } else {
            // Insert Message-ID header
            insert_message_id(normalized.as_ref())
        };

        Self {
            raw: final_raw,
            envelope_from: None,
            headers: OnceLock::new(),
            body_cache: OnceLock::new(),
        }
    }

    /// Load a single message from an EML file.
    pub fn from_eml_file<P: AsRef<Path>>(path: P) -> Result<Self, MailError> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;
        ensure_body_size_limit(&data)?;
        Ok(Self::from_raw(data))
    }

    // -----------------------------------------------------------------------
    // Raw / envelope access
    // -----------------------------------------------------------------------

    /// The complete raw message bytes.
    #[inline]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Replace the raw bytes and auto-invalidate the header cache.
    pub fn set_raw(&mut self, raw: Vec<u8>) {
        self.raw = normalize_line_endings(&raw).into_owned();
        self.headers = OnceLock::new();
        self.body_cache = OnceLock::new();
    }

    /// The sender from the mbox envelope `From ` line, if present.
    #[inline]
    pub fn envelope_from(&self) -> Option<&str> {
        self.envelope_from.as_deref()
    }

    /// Set the envelope sender.
    pub fn set_envelope_from(&mut self, envelope: Option<String>) {
        self.envelope_from = envelope;
    }

    // -----------------------------------------------------------------------
    // Header access (cached)
    // -----------------------------------------------------------------------

    /// Return a reference to the lazily-populated header map.
    fn ensure_headers(&self) -> &HashMap<String, Vec<String>> {
        let raw = &self.raw;
        self.headers.get_or_init(|| Self::parse_headers(raw))
    }

    fn parse_headers(raw: &[u8]) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(parsed) = parse_mail(raw) {
            for h in &parsed.headers {
                let key = h.get_key().to_ascii_lowercase();
                map.entry(key).or_default().push(h.get_value());
            }
        }
        map
    }

    /// Return the first value of a header, or `None`.
    /// The returned `&str` borrows from the internal cache — zero copy.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.ensure_headers()
            .get(&name.to_ascii_lowercase())
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }

    /// Return **all** values for a given header name (zero-copy from cache).
    pub fn headers_all(&self, name: &str) -> &[String] {
        self.ensure_headers()
            .get(&name.to_ascii_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    // -----------------------------------------------------------------------
    // Shorthand accessors
    // -----------------------------------------------------------------------

    pub fn from(&self) -> &str {
        self.header("From").unwrap_or_default()
    }

    pub fn subject(&self) -> &str {
        self.header("Subject").unwrap_or_default()
    }

    pub fn message_id(&self) -> Option<&str> {
        self.header("Message-ID")
    }

    /// Get the Supplements-Message-ID header if this is a supplement message.
    pub fn supplements_message_id(&self) -> Option<&str> {
        self.header("Supplements-Message-ID")
    }

    pub fn date(&self) -> Option<DateTime<Utc>> {
        let val = self.header("Date")?;
        mailparse::dateparse(val)
            .ok()
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
    }

    /// Return the decoded body text (first `text/plain` part).
    /// Cached on first access — subsequent calls are zero-cost.
    pub fn body(&self) -> &str {
        let raw = &self.raw;
        self.body_cache.get_or_init(|| {
            parse_mail(raw)
                .ok()
                .and_then(|p| p.get_body().ok())
                .unwrap_or_default()
        })
    }

    /// Access to the full parsed mail structure.
    pub fn parsed(&self) -> Result<mailparse::ParsedMail<'_>, MailError> {
        Ok(parse_mail(self.raw())?)
    }

    /// Extract attachments from a MIME multipart message.
    /// Returns an empty Vec for plain-text messages.
    pub fn attachments(&self) -> Vec<Attachment> {
        self.attachments_result().unwrap_or_default()
    }

    /// Extract attachments from a MIME multipart message.
    ///
    /// Unlike `attachments()`, this returns parse/decode failures.
    pub fn attachments_result(&self) -> Result<Vec<Attachment>, MailError> {
        let parsed = parse_mail(self.raw())?;
        let mut result = Vec::new();
        collect_attachments(&parsed, &mut result)?;
        Ok(result)
    }
}

impl fmt::Display for MailMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.raw()))
    }
}

/// Recursively walk MIME parts and collect attachment parts.
fn collect_attachments(
    part: &mailparse::ParsedMail<'_>,
    out: &mut Vec<Attachment>,
) -> Result<(), MailError> {
    // Check Content-Disposition for "attachment"
    let disposition = part
        .headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Content-Disposition"))
        .map(|h| h.get_value());

    if let Some(ref disp) = disposition {
        if disp.starts_with("attachment") {
            let filename = extract_param(disp, "filename")
                .map(|s| s.to_string())
                .or_else(|| extract_rfc2231_param(disp, "filename"))
                .unwrap_or_else(|| "attachment".to_string());
            let content_type = part
                .ctype
                .mimetype
                .clone();
            // Get the decoded body bytes
            let data = part.get_body_raw()?;
            out.push(Attachment::new(filename, content_type, data));
            return Ok(());
        }
    }

    // Recurse into sub-parts
    for sub in &part.subparts {
        collect_attachments(sub, out)?;
    }

    Ok(())
}

fn is_attachment_part(part: &ParsedMail<'_>) -> bool {
    part.headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Content-Disposition"))
        .map(|h| h.get_value())
        .and_then(|value| {
            value
                .split(';')
                .next()
                .map(|token| token.trim().eq_ignore_ascii_case("attachment"))
        })
        .unwrap_or(false)
}

fn non_attachment_body_size(part: &ParsedMail<'_>) -> usize {
    if is_attachment_part(part) {
        return 0;
    }

    if part.subparts.is_empty() {
        return part.get_body_raw().map(|bytes| bytes.len()).unwrap_or(0);
    }

    part.subparts.iter().map(non_attachment_body_size).sum()
}

pub(crate) fn ensure_body_size_limit(raw: &[u8]) -> Result<(), MailError> {
    let parsed = parse_mail(raw)?;
    let body_size = non_attachment_body_size(&parsed);

    if body_size >= MAX_MESSAGE_BODY_BYTES {
        return Err(MailError::InvalidFormat(format!(
            "message body too large: {} bytes (limit < {} bytes)",
            body_size, MAX_MESSAGE_BODY_BYTES
        )));
    }

    Ok(())
}

/// Extract a named parameter from a header value like
/// `attachment; filename="foo.png"`.
///
/// Splits on `;` to avoid partial matches (e.g. `name` vs `filename`).
fn extract_param<'a>(header_value: &'a str, param_name: &str) -> Option<&'a str> {
    for part in header_value.split(';').skip(1) {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(param_name) {
            let rest = rest.trim_start();
            if let Some(val) = rest.strip_prefix('=') {
                let val = val.trim_start();
                if let Some(inner) = val.strip_prefix('"') {
                    let end = inner.find('"')?;
                    return Some(&inner[..end]);
                } else {
                    return Some(val);
                }
            }
        }
    }
    None
}

fn extract_rfc2231_param(header_value: &str, param_name: &str) -> Option<String> {
    let target = format!("{}*", param_name);
    for part in header_value.split(';').skip(1) {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(&target) {
            let rest = rest.trim_start();
            let val = rest.strip_prefix('=')?.trim().trim_matches('"');
            return decode_rfc2231_value(val);
        }
    }
    None
}

fn decode_rfc2231_value(value: &str) -> Option<String> {
    if let Some(first_quote) = value.find('\'') {
        let remaining = &value[first_quote + 1..];
        if let Some(second_quote_rel) = remaining.find('\'') {
            let charset = &value[..first_quote];
            let encoded = &remaining[second_quote_rel + 1..];
            let decoded = percent_decode(encoded)?;
            if charset.eq_ignore_ascii_case("utf-8") {
                return String::from_utf8(decoded).ok();
            }
            return Some(String::from_utf8_lossy(&decoded).into_owned());
        }
    }

    let decoded = percent_decode(value)?;
    Some(String::from_utf8_lossy(&decoded).into_owned())
}

fn percent_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            let value = u8::from_str_radix(hex, 16).ok()?;
            out.push(value);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }

    Some(out)
}

/// Insert a Message-ID header into a raw message if missing.
/// Returns a new Vec<u8> with the Message-ID inserted.
/// The generated Message-ID will be persisted when the message is written.
fn insert_message_id(raw: &[u8]) -> Vec<u8> {
    let uuid = Uuid::new_v4();
    let message_id = format!("Message-ID: <{}.emx@localhost>", uuid);

    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        let insert_pos = pos + 2;
        let mut result = Vec::with_capacity(raw.len() + message_id.len() + 2);
        result.extend_from_slice(&raw[..insert_pos]);
        result.extend_from_slice(message_id.as_bytes());
        result.extend_from_slice(b"\r\n");
        result.extend_from_slice(&raw[insert_pos..]);
        return result;
    }

    if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        let insert_pos = pos + 1;
        let mut result = Vec::with_capacity(raw.len() + message_id.len() + 1);
        result.extend_from_slice(&raw[..insert_pos]);
        result.extend_from_slice(message_id.as_bytes());
        result.extend_from_slice(b"\n");
        result.extend_from_slice(&raw[insert_pos..]);
        return result;
    }

    let mut result = Vec::with_capacity(raw.len() + message_id.len() + 2);
    result.extend_from_slice(raw);
    if !raw.ends_with(b"\n") {
        result.push(b'\n');
    }
    result.extend_from_slice(message_id.as_bytes());
    result.extend_from_slice(b"\n\n");
    result
}
