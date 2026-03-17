// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use embedded_graphics::{
    Drawable, Pixel,
    draw_target::DrawTarget,
    geometry::{AngleUnit, OriginDimensions, Point, Size},
    pixelcolor::{BinaryColor, PixelColor, Rgb565, RgbColor, WebColors},
    primitives::{Arc as DisplayArc, Line, Primitive, PrimitiveStyle, Triangle},
};
use u8g2_fonts::{
    FontRenderer, fonts,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
};

use crate::cedar_client::ServerState;

// Represents the visual state of the screen
pub enum DrawState<'a> {
    Message(String),
    // State, stale_angle
    Operating(&'a ServerState, Option<u32>),
}

// Rotation is clockwise
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub fn from_degrees(deg: u16) -> Self {
        match deg {
            90 => Rotation::Deg90,
            180 => Rotation::Deg180,
            270 => Rotation::Deg270,
            _ => Rotation::Deg0,
        }
    }
}

// Configuration for text position
#[derive(Clone)]
struct TextPosition {
    start: Point,
    vertical: VerticalPosition,
    horizontal: HorizontalAlignment,
}

#[derive(Clone)]
// Stores positioning for an orientation
struct RenderingConfiguration {
    // Direction triangles
    up_triangle: Triangle,
    down_triangle: Triangle,
    left_triangle: Triangle,
    right_triangle: Triangle,

    // Stale arc and guidance arrow
    guidance_center: Point,
    arc_radius: i32,
    arrow_length: f64,
    arrowhead_size: f64,

    // Text positions
    status_position: TextPosition,
    tilt_position: TextPosition,
    rot_position: TextPosition,
    dec_label_position: TextPosition,
    ra_label_position: TextPosition,
}

// Stores configuration for a particular display
pub struct RotatedDisplay<D, C: PixelColor> {
    pub parent: D,
    rotation: Rotation,

    // Fonts and styles
    status_font: FontRenderer,
    guidance_font: FontRenderer,
    fg_color: C,
    bg_color: C,
    stale_color: C,
    triangle_style: PrimitiveStyle<C>,
    triangle_stale_style: PrimitiveStyle<C>,
    arrow_shaft_style: PrimitiveStyle<C>,
    arrowhead_style: PrimitiveStyle<C>,
    arc_style: PrimitiveStyle<C>,

    default_rendering: RenderingConfiguration,
    rotated_rendering: RenderingConfiguration,
    rotate_status: bool,
    high_precision: bool,
}

impl<D> RotatedDisplay<D, Rgb565>
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: std::fmt::Debug,
{
    pub fn new_rgb_240_240(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_240_240(
            parent,
            rotation,
            Rgb565::RED,
            Rgb565::BLACK,
            Rgb565::CSS_MAROON,
        )
    }

    pub fn new_rgb_135_240(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_135_240(
            parent,
            rotation,
            Rgb565::RED,
            Rgb565::BLACK,
            Rgb565::CSS_MAROON,
        )
    }

    pub fn new_rgb_128_128(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_128_128(
            parent,
            rotation,
            Rgb565::RED,
            Rgb565::BLACK,
            Rgb565::CSS_MAROON,
        )
    }

    pub fn new_rgb_128_64(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_128_64(
            parent,
            rotation,
            Rgb565::RED,
            Rgb565::BLACK,
            Rgb565::CSS_MAROON,
        )
    }

    pub fn new_rgb_128_32(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_128_32(
            parent,
            rotation,
            Rgb565::RED,
            Rgb565::BLACK,
            Rgb565::CSS_MAROON,
        )
    }
}

impl<D> RotatedDisplay<D, BinaryColor>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: std::fmt::Debug,
{
    pub fn new_binary_128_64(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_128_64(
            parent,
            rotation,
            BinaryColor::On,
            BinaryColor::Off,
            BinaryColor::On,
        )
    }

    pub fn new_binary_128_32(parent: D, rotation: Rotation) -> Self {
        RotatedDisplay::new_128_32(
            parent,
            rotation,
            BinaryColor::On,
            BinaryColor::Off,
            BinaryColor::On,
        )
    }
}

