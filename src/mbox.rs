use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::builder::MessageBuilder;
use crate::error::MailError;
use crate::format::MboxFormat;
use crate::message::{ensure_body_size_limit, MailMessage};
use crate::reader::MboxReader;
use crate::store::{MailStore, MailStoreFactory};
use crate::writer::MboxWriter;

/// In-memory collection of messages with load / append operations.
///
/// This module follows the **append-only** principle for mbox files.
/// Use `append_to_file` for writing messages to disk.
pub struct Mbox {
    messages: Vec<MailMessage>,
}

impl Mbox {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Load an mbox archive from a reader.
    pub fn load<R: Read>(reader: R) -> Result<Self, MailError> {
        let reader = MboxReader::new(reader);
        let mut messages = Vec::new();
        for result in reader {
            messages.push(result?);
        }
        Ok(Self { messages })
    }

    /// Load an mbox archive from a reader with a specific format.
    pub fn load_with_format<R: Read>(reader: R, format: MboxFormat) -> Result<Self, MailError> {
        let reader = MboxReader::new(reader).with_format(format);
        let mut messages = Vec::new();
        for result in reader {
            messages.push(result?);
        }
        Ok(Self { messages })
    }

    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Self, MailError> {
        let file = File::open(path)?;
        Self::load(file)
    }

    /// Append raw RFC 5322 bytes (e.g. from an EML file or network).
    pub fn append_raw(&mut self, raw: Vec<u8>) -> Result<(), MailError> {
        ensure_body_size_limit(&raw)?;
        self.messages.push(MailMessage::from_raw(raw));
        Ok(())
    }

    /// Append from an EML file on disk.
    pub fn append_eml<P: AsRef<Path>>(&mut self, path: P) -> Result<(), MailError> {
        self.messages.push(MailMessage::from_eml_file(path)?);
        Ok(())
    }

    /// Build and append a new message using `MessageBuilder`.
    pub fn append_built(&mut self, builder: MessageBuilder) {
        self.messages.push(builder.build());
    }

    /// Legacy helper kept for backward compatibility.
    pub fn add_message(&mut self, from: &str, subject: &str, body: &str) {
        self.append_built(MessageBuilder::new(from, subject).body(body));
    }

    /// Write the entire archive through an `MboxWriter`.
    pub fn write_to<W: Write>(&self, writer: &mut MboxWriter<W>) -> Result<(), MailError> {
        for msg in &self.messages {
            writer.write_mail_message(msg)?;
        }
        Ok(())
    }

    /// Append a single message to an existing mbox file on disk.
    pub fn append_to_file<P: AsRef<Path>>(path: P, msg: &MailMessage) -> Result<(), MailError> {
        let mut w = MboxWriter::open_append(path)?;
        w.write_mail_message(msg)
    }
}

impl Default for Mbox {
    fn default() -> Self {
        Self::new()
    }
}

impl MailStoreFactory for Mbox {
    fn load(path: &Path) -> Result<Box<dyn MailStore>, MailError> {
        let mbox = Self::load_file(path)?;
        Ok(Box::new(mbox))
    }

    fn append_to(path: &Path, msg: &MailMessage) -> Result<(), MailError> {
        Self::append_to_file(path, msg)
    }

    fn detect(path: &Path) -> bool {
        use std::io::Read;
        if !path.is_file() {
            return false;
        }
        let Ok(mut f) = File::open(path) else {
            return false;
        };
        // Use `read` instead of `read_exact` to avoid blocking on small files
        let mut buf = [0u8; 5];
        let n = f.read(&mut buf).unwrap_or(0);
        n == 5 && buf == *b"From "
    }
}

impl MailStore for Mbox {
    #[inline]
    fn len(&self) -> usize {
        self.messages.len()
    }

    #[inline]
    fn messages(&self) -> &[MailMessage] {
        &self.messages
    }

    fn append(&mut self, msg: MailMessage) {
        self.messages.push(msg);
    }
}

impl<'a> IntoIterator for &'a Mbox {
    type Item = &'a MailMessage;
    type IntoIter = std::slice::Iter<'a, MailMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

impl IntoIterator for Mbox {
    type Item = MailMessage;
    type IntoIter = std::vec::IntoIter<MailMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}
