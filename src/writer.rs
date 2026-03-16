use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::MboxError;
use crate::format::{escape_from_line, normalize_line_endings, extract_email_address, MboxFormat};
use crate::message::MailMessage;

/// Streaming mbox writer.
pub struct MboxWriter<W: Write> {
    writer: W,
    format: MboxFormat,
}

impl MboxWriter<File> {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = File::create(path)?;
        Ok(Self::new(file))
    }

    /// Open for append — does not truncate the file.
    pub fn open_append<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: file,
            format: MboxFormat::default(),
        })
    }
}

impl<W: Write> MboxWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            format: MboxFormat::default(),
        }
    }

    pub fn with_format(mut self, format: MboxFormat) -> Self {
        self.format = format;
        self
    }

    /// Write a raw message (headers + body bytes) into the mbox.
    ///
    /// * `envelope_from` — sender for the `From ` separator line.
    /// * `date` — timestamp for the separator line.
    pub fn write_message(
        &mut self,
        envelope_from: &str,
        date: &DateTime<Utc>,
        raw_message: &[u8],
    ) -> Result<(), MboxError> {
        let from = if envelope_from.is_empty() {
            "mboxrd@z"
        } else {
            envelope_from
        };

        // ANSIC date: `Thu Jan  1 00:00:00 1970`
        let date_str = date.format("%a %b %e %H:%M:%S %Y").to_string();
        write!(self.writer, "From {} {}\n", from, date_str)?;

        let normalized = normalize_line_endings(raw_message);
        let msg = ensure_trailing_newline(normalized);

        // Split on `\n`.  The final empty element after the trailing `\n`
        // is an artifact of split and should be skipped.
        let lines: Vec<&[u8]> = msg.split(|&b| b == b'\n').collect();
        let count = if lines.last() == Some(&&b""[..]) {
            lines.len() - 1
        } else {
            lines.len()
        };

        for line in &lines[..count] {
            if let Some(prefix) = escape_from_line(line, self.format) {
                self.writer.write_all(&[prefix])?;
            }
            self.writer.write_all(line)?;
            self.writer.write_all(b"\n")?;
        }

        // Blank line separator after each message.
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    /// Convenience: write a `MailMessage`.
    pub fn write_mail_message(&mut self, msg: &MailMessage) -> Result<(), MboxError> {
        let date = msg.date().unwrap_or_else(Utc::now);
        let envelope = msg
            .envelope_from
            .clone()
            .or_else(|| {
                msg.header("From")
                    .and_then(|v| extract_email_address(&v))
            })
            .unwrap_or_else(|| "mboxrd@z".into());
        self.write_message(&envelope, &date, &msg.raw)
    }
}

fn ensure_trailing_newline(mut data: Vec<u8>) -> Vec<u8> {
    if data.last() != Some(&b'\n') {
        data.push(b'\n');
    }
    data
}
