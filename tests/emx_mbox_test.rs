use chrono::{TimeZone, Utc};
use emx_mbox::{Attachment, Mbox, MboxReader, MboxWriter, MailMessage, MessageBuilder, MboxFormat};

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

// -----------------------------------------------------------------------
// Envelope From preservation
// -----------------------------------------------------------------------

#[test]
fn test_envelope_from_preserved() {
    let mbox_data = "From custom-sender@kernel.org Thu Jan  1 00:00:00 1970\n\
From: Display Name <display@example.com>\n\
Subject: Test\n\
\n\
Body\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    let msg = &mbox.messages()[0];
    assert_eq!(
        msg.envelope_from.as_deref(),
        Some("custom-sender@kernel.org"),
        "envelope_from should be preserved from the From line"
    );
    // The header From is different
    assert_eq!(msg.from(), "Display Name <display@example.com>");
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
    assert_eq!(mbox.messages()[0].envelope_from.as_deref(), Some("custom@kernel.org"));

    // Save and reload — writer should use envelope_from
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

// -----------------------------------------------------------------------
// CRLF input handling
// -----------------------------------------------------------------------

#[test]
fn test_crlf_input() {
    let mbox_data = "From alice@example.com Mon Mar 16 10:00:00 2026\r\n\
From: alice@example.com\r\n\
Subject: CRLF Test\r\n\
\r\n\
Body with CRLF\r\n";

    let mbox = Mbox::load(mbox_data.as_bytes()).unwrap();
    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "CRLF Test");
    let body = mbox.messages()[0].body();
    assert!(body.contains("Body with CRLF"));
    // Internal storage should be LF-only
    assert!(
        !mbox.messages()[0].raw.windows(2).any(|w| w == b"\r\n"),
        "raw data should not contain CRLF"
    );
}

// -----------------------------------------------------------------------
// Iterator interface
// -----------------------------------------------------------------------

#[test]
fn test_iterator_interface() {
    let mbox_data = "\
From a@example.com Thu Jan  1 00:00:00 1970\n\
From: a@example.com\n\
Subject: First\n\
\n\
Body 1\n\
\n\
From b@example.com Thu Jan  2 00:00:00 1970\n\
From: b@example.com\n\
Subject: Second\n\
\n\
Body 2\n";

    let reader = MboxReader::new(mbox_data.as_bytes());
    let subjects: Vec<String> = reader
        .map(|r| r.unwrap().subject())
        .collect();
    assert_eq!(subjects, vec!["First", "Second"]);
}

// -----------------------------------------------------------------------
// Multi-message roundtrip
// -----------------------------------------------------------------------

#[test]
fn test_multi_message_roundtrip() {
    let mut mbox = Mbox::new();
    for i in 0..5 {
        let msg = MessageBuilder::new(
            format!("user{}@example.com", i),
            format!("Message {}", i),
        )
        .body(format!("Body of message {}\n", i))
        .build();
        mbox.append(msg);
    }

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.save(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 5);
    for i in 0..5 {
        assert_eq!(
            reloaded.messages()[i].subject(),
            format!("Message {}", i)
        );
        assert!(reloaded.messages()[i]
            .body()
            .contains(&format!("Body of message {}", i)));
    }
}

// -----------------------------------------------------------------------
// Builder trailers
// -----------------------------------------------------------------------

#[test]
fn test_builder_trailers() {
    let msg = MessageBuilder::new("dev@kernel.org", "[PATCH] Fix something")
        .body("Patch description\n\n---\ndiff goes here\n")
        .signed_off_by("Dev <dev@kernel.org>")
        .reviewed_by("Reviewer <rev@kernel.org>")
        .acked_by("Acker <ack@kernel.org>")
        .build();

    let body = String::from_utf8_lossy(&msg.raw);
    assert!(body.contains("Signed-off-by: Dev <dev@kernel.org>"), "body = {}", body);
    assert!(body.contains("Reviewed-by: Reviewer <rev@kernel.org>"), "body = {}", body);
    assert!(body.contains("Acked-by: Acker <ack@kernel.org>"), "body = {}", body);
}

