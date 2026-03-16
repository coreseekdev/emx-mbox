use std::path::Path;

use crate::error::MboxError;
use crate::message::MailMessage;

/// Unified interface for mail storage backends (mbox, maildir, …).
///
/// Every backend can load, save, append, and count messages.
/// Implementations are free to keep messages in memory or stream from disk.
pub trait MailStore {
    /// Load all messages from `path`.
    fn load(path: &Path) -> Result<Self, MboxError>
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
    fn save(&self, path: &Path) -> Result<(), MboxError>;

    /// Append a single message to an existing store on disk.
    fn append_to(path: &Path, msg: &MailMessage) -> Result<(), MboxError>
    where
        Self: Sized;

    /// Detect whether `path` looks like this storage format.
    fn detect(path: &Path) -> bool
    where
        Self: Sized;
}
