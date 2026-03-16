use emx_mbox::{Mbox, MboxWriter, MessageBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sample_email = concat!(
        "From: alice@example.com\n",
        "To: bob@example.com\n",
        "Subject: Hello World\n",
        "Date: Mon, 16 Mar 2026 10:00:00 +0000\n",
        "Message-ID: <hello-world@example.com>\n",
        "MIME-Version: 1.0\n",
        "Content-Type: text/plain; charset=utf-8\n",
        "\n",
        "This is a test message.\n",
    );

    let mbox_data = format!("From alice@example.com Mon Mar 16 10:00:00 2026\n{}", sample_email);

    let mbox = Mbox::load(mbox_data.as_bytes())?;
    println!("Loaded {} message(s)", mbox.len());

    for (i, msg) in mbox.iter().enumerate() {
        println!("\n--- Message {} ---", i + 1);
        println!("From: {}", msg.from());
        println!("Subject: {}", msg.subject());
        println!("Message-ID: {}", msg.message_id().unwrap_or_default());
        println!("Body: {}", msg.body());
    }

    // Build a new message using MessageBuilder (b4-compatible)
    let mut mbox2 = Mbox::load(mbox_data.as_bytes())?;
    let new_msg = MessageBuilder::new(
        "Charlie <charlie@example.com>",
        "Fix null pointer dereference",
    )
    .to("linux-kernel@vger.kernel.org")
    .patch_subject(Some("PATCH"), Some(1), 1, 1)
    .body("commit body\n\nSigned-off-by: Charlie <charlie@example.com>\n")
    .build();
    mbox2.append(new_msg);

    // Also append a raw EML
    mbox2.append_raw(
        b"From: dave@example.com\nSubject: Raw append\nDate: Mon, 16 Mar 2026 12:00:00 +0000\nMessage-ID: <raw@example.com>\n\nRaw body\n"
            .to_vec(),
    );

    let mut writer = Vec::new();
    let mut mbox_writer = MboxWriter::new(&mut writer);
    mbox2.save(&mut mbox_writer)?;

    println!("\n--- Re-saved mbox ({} messages) ---", mbox2.len());
    println!("{}", String::from_utf8_lossy(&writer));

    Ok(())
}
