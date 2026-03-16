//! Read and iterate over messages in an mbox file.
//!
//! Usage:
//!   cargo run --example read_mbox -- tests/fixtures/three_messages.mbox

use emx_mbox::{MailStore, Mbox, MboxReader};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: read_mbox <mbox-file>");
        std::process::exit(1);
    });

    // ── Method 1: Load entire mbox into memory ──────────────────────────
    println!("=== Load all at once ===");
    let mbox = Mbox::load_file(&path)?;
    println!("Loaded {} message(s)\n", mbox.len());

    for (i, msg) in mbox.iter().enumerate() {
        println!("--- Message {} ---", i + 1);
        println!("  Envelope-From: {}", msg.envelope_from().unwrap_or("-"));
        println!("  From:       {}", msg.from());
        println!("  Subject:    {}", msg.subject());
        println!("  Message-ID: {}", msg.message_id().unwrap_or("-"));
        if let Some(date) = msg.date() {
            println!("  Date:       {}", date);
        }
        println!("  Body:       {}…", &msg.body().trim().chars().take(60).collect::<String>());
        println!();
    }

    // ── Method 2: Streaming iterator (like go-mbox's NextMessage) ───────
    println!("=== Streaming iterator ===");
    let reader = MboxReader::from_file(&path)?;
    let subjects: Vec<String> = reader.map(|r| r.unwrap().subject().to_owned()).collect();
    println!("Subjects: {:?}", subjects);

    Ok(())
}
