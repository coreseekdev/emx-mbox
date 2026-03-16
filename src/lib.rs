use chrono::{DateTime, Local, Utc};
use mailparse::parse_mail;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum MboxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] mailparse::MailParseError),
    #[error("Invalid mbox format: {0}")]
    InvalidFormat(String),
}

// ---------------------------------------------------------------------------
// Mbox format variant
// ---------------------------------------------------------------------------

/// Which flavour of "From " escaping to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MboxFormat {
    /// mboxo – only bare `From ` lines are escaped with a single `>`.
    Mboxo,
    /// mboxrd – every leading `>*From ` is escaped by prepending one more `>`.
    /// This is what b4 prefers.
    Mboxrd,
}

impl Default for MboxFormat {
    fn default() -> Self {
        MboxFormat::Mboxrd
    }
}

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

/// Check whether a byte-line is an mbox `From ` separator.
fn is_from_line(line: &[u8]) -> bool {
    line.starts_with(b"From ") && !line.starts_with(b"From:")
}

/// Strip exactly one leading `>` from an mboxrd-escaped `>+From ` line.
/// Returns the unescaped bytes (without trailing newline handling).
fn unescape_from_line(line: &[u8]) -> Vec<u8> {
    // mboxrd: `>+From ` → remove one `>`
    if line.starts_with(b">") {
        let rest = &line[1..];
        if rest.starts_with(b"From ") || rest.starts_with(b">") && is_escaped_from(rest) {
            return rest.to_vec();
        }
    }
    line.to_vec()
}

fn is_escaped_from(line: &[u8]) -> bool {
    let stripped = line.iter().skip_while(|&&b| b == b'>').copied().collect::<Vec<_>>();
    stripped.starts_with(b"From ")
}

/// Escape a line for mboxrd: if the line starts with `>*From `, prepend `>`.
fn escape_from_line_mboxrd(line: &[u8]) -> Vec<u8> {
    if line.starts_with(b"From ") || (line.starts_with(b">") && is_escaped_from(line)) {
        let mut out = vec![b'>'];
        out.extend_from_slice(line);
        out
    } else {
        line.to_vec()
    }
}

/// Escape a line for mboxo: only bare `From ` is escaped.
fn escape_from_line_mboxo(line: &[u8]) -> Vec<u8> {
    if line.starts_with(b"From ") {
        let mut out = vec![b'>'];
        out.extend_from_slice(line);
        out
    } else {
        line.to_vec()
    }
}

/// Normalize CRLF to LF in a byte slice.
fn normalize_line_endings(data: &[u8]) -> Vec<u8> {
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
    out
}

// ---------------------------------------------------------------------------
// MboxReader – streaming reader, returns owned raw bytes per message
// ---------------------------------------------------------------------------

pub struct MboxReader<R: Read> {
    reader: BufReader<R>,
    /// Whether we've found the first `From ` separator yet.
    started: bool,
    /// Whether the underlying reader is at EOF.
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

        // ------------------------------------------------------------------
        // Phase 1: find the next `From ` separator
        // ------------------------------------------------------------------
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
        // If `started` is already true, we stopped at a `From ` line last
        // time – it was already consumed by the previous call's loop.

        // ------------------------------------------------------------------
        // Phase 2: collect message lines until the next separator or EOF
        // ------------------------------------------------------------------
        let mut message = Vec::new();
        let mut prev_blank: Option<Vec<u8>> = None;

        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line)?;
            if n == 0 {
                // EOF – flush any pending blank line as part of the message.
                if let Some(blank) = prev_blank.take() {
                    message.extend_from_slice(&blank);
                }
                self.at_eof = true;
                break;
            }

            let raw_bytes = line.as_bytes();
            let trimmed = line.trim_end_matches(['\r', '\n']);

            // Blank line: buffer it – if the next line is `From ` separator
            // this blank line is the separator gap, not part of the body.
            if trimmed.is_empty() {
                // Flush any previous blank (consecutive blanks are body).
                if let Some(blank) = prev_blank.take() {
                    message.extend_from_slice(&blank);
                }
                prev_blank = Some(normalize_line_endings(raw_bytes));
                continue;
            }

            if is_from_line(trimmed.as_bytes()) {
                // We hit the next message's separator.
                // The buffered blank line (if any) was the inter-message gap.
                // Don't add it to the current message.
                break;
            }

            // Not a separator – flush any buffered blank line, then add this.
            if let Some(blank) = prev_blank.take() {
                message.extend_from_slice(&blank);
            }

            // Unescape mboxrd `>From` lines.
            let unescaped = unescape_from_line(trimmed.as_bytes());
            message.extend_from_slice(&unescaped);
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

// ---------------------------------------------------------------------------
// MboxWriter – streaming writer
// ---------------------------------------------------------------------------

pub struct MboxWriter<W: Write> {
    writer: W,
    format: MboxFormat,
    has_written: bool,
}

impl MboxWriter<File> {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = File::create(path)?;
        Ok(Self::new(file))
    }

    /// Open for append – does not truncate the file.
    pub fn open_append<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: file,
            format: MboxFormat::default(),
            has_written: false,
        })
    }
}

