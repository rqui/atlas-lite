//! E-paper-native list and paginated-result rendering for Atlas Views.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
    Drawable,
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
    atlas_state::AtlasConnectionState,
    atlas_views::{AtlasViewsFocus, AtlasViewsState, VIEW_VISIBLE_ROWS},
    orientation::OrientedFrameBuffer,
};

const VIEW_HEADER_MAX_WIDTH: i32 = 314;

pub fn render_atlas_views(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let views = &state.atlas_views;
    let body = state.display.body_style();
    let heading = state.display.heading_style();
    draw_header(display, state.display, "ATLAS LITE", "VIEWS")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: views_status(state),
            middle: views_source(state),
            right: views_page_label(views),
        },
    )?;
    match views.focus() {
        AtlasViewsFocus::List => draw_view_list(display, views, body, heading)?,
        AtlasViewsFocus::Results => draw_results(display, views, body, heading)?,
    }
    draw_footer(
        display,
        state.display,
        match views.focus() {
            AtlasViewsFocus::List => "UP/DOWN VIEWS  SELECT OPEN  HOLD BOOT BACK",
            AtlasViewsFocus::Results => "UP/DOWN ROWS  SELECT OPEN/NEXT  HOLD BOOT BACK",
        },
    )
}

#[must_use]
pub fn views_status(state: &AppState) -> &'static str {
    match state.atlas_views_connection {
        AtlasConnectionState::Connected => {
            if state.atlas_views.focus() == AtlasViewsFocus::List
                && state.atlas_views.views().is_empty()
            {
                "NO VIEWS"
            } else if state.atlas_views.focus() == AtlasViewsFocus::Results
                && state.atlas_views.results().is_empty()
            {
                if state.atlas_views.next_page_available() {
                    "MORE RESULTS"
                } else {
                    "NO RESULTS"
                }
            } else {
                "READY"
            }
        }
        AtlasConnectionState::Offline => {
            if has_data(&state.atlas_views) {
                "OFFLINE CACHED"
            } else {
                "OFFLINE"
            }
        }
        AtlasConnectionState::Timeout => {
            if has_data(&state.atlas_views) {
                "ERROR CACHED"
            } else {
                "ERROR"
            }
        }
        AtlasConnectionState::Unconfigured => "LOAD VIEWS",
        AtlasConnectionState::Connecting => "LOADING",
        AtlasConnectionState::Unauthorized | AtlasConnectionState::Forbidden => "ERROR",
        AtlasConnectionState::ServerError => {
            if has_data(&state.atlas_views) {
                "ERROR CACHED"
            } else {
                "ERROR"
            }
        }
    }
}

fn has_data(views: &AtlasViewsState) -> bool {
    !views.views().is_empty() || !views.results().is_empty()
}
pub fn views_source(state: &AppState) -> &'static str {
    match state.atlas_views_connection {
        AtlasConnectionState::Connected => "LIVE",
        AtlasConnectionState::Offline
        | AtlasConnectionState::Timeout
        | AtlasConnectionState::Unauthorized
        | AtlasConnectionState::Forbidden
        | AtlasConnectionState::ServerError => {
            if has_data(&state.atlas_views) {
                "CACHED"
            } else {
                "EMPTY"
            }
        }
        AtlasConnectionState::Unconfigured | AtlasConnectionState::Connecting => "EMPTY",
    }
}
pub fn views_page_label(views: &AtlasViewsState) -> &'static str {
    if views.focus() == AtlasViewsFocus::List {
        "LIST"
    } else if views.pagination_cap_reached() && views.has_next_page() {
        "MORE LIMIT"
    } else if views.has_next_page() {
        "MORE"
    } else {
        "END"
    }
}

