use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::error::MboxError;
use crate::format::{is_from_line, normalize_line_endings, unescape_from_line, MboxFormat};
use crate::message::MailMessage;

/// Raw result from `next_raw` — envelope info plus message bytes.
pub struct RawMessage {
    /// The full `From ` separator line (without trailing newline).
    /// e.g. `From sender@example.com Thu Jan  1 00:00:00 1970`
    pub envelope: String,
    /// Message bytes (headers + body, without the envelope line).
    pub data: Vec<u8>,
}

/// Streaming mbox reader — yields one message at a time.
pub struct MboxReader<R: Read> {
    reader: BufReader<R>,
    format: MboxFormat,
    started: bool,
    at_eof: bool,
    /// The most recently consumed `From ` line.
    last_envelope: String,
}

impl MboxReader<File> {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = File::open(path)?;
        Ok(Self::new(file))
    }
}

impl<R: Read> MboxReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            format: MboxFormat::default(),
            started: false,
            at_eof: false,
            last_envelope: String::new(),
        }
    }

    pub fn with_format(mut self, format: MboxFormat) -> Self {
        self.format = format;
        self
    }

    /// Return the raw bytes of the next message together with the envelope
    /// `From ` line.  Returns `Ok(None)` at end of archive.
    pub fn next_raw(&mut self) -> Result<Option<RawMessage>, MboxError> {
        if self.at_eof {
            return Ok(None);
        }

        // Phase 1: find the next `From ` separator.
        if !self.started {
            loop {
                let mut line = String::new();
                let n = self.reader.read_line(&mut line)?;
                if n == 0 {
                    self.at_eof = true;
                    return Ok(None);
                }
                let trimmed = line.trim_end_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    continue; // skip leading blank lines
                }
                if is_from_line(trimmed.as_bytes()) {
                    self.last_envelope = trimmed.to_string();
                    self.started = true;
                    break;
                } else {
                    return Err(MboxError::InvalidFormat(
                        "expected mbox From separator".into(),
                    ));
                }
            }
        }

        // Phase 2: collect message lines until the next separator or EOF.
        let mut message = Vec::new();
        let mut prev_blank: Option<Vec<u8>> = None;

        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line)?;
            if n == 0 {
                if let Some(blank) = prev_blank.take() {
                    message.extend_from_slice(&blank);
                }
                self.at_eof = true;
                break;
            }

            let raw_bytes = line.as_bytes();
            let trimmed = line.trim_end_matches(['\r', '\n']);

            if trimmed.is_empty() {
                // Buffer blank line — if the next line is a `From ` separator
                // this is the inter-message gap rather than body content.
                if let Some(blank) = prev_blank.take() {
                    message.extend_from_slice(&blank);
                }
                prev_blank = Some(normalize_line_endings(raw_bytes));
                continue;
            }

            if is_from_line(trimmed.as_bytes()) {
                // Next message starts here. Drop the buffered blank line.
                self.last_envelope = trimmed.to_string();
                break;
            }

            // Regular content — flush any buffered blank line first.
            if let Some(blank) = prev_blank.take() {
                message.extend_from_slice(&blank);
            }

            let unescaped = unescape_from_line(trimmed.as_bytes(), self.format);
            message.extend_from_slice(unescaped);
            message.push(b'\n');
        }

        if message.is_empty() {
            return Ok(None);
        }
        Ok(Some(RawMessage {
            envelope: self.last_envelope.clone(),
            data: message,
        }))
    }

    /// Convenience: parse the next message and return a `MailMessage`.
    pub fn next_message(&mut self) -> Result<Option<MailMessage>, MboxError> {
        match self.next_raw()? {
            Some(raw_msg) => {
                let mut msg = MailMessage::from_raw(raw_msg.data);
                msg.set_envelope_from(parse_envelope_from(&raw_msg.envelope));
                Ok(Some(msg))
            }
            None => Ok(None),
        }
    }
}

/// Iterator adapter so you can write `for msg in reader { ... }`.
impl<R: Read> Iterator for MboxReader<R> {
    type Item = Result<MailMessage, MboxError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_message() {
            Ok(Some(msg)) => Some(Ok(msg)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// Parse the sender address from a `From ` envelope line.
/// Input: `From sender@example.com Thu Jan  1 00:00:00 1970`
/// Output: `Some("sender@example.com")`
fn parse_envelope_from(envelope: &str) -> Option<String> {
    let rest = envelope.strip_prefix("From ")?;
    let sender = rest.split_whitespace().next()?;
    if sender.is_empty() {
        None
    } else {
        Some(sender.to_string())
    }
}
