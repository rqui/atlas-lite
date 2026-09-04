//! Minimal, non-networked renderer for the bounded Atlas Library hierarchy.

use core::convert::Infallible;

use embedded_graphics::prelude::Point;

use crate::{
    app::{
        state::AppState,
        typography::Text,
        widgets::{
            footer::draw_footer,
            header::draw_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    atlas_library::{LibraryCompleteness, LibraryHierarchy, LIBRARY_VISIBLE_ROWS},
    atlas_state::AtlasConnectionState,
    orientation::OrientedFrameBuffer,
};

/// Display-only Library content built from an owned hierarchy snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasLibraryContent {
    status: &'static str,
    entries: Vec<String>,
}

impl AtlasLibraryContent {
    #[must_use]
    pub const fn status(&self) -> &'static str {
        self.status
    }

    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// Freshness/error/cache labels shown by the Library status strip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasLibraryChrome {
    status: &'static str,
    source: &'static str,
    connection: &'static str,
}

impl AtlasLibraryChrome {
    #[must_use]
    pub const fn status(self) -> &'static str {
        self.status
    }

    #[must_use]
    pub const fn source(self) -> &'static str {
        self.source
    }

    #[must_use]
    pub const fn connection(self) -> &'static str {
        self.connection
    }
}

/// Derive compact chrome from the last Atlas outcome without discarding the
/// previous bounded hierarchy on a refresh failure.
#[must_use]
pub fn atlas_library_chrome(
    state: &AppState,
    content: &AtlasLibraryContent,
) -> AtlasLibraryChrome {
    let has_cache = !content.entries().is_empty();
    let cached_source = if has_cache { "CACHED" } else { "EMPTY" };
    match state.atlas.connection {
        AtlasConnectionState::Connected => AtlasLibraryChrome {
            status: content.status(),
            source: "LIVE",
            connection: "ONLINE",
        },
        AtlasConnectionState::Offline => AtlasLibraryChrome {
            status: if has_cache { "OFFLINE CACHED" } else { "OFFLINE" },
            source: cached_source,
            connection: "OFFLINE",
        },
        AtlasConnectionState::Connecting => AtlasLibraryChrome {
            status: "SYNCING",
            source: cached_source,
            connection: "CONNECTING",
        },
        AtlasConnectionState::Unconfigured => AtlasLibraryChrome {
            status: "NOT CONFIGURED",
            source: cached_source,
            connection: "UNCONFIGURED",
        },
        AtlasConnectionState::Unauthorized => AtlasLibraryChrome {
            status: if has_cache { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "UNAUTHORIZED",
        },
        AtlasConnectionState::Forbidden => AtlasLibraryChrome {
            status: if has_cache { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "FORBIDDEN",
        },
        AtlasConnectionState::Timeout => AtlasLibraryChrome {
            status: if has_cache { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "TIMEOUT",
        },
        AtlasConnectionState::ServerError => AtlasLibraryChrome {
            status: if has_cache { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "SERVER ERROR",
        },
    }
}

/// Flattens the already bounded tree for the small e-paper viewport.
#[must_use]
pub fn atlas_library_content(hierarchy: &LibraryHierarchy) -> AtlasLibraryContent {
    let mut entries = Vec::with_capacity(hierarchy.nodes().len());
    for id in hierarchy.visible_ids() {
        let Some(node) = hierarchy.nodes().iter().find(|node| node.id() == id) else {
            continue;
        };
        let depth = node_depth(hierarchy, node.id());
        entries.push(format!("{}{}", "  ".repeat(depth), node.title()));
    }

    AtlasLibraryContent {
        status: match hierarchy.completeness() {
            LibraryCompleteness::Complete => "READY",
            LibraryCompleteness::CursorRemaining | LibraryCompleteness::NodeBudgetReached => {
                "PARTIAL"
            }
        },
        entries,
    }
}

fn node_depth(hierarchy: &LibraryHierarchy, id: &str) -> usize {
    let mut depth = 0;
    let mut current = hierarchy
        .nodes()
        .iter()
        .find(|node| node.id() == id)
        .and_then(|node| node.parent_id());
    while let Some(parent_id) = current {
        depth += 1;
        current = hierarchy
            .nodes()
            .iter()
            .find(|node| node.id() == parent_id)
            .and_then(|node| node.parent_id());
    }
    depth
}

/// Renders only data already owned by [`AppState`]; refresh stays explicit.
pub fn render_atlas_library(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_library_content(state.atlas_library.hierarchy());
    let chrome = atlas_library_chrome(state, &content);
    let body = state.display.body_style();
    let heading = state.display.heading_style();

    draw_header(display, state.display, "ATLAS LITE", "LIBRARY")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: chrome.status(),
            middle: chrome.source(),
            right: chrome.connection(),
        },
    )?;
    if content.entries().is_empty() {
        Text::new("NO NOTES LOADED", Point::new(22, 186), heading).draw(display)?;
    } else {
        let max_offset = content.entries().len().saturating_sub(LIBRARY_VISIBLE_ROWS);
        let offset = state.atlas_library_window_offset.min(max_offset);
        let selected = state
            .atlas_library_selected
            .min(content.entries().len().saturating_sub(1));
        let end = (offset + LIBRARY_VISIBLE_ROWS).min(content.entries().len());
        for (row, entry) in content.entries()[offset..end].iter().enumerate() {
            let baseline = 186 + row as i32 * 36;
            let bounds =
                crate::app::typography::TextBounds::new(22, baseline - 24, 458, baseline + 4);
            let label = if row + offset == selected {
                format!("> {entry}")
            } else {
                format!("  {entry}")
            };
            Text::new(&label, Point::new(22, baseline), body).draw_clipped(display, bounds)?;
        }
    }
    draw_footer(display, state.display, "REFRESH ON ENTRY  HOLD BOOT BACK")
}
