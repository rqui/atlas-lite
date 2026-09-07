//! Menu-first, e-paper-native Atlas Home.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        menu::atlas_home_entries,
        state::AppState,
        typography::{Text, TextBounds},
        widgets::{
            footer::draw_footer, header::draw_atlas_topbar, selection::draw_selection_chrome,
        },
    },
    atlas_state::AtlasConnectionState,
    orientation::OrientedFrameBuffer,
};

const HOME_MENU_X: i32 = 20;
const HOME_MENU_FIRST_TOP: i32 = 92;
const HOME_MENU_ROW_STEP: i32 = 106;
const HOME_MENU_WIDTH: u32 = 440;
const HOME_MENU_HEIGHT: u32 = 92;
const ATLAS_HOME_FOOTER_HINT: &str = "UP / DOWN / SELECT   HOLD BOOT BACK";

/// Compact Home control legend that remains visible at every supported font
/// family and size profile.
#[must_use]
pub const fn atlas_home_footer_hint() -> &'static str {
    ATLAS_HOME_FOOTER_HINT
}

/// The compact, secret-free product content shown by the Atlas Home renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeContent {
    entries: [AtlasHomeEntry; 6],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeEntry {
    pub detail: String,
    pub count: String,
}

impl AtlasHomeContent {
    #[must_use]
    pub fn entries(&self) -> &[AtlasHomeEntry; 6] {
        &self.entries
    }
    #[must_use]
    pub fn status(&self) -> [&str; 3] {
        ["", "", ""]
    }
}

/// Build Home chrome from already-owned snapshots only. Rendering this model
/// never calls AtlasClient; Home intentionally does not fetch note or View
/// content that is no longer displayed.
#[must_use]
pub fn atlas_home_content(state: &AppState) -> AtlasHomeContent {
    let hierarchy = state.atlas_library.hierarchy();
    let partial = !matches!(
        hierarchy.completeness(),
        crate::atlas_library::LibraryCompleteness::Complete
    );
    let library_count = if hierarchy.nodes().is_empty()
        && state.atlas_library_connection == AtlasConnectionState::Unconfigured
    {
        "—".into()
    } else {
        bounded_count(hierarchy.root_ids().len(), partial)
    };
    let library_detail = if hierarchy.nodes().is_empty() {
        "NOT LOADED".into()
    } else {
        format!(
            "{} KNOWN NODES",
            bounded_count(hierarchy.nodes().len(), partial)
        )
    };
    let books_count = bounded_count(
        state.atlas_books.books.len(),
        state.atlas_books.list_has_more,
    );
    AtlasHomeContent {
        entries: [
            AtlasHomeEntry {
                detail: library_detail,
                count: library_count,
            },
            AtlasHomeEntry {
                detail: "READY TO READ".into(),
                count: books_count,
            },
            AtlasHomeEntry {
                detail: "FIND NOTES".into(),
                count: "›".into(),
            },
            AtlasHomeEntry {
                detail: "SAVED VIEWS".into(),
                count: "›".into(),
            },
            AtlasHomeEntry {
                detail: "CAPTURE INBOX".into(),
                count: "›".into(),
            },
            AtlasHomeEntry {
                detail: "DEVICE & SYNC".into(),
                count: "›".into(),
            },
        ],
    }
}

fn bounded_count(count: usize, partial: bool) -> String {
    if partial {
        format!("{count}+")
    } else {
        count.to_string()
    }
}

/// Return the logical portrait rectangle for one visible Atlas Home entry.
/// Keeping this geometry pure makes screen bounds host-testable without a
/// panel handle.
#[must_use]
pub(crate) fn atlas_home_menu_rect(index: usize) -> Option<Rectangle> {
    if index >= atlas_home_entries().len() {
        return None;
    }

    Some(Rectangle::new(
        Point::new(
            HOME_MENU_X,
            HOME_MENU_FIRST_TOP + index as i32 * HOME_MENU_ROW_STEP,
        ),
        Size::new(HOME_MENU_WIDTH, HOME_MENU_HEIGHT),
    ))
}