impl<D, C: PixelColor> RotatedDisplay<D, C>
where
    D: DrawTarget<Color = C>,
    D::Error: std::fmt::Debug,
{
    fn new_240_240(
        parent: D,
        rotation: Rotation,
        fg_color: C,
        bg_color: C,
        stale_color: C,
    ) -> Self {
        let positions = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(32, 2), Point::new(2, 62), Point::new(62, 62)),
            down_triangle: Triangle::new(Point::new(2, 2), Point::new(62, 2), Point::new(32, 62)),
            right_triangle: Triangle::new(
                Point::new(2, 177),
                Point::new(2, 237),
                Point::new(62, 207),
            ),
            left_triangle: Triangle::new(
                Point::new(62, 177),
                Point::new(62, 237),
                Point::new(2, 207),
            ),
            guidance_center: Point::new(120, 120),
            arc_radius: 40,
            arrow_length: 80.0,
            arrowhead_size: 24.0,
            status_position: TextPosition {
                start: Point::new(120, 120),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(237, 2),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(237, 237),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(2, 2),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Left,
            },
            ra_label_position: TextPosition {
                start: Point::new(2, 237),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Left,
            },
        };

        Self {
            parent,
            rotation,
            status_font: FontRenderer::new::<fonts::u8g2_font_logisoso32_tr>(),
            guidance_font: FontRenderer::new::<fonts::u8g2_font_logisoso58_tr>(),
            fg_color,
            bg_color,
            stale_color,
            triangle_style: PrimitiveStyle::with_fill(fg_color),
            triangle_stale_style: PrimitiveStyle::with_stroke(fg_color, 2),
            arrow_shaft_style: PrimitiveStyle::with_stroke(fg_color, 6),
            arrowhead_style: PrimitiveStyle::with_fill(fg_color),
            arc_style: PrimitiveStyle::with_stroke(fg_color, 6),
            default_rendering: positions.clone(),
            rotated_rendering: positions,
            rotate_status: true,
            high_precision: true,
        }
    }

    fn new_135_240(
        parent: D,
        rotation: Rotation,
        fg_color: C,
        bg_color: C,
        stale_color: C,
    ) -> Self {
        let default_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(
                Point::new(106, 64),
                Point::new(82, 112),
                Point::new(130, 112),
            ),
            down_triangle: Triangle::new(
                Point::new(82, 64),
                Point::new(130, 64),
                Point::new(106, 112),
            ),
            right_triangle: Triangle::new(
                Point::new(82, 126),
                Point::new(82, 174),
                Point::new(130, 150),
            ),
            left_triangle: Triangle::new(
                Point::new(130, 126),
                Point::new(130, 174),
                Point::new(82, 150),
            ),
            guidance_center: Point::new(34, 120),
            arc_radius: 26,
            arrow_length: 52.0,
            arrowhead_size: 16.0,
            status_position: TextPosition {
                start: Point::new(120, 68),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(130, 2),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(130, 238),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(130, 64),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            ra_label_position: TextPosition {
                start: Point::new(130, 182),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
        };
        let rotated_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(26, 2), Point::new(2, 50), Point::new(50, 50)),
            down_triangle: Triangle::new(Point::new(2, 2), Point::new(50, 2), Point::new(26, 50)),
            right_triangle: Triangle::new(
                Point::new(2, 72),
                Point::new(2, 120),
                Point::new(50, 96),
            ),
            left_triangle: Triangle::new(
                Point::new(50, 72),
                Point::new(50, 120),
                Point::new(2, 96),
            ),
            guidance_center: Point::new(208, 68),
            arc_radius: 26,
            arrow_length: 52.0,
            arrowhead_size: 16.0,
            status_position: TextPosition {
                start: Point::new(120, 68),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(172, 2),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(172, 124),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(2, 2),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Left,
            },
            ra_label_position: TextPosition {
                start: Point::new(2, 124),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Left,
            },
        };
        Self {
            parent,
            rotation,
            status_font: FontRenderer::new::<fonts::u8g2_font_logisoso32_tr>(),
            guidance_font: FontRenderer::new::<fonts::u8g2_font_logisoso50_tr>(),
            fg_color,
            bg_color,
            stale_color,
            triangle_style: PrimitiveStyle::with_fill(fg_color),
            triangle_stale_style: PrimitiveStyle::with_stroke(fg_color, 2),
            arrow_shaft_style: PrimitiveStyle::with_stroke(fg_color, 6),
            arrowhead_style: PrimitiveStyle::with_fill(fg_color),
            arc_style: PrimitiveStyle::with_stroke(fg_color, 6),
            default_rendering,
            rotated_rendering,
            rotate_status: false,
            high_precision: true,
        }
    }

    fn new_128_128(
        parent: D,
        rotation: Rotation,
        fg_color: C,
        bg_color: C,
        stale_color: C,
    ) -> Self {
        let positions = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(15, 0), Point::new(0, 30), Point::new(30, 30)),
            down_triangle: Triangle::new(Point::new(0, 0), Point::new(30, 0), Point::new(15, 30)),
            right_triangle: Triangle::new(
                Point::new(0, 97),
                Point::new(0, 127),
                Point::new(30, 112),
            ),
            left_triangle: Triangle::new(
                Point::new(30, 97),
                Point::new(30, 127),
                Point::new(0, 112),
            ),
            guidance_center: Point::new(64, 64),
            arc_radius: 20,
            arrow_length: 40.0,
            arrowhead_size: 12.0,
            status_position: TextPosition {
                start: Point::new(64, 64),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(127, 0),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(127, 127),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(0, 0),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Left,
            },
            ra_label_position: TextPosition {
                start: Point::new(0, 127),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Left,
            },
        };

        Self {
            parent,
            rotation,
            status_font: FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>(),
            guidance_font: FontRenderer::new::<fonts::u8g2_font_logisoso34_tr>(),
            fg_color,
            bg_color,
            stale_color,
            triangle_style: PrimitiveStyle::with_fill(fg_color),
            triangle_stale_style: PrimitiveStyle::with_stroke(fg_color, 1),
            arrow_shaft_style: PrimitiveStyle::with_stroke(fg_color, 3),
            arrowhead_style: PrimitiveStyle::with_fill(fg_color),
            arc_style: PrimitiveStyle::with_stroke(fg_color, 3),
            default_rendering: positions.clone(),
            rotated_rendering: positions,
            rotate_status: true,
            high_precision: true,
        }
    }

    fn new_128_64(parent: D, rotation: Rotation, fg_color: C, bg_color: C, stale_color: C) -> Self {
        // Intentionally leave 1 pixel border due to vertical shift observed on SSD1309 display with
        // an RPI5 but not an RPIZ2W
        let default_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(13, 1), Point::new(1, 25), Point::new(25, 25)),
            down_triangle: Triangle::new(Point::new(1, 1), Point::new(25, 1), Point::new(13, 25)),
            right_triangle: Triangle::new(Point::new(1, 36), Point::new(1, 60), Point::new(25, 48)),
            left_triangle: Triangle::new(Point::new(25, 36), Point::new(25, 60), Point::new(1, 48)),
            guidance_center: Point::new(112, 32),
            arc_radius: 14,
            arrow_length: 28.0,
            arrowhead_size: 8.0,
            status_position: TextPosition {
                start: Point::new(64, 32),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(91, 1),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(91, 62),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(1, 1),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Left,
            },
            ra_label_position: TextPosition {
                start: Point::new(1, 62),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Left,
            },
        };
        let rotated_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(50, 33), Point::new(38, 57), Point::new(62, 57)),
            down_triangle: Triangle::new(
                Point::new(38, 33),
                Point::new(62, 33),
                Point::new(50, 57),
            ),
            right_triangle: Triangle::new(
                Point::new(38, 67),
                Point::new(38, 91),
                Point::new(62, 79),
            ),
            left_triangle: Triangle::new(
                Point::new(62, 67),
                Point::new(62, 91),
                Point::new(38, 79),
            ),
            guidance_center: Point::new(16, 64),
            arc_radius: 14,
            arrow_length: 28.0,
            arrowhead_size: 8.0,
            status_position: TextPosition {
                start: Point::new(64, 32),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(62, 1),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            rot_position: TextPosition {
                start: Point::new(62, 126),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(62, 33),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Right,
            },
            ra_label_position: TextPosition {
                start: Point::new(62, 95),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Right,
            },
        };
        Self {
            parent,
            rotation,
            status_font: FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>(),
            guidance_font: FontRenderer::new::<fonts::u8g2_font_logisoso26_tr>(),
            fg_color,
            bg_color,
            stale_color,
            triangle_style: PrimitiveStyle::with_fill(fg_color),
            triangle_stale_style: PrimitiveStyle::with_stroke(fg_color, 1),
            arrow_shaft_style: PrimitiveStyle::with_stroke(fg_color, 3),
            arrowhead_style: PrimitiveStyle::with_fill(fg_color),
            arc_style: PrimitiveStyle::with_stroke(fg_color, 3),
            default_rendering,
            rotated_rendering,
            rotate_status: false,
            high_precision: true,
        }
    }

    fn new_128_32(parent: D, rotation: Rotation, fg_color: C, bg_color: C, stale_color: C) -> Self {
        let default_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(6, 10), Point::new(0, 22), Point::new(12, 22)),
            down_triangle: Triangle::new(Point::new(0, 10), Point::new(12, 10), Point::new(6, 22)),
            right_triangle: Triangle::new(
                Point::new(115, 10),
                Point::new(115, 22),
                Point::new(127, 16),
            ),
            left_triangle: Triangle::new(
                Point::new(127, 10),
                Point::new(127, 22),
                Point::new(115, 16),
            ),
            guidance_center: Point::new(64, 16),
            arc_radius: 9,
            arrow_length: 24.0,
            arrowhead_size: 8.0,
            status_position: TextPosition {
                start: Point::new(64, 16),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(18, 18),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Left,
            },
            rot_position: TextPosition {
                start: Point::new(109, 18),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Right,
            },
            dec_label_position: TextPosition {
                start: Point::new(0, 18),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Left,
            },
            ra_label_position: TextPosition {
                start: Point::new(127, 18),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Right,
            },
        };
        let rotated_rendering = RenderingConfiguration {
            up_triangle: Triangle::new(Point::new(16, 2), Point::new(10, 14), Point::new(22, 14)),
            down_triangle: Triangle::new(Point::new(10, 2), Point::new(22, 2), Point::new(16, 14)),
            right_triangle: Triangle::new(
                Point::new(22, 119),
                Point::new(10, 113),
                Point::new(10, 125),
            ),
            left_triangle: Triangle::new(
                Point::new(10, 119),
                Point::new(22, 113),
                Point::new(22, 125),
            ),
            guidance_center: Point::new(16, 64),
            arc_radius: 9,
            arrow_length: 24.0,
            arrowhead_size: 8.0,
            status_position: TextPosition {
                start: Point::new(64, 16),
                vertical: VerticalPosition::Center,
                horizontal: HorizontalAlignment::Center,
            },
            tilt_position: TextPosition {
                start: Point::new(16, 24),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Center,
            },
            rot_position: TextPosition {
                start: Point::new(16, 103),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Center,
            },
            dec_label_position: TextPosition {
                start: Point::new(16, 0),
                vertical: VerticalPosition::Top,
                horizontal: HorizontalAlignment::Center,
            },
            ra_label_position: TextPosition {
                start: Point::new(16, 127),
                vertical: VerticalPosition::Baseline,
                horizontal: HorizontalAlignment::Center,
            },
        };
        Self {
            parent,
            rotation,
            status_font: FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>(),
            guidance_font: FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>(),
            fg_color,
            bg_color,
            stale_color,
            triangle_style: PrimitiveStyle::with_fill(fg_color),
            triangle_stale_style: PrimitiveStyle::with_stroke(fg_color, 1),
            arrow_shaft_style: PrimitiveStyle::with_stroke(fg_color, 3),
            arrowhead_style: PrimitiveStyle::with_fill(fg_color),
            arc_style: PrimitiveStyle::with_stroke(fg_color, 3),
            default_rendering,
            rotated_rendering,
            rotate_status: false,
            high_precision: false,
        }
    }

    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }

    pub fn clear(&mut self) {
        self.parent.clear(self.bg_color).unwrap();
    }

    fn is_rotated(&self) -> bool {
        self.rotation == Rotation::Deg90 || self.rotation == Rotation::Deg270
    }
}

