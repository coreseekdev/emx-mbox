//! Auto-detect and open any mail store (mbox or Maildir).
//!
//! Demonstrates the `emx_mbox::open()` function which mirrors b4's
//! auto-detection logic: try Maildir (check for new/cur/tmp dirs),
//! then fall back to mbox (check for "From " prefix).
//!
//! Usage:
//!   cargo run --example open_any -- tests/fixtures/three_messages.mbox
//!   cargo run --example open_any -- /tmp/test.maildir

use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: open_any <mbox-or-maildir-path>");
        std::process::exit(1);
    });

    let store = emx_mbox::open(Path::new(&path))?;
    println!("Opened {} with {} message(s)\n", path, store.len());

    for (i, msg) in store.messages().iter().enumerate() {
        println!("Message {}:", i + 1);
        println!("  From:    {}", msg.from());
        println!("  Subject: {}", msg.subject());
        if let Some(id) = msg.message_id() {
            println!("  ID:      {}", id);
        }
        println!();
    }

    Ok(())
}
