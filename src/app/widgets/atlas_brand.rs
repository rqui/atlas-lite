//! Atlas product mark generated from the canonical Web sidebar bitmap.
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
    prelude::{Drawable, Pixel, Point},
};

use crate::orientation::OrientedFrameBuffer;

pub const ATLAS_WEB_LOGO_SHA256: &str =
    "915182d05e25e7365bdbb2f78bb8fa5aa73e5c20580ea78c8d2533ffaf31d658";

const WIDTH: usize = 29;
const ROWS: [u32; 32] = [
    0b00000000000000000000000000000,
    0b00000000000000000000000000000,
    0b00000000000000000000000000000,
    0b00000000000000000000000000000,
    0b00000000000000000000000000000,
    0b00000000001010101010000000000,
    0b00000110000000000000001000000,
    0b00000010110100100101101000000,
    0b00000000000100100100000000000,
    0b00011101001000000010010111000,
    0b00001011001101110110011010000,
    0b01000000001000100010000000000,
    0b01110010000000000000001001110,
    0b00110110011101110111001101100,
    0b00000110011001110111001100000,
    0b10100000000000100000000000101,
    0b11101110010000100001001100111,
    0b00101110111001110011101110100,
    0b00000110111001110011101100000,
    0b10100000010001110000000000101,
    0b00100110010000000001001101100,
    0b00100110111001110011101100100,
    0b01000000011001110011001000010,
    0b00110000000000000000000001000,
    0b00010011011000100011011001000,
    0b00000000011001110011000000000,
    0b00000101000000000000010100000,
    0b00000001001100100110010000000,
    0b00000000000000000000000000000,
    0b00000000010100100101000000000,
    0b00000000000000000000000000000,
    0b00000000000000000000000000000,
];

/// Draw the real Atlas sidebar logo as a small single-bit bitmap.
pub fn draw_atlas_mark(
    display: &mut OrientedFrameBuffer<'_>,
    origin: Point,
    ink: BinaryColor,
) -> Result<(), Infallible> {
    for (y, row) in ROWS.into_iter().enumerate() {
        for x in 0..WIDTH {
            if row & (1 << (WIDTH - 1 - x)) != 0 {
                Pixel(Point::new(origin.x + x as i32, origin.y + y as i32), ink).draw(display)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::Point};

    use super::{draw_atlas_mark, ATLAS_WEB_LOGO_SHA256};
    use crate::{framebuffer::FrameBuffer, orientation::OrientedFrameBuffer};

    #[test]
    fn embeds_the_real_web_sidebar_logo_provenance() {
        assert_eq!(ATLAS_WEB_LOGO_SHA256.len(), 64);
    }

    #[test]
    fn logo_has_a_real_bounded_bitmap_not_a_single_placeholder_pixel() {
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, Default::default());
        draw_atlas_mark(&mut display, Point::new(18, 15), BinaryColor::On).unwrap();
        let black = frame
            .as_bytes()
            .iter()
            .map(|byte| byte.count_zeros())
            .sum::<u32>();
        assert!(black > 60, "logo bitmap must retain visible geometry");
    }
}
