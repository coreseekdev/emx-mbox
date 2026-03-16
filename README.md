# emx-mbox

Rust library for reading & writing mbox files, with first-class support for
Linux kernel patch workflows (b4 / git-am).

## Features

- **mboxrd / mboxo** format support (mboxrd is the default, used by b4)
- **Streaming reader** (`MboxReader`) with `Iterator` support
- **Streaming writer** (`MboxWriter`) with proper `From ` escaping
- **MessageBuilder** — construct RFC 5322 messages with patch subjects,
  threading (`In-Reply-To`, `References`), and git trailers
  (`Signed-off-by`, `Reviewed-by`, `Acked-by`)
- **Envelope From preservation** — round-trips the sender from the `From `
  separator line
- **CRLF normalisation** — input with `\r\n` is stored internally as `\n`
- Append messages from raw bytes, `.eml` files, or via the builder API
- In-memory collection (`Mbox`) for load / append / save workflows

## Quick start

```rust
use emx_mbox::{Mbox, MboxReader, MboxWriter, MessageBuilder, MboxFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Load an existing mbox ------------------------------------------------
    let mbox = Mbox::load_file("patches.mbox")?;
    for msg in mbox.messages() {
        println!("{}: {}", msg.from(), msg.subject());
    }

    // --- Streaming reader (low memory) ----------------------------------------
    let reader = MboxReader::from_file("patches.mbox")?;
    for result in reader {
        let msg = result?;
        println!("[{}] {}", msg.message_id().unwrap_or_default(), msg.subject());
    }

    // --- Build a patch message ------------------------------------------------
    let msg = MessageBuilder::new("Dev <dev@kernel.org>", "Fix null deref")
        .patch_subject(Some("PATCH"), Some(2), 1, 3)
        .to("maintainer@kernel.org")
        .in_reply_to("<cover-v2@kernel.org>")
        .reference("<cover-v2@kernel.org>")
        .body("commit message\n\n---\ndiff --git ...\n")
        .signed_off_by("Dev <dev@kernel.org>")
        .reviewed_by("Reviewer <rev@kernel.org>")
        .build();

    // --- Append to an mbox file -----------------------------------------------
    let mut mbox = Mbox::new();
    mbox.append(msg);
    mbox.append_eml("another.eml")?;
    mbox.save_file("out.mbox")?;

    // --- Writer with explicit format ------------------------------------------
    let mut buf = Vec::new();
    let mut writer = MboxWriter::new(&mut buf).with_format(MboxFormat::Mboxrd);
    for msg in mbox.messages() {
        writer.write_mail_message(msg)?;
    }

    Ok(())
}
```

## API overview

| Type | Purpose |
|------|---------|
| `MboxReader<R>` | Streaming reader; implements `Iterator<Item = Result<MailMessage>>` |
| `MboxWriter<W>` | Streaming writer; `write_message()` / `write_mail_message()` |
| `MailMessage` | Owned RFC 5322 message with cached header access |
| `MessageBuilder` | Fluent API for constructing messages (b4-compatible) |
| `Mbox` | In-memory message collection with load / append / save |
| `MboxFormat` | `Mboxrd` (default) or `Mboxo` |
| `MboxError` | Error type (`Io`, `Parse`, `InvalidFormat`) |

## b4 compatibility

Messages produced by `MessageBuilder` include the headers b4 expects:
`From`, `Date`, `Subject`, `Message-ID`, `MIME-Version`, `Content-Type`,
and optionally `In-Reply-To` / `References` for threading.

The writer uses the `mboxrd@z` placeholder when no envelope sender is
available, matching b4's own convention.

## Related projects

- [go-mbox](https://github.com/emersion/go-mbox) — Go mbox reader/writer
- [go-message](https://github.com/emersion/go-message) — Go RFC 5322 parser
- [b4](https://git.kernel.org/pub/scm/utils/b4/b4.git/) — Linux patch workflow tool

## License

MIT