impl<W: Write> MboxWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            format: MboxFormat::default(),
            has_written: false,
        }
    }

    pub fn with_format(mut self, format: MboxFormat) -> Self {
        self.format = format;
        self
    }

    /// Write a raw message (headers + body bytes) into the mbox.
    /// `envelope_from` is the sender in the `From ` separator line.
    /// `date` is used in the separator line timestamp.
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

        // ANSIC date format: `Thu Jan  1 00:00:00 1970`
        let date_str = date.format("%a %b %e %H:%M:%S %Y").to_string();

        write!(self.writer, "From {} {}\n", from, date_str)?;

        let normalized = normalize_line_endings(raw_message);

        // Ensure message ends with a newline before the blank separator.
        let msg = if normalized.last() != Some(&b'\n') {
            let mut m = normalized;
            m.push(b'\n');
            m
        } else {
            normalized
        };

        // Write each line, escaping `From ` as needed.
        // Use split-inclusive logic: split on \n but preserve the semantics
        // of blank lines.
        let lines: Vec<&[u8]> = msg.split(|&b| b == b'\n').collect();
        // `split` produces N+1 elements for N newlines. The last element
        // after trailing \n is empty and should be ignored (it's just the
        // artifact of split on the final \n).
        let line_count = if lines.last() == Some(&&b""[..]) {
            lines.len() - 1
        } else {
            lines.len()
        };
        for line in &lines[..line_count] {
            if line.is_empty() {
                // Blank line – write just a newline.
                self.writer.write_all(b"\n")?;
            } else {
                let escaped = match self.format {
                    MboxFormat::Mboxrd => escape_from_line_mboxrd(line),
                    MboxFormat::Mboxo => escape_from_line_mboxo(line),
                };
                self.writer.write_all(&escaped)?;
                self.writer.write_all(b"\n")?;
            }
        }

        // Blank line separator after each message.
        self.writer.write_all(b"\n")?;
        self.has_written = true;
        Ok(())
    }

    /// Convenience: write a `MailMessage`.
    pub fn write_mail_message(&mut self, msg: &MailMessage) -> Result<(), MboxError> {
        let date = msg.date().unwrap_or_else(Utc::now);
        let envelope = msg
            .header("From")
            .and_then(|v| extract_email_address(&v))
            .unwrap_or_else(|| "mboxrd@z".into());
        self.write_message(&envelope, &date, &msg.raw)
    }
}

// ---------------------------------------------------------------------------
// MailMessage – owned message with lazy header access
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MailMessage {
    /// The complete raw message bytes (headers + body), with LF line endings.
    pub raw: Vec<u8>,
}

impl MailMessage {
    /// Build a `MailMessage` from raw RFC 5322 bytes.
    pub fn from_raw(raw: Vec<u8>) -> Self {
        Self {
            raw: normalize_line_endings(&raw),
        }
    }

    /// Load a single message from an EML file.
    pub fn from_eml_file<P: AsRef<Path>>(path: P) -> Result<Self, MboxError> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;
        Ok(Self::from_raw(data))
    }

    /// Return the value of a header (first occurrence), or `None`.
    pub fn header(&self, name: &str) -> Option<String> {
        let parsed = parse_mail(&self.raw).ok()?;
        parsed
            .headers
            .iter()
            .find(|h| h.get_key().eq_ignore_ascii_case(name))
            .map(|h| h.get_value())
    }

    /// Shorthand accessors
    pub fn from(&self) -> String {
        self.header("From").unwrap_or_default()
    }

    pub fn subject(&self) -> String {
        self.header("Subject").unwrap_or_default()
    }

    pub fn message_id(&self) -> Option<String> {
        self.header("Message-ID").or_else(|| self.header("Message-Id"))
    }

    pub fn date(&self) -> Option<DateTime<Utc>> {
        let val = self.header("Date")?;
        mailparse::dateparse(&val)
            .ok()
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now))
    }

    /// Return the decoded body text (first text/plain part).
    pub fn body(&self) -> String {
        parse_mail(&self.raw)
            .ok()
            .and_then(|p| p.get_body().ok())
            .unwrap_or_default()
    }

    /// Access to the full parsed structure.
    pub fn parsed(&self) -> Result<mailparse::ParsedMail<'_>, MboxError> {
        Ok(parse_mail(&self.raw)?)
    }
}

impl fmt::Display for MailMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.raw))
    }
}

// ---------------------------------------------------------------------------
// MessageBuilder – construct RFC 5322 messages suitable for b4 / git-am
// ---------------------------------------------------------------------------

pub struct MessageBuilder {
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    message_id: Option<String>,
    in_reply_to: Option<String>,
    references: Vec<String>,
    date: Option<DateTime<Utc>>,
    extra_headers: Vec<(String, String)>,
}

