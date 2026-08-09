//! Watch & Control — the enforcement core (host-side). A host shares a workspace
//! SURFACE with guests; guest input arrives here and is INJECTED into the OS only
//! for the single Controller. A Viewer's input is rejected at this gate, not
//! merely hidden in a UI. Product/session layer — no engine or WRM change.
//! See docs/watch-and-control.md.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Viewer,
    Controller,
}

impl Role {
    fn label(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Controller => "controller",
        }
    }
}

struct Guest {
    name: String,
    role: Role,
}

/// A live session for one workspace: its connected guests + the single guest (if
/// any) currently holding control.
#[derive(Default)]
struct Session {
    guests: HashMap<String, Guest>,
    controller: Option<String>,
}

impl Session {
    fn join(&mut self, guest: &str, name: &str) {
        self.guests
            .entry(guest.to_string())
            .or_insert_with(|| Guest { name: name.to_string(), role: Role::Viewer })
            .name = name.to_string();
    }

    /// Promote one guest to Controller, demoting any previous one — at most one
    /// Controller at a time.
    fn grant(&mut self, guest: &str) -> bool {
        if !self.guests.contains_key(guest) {
            return false;
        }
        self.revoke();
        if let Some(g) = self.guests.get_mut(guest) {
            g.role = Role::Controller;
        }
        self.controller = Some(guest.to_string());
        true
    }

    /// Revoke control: the current Controller (if any) drops to Viewer.
    fn revoke(&mut self) {
        if let Some(prev) = self.controller.take() {
            if let Some(g) = self.guests.get_mut(&prev) {
                g.role = Role::Viewer;
            }
        }
    }

    /// Remove a guest entirely — access ends immediately.
    fn disconnect(&mut self, guest: &str) {
        self.guests.remove(guest);
        if self.controller.as_deref() == Some(guest) {
            self.controller = None;
        }
    }

