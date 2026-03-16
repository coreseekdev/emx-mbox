use thiserror::Error;

#[derive(Error, Debug)]
pub enum MboxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] mailparse::MailParseError),
    #[error("Invalid mbox format: {0}")]
    InvalidFormat(String),
}
