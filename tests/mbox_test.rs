use emx_mbox::{MailStore, Mbox, MboxWriter, MailMessage, MessageBuilder};

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// -----------------------------------------------------------------------
// Mbox collection: append, add_message
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
fn test_append_raw() {
    let mut mbox = Mbox::new();
    let eml = b"From: test@example.com\nSubject: Raw\n\nRaw body\n";
    mbox.append_raw(eml.to_vec());

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "Raw");
}

#[test]
fn test_append_eml_file() {
    let mut mbox = Mbox::new();
    mbox.append_eml(fixture("sample.eml")).unwrap();

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "Sample EML");
    assert_eq!(mbox.messages()[0].from(), "tester@example.com");
    assert!(mbox.messages()[0].body().contains("sample EML file"));
}

#[test]
fn test_load_mbox_file() {
    let mbox = Mbox::load_file(fixture("three_messages.mbox")).unwrap();
    assert_eq!(mbox.len(), 3);
    assert_eq!(mbox.messages()[0].from(), "herp@example.com");
    assert_eq!(mbox.messages()[2].subject(), "Third");
}

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
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 5);
    for i in 0..5 {
        assert_eq!(reloaded.messages()[i].subject(), format!("Message {}", i));
        assert!(reloaded.messages()[i]
            .body()
            .contains(&format!("Body of message {}", i)));
    }
}

#[test]
fn test_append_mail_message() {
    let msg = MailMessage::from_raw(
        b"From: direct@example.com\nSubject: Direct\n\nDirect body\n".to_vec(),
    );

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "Direct");
}
