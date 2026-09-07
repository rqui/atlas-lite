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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AtlasHomeGeometry {
    pub margin: i32,
    pub status_height: i32,
    pub hero_top: i32,
    pub hero_bottom: i32,
    pub section_baseline: i32,
    pub divider_y: i32,
    pub rows_top: i32,
    pub row_height: u32,
    pub footer_top: i32,
}

pub(crate) const ATLAS_HOME_GEOMETRY: AtlasHomeGeometry = AtlasHomeGeometry {
    margin: 10,
    status_height: 48,
    hero_top: 54,
    hero_bottom: 174,
    section_baseline: 204,
    divider_y: 216,
    rows_top: 222,
    row_height: 72,
    footer_top: 746,
};
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
        String::new()
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
                detail: if let Some(progress) = state.atlas_books.resume_percentage {
                    format!("Continue reading · {progress}%")
                } else if state.atlas_books.list_loaded {
                    "Continue reading".into()
                } else {
                    String::new()
                },
                count: if state.atlas_books.list_loaded {
                    books_count
                } else {
                    String::new()
                },
            },
            AtlasHomeEntry {
                detail: "Find anything".into(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: "Notes, tags, more".into(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: "Quick voice note".into(),
                count: String::new(),
            },
            AtlasHomeEntry {
                detail: "Device & sync".into(),
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

    let geometry = ATLAS_HOME_GEOMETRY;
    let top = geometry.rows_top + index as i32 * geometry.row_height as i32;
    Some(Rectangle::new(
        Point::new(geometry.margin, top),
        Size::new(480 - (geometry.margin as u32 * 2), geometry.row_height),
    ))
}

/// Render the static, offline-capable Home navigation surface.
pub fn render_atlas_home(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_home_content(state);
    draw_atlas_topbar(display, state, "")?;
    let geometry = ATLAS_HOME_GEOMETRY;
    Text::new(
        "Capture that",
        Point::new(12, 103),
        state.display.large_style(),
    )
    .draw_clipped(
        display,
        TextBounds::new(
            geometry.margin,
            geometry.hero_top,
            470,
            geometry.hero_bottom,
        ),
    )?;
    Text::new("thought.", Point::new(12, 145), state.display.large_style()).draw_clipped(
        display,
        TextBounds::new(
            geometry.margin,
            geometry.hero_top,
            470,
            geometry.hero_bottom,
        ),
    )?;
    Text::new(
        "Atlas",
        Point::new(12, geometry.section_baseline),
        state.display.heading_style(),
    )
    .draw(display)?;
    Rectangle::new(
        Point::new(geometry.margin, geometry.divider_y),
        Size::new(460, 1),
    )
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
        let baseline = row.top_left.y + 30;
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
                row.top_left.y + 38,
            ),
        )?;
        Text::new(
            &content.entries()[index].detail,
            Point::new(left, baseline + 27),
            state.display.text_style(UiTextRole::Detail, ink),
        )
        .draw_clipped(
            display,
            TextBounds::new(
                left,
                baseline + 7,
                row.bottom_right().unwrap().x - 76,
                row.bottom_right().unwrap().y - 8,
            ),
        )?;
        if primary {
            let badge = Rectangle::new(
                Point::new(row.bottom_right().unwrap().x - 58, row.top_left.y + 20),
                Size::new(40, 30),
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
        ATLAS_HOME_GEOMETRY,
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
        assert_eq!(ATLAS_HOME_GEOMETRY.status_height, 48);
        assert_eq!(
            ATLAS_HOME_GEOMETRY.hero_bottom - ATLAS_HOME_GEOMETRY.hero_top,
            120
        );
        assert_eq!(ATLAS_HOME_GEOMETRY.row_height, 72);
        for index in 0..6 {
            let row = atlas_home_menu_rect(index).unwrap();
            assert!(row.bottom_right().unwrap().y < ATLAS_HOME_GEOMETRY.footer_top);
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
        assert_eq!(content.entries()[0].count, "");
        assert_eq!(content.entries()[0].detail, "");
        assert_eq!(content.entries()[1].count, "");
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