// Wrapper that implements DrawTarget, used to handle rotation and avoid self-referential borrows
// in RotatedDisplay.
struct RotatedTarget<'a, D> {
    parent: &'a mut D,
    rotation: Rotation,
}

impl<'a, D> OriginDimensions for RotatedTarget<'a, D>
where
    D: OriginDimensions,
{
    fn size(&self) -> Size {
        let size = self.parent.size();
        match self.rotation {
            Rotation::Deg0 | Rotation::Deg180 => size,
            Rotation::Deg90 | Rotation::Deg270 => Size::new(size.height, size.width),
        }
    }
}

impl<'a, D> DrawTarget for RotatedTarget<'a, D>
where
    D: DrawTarget + OriginDimensions,
{
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let size = self.parent.size();
        let max_x = size.width as i32 - 1;
        let max_y = size.height as i32 - 1;

        let rotated_pixels = pixels.into_iter().map(|Pixel(pt, color)| {
            let rotated_point = match self.rotation {
                Rotation::Deg0 => pt,
                Rotation::Deg90 => Point::new(max_x - pt.y, pt.x),
                Rotation::Deg180 => Point::new(max_x - pt.x, max_y - pt.y),
                Rotation::Deg270 => Point::new(pt.y, max_y - pt.x),
            };
            Pixel(rotated_point, color)
        });

        self.parent.draw_iter(rotated_pixels)
    }
}

