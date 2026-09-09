//! Compact reusable shell status row.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle},
};

use crate::{
    app::{
        display::DisplayPreferences,
        typography::{Text, TextBounds},
    },
    orientation::OrientedFrameBuffer,
};

/// Product status labels rendered directly below the header.
pub struct StatusRow<'a> {
    pub left: &'a str,
    pub middle: &'a str,
    pub right: &'a str,
}

/// Draw a three-part status strip.
pub fn draw_status_row(
    display: &mut OrientedFrameBuffer<'_>,
    preferences: DisplayPreferences,
    status: StatusRow<'_>,
) -> Result<(), Infallible> {
    let outline = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let detail = preferences.detail_style();

    Rectangle::new(Point::new(14, 80), Size::new(452, 42))
        .into_styled(outline)
        .draw(display)?;
    Text::new(status.left, Point::new(24, 108), detail)
        .draw_clipped(display, TextBounds::new(24, 86, 166, 114))?;
    Text::new(status.middle, Point::new(176, 108), detail)
        .draw_clipped(display, TextBounds::new(176, 86, 348, 114))?;
    Text::new(status.right, Point::new(358, 108), detail)
        .draw_clipped(display, TextBounds::new(358, 86, 456, 114))?;
    Ok(())
}
