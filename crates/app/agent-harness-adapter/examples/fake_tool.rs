//! A tool for the bridge's tests: reads the hook input on stdin and answers
//! as its first argument says.
//!
//! - `allow` / `deny` / `continue` / `context`: that answer; `context`'s text
//!   is the input as read
//! - `fail`: exits 3
//! - `abort`: dies (by a signal where there are signals)
//! - `garbage`: writes something that is not an answer

use std::io::Read;

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let mode = std::env::args().nth(1).unwrap_or_default();
    eprint!("fake-tool ran ");
    let answer = match mode.as_str() {
        "allow" => serde_json::json!({"answer": "allow"}),
        "deny" => serde_json::json!({"answer": "deny", "reason": "no"}),
        "continue" => serde_json::json!({"answer": "continue", "reason": "go on"}),
        "context" => serde_json::json!({"answer": "context", "text": input.trim_end()}),
        "fail" => std::process::exit(3),
        "abort" => std::process::abort(),
        _ => {
            print!("not an answer");
            return;
        }
    };
    println!("{answer}");
}
