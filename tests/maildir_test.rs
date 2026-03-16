use std::fs;
use std::path::PathBuf;

use emx_mbox::{Maildir, MailStore, Mbox, MessageBuilder, MboxWriter};

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("emx_mbox_test").join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// -----------------------------------------------------------------------
// Basic Maildir operations
// -----------------------------------------------------------------------

#[test]
fn test_maildir_save_and_load() {
    let dir = tmp_dir("save_and_load");

    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("alice@example.com", "[PATCH 1/2] Fix bug")
            .body("Bug fix\n")
            .build(),
    );
    md.append(
        MessageBuilder::new("bob@example.com", "[PATCH 2/2] Add feature")
            .body("Feature\n")
            .build(),
    );

    md.save(dir.as_path()).unwrap();

    // Verify directory structure
    assert!(dir.join("new").is_dir());
    assert!(dir.join("cur").is_dir());
    assert!(dir.join("tmp").is_dir());

    // Verify files in new/
    let files: Vec<_> = fs::read_dir(dir.join("new"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 2);

    // Load it back
    let loaded = Maildir::load(&dir).unwrap();
    assert_eq!(loaded.len(), 2);
    assert!(loaded.messages()[0].subject().contains("Fix bug")
         || loaded.messages()[1].subject().contains("Fix bug"));
}

#[test]
fn test_maildir_b4_compatible_filenames() {
    let dir = tmp_dir("b4_filenames");

    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("dev@kernel.org", "[PATCH v3 0/4] Series: my cool fix")
            .body("Cover letter\n")
            .build(),
    );
    md.append(
        MessageBuilder::new("dev@kernel.org", "[PATCH v3 1/4] Fix null pointer")
            .body("Fix\n")
            .build(),
    );

    md.save(dir.as_path()).unwrap();

    let mut files: Vec<String> = fs::read_dir(dir.join("new"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();

    // Counter from [PATCH v3 0/4] → 0, from [PATCH v3 1/4] → 1
    assert!(files[0].starts_with("0000_"), "got: {}", files[0]);
    assert!(files[0].ends_with(".eml"), "got: {}", files[0]);
    assert!(files[1].starts_with("0001_"), "got: {}", files[1]);
}

#[test]
fn test_maildir_detect() {
    let dir = tmp_dir("detect");
    assert!(!Maildir::is_maildir(&dir));

    fs::create_dir_all(dir.join("new")).unwrap();
    fs::create_dir_all(dir.join("cur")).unwrap();
    fs::create_dir_all(dir.join("tmp")).unwrap();
    assert!(Maildir::is_maildir(&dir));
    assert!(<Maildir as MailStore>::detect(dir.as_path()));
}

#[test]
fn test_maildir_append_to_existing() {
    let dir = tmp_dir("append_to");

    // Create initial maildir with one message
    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("alice@example.com", "First")
            .body("First\n")
            .build(),
    );
    md.save(dir.as_path()).unwrap();
    assert_eq!(Maildir::load(dir.as_path()).unwrap().len(), 1);

    // Append another message
    let msg2 = MessageBuilder::new("bob@example.com", "Second")
        .body("Second\n")
        .build();
    Maildir::append_to(dir.as_path(), &msg2).unwrap();

    let reloaded = Maildir::load(dir.as_path()).unwrap();
    assert_eq!(reloaded.len(), 2);
}

