//! Product-facing Atlas connectivity snapshots shared by firmware and host code.

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
