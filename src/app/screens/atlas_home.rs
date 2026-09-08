//! Menu-first, e-paper-native Atlas Home.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        menu::atlas_home_entries,
        state::AppState,
        typography::{Text, TextBounds, UiTextRole},
        widgets::{atlas_home_hero::draw_atlas_home_hero, header::draw_atlas_home_topbar},
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
    pub rows_top: i32,
    pub row_height: u32,
    pub rows_bottom: i32,
}

pub(crate) const ATLAS_HOME_GEOMETRY: AtlasHomeGeometry = AtlasHomeGeometry {
    margin: 10,
    status_height: 56,
    hero_top: 62,
    hero_bottom: 172,
    rows_top: 178,
    row_height: 72,
    rows_bottom: 682,
};

const HOME_ICON_SIZE: u32 = 32;

/// The compact, secret-free product content shown by the Atlas Home renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeContent {
    entries: [AtlasHomeEntry; 7],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtlasHomeEntry {
    pub detail: String,
    pub count: String,
}

impl AtlasHomeContent {
    #[must_use]
    pub fn entries(&self) -> &[AtlasHomeEntry; 7] {
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
    let live_library_partial = !matches!(
        hierarchy.completeness(),
        crate::atlas_library::LibraryCompleteness::Complete
    );
    let summary = state.atlas_home_summary;
    let (library_count, library_detail) =
        if state.atlas_library_connection == AtlasConnectionState::Connected {
            (
                bounded_count(hierarchy.root_ids().len(), live_library_partial),
                format!(
                    "{} notes",
                    bounded_count(hierarchy.nodes().len(), live_library_partial)
                ),
            )
        } else if let Some(summary) = summary.filter(|value| value.library_roots.is_some()) {
            (
                bounded_count(
                    usize::from(summary.library_roots.unwrap_or(0)),
                    summary.library_partial,
                ),
                format!(
                    "{} notes",
                    bounded_count(
                        usize::from(summary.library_notes.unwrap_or(0)),
                        summary.library_partial,
                    )
                ),
            )
        } else {
            (String::new(), String::new())
        };
    let (books_count, books_partial, resume_percentage, books_loaded) =
        if state.atlas_books.list_loaded {
            (
                state.atlas_books.books.len(),
                state.atlas_books.list_has_more,
                state.atlas_books.resume_percentage,
                true,
            )
        } else if let Some(summary) = summary.filter(|value| value.books_count.is_some()) {
            (
                usize::from(summary.books_count.unwrap_or(0)),
                summary.books_partial,
                summary.resume_percentage,
                true,
            )
        } else {
            (0, false, None, false)
        };
    let books_count = if books_loaded {
        bounded_count(books_count, books_partial)
    } else {
        String::new()
    };
    AtlasHomeContent {
        entries: [
            AtlasHomeEntry {
                detail: library_detail,
                count: library_count,
            },
            AtlasHomeEntry {
                detail: if let Some(progress) = resume_percentage {
                    format!("Continue reading · {progress}%")
                } else if books_loaded {
                    "Continue reading".into()
                } else {
                    String::new()
                },
                count: books_count,
            },
            AtlasHomeEntry {
                detail: if state.voice_notes.notes.is_empty() {
                    String::new()
                } else {
                    "Available offline".into()
                },
                count: if state.voice_notes.notes.is_empty() {
                    String::new()
                } else {
                    state.voice_notes.notes.len().to_string()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HomeIcon {
    Library,
    Books,
    Voice,
    Search,
    Views,
    Capture,
    Settings,
}

const HOME_ICONS: [HomeIcon; 7] = [
    HomeIcon::Library,
    HomeIcon::Books,
    HomeIcon::Voice,
    HomeIcon::Search,
    HomeIcon::Views,
    HomeIcon::Capture,
    HomeIcon::Settings,
];

#[must_use]
pub(crate) fn atlas_home_icon_rect(index: usize) -> Option<Rectangle> {
    let row = atlas_home_menu_rect(index)?;
    Some(Rectangle::new(
        Point::new(row.top_left.x + 10, row.top_left.y + 26),
        Size::new(HOME_ICON_SIZE, HOME_ICON_SIZE),
    ))
}

fn draw_home_icon(
    display: &mut OrientedFrameBuffer<'_>,
    index: usize,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    let bounds = atlas_home_icon_rect(index).expect("six Home icons have bounded rows");
    let o = bounds.top_left;
    let stroke = PrimitiveStyle::with_stroke(ink, 3);
    let fill = PrimitiveStyle::with_fill(ink);
    match HOME_ICONS[index] {
        HomeIcon::Library => {
            Rectangle::new(o + Point::new(7, 2), Size::new(21, 27))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(12, 9), o + Point::new(23, 9))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(12, 16), o + Point::new(23, 16))
                .into_styled(stroke)
                .draw(display)?;
            Rectangle::new(o + Point::new(2, 7), Size::new(3, 23))
                .into_styled(fill)
                .draw(display)?;
        }
        HomeIcon::Books => {
            for line in [
                Line::new(o + Point::new(16, 6), o + Point::new(16, 28)),
                Line::new(o + Point::new(3, 4), o + Point::new(15, 8)),
                Line::new(o + Point::new(29, 4), o + Point::new(17, 8)),
                Line::new(o + Point::new(3, 4), o + Point::new(3, 24)),
                Line::new(o + Point::new(29, 4), o + Point::new(29, 24)),
                Line::new(o + Point::new(3, 24), o + Point::new(15, 28)),
                Line::new(o + Point::new(29, 24), o + Point::new(17, 28)),
            ] {
                line.into_styled(stroke).draw(display)?;
            }
        }
        HomeIcon::Voice => {
            Rectangle::new(o + Point::new(11, 2), Size::new(10, 20))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(5, 15), o + Point::new(5, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(27, 15), o + Point::new(27, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(5, 19), o + Point::new(27, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(16, 20), o + Point::new(16, 29))
                .into_styled(stroke)
                .draw(display)?;
        }
        HomeIcon::Search => {
            Circle::new(o + Point::new(2, 2), 22)
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(20, 20), o + Point::new(30, 30))
                .into_styled(stroke)
                .draw(display)?;
        }
        HomeIcon::Views => {
            for y in [2, 18] {
                for x in [2, 18] {
                    Rectangle::new(o + Point::new(x, y), Size::new(11, 11))
                        .into_styled(stroke)
                        .draw(display)?;
                }
            }
        }
        HomeIcon::Capture => {
            Rectangle::new(o + Point::new(11, 2), Size::new(10, 20))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(5, 15), o + Point::new(5, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(27, 15), o + Point::new(27, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(5, 19), o + Point::new(27, 19))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(16, 20), o + Point::new(16, 29))
                .into_styled(stroke)
                .draw(display)?;
            Line::new(o + Point::new(10, 29), o + Point::new(22, 29))
                .into_styled(stroke)
                .draw(display)?;
        }
        HomeIcon::Settings => {
            for (y, knob) in [(5, 9), (16, 22), (27, 13)] {
                Line::new(o + Point::new(2, y), o + Point::new(30, y))
                    .into_styled(stroke)
                    .draw(display)?;
                Circle::new(o + Point::new(knob - 3, y - 3), 7)
                    .into_styled(fill)
                    .draw(display)?;
            }
        }
    }
    Ok(())
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
    let top = geometry.rows_top + index as i32 * 72;
    Some(Rectangle::new(
        Point::new(geometry.margin, top),
        Size::new(480 - (geometry.margin as u32 * 2), 72),
    ))
}

/// Render the static, offline-capable Home navigation surface.
pub fn render_atlas_home(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let content = atlas_home_content(state);
    draw_atlas_home_topbar(display, state)?;
    draw_atlas_home_hero(display)?;

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
        let primary = index < 3;
        let baseline = if primary {
            row.top_left.y + 36
        } else {
            row.top_left.y + 53
        };
        let left = row.top_left.x + 54;
        let ink = if selected {
            BinaryColor::Off
        } else {
            BinaryColor::On
        };
        Text::new(
            entry.label,
            Point::new(left, baseline),
            state.display.text_style(UiTextRole::Heading, ink),
        )
        .draw_clipped(
            display,
            TextBounds::new(
                left,
                row.top_left.y + 6,
                row.bottom_right().unwrap().x - if primary { 76 } else { 16 },
                if primary {
                    row.top_left.y + 48
                } else {
                    row.bottom_right().unwrap().y
                },
            ),
        )?;
        draw_home_icon(display, index, ink)?;
        if primary {
            Text::new(
                &content.entries()[index].detail,
                Point::new(left, row.top_left.y + 68),
                state.display.text_style(UiTextRole::Detail, ink),
            )
            .draw_clipped(
                display,
                TextBounds::new(
                    left,
                    row.top_left.y + 45,
                    row.bottom_right().unwrap().x - 76,
                    row.bottom_right().unwrap().y - 3,
                ),
            )?;
        }
        if primary {
            let badge = Rectangle::new(
                Point::new(row.bottom_right().unwrap().x - 52, row.top_left.y + 24),
                Size::new(42, 34),
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
                Point::new(badge.top_left.x + 6, badge.top_left.y + 26),
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        atlas_home_content, atlas_home_icon_rect, atlas_home_menu_rect, render_atlas_home,
        ATLAS_HOME_GEOMETRY, HOME_ICONS,
    };
    use crate::{
        app::{
            display::{DisplayPreferences, UiFontFamily, UiFontSize},
            menu::atlas_home_entries,
            AppState,
        },
        atlas_home_summary::AtlasHomeSummary,
        atlas_state::{AtlasConnectionState, AtlasSnapshot},
        board_services::BoardSnapshot,
        framebuffer::FrameBuffer,
        network::{NetworkSnapshot, WifiConnectionState},
        orientation::{DisplayOrientation, OrientedFrameBuffer},
        power::PowerSnapshot,
    };

    #[test]
    fn menu_rows_are_large_non_overlapping_and_fit_without_a_footer() {
        assert_eq!(ATLAS_HOME_GEOMETRY.status_height, 56);
        assert_eq!(
            ATLAS_HOME_GEOMETRY.hero_bottom - ATLAS_HOME_GEOMETRY.hero_top,
            110
        );
        assert_eq!(ATLAS_HOME_GEOMETRY.row_height, 72);
        for index in 0..7 {
            let row = atlas_home_menu_rect(index).unwrap();
            assert!(row.bottom_right().unwrap().y <= ATLAS_HOME_GEOMETRY.rows_bottom);
            let icon = atlas_home_icon_rect(index).unwrap();
            assert_eq!(icon.size, embedded_graphics::prelude::Size::new(32, 32));
            assert!(row.contains(icon.top_left));
            assert!(row.contains(icon.bottom_right().unwrap()));
            for other in 0..index {
                let overlap = row.intersection(&atlas_home_menu_rect(other).unwrap());
                assert!(overlap.size.width == 0 || overlap.size.height == 0);
            }
        }
        assert!(atlas_home_menu_rect(7).is_none());
        assert_eq!(HOME_ICONS.len(), 7);
        // The supplied bitmap's visible ink is y=75..158. Keep the same
        // whitespace below the masthead and above the first menu row.
        assert_eq!(75 - ATLAS_HOME_GEOMETRY.status_height, 19);
        assert_eq!(ATLAS_HOME_GEOMETRY.rows_top - 159, 19);
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
    fn persisted_summary_hydrates_home_before_live_lists_load() {
        let mut state = AppState::default();
        state.hydrate_atlas_home_summary(Some(AtlasHomeSummary {
            library_roots: Some(4),
            library_notes: Some(19),
            library_partial: true,
            books_count: Some(12),
            books_partial: false,
            resume_percentage: Some(68),
        }));

        let content = atlas_home_content(&state);
        assert_eq!(content.entries()[0].count, "4+");
        assert_eq!(content.entries()[0].detail, "19+ notes");
        assert_eq!(content.entries()[1].count, "12");
        assert_eq!(content.entries()[1].detail, "Continue reading · 68%");
    }

    #[test]
    fn home_contains_the_seven_ordered_navigation_targets() {
        let labels: Vec<_> = atlas_home_entries()
            .iter()
            .map(|entry| entry.label)
            .collect();
        assert_eq!(
            labels,
            [
                "Library",
                "Books",
                "Voice Recordings",
                "Search",
                "Views",
                "Capture",
                "Settings"
            ]
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

                // The exact 29x32 official mark is white against the masthead.
                let mut white_logo_pixels = 0;
                for y in 12..44 {
                    for x in 8..37 {
                        let native = orientation
                            .map_logical_to_native(embedded_graphics::prelude::Point::new(x, y))
                            .unwrap();
                        if frame.is_black(native) == Some(false) {
                            white_logo_pixels += 1;
                        }
                    }
                }
                assert!(white_logo_pixels > 60);

                let selected = atlas_home_menu_rect(4).unwrap();
                let selected_native = orientation
                    .map_logical_to_native(embedded_graphics::prelude::Point::new(
                        selected.top_left.x + 440,
                        selected.top_left.y + selected.size.height as i32 / 2,
                    ))
                    .unwrap();
                assert_eq!(frame.is_black(selected_native), Some(true));
            }
        }
    }

    #[test]
    fn standard_home_uses_physical_32px_menu_labels() {
        for font_family in [UiFontFamily::Inter, UiFontFamily::AtkinsonHyperlegible] {
            let preferences = DisplayPreferences {
                font_family,
                font_size: UiFontSize::Standard,
            };
            assert!(preferences.heading_style().line_height() >= 30);
        }
    }

    #[test]
    fn selected_icon_is_white_while_normal_icon_is_black() {
        let orientation = DisplayOrientation::Portrait;
        let mut state = AppState::default();
        state.home_selected = 0;
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
        render_atlas_home(&mut display, &state).unwrap();
        drop(display);
        let selected = orientation
            .map_logical_to_native(
                atlas_home_icon_rect(0).unwrap().top_left
                    + embedded_graphics::prelude::Point::new(9, 3),
            )
            .unwrap();
        let normal = orientation
            .map_logical_to_native(
                atlas_home_icon_rect(1).unwrap().top_left
                    + embedded_graphics::prelude::Point::new(3, 4),
            )
            .unwrap();
        assert_eq!(frame.is_black(selected), Some(false));
        assert_eq!(frame.is_black(normal), Some(true));
    }
}
