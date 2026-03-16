use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::builder::MessageBuilder;
use crate::error::MboxError;
use crate::format::MboxFormat;
use crate::message::MailMessage;
use crate::reader::MboxReader;
use crate::writer::MboxWriter;

/// In-memory collection of messages with load / save / append.
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
    pub fn load<R: Read>(reader: R) -> Result<Self, MboxError> {
        let reader = MboxReader::new(reader);
        let mut messages = Vec::new();
        for result in reader {
            messages.push(result?);
        }
        Ok(Self { messages })
    }

    /// Load an mbox archive from a reader with a specific format.
    pub fn load_with_format<R: Read>(reader: R, format: MboxFormat) -> Result<Self, MboxError> {
        let reader = MboxReader::new(reader).with_format(format);
        let mut messages = Vec::new();
        for result in reader {
            messages.push(result?);
        }
        Ok(Self { messages })
    }

    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = File::open(path)?;
        Self::load(file)
    }

    pub fn messages(&self) -> &[MailMessage] {
        &self.messages
    }

    pub fn iter(&self) -> impl Iterator<Item = &MailMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Append a pre-built `MailMessage`.
    pub fn append(&mut self, msg: MailMessage) {
        self.messages.push(msg);
    }

    /// Append raw RFC 5322 bytes (e.g. from an EML file or network).
    pub fn append_raw(&mut self, raw: Vec<u8>) {
        self.messages.push(MailMessage::from_raw(raw));
    }

    /// Append from an EML file on disk.
    pub fn append_eml<P: AsRef<Path>>(&mut self, path: P) -> Result<(), MboxError> {
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

    /// Save the entire archive to a writer.
    pub fn save<W: Write>(&self, writer: &mut MboxWriter<W>) -> Result<(), MboxError> {
        for msg in &self.messages {
            writer.write_mail_message(msg)?;
        }
        Ok(())
    }

    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> Result<(), MboxError> {
        let mut w = MboxWriter::from_file(path)?;
        self.save(&mut w)
    }

    /// Append a single message to an existing mbox file on disk.
    pub fn append_to_file<P: AsRef<Path>>(path: P, msg: &MailMessage) -> Result<(), MboxError> {
        let mut w = MboxWriter::open_append(path)?;
        w.write_mail_message(msg)
    }
}

impl Default for Mbox {
    fn default() -> Self {
        Self::new()
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
