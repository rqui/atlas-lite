//! Atlas product mark adapted from the canonical Web sidebar icon.
//!
//! The source icon is the `archive` path in Atlas Web's `Icon.tsx`.  It is
//! reproduced here as a few bounded e-paper strokes instead of importing SVG
//! or browser assets into the firmware image.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive},
    primitives::{Line, PrimitiveStyle},
};

use crate::orientation::OrientedFrameBuffer;

/// Exact SVG path used by the Atlas Web sidebar `archive` icon.
pub const ATLAS_SIDEBAR_ARCHIVE_PATH: &str =
    "M4 7.5h16M5.5 7.5v11h13v-11M8 11h8M7 4.5h10l1 3H6l1-3Z";

/// Draw the small, monochrome Atlas archive mark at a predictable size.
///
/// The 24 by 24 SVG is scaled to a 32 by 28 logical-pixel outline. Curves,
/// color and web-only spacing are deliberately absent: the original icon is
/// all straight strokes, so this preserves its recognisable geometry without
/// an SVG parser, antialiasing buffer or image asset in firmware.
pub fn draw_atlas_mark(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    let stroke = PrimitiveStyle::with_stroke(ink, 2);
    let point = |x, y| Point::new(origin.x + x, origin.y + y);

    // SVG: M4 7.5h16
    Line::new(point(0, 15), point(32, 15))
        .into_styled(stroke)
        .draw(display)?;
    // SVG: M5.5 7.5v11h13v-11
    Line::new(point(3, 15), point(3, 37))
        .into_styled(stroke)
        .draw(display)?;
    Line::new(point(3, 37), point(29, 37))
        .into_styled(stroke)
        .draw(display)?;
    Line::new(point(29, 37), point(29, 15))
        .into_styled(stroke)
        .draw(display)?;
    // SVG: M8 11h8
    Line::new(point(8, 22), point(24, 22))
        .into_styled(stroke)
        .draw(display)?;
    // SVG: M7 4.5h10l1 3H6l1-3Z
    Line::new(point(6, 9), point(26, 9))
        .into_styled(stroke)
        .draw(display)?;
    Line::new(point(26, 9), point(28, 15))
        .into_styled(stroke)
        .draw(display)?;
    Line::new(point(28, 15), point(4, 15))
        .into_styled(stroke)
        .draw(display)?;
    Line::new(point(4, 15), point(6, 9))
        .into_styled(stroke)
        .draw(display)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::Point};

    use super::{draw_atlas_mark, ATLAS_SIDEBAR_ARCHIVE_PATH};
    use crate::{framebuffer::FrameBuffer, orientation::OrientedFrameBuffer};

    #[test]
    fn preserves_the_canonical_sidebar_path_as_brand_provenance() {
        assert_eq!(
            ATLAS_SIDEBAR_ARCHIVE_PATH,
            "M4 7.5h16M5.5 7.5v11h13v-11M8 11h8M7 4.5h10l1 3H6l1-3Z"
        );
    }

    #[test]
    fn mark_places_bounded_ink_on_the_portrait_canvas() {
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, Default::default());
        draw_atlas_mark(&mut display, Point::new(18, 15), BinaryColor::On).unwrap();

        let native = display
            .orientation()
            .map_logical_to_native(Point::new(22, 30))
            .unwrap();
        assert_eq!(frame.is_black(native), Some(true));
    }
}
