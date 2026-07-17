//! Mock data only — no backend, no persistence. Stand-ins so the UI has
//! something to render while the real platform is built underneath it.

pub struct User {
    pub name: String,
    pub email: String,
    pub role: String,
}

pub struct Notification {
    pub title: String,
    pub body: String,
    pub time: String,
    pub read: bool,
}

pub struct Friend {
    pub name: String,
    pub online: bool,
}

/// Workspaces the user owns. Empty on purpose — the app starts with none.
pub struct Workspace {
    pub name: String,
    pub state: String,
}

pub fn user() -> User {
    User {
        name: "Tarun Matta".into(),
        email: "tarun@arna.dev".into(),
        role: "Host".into(),
    }
}

pub fn notifications() -> Vec<Notification> {
    vec![
        Notification {
            title: "Welcome to Arna".into(),
            body: "Your platform is ready. Create your first workspace to lend some compute.".into(),
            time: "just now".into(),
            read: false,
        },
        Notification {
            title: "Friend request".into(),
            body: "Aisha wants to connect with you.".into(),
            time: "2h ago".into(),
            read: false,
        },
        Notification {
            title: "Tip".into(),
            body: "Try Light mode from Settings → Appearance.".into(),
            time: "yesterday".into(),
            read: true,
        },
    ]
}

pub fn friends() -> Vec<Friend> {
    vec![
        Friend { name: "Aisha".into(), online: true },
        Friend { name: "Marco".into(), online: true },
        Friend { name: "Devan".into(), online: false },
    ]
}

/// No workspaces yet — the list starts empty by design.
pub fn workspaces() -> Vec<Workspace> {
    Vec::new()
}
