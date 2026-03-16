//! Roundtrip: read mbox → modify messages → write mbox.
//!
//! Demonstrates the full pipeline that b4 performs:
//!  1. Read an mbox (b4 mbox <msgid>)
//!  2. Inspect / filter messages
//!  3. Add trailers (like b4's trailer collection from follow-ups)
//!  4. Re-serialize as mboxrd (b4 am --write-mbox)
//!
//! This also exercises From-line escaping roundtrip, similar to go-mbox's
//! reader → writer test.
//!
//! Usage:
//!   cargo run --example roundtrip

use emx_mbox::{MailStore, Mbox, MboxWriter, MessageBuilder, MboxFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Source mbox with tricky content ──────────────────────────────────
    // Note: In mboxrd format, bare "From " lines in the body MUST be
    // escaped as ">From ".  The ">From " below is a once-escaped "From "
    // that the reader will unescape back to "From ".
    // ">>From " is a doubly-escaped line that becomes ">From " on read.
    let mbox_data = "\
From dev@kernel.org Thu Jan  1 00:00:00 1970\n\
From: Developer <dev@kernel.org>\n\
To: linux-kernel@vger.kernel.org\n\
Subject: [PATCH v1] Fix something important\n\
Date: Thu, 01 Jan 1970 00:00:00 +0000\n\
Message-ID: <patch-1@kernel.org>\n\
\n\
This patch fixes something important.\n\
\n\
>From the user's perspective this was confusing.\n\
>>From a doubly-escaped line.\n\
\n\
Signed-off-by: Developer <dev@kernel.org>\n\
---\n\
 file.c | 2 +-\n\
 1 file changed, 1 insertion(+), 1 deletion(-)\n\
\n\
diff --git a/file.c b/file.c\n\
--- a/file.c\n\
+++ b/file.c\n\
@@ -1,3 +1,3 @@\n\
 int main() {\n\
-    return 1;\n\
+    return 0;\n\
 }\n";

    // ── Step 1: Parse ───────────────────────────────────────────────────
    println!("=== Step 1: Parse mbox ===");
    let mbox = Mbox::load(mbox_data.as_bytes())?;
    let msg = &mbox.messages()[0];
    println!("Subject: {}", msg.subject());
    println!("From:    {}", msg.from());
    println!("Body contains 'From the user': {}", msg.body().contains("From the user"));
    println!("Body contains '>From a doubly': {}", msg.body().contains(">From a doubly"));

    // ── Step 2: Build a follow-up review (b4-style trailer addition) ────
    println!("\n=== Step 2: Build review follow-up ===");
    let review = MessageBuilder::new("Reviewer <rev@kernel.org>", "Re: [PATCH v1] Fix something important")
        .to("linux-kernel@vger.kernel.org")
        .in_reply_to("<patch-1@kernel.org>")
        .reference("<patch-1@kernel.org>")
        .body("Looks good to me.\n")
        .reviewed_by("Reviewer <rev@kernel.org>")
        .build();
    println!("Review Message-ID: {}", review.message_id().unwrap_or("-"));

    // ── Step 3: Combine into output mbox ────────────────────────────────
    println!("\n=== Step 3: Write combined mbox (mboxrd format) ===");
    let mut combined = Mbox::new();
    for m in mbox.iter() {
        combined.append(m.clone());
    }
    combined.append(review);

    let mut buf = Vec::new();
    let mut writer = MboxWriter::new(&mut buf).with_format(MboxFormat::Mboxrd);
    combined.write_to(&mut writer)?;

    println!("Output: {} bytes, {} messages", buf.len(), combined.len());

    // ── Step 4: Verify roundtrip ────────────────────────────────────────
    println!("\n=== Step 4: Verify roundtrip ===");
    let reloaded = Mbox::load(buf.as_slice())?;
    assert_eq!(reloaded.len(), 2, "expected 2 messages after roundtrip");

    let patch = &reloaded.messages()[0];
    assert!(
        patch.body().contains("From the user"),
        "escaped From-line in body must survive roundtrip"
    );
    assert!(
        patch.body().contains(">From a doubly"),
        "doubly-escaped From-line must survive roundtrip"
    );
    println!("Roundtrip OK: {} messages, From-escape preserved", reloaded.len());

    // Print the envelope lines for inspection
    println!("\nEnvelope lines:");
    for line in String::from_utf8_lossy(&buf).lines() {
        if line.starts_with("From ") && !line.starts_with("From:") {
            println!("  {}", line);
        }
    }

    Ok(())
}
