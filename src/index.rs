//! In-memory index for mbox files.
//!
//! The index stores message metadata (headers) separately from message locations,
//! allowing efficient queries without loading full message bodies into memory.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::error::MailError;
use crate::message::MailMessage;
use crate::reader::MboxReader;
use crate::{TOMBSTONE_STATUS, X_LLM_DELETED_ID, X_LLM_STATUS, X_SUPPLEMENTS_MESSAGE_ID};

/// Message metadata containing parsed headers.
///
/// This stores the key-value pairs of all headers in memory,
/// allowing fast access without parsing the raw message again.
#[derive(Debug, Clone)]
pub struct MessageMeta {
    /// All headers as a key-value map (lowercased keys).
    pub headers: HashMap<String, String>,
    /// Whether this message is marked as deleted (by tombstone).
    pub deleted: bool,
}

impl MessageMeta {
    /// Create metadata from a mail message.
    pub fn from_message(msg: &MailMessage) -> Self {
        let mut headers = HashMap::new();

        // Collect all headers
        if let Ok(parsed) = msg.parsed() {
            for h in &parsed.headers {
                let key = h.get_key().to_ascii_lowercase();
                let value = h.get_value();
                headers.insert(key, value);
            }
        }

        Self {
            headers,
            deleted: false,
        }
    }

    /// Get a header value by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_ascii_lowercase()).map(|s| s.as_str())
    }

    /// Get the subject header.
    pub fn subject(&self) -> &str {
        self.get("Subject").unwrap_or("")
    }

    /// Get the from header.
    pub fn from(&self) -> &str {
        self.get("From").unwrap_or("")
    }

    /// Get the message-id header.
    pub fn message_id(&self) -> Option<&str> {
        self.get("Message-ID")
    }

    /// Check if this message is marked as deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }
}

/// Location of a message within the mbox file.
///
/// # Note
///
/// Currently `offset` and `size` are placeholders (0, 0) because `MboxReader`
/// does not track byte positions. This is reserved for future implementation
/// of on-demand body reading.
///
/// When implemented, this will allow efficient seeking to specific messages
/// without loading the entire mbox into memory.
#[derive(Debug, Clone, Copy)]
pub struct MessageLocation {
    /// Byte offset of the message start in the mbox file.
    /// Currently a placeholder (always 0).
    pub offset: u64,
    /// Size of the message in bytes.
    /// Currently a placeholder (always 0).
    pub size: u64,
}

impl MessageLocation {
    pub fn new(offset: u64, size: u64) -> Self {
        Self { offset, size }
    }
}

/// In-memory index for an mbox file.
///
/// Maps message-id → (metadata, location), preserving insertion order.
///
/// The index tracks:
/// - Regular messages with their headers and locations
/// - Supplement messages (subject overrides)
/// - Tombstone messages (deletion markers)
#[derive(Debug, Clone)]
pub struct MboxIndex {
    /// Ordered map preserving message sequence in the mbox.
    messages: IndexMap<String, (MessageMeta, MessageLocation)>,
    /// Supplements mapping: target message-id → supplement subject
    supplements: HashMap<String, String>,
    /// Deleted message IDs (from tombstones)
    deleted: HashSet<String>,
}

