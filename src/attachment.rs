use std::fs;
use std::path::Component;
use std::path::Path;

use crate::error::MailError;

/// A file attachment with content and metadata.
pub struct Attachment {
    /// The filename (e.g. `image.png`).
    pub filename: String,
    /// MIME type (e.g. `image/png`). Auto-detected if not specified.
    pub content_type: String,
    /// Raw file content.
    pub data: Vec<u8>,
}

impl Attachment {
    /// Create an attachment from raw bytes.
    pub fn new(filename: impl Into<String>, content_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            data,
        }
    }

    /// Create an attachment by reading a file from disk.
    ///
    /// # Security
    ///
    /// This method treats `path` as trusted input and performs no path
    /// traversal checks. For untrusted input, prefer `from_file_in_root`.
    /// MIME type is guessed from the file extension.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MailError> {
        Self::from_file_trusted(path)
    }

    /// Create an attachment by reading a trusted file path from disk.
    /// MIME type is guessed from the file extension.
    pub fn from_file_trusted<P: AsRef<Path>>(path: P) -> Result<Self, MailError> {
        let path = path.as_ref();
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "attachment".into());
        let content_type = guess_mime(&filename);
        let data = fs::read(path)?;
        Ok(Self {
            filename,
            content_type,
            data,
        })
    }

    /// Create an attachment from a path constrained under `root`.
    ///
    /// Rejects absolute paths and parent-directory (`..`) traversal.
    pub fn from_file_in_root<P: AsRef<Path>, Q: AsRef<Path>>(
        root: P,
        relative_path: Q,
    ) -> Result<Self, MailError> {
        let relative_path = relative_path.as_ref();
        if relative_path.is_absolute() {
            return Err(MailError::InvalidFormat(
                "attachment path must be relative".into(),
            ));
        }

        if relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(MailError::InvalidFormat(
                "attachment path traversal is not allowed".into(),
            ));
        }

        let root = fs::canonicalize(root.as_ref())?;
        let candidate = root.join(relative_path);
        let candidate = fs::canonicalize(candidate)?;

        if !candidate.starts_with(&root) {
            return Err(MailError::InvalidFormat(
                "attachment path escapes root directory".into(),
            ));
        }

        Self::from_file_trusted(candidate)
    }
}

/// Guess MIME type from file extension.
pub(crate) fn guess_mime(filename: &str) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "patch" | "diff" => "text/x-patch",
        _ => "application/octet-stream",
    }
    .into()
}
