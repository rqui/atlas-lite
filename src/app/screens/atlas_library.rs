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
    atlas_library::{LibraryCompleteness, LibraryHierarchy},
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

/// Flattens the already bounded tree for the small e-paper viewport.
#[must_use]
pub fn atlas_library_content(hierarchy: &LibraryHierarchy) -> AtlasLibraryContent {
    let mut entries = Vec::with_capacity(hierarchy.nodes().len());
    let mut pending: Vec<(&str, usize)> = hierarchy
        .root_ids()
        .iter()
        .rev()
        .map(|id| (id.as_str(), 0))
        .collect();

    while let Some((id, depth)) = pending.pop() {
        let Some(node) = hierarchy.nodes().iter().find(|node| node.id() == id) else {
            continue;
        };
        entries.push(format!("{}{}", "  ".repeat(depth), node.title()));
        for child_id in hierarchy.child_ids(id).iter().rev() {
            pending.push((child_id, depth + 1));
        }
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

/// Renders only data already owned by [`AppState`]; refresh stays explicit.
pub fn render_atlas_library(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_library_content(state.atlas_library.hierarchy());
    let body = state.display.body_style();
    let heading = state.display.heading_style();

    draw_header(display, state.display, "ATLAS LITE", "LIBRARY")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: content.status(),
            middle: "BOUNDED",
            right: "LOCAL",
        },
    )?;
    if content.entries().is_empty() {
        Text::new("NO NOTES LOADED", Point::new(22, 186), heading).draw(display)?;
    } else {
        for (index, entry) in content.entries().iter().take(12).enumerate() {
            let baseline = 186 + index as i32 * 36;
            let bounds =
                crate::app::typography::TextBounds::new(22, baseline - 24, 458, baseline + 4);
            Text::new(entry, Point::new(22, baseline), body).draw_clipped(display, bounds)?;
        }
    }
    draw_footer(display, state.display, "REFRESH ON ENTRY  HOLD BOOT BACK")
}
