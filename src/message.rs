use chrono::{DateTime, Utc};
use mailparse::parse_mail;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use crate::attachment::Attachment;
use crate::error::MailError;
use crate::format::normalize_line_endings;

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
    pub fn from_raw(raw: Vec<u8>) -> Self {
        Self {
            raw: normalize_line_endings(&raw),
            envelope_from: None,
            headers: OnceLock::new(),
            body_cache: OnceLock::new(),
        }
    }

    /// Load a single message from an EML file.
    pub fn from_eml_file<P: AsRef<Path>>(path: P) -> Result<Self, MailError> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;
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
        self.raw = normalize_line_endings(&raw);
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

    pub fn date(&self) -> Option<DateTime<Utc>> {
        let val = self.header("Date")?;
        mailparse::dateparse(val)
            .ok()
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now))
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
        let parsed = match parse_mail(self.raw()) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        let mut result = Vec::new();
        collect_attachments(&parsed, &mut result);
        result
    }
}

impl fmt::Display for MailMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(self.raw()))
    }
}

/// Recursively walk MIME parts and collect attachment parts.
fn collect_attachments(part: &mailparse::ParsedMail<'_>, out: &mut Vec<Attachment>) {
    // Check Content-Disposition for "attachment"
    let disposition = part
        .headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Content-Disposition"))
        .map(|h| h.get_value());

    if let Some(ref disp) = disposition {
        if disp.starts_with("attachment") {
            let filename = extract_param(disp, "filename").unwrap_or("attachment");
            let content_type = part
                .ctype
                .mimetype
                .clone();
            // Get the decoded body bytes
            if let Ok(data) = part.get_body_raw() {
                out.push(Attachment::new(filename, content_type, data));
                return;
            }
        }
    }

    // Recurse into sub-parts
    for sub in &part.subparts {
        collect_attachments(sub, out);
    }
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