// Draw the UI to any target display
pub fn draw_ui<D, C>(display: &mut RotatedDisplay<D, C>, state: &DrawState)
where
    D: DrawTarget<Color = C> + OriginDimensions,
    C: PixelColor,
    D::Error: std::fmt::Debug,
{
    match state {
        DrawState::Message(msg) => {
            let is_rotated = display.is_rotated();

            // Construct target to allow splitting borrows
            let mut target = RotatedTarget {
                parent: &mut display.parent,
                rotation: if display.rotate_status {
                    display.rotation
                } else {
                    Rotation::Deg0
                },
            };

            let positions = if is_rotated {
                &display.rotated_rendering
            } else {
                &display.default_rendering
            };

            display
                .status_font
                .render_aligned(
                    msg.as_str(),
                    positions.status_position.start,
                    positions.status_position.vertical,
                    positions.status_position.horizontal,
                    FontColor::Transparent(display.fg_color),
                    &mut target,
                )
                .unwrap();
        }
        DrawState::Operating(s, stale) => {
            draw_operating_state(display, s, *stale);
        }
    }
}

fn draw_operating_state<D, C>(
    display: &mut RotatedDisplay<D, C>,
    state: &ServerState,
    stale_angle: Option<u32>,
) where
    D: DrawTarget<Color = C> + OriginDimensions,
    C: PixelColor,
    D::Error: std::fmt::Debug,
{
    let is_current = stale_angle.is_none();
    let tilt = state.tilt_target_distance;
    let rot = state.rotation_target_distance;
    let is_rotated = display.is_rotated();

    // Construct target to allow splitting borrows
    let mut target = RotatedTarget {
        parent: &mut display.parent,
        rotation: display.rotation,
    };

    let positions = if is_rotated {
        &display.rotated_rendering
    } else {
        &display.default_rendering
    };
    let high_precision = display.high_precision;

    display
        .guidance_font
        .render_aligned(
            format_offset(tilt, high_precision).as_str(),
            positions.tilt_position.start,
            positions.tilt_position.vertical,
            positions.tilt_position.horizontal,
            FontColor::Transparent(display.fg_color),
            &mut target,
        )
        .unwrap();

    display
        .guidance_font
        .render_aligned(
            format_offset(rot, high_precision).as_str(),
            positions.rot_position.start,
            positions.rot_position.vertical,
            positions.rot_position.horizontal,
            FontColor::Transparent(display.fg_color),
            &mut target,
        )
        .unwrap();

    if !state.is_alt_az {
        let color = if is_current {
            display.fg_color
        } else {
            display.stale_color
        };
        display
            .guidance_font
            .render_aligned(
                if tilt > 0.0 { "N" } else { "S" },
                positions.dec_label_position.start,
                positions.dec_label_position.vertical,
                positions.dec_label_position.horizontal,
                FontColor::Transparent(color),
                &mut target,
            )
            .unwrap();

        display
            .guidance_font
            .render_aligned(
                if rot > 0.0 { "E" } else { "W" },
                positions.ra_label_position.start,
                positions.ra_label_position.vertical,
                positions.ra_label_position.horizontal,
                FontColor::Transparent(color),
                &mut target,
            )
            .unwrap();
    } else {
        let tri_style = if is_current {
            display.triangle_style
        } else {
            display.triangle_stale_style
        };
        if tilt > 0.0 {
            positions.up_triangle
        } else {
            positions.down_triangle
        }
        .into_styled(tri_style)
        .draw(&mut target)
        .unwrap();

        if rot > 0.0 {
            positions.right_triangle
        } else {
            positions.left_triangle
        }
        .into_styled(tri_style)
        .draw(&mut target)
        .unwrap();
    }

    if !is_current {
        DisplayArc::new(
            Point::new(
                positions.guidance_center.x - positions.arc_radius,
                positions.guidance_center.y - positions.arc_radius,
            ),
            (positions.arc_radius * 2) as u32,
            (stale_angle.unwrap() as f32).deg(),
            90.0.deg(),
        )
        .into_styled(display.arc_style)
        .draw(&mut target)
        .unwrap();
        return;
    }

    let display_angle_rad = (state.target_angle as f64 + 90.0).to_radians();

    let half_len = positions.arrow_length / 2.0;

    let cos_a = display_angle_rad.cos();
    let sin_a = display_angle_rad.sin();

    let tip = Point::new(
        positions.guidance_center.x + (half_len * cos_a) as i32,
        positions.guidance_center.y - (half_len * sin_a) as i32,
    );

    let tail = Point::new(
        positions.guidance_center.x - (half_len * cos_a) as i32,
        positions.guidance_center.y + (half_len * sin_a) as i32,
    );

    let head_base_offset = half_len - positions.arrowhead_size;
    let head_base_center = Point::new(
        positions.guidance_center.x + (head_base_offset * cos_a) as i32,
        positions.guidance_center.y - (head_base_offset * sin_a) as i32,
    );

    let angle_perp_plus = display_angle_rad + std::f64::consts::FRAC_PI_2;
    let angle_perp_minus = display_angle_rad - std::f64::consts::FRAC_PI_2;
    let half_width = positions.arrowhead_size / 2.0;

    let corner1 = Point::new(
        head_base_center.x + (half_width * angle_perp_plus.cos()) as i32,
        head_base_center.y - (half_width * angle_perp_plus.sin()) as i32,
    );

    let corner2 = Point::new(
        head_base_center.x + (half_width * angle_perp_minus.cos()) as i32,
        head_base_center.y - (half_width * angle_perp_minus.sin()) as i32,
    );

    Line::new(tail, head_base_center)
        .into_styled(display.arrow_shaft_style)
        .draw(&mut target)
        .unwrap();

    Triangle::new(tip, corner1, corner2)
        .into_styled(display.arrowhead_style)
        .draw(&mut target)
        .unwrap();
}

fn format_offset(num: f64, high_precision: bool) -> String {
    let n = num.abs();
    if n >= 100.0 || (!high_precision && n >= 10.0) {
        format!("{:.0}", n)
    } else if high_precision && n < 10.0 {
        format!("{:.2}", n)
    } else {
        format!("{:.1}", n)
    }
}
