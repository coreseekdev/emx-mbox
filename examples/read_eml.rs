//! Read a single .eml file and inspect its headers, body, and attachments.
//!
//! Usage:
//!   cargo run --example read_eml -- tests/fixtures/sample.eml

use emx_mbox::MailMessage;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: read_eml <eml-file>");
        std::process::exit(1);
    });

    let msg = MailMessage::from_eml_file(&path)?;

    println!("From:       {}", msg.from());
    println!("Subject:    {}", msg.subject());
    println!("Message-ID: {}", msg.message_id().unwrap_or("-"));
    if let Some(date) = msg.date() {
        println!("Date:       {}", date);
    }
    println!();

    // Show all values of multi-valued headers
    let to_values = msg.headers_all("To");
    if !to_values.is_empty() {
        println!("To ({} value(s)):", to_values.len());
        for v in to_values {
            println!("  {}", v);
        }
    }

    // Attachments
    let attachments = msg.attachments();
    if attachments.is_empty() {
        println!("\nNo attachments.");
    } else {
        println!("\nAttachments ({}):", attachments.len());
        for att in &attachments {
            println!("  {} ({}, {} bytes)", att.filename, att.content_type, att.data.len());
        }
    }

    println!("\n--- Body ---");
    println!("{}", msg.body());

    Ok(())
}
