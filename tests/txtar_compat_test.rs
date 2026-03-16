/// Tests for txtar-format content compatibility with mbox escaping rules.
///
/// txtar (Go testscript archive) uses `-- filename --` delimiters.
/// mbox only escapes lines starting with `From ` (mboxo) or `>*From ` (mboxrd).
/// These are completely orthogonal — but we verify edge cases here.
use emx_mbox::{MailStore, Mbox, MboxWriter, MessageBuilder, MboxFormat};

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// -----------------------------------------------------------------------
// txtar delimiters survive mbox roundtrip
// -----------------------------------------------------------------------

#[test]
fn test_txtar_delimiters_preserved_in_body() {
    let txtar_body = "\
-- hello.go --\n\
package main\n\
\n\
func main() {}\n\
-- world.txt --\n\
Hello, world!\n";

    let msg = MessageBuilder::new("dev@example.com", "txtar archive")
        .body(txtar_body)
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);

    let body = reloaded.messages()[0].body();
    assert!(body.contains("-- hello.go --"), "delimiter lost: {}", body);
    assert!(body.contains("-- world.txt --"), "delimiter lost: {}", body);
    assert!(body.contains("package main"), "content lost: {}", body);
    assert!(body.contains("Hello, world!"), "content lost: {}", body);
}

// -----------------------------------------------------------------------
// txtar content with `From ` lines → mbox escapes transparently
// -----------------------------------------------------------------------

#[test]
fn test_txtar_from_lines_in_content_roundtrip() {
    // A txtar file entry whose content starts with "From " at line beginning.
    let txtar_body = "\
-- message.txt --\n\
From the beginning of time,\n\
people have used email.\n\
\n\
From sender@example.com - looks like mbox!\n\
>From already-escaped line\n\
-- end.txt --\n\
done\n";

    let msg = MessageBuilder::new("dev@example.com", "txtar with From lines")
        .body(txtar_body)
        .build();

    // Save as mbox
    let mut buf = Vec::new();
    {
        let mut mbox = Mbox::new();
        mbox.append(msg);
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    // The raw mbox should have escaped the From lines
    let raw_mbox = String::from_utf8_lossy(&buf);
    assert!(
        raw_mbox.contains(">From the beginning"),
        "should escape bare 'From ': {}",
        raw_mbox
    );
    assert!(
        raw_mbox.contains(">From sender@example.com"),
        "should escape bare 'From sender': {}",
        raw_mbox
    );
    // In mboxrd, `>From` becomes `>>From`
    assert!(
        raw_mbox.contains(">>From already-escaped"),
        "should double-escape '>From': {}",
        raw_mbox
    );

    // Reload — unescaping should restore original content
    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    let body = reloaded.messages()[0].body();
    assert!(
        body.contains("From the beginning of time,"),
        "unescaped body: {}",
        body
    );
    assert!(
        body.contains("From sender@example.com - looks like mbox!"),
        "unescaped body: {}",
        body
    );
    assert!(
        body.contains(">From already-escaped line"),
        "unescaped body: {}",
        body
    );
    // txtar delimiters must also survive
    assert!(body.contains("-- message.txt --"), "delimiter lost: {}", body);
    assert!(body.contains("-- end.txt --"), "delimiter lost: {}", body);
}

// -----------------------------------------------------------------------
// Full txtar fixture file as message body
// -----------------------------------------------------------------------

#[test]
fn test_txtar_fixture_file_as_body_roundtrip() {
    let txtar_content = std::fs::read_to_string(fixture("sample.txtar")).unwrap();

    let msg = MessageBuilder::new("dev@example.com", "Full txtar archive")
        .body(&txtar_content)
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);

    let body = reloaded.messages()[0].body();

    // All txtar delimiters must survive
    assert!(body.contains("-- hello.go --"), "body = {}", body);
    assert!(body.contains("-- mail/From_header.txt --"), "body = {}", body);
    assert!(body.contains("-- config.toml --"), "body = {}", body);
    assert!(body.contains("-- empty.txt --"), "body = {}", body);
    assert!(body.contains("-- README.md --"), "body = {}", body);

    // File contents must survive (including From-line roundtrip)
    assert!(body.contains("fmt.Println"), "body = {}", body);
    assert!(
        body.contains("From the very beginning of time,"),
        "From line should roundtrip: {}",
        body
    );
    assert!(
        body.contains("From sender@example.com - this looks like an mbox separator!"),
        "From line should roundtrip: {}",
        body
    );
    assert!(
        body.contains(">From escaped in mboxrd style already"),
        ">From should roundtrip: {}",
        body
    );
}

// -----------------------------------------------------------------------
// txtar file as attachment
// -----------------------------------------------------------------------

#[test]
fn test_txtar_file_as_attachment_roundtrip() {
    let txtar_bytes = std::fs::read(fixture("sample.txtar")).unwrap();

    let msg = MessageBuilder::new("dev@example.com", "txtar attachment")
        .body("See attached txtar archive.\n")
        .attach_file(fixture("sample.txtar"))
        .unwrap()
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    let attachments = reloaded.messages()[0].attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "sample.txtar");
    // Attachment data should be byte-identical (base64 roundtrip)
    assert_eq!(
        attachments[0].data, txtar_bytes,
        "attachment data should be byte-identical after mbox roundtrip"
    );
}