#[test]
fn test_maildir_load_nonexistent_fails() {
    let dir = tmp_dir("nonexistent_load");
    let result = Maildir::load(dir.as_path());
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// Roundtrip between Mbox and Maildir
// -----------------------------------------------------------------------

#[test]
fn test_mbox_to_maildir_roundtrip() {
    let dir = tmp_dir("mbox_to_maildir");

    // Load from mbox fixture
    let mbox = Mbox::load_file(fixture("three_messages.mbox")).unwrap();
    assert_eq!(mbox.len(), 3);

    // Transfer to maildir
    let mut md = Maildir::new(&dir);
    for msg in mbox.messages() {
        md.append(msg.clone());
    }
    md.save(dir.as_path()).unwrap();

    // Load back from maildir
    let loaded = Maildir::load(dir.as_path()).unwrap();
    assert_eq!(loaded.len(), 3);

    // Verify all subjects are preserved
    let subjects: Vec<&str> = loaded.messages().iter().map(|m| m.subject()).collect();
    for msg in mbox.messages() {
        assert!(
            subjects.iter().any(|s| *s == msg.subject()),
            "Missing subject: {}",
            msg.subject()
        );
    }
}

#[test]
fn test_maildir_to_mbox_roundtrip() {
    let dir = tmp_dir("maildir_to_mbox");

    // Build a maildir
    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("alice@a.com", "[PATCH 1/1] My change")
            .body("diff --git a/file\n")
            .build(),
    );
    md.save(dir.as_path()).unwrap();

    // Load from maildir, write as mbox
    let loaded = Maildir::load(dir.as_path()).unwrap();
    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        for msg in loaded.messages() {
            writer.write_mail_message(msg).unwrap();
        }
    }

    // Load the mbox back
    let mbox = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(mbox.len(), 1);
    assert!(mbox.messages()[0].subject().contains("My change"));
}

// -----------------------------------------------------------------------
// MailStore trait polymorphism
// -----------------------------------------------------------------------

#[test]
fn test_mail_store_trait_polymorphism() {
    let dir = tmp_dir("polymorphism");

    fn count_messages(store: &dyn MailStore) -> usize {
        store.len()
    }

    fn first_subject<'a>(store: &'a dyn MailStore) -> &'a str {
        store.messages().first().map(|m| m.subject()).unwrap_or("")
    }

    // Mbox via trait
    let mbox_path = fixture("three_messages.mbox");
    let mbox = Mbox::load_file(&mbox_path).unwrap();
    assert_eq!(count_messages(&mbox), 3);

    // Maildir via trait
    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("test@test.com", "Trait test")
            .body("body\n")
            .build(),
    );
    md.save(dir.as_path()).unwrap();
    let md_loaded = Maildir::load(dir.as_path()).unwrap();
    assert_eq!(count_messages(&md_loaded), 1);
    assert_eq!(first_subject(&md_loaded), "Trait test");
}

// -----------------------------------------------------------------------
// IntoIterator
// -----------------------------------------------------------------------

#[test]
fn test_maildir_into_iterator() {
    let dir = tmp_dir("into_iter");

    let mut md = Maildir::new(&dir);
    md.append(
        MessageBuilder::new("a@a.com", "One").body("1\n").build(),
    );
    md.append(
        MessageBuilder::new("b@b.com", "Two").body("2\n").build(),
    );

    // Borrow iteration
    let mut count = 0;
    for _msg in &md {
        count += 1;
    }
    assert_eq!(count, 2);

    // Owned iteration
    let subjects: Vec<String> = md.into_iter().map(|m| m.subject().to_owned()).collect();
    assert_eq!(subjects.len(), 2);
}

// -----------------------------------------------------------------------
// Attachment roundtrip through Maildir
// -----------------------------------------------------------------------

#[test]
fn test_maildir_attachment_roundtrip() {
    let dir = tmp_dir("attach_roundtrip");
    let png_path = fixture("test.png");
    let original_bytes = fs::read(&png_path).unwrap();

    let msg = MessageBuilder::new("dev@kernel.org", "[PATCH 1/1] Add icon")
        .body("See attached\n")
        .attach_file(&png_path)
        .unwrap()
        .build();

    let mut md = Maildir::new(&dir);
    md.append(msg);
    md.save(dir.as_path()).unwrap();

    let loaded = Maildir::load(dir.as_path()).unwrap();
    assert_eq!(loaded.len(), 1);
    let attachments = loaded.messages()[0].attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].filename, "test.png");
    assert_eq!(attachments[0].data, original_bytes);
}

// -----------------------------------------------------------------------
// EML append
// -----------------------------------------------------------------------

#[test]
fn test_maildir_append_eml() {
    let dir = tmp_dir("append_eml");

    let mut md = Maildir::new(&dir);
    md.append_eml(fixture("sample.eml")).unwrap();
    assert_eq!(md.len(), 1);
    assert_eq!(md.messages()[0].subject(), "Sample EML");

    <Maildir as MailStore>::save(&md, dir.as_path()).unwrap();
    let loaded = Maildir::load(dir.as_path()).unwrap();
    assert_eq!(loaded.len(), 1);
}
