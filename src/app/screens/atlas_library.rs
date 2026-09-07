//! Minimal, non-networked renderer for the bounded Atlas Library hierarchy.

use core::convert::Infallible;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::{
    app::{
        state::AppState,
        typography::Text,
        widgets::{
            footer::draw_footer, header::draw_atlas_topbar, selection::draw_selection_chrome,
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
pub fn atlas_library_chrome(state: &AppState, content: &AtlasLibraryContent) -> AtlasLibraryChrome {
    let has_cache = !content.entries().is_empty();
    let cached_source = if has_cache { "CACHED" } else { "EMPTY" };
    match state.atlas_library_connection {
        AtlasConnectionState::Connected => AtlasLibraryChrome {
            status: content.status(),
            source: "LIVE",
            connection: "ONLINE",
        },
        AtlasConnectionState::Offline => AtlasLibraryChrome {
            status: if has_cache {
                "OFFLINE CACHED"
            } else {
                "OFFLINE"
            },
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
pub fn atlas_library_content(
    hierarchy: &LibraryHierarchy,
    expanded_ids: &[String],
) -> AtlasLibraryContent {
    let mut entries = Vec::with_capacity(hierarchy.nodes().len());
    for id in hierarchy.visible_ids_with_expanded(expanded_ids) {
        let Some(node) = hierarchy.nodes().iter().find(|node| node.id() == id) else {
            continue;
        };
        let depth = node_depth(hierarchy, node.id());
        let marker = if hierarchy.has_children(id) {
            if expanded_ids.iter().any(|expanded_id| expanded_id == id) {
                "- "
            } else {
                "+ "
            }
        } else {
            "  "
        };
        entries.push(format!("{}{marker}{}", "  ".repeat(depth), node.title()));
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
    let content = atlas_library_content(
        state.atlas_library.hierarchy(),
        &state.atlas_library_expanded,
    );
    let chrome = atlas_library_chrome(state, &content);
    let heading = state.display.heading_style();

    draw_atlas_topbar(display, state, "LIBRARY")?;
    let notice = match state.atlas_library_connection {
        AtlasConnectionState::Connecting => Some("Loading…"),
        AtlasConnectionState::Unauthorized | AtlasConnectionState::Forbidden => {
            Some("Authorization required")
        }
        AtlasConnectionState::Offline | AtlasConnectionState::Timeout
            if !content.entries().is_empty() =>
        {
            Some("Offline — showing saved notes")
        }
        AtlasConnectionState::ServerError if !content.entries().is_empty() => {
            Some("Unable to refresh — showing saved notes")
        }
        AtlasConnectionState::Connected if chrome.status() == "PARTIAL" => {
            Some("Some notes may be missing")
        }
        _ => None,
    };
    if let Some(notice) = notice {
        Text::new(notice, Point::new(22, 98), state.display.detail_style()).draw(display)?;
    }
    if content.entries().is_empty() {
        let message = match state.atlas_library_connection {
            AtlasConnectionState::Connecting => "Loading…",
            AtlasConnectionState::Connected => "No notes",
            AtlasConnectionState::Unconfigured => "Atlas setup required",
            AtlasConnectionState::Unauthorized | AtlasConnectionState::Forbidden => {
                "Authorization required"
            }
            AtlasConnectionState::Offline | AtlasConnectionState::Timeout => {
                "Offline — Select to retry"
            }
            AtlasConnectionState::ServerError => "Unable to load — Select to retry",
        };
        Text::new(message, Point::new(22, 146), heading).draw(display)?;
    } else {
        let max_offset = content.entries().len().saturating_sub(LIBRARY_VISIBLE_ROWS);
        let offset = state.atlas_library_window_offset.min(max_offset);
        let selected = state
            .atlas_library_selected
            .min(content.entries().len().saturating_sub(1));
        let end = (offset + LIBRARY_VISIBLE_ROWS).min(content.entries().len());
        for (row, entry) in content.entries()[offset..end].iter().enumerate() {
            let baseline = 132 + row as i32 * 36;
            let is_selected = row + offset == selected;
            draw_selection_chrome(
                display,
                Rectangle::new(Point::new(18, baseline - 27), Size::new(440, 33)),
                is_selected,
            )?;
            let bounds =
                crate::app::typography::TextBounds::new(50, baseline - 24, 390, baseline + 4);
            Text::new(entry, Point::new(50, baseline), heading).draw_clipped(display, bounds)?;
            if let Some(node) = state
                .atlas_library
                .hierarchy()
                .nodes()
                .iter()
                .find(|node| entry.ends_with(node.title()))
            {
                let children = state.atlas_library.hierarchy().child_ids(node.id()).len();
                if children > 0 {
                    let count = children.to_string();
                    Text::new(
                        &count,
                        Point::new(430, baseline),
                        state.display.detail_style(),
                    )
                    .draw_clipped(
                        display,
                        crate::app::typography::TextBounds::new(
                            404,
                            baseline - 18,
                            454,
                            baseline + 4,
                        ),
                    )?;
                }
            }
        }
    }
    draw_footer(
        display,
        state.display,
        "SELECT OPEN / RETRY  HOLD BOOT BACK",
    )
}

#[cfg(test)]
mod tests {
    use embedded_graphics::prelude::Point;

    use super::render_atlas_library;
    use crate::{
        app::AppState,
        framebuffer::FrameBuffer,
        orientation::{DisplayOrientation, OrientedFrameBuffer},
    };

    #[test]
    fn library_uses_the_same_atlas_brand_header_as_home() {
        let orientation = DisplayOrientation::Portrait;
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
        render_atlas_library(&mut display, &AppState::default()).unwrap();
        drop(display);

        // Existing real bitmap: row 5, column 10 at origin (18, 15).
        let logo_pixel = orientation
            .map_logical_to_native(Point::new(28, 20))
            .unwrap();
        assert_eq!(frame.is_black(logo_pixel), Some(true));
    }
}
