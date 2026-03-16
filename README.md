# emx-mbox

Mbox format storage and management for mail-based chat history.

## Features

- Parse mbox format files
- Create and append new messages
- Save mbox files
- Built on top of `mailparse` crate for RFC 5322 email parsing

## Usage

```rust
use emx_mbox::{Mbox, MboxWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load existing mbox
    let mbox = Mbox::load_file("chat.mbox")?;
    
    // Or create from raw data
    let mbox_data = "From user@example.com Mon Mar 16 10:00:00 2026\n\
From: alice@example.com\n\
Subject: Hello\n\
\n\
Message body\n";
    let mbox = Mbox::load(mbox_data.as_bytes())?;

    // Iterate messages
    for msg in mbox.iter() {
        println!("From: {}", msg.from);
        println!("Subject: {}", msg.subject);
        println!("Body: {}", msg.body);
    }

    // Add new message
    let mut mbox = Mbox::load(mbox_data.as_bytes())?;
    mbox.add_message("bob@example.com", "Reply", "Hello back!");

    // Save to file
    mbox.save_file("chat.mbox")?;

    Ok(())
}
```

## Similar Go Projects

- [go-mbox](https://github.com/emersion/go-mbox) - Mbox file format parser
- [go-message](https://github.com/emersion/go-message) - RFC 5322 message parser

## License

MIT
