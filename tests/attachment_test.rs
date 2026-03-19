use emx_mbox::{Attachment, MailStore, Mbox, MboxWriter, MessageBuilder};

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// -----------------------------------------------------------------------
// Attach real files from disk
// -----------------------------------------------------------------------

#[test]
fn test_attach_png_file() {
    let path = fixture("test.png");
    let original_bytes = std::fs::read(&path).unwrap();
    assert!(original_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]), "must be valid PNG");

    let msg = MessageBuilder::new("alice@example.com", "Screenshot")
        .body("See attached PNG.\n")
        .attach_file(&path)
        .unwrap()
        .build();

    let ct = msg.header("Content-Type").unwrap();
    assert!(ct.contains("multipart/mixed"), "Content-Type = {}", ct);

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.png");
    assert_eq!(attachments[0].content_type, "image/png");
    assert_eq!(attachments[0].data, original_bytes);
}

#[test]
fn test_attach_svg_file() {
    let path = fixture("test.svg");
    let original_bytes = std::fs::read(&path).unwrap();
    assert!(
        String::from_utf8_lossy(&original_bytes).contains("<svg"),
        "must be valid SVG"
    );

    let msg = MessageBuilder::new("alice@example.com", "Icon")
        .body("See attached SVG.\n")
        .attach_file(&path)
        .unwrap()
        .build();

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.svg");
    assert_eq!(attachments[0].content_type, "image/svg+xml");
    assert_eq!(attachments[0].data, original_bytes);
}

#[test]
fn test_attach_multiple_real_files() {
    let png_path = fixture("test.png");
    let svg_path = fixture("test.svg");
    let png_bytes = std::fs::read(&png_path).unwrap();
    let svg_bytes = std::fs::read(&svg_path).unwrap();

    let msg = MessageBuilder::new("alice@example.com", "Designs")
        .body("Both files attached.\n")
        .attach_file(&png_path)
        .unwrap()
        .attach_file(&svg_path)
        .unwrap()
        .build();

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 2, "should have 2 attachments");
    assert_eq!(attachments[0].filename, "test.png");
    assert_eq!(attachments[0].data, png_bytes);
    assert_eq!(attachments[1].filename, "test.svg");
    assert_eq!(attachments[1].data, svg_bytes);
}

// -----------------------------------------------------------------------
// Attachment roundtrip through mbox save/load
// -----------------------------------------------------------------------

#[test]
fn test_attachment_roundtrip_through_mbox() {
    let png_path = fixture("test.png");
    let png_bytes = std::fs::read(&png_path).unwrap();

    let msg = MessageBuilder::new("sender@example.com", "With attachment")
        .body("Please see the attached file.\n")
        .attach_file(&png_path)
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
    assert_eq!(reloaded.len(), 1);

    let loaded_msg = &reloaded.messages()[0];
    assert_eq!(loaded_msg.subject(), "With attachment");

    let attachments = loaded_msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.png");
    assert_eq!(attachments[0].data, png_bytes);
}

#[test]
fn test_multiple_file_attachments_roundtrip() {
    let png_bytes = std::fs::read(fixture("test.png")).unwrap();
    let svg_bytes = std::fs::read(fixture("test.svg")).unwrap();

    let msg = MessageBuilder::new("sender@example.com", "Multi-attach")
        .body("Two real files.\n")
        .attach_file(fixture("test.png"))
        .unwrap()
        .attach_file(fixture("test.svg"))
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
    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].filename, "test.png");
    assert_eq!(attachments[0].data, png_bytes);
    assert_eq!(attachments[1].filename, "test.svg");
    assert_eq!(attachments[1].data, svg_bytes);
}

// -----------------------------------------------------------------------
// Inline attachment data
// -----------------------------------------------------------------------

#[test]
fn test_attach_inline_data() {
    let data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // fake JPEG header

    let msg = MessageBuilder::new("alice@example.com", "Inline")
        .body("Inline bytes.\n")
        .attach("photo.jpg", "image/jpeg", data.clone())
        .build();

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "photo.jpg");
    assert_eq!(attachments[0].content_type, "image/jpeg");
    assert_eq!(attachments[0].data, data);
}

// -----------------------------------------------------------------------
// Attachments + trailers
// -----------------------------------------------------------------------

#[test]
fn test_attachment_with_trailers() {
    let msg = MessageBuilder::new("dev@kernel.org", "[PATCH] Add icon")
        .body("Patch body\n")
        .attach_file(fixture("test.png"))
        .unwrap()
        .signed_off_by("Dev <dev@kernel.org>")
        .build();

    let raw = String::from_utf8_lossy(msg.raw());
    assert!(
        raw.contains("Signed-off-by: Dev <dev@kernel.org>"),
        "raw = {}",
        raw
    );

    let attachments = msg.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.png");
}

// -----------------------------------------------------------------------
// No attachments → plain text
// -----------------------------------------------------------------------

#[test]
fn test_no_attachments_still_plain() {
    let msg = MessageBuilder::new("alice@example.com", "Plain")
        .body("Just text.\n")
        .build();

    let ct = msg.header("Content-Type").unwrap();
    assert!(ct.contains("text/plain"), "Content-Type = {}", ct);
    assert!(msg.attachments().is_empty());
}

// -----------------------------------------------------------------------
// Attachment struct
// -----------------------------------------------------------------------

#[test]
fn test_attachment_struct() {
    let att = Attachment::new("report.pdf", "application/pdf", vec![0x25, 0x50, 0x44, 0x46]);
    assert_eq!(att.filename, "report.pdf");
    assert_eq!(att.content_type, "application/pdf");
    assert_eq!(att.data.len(), 4);
}

#[test]
fn test_attachment_from_file() {
    let att = Attachment::from_file(fixture("test.svg")).unwrap();
    assert_eq!(att.filename, "test.svg");
    assert_eq!(att.content_type, "image/svg+xml");
    assert!(String::from_utf8_lossy(&att.data).contains("<svg"));
}
