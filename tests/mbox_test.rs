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

// -----------------------------------------------------------------------
// CJK (Chinese, Japanese, Korean) and Emoji support
// -----------------------------------------------------------------------

#[test]
fn test_cjk_subject_and_body() {
    let msg = MessageBuilder::new("张三@example.com", "测试邮件主题")
        .to(" receiver@例え.jp")
        .body("这是一封中文测试邮件。\n内容包括多行文本。\n最后一行。")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "测试邮件主题");
    assert!(mbox.messages()[0].body().contains("中文测试邮件"));
    assert!(mbox.messages()[0].body().contains("多行文本"));
}

#[test]
fn test_japanese_subject_and_body() {
    let msg = MessageBuilder::new("tanaka@日本.jp", "テストメールの件名")
        .body("こんにちは、世界！\nこれは日本語のテストです。\nよろしくお願いします。")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "テストメールの件名");
    assert!(mbox.messages()[0].body().contains("こんにちは"));
    assert!(mbox.messages()[0].body().contains("日本語"));
}

#[test]
fn test_korean_subject_and_body() {
    let msg = MessageBuilder::new("kim@테스트.kr", "테스트 이메일 제목")
        .body("안녕하세요!\n이것은 한글 테스트입니다.\n감사합니다.")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "테스트 이메일 제목");
    assert!(mbox.messages()[0].body().contains("안녕하세요"));
    assert!(mbox.messages()[0].body().contains("한글"));
}

#[test]
fn test_emoji_subject_and_body() {
    let msg = MessageBuilder::new("emoji@example.com", "Hello 🌍 World 🚀")
        .body("Greetings! 👋\nThis is a test with emojis 🎉🎊🎁\nBye! 👋")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    assert_eq!(mbox.messages()[0].subject(), "Hello 🌍 World 🚀");
    assert!(mbox.messages()[0].body().contains("👋"));
    assert!(mbox.messages()[0].body().contains("🎉"));
    assert!(mbox.messages()[0].body().contains("🎁"));
}

#[test]
fn test_mixed_cjk_and_emoji() {
    let msg = MessageBuilder::new("mixed@example.com", "混合测试 🇨🇳🇯🇵🇰🇷 Mix")
        .body("中文测试 ✅\n日本語テスト ✨\n한글 테스트 🌟\nEmoji: 😀😁😂🤣😃😄")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    assert_eq!(mbox.len(), 1);
    let subject = mbox.messages()[0].subject();
    assert!(subject.contains("混合测试"));
    assert!(subject.contains("🇨🇳"));

    let body = mbox.messages()[0].body();
    assert!(body.contains("中文测试"));
    assert!(body.contains("日本語"));
    assert!(body.contains("한글"));
    assert!(body.contains("😀"));
}

#[test]
fn test_cjk_emoji_roundtrip() {
    let mut mbox = Mbox::new();

    // Add Chinese message
    mbox.add_message(
        "chinese@example.com",
        "中文邮件 🔥",
        "正文内容\n测试换行 ✅",
    );

    // Add Japanese message
    mbox.add_message(
        "japanese@example.com",
        "日本語 🗾",
        "テスト本文\n改行テスト ⭐",
    );

    // Add Korean message
    mbox.add_message(
        "korean@example.com",
        "한글 🇰🇷",
        "테스트 본문\n줄바꿈 테스트 💫",
    );

    // Roundtrip: write to buffer and reload
    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 3);

    // Verify Chinese
    assert_eq!(reloaded.messages()[0].subject(), "中文邮件 🔥");
    assert!(reloaded.messages()[0].body().contains("正文内容"));
    assert!(reloaded.messages()[0].body().contains("✅"));

    // Verify Japanese
    assert_eq!(reloaded.messages()[1].subject(), "日本語 🗾");
    assert!(reloaded.messages()[1].body().contains("テスト本文"));
    assert!(reloaded.messages()[1].body().contains("⭐"));

    // Verify Korean
    assert_eq!(reloaded.messages()[2].subject(), "한글 🇰🇷");
    assert!(reloaded.messages()[2].body().contains("테스트 본문"));
    assert!(reloaded.messages()[2].body().contains("💫"));
}

#[test]
fn test_cjk_with_from_escape() {
    // Test that "From " escaping works correctly with CJK content
    let msg = MessageBuilder::new("test@example.com", "测试 From 转义")
        .body("开始\nFrom 这里需要转义\n结束 🎯")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let saved = String::from_utf8_lossy(&buf);
    assert!(
        saved.contains(">From 这里需要转义"),
        "should escape 'From ' in CJK body: {}",
        saved
    );

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);
    let body = reloaded.messages()[0].body();
    assert!(
        body.contains("From 这里需要转义"),
        "should unescape correctly: {:?}",
        body
    );
    assert!(body.contains("🎯"));
}

#[test]
fn test_complex_emoji_sequences() {
    // Test complex emoji: skin tone modifiers, ZWJ sequences, etc.
    let msg = MessageBuilder::new("complex@example.com", "Complex 👨‍👩‍👧‍👦 Emoji 🏳️‍🌈")
        .body("Family: 👨‍👩‍👧‍👦\nCouple: 👩‍❤️‍👨\nFlag: 🏳️‍🌈\nSkin: 👍🏽👏🏾🙌🏿")
        .build();

    let mut mbox = Mbox::new();
    mbox.append(msg);

    // Roundtrip
    let mut buf = Vec::new();
    {
        let mut writer = MboxWriter::new(&mut buf);
        mbox.write_to(&mut writer).unwrap();
    }

    let reloaded = Mbox::load(buf.as_slice()).unwrap();
    assert_eq!(reloaded.len(), 1);

    let subject = reloaded.messages()[0].subject();
    assert!(subject.contains("👨‍👩‍👧‍👦"));
    assert!(subject.contains("🏳️‍🌈"));

    let body = reloaded.messages()[0].body();
    assert!(body.contains("👨‍👩‍👧‍👦"));
    assert!(body.contains("👩‍❤️‍👨"));
    assert!(body.contains("🏳️‍🌈"));
    assert!(body.contains("👍🏽"));
    assert!(body.contains("👏🏾"));
    assert!(body.contains("🙌🏿"));
}
