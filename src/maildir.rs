use std::fs;
use std::path::{Path, PathBuf};

use crate::error::MailError;
use crate::message::MailMessage;
use crate::store::MailStore;

/// Maildir storage backend (new / cur / tmp).
///
/// Compatible with b4 `--save-as-maildir`: messages are written atomically
/// via `tmp/` then renamed into `new/`.
pub struct Maildir {
    root: PathBuf,
    messages: Vec<MailMessage>,
}

impl Maildir {
    /// Create an empty Maildir handle (no I/O until `save`).
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            messages: Vec::new(),
        }
    }

    /// The root directory.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Detect whether `path` has the standard maildir subdirectories.
    #[inline]
    pub fn is_maildir<P: AsRef<Path>>(path: P) -> bool {
        let p = path.as_ref();
        p.join("new").is_dir() && p.join("cur").is_dir() && p.join("tmp").is_dir()
    }

    /// Ensure `new/`, `cur/`, `tmp/` exist under the given root.
    fn ensure_dirs(root: &Path) -> Result<(), MailError> {
        fs::create_dir_all(root.join("new"))?;
        fs::create_dir_all(root.join("cur"))?;
        fs::create_dir_all(root.join("tmp"))?;
        Ok(())
    }

    /// Generate a b4-compatible filename from a message.
    ///
    /// Pattern: `{counter:04}_{subject_slug}.eml`
    fn message_filename(msg: &MailMessage, index: usize) -> String {
        let subject = msg.subject();
        let (counter, slug) = parse_patch_subject(subject, index);
        format!("{:04}_{}.eml", counter, slug)
    }

    /// Write a single message atomically: tmp → new.
    /// If the target filename already exists in `new/`, a suffix is appended
    /// to avoid silently overwriting data.
    fn write_message_atomic(root: &Path, msg: &MailMessage, index: usize) -> Result<(), MailError> {
        let base_name = Self::message_filename(msg, index);
        let new_path = root.join("new").join(&base_name);
        let (final_new, final_name) = if !new_path.exists() {
            (new_path, base_name)
        } else {
            // Append _1, _2, … to deduplicate
            let stem = base_name.trim_end_matches(".eml");
            let mut found = None;
            for i in 1u32..10000 {
                let alt = format!("{}_{}.eml", stem, i);
                let alt_path = root.join("new").join(&alt);
                if !alt_path.exists() {
                    found = Some((alt_path, alt));
                    break;
                }
            }
            found.ok_or_else(|| MailError::InvalidFormat("too many filename collisions".into()))?
        };
        let tmp_path = root.join("tmp").join(&final_name);
        fs::write(&tmp_path, msg.raw())?;
        fs::rename(&tmp_path, &final_new)?;
        Ok(())
    }

    /// Append raw RFC 5322 bytes.
    pub fn append_raw(&mut self, raw: Vec<u8>) {
        self.messages.push(MailMessage::from_raw(raw));
    }

    /// Append from an EML file on disk.
    pub fn append_eml<P: AsRef<Path>>(&mut self, path: P) -> Result<(), MailError> {
        self.messages.push(MailMessage::from_eml_file(path)?);
        Ok(())
    }
}

impl MailStore for Maildir {
    fn load(path: &Path) -> Result<Self, MailError> {
        let root = path.to_path_buf();
        if !Self::is_maildir(&root) {
            return Err(MailError::InvalidFormat(
                "not a maildir (missing new/, cur/, tmp/)".into(),
            ));
        }

        let mut messages = Vec::new();
        // Read from both `new/` and `cur/` — mirrors Python mailbox.Maildir.
        for subdir in &["new", "cur"] {
            let dir = root.join(subdir);
            if !dir.is_dir() {
                continue;
            }
            let mut entries: Vec<_> = fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .collect();
            // Sort by filename for deterministic order.
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                messages.push(MailMessage::from_eml_file(entry.path())?);
            }
        }

        Ok(Self { root, messages })
    }

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

    fn append_to(path: &Path, msg: &MailMessage) -> Result<(), MailError> {
        Self::ensure_dirs(path)?;
        // Count existing messages for index hint.
        let count = fs::read_dir(path.join("new"))
            .map(|rd| rd.count())
            .unwrap_or(0)
            + fs::read_dir(path.join("cur"))
                .map(|rd| rd.count())
                .unwrap_or(0);
        Self::write_message_atomic(path, msg, count)?;
        Ok(())
    }

    fn detect(path: &Path) -> bool {
        Self::is_maildir(path)
    }
}

impl Default for Maildir {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            messages: Vec::new(),
        }
    }
}

impl<'a> IntoIterator for &'a Maildir {
    type Item = &'a MailMessage;
    type IntoIter = std::slice::Iter<'a, MailMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

impl IntoIterator for Maildir {
    type Item = MailMessage;
    type IntoIter = std::vec::IntoIter<MailMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse patch counter and slug from subject like `[PATCH v3 2/4] Fix foo`.
/// Falls back to `index` if no counter is found.
fn parse_patch_subject(subject: &str, fallback_index: usize) -> (usize, String) {
    let mut counter = fallback_index;
    let mut clean = subject.to_string();

    // Try to extract counter from [PATCH ...N/M] prefix.
    if let Some(start) = subject.find('[') {
        if let Some(end) = subject[start..].find(']') {
            let bracket = &subject[start + 1..start + end];
            // Look for the "N/M" part.
            for token in bracket.split_whitespace() {
                if let Some(slash) = token.find('/') {
                    if let Ok(n) = token[..slash].parse::<usize>() {
                        counter = n;
                    }
                }
            }
            // Subject after the bracket.
            clean = subject[start + end + 1..].trim().to_string();
        }
    }

    // Slugify: non-word chars → underscore, collapse, lowercase.
    let slug: String = clean
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_ascii_lowercase();
    // Trim leading/trailing underscores and collapse runs.
    let slug = collapse_underscores(&slug);

    (counter, slug)
}

/// Collapse consecutive underscores and trim edges.
fn collapse_underscores(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = true; // trim leading
    for c in s.chars() {
        if c == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(c);
            prev_underscore = false;
        }
    }
    // Trim trailing underscore.
    if out.ends_with('_') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patch_subject_with_counter() {
        let (c, s) = parse_patch_subject("[PATCH v3 2/4] Fix null pointer", 0);
        assert_eq!(c, 2);
        assert_eq!(s, "fix_null_pointer");
    }

    #[test]
    fn test_parse_patch_subject_no_bracket() {
        let (c, s) = parse_patch_subject("Hello World!", 5);
        assert_eq!(c, 5);
        assert_eq!(s, "hello_world");
    }

    #[test]
    fn test_parse_patch_subject_cover_letter() {
        let (c, s) = parse_patch_subject("[PATCH 0/3] Series: my cool fix", 0);
        assert_eq!(c, 0);
        assert_eq!(s, "series_my_cool_fix");
    }

    #[test]
    fn test_collapse_underscores() {
        assert_eq!(collapse_underscores("__a__b__"), "a_b");
        assert_eq!(collapse_underscores("hello"), "hello");
        assert_eq!(collapse_underscores("___"), "");
    }
}
