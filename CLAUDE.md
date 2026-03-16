# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

```bash
# Build the library
cargo build

# Run all tests
cargo test

# Run a specific test file
cargo test --test mbox_test

# Run a specific test by name (substring match)
cargo test test_cjk_emoji_roundtrip

# Run tests with output
cargo test -- --nocapture

# Check for compilation errors
cargo check

# Run the binary examples
cargo run --example read_mbox
cargo run --example roundtrip
```

## Architecture

This is a Rust library for reading and writing mbox files, designed for Linux kernel patch workflows (b4 / git-am compatibility).

### Core Types

| Type | File | Purpose |
|------|------|---------|
| `MboxReader<R>` | `reader.rs` | Streaming iterator over mbox messages |
| `MboxWriter<W>` | `writer.rs` | Streaming writer with proper `From ` escaping |
| `MailMessage` | `message.rs` | Owned RFC 5322 message with lazy cached headers/body |
| `MessageBuilder` | `builder.rs` | Fluent API for constructing b4-compatible messages |
| `Mbox` | `mbox.rs` | In-memory collection implementing `MailStore` |
| `Maildir` | `maildir.rs` | Maildir backend (new/cur/tmp) implementing `MailStore` |

### Data Flow

```
MboxReader (streaming) → MailMessage (owned) → MboxWriter (streaming)
                         ↓
                    Mbox (in-memory collection)
```

### Key Design Points

- **mboxrd is default**: Uses mboxrd format (b4-compatible) for `From ` escaping. mboxo also supported via `MboxFormat` enum.
- **CRLF normalization**: All input is normalized to LF internally; writer outputs LF.
- **Envelope preservation**: The `From sender@host date` separator line's sender is preserved in `MailMessage::envelope_from()`.
- **Lazy parsing**: `MailMessage` parses headers and body on first access via `OnceLock` caching.
- **MailStore trait**: Unified interface for both `Mbox` and `Maildir` backends; `open()` auto-detects format.

### From-line Escaping Rules

- **mboxrd**: Lines starting with `>*From ` get one `>` prepended. Unescaping removes one `>` from `>+From `.
- **mboxo**: Only bare `From ` gets `>` prepended. `>From ` unescapes to `From `.

### Dependencies

- `mailparse` — RFC 5322 parsing
- `chrono` — date/time handling
- `base64` — attachment encoding
- `uuid` — Message-ID generation

## b4 Compatibility

Messages from `MessageBuilder` include headers b4 expects: `From`, `Date`, `Subject`, `Message-ID`, `MIME-Version`, `Content-Type: text/plain; charset=utf-8`.

The writer uses `mboxrd@z` as envelope sender when none is available (matches b4's convention).
