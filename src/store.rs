use std::path::Path;

use crate::error::MailError;
use crate::message::MailMessage;

/// Unified interface for mail storage backends (mbox, maildir, …).
///
/// Every backend can load, save, append, and count messages.
/// Implementations are free to keep messages in memory or stream from disk.
pub trait MailStore {
    /// Load all messages from `path`.
    fn load(path: &Path) -> Result<Self, MailError>
    where
        Self: Sized;

    /// Number of messages currently held.
    fn len(&self) -> usize;

    /// Whether the store is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow all messages as a slice.
    fn messages(&self) -> &[MailMessage];

    /// Iterate over messages.
    fn iter(&self) -> std::slice::Iter<'_, MailMessage> {
        self.messages().iter()
    }

    /// Append a pre-built message.
    fn append(&mut self, msg: MailMessage);

    /// Save the entire store to `path`, replacing any previous content.
    fn save(&self, path: &Path) -> Result<(), MailError>;

    /// Append a single message to an existing store on disk.
    fn append_to(path: &Path, msg: &MailMessage) -> Result<(), MailError>
    where
        Self: Sized;

    /// Detect whether `path` looks like this storage format.
    fn detect(path: &Path) -> bool
    where
        Self: Sized;
}

/// Auto-detect the storage format at `path` and load it.
///
/// Tries Maildir first (checks for `new/`, `cur/`, `tmp/` subdirs),
/// then falls back to mbox (checks for `From ` prefix).
pub fn open(path: &Path) -> Result<Box<dyn MailStore>, MailError> {
    use crate::maildir::Maildir;
    use crate::mbox::Mbox;

    if Maildir::detect(path) {
        Ok(Box::new(Maildir::load(path)?))
    } else if Mbox::detect(path) {
        Ok(Box::new(Mbox::load_file(path)?))
    } else {
        Err(MailError::InvalidFormat(
            "unrecognized mail store format".into(),
        ))
    }
}
