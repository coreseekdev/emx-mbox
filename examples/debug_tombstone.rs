use emx_mbox::{is_tombstone, MailStore, Mbox};

fn main() {
    let mbox = Mbox::load_file("test_delete.mbox").unwrap();

    println!("Total messages in mbox: {}", mbox.len());

    for (i, msg) in mbox.iter().enumerate() {
        let msg_id = msg.message_id().unwrap_or("no-id");
        let subject = msg.subject();
        let is_tomb = is_tombstone(msg);

        println!(
            "{}: {} | is_tombstone={} | Message-ID={}",
            i + 1,
            if subject.is_empty() { "(empty)" } else { subject },
            is_tomb,
            msg_id
        );

        // Debug: print all headers for tombstone candidates
        if is_tomb {
            println!("  -> Tombstone headers:");
            if let Some(status) = msg.header("X-LLM-Status") {
                println!("     X-LLM-Status: {}", status);
            }
            if let Some(del_id) = msg.header("X-LLM-Deleted-Message-ID") {
                println!("     X-LLM-Deleted-Message-ID: {}", del_id);
            }
        }
    }
}