/// Render the static, offline-capable Home navigation surface.
pub fn render_atlas_home(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_home_content(state);
    let heading = state.display.heading_style();

    draw_atlas_topbar(display, state, "HOME")?;

    for (index, entry) in atlas_home_entries().iter().enumerate() {
        let row = atlas_home_menu_rect(index).expect("Atlas Home entries have visible rows");
        row.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        draw_selection_chrome(display, row, state.home_selected == index)?;
        let baseline = row.top_left.y + 39;
        Text::new(entry.label, Point::new(HOME_MENU_X + 38, baseline), heading).draw_clipped(
            display,
            TextBounds::new(
                HOME_MENU_X + 38,
                row.top_left.y + 10,
                HOME_MENU_X + HOME_MENU_WIDTH as i32 - 12,
                row.top_left.y + 48,
            ),
        )?;
        Text::new(
            &content.entries()[index].detail,
            Point::new(HOME_MENU_X + 38, baseline + 27),
            state.display.detail_style(),
        )
        .draw_clipped(
            display,
            TextBounds::new(HOME_MENU_X + 38, baseline + 8, 370, row.top_left.y + 82),
        )?;
        Text::new(
            &content.entries()[index].count,
            Point::new(410, baseline + 12),
            state.display.heading_style(),
        )
        .draw_clipped(
            display,
            TextBounds::new(390, row.top_left.y + 10, 454, row.top_left.y + 55),
        )?;
    }

    draw_footer(display, state.display, atlas_home_footer_hint())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        atlas_home_content, atlas_home_footer_hint, atlas_home_menu_rect, render_atlas_home,
    };
    use crate::{
        app::{
            display::{DisplayPreferences, UiFontFamily, UiFontSize},
            menu::atlas_home_entries,
            AppState,
        },
        atlas_state::{AtlasConnectionState, AtlasSnapshot},
        board_services::BoardSnapshot,
        framebuffer::FrameBuffer,
        network::{NetworkSnapshot, WifiConnectionState},
        orientation::{DisplayOrientation, OrientedFrameBuffer},
        power::PowerSnapshot,
    };

    #[test]
    fn menu_rows_are_non_overlapping_and_leave_a_clear_footer_gap() {
        let mut bottom = 0;
        for index in 0..6 {
            let row = atlas_home_menu_rect(index).unwrap();
            assert!(row.top_left.y >= bottom);
            assert!(row.bottom_right().unwrap().y < 730);
            bottom = row.bottom_right().unwrap().y + 1;
        }
        assert!(atlas_home_menu_rect(6).is_none());
    }

    #[test]
    fn home_content_uses_bounded_snapshot_counts_without_a_fetch() {
        let mut state = AppState::default();
        state.update_board_snapshot(BoardSnapshot {
            power: Some(PowerSnapshot {
                battery_percent: Some(50),
                ..PowerSnapshot::default()
            }),
            ..BoardSnapshot::default()
        });
        state.update_network_snapshot(NetworkSnapshot {
            wifi_state: WifiConnectionState::Connected,
            ..NetworkSnapshot::default()
        });
        state.update_atlas_snapshot(AtlasSnapshot {
            connection: AtlasConnectionState::Offline,
        });

        let content = atlas_home_content(&state);
        assert_eq!(content.entries()[0].count, "—");
        assert_eq!(content.entries()[0].detail, "NOT LOADED");
        assert_eq!(content.entries()[1].count, "0");
    }

    #[test]
    fn home_contains_the_six_ordered_navigation_targets() {
        let labels: Vec<_> = atlas_home_entries()
            .iter()
            .map(|entry| entry.label)
            .collect();
        assert_eq!(
            labels,
            ["Library", "Books", "Search", "Views", "Capture", "Settings"]
        );
    }

    #[test]
    fn home_logo_and_active_rail_render_for_every_supported_font_profile() {
        let orientation = DisplayOrientation::Portrait;
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let mut state = AppState::default();
                state.display = DisplayPreferences {
                    font_family,
                    font_size,
                };
                state.home_selected = 4;
                let mut frame = FrameBuffer::new_white();
                let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
                render_atlas_home(&mut display, &state).unwrap();
                drop(display);

                // Existing real bitmap: row 5, column 10 at origin (18, 15).
                let logo_native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(28, 20))
                    .unwrap();
                assert_eq!(frame.is_black(logo_native), Some(false));

                let selected = atlas_home_menu_rect(4).unwrap();
                let selected_native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(
                        selected.top_left.x + 14,
                        selected.top_left.y + 39,
                    ))
                    .unwrap();
                assert_eq!(frame.is_black(selected_native), Some(true));
            }
        }
    }

    #[test]
    fn footer_hint_fits_every_supported_font_profile() {
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            for font_size in [UiFontSize::Compact, UiFontSize::Standard, UiFontSize::Large] {
                let preferences = DisplayPreferences {
                    font_family,
                    font_size,
                };
                assert!(
                    preferences
                        .footer_style()
                        .text_width(atlas_home_footer_hint())
                        <= 448,
                    "{font_family:?} {font_size:?} footer overflows"
                );
            }
        }
    }
}
