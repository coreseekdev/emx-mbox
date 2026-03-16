//! Write messages to an mbox file (mboxrd format, b4-compatible).
//!
//! Demonstrates the writer API that mirrors go-mbox's Writer and b4's
//! save_mboxrd_mbox().
//!
//! Usage:
//!   cargo run --example write_mbox -- /tmp/output.mbox

use chrono::{TimeZone, Utc};
use emx_mbox::{MailMessage, MailStore, Mbox, MboxWriter};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dest = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: write_mbox <output-file>");
        std::process::exit(1);
    });

    // ── Write raw messages via MboxWriter ───────────────────────────────
    // This is the low-level API, analogous to go-mbox's
    //   w.CreateMessage(from, t)  → io.Writer
    // and b4's save_mboxrd_mbox() which writes:
    //   "From mboxrd@z Thu Jan  1 00:00:00 1970\n" + escaped body

    let date = Utc.with_ymd_and_hms(2026, 3, 16, 10, 0, 0).unwrap();

    let raw_msg1 = b"From: alice@example.com\n\
To: bob@example.com\n\
Subject: Hello from emx-mbox\n\
Date: Mon, 16 Mar 2026 10:00:00 +0000\n\
Message-ID: <hello@example.com>\n\
MIME-Version: 1.0\n\
Content-Type: text/plain; charset=utf-8\n\
\n\
This is the first message.\n\
\n\
From the second paragraph, note this line starts with \"From \".\n\
The writer will escape it to \">From \" in the mbox file.\n";

    let raw_msg2 = b"From: charlie@example.com\n\
Subject: Second message\n\
Date: Mon, 16 Mar 2026 11:00:00 +0000\n\
Message-ID: <second@example.com>\n\
\n\
Body of the second message.\n";

    let mut writer = MboxWriter::from_file(&dest)?;
    writer.write_message("alice@example.com", &date, raw_msg1)?;
    writer.write_message("charlie@example.com", &date, raw_msg2)?;
    println!("Wrote 2 messages to {}", dest);

    // ── Verify by re-reading ────────────────────────────────────────────
    let mbox = Mbox::load_file(&dest)?;
    println!("Re-read {} message(s):", mbox.len());
    for msg in mbox.iter() {
        println!("  {}: {}", msg.from(), msg.subject());
    }

    // ── High-level: Mbox.save_file() ───────────────────────────────────
    println!("\n--- High-level Mbox.save_file() ---");
    let mut mbox = Mbox::new();
    mbox.append(MailMessage::from_raw(raw_msg1.to_vec()));
    mbox.append(MailMessage::from_raw(raw_msg2.to_vec()));
    let dest2 = format!("{}.2", dest);
    mbox.save_file(&dest2)?;
    println!("Saved {} messages to {}", mbox.len(), dest2);

    Ok(())
}