impl MboxIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self {
            messages: IndexMap::new(),
            supplements: HashMap::new(),
            deleted: HashSet::new(),
        }
    }

    /// Build an index by streaming through an mbox reader.
    ///
    /// This processes the mbox in a single pass:
    /// 1. Collect all supplement messages
    /// 2. Collect all tombstone messages
    /// 3. Index all regular messages
    /// 4. Apply supplements and tombstones automatically
    pub fn from_reader<R: std::io::Read>(
        reader: &mut MboxReader<R>,
    ) -> Result<Self, MailError> {
        let mut index = Self::new();

        // First pass: collect supplements, tombstones, and index all messages
        for msg in reader.by_ref() {
            let msg = msg?;

            // Check if this is a tombstone message
            if msg.header(X_LLM_STATUS)
                .map(|s| s == TOMBSTONE_STATUS)
                .unwrap_or(false)
            {
                if let Some(deleted_id) = msg.header(X_LLM_DELETED_ID) {
                    index.deleted.insert(deleted_id.to_string());
                }
                continue; // Don't index tombstone messages themselves
            }

            // Check if this is a supplement message
            if let Some(target_id) = msg.header(X_SUPPLEMENTS_MESSAGE_ID) {
                let subject = msg.subject();
                if !subject.is_empty() {
                    index.supplements.insert(target_id.to_string(), subject.to_string());
                }
                continue; // Don't index supplement messages themselves
            }

            // Regular message - index it
            if let Some(msg_id) = msg.message_id() {
                // Note: We don't have offset/size from the current reader API
                // This will need to be enhanced or use placeholders for now
                let meta = MessageMeta::from_message(&msg);
                let location = MessageLocation::new(0, 0); // Placeholder
                index.messages.insert(msg_id.to_string(), (meta, location));
            }
        }

        // Apply supplements and tombstones
        index.apply_supplements();
        index.apply_tombstones();

        Ok(index)
    }

    /// Apply supplements to the indexed messages.
    fn apply_supplements(&mut self) {
        for (_msg_id, (meta, _location)) in &mut self.messages {
            if let Some(msg_id) = meta.message_id() {
                if let Some(supplement_subject) = self.supplements.get(msg_id) {
                    meta.headers.insert("subject".to_string(), supplement_subject.clone());
                }
            }
        }
    }

    /// Apply tombstones to the indexed messages.
    fn apply_tombstones(&mut self) {
        for (_msg_id, (meta, _location)) in &mut self.messages {
            if let Some(msg_id) = meta.message_id() {
                if self.deleted.contains(msg_id) {
                    meta.deleted = true;
                }
            }
        }
    }

    /// Get message metadata by message-id.
    pub fn get(&self, message_id: &str) -> Option<&(MessageMeta, MessageLocation)> {
        self.messages.get(message_id)
    }

    /// Get all message IDs in insertion order.
    pub fn message_ids(&self) -> impl Iterator<Item = &String> {
        self.messages.keys()
    }

    /// Get the number of indexed messages (excluding supplements/tombstones).
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the number of supplement messages.
    pub fn supplement_count(&self) -> usize {
        self.supplements.len()
    }
}

impl Default for MboxIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::MessageBuilder;

    #[test]
    fn test_message_meta_from_message() {
        let msg = MessageBuilder::new("alice@example.com", "Test Subject")
            .body("Test body")
            .build();

        let meta = MessageMeta::from_message(&msg);

        assert_eq!(meta.subject(), "Test Subject");
        assert_eq!(meta.from(), "alice@example.com");
        assert!(meta.message_id().is_some());
        assert!(!meta.deleted);
    }

    #[test]
    fn test_message_meta_get() {
        let msg = MessageBuilder::new("bob@example.com", "Hello")
            .body("World")
            .build();

        let meta = MessageMeta::from_message(&msg);

        assert_eq!(meta.get("Subject"), Some("Hello"));
        assert_eq!(meta.get("subject"), Some("Hello")); // case-insensitive
        assert_eq!(meta.get("SUBJECT"), Some("Hello")); // case-insensitive
    }

    #[test]
    fn test_message_meta_deleted() {
        let mut meta = MessageMeta::from_message(
            &MessageBuilder::new("test@example.com", "Test").build(),
        );
        assert!(!meta.deleted);

        meta.deleted = true;
        assert!(meta.is_deleted());
    }

    #[test]
    fn test_mbox_index_new() {
        let index = MboxIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.supplement_count(), 0);
    }

    #[test]
    fn test_message_location() {
        let loc = MessageLocation::new(1024, 2048);
        assert_eq!(loc.offset, 1024);
        assert_eq!(loc.size, 2048);
    }
}
