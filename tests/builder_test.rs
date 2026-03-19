use emx_mbox::MessageBuilder;

// -----------------------------------------------------------------------
// MessageBuilder basics
// -----------------------------------------------------------------------

#[test]
fn test_message_builder_basic() {
    let msg = MessageBuilder::new("Alice <alice@example.com>", "Hello World")
        .to("Bob <bob@example.com>")
        .body("This is the body.\n")
        .build();

    assert_eq!(msg.from(), "Alice <alice@example.com>");
    assert_eq!(msg.subject(), "Hello World");
    assert!(msg.message_id().is_some());
    assert!(msg.header("Date").is_some());
    assert_eq!(msg.header("MIME-Version").unwrap(), "1.0");
    assert!(msg.header("Content-Type").unwrap().contains("text/plain"));
}

#[test]
fn test_message_builder_patch_subject() {
    let msg = MessageBuilder::new("dev@kernel.org", "Fix null pointer dereference")
        .patch_subject(Some("PATCH"), Some(2), 1, 3)
        .body("commit body\n\nSigned-off-by: dev@kernel.org\n")
        .build();

    assert_eq!(msg.subject(), "[PATCH v2 1/3] Fix null pointer dereference");
}

#[test]
fn test_message_builder_threading() {
    let cover_id = "<cover-v1@example.com>";
    let msg = MessageBuilder::new("dev@kernel.org", "Patch description")
        .in_reply_to(cover_id)
        .reference(cover_id)
        .build();

    assert_eq!(msg.header("In-Reply-To").unwrap(), cover_id);
    assert!(msg.header("References").unwrap().contains(cover_id));
}

// -----------------------------------------------------------------------
// Trailers
// -----------------------------------------------------------------------

#[test]
fn test_builder_trailers() {
    let msg = MessageBuilder::new("dev@kernel.org", "[PATCH] Fix something")
        .body("Patch description\n\n---\ndiff goes here\n")
        .signed_off_by("Dev <dev@kernel.org>")
        .reviewed_by("Reviewer <rev@kernel.org>")
        .acked_by("Acker <ack@kernel.org>")
        .build();

    let body = String::from_utf8_lossy(msg.raw());
    assert!(body.contains("Signed-off-by: Dev <dev@kernel.org>"), "body = {}", body);
    assert!(body.contains("Reviewed-by: Reviewer <rev@kernel.org>"), "body = {}", body);
    assert!(body.contains("Acked-by: Acker <ack@kernel.org>"), "body = {}", body);
}

#[test]
fn test_builder_custom_trailer() {
    let msg = MessageBuilder::new("dev@kernel.org", "Test")
        .body("Body\n")
        .trailer("Tested-by", "QA <qa@example.com>")
        .build();

    let body = String::from_utf8_lossy(msg.raw());
    assert!(body.contains("Tested-by: QA <qa@example.com>"), "body = {}", body);
}
