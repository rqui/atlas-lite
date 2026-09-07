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
        typography::{Text, TextBounds, UiTextRole},
        widgets::{footer::draw_footer, header::draw_atlas_topbar},
    },
    atlas_state::AtlasConnectionState,
    orientation::OrientedFrameBuffer,
};

const HOME_MENU_X: i32 = 18;
const HOME_MENU_WIDTH: u32 = 444;
const HOME_PRIMARY_HEIGHT: u32 = 92;
const HOME_SECONDARY_HEIGHT: u32 = 78;
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
        String::new()
    } else {
        format!("{} notes", bounded_count(hierarchy.nodes().len(), partial))
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
                detail: if state.atlas_books.list_loaded {
                    "Continue reading".into()
                } else {
                    String::new()
                },
                count: if state.atlas_books.list_loaded {
                    books_count
                } else {
                    "—".into()
                },
            },
            AtlasHomeEntry {
                detail: String::new(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: String::new(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: String::new(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: String::new(),
                count: String::new(),
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

    let (top, height) = match index {
        0 => (178, HOME_PRIMARY_HEIGHT),
        1 => (270, HOME_PRIMARY_HEIGHT),
        2 => (362, HOME_SECONDARY_HEIGHT),
        3 => (440, HOME_SECONDARY_HEIGHT),
        4 => (518, HOME_SECONDARY_HEIGHT),
        _ => (596, HOME_SECONDARY_HEIGHT),
    };
    Some(Rectangle::new(
        Point::new(HOME_MENU_X, top),
        Size::new(HOME_MENU_WIDTH, height),
    ))
}

/// Render the static, offline-capable Home navigation surface.
pub fn render_atlas_home(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_home_content(state);
    draw_atlas_topbar(display, state, "")?;
    Text::new("Home", Point::new(18, 96), state.display.large_style()).draw(display)?;
    Text::new(
        "Capture that thought.",
        Point::new(18, 138),
        state.display.heading_style(),
    )
    .draw_clipped(display, TextBounds::new(18, 104, 462, 146))?;
    Rectangle::new(Point::new(18, 158), Size::new(444, 1))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(display)?;

    for (index, entry) in atlas_home_entries().iter().enumerate() {
        let row = atlas_home_menu_rect(index).expect("Atlas Home entries have visible rows");
        let selected = state.home_selected == index;
        if selected {
            row.into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }
        Rectangle::new(
            Point::new(row.top_left.x, row.bottom_right().unwrap().y),
            Size::new(row.size.width, 1),
        )
        .into_styled(PrimitiveStyle::with_fill(if selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        }))
        .draw(display)?;
        let primary = index < 2;
        let baseline = row.top_left.y + if primary { 37 } else { 48 };
        let left = row.top_left.x + 14;
        let ink = if selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };
        Text::new(
            entry.label,
            Point::new(left, baseline),
            if primary {
                state.display.text_style(UiTextRole::Heading, ink)
            } else {
                state.display.text_style(UiTextRole::Body, ink)
            },
        )
        .draw_clipped(
            display,
            TextBounds::new(
                left,
                row.top_left.y + 6,
                row.bottom_right().unwrap().x - if primary { 76 } else { 16 },
                row.top_left.y + 50,
            ),
        )?;
        Text::new(
            &content.entries()[index].detail,
            Point::new(left, baseline + 31),
            state.display.text_style(UiTextRole::Detail, ink),
        )
        .draw_clipped(
            display,
            TextBounds::new(
                left,
                baseline + 6,
                row.bottom_right().unwrap().x - 76,
                row.bottom_right().unwrap().y - 8,
            ),
        )?;
        if primary {
            let badge = Rectangle::new(
                Point::new(row.bottom_right().unwrap().x - 60, row.top_left.y + 25),
                Size::new(42, 30),
            );
            badge
                .into_styled(PrimitiveStyle::with_fill(if selected {
                    BinaryColor::Off
                } else {
                    BinaryColor::On
                }))
                .draw(display)?;
            Text::new(
                &content.entries()[index].count,
                Point::new(badge.top_left.x + 9, badge.top_left.y + 22),
                state.display.text_style(
                    UiTextRole::Body,
                    if selected {
                        BinaryColor::On
                    } else {
                        BinaryColor::Off
                    },
                ),
            )
            .draw_clipped(
                display,
                TextBounds::new(
                    badge.top_left.x + 4,
                    badge.top_left.y + 3,
                    badge.bottom_right().unwrap().x - 3,
                    badge.bottom_right().unwrap().y - 3,
                ),
            )?;
        }
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
        for index in 0..6 {
            let row = atlas_home_menu_rect(index).unwrap();
            assert!(row.bottom_right().unwrap().y < 730);
            for other in 0..index {
                let overlap = row.intersection(&atlas_home_menu_rect(other).unwrap());
                assert!(overlap.size.width == 0 || overlap.size.height == 0);
            }
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
        assert_eq!(content.entries()[0].detail, "");
        assert_eq!(content.entries()[1].count, "—");
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
    fn home_logo_and_inverted_active_row_render_for_every_supported_font_profile() {
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

                // Existing real bitmap remains visible in the compact topbar.
                let logo_native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(28, 20))
                    .unwrap();
                assert_eq!(frame.is_black(logo_native), Some(true));

                let selected = atlas_home_menu_rect(4).unwrap();
                let selected_native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(
                        selected.top_left.x + 14,
                        selected.top_left.y + selected.size.height as i32 / 2,
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
