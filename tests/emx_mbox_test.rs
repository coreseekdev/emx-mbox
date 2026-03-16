use chrono::{TimeZone, Utc};
use emx_mbox::{Mbox, MboxWriter, MailMessage, MessageBuilder, MboxFormat};

// -----------------------------------------------------------------------
// Basic parsing
// -----------------------------------------------------------------------

#[test]
fn test_parse_single_message() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
To: bob@example.com\n\
Subject: Hello World\n\
\n\
Test body\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(mbox.messages().len(), 1);

    let msg = &mbox.messages()[0];
    assert_eq!(msg.from(), "alice@example.com");
    assert_eq!(msg.subject(), "Hello World");
    assert_eq!(msg.body().trim(), "Test body");
}

#[test]
fn test_parse_three_messages() {
    let mbox_data = "\
From herp@example.com Thu Jan  1 00:00:01 2015\n\
From: herp@example.com\n\
Subject: First\n\
\n\
Body 1\n\
\n\
From derp@example.com Thu Jan  2 00:00:01 2015\n\
From: derp@example.com\n\
Subject: Second\n\
\n\
Body 2\n\
\n\
From bernd@example.com Thu Jan  3 00:00:01 2015\n\
From: bernd@example.com\n\
Subject: Third\n\
\n\
Body 3\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(mbox.len(), 3);
    assert_eq!(mbox.messages()[0].subject(), "First");
    assert_eq!(mbox.messages()[1].subject(), "Second");
    assert_eq!(mbox.messages()[2].subject(), "Third");
}

#[test]
fn test_parse_malformed_but_valid_no_blank_before_from() {
    // Go-mbox test: message not separated by blank line is still valid
    let mbox_data = "\
From herp@example.com Thu Jan  1 00:00:01 2015\n\
From: herp@example.com\n\
Subject: First\n\
\n\
Body 1\n\
From derp@example.com Thu Jan  2 00:00:01 2015\n\
From: derp@example.com\n\
Subject: Second\n\
\n\
Body 2\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(mbox.len(), 2);
    assert_eq!(mbox.messages()[0].subject(), "First");
    assert_eq!(mbox.messages()[1].subject(), "Second");
}

#[test]
fn test_parse_with_leading_blank_lines() {
    let mbox_data = "\n\n\
From herp@example.com Thu Jan  1 00:00:01 2015\n\
From: herp@example.com\n\
Subject: Test\n\
\n\
Body\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(mbox.len(), 1);
}

#[test]
fn test_empty_mbox() {
    let mbox = Mbox::load("".as_bytes()).unwrap();
    assert!(mbox.is_empty());
}

// -----------------------------------------------------------------------
// From-line escaping / unescaping
// -----------------------------------------------------------------------

#[test]
fn test_escaped_from_in_body() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Test\n\
\n\
Some text\n\
>From this was escaped\n\
More text\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    let body = mbox.messages()[0].body();
    assert!(body.contains("Some text"));
    assert!(body.contains("From this was escaped"), "body = {:?}", body);
    assert!(body.contains("More text"));
}

#[test]
fn test_mboxrd_multilevel_from_escape() {
    // In mboxrd, `>>From` in storage means the original had `>From`
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Test\n\
\n\
Some text\n\
>>From doubly escaped\n\
More text\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    let body = mbox.messages()[0].body();
    assert!(
        body.contains(">From doubly escaped"),
        "should unescape one level: body = {:?}",
        body
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
    // Build a message whose body contains a bare "From " line.
    // Use MessageBuilder to create it properly, then save → escaping → reload.
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
    // The writer should have escaped "From someone"
    assert!(
        saved.contains(">From someone in the body"),
        "saved should escape 'From ': {}",
        saved
    );

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1, "should still be one message");
    let body = reloaded.messages()[0].body();
    assert!(
        body.contains("From someone in the body"),
        "reloaded body should unescape: {:?}",
        body
    );
}

// -----------------------------------------------------------------------
// add_message / MessageBuilder
// -----------------------------------------------------------------------

#[test]
fn test_add_message_legacy() {
    let mut mbox = Mbox::new();
    mbox.add_message("bob@example.com", "New Message", "New body");

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].from(), "bob@example.com");
    assert_eq!(mbox.messages()[0].subject(), "New Message");
}

#[test]
fn test_message_builder_basic() {
    let msg = MessageBuilder::new("Alice <alice@example.com>", "Hello World")
        .to("Bob <bob@example.com>")
        .body("This is the body.\n")
        .build();

    assert_eq!(msg.from(), "Alice <alice@example.com>");
    assert_eq!(msg.subject(), "Hello World");
    assert!(msg.message_id().is_some());
    assert!(msg.header("Date").is_some());
    assert_eq!(msg.header("MIME-Version").unwrap(), "1.0");
    assert!(msg
        .header("Content-Type")
        .unwrap()
        .contains("text/plain"));
}

#[test]
fn test_message_builder_patch_subject() {
    let msg = MessageBuilder::new("dev@kernel.org", "Fix null pointer dereference")
        .patch_subject(Some("PATCH"), Some(2), 1, 3)
        .body("commit body\n\nSigned-off-by: dev@kernel.org\n")
        .build();

    assert_eq!(msg.subject(), "[PATCH v2 1/3] Fix null pointer dereference");
}

#[test]
fn test_message_builder_threading() {
    let cover_id = "<cover-v1@example.com>";
    let msg = MessageBuilder::new("dev@kernel.org", "Patch description")
        .in_reply_to(cover_id)
        .reference(cover_id)
        .build();

    assert_eq!(
        msg.header("In-Reply-To").unwrap(),
        cover_id
    );
    assert!(msg.header("References").unwrap().contains(cover_id));
}

// -----------------------------------------------------------------------
// EML file append
// -----------------------------------------------------------------------

#[test]
fn test_append_raw() {
    let mut mbox = Mbox::new();
    let eml = b"From: test@example.com\nSubject: Raw\n\nRaw body\n";
    mbox.append_raw(eml.to_vec());

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "Raw");
}

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
    // Should start with "From " separator
    assert!(output.starts_with("From "), "output = {}", output);
    // Should end with a blank line
    assert!(output.ends_with("\n\n"), "output = {:?}", output);
}

#[test]
fn test_writer_b4_compatible_separator() {
    let date = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
    let msg_raw = b"From: dev@kernel.org\nSubject: [PATCH] fix\nDate: Thu, 01 Jan 1970 00:00:00 +0000\nMessage-ID: <test@example.com>\n\nPatch body\n";

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        writer
            .write_message("mboxrd@z", &date, msg_raw)
            .unwrap();
    }

    let output = String::from_utf8(buf).unwrap();
    assert!(
        output.starts_with("From mboxrd@z Thu Jan  1 00:00:00 1970"),
        "output = {}",
        output
    );
}

// -----------------------------------------------------------------------
// Multiline body
// -----------------------------------------------------------------------

#[test]
fn test_multiline_body() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Multiline\n\
\n\
Line 1\n\
Line 2\n\
Line 3\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    let body = mbox.messages()[0].body();
    assert!(body.contains("Line 1"));
    assert!(body.contains("Line 2"));
    assert!(body.contains("Line 3"));
}

// -----------------------------------------------------------------------
// Body with blank lines preserved
// -----------------------------------------------------------------------

#[test]
fn test_body_blank_lines_preserved() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Blanks\n\
\n\
Para 1\n\
\n\
Para 2\n\
\n\
Para 3\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    let body = mbox.messages()[0].body();
    assert!(body.contains("Para 1"));
    assert!(body.contains("Para 2"));
    assert!(body.contains("Para 3"));
}
