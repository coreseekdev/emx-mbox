use std::path::Path;

use crate::error::MailError;
use crate::message::MailMessage;

/// Factory trait for loading, detecting, and appending to mail stores.
///
/// Implemented by concrete types (Mbox, Maildir).
/// These methods cannot be part of the object-safe `MailStore` trait
/// because they require `Self: Sized`.
pub trait MailStoreFactory: Sized {
    /// Load all messages from `path` and return a boxed instance for polymorphic use.
    fn load(path: &Path) -> Result<Box<dyn MailStore>, MailError>;

    /// Append a single message to an existing store on disk.
    fn append_to(path: &Path, msg: &MailMessage) -> Result<(), MailError>;

    /// Detect whether `path` looks like this storage format.
    fn detect(path: &Path) -> bool;
}

/// Object-safe interface for interacting with loaded mail stores.
///
/// Instance methods only; can be used as `&dyn MailStore`, `Box<dyn MailStore>`, etc.
/// Factory operations (load, detect, append_to) are on the `MailStoreFactory` trait.
pub trait MailStore {
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
}

/// Auto-detect the storage format at `path` and load it.
///
/// Tries Maildir first (checks for `new/`, `cur/`, `tmp/` subdirs),
/// then falls back to mbox (checks for `From ` prefix).
pub fn open(path: &Path) -> Result<Box<dyn MailStore>, MailError> {
    use crate::maildir::Maildir;
    use crate::mbox::Mbox;

    if Maildir::detect(path) {
        <Maildir as MailStoreFactory>::load(path)
    } else if Mbox::detect(path) {
        <Mbox as MailStoreFactory>::load(path)
    } else {
        Err(MailError::InvalidFormat(
            "unrecognized mail store format".into(),
        ))
    }
}
