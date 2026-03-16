use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::error::MboxError;
use crate::format::{is_from_line, normalize_line_endings, unescape_from_line};
use crate::message::MailMessage;

/// Streaming mbox reader — yields one message at a time.
pub struct MboxReader<R: Read> {
    reader: BufReader<R>,
    started: bool,
    at_eof: bool,
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
            started: false,
            at_eof: false,
        }
    }

    /// Return the raw bytes of the next message (headers + body, without the
    /// envelope `From ` line).  Returns `Ok(None)` at end of archive.
    pub fn next_raw(&mut self) -> Result<Option<Vec<u8>>, MboxError> {
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
                break;
            }

            // Regular content — flush any buffered blank line first.
            if let Some(blank) = prev_blank.take() {
                message.extend_from_slice(&blank);
            }

            let unescaped = unescape_from_line(trimmed.as_bytes());
            message.extend_from_slice(unescaped);
            message.push(b'\n');
        }

        if message.is_empty() {
            return Ok(None);
        }
        Ok(Some(message))
    }

    /// Convenience: parse the next message and return a `MailMessage`.
    pub fn next_message(&mut self) -> Result<Option<MailMessage>, MboxError> {
        match self.next_raw()? {
            Some(raw) => Ok(Some(MailMessage::from_raw(raw))),
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
