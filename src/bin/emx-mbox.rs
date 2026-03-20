//! emx-mbox CLI tool - append-only mbox/maildir management
//!
//! Commands:
//!   list <path>           List messages in mbox or maildir
//!   add <path> <eml>...   Append EML file(s) to mbox or maildir
//!   del <path> <n>...     Mark messages for deletion (tombstone)

use clap::{Parser, Subcommand};
use emx_mbox::{deleted_message_ids, is_tombstone, MailStore, Maildir, Mbox, MboxWriter, MailMessage, MessageBuilder, TOMBSTONE_STATUS, X_LLM_STATUS};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "emx-mbox", about = "Append-only mbox/maildir management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List messages in mbox or maildir
    List {
        /// Path to mbox file or maildir directory
        path: PathBuf,
        /// Show verbose output (include Message-ID)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Append EML file(s) to mbox or maildir
    Add {
        /// Path to mbox file or maildir directory
        path: PathBuf,
        /// EML file(s) to append
        eml: Vec<PathBuf>,
    },
    /// Mark messages for deletion using tombstone
    Del {
        /// Path to mbox file or maildir directory
        path: PathBuf,
        /// Message numbers to delete (from list command, 1-based)
        #[arg(required = true)]
        indices: Vec<usize>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List { path, verbose } => cmd_list(&path, verbose),
        Commands::Add { path, eml } => cmd_add(&path, &eml),
        Commands::Del { path, indices } => cmd_del(&path, &indices),
    }
}

/// Detect storage format and list messages
fn cmd_list(path: &PathBuf, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    if Maildir::is_maildir(path) {
        let maildir = Maildir::load(path)?;
        list_messages(&maildir, verbose);
    } else if path.is_file() || !path.exists() {
        let mbox = Mbox::load_file(path)?;
        list_messages(&mbox, verbose);
    } else {
        eprintln!("Error: {} is not a valid mbox file or maildir", path.display());
        std::process::exit(1);
    }
    Ok(())
}

/// List messages with index (excludes deleted messages and tombstones)
fn list_messages(store: &impl MailStore, verbose: bool) {
    let all_messages: Vec<_> = store.messages().to_vec();
    let deleted_ids = deleted_message_ids(&all_messages);

    for (i, msg) in all_messages.iter().enumerate() {
        // Skip tombstone messages and messages marked as deleted
        if is_tombstone(msg)
            || msg
                .message_id()
                .map(|id| deleted_ids.contains(id))
                .unwrap_or(false)
        {
            continue;
        }

        let num = i + 1;
        if verbose {
            println!(
                "{:4} | {:40} | {:20} | {}",
                num,
                truncate(msg.from(), 40),
                truncate(msg.subject(), 20),
                msg.message_id().unwrap_or("")
            );
        } else {
            println!(
                "{:4} | {:40} | {}",
                num,
                truncate(msg.from(), 40),
                truncate(msg.subject(), 40)
            );
        }
    }
}

/// Append EML files to mbox or maildir
fn cmd_add(path: &PathBuf, eml_files: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    if eml_files.is_empty() {
        eprintln!("Error: no EML files specified");
        std::process::exit(1);
    }

    // Check if target is maildir
    if Maildir::is_maildir(path) {
        // For maildir, append each message individually
        for eml in eml_files {
            let msg = MailMessage::from_eml_file(eml)?;
            Maildir::append_to(path, &msg)?;
            println!("Added: {}", eml.display());
        }
    } else {
        // Append to mbox file
        for eml in eml_files {
            let msg = MailMessage::from_eml_file(eml)?;
            Mbox::append_to_file(path, &msg)?;
            println!("Added: {}", eml.display());
        }
    }
    Ok(())
}

/// Mark messages for deletion using tombstone
fn cmd_del(path: &PathBuf, indices: &[usize]) -> Result<(), Box<dyn std::error::Error>> {
    if indices.is_empty() {
        eprintln!("Error: no message indices specified");
        std::process::exit(1);
    }

    if Maildir::is_maildir(path) {
        // For maildir, we need to handle differently
        // For now, just error out - maildir uses file flags
        eprintln!("Error: del command not yet supported for maildir");
        std::process::exit(1);
    }

    // Load mbox to get message info
    let mbox = Mbox::load_file(path)?;
    let messages: Vec<_> = mbox.messages().to_vec();

    // Validate indices and collect messages to delete
    let mut to_delete = Vec::new();
    for &idx in indices {
        if idx == 0 || idx > messages.len() {
            eprintln!("Error: invalid message index {} (valid: 1-{})", idx, messages.len());
            std::process::exit(1);
        }
        to_delete.push(&messages[idx - 1]);
    }

    // Write tombstone messages
    let mut writer = MboxWriter::open_append(path)?;

    for (&idx, msg) in indices.iter().zip(to_delete.iter()) {
        let tombstone = create_tombstone(msg)?;
        writer.write_mail_message(&tombstone)?;
        println!(
            "Marked deleted: #{} (Message-ID: {})",
            idx,
            msg.message_id().unwrap_or("N/A")
        );
    }

    Ok(())
}

/// Create a tombstone message for deletion
///
/// The tombstone message:
/// - Uses the same From (envelope sender) as the original
/// - References the original Message-ID
/// - Includes X-LLM-Status: deleted header
fn create_tombstone(original: &MailMessage) -> Result<MailMessage, Box<dyn std::error::Error>> {
    let original_msg_id = original.message_id().unwrap_or("unknown");
    let original_from = original.from();

    // Build tombstone message
    let tombstone = MessageBuilder::new(
        original_from,
        "", // Empty subject for tombstone
    )
    .message_id(format!("<tombstone.{}>", original_msg_id.trim_matches(|c| c == '<' || c == '>')))
    .extra_header(X_LLM_STATUS, TOMBSTONE_STATUS)
    .extra_header("X-LLM-Deleted-Message-ID", original_msg_id)
    .body("") // Empty body
    .build();

    Ok(tombstone)
}

/// Truncate a string to max length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    let prefix: String = s.chars().take(max).collect();
    if prefix.chars().count() < max {
        return prefix;
    }

    if s.chars().nth(max).is_none() {
        return prefix;
    }

    if max <= 3 {
        return prefix;
    }

    let truncated: String = prefix.chars().take(max - 3).collect();
    format!("{}...", truncated)
}
