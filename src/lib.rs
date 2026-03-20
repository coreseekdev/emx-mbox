mod attachment;
mod builder;
mod error;
mod format;
mod index;
mod maildir;
mod mbox;
mod message;
mod reader;
mod store;
mod writer;

use std::collections::HashSet;

pub use attachment::Attachment;
pub use builder::MessageBuilder;
pub use error::MailError;
pub use format::MboxFormat;
pub use index::{MessageLocation, MessageMeta, MboxIndex};
pub use maildir::Maildir;
pub use mbox::Mbox;
pub use message::MailMessage;
pub use reader::MboxReader;
pub use store::{open, MailStore};
pub use writer::MboxWriter;

// Tombstone support for mark-deletion
// Tombstone is a special message appended to mark another message as deleted.

/// Header for tombstone status
pub const X_LLM_STATUS: &str = "X-LLM-Status";

/// Value indicating a deleted message
pub const TOMBSTONE_STATUS: &str = "deleted";

/// Header referencing the original deleted message ID
pub const X_LLM_DELETED_ID: &str = "X-LLM-Deleted-Message-ID";

// Supplement support for subject override
// Supplement is a special message appended to override another message's subject.

/// Header referencing the target message ID for subject supplement
pub const X_SUPPLEMENTS_MESSAGE_ID: &str = "Supplements-Message-ID";

/// Check if a message is a tombstone (marks another message as deleted)
pub fn is_tombstone(msg: &MailMessage) -> bool {
    msg.header(X_LLM_STATUS)
        .map(|s| s == TOMBSTONE_STATUS)
        .unwrap_or(false)
}

/// Get the original message ID from a tombstone message
pub fn get_deleted_message_id(msg: &MailMessage) -> Option<&str> {
    if is_tombstone(msg) {
        msg.header(X_LLM_DELETED_ID)
    } else {
        None
    }
}

/// Collect all deleted message IDs referenced by tombstone messages.
pub fn deleted_message_ids(all_messages: &[MailMessage]) -> HashSet<&str> {
    let mut deleted_ids = HashSet::new();
    for msg in all_messages {
        if let Some(deleted_id) = get_deleted_message_id(msg) {
            deleted_ids.insert(deleted_id);
        }
    }
    deleted_ids
}

/// Check if a message is marked as deleted by any tombstone
/// This is used to filter out deleted messages from listings
pub fn is_deleted(msg: &MailMessage, all_messages: &[MailMessage]) -> bool {
    let msg_id = match msg.message_id() {
        Some(id) => id,
        None => return false,
    };

    for other in all_messages {
        if is_tombstone(other) {
            if let Some(deleted_id) = get_deleted_message_id(other) {
                if deleted_id == msg_id {
                    return true;
                }
            }
        }
    }
    false
}
