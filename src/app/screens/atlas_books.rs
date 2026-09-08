//! Bounded, e-paper-native rendering for the remote Atlas Books surface.

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds},
        widgets::{
            footer::draw_footer, header::draw_atlas_topbar, selection::draw_selection_chrome,
        },
    },
    atlas_books::{BooksConnection, BooksView},
    atlas_dto::AtlasBookSummary,
    orientation::OrientedFrameBuffer,
};
use core::convert::Infallible;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, Rectangle},
};

const BOOK_CARDS_VISIBLE: usize = 6;

/// Rendering consumes only the bounded Book state. It cannot perform a cosmetic request.
pub fn render_atlas_books(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let books = &state.atlas_books;
    let heading = state.display.heading_style();
    draw_atlas_topbar(
        display,
        state,
        if books.view == BooksView::Reader {
            "READING"
        } else {
            "BOOKS"
        },
    )?;
    if !matches!(
        books.connection,
        BooksConnection::Connected | BooksConnection::Unconfigured
    ) {
        Text::new(
            connection_label(books.connection),
            Point::new(22, 96),
            state.display.detail_style(),
        )
        .draw(display)?;
    }
    match books.view {
        BooksView::List => render_list(display, state)?,
        BooksView::Detail => render_detail(display, state)?,
        BooksView::Toc => render_rows(
            display,
            state,
            books
                .manifest
                .as_ref()
                .map(|m| m.toc.iter().map(|e| e.label.as_str()).collect())
                .unwrap_or_default(),
            "NO TABLE OF CONTENTS",
        )?,
        BooksView::Bookmarks => render_rows(
            display,
            state,
            books
                .bookmarks
                .items
                .iter()
                .map(|e| e.label.as_deref().unwrap_or("BOOKMARK"))
                .collect(),
            "NO BOOKMARKS",
        )?,
        BooksView::Reader => {
            if let Some(page) = books.current_page.as_ref() {
                let style = crate::app::reader_typography::reader_body_style(
                    state.reader.preferences.book_font,
                    state.reader.preferences.font_size,
                    state.reader.preferences.theme,
                );
                let viewport = state.reader.preferences.viewport();
                let bounds =
                    TextBounds::new(viewport.left, viewport.top, viewport.right, viewport.bottom);
                for (row, line) in page.lines.iter().enumerate() {
                    let baseline = viewport.first_baseline + row as i32 * viewport.line_step;
                    if baseline > viewport.last_baseline {
                        break;
                    }
                    // Pagination measures this exact strike. Heading semantics
                    // stay in the bounded page model without swapping to a
                    // wider UI font after wrapping has already completed.
                    Text::new(&line.text, Point::new(viewport.left, baseline), style)
                        .draw_clipped(display, bounds)?;
                }
                if state.reader.preferences.show_progress {
                    Rectangle::new(
                        Point::new(10, viewport.logical_height - viewport.footer_height),
                        Size::new((viewport.logical_width - 20) as u32, 1),
                    )
                    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                    .draw(display)?;
                    Text::new(
                        &format!("{}:{}", page.anchor.spine_item + 1, page.anchor.block + 1),
                        Point::new(10, viewport.logical_height - 7),
                        state.display.detail_style(),
                    )
                    .draw(display)?;
                    if let Some(progress) = books.resume_percentage {
                        Text::new(
                            &format!("{progress}%"),
                            Point::new(viewport.logical_width - 48, viewport.logical_height - 7),
                            state.display.detail_style(),
                        )
                        .draw(display)?;
                    }
                }
            } else {
                Text::new(
                    books.feedback.unwrap_or("LOADING PAGE..."),
                    Point::new(22, 146),
                    heading,
                )
                .draw(display)?;
            }
        }
    }
    if books.view != BooksView::Reader {
        draw_footer(
            display,
            state.display,
            "UP / DOWN / SELECT   HOLD BOOT BACK",
        )?;
    }
    Ok(())
}