// -----------------------------------------------------------------------
// Dashes-heavy content doesn't confuse mbox parser
// -----------------------------------------------------------------------

#[test]
fn test_dashes_in_body_not_confused_with_anything() {
    let body = "\
---\n\
----\n\
-- --\n\
-- not a separator\n\
--- separator-like ---\n\
----- many dashes -----\n\
-- From_edge_case --\n\
From inside a txtar entry after a dash-header\n";

    let msg = MessageBuilder::new("dev@example.com", "Dashes test")
        .body(body)
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1, "dashes should not split message");

    let result = reloaded.messages()[0].body();
    assert!(result.contains("---"), "body = {}", result);
    assert!(result.contains("----"), "body = {}", result);
    assert!(result.contains("-- --"), "body = {}", result);
    assert!(result.contains("-- not a separator"), "body = {}", result);
    assert!(result.contains("--- separator-like ---"), "body = {}", result);
    assert!(result.contains("----- many dashes -----"), "body = {}", result);
    assert!(result.contains("-- From_edge_case --"), "body = {}", result);
    assert!(
        result.contains("From inside a txtar entry"),
        "From line should roundtrip: {}",
        result
    );
}

// -----------------------------------------------------------------------
// mboxo format also handles txtar correctly
// -----------------------------------------------------------------------

#[test]
fn test_txtar_with_mboxo_format() {
    let txtar_body = "\
-- test.go --\n\
package main\n\
\n\
From the start\n\
>From already escaped\n\
-- end --\n";

    let msg = MessageBuilder::new("dev@example.com", "txtar mboxo")
        .body(txtar_body)
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf).with_format(MboxFormat::Mboxo);
        mbox.write_to(&mut writer).unwrap();
    }

    let raw = String::from_utf8_lossy(&buf);
    // mboxo only escapes bare `From `, NOT `>From`
    assert!(raw.contains(">From the start"), "mboxo should escape: {}", raw);
    assert!(
        raw.contains(">From already escaped"),
        "mboxo should NOT double-escape >From: {}",
        raw
    );

    // Reload (reader always uses mboxrd-style unescape)
    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    let body = reloaded.messages()[0].body();
    assert!(body.contains("-- test.go --"), "body = {}", body);
    assert!(body.contains("-- end --"), "body = {}", body);
    assert!(body.contains("From the start"), "body = {}", body);
}

// -----------------------------------------------------------------------
// Multiple messages with txtar content
// -----------------------------------------------------------------------

#[test]
fn test_multiple_txtar_messages_in_mbox() {
    let mut mbox = Mbox::new();

    let msg1 = MessageBuilder::new("alice@example.com", "Archive 1")
        .body("-- file1.txt --\ncontent1\n-- file2.txt --\ncontent2\n")
        .build();

    let msg2 = MessageBuilder::new("bob@example.com", "Archive 2")
        .body("-- main.go --\npackage main\n-- go.mod --\nmodule test\n")
        .build();

    mbox.append(msg1);
    mbox.append(msg2);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 2);

    let body1 = reloaded.messages()[0].body();
    assert!(body1.contains("-- file1.txt --"), "body1 = {}", body1);
    assert!(body1.contains("-- file2.txt --"), "body1 = {}", body1);

    let body2 = reloaded.messages()[1].body();
    assert!(body2.contains("-- main.go --"), "body2 = {}", body2);
    assert!(body2.contains("-- go.mod --"), "body2 = {}", body2);
}

// -----------------------------------------------------------------------
// Embedded mbox inside txtar inside mbox (nested)
// -----------------------------------------------------------------------

#[test]
fn test_mbox_inside_txtar_inside_mbox() {
    // A txtar archive that contains an mbox file as one of its entries
    let txtar_body = "\
-- patches.mbox --\n\
From dev@kernel.org Thu Jan  1 00:00:00 1970\n\
From: dev@kernel.org\n\
Subject: [PATCH] Fix bug\n\
\n\
Patch body here\n\
-- description.txt --\n\
This txtar contains an embedded mbox file.\n";

    let msg = MessageBuilder::new("sender@example.com", "Nested mbox in txtar")
        .body(txtar_body)
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    // Must be exactly 1 message — the inner `From ` line must NOT
    // be treated as a message separator.
    assert_eq!(
        reloaded.len(),
        1,
        "inner mbox From line should be escaped, not split"
    );

    let body = reloaded.messages()[0].body();
    assert!(body.contains("-- patches.mbox --"), "body = {}", body);
    assert!(body.contains("-- description.txt --"), "body = {}", body);
    // The inner "From dev@kernel.org..." should have roundtripped
    assert!(
        body.contains("From dev@kernel.org Thu Jan  1 00:00:00 1970"),
        "inner From line should roundtrip: {}",
        body
    );
    assert!(
        body.contains("Subject: [PATCH] Fix bug"),
        "inner headers should survive: {}",
        body
    );
}
