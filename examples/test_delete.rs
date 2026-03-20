//! Create a test mbox file for deletion testing

use emx_mbox::{MboxFormat, MboxWriter};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dest = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: test_delete <output-file>");
        std::process::exit(1);
    });

    // Create sample messages with unique Message-IDs
    let msgs = vec![
        (
            "alice@example.com",
            "First patch: Fix memory leak",
            "20250319100001.abc1@example.com",
            "This is the first patch.\n\nSigned-off-by: Alice <alice@example.com>",
        ),
        (
            "bob@example.com",
            "Second patch: Add feature X",
            "20250319100002.def2@example.com",
            "This is the second patch.\n\nSigned-off-by: Bob <bob@example.com>",
        ),
        (
            "charlie@example.com",
            "Third patch: Refactor code",
            "20250319100003.ghi3@example.com",
            "This is the third patch.\n\nSigned-off-by: Charlie <charlie@example.com>",
        ),
        (
            "dave@example.com",
            "Fourth patch: Update docs",
            "20250319100004.jkl4@example.com",
            "This is the fourth patch.\n\nSigned-off-by: Dave <dave@example.com>",
        ),
        (
            "eve@example.com",
            "Fifth patch: Fix bug Y",
            "20250319100005.mno5@example.com",
            "This is the fifth patch.\n\nSigned-off-by: Eve <eve@example.com>",
        ),
    ];

    // Write to mbox file
    let mut writer = MboxWriter::from_file(&dest)?.with_format(MboxFormat::Mboxrd);

    for (from, subject, message_id, body) in &msgs {
        let raw = format!(
            "From: {}\n\
             Subject: {}\n\
             Date: Mon, 16 Mar 2026 10:00:00 +0000\n\
             Message-ID: <{}>\n\
             MIME-Version: 1.0\n\
             Content-Type: text/plain; charset=utf-8\n\
             \n\
             {}",
            from, subject, message_id, body
        );

        writer.write_message(from, &chrono::Utc::now(), raw.as_bytes())?;
        println!("Added: {} (Message-ID: <{}>)", subject, message_id);
    }

    println!("\nCreated test mbox file: {}", dest);
    println!("Total messages: {}", msgs.len());

    Ok(())
}
