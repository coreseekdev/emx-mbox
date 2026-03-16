use emx_mbox::{Mbox, MboxReader};

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

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
fn test_parse_three_messages_from_file() {
    let mbox = Mbox::load_file(fixture("three_messages.mbox")).unwrap();
    assert_eq!(mbox.len(), 3);
    assert_eq!(mbox.messages()[0].subject(), "First");
    assert_eq!(mbox.messages()[1].subject(), "Second");
    assert_eq!(mbox.messages()[2].subject(), "Third");
}

#[test]
fn test_parse_malformed_but_valid_no_blank_before_from() {
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
    assert!(
        !mbox.messages()[0].raw.windows(2).any(|w| w == b"\r\n"),
        "raw data should not contain CRLF"
    );
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
    );
    assert_eq!(msg.from(), "Display Name <display@example.com>");
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
    let subjects: Vec<String> = reader.map(|r| r.unwrap().subject()).collect();
    assert_eq!(subjects, vec!["First", "Second"]);
}

#[test]
fn test_iterator_from_file() {
    let reader = MboxReader::from_file(fixture("three_messages.mbox")).unwrap();
    let subjects: Vec<String> = reader.map(|r| r.unwrap().subject()).collect();
    assert_eq!(subjects, vec!["First", "Second", "Third"]);
}

// -----------------------------------------------------------------------
// Multiline body / blank lines
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
