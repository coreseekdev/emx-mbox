//! Build email messages using MessageBuilder, including patch series.
//!
//! Mirrors b4's LoreMessage / LoreSeries construction for kernel patch
//! workflows: cover letters, numbered patches, trailers, threading, etc.
//!
//! Usage:
//!   cargo run --example build_message

use emx_mbox::{MailStore, Mbox, MessageBuilder, MboxWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Simple message ───────────────────────────────────────────────
    println!("=== Simple message ===");
    let msg = MessageBuilder::new("Alice <alice@example.com>", "Hello World")
        .to("Bob <bob@example.com>")
        .cc("Carol <carol@example.com>")
        .body("This is the message body.\n")
        .build();

    println!("From:       {}", msg.from());
    println!("Subject:    {}", msg.subject());
    println!("Message-ID: {}", msg.message_id().unwrap_or("-"));
    println!();

    // ── 2. Patch series with cover letter (b4-style) ────────────────────
    // Equivalent to b4's LoreSeries: cover letter + numbered patches,
    // with trailers like Signed-off-by, Reviewed-by, Acked-by.
    println!("=== Patch series (3 patches) ===");

    let cover_id = "<cover-v1-0-3-fix-series@kernel.org>";

    // Cover letter: [PATCH v1 0/3]
    let cover = MessageBuilder::new(
        "Developer <dev@kernel.org>",
        "Fix null pointer dereferences in driver",
    )
    .to("linux-kernel@vger.kernel.org")
    .cc("maintainer@kernel.org")
    .message_id(cover_id)
    .patch_subject(Some("PATCH"), Some(1), 0, 3)
    .body(
        "This series fixes three null pointer dereferences found by\n\
         static analysis in the XYZ driver.\n\
         \n\
         Developer (3):\n\
         \x20 driver/xyz: check return value of alloc\n\
         \x20 driver/xyz: validate input pointer\n\
         \x20 driver/xyz: add missing null check in probe\n",
    )
    .build();

    // Patch 1: [PATCH v1 1/3]
    let patch1 = MessageBuilder::new(
        "Developer <dev@kernel.org>",
        "driver/xyz: check return value of alloc",
    )
    .to("linux-kernel@vger.kernel.org")
    .in_reply_to(cover_id)
    .reference(cover_id)
    .patch_subject(Some("PATCH"), Some(1), 1, 3)
    .body(
        "The return value of kmalloc() was not checked, leading to a\n\
         null pointer dereference.\n\
         \n\
         ---\n\
         drivers/xyz/core.c | 3 ++-\n\
         1 file changed, 2 insertions(+), 1 deletion(-)\n\
         \n\
         diff --git a/drivers/xyz/core.c b/drivers/xyz/core.c\n\
         --- a/drivers/xyz/core.c\n\
         +++ b/drivers/xyz/core.c\n\
         @@ -42,6 +42,8 @@\n\
         \x20    buf = kmalloc(size, GFP_KERNEL);\n\
         +   if (!buf)\n\
         +       return -ENOMEM;\n",
    )
    .signed_off_by("Developer <dev@kernel.org>")
    .reviewed_by("Reviewer <rev@kernel.org>")
    .build();

    // Patch 2: [PATCH v1 2/3]
    let patch2 = MessageBuilder::new(
        "Developer <dev@kernel.org>",
        "driver/xyz: validate input pointer",
    )
    .to("linux-kernel@vger.kernel.org")
    .in_reply_to(cover_id)
    .reference(cover_id)
    .patch_subject(Some("PATCH"), Some(1), 2, 3)
    .body("Validate input pointer before use.\n")
    .signed_off_by("Developer <dev@kernel.org>")
    .build();

    // Patch 3: [PATCH v1 3/3]
    let patch3 = MessageBuilder::new(
        "Developer <dev@kernel.org>",
        "driver/xyz: add missing null check in probe",
    )
    .to("linux-kernel@vger.kernel.org")
    .in_reply_to(cover_id)
    .reference(cover_id)
    .patch_subject(Some("PATCH"), Some(1), 3, 3)
    .body("Add missing null check in probe function.\n")
    .signed_off_by("Developer <dev@kernel.org>")
    .acked_by("Maintainer <maintainer@kernel.org>")
    .build();

    // Collect into an Mbox and write out (like b4's save_mboxrd_mbox)
    let mut mbox = Mbox::new();
    for msg in [cover, patch1, patch2, patch3] {
        println!("  {}", msg.subject());
        mbox.append(msg);
    }

    let mut buf = Vec::new();
    let mut writer = MboxWriter::new(&mut buf);
    mbox.write_to(&mut writer)?;

    println!("\n--- Serialized mbox ({} bytes) ---", buf.len());
    // Print just the From lines
    for line in String::from_utf8_lossy(&buf).lines() {
        if line.starts_with("From ") && !line.starts_with("From:") {
            println!("  {}", line);
        }
    }

    Ok(())
}
