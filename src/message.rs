use chrono::{DateTime, Utc};
use mailparse::parse_mail;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::attachment::Attachment;
use crate::error::MboxError;
use crate::format::normalize_line_endings;

/// An owned RFC 5322 message with cached header access.
#[derive(Clone, Debug)]
pub struct MailMessage {
    /// The complete raw message bytes (headers + body), LF line endings.
    raw: Vec<u8>,
    /// The sender from the mbox envelope `From ` line (if loaded from mbox).
    envelope_from: Option<String>,
    /// Lazily-populated header cache (lowercased key → all values).
    headers: RefCell<Option<HashMap<String, Vec<String>>>>,
}

impl MailMessage {
    /// Build a `MailMessage` from raw RFC 5322 bytes.
    pub fn from_raw(raw: Vec<u8>) -> Self {
        Self {
            raw: normalize_line_endings(&raw),
            envelope_from: None,
            headers: RefCell::new(None),
        }
    }

    /// Load a single message from an EML file.
    pub fn from_eml_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
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
        *self.headers.borrow_mut() = None;
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

    /// Ensure the header cache is populated.
    fn ensure_headers(&self) {
        let mut cache = self.headers.borrow_mut();
        if cache.is_some() {
            return;
        }
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(parsed) = parse_mail(&self.raw) {
            for h in &parsed.headers {
                let key = h.get_key().to_ascii_lowercase();
                map.entry(key).or_default().push(h.get_value());
            }
        }
        *cache = Some(map);
    }

    /// Return the value of a header (first occurrence), or `None`.
    pub fn header(&self, name: &str) -> Option<String> {
        self.ensure_headers();
        self.headers
            .borrow()
            .as_ref()
            .and_then(|m| m.get(&name.to_ascii_lowercase()))
            .and_then(|v| v.first().cloned())
    }

    /// Return **all** values for a given header name (from cache, no re-parse).
    pub fn headers_all(&self, name: &str) -> Vec<String> {
        self.ensure_headers();
        self.headers
            .borrow()
            .as_ref()
            .and_then(|m| m.get(&name.to_ascii_lowercase()).cloned())
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Shorthand accessors
    // -----------------------------------------------------------------------

    pub fn from(&self) -> String {
        self.header("From").unwrap_or_default()
    }

    pub fn subject(&self) -> String {
        self.header("Subject").unwrap_or_default()
    }

    pub fn message_id(&self) -> Option<String> {
        self.header("Message-ID")
    }

    pub fn date(&self) -> Option<DateTime<Utc>> {
        let val = self.header("Date")?;
        mailparse::dateparse(&val)
            .ok()
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now))
    }

    /// Return the decoded body text (first `text/plain` part).
    pub fn body(&self) -> String {
        parse_mail(self.raw())
            .ok()
            .and_then(|p| p.get_body().ok())
            .unwrap_or_default()
    }

    /// Access to the full parsed mail structure.
    pub fn parsed(&self) -> Result<mailparse::ParsedMail<'_>, MboxError> {
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
            let filename = extract_param(disp, "filename")
                .unwrap_or_else(|| "attachment".into());
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
fn extract_param(header_value: &str, param_name: &str) -> Option<String> {
    let needle = format!("{}=", param_name);
    let pos = header_value.find(&needle)?;
    let rest = &header_value[pos + needle.len()..];
    if rest.starts_with('"') {
        // Quoted value
        let end = rest[1..].find('"')?;
        Some(rest[1..1 + end].to_string())
    } else {
        // Unquoted — take until `;` or end
        let end = rest.find(';').unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}
