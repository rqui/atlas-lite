//! One e-paper-safe active-row treatment for Atlas navigation surfaces.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::{Drawable, Point, Primitive, Size},
    primitives::{Circle, PrimitiveStyle, Rectangle},
};

use crate::orientation::OrientedFrameBuffer;

/// Draw the shared Atlas active-row chrome.
///
/// One selected item receives a light two-pixel frame and a solid rounded
/// rail. Neutral rows keep the surface that their enclosing screen already
/// provides. This avoids an inverted full row (which can ghost more visibly)
/// while remaining readable at e-paper viewing distance and under partial
/// refresh.
pub fn draw_selection_chrome(
    display: &mut OrientedFrameBuffer<'_>,
    bounds: Rectangle,
    selected: bool,
) -> Result<(), Infallible> {
    if !selected {
        return Ok(());
    }

    bounds
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;

    let height = bounds.size.height as i32;
    let diameter = if height >= 56 { 12 } else { 8 };
    let rail_x = bounds.top_left.x + 9;
    let rail_top = bounds.top_left.y + (height - diameter * 2) / 2;
    let rail_style = PrimitiveStyle::with_fill(BinaryColor::On);
    Circle::new(Point::new(rail_x, rail_top), diameter as u32)
        .into_styled(rail_style)
        .draw(display)?;
    Rectangle::new(
        Point::new(rail_x, rail_top + diameter / 2),
        Size::new(diameter as u32, diameter as u32),
    )
    .into_styled(rail_style)
    .draw(display)?;
    Circle::new(Point::new(rail_x, rail_top + diameter), diameter as u32)
        .into_styled(rail_style)
        .draw(display)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use embedded_graphics::prelude::{Point, Size};

    use super::draw_selection_chrome;
    use crate::{framebuffer::FrameBuffer, orientation::OrientedFrameBuffer};

    #[test]
    fn only_the_active_row_receives_the_shared_rail() {
        let mut frame = FrameBuffer::new_white();
        let mut display = OrientedFrameBuffer::new(&mut frame, Default::default());
        draw_selection_chrome(
            &mut display,
            embedded_graphics::primitives::Rectangle::new(Point::new(20, 160), Size::new(436, 72)),
            true,
        )
        .unwrap();
        draw_selection_chrome(
            &mut display,
            embedded_graphics::primitives::Rectangle::new(Point::new(20, 248), Size::new(436, 72)),
            false,
        )
        .unwrap();

        let selected = display
            .orientation()
            .map_logical_to_native(Point::new(30, 190))
            .unwrap();
        let neutral = display
            .orientation()
            .map_logical_to_native(Point::new(30, 278))
            .unwrap();
        assert_eq!(frame.is_black(selected), Some(true));
        assert_eq!(frame.is_black(neutral), Some(false));
    }
}