#[test]
fn test_builder_custom_trailer() {
    let msg = MessageBuilder::new("dev@kernel.org", "Test")
        .body("Body\n")
        .trailer("Tested-by", "QA <qa@example.com>")
        .build();

    let body = String::from_utf8_lossy(&msg.raw);
    assert!(body.contains("Tested-by: QA <qa@example.com>"), "body = {}", body);
}

// -----------------------------------------------------------------------
// Attachments
// -----------------------------------------------------------------------

#[test]
fn test_single_attachment() {
    let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
    let msg = MessageBuilder::new("alice@example.com", "Screenshot")
        .body("See attached.\n")
        .attach("screenshot.png", "image/png", png_data.clone())
        .build();

    // Should be multipart/mixed
    let ct = msg.header("Content-Type").unwrap();
    assert!(ct.contains("multipart/mixed"), "Content-Type = {}", ct);

    // Should contain the text body
    let raw = String::from_utf8_lossy(&msg.raw);
    assert!(raw.contains("See attached."), "raw = {}", raw);

    // Should contain the attachment filename
    assert!(raw.contains("screenshot.png"), "raw = {}", raw);

    // Parse attachments back
    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1, "should have 1 attachment");
    assert_eq!(attachments[0].filename, "screenshot.png");
    assert_eq!(attachments[0].content_type, "image/png");
    assert_eq!(attachments[0].data, png_data);
}

#[test]
fn test_multiple_attachments() {
    let png_data = vec![0x89, 0x50, 0x4E, 0x47]; // fake PNG
    let svg_data = b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".to_vec();

    let msg = MessageBuilder::new("alice@example.com", "Designs")
        .body("Two attachments.\n")
        .attach("logo.png", "image/png", png_data.clone())
        .attach("icon.svg", "image/svg+xml", svg_data.clone())
        .build();

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 2, "should have 2 attachments");
    assert_eq!(attachments[0].filename, "logo.png");
    assert_eq!(attachments[0].data, png_data);
    assert_eq!(attachments[1].filename, "icon.svg");
    assert_eq!(attachments[1].data, svg_data);
}

#[test]
fn test_attachment_roundtrip_through_mbox() {
    let file_data = b"Hello, this is a text file attachment.".to_vec();

    let msg = MessageBuilder::new("sender@example.com", "With attachment")
        .body("Please see the attached file.\n")
        .attach("notes.txt", "text/plain", file_data.clone())
        .build();

    // Save to mbox and reload
    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.save(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);

    let loaded_msg = &reloaded.messages()[0];
    assert_eq!(loaded_msg.subject(), "With attachment");

    let attachments = loaded_msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "notes.txt");
    assert_eq!(attachments[0].data, file_data);
}

#[test]
fn test_no_attachments_still_plain() {
    // Without attachments, Content-Type should be text/plain
    let msg = MessageBuilder::new("alice@example.com", "Plain")
        .body("Just text.\n")
        .build();

    let ct = msg.header("Content-Type").unwrap();
    assert!(ct.contains("text/plain"), "Content-Type = {}", ct);
    assert!(msg.attachments().is_empty());
}

#[test]
fn test_attachment_with_trailers() {
    let data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // fake JPEG header

    let msg = MessageBuilder::new("dev@kernel.org", "[PATCH] Add icon")
        .body("Patch body\n")
        .attach("icon.jpg", "image/jpeg", data.clone())
        .signed_off_by("Dev <dev@kernel.org>")
        .build();

    let raw = String::from_utf8_lossy(&msg.raw);
    // Trailers should be in the text part
    assert!(raw.contains("Signed-off-by: Dev <dev@kernel.org>"), "raw = {}", raw);

    // Attachment should still be parseable
    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "icon.jpg");
    assert_eq!(attachments[0].data, data);
}

#[test]
fn test_attachment_from_struct() {
    let att = Attachment::new("report.pdf", "application/pdf", vec![0x25, 0x50, 0x44, 0x46]);
    assert_eq!(att.filename, "report.pdf");
    assert_eq!(att.content_type, "application/pdf");
    assert_eq!(att.data.len(), 4);
}
