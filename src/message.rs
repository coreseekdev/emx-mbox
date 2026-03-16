use chrono::{DateTime, Utc};
use mailparse::parse_mail;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::builder::Attachment;
use crate::error::MboxError;
use crate::format::normalize_line_endings;

/// An owned RFC 5322 message with cached header access.
#[derive(Clone, Debug)]
pub struct MailMessage {
    /// The complete raw message bytes (headers + body), LF line endings.
    pub raw: Vec<u8>,
    /// The sender from the mbox envelope `From ` line (if loaded from mbox).
    pub envelope_from: Option<String>,
    /// Lazily-populated header cache (lowercased key → original value).
    headers: RefCell<Option<HashMap<String, String>>>,
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
    // Header access (cached)
    // -----------------------------------------------------------------------

    /// Ensure the header cache is populated.
    fn ensure_headers(&self) {
        let mut cache = self.headers.borrow_mut();
        if cache.is_some() {
            return;
        }
        let mut map = HashMap::new();
        if let Ok(parsed) = parse_mail(&self.raw) {
            for h in &parsed.headers {
                let key = h.get_key().to_ascii_lowercase();
                // Only store the first occurrence of each header.
                map.entry(key).or_insert_with(|| h.get_value());
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
            .and_then(|m| m.get(&name.to_ascii_lowercase()).cloned())
    }

    /// Return **all** values for a given header name.
    pub fn headers_all(&self, name: &str) -> Vec<String> {
        // For multi-value we fall back to a fresh parse (cache stores first only).
        let key_lower = name.to_ascii_lowercase();
        parse_mail(&self.raw)
            .map(|p| {
                p.headers
                    .iter()
                    .filter(|h| h.get_key().to_ascii_lowercase() == key_lower)
                    .map(|h| h.get_value())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Invalidate the header cache (e.g. after mutating `raw`).
    pub fn invalidate_cache(&self) {
        *self.headers.borrow_mut() = None;
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
        parse_mail(&self.raw)
            .ok()
            .and_then(|p| p.get_body().ok())
            .unwrap_or_default()
    }

    /// Access to the full parsed mail structure.
    pub fn parsed(&self) -> Result<mailparse::ParsedMail<'_>, MboxError> {
        Ok(parse_mail(&self.raw)?)
    }

    /// Extract attachments from a MIME multipart message.
    /// Returns an empty Vec for plain-text messages.
    pub fn attachments(&self) -> Vec<Attachment> {
        let parsed = match parse_mail(&self.raw) {
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
        write!(f, "{}", String::from_utf8_lossy(&self.raw))
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
