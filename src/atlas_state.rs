//! Product-facing Atlas connectivity snapshots shared by firmware and host code.

/// The number of recent notes retained for the compact Home surface.
///
/// This remains below the bounded DTO/client page maximum and is deliberately
/// smaller than the screen can scroll: Home is a summary, not a Library.
pub const HOME_RECENT_NOTE_LIMIT: usize = 3;
/// The number of View shortcuts retained for the compact Home surface.
pub const HOME_VIEW_SHORTCUT_LIMIT: usize = 2;
const HOME_LABEL_MAX_CHARS: usize = 50;

/// Bounded Atlas connection state used by product screens and host fakes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AtlasConnectionState {
    #[default]
    Unconfigured,
    Connecting,
    Connected,
    Unauthorized,
    Forbidden,
    Timeout,
    ServerError,
    Offline,
}

impl AtlasConnectionState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Timeout => "timeout",
            Self::ServerError => "server_error",
            Self::Offline => "offline",
        }
    }
}

/// Secret-free Atlas connectivity snapshot consumed by [`crate::app::AppState`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AtlasSnapshot {
    pub connection: AtlasConnectionState,
}

/// Secret-free, display-ready Atlas data retained by the Home surface.
///
/// The model stores only short labels needed by Home. IDs, paths, revisions,
/// tokens, and arbitrary View values remain at the client boundary until a
/// later screen explicitly needs them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AtlasHomeSnapshot {
    recent_notes: Vec<String>,
    view_shortcuts: Vec<String>,
}

impl AtlasHomeSnapshot {
    #[must_use]
    pub fn recent_notes(&self) -> &[String] {
        &self.recent_notes
    }

    #[must_use]
    pub fn view_shortcuts(&self) -> &[String] {
        &self.view_shortcuts
    }

    pub(crate) fn replace_recent_notes<I>(&mut self, labels: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.recent_notes = labels
            .into_iter()
            .take(HOME_RECENT_NOTE_LIMIT)
            .map(|label| home_label(&label))
            .collect();
    }

    pub(crate) fn replace_view_shortcuts<I>(&mut self, labels: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.view_shortcuts = labels
            .into_iter()
            .take(HOME_VIEW_SHORTCUT_LIMIT)
            .map(|label| home_label(&label))
            .collect();
    }
}

fn home_label(label: &str) -> String {
    let mut characters = label.chars();
    let visible: String = characters.by_ref().take(HOME_LABEL_MAX_CHARS - 1).collect();
    if characters.next().is_some() {
        format!("{visible}…")
    } else {
        visible
    }
}
