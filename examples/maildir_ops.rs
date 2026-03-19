//! Create and manipulate a Maildir, convert between mbox and Maildir.
//!
//! Mirrors b4's save_maildir() which creates new/cur/tmp structure and
//! writes messages atomically.
//!
//! Usage:
//!   cargo run --example maildir_ops -- /tmp/test.maildir

use emx_mbox::{MailStore, Maildir, Mbox, MessageBuilder};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: maildir_ops <maildir-path>");
        std::process::exit(1);
    });

    // ── 1. Create a Maildir with messages ───────────────────────────────
    println!("=== Creating Maildir at {} ===", dir);

    let msg1 = MessageBuilder::new("alice@example.com", "[PATCH 1/2] Fix memory leak")
        .to("linux-kernel@vger.kernel.org")
        .body("Fix a memory leak in driver init.\n")
        .signed_off_by("Alice <alice@example.com>")
        .build();

    let msg2 = MessageBuilder::new("alice@example.com", "[PATCH 2/2] Add error handling")
        .to("linux-kernel@vger.kernel.org")
        .body("Add proper error handling to probe().\n")
        .signed_off_by("Alice <alice@example.com>")
        .build();

    Maildir::append_to(std::path::Path::new(&dir), &msg1)?;
    Maildir::append_to(std::path::Path::new(&dir), &msg2)?;
    println!("Saved 2 messages");

    // ── 2. Auto-detect and open ─────────────────────────────────────────
    println!("\n=== Auto-detect format ===");
    println!("Is maildir? {}", Maildir::is_maildir(&dir));
    let store = emx_mbox::open(std::path::Path::new(&dir))?;
    println!("Opened store with {} message(s)", store.len());
    for msg in store.messages() {
        println!("  {}: {}", msg.from(), msg.subject());
    }

    // ── 3. Convert mbox → Maildir ──────────────────────────────────────
    println!("\n=== Convert mbox → Maildir ===");
    let mbox_data = "\
From herp@example.com Thu Jan  1 00:00:01 2015\n\
From: herp@example.com\n\
Subject: Message from mbox\n\
Date: Thu, 01 Jan 2015 00:00:01 +0000\n\
\n\
Body from mbox file.\n";

    let mbox = Mbox::load(mbox_data.as_bytes())?;
    let dir2 = format!("{}_converted", dir);
    for msg in mbox.iter() {
        Maildir::append_to(std::path::Path::new(&dir2), msg)?;
    }
    println!("Converted {} mbox messages → Maildir at {}", mbox.len(), dir2);

    Ok(())
}
