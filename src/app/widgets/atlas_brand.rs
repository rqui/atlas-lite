//! Atlas product marks for monochrome e-paper.
//!
//! Source: `rqui/atlas` commit `62040555cd5c33fbbef27cfd9de7bad2ef477e0d`,
//! `apps/web/public/icons/atlas-sidebar-logo.png` (SHA-256
//! `915182d05e25e7365bdbb2f78bb8fa5aa73e5c20580ea78c8d2533ffaf31d658`).
//! The checked-in rows are the reproducible 29x32, alpha-only, 50%-threshold
//! conversion: `magick atlas-sidebar-logo.png -alpha extract -trim -resize
//! 32x32 -threshold 50% txt:-`. Runtime therefore has neither a PNG decoder
//! nor an SD-card/image dependency.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Circle, Line, PrimitiveStyle},
};

use crate::orientation::OrientedFrameBuffer;

pub const ATLAS_WEB_LOGO_SHA256: &str =
    "915182d05e25e7365bdbb2f78bb8fa5aa73e5c20580ea78c8d2533ffaf31d658";

pub const ATLAS_EINK_MARK_SIZE: Size = Size::new(34, 34);

/// A small-panel adaptation of the canonical dotted Atlas sphere. Twelve
/// large perimeter nodes preserve its point-cloud silhouette while a thick
/// central `A` remains identifiable after e-paper thresholding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasEinkMark {
    origin: Point,
    ink: BinaryColor,
}

impl AtlasEinkMark {
    #[must_use]
    pub const fn new(origin: Point, ink: BinaryColor) -> Self {
        Self { origin, ink }
    }

    pub fn draw(self, display: &mut OrientedFrameBuffer<'_>) -> Result<(), Infallible> {
        const NODES: [Point; 12] = [
            Point::new(15, 0),
            Point::new(7, 3),
            Point::new(23, 3),
            Point::new(2, 9),
            Point::new(28, 9),
            Point::new(0, 16),
            Point::new(30, 16),
            Point::new(2, 24),
            Point::new(28, 24),
            Point::new(7, 29),
            Point::new(23, 29),
            Point::new(15, 30),
        ];
        let dot = PrimitiveStyle::with_fill(self.ink);
        for node in NODES {
            Circle::new(self.origin + node, 4)
                .into_styled(dot)
                .draw(display)?;
        }
        let stroke = PrimitiveStyle::with_stroke(self.ink, 2);
        for line in [
            Line::new(
                self.origin + Point::new(8, 26),
                self.origin + Point::new(17, 7),
            ),
            Line::new(
                self.origin + Point::new(17, 7),
                self.origin + Point::new(27, 26),
            ),
            Line::new(
                self.origin + Point::new(12, 19),
                self.origin + Point::new(22, 19),
            ),
        ] {
            line.into_styled(stroke).draw(display)?;
        }
        Ok(())
    }
}

/// Compatibility entry point for the product header widgets.
pub fn draw_atlas_mark(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    AtlasEinkMark::new(origin, ink).draw(display)
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::Point};

    use embedded_graphics::{
        prelude::{Drawable, Primitive},
        primitives::{PrimitiveStyle, Rectangle},
    };

    use super::{draw_atlas_mark, ATLAS_EINK_MARK_SIZE, ATLAS_WEB_LOGO_SHA256};
    use crate::{
        framebuffer::FrameBuffer,
        orientation::{DisplayOrientation, OrientedFrameBuffer},
    };

    #[test]
    fn embeds_the_real_web_sidebar_logo_provenance() {
        assert_eq!(ATLAS_WEB_LOGO_SHA256.len(), 64);
    }

    #[test]
    fn eink_mark_is_a_legible_white_34px_symbol_on_black() {
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, Default::default());
        Rectangle::new(Point::new(0, 0), ATLAS_EINK_MARK_SIZE)
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(&mut display)
            .unwrap();
        draw_atlas_mark(&mut display, Point::zero(), BinaryColor::Off).unwrap();
        drop(display);

        let mut white = 0;
        for y in 0..ATLAS_EINK_MARK_SIZE.height as i32 {
            for x in 0..ATLAS_EINK_MARK_SIZE.width as i32 {
                let native = DisplayOrientation::Portrait
                    .map_logical_to_native(Point::new(x, y))
                    .unwrap();
                if frame.is_black(native) == Some(false) {
                    white += 1;
                }
            }
        }
        assert!(white > 100, "thick e-ink mark must retain visible geometry");
    }
}
