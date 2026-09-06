//! E-paper rendering for the remote Atlas Books surface. Network work is
//! deliberately absent: this screen reads only bounded `AtlasBooksState`.

use core::convert::Infallible;
use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::{
    app::{
        state::AppState,
        typography::{Text, TextBounds},
        widgets::{
            footer::draw_footer,
            header::draw_atlas_header,
            selection::draw_selection_chrome,
            status_row::{draw_status_row, StatusRow},
        },
    },
    atlas_books::{BooksConnection, BooksView},
    orientation::OrientedFrameBuffer,
};

pub fn render_atlas_books(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
) -> Result<(), Infallible> {
    let books = &state.atlas_books;
    let body = state.display.body_style();
    let heading = state.display.heading_style();
    draw_atlas_header(display, state.display, "BOOKS")?;
    draw_status_row(
        display,
        state.display,
        StatusRow {
            left: view_label(books.view),
            middle: source_label(books.connection),
            right: connection_label(books.connection),
        },
    )?;
    match books.view {
        BooksView::List => {
            if books.books.is_empty() {
                let message =
                    books
                        .feedback
                        .unwrap_or(if books.connection == BooksConnection::Connecting {
                            "LOADING BOOKS..."
                        } else {
                            "NO BOOKS - SELECT RETRY"
                        });
                Text::new(message, Point::new(22, 186), heading).draw(display)?;
            } else {
                for (row, book) in books.books.iter().take(12).enumerate() {
                    let baseline = 160 + row as i32 * 44;
                    draw_selection_chrome(
                        display,
                        Rectangle::new(Point::new(18, baseline - 27), Size::new(440, 39)),
                        row == books.selected,
                    )?;
                    Text::new(
                        &book.title,
                        Point::new(50, baseline),
                        if row == books.selected { heading } else { body },
                    )
                    .draw_clipped(
                        display,
                        TextBounds::new(50, baseline - 25, 452, baseline + 2),
                    )?;
                    let author = book
                        .authors
                        .first()
                        .map(String::as_str)
                        .unwrap_or("Unknown author");
                    Text::new(
                        author,
                        Point::new(50, baseline + 19),
                        state.display.detail_style(),
                    )
                    .draw_clipped(
                        display,
                        TextBounds::new(50, baseline + 4, 452, baseline + 24),
                    )?;
                }
            }
        }
        BooksView::Detail => {
            let title = books
                .manifest
                .as_ref()
                .map(|value| value.book.title.as_str())
                .unwrap_or("BOOK");
            Text::new(title, Point::new(22, 164), heading)
                .draw_clipped(display, TextBounds::new(22, 138, 460, 170))?;
            for (index, label) in ["READ", "TABLE OF CONTENTS", "BOOKMARKS"]
                .iter()
                .enumerate()
            {
                let baseline = 224 + index as i32 * 62;
                draw_selection_chrome(
                    display,
                    Rectangle::new(Point::new(18, baseline - 30), Size::new(440, 44)),
                    index == books.selected,
                )?;
                Text::new(
                    label,
                    Point::new(50, baseline),
                    if index == books.selected {
                        heading
                    } else {
                        body
                    },
                )
                .draw(display)?;
            }
        }
        BooksView::Toc => render_rows(
            display,
            state,
            books
                .manifest
                .as_ref()
                .map(|value| value.toc.iter().map(|entry| entry.label.as_str()).collect())
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
                .map(|entry| entry.label.as_deref().unwrap_or("BOOKMARK"))
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
                    let baseline = 152 + row as i32 * style.line_height() as i32;
                    Text::new(&line.text, Point::new(20, baseline), style).draw_clipped(
                        display,
                        TextBounds::new(20, baseline - style.line_height() as i32, 462, 736),
                    )?;
                }
            } else {
                Text::new(
                    books.feedback.unwrap_or("LOADING PAGE..."),
                    Point::new(22, 186),
                    heading,
                )
                .draw(display)?;
            }
        }
    }
    let hint = match books.view {
        BooksView::Reader => "UP / DOWN PAGE  SELECT BOOKMARK  HOLD BOOT",
        _ => "UP / DOWN / SELECT   HOLD BOOT BACK",
    };
    draw_footer(display, state.display, hint)
}

fn render_rows(
    display: &mut OrientedFrameBuffer<'_>,
    state: &AppState,
    rows: Vec<&str>,
    empty: &str,
) -> Result<(), Infallible> {
    if rows.is_empty() {
        Text::new(empty, Point::new(22, 186), state.display.heading_style()).draw(display)?;
        return Ok(());
    }
    for (row, label) in rows.iter().take(12).enumerate() {
        let baseline = 168 + row as i32 * 44;
        draw_selection_chrome(
            display,
            Rectangle::new(Point::new(18, baseline - 27), Size::new(440, 36)),
            row == state.atlas_books.selected,
        )?;
        Text::new(
            label,
            Point::new(50, baseline),
            if row == state.atlas_books.selected {
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

const fn view_label(view: BooksView) -> &'static str {
    match view {
        BooksView::List => "LIBRARY",
        BooksView::Detail => "DETAIL",
        BooksView::Toc => "CONTENTS",
        BooksView::Bookmarks => "BOOKMARKS",
        BooksView::Reader => "READING",
    }
}
const fn source_label(connection: BooksConnection) -> &'static str {
    if matches!(connection, BooksConnection::Connected) {
        "REMOTE"
    } else {
        "MEMORY"
    }
}
const fn connection_label(connection: BooksConnection) -> &'static str {
    match connection {
        BooksConnection::Unconfigured => "OPEN",
        BooksConnection::Connecting => "SYNCING",
        BooksConnection::Connected => "ONLINE",
        BooksConnection::Offline => "OFFLINE",
        BooksConnection::Error => "ERROR",
        BooksConnection::RePairRequired => "RE-PAIR",
    }
}
