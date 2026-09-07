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
    atlas_dto::BookBlockKind,
    orientation::OrientedFrameBuffer,
};
use core::convert::Infallible;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
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
                for (row, line) in page.lines.iter().enumerate() {
                    let baseline = 108 + row as i32 * style.line_height() as i32;
                    let line_style = if line.kind == BookBlockKind::Heading {
                        heading
                    } else {
                        style
                    };
                    Text::new(&line.text, Point::new(20, baseline), line_style).draw_clipped(
                        display,
                        TextBounds::new(20, baseline - line_style.line_height() as i32, 462, 736),
                    )?;
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
    draw_footer(
        display,
        state.display,
        if books.view == BooksView::Reader {
            "UP / DOWN PAGE  SELECT BOOKMARK  HOLD BOOT"
        } else {
            "UP / DOWN / SELECT   HOLD BOOT BACK"
        },
    )
}

fn render_list(display: &mut OrientedFrameBuffer<'_>, state: &AppState) -> Result<(), Infallible> {
    let books = &state.atlas_books;
    if books.books.is_empty() {
        Text::new(
            books
                .feedback
                .unwrap_or(if books.connection == BooksConnection::Connecting {
                    "LOADING BOOKS..."
                } else {
                    "NO BOOKS - SELECT RETRY"
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
        let title_style = if index == books.selected {
            state.display.heading_style()
        } else {
            state.display.body_style()
        };
        Text::new(&book.title, Point::new(50, baseline), title_style).draw_clipped(
            display,
            TextBounds::new(50, baseline - 28, 452, baseline + 4),
        )?;
        Text::new(
            book.authors
                .first()
                .map(String::as_str)
                .unwrap_or("AUTHOR UNKNOWN"),
            Point::new(50, baseline + 25),
            state.display.detail_style(),
        )
        .draw_clipped(
            display,
            TextBounds::new(50, baseline + 7, 340, baseline + 31),
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
    Text::new(
        books
            .manifest
            .as_ref()
            .map(|m| m.book.title.as_str())
            .unwrap_or("BOOK"),
        Point::new(22, 126),
        state.display.heading_style(),
    )
    .draw_clipped(display, TextBounds::new(22, 98, 460, 132))?;
    Text::new(
        books
            .manifest
            .as_ref()
            .and_then(|m| m.book.authors.first())
            .map(String::as_str)
            .unwrap_or("AUTHOR UNKNOWN"),
        Point::new(22, 154),
        state.display.body_style(),
    )
    .draw_clipped(display, TextBounds::new(22, 136, 460, 160))?;
    if let Some(progress) = books.resume_percentage {
        Text::new(
            &format!("{progress}% READ"),
            Point::new(22, 184),
            state.display.detail_style(),
        )
        .draw(display)?;
        Rectangle::new(Point::new(22, 192), Size::new(414, 5))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
        let width = u32::from(progress.min(100)) * 410 / 100;
        if width > 0 {
            Rectangle::new(Point::new(24, 194), Size::new(width, 1))
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
        let baseline = 258 + index as i32 * 62;
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
        BooksConnection::Offline => "OFFLINE CACHED",
        BooksConnection::Error => "BOOKS ERROR",
        BooksConnection::RePairRequired => "RE-PAIR REQUIRED",
    }
}