fn draw_view_list(
    display: &mut OrientedFrameBuffer<'_>,
    views: &AtlasViewsState,
    body: UiTextStyle,
    heading: UiTextStyle,
) -> Result<(), Infallible> {
    Text::new("Select a server View", Point::new(22, 158), heading).draw(display)?;
    Rectangle::new(Point::new(22, 178), Size::new(436, 290))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;
    if views.views().is_empty() {
        Text::new("NO VIEWS AVAILABLE", Point::new(38, 234), heading).draw(display)?;
        Text::new("LOAD ON EXPLICIT ENTRY", Point::new(38, 274), body).draw(display)?;
        return Ok(());
    }
    for (row, view) in views.views().iter().enumerate() {
        let baseline = 222 + row as i32 * 36;
        let selected = row == views.selected_view();
        let title = format!("{}{}", if selected { "> " } else { "  " }, view.name());
        Text::new(
            &title,
            Point::new(34, baseline),
            if selected { heading } else { body },
        )
        .draw_clipped(
            display,
            TextBounds::new(34, baseline - 22, 448, baseline + 2),
        )?;
        Text::new(
            if view.valid() { "READY" } else { "INVALID" },
            Point::new(52, baseline + 16),
            body,
        )
        .draw_clipped(display, TextBounds::new(52, baseline, 448, baseline + 20))?;
    }
    Ok(())
}

fn draw_results(
    display: &mut OrientedFrameBuffer<'_>,
    views: &AtlasViewsState,
    body: UiTextStyle,
    heading: UiTextStyle,
) -> Result<(), Infallible> {
    let header = fit_view_header(views.selected_view_name(), heading, VIEW_HEADER_MAX_WIDTH);
    Text::new(&header, Point::new(22, 158), heading)
        .draw_clipped(display, TextBounds::new(22, 132, 336, 162))?;
    let page = format!("PAGE {}", views.page_number());
    Text::new(&page, Point::new(344, 158), body).draw(display)?;
    Rectangle::new(Point::new(22, 178), Size::new(436, 290))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;
    if views.results().is_empty() {
        Text::new("NO VIEW RESULTS", Point::new(38, 238), heading).draw(display)?;
        if views.next_page_available() {
            Text::new(
                if views.next_page_selected() {
                    "> NEXT PAGE"
                } else {
                    "  NEXT PAGE"
                },
                Point::new(38, 294),
                if views.next_page_selected() {
                    heading
                } else {
                    body
                },
            )
            .draw(display)?;
        }
        return Ok(());
    }
    let offset = views.window_offset();
    let end = (offset + VIEW_VISIBLE_ROWS)
        .min(views.results().len() + usize::from(views.next_page_available()));
    for index in offset..end {
        let baseline = 218 + (index - offset) as i32 * 42;
        if index == views.results().len() {
            Text::new(
                if views.next_page_selected() {
                    "> NEXT PAGE"
                } else {
                    "  NEXT PAGE"
                },
                Point::new(34, baseline),
                if views.next_page_selected() {
                    heading
                } else {
                    body
                },
            )
            .draw(display)?;
            continue;
        }
        let result = &views.results()[index];
        let title = format!(
            "{}{}",
            if index == views.selected_result() {
                "> "
            } else {
                "  "
            },
            result.title()
        );
        Text::new(
            &title,
            Point::new(34, baseline),
            if index == views.selected_result() {
                heading
            } else {
                body
            },
        )
        .draw_clipped(
            display,
            TextBounds::new(34, baseline - 22, 448, baseline + 2),
        )?;
        Text::new(result.path(), Point::new(52, baseline + 17), body).draw_clipped(
            display,
            TextBounds::new(52, baseline + 2, 448, baseline + 20),
        )?;
    }
    Ok(())
}

/// Fit the selected View name to the measured heading strike before the PAGE
/// label begins. The clipped draw remains a defense against wide glyph ink.
pub fn fit_view_header(value: &str, style: UiTextStyle, max_width: i32) -> String {
    if max_width <= 0 {
        return String::new();
    }
    if style.text_width(value) <= max_width {
        return value.to_owned();
    }
    let ellipsis = "…";
    let ellipsis_width = style.text_width(ellipsis);
    let mut fitted = String::new();
    let mut width = 0;
    for character in value.chars() {
        let character_width = style.text_width(&character.to_string());
        if width + character_width + ellipsis_width > max_width {
            break;
        }
        fitted.push(character);
        width += character_width;
    }
    if ellipsis_width <= max_width {
        fitted.push_str(ellipsis);
    }
    fitted
}
