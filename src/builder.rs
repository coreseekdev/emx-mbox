use chrono::{DateTime, Local, Utc};
use uuid::Uuid;

use crate::message::MailMessage;

/// Construct RFC 5322 messages suitable for b4 / git-am workflows.
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

    /// Build a `[PATCH vN M/N]` style subject prefix.
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
        let date_str = date
            .with_timezone(&Local)
            .format("%a, %d %b %Y %H:%M:%S %z")
            .to_string();

        let msg_id = self
            .message_id
            .unwrap_or_else(generate_message_id);

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

        // Blank line separating headers from body.
        raw.push('\n');
        raw.push_str(&self.body);
        if !self.body.ends_with('\n') {
            raw.push('\n');
        }

        MailMessage::from_raw(raw.into_bytes())
    }
}

fn generate_message_id() -> String {
    let uuid = Uuid::new_v4();
    format!("<{}.emx@localhost>", uuid)
}
