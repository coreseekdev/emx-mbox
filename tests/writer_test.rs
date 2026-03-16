use chrono::{TimeZone, Utc};
use emx_mbox::{Mbox, MboxWriter, MailMessage, MessageBuilder, MboxFormat};

// -----------------------------------------------------------------------
// Writer format
// -----------------------------------------------------------------------

#[test]
fn test_writer_mboxrd_format() {
    let msg = MailMessage::from_raw(
        b"From: test@example.com\nSubject: Hi\nDate: Mon, 16 Mar 2026 10:00:00 +0000\n\nBody\n"
            .to_vec(),
    );

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf).with_format(MboxFormat::Mboxrd);
        writer.write_mail_message(&msg).unwrap();
    }

    let output = String::from_utf8(buf).unwrap();
    assert!(output.starts_with("From "), "output = {}", output);
    assert!(output.ends_with("\n\n"), "output = {:?}", output);
}

#[test]
fn test_writer_b4_compatible_separator() {
    let date = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
    let msg_raw = b"From: dev@kernel.org\nSubject: [PATCH] fix\nDate: Thu, 01 Jan 1970 00:00:00 +0000\nMessage-ID: <test@example.com>\n\nPatch body\n";

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        writer.write_message("mboxrd@z", &date, msg_raw).unwrap();
    }

    let output = String::from_utf8(buf).unwrap();
    assert!(
        output.starts_with("From mboxrd@z Thu Jan  1 00:00:00 1970"),
        "output = {}",
        output
    );
}

// -----------------------------------------------------------------------
// Round-trip: save and reload
// -----------------------------------------------------------------------

#[test]
fn test_save_and_load_roundtrip() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Test\n\
Date: Mon, 16 Mar 2026 10:00:00 +0000\n\
\n\
Body content\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.save(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded.messages()[0].subject(), "Test");
    assert_eq!(reloaded.messages()[0].body().trim(), "Body content");
}

#[test]
fn test_from_escape_roundtrip() {
    let msg = MessageBuilder::new("alice@example.com", "Test")
        .body("Hello\nFrom someone in the body\nEnd\n")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.save(&mut writer).unwrap();
    }
    let saved = String::from_utf8_lossy(&buf);
    assert!(
        saved.contains(">From someone in the body"),
        "saved should escape 'From ': {}",
        saved
    );

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);
    let body = reloaded.messages()[0].body();
    assert!(
        body.contains("From someone in the body"),
        "reloaded body should unescape: {:?}",
        body
    );
}

#[test]
fn test_envelope_from_roundtrip() {
    let mbox_data = "From custom@kernel.org Thu Jan  1 00:00:00 1970\n\
From: Display <display@ex.com>\n\
Subject: Test\n\
Date: Thu, 01 Jan 1970 00:00:00 +0000\n\
\n\
Body\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(
        mbox.messages()[0].envelope_from.as_deref(),
        Some("custom@kernel.org")
    );

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.save(&mut writer).unwrap();
    }
    let saved = String::from_utf8_lossy(&buf);
    assert!(
        saved.starts_with("From custom@kernel.org "),
        "writer should use envelope_from: {}",
        saved
    );

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(
        reloaded.messages()[0].envelope_from.as_deref(),
        Some("custom@kernel.org")
    );
}
