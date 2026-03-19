//! Test Message-ID generation with debug output

use emx_mbox::MailMessage;

fn main() {
    println!("=== Testing Message-ID auto-generation ===\n");

    // Test 1: from_raw WITHOUT Message-ID
    println!("Test 1: from_raw WITHOUT Message-ID");
    let raw_without_id = b"From: test@example.com\n\
        Subject: Test\n\
        \n\
        Body";

    println!("Raw bytes length: {}", raw_without_id.len());

    let msg = MailMessage::from_raw(raw_without_id.to_vec());
    println!("After from_raw:");
    println!("  Raw bytes length: {}", msg.raw().len());

    // Check if Message-ID was added
    let raw_str = String::from_utf8_lossy(msg.raw());
    if raw_str.contains("Message-ID:") {
        println!("  ✅ Message-ID header found in raw bytes");
        if let Some(idx) = raw_str.find("Message-ID:") {
            let line_start = idx;
            let line_end = raw_str[idx..].find('\n').map(|x| idx + x).unwrap_or(raw_str.len());
            println!("  Line: {}", &raw_str[line_start..line_end]);
        }
    } else {
        println!("  ❌ Message-ID header NOT found in raw bytes");
    }

    println!("\n  Parsed Message-ID: {:?}", msg.message_id());

    // Test 2: Check headers
    println!("\nAll headers in raw message:");
    for line in raw_str.lines().take(10) {
        if !line.is_empty() && !line.starts_with("Body") {
            println!("  {}", line);
        }
    }
}
