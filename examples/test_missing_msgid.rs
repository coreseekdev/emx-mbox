//! Test Message-ID generation for messages without Message-ID header

use emx_mbox::{MailMessage, MessageBuilder};

fn main() {
    println!("=== Testing Message-ID generation ===\n");

    // 1. MessageBuilder - should auto-generate
    println!("1. MessageBuilder (should auto-generate):");
    let msg_from_builder = MessageBuilder::new("test@example.com", "Test")
        .body("Test body")
        .build();
    println!("   Message-ID: {:?}\n", msg_from_builder.message_id());

    // 2. from_raw with Message-ID
    println!("2. from_raw with Message-ID:");
    let raw_with_id = b"From: test@example.com\n\
        Subject: Test\n\
        Message-ID: <test@example.com>\n\
        \n\
        Body";
    let msg_with_id = MailMessage::from_raw(raw_with_id.to_vec());
    println!("   Message-ID: {:?}\n", msg_with_id.message_id());

    // 3. from_raw WITHOUT Message-ID
    println!("3. from_raw WITHOUT Message-ID:");
    let raw_without_id = b"From: test@example.com\n\
        Subject: Test\n\
        \n\
        Body";
    let msg_without_id = MailMessage::from_raw(raw_without_id.to_vec());
    println!("   Message-ID: {:?}\n", msg_without_id.message_id());

    // Summary
    println!("=== Summary ===");
    println!("MessageBuilder: {:?}", msg_from_builder.message_id().is_some());
    println!("from_raw (with ID): {:?}", msg_with_id.message_id().is_some());
    println!("from_raw (without ID): {:?}", msg_without_id.message_id().is_some());

    // Check if from_raw without ID is None
    if msg_without_id.message_id().is_none() {
        println!("\n⚠️  WARNING: from_raw does NOT auto-generate Message-ID!");
        println!("   This can break tombstone deletion (is_deleted relies on Message-ID)");
    }
}
