use thiserror::Error;

#[derive(Error, Debug)]
pub enum MailError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] mailparse::MailParseError),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}