fn render_list(display: &mut OrientedFrameBuffer<'_>, state: &AppState) -> Result<(), Infallible> {
    let books = &state.atlas_books;
    if books.books.is_empty() {
        Text::new(
            books
                .feedback
                .unwrap_or(if books.connection == BooksConnection::Connecting {
                    "Loading…"
                } else if books.list_loaded {
                    "No books"
                } else if books.connection == BooksConnection::Unconfigured {
                    "Atlas setup required"
                } else {
                    "Unable to load — Select to retry"
                }),
            Point::new(22, 146),
            state.display.heading_style(),
        )
        .draw(display)?;
        return Ok(());
    }
    let offset = books
        .selected
        .saturating_sub(BOOK_CARDS_VISIBLE - 1)
        .min(books.books.len().saturating_sub(BOOK_CARDS_VISIBLE));
    let total = if books.list_has_more {
        format!("{}+", books.books.len())
    } else {
        books.books.len().to_string()
    };
    for (row, book) in books
        .books
        .iter()
        .skip(offset)
        .take(BOOK_CARDS_VISIBLE)
        .enumerate()
    {
        let index = row + offset;
        let baseline = 132 + row as i32 * 98;
        draw_selection_chrome(
            display,
            Rectangle::new(Point::new(18, baseline - 31), Size::new(440, 82)),
            index == books.selected,
        )?;
        draw_book_cover(
            display,
            state,
            book,
            Rectangle::new(Point::new(34, baseline - 24), Size::new(48, 64)),
        )?;
        let title_style = if index == books.selected {
            state.display.heading_style()
        } else {
            state.display.body_style()
        };
        Text::new(&book.title, Point::new(98, baseline), title_style).draw_clipped(
            display,
            TextBounds::new(98, baseline - 28, 452, baseline + 4),
        )?;
        Text::new(
            book.authors
                .first()
                .map(String::as_str)
                .unwrap_or("AUTHOR UNKNOWN"),
            Point::new(98, baseline + 25),
            state.display.detail_style(),
        )
        .draw_clipped(
            display,
            TextBounds::new(98, baseline + 7, 340, baseline + 31),
        )?;
        Text::new(
            &format!("{} / {total}", index + 1),
            Point::new(370, baseline + 25),
            state.display.detail_style(),
        )
        .draw(display)?;
    }
    Ok(())
}

fn render_detail(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let books = &state.atlas_books;
    if let Some(book) = books.manifest.as_ref().map(|manifest| &manifest.book) {
        draw_book_cover(
            display,
            state,
            book,
            Rectangle::new(Point::new(22, 94), Size::new(104, 142)),
        )?;
    }
    Text::new(
        books
            .manifest
            .as_ref()
            .map(|m| m.book.title.as_str())
            .unwrap_or("BOOK"),
        Point::new(148, 126),
        state.display.heading_style(),
    )
    .draw_clipped(display, TextBounds::new(148, 98, 460, 166))?;
    Text::new(
        books
            .manifest
            .as_ref()
            .and_then(|m| m.book.authors.first())
            .map(String::as_str)
            .unwrap_or("AUTHOR UNKNOWN"),
        Point::new(148, 186),
        state.display.body_style(),
    )
    .draw_clipped(display, TextBounds::new(148, 168, 460, 218))?;
    if let Some(progress) = books.resume_percentage {
        Text::new(
            &format!("{progress}% READ"),
            Point::new(148, 228),
            state.display.detail_style(),
        )
        .draw(display)?;
        Rectangle::new(Point::new(148, 238), Size::new(288, 5))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        let width = u32::from(progress.min(100)) * 284 / 100;
        if width > 0 {
            Rectangle::new(Point::new(150, 240), Size::new(width, 1))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display)?;
        }
    }
    for (index, label) in [
        if books.resume_anchor.is_some() {
            "CONTINUE"
        } else {
            "READ"
        },
        "TABLE OF CONTENTS",
        "BOOKMARKS",
    ]
    .iter()
    .enumerate()
    {
        let baseline = 304 + index as i32 * 62;
        draw_selection_chrome(
            display,
            Rectangle::new(Point::new(18, baseline - 30), Size::new(440, 44)),
            index == books.selected,
        )?;
        Text::new(
            label,
            Point::new(50, baseline),
            if index == books.selected {
                state.display.heading_style()
            } else {
                state.display.body_style()
            },
        )
        .draw(display)?;
    }
    Ok(())
}

