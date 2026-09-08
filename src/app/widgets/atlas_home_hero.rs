//! Firmware-local 1-bit Atlas Home hero generated from the supplied artwork.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Pixel, Point, Size},
    primitives::Rectangle,
};

use crate::orientation::OrientedFrameBuffer;

pub const ATLAS_HOME_HERO_SOURCE_SHA256: &str =
    "b48fbe5f71d1018533257f81727ab759fe793087d6245c8dd274360329837179";
pub const ATLAS_HOME_HERO_SIZE: Size = Size::new(456, 106);
pub const ATLAS_HOME_HERO_ORIGIN: Point = Point::new(12, 64);

const ROW_BYTES: usize = (ATLAS_HOME_HERO_SIZE.width as usize + 7) / 8;
const BITS: &[u8; ROW_BYTES * ATLAS_HOME_HERO_SIZE.height as usize] =
    include_bytes!("../assets/atlas-home-mark-456x106.bin");

#[must_use]
pub const fn atlas_home_hero_bounds() -> Rectangle {
    Rectangle::new(ATLAS_HOME_HERO_ORIGIN, ATLAS_HOME_HERO_SIZE)
}

pub fn draw_atlas_home_hero(display: &mut OrientedFrameBuffer<'_>) -> Result<(), Infallible> {
    for y in 0..ATLAS_HOME_HERO_SIZE.height as usize {
        for x in 0..ATLAS_HOME_HERO_SIZE.width as usize {
            // ImageMagick MONO packs each source byte least-significant bit first.
            if BITS[y * ROW_BYTES + x / 8] & (1 << (x % 8)) != 0 {
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
    fn hero_is_bounded_and_centers_the_trimmed_mark() {
        let bounds = atlas_home_hero_bounds();
        assert_eq!(bounds.top_left.x, 12);
        assert_eq!(bounds.bottom_right().unwrap().x, 467);
        assert!(bounds.top_left.y >= 62);
        assert!(bounds.bottom_right().unwrap().y < 172);
        assert_eq!(BITS.len(), 6_042);

        let mut min_x = ATLAS_HOME_HERO_SIZE.width as usize;
        let mut min_y = ATLAS_HOME_HERO_SIZE.height as usize;
        let mut max_x = 0;
        let mut max_y = 0;
        for y in 0..ATLAS_HOME_HERO_SIZE.height as usize {
            for x in 0..ATLAS_HOME_HERO_SIZE.width as usize {
                if BITS[y * ROW_BYTES + x / 8] & (1 << (x % 8)) != 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        assert_eq!((min_x, min_y, max_x, max_y), (48, 11, 407, 94));
        let source_ratio = 880.0_f32 / 205.0;
        let mark_ratio = (max_x - min_x + 1) as f32 / (max_y - min_y + 1) as f32;
        assert!((source_ratio - mark_ratio).abs() < 0.01);
    }
}