impl MessageBuilder {
    pub fn new(from: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: Vec::new(),
            cc: Vec::new(),
            subject: subject.into(),
            body: String::new(),
            message_id: None,
            in_reply_to: None,
            references: Vec::new(),
            date: None,
            extra_headers: Vec::new(),
        }
    }

    pub fn to(mut self, addr: impl Into<String>) -> Self {
        self.to.push(addr.into());
        self
    }

    pub fn cc(mut self, addr: impl Into<String>) -> Self {
        self.cc.push(addr.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn message_id(mut self, id: impl Into<String>) -> Self {
        self.message_id = Some(id.into());
        self
    }

    pub fn in_reply_to(mut self, id: impl Into<String>) -> Self {
        self.in_reply_to = Some(id.into());
        self
    }

    pub fn reference(mut self, id: impl Into<String>) -> Self {
        self.references.push(id.into());
        self
    }

    pub fn date(mut self, date: DateTime<Utc>) -> Self {
        self.date = Some(date);
        self
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    /// Helper: build a `[PATCH vN M/N]` style subject.
    pub fn patch_subject(
        mut self,
        prefix: Option<&str>,
        version: Option<u32>,
        index: u32,
        total: u32,
    ) -> Self {
        let mut tag = String::from("[");
        tag.push_str(prefix.unwrap_or("PATCH"));
        if let Some(v) = version {
            tag.push_str(&format!(" v{}", v));
        }
        tag.push_str(&format!(" {}/{}", index, total));
        tag.push(']');
        self.subject = format!("{} {}", tag, self.subject);
        self
    }

    /// Build the message and return a `MailMessage`.
    pub fn build(self) -> MailMessage {
        let mut raw = String::new();

        let date = self.date.unwrap_or_else(Utc::now);
        // RFC 5322 date
        let date_str = date
            .with_timezone(&Local)
            .format("%a, %d %b %Y %H:%M:%S %z")
            .to_string();

        let msg_id = self.message_id.unwrap_or_else(|| generate_message_id());

        // Required headers
        raw.push_str(&format!("From: {}\n", self.from));
        if !self.to.is_empty() {
            raw.push_str(&format!("To: {}\n", self.to.join(", ")));
        }
        if !self.cc.is_empty() {
            raw.push_str(&format!("Cc: {}\n", self.cc.join(", ")));
        }
        raw.push_str(&format!("Subject: {}\n", self.subject));
        raw.push_str(&format!("Date: {}\n", date_str));
        raw.push_str(&format!("Message-ID: {}\n", msg_id));
        raw.push_str("MIME-Version: 1.0\n");
        raw.push_str("Content-Type: text/plain; charset=utf-8\n");

        if let Some(ref irt) = self.in_reply_to {
            raw.push_str(&format!("In-Reply-To: {}\n", irt));
        }
        if !self.references.is_empty() {
            raw.push_str(&format!("References: {}\n", self.references.join("\n ")));
        }

        for (name, value) in &self.extra_headers {
            raw.push_str(&format!("{}: {}\n", name, value));
        }

        // Blank line separating headers from body
        raw.push('\n');
        raw.push_str(&self.body);
        if !self.body.ends_with('\n') {
            raw.push('\n');
        }

        MailMessage::from_raw(raw.into_bytes())
    }
}

/// Generate a unique Message-ID.
fn generate_message_id() -> String {
    let uuid = Uuid::new_v4();
    format!("<{}.emx@localhost>", uuid)
}

/// Extract the bare email address from a `From:` header value.
fn extract_email_address(from: &str) -> Option<String> {
    // Try `Name <addr>` format
    if let Some(start) = from.find('<') {
        if let Some(end) = from.find('>') {
            return Some(from[start + 1..end].to_string());
        }
    }
    // Bare email
    let trimmed = from.trim();
    if trimmed.contains('@') {
        return Some(trimmed.to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// Mbox – in-memory collection with load / save / append
// ---------------------------------------------------------------------------

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
        let mut mbox_reader = MboxReader::new(reader);
        let mut messages = Vec::new();

        while let Some(msg) = mbox_reader.next_message()? {
            messages.push(msg);
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
        let msg = MailMessage::from_eml_file(path)?;
        self.messages.push(msg);
        Ok(())
    }

    /// Build and append a new message using `MessageBuilder`.
    pub fn append_built(&mut self, builder: MessageBuilder) {
        self.messages.push(builder.build());
    }

    /// Legacy helper kept for backward compatibility.
    pub fn add_message(&mut self, from: &str, subject: &str, body: &str) {
        let builder = MessageBuilder::new(from, subject).body(body);
        self.append_built(builder);
    }

    /// Save the entire archive to a writer.
    pub fn save<W: Write>(&self, writer: &mut MboxWriter<W>) -> Result<(), MboxError> {
        for msg in &self.messages {
            writer.write_mail_message(msg)?;
        }
        Ok(())
    }

    pub fn save_file<P: AsRef<Path>>(&self, path: P) -> Result<(), MboxError> {
        let mut mbox_writer = MboxWriter::from_file(path)?;
        self.save(&mut mbox_writer)
    }

    /// Append a single message to an existing mbox file on disk.
    pub fn append_to_file<P: AsRef<Path>>(
        path: P,
        msg: &MailMessage,
    ) -> Result<(), MboxError> {
        let mut writer = MboxWriter::open_append(path)?;
        writer.write_mail_message(msg)
    }
}

impl Default for Mbox {
    fn default() -> Self {
        Self::new()
    }
}