/// Draw a deterministic, firmware-local e-ink cover. It provides book-specific
/// visual identity without a cover request, bitmap allocation, or API change.
fn draw_book_cover(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    book: &AtlasBookSummary,
    bounds: Rectangle,
) -> Result<(), Infallible> {
    bounds
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;
    let hash = book.title.bytes().fold(0x811c_9dc5_u32, |value, byte| {
        (value ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    let right = bounds.bottom_right().unwrap().x;
    for index in 0..3 {
        let y = bounds.top_left.y + 10 + index * 8;
        let inset = 5 + ((hash >> (index * 3)) & 0x7) as i32;
        Line::new(
            Point::new(bounds.top_left.x + inset, y),
            Point::new(right - 5, y),
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)?;
    }
    let monogram = book
        .title
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".into());
    let baseline = bounds.top_left.y + bounds.size.height as i32 - 10;
    Text::new(
        &monogram,
        Point::new(
            bounds.top_left.x + bounds.size.width as i32 / 2 - 8,
            baseline,
        ),
        state.display.heading_style(),
    )
    .draw_clipped(
        display,
        TextBounds::new(
            bounds.top_left.x + 4,
            bounds.top_left.y + 30,
            right - 4,
            bounds.bottom_right().unwrap().y - 3,
        ),
    )?;
    Ok(())
}

fn render_rows(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    rows: Vec<&str>,
    empty: &str,
) -> Result<(), Infallible> {
    if rows.is_empty() {
        Text::new(empty, Point::new(22, 146), state.display.heading_style()).draw(display)?;
        return Ok(());
    }
    let offset = state
        .atlas_books
        .selected
        .saturating_sub(11)
        .min(rows.len().saturating_sub(12));
    for (row, label) in rows.iter().skip(offset).take(12).enumerate() {
        let baseline = 132 + row as i32 * 44;
        let selected = row + offset == state.atlas_books.selected;
        draw_selection_chrome(
            display,
            Rectangle::new(Point::new(18, baseline - 27), Size::new(440, 36)),
            selected,
        )?;
        Text::new(
            label,
            Point::new(50, baseline),
            if selected {
                state.display.heading_style()
            } else {
                state.display.body_style()
            },
        )
        .draw_clipped(
            display,
            TextBounds::new(50, baseline - 24, 452, baseline + 4),
        )?;
    }
    Ok(())
}

const fn connection_label(connection: BooksConnection) -> &'static str {
    match connection {
        BooksConnection::Unconfigured | BooksConnection::Connected => "",
        BooksConnection::Connecting => "SYNCING",
        BooksConnection::Offline => "Offline",
        BooksConnection::Error => "Unable to load",
        BooksConnection::RePairRequired => "Authorization required",
    }
}

#[cfg(test)]
mod tests {
    use super::render_atlas_books;
    use crate::{
        app::AppState,
        atlas_books::{BooksConnection, BooksView},
        atlas_dto::{AtlasBookSummary, BookImportStatus},
        framebuffer::FrameBuffer,
        orientation::{DisplayOrientation, OrientedFrameBuffer},
    };
    use embedded_graphics::prelude::Point;

    #[test]
    fn books_list_draws_a_local_cover_without_network_assets() {
        let mut state = AppState::default();
        state.atlas_books.connection = BooksConnection::Connected;
        state.atlas_books.view = BooksView::List;
        state.atlas_books.list_loaded = true;
        state.atlas_books.books.push(AtlasBookSummary {
            id: "book_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            title: "Dune".into(),
            authors: vec!["Frank Herbert".into()],
            language: Some("en".into()),
            byte_size: 42,
            import_status: BookImportStatus::Ready,
        });
        let orientation = DisplayOrientation::Portrait;
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, orientation);
        render_atlas_books(&mut display, &state).unwrap();
        drop(display);

        let mut cover_ink = 0;
        for y in 108..173 {
            for x in 33..83 {
                let native = orientation.map_logical_to_native(Point::new(x, y)).unwrap();
                cover_ink += usize::from(frame.is_black(native) == Some(true));
            }
        }
        assert!(cover_ink > 180);
    }
}
