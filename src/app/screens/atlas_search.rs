//! Deterministic e-paper renderer for the bounded Atlas Search surface.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds, UiTextStyle},
        widgets::{
            footer::draw_footer,
            header::draw_header,
            status_row::{draw_status_row, StatusRow},
        },
    },
    atlas_search::{AtlasSearchFocus, AtlasSearchState, SEARCH_KEY_ROWS, SEARCH_VISIBLE_ROWS},
    atlas_state::AtlasConnectionState,
    orientation::OrientedFrameBuffer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasSearchChrome {
    status: &'static str,
    source: &'static str,
    connection: &'static str,
}

impl AtlasSearchChrome {
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

#[must_use]
pub fn atlas_search_chrome(state: &AppState) -> AtlasSearchChrome {
    let search = &state.atlas_search;
    let has_results = !search.results().is_empty();
    let cached_source = if has_results { "CACHED" } else { "EMPTY" };
    match state.atlas_search_connection {
        AtlasConnectionState::Connected => AtlasSearchChrome {
            status: if search.query().is_empty() {
                "TYPE QUERY"
            } else if has_results {
                "READY"
            } else {
                "NO RESULTS"
            },
            source: "LIVE",
            connection: "ONLINE",
        },
        AtlasConnectionState::Offline => AtlasSearchChrome {
            status: if has_results {
                "OFFLINE CACHED"
            } else {
                "OFFLINE"
            },
            source: cached_source,
            connection: "OFFLINE",
        },
        AtlasConnectionState::Timeout => AtlasSearchChrome {
            status: if has_results { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "TIMEOUT",
        },
        AtlasConnectionState::Unconfigured => AtlasSearchChrome {
            status: "TYPE QUERY",
            source: cached_source,
            connection: "IDLE",
        },
        AtlasConnectionState::Connecting => AtlasSearchChrome {
            status: "SEARCHING",
            source: cached_source,
            connection: "CONNECTING",
        },
        AtlasConnectionState::Unauthorized => AtlasSearchChrome {
            status: if has_results { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "UNAUTHORIZED",
        },
        AtlasConnectionState::Forbidden => AtlasSearchChrome {
            status: if has_results { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "FORBIDDEN",
        },
        AtlasConnectionState::ServerError => AtlasSearchChrome {
            status: if has_results { "ERROR CACHED" } else { "ERROR" },
            source: cached_source,
            connection: "SERVER ERROR",
        },
    }
}

pub fn render_atlas_search(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let search = &state.atlas_search;
    let chrome = atlas_search_chrome(state);
    let body = state.display.body_style();
    let heading = state.display.heading_style();
    draw_header(display, state.display, "ATLAS LITE", "SEARCH")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: chrome.status(),
            middle: chrome.source(),
            right: chrome.connection(),
        },
    )?;
    Text::new("Query", Point::new(22, 158), heading).draw(display)?;
    Rectangle::new(Point::new(22, 176), Size::new(436, 52))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;
    let query = if search.query().is_empty() {
        "_"
    } else {
        search.query()
    };
    Text::new(query, Point::new(38, 210), body)
        .draw_clipped(display, TextBounds::new(38, 184, 442, 214))?;
    draw_results(display, search, body, heading)?;
    draw_keyboard(display, search, body, heading)?;
    draw_footer(
        display,
        state.display,
        "UP/DOWN MOVE  BOOT H/V  SELECT  HOLD BOOT BACK",
    )
}

fn draw_results(
    display: &mut OrientedFrameBuffer<'_>,
    search: &AtlasSearchState,
    body: UiTextStyle,
    heading: UiTextStyle,
) -> Result<(), Infallible> {
    Rectangle::new(Point::new(22, 242), Size::new(436, 224))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;
    if search.results().is_empty() {
        Text::new(
            if search.query().is_empty() {
                "TYPE A QUERY, THEN GO"
            } else {
                "NO MATCHING NOTES"
            },
            Point::new(38, 286),
            heading,
        )
        .draw(display)?;
        return Ok(());
    }
    let offset = search
        .window_offset()
        .min(search.results().len().saturating_sub(SEARCH_VISIBLE_ROWS));
    let end = (offset + SEARCH_VISIBLE_ROWS).min(search.results().len());
    for (row, result) in search.results()[offset..end].iter().enumerate() {
        let baseline = 274 + row as i32 * 36;
        let selected =
            search.focus() == AtlasSearchFocus::Results && offset + row == search.selected();
        let title = if selected {
            format!("> {}", result.title())
        } else {
            format!("  {}", result.title())
        };
        Text::new(
            &title,
            Point::new(32, baseline),
            if selected { heading } else { body },
        )
        .draw_clipped(
            display,
            TextBounds::new(32, baseline - 22, 448, baseline + 2),
        )?;
        Text::new(result.snippet(), Point::new(48, baseline + 16), body).draw_clipped(
            display,
            TextBounds::new(48, baseline + 2, 448, baseline + 19),
        )?;
    }
    Ok(())
}

fn draw_keyboard(
    display: &mut OrientedFrameBuffer<'_>,
    search: &AtlasSearchState,
    body: UiTextStyle,
    heading: UiTextStyle,
) -> Result<(), Infallible> {
    for (row, keys) in SEARCH_KEY_ROWS.iter().enumerate() {
        for (column, label) in keys.iter().enumerate() {
            let index = row * 6 + column;
            let left = 22 + column as i32 * 73;
            let top = 486 + row as i32 * 48;
            let selected = search.focus() == AtlasSearchFocus::Input
                && search.keyboard_navigation().selected() == index;
            Rectangle::new(Point::new(left, top), Size::new(68, 40))
                .into_styled(PrimitiveStyle::with_stroke(
                    BinaryColor::On,
                    if selected { 3 } else { 1 },
                ))
                .draw(display)?;
            Text::new(
                label,
                Point::new(left + if label.len() > 1 { 7 } else { 24 }, top + 27),
                if selected { heading } else { body },
            )
            .draw(display)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render_atlas_search;
    use crate::{app::AppState, framebuffer::FrameBuffer, orientation::OrientedFrameBuffer};

    #[test]
    fn search_screen_renders_without_network_work() {
        let mut frame = FrameBuffer::new_white();
        render_atlas_search(
            &mut OrientedFrameBuffer::new(&mut frame, Default::default()),
            &AppState::default(),
        )
        .unwrap();
    }
}
