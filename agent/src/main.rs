//! Arna agent — headless binary (capture + control + consent via policy).
//!
//! The reusable agent loop lives in the library (`arna_agent::run`); this
//! binary just supplies a terminal/policy consent decision. The Tauri desktop
//! app supplies a popup-window consent instead.
//!
//!   # backend:  cargo run --manifest-path backend/Cargo.toml
//!   # agent:    cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
//!   # console:  cd console && npm install && npm run dev  ->  http://localhost:4310
//!
//! (Run the agent with --release for smooth capture.)

use std::sync::Arc;

use arna_agent::{session_code, ChatBridge, Consent, ConnectRequest, ConsentFn};
use tokio::io::AsyncBufReadExt;

/// What the agent does when a console asks to connect. Set with `ARNA_CONSENT`:
/// `accept` (default — auto-admit), `prompt` (ask y/n on the terminal), or
/// `decline` (always refuse). The Tauri app replaces this with a popup window.
#[derive(Clone, Copy)]
enum ConsentPolicy {
    Accept,
    Prompt,
    Decline,
}

fn consent_policy() -> ConsentPolicy {
    match std::env::var("ARNA_CONSENT").as_deref() {
        Ok("prompt") => ConsentPolicy::Prompt,
        Ok("decline") => ConsentPolicy::Decline,
        _ => ConsentPolicy::Accept,
    }
}

/// Read a single y/n answer from the terminal without blocking the async runtime.
async fn ask_terminal(prompt: String) -> bool {
    tokio::task::spawn_blocking(move || {
        use std::io::{BufRead, Write};
        print!("{prompt} [y/N] ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().lock().read_line(&mut line);
        matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
    })
    .await
    .unwrap_or(false)
}

fn build_consent(policy: ConsentPolicy) -> ConsentFn {
    Arc::new(move |req: ConnectRequest| {
        Box::pin(async move {
            let code = session_code();
            match policy {
                ConsentPolicy::Accept => {
                    println!(
                        "agent: auto-accepting {} ({}) — session code {code}",
                        req.name, req.from
                    );
                    Consent::Accept { code: Some(code) }
                }
                ConsentPolicy::Decline => Consent::Decline {
                    reason: "operator policy: connections disabled".into(),
                },
                ConsentPolicy::Prompt => {
                    println!("agent: session code for {} is {code}", req.name);
                    if ask_terminal(format!("Allow {} ({}) to connect?", req.name, req.from)).await
                    {
                        Consent::Accept { code: Some(code) }
                    } else {
                        Consent::Decline {
                            reason: "declined by operator".into(),
                        }
                    }
                }
            }
        })
    })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "ws://127.0.0.1:8081/ws".to_string());
    let id = args.get(2).cloned().unwrap_or_else(|| "agent-1".to_string());

    let consent = build_consent(consent_policy());

    // Chat: print what the admin sends; send whatever the operator types here.
    // (Lines from stdin go to the connected console.)
    let chat = ChatBridge::new(|text| println!("\n[chat] admin: {text}"));
    {
        let chat = chat.clone();
        tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    chat.send(&line).await;
                }
            }
        });
    }

    arna_agent::run(url, id, consent, chat).await;
}