    /// THE gate: may this guest send input right now? Only the sole Controller.
    fn can_send_input(&self, guest: &str) -> bool {
        self.controller.as_deref() == Some(guest)
            && matches!(self.guests.get(guest).map(|g| g.role), Some(Role::Controller))
    }
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static S: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with<R>(ws: &str, f: impl FnOnce(&mut Session) -> R) -> R {
    let mut map = sessions().lock().unwrap();
    f(map.entry(ws.to_string()).or_default())
}

// ── public API (driven by Tauri commands) ────────────────────────────────────
pub fn join(ws: &str, guest: &str, name: &str) {
    with(ws, |s| s.join(guest, name));
}
pub fn grant(ws: &str, guest: &str) -> bool {
    with(ws, |s| s.grant(guest))
}
pub fn revoke(ws: &str) {
    with(ws, |s| s.revoke());
}
pub fn disconnect(ws: &str, guest: &str) {
    with(ws, |s| s.disconnect(guest));
}

/// Gated input: inject into the OS only if the guest is the Controller. Returns
/// whether it was injected (false = rejected at the enforcement point).
pub fn input(ws: &str, guest: &str, event_json: &str) -> bool {
    let allowed = with(ws, |s| s.can_send_input(guest));
    if !allowed {
        return false;
    }
    if let Ok(ev) = serde_json::from_str::<InputEvent>(event_json) {
        inject(&ev);
    }
    true
}

/// The session state for the host UI: each guest's role + who holds control.
pub fn state_json(ws: &str) -> String {
    let map = sessions().lock().unwrap();
    let Some(s) = map.get(ws) else {
        return "{\"guests\":[],\"controller\":null}".into();
    };
    let guests: Vec<_> = s
        .guests
        .iter()
        .map(|(id, g)| {
            serde_json::json!({ "id": id, "name": g.name, "role": g.role.label() })
        })
        .collect();
    serde_json::json!({
        "guests": guests,
        "controller": s.controller,
    })
    .to_string()
}

// ── OS input injection (Windows SendInput) ───────────────────────────────────
// Reached ONLY through the gate above.
#[derive(Clone, Copy)]
#[repr(C)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    dw_flags: u32,
    time: u32,
    extra: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct KeybdInput {
    w_vk: u16,
    w_scan: u16,
    dw_flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
union InputUnion {
    mi: MouseInput,
    ki: KeybdInput,
}

#[repr(C)]
struct Input {
    r#type: u32,
    u: InputUnion,
}

const INPUT_MOUSE: u32 = 0;
const INPUT_KEYBOARD: u32 = 1;
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
const KEYEVENTF_KEYUP: u32 = 0x0002;

#[link(name = "user32")]
extern "system" {
    fn SendInput(n: u32, inputs: *const Input, cb: i32) -> u32;
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind")]
enum InputEvent {
    /// Pointer move to a normalised position (0..1) across the primary display.
    #[serde(rename = "move")]
    Move { x: f64, y: f64 },
    /// Mouse button ("left" | "right" | "middle") down/up.
    #[serde(rename = "button")]
    Button { button: String, down: bool },
    /// Key by Windows virtual-key code, down/up.
    #[serde(rename = "key")]
    Key { vk: u16, down: bool },
    /// Vertical wheel; positive scrolls up.
    #[serde(rename = "scroll")]
    Scroll { dy: i32 },
}

fn send_mouse(mi: MouseInput) {
    let input = Input { r#type: INPUT_MOUSE, u: InputUnion { mi } };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<Input>() as i32);
    }
}

fn send_key(ki: KeybdInput) {
    let input = Input { r#type: INPUT_KEYBOARD, u: InputUnion { ki } };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<Input>() as i32);
    }
}

fn inject(ev: &InputEvent) {
    match ev {
        InputEvent::Move { x, y } => send_mouse(MouseInput {
            dx: (x.clamp(0.0, 1.0) * 65535.0) as i32,
            dy: (y.clamp(0.0, 1.0) * 65535.0) as i32,
            mouse_data: 0,
            dw_flags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
            time: 0,
            extra: 0,
        }),
        InputEvent::Button { button, down } => {
            let flag = match (button.as_str(), down) {
                ("left", true) => MOUSEEVENTF_LEFTDOWN,
                ("left", false) => MOUSEEVENTF_LEFTUP,
                ("right", true) => MOUSEEVENTF_RIGHTDOWN,
                ("right", false) => MOUSEEVENTF_RIGHTUP,
                ("middle", true) => MOUSEEVENTF_MIDDLEDOWN,
                ("middle", false) => MOUSEEVENTF_MIDDLEUP,
                _ => return,
            };
            send_mouse(MouseInput { dx: 0, dy: 0, mouse_data: 0, dw_flags: flag, time: 0, extra: 0 });
        }
        InputEvent::Key { vk, down } => send_key(KeybdInput {
            w_vk: *vk,
            w_scan: 0,
            dw_flags: if *down { 0 } else { KEYEVENTF_KEYUP },
            time: 0,
            extra: 0,
        }),
        InputEvent::Scroll { dy } => send_mouse(MouseInput {
            dx: 0,
            dy: 0,
            mouse_data: (dy * 120) as u32,
            dw_flags: MOUSEEVENTF_WHEEL,
            time: 0,
            extra: 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The acceptance test's invariants, at the enforcement point (no transport).
    #[test]
    fn viewer_cannot_send_input_until_granted() {
        let mut s = Session::default();
        s.join("g1", "Guest One");
        assert!(!s.can_send_input("g1"), "a fresh guest is a Viewer: input rejected");

        assert!(s.grant("g1"));
        assert!(s.can_send_input("g1"), "granted Controller may send input");

        s.revoke();
        assert!(!s.can_send_input("g1"), "revoke immediately removes input");
    }

    #[test]
    fn only_one_controller_at_a_time() {
        let mut s = Session::default();
        s.join("a", "A");
        s.join("b", "B");
        s.grant("a");
        s.grant("b"); // promoting b demotes a
        assert!(s.can_send_input("b"));
        assert!(!s.can_send_input("a"), "the previous controller is demoted");
    }

    #[test]
    fn disconnect_ends_access_immediately() {
        let mut s = Session::default();
        s.join("g", "G");
        s.grant("g");
        assert!(s.can_send_input("g"));
        s.disconnect("g");
        assert!(!s.can_send_input("g"), "a disconnected guest has no access");
        assert!(s.grant("g") == false, "and cannot be granted control while gone");
    }

    #[test]
    fn granting_an_unknown_guest_is_refused() {
        let mut s = Session::default();
        assert!(!s.grant("nobody"));
    }
}
