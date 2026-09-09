//! Minimal, non-networked renderer for the bounded Atlas Library hierarchy.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds, UiTextRole},
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
        let hierarchy = state.atlas_library.hierarchy();
        let visible_ids = hierarchy.visible_ids_with_expanded(&state.atlas_library_expanded);
        let max_offset = content.entries().len().saturating_sub(LIBRARY_VISIBLE_ROWS);
        let offset = state.atlas_library_window_offset.min(max_offset);
        let selected = state
            .atlas_library_selected
            .min(content.entries().len().saturating_sub(1));
        let end = (offset + LIBRARY_VISIBLE_ROWS).min(content.entries().len());
        for (row, id) in visible_ids[offset..end].iter().enumerate() {
            let Some(node) = hierarchy.nodes().iter().find(|node| node.id() == *id) else {
                continue;
            };
            let baseline = 140 + row as i32 * 50;
            let is_selected = row + offset == selected;
            let row_bounds = Rectangle::new(Point::new(18, baseline - 35), Size::new(440, 46));
            row_bounds
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(display)?;
            draw_selection_chrome(display, row_bounds, is_selected)?;
            let depth = node_depth(hierarchy, node.id()).min(6) as i32;
            let marker_x = 42 + depth * 18;
            let children = hierarchy.child_ids(node.id()).len();
            if children > 0 {
                let marker = Rectangle::new(Point::new(marker_x, baseline - 23), Size::new(20, 20));
                marker
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                    .draw(display)?;
                Line::new(
                    Point::new(marker_x + 5, baseline - 13),
                    Point::new(marker_x + 15, baseline - 13),
                )
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
                .draw(display)?;
                if !state
                    .atlas_library_expanded
                    .iter()
                    .any(|expanded| expanded == node.id())
                {
                    Line::new(
                        Point::new(marker_x + 10, baseline - 18),
                        Point::new(marker_x + 10, baseline - 8),
                    )
                    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
                    .draw(display)?;
                }

                let badge = Rectangle::new(Point::new(408, baseline - 25), Size::new(34, 24));
                badge
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
                Text::new(
                    &children.to_string(),
                    Point::new(415, baseline - 5),
                    state
                        .display
                        .text_style(UiTextRole::Detail, BinaryColor::Off),
                )
                .draw_clipped(
                    display,
                    TextBounds::new(412, baseline - 24, 439, baseline - 2),
                )?;
            } else {
                Circle::new(Point::new(marker_x + 7, baseline - 16), 6)
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
            }
            let title_x = marker_x + 30;
            Text::new(
                node.title(),
                Point::new(title_x, baseline),
                state.display.heading_style(),
            )
            .draw_clipped(
                display,
                TextBounds::new(title_x, baseline - 31, 398, baseline + 5),
            )?;
        }
    }
    draw_footer(
        display,
        state.display,
        "SELECT EXPAND  HOLD SELECT OPEN  BOOT BACK",
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

        // Shared 34 px e-paper mark is visibly white on the solid black header.
        let mut white = 0;
        for y in 7..41 {
            for x in 8..42 {
                let pixel = orientation.map_logical_to_native(Point::new(x, y)).unwrap();
                white += usize::from(frame.is_black(pixel) == Some(false));
            }
        }
        assert!(white > 100);
    }
}
