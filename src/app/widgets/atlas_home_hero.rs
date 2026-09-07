//! Firmware-local 1-bit Atlas Home hero generated from the supplied artwork.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Pixel, Point, Size},
    primitives::Rectangle,
};

use crate::orientation::OrientedFrameBuffer;

pub const ATLAS_HOME_HERO_SOURCE_SHA256: &str =
    "a8cc0ddcebd77eaa5d087b0fc3ed14a488a271b05c8b2507f9e55ea4435ea163";
pub const ATLAS_HOME_HERO_SIZE: Size = Size::new(454, 76);
pub const ATLAS_HOME_HERO_ORIGIN: Point = Point::new(13, 79);

const ROW_BYTES: usize = (ATLAS_HOME_HERO_SIZE.width as usize + 7) / 8;
const BITS: &[u8; ROW_BYTES * ATLAS_HOME_HERO_SIZE.height as usize] =
    include_bytes!("../assets/atlas-home-winged-hero-454x76.bin");

#[must_use]
pub const fn atlas_home_hero_bounds() -> Rectangle {
    Rectangle::new(ATLAS_HOME_HERO_ORIGIN, ATLAS_HOME_HERO_SIZE)
}

pub fn draw_atlas_home_hero(display: &mut OrientedFrameBuffer<'_>) -> Result<(), Infallible> {
    for y in 0..ATLAS_HOME_HERO_SIZE.height as usize {
        for x in 0..ATLAS_HOME_HERO_SIZE.width as usize {
            // ImageMagick MONO uses one for white and zero for black.
            if BITS[y * ROW_BYTES + x / 8] & (1 << (7 - x % 8)) == 0 {
                Pixel(
                    ATLAS_HOME_HERO_ORIGIN + Point::new(x as i32, y as i32),
                    BinaryColor::On,
                )
                .draw(display)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_is_bounded_and_preserves_the_supplied_aspect_ratio() {
        let bounds = atlas_home_hero_bounds();
        assert_eq!(bounds.top_left.x, 13);
        assert_eq!(bounds.bottom_right().unwrap().x, 466);
        assert!(bounds.top_left.y >= 62);
        assert!(bounds.bottom_right().unwrap().y < 172);
        let source_ratio = 800.0_f32 / 134.0;
        let bitmap_ratio = bounds.size.width as f32 / bounds.size.height as f32;
        assert!((source_ratio - bitmap_ratio).abs() < 0.004);
        assert_eq!(BITS.len(), 4_332);
    }
}
