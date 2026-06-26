//! Arna Agent desktop shell.
//!
//! Runs the reusable agent loop (`arna_agent::run`) in the background and gives
//! it a GUI **consent**: when a console asks to connect, a small always-on-top
//! popup window appears with the admin's name, a 6-digit session code, and
//! Accept/Decline. The button's choice flows back to the agent through a oneshot
//! channel, gating the WebRTC session.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arna_agent::{session_code, ChatBridge, Consent, ConnectRequest, ConsentFn};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tokio::sync::{oneshot, Mutex};

/// Pending consent decisions, keyed by the requesting console id. The popup's
/// `respond_consent` command fulfils the matching sender.
type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Consent>>>>;

// ---------------------------------------------------------------------------
// Chat window
// ---------------------------------------------------------------------------

/// One chat message in the session log. `id` (its index) lets the window dedupe
/// the initial history against live events.
#[derive(Clone, serde::Serialize)]
struct ChatMsg {
    id: u64,
    mine: bool,
    text: String,
}

/// In-session chat log, so a freshly-opened chat window can show what came before.
type ChatLog = Arc<std::sync::Mutex<Vec<ChatMsg>>>;

/// Append a message and return it (with its assigned id).
fn push_msg(log: &ChatLog, mine: bool, text: String) -> ChatMsg {
    let mut l = log.lock().expect("chat log");
    let msg = ChatMsg {
        id: l.len() as u64,
        mine,
        text,
    };
    l.push(msg.clone());
    msg
}

/// Create the chat window if it isn't already open (and focus it).
fn ensure_chat_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("chat") {
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "chat", WebviewUrl::App("index.html?view=chat".into()))
        .title("Arna — chat")
        .inner_size(380.0, 520.0)
        .min_inner_size(320.0, 380.0)
        .build();
}

/// The chat window fetches the existing log on open.
#[tauri::command]
fn chat_history(log: tauri::State<'_, ChatLog>) -> Vec<ChatMsg> {
    log.lock().map(|l| l.clone()).unwrap_or_default()
}

/// Send a reply from the store to the connected console.
#[tauri::command]
async fn send_chat(
    app: AppHandle,
    chat: tauri::State<'_, ChatBridge>,
    log: tauri::State<'_, ChatLog>,
    text: String,
) -> Result<(), String> {
    let msg = push_msg(&log, true, text.clone());
    let _ = app.emit("chat://msg", &msg);
    chat.send(&text).await;
    Ok(())
}

/// Window label for a console's consent popup (ids are safe: `viewer-<n>`).
fn consent_label(console_id: &str) -> String {
    format!("consent-{console_id}")
}

/// Invoked by the popup's Accept/Decline buttons.
#[tauri::command]
async fn respond_consent(
    app: AppHandle,
    pending: tauri::State<'_, Pending>,
    id: String,
    accept: bool,
    code: Option<String>,
) -> Result<(), String> {
    if let Some(tx) = pending.lock().await.remove(&id) {
        let decision = if accept {
            Consent::Accept { code }
        } else {
            Consent::Decline {
                reason: "declined by operator".into(),
            }
        };
        let _ = tx.send(decision);
    }
    if let Some(win) = app.get_webview_window(&consent_label(&id)) {
        let _ = win.close();
    }
    Ok(())
}

/// Build the agent's [`ConsentFn`]: each request pops an always-on-top window and
/// awaits the operator's choice (auto-declining after 60s of no response).
fn make_consent(app: AppHandle, pending: Pending) -> ConsentFn {
    Arc::new(move |req: ConnectRequest| {
        let app = app.clone();
        let pending = pending.clone();
        Box::pin(async move {
            let code = session_code();
            let label = consent_label(&req.from);

            let (tx, rx) = oneshot::channel();
            pending.lock().await.insert(req.from.clone(), tx);

            // Pass the request details to the popup via the query string.
            let url = format!(
                "index.html?from={}&name={}&code={}",
                urlencoding::encode(&req.from),
                urlencoding::encode(&req.name),
                code,
            );
            let built = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
                .title("Arna — connection request")
                .inner_size(420.0, 300.0)
                .resizable(false)
                .always_on_top(true)
                .center()
                .build();
            if let Err(e) = built {
                pending.lock().await.remove(&req.from);
                return Consent::Decline {
                    reason: format!("could not show consent popup: {e}"),
                };
            }

            match tokio::time::timeout(Duration::from_secs(60), rx).await {
                Ok(Ok(decision)) => decision,
                _ => {
                    // Timed out or the window closed without answering.
                    pending.lock().await.remove(&req.from);
                    if let Some(win) = app.get_webview_window(&label) {
                        let _ = win.close();
                    }
                    Consent::Decline {
                        reason: "no response (timed out)".into(),
                    }
                }
            }
        })
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                // The WS/WebRTC stacks are extremely chatty at trace/debug.
                .level_for("tungstenite", log::LevelFilter::Warn)
                .level_for("tokio_tungstenite", log::LevelFilter::Warn)
                .build(),
        )
        .manage(Arc::new(Mutex::new(
            HashMap::<String, oneshot::Sender<Consent>>::new(),
        )) as Pending)
        .manage(Arc::new(std::sync::Mutex::new(Vec::<ChatMsg>::new())) as ChatLog)
        .invoke_handler(tauri::generate_handler![respond_consent, chat_history, send_chat])
        .setup(|app| {
            // Tray icon: the agent runs in the background; the menu offers Quit.
            let quit = MenuItem::with_id(app, "quit", "Quit Arna Agent", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("bundled icon").clone())
                .tooltip("Arna Agent")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            // Start the agent loop in the background with popup-driven consent.
            let handle = app.handle().clone();
            let pending = app.state::<Pending>().inner().clone();
            let consent = make_consent(handle, pending);

            let url = std::env::var("ARNA_BACKEND")
                .unwrap_or_else(|_| "ws://127.0.0.1:8081/ws".to_string());
            let id = std::env::var("ARNA_AGENT_ID").unwrap_or_else(|_| "agent-1".to_string());

            // Chat: log the message, make sure the chat window is open, and push
            // it to the window. The window dedupes history vs. live events by id.
            let chat_log = app.state::<ChatLog>().inner().clone();
            let chat_handle = app.handle().clone();
            let chat = ChatBridge::new(move |text| {
                let msg = push_msg(&chat_log, false, text);
                let handle = chat_handle.clone();
                let _ = chat_handle.run_on_main_thread(move || {
                    ensure_chat_window(&handle);
                    let _ = handle.emit("chat://msg", &msg);
                });
            });
            app.manage(chat.clone());

            // Download: when the console asks for a file, show a native picker so
            // the operator chooses what leaves their PC.
            let download: arna_agent::DownloadProvider = Arc::new(|| {
                Box::pin(async {
                    let file = rfd::AsyncFileDialog::new()
                        .set_title("Choose a file to send")
                        .pick_file()
                        .await?;
                    Some(arna_agent::DownloadFile {
                        name: file.file_name(),
                        bytes: file.read().await,
                    })
                })
            });

            tauri::async_runtime::spawn(async move {
                arna_agent::run(url, id, consent, chat, download).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Arna Agent");
}
