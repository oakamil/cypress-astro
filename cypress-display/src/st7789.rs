// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use crate::display::TargetDisplay;

use async_trait::async_trait;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*, primitives::Rectangle};
use embedded_graphics_framebuf::FrameBuf;
use linux_embedded_hal::Delay;
use mipidsi::{Builder, Display, interface::SpiInterface, models::ST7789, options::ColorInversion};
use rppal::{
    gpio::{Gpio, OutputPin},
    pwm::{Channel, Polarity, Pwm},
    spi::{Bus, Mode, SimpleHalSpiDevice, SlaveSelect, Spi},
};
use std::error::Error;
use std::marker::PhantomData;

// Trait to configure resolution and offsets for various ST7789 screen variations
pub trait St7789Config: Send + Sync {
    const WIDTH: u16;
    const HEIGHT: u16;
    const OFFSET_X: u16;
    const OFFSET_Y: u16;
}

pub struct Display135x240;
impl St7789Config for Display135x240 {
    const WIDTH: u16 = 135;
    const HEIGHT: u16 = 240;
    const OFFSET_X: u16 = 52;
    const OFFSET_Y: u16 = 40;
}

pub struct Display240x240;
impl St7789Config for Display240x240 {
    const WIDTH: u16 = 240;
    const HEIGHT: u16 = 240;
    const OFFSET_X: u16 = 0;
    const OFFSET_Y: u16 = 0;
}

pub struct St7789Display<C: St7789Config, const N: usize> {
    display: Display<SpiInterface<'static, SimpleHalSpiDevice, OutputPin>, ST7789, OutputPin>,
    // Boxed fixed-size array for the in-memory framebuffer required by `embedded_graphics_framebuf`
    buffer: Box<[Rgb565; N]>,
    bl: Pwm,
    _config: PhantomData<C>,
}

impl<C: St7789Config, const N: usize> St7789Display<C, N> {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 40_000_000, Mode::Mode0)?;

        let gpio = Gpio::new()?;
        let dc = gpio.get(25)?.into_output();
        let rst = gpio.get(27)?.into_output();

        let bl = Pwm::with_frequency(Channel::Pwm0, 100.0, 0.0, Polarity::Normal, false)?;

        // mipidsi 0.10.0+ internal SPI batching buffer
        let spi_buffer = vec![0u8; 1024];
        let spi_buffer_slice: &'static mut [u8] = Box::leak(spi_buffer.into_boxed_slice());

        let spii = SpiInterface::new(SimpleHalSpiDevice::new(spi), dc, spi_buffer_slice);

        let mut display = Builder::new(ST7789, spii)
            .reset_pin(rst)
            .display_size(C::WIDTH, C::HEIGHT)
            .display_offset(C::OFFSET_X, C::OFFSET_Y)
            .invert_colors(ColorInversion::Inverted)
            .init(&mut Delay)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;

        // Allocate the main rendering framebuffer on the heap
        let vec_buffer = vec![Rgb565::BLACK; N];
        let buffer = vec_buffer
            .into_boxed_slice()
            .try_into()
            .map_err(|_| "Failed to convert buffer to fixed-size array")?;

        display
            .clear(Rgb565::BLACK)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;

        Ok(Self {
            display,
            buffer,
            bl,
            _config: PhantomData,
        })
    }
}

#[async_trait]
impl<C: St7789Config, const N: usize> TargetDisplay for St7789Display<C, N> {
    async fn turn_on(&mut self) -> Result<(), Box<dyn Error>> {
        self.bl
            .set_duty_cycle(0.5)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;

        self.bl
            .enable()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn turn_off(&mut self) -> Result<(), Box<dyn Error>> {
        self.bl
            .set_duty_cycle(0.0)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;

        self.bl
            .disable()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        let screen_area =
            Rectangle::new(Point::zero(), Size::new(C::WIDTH as u32, C::HEIGHT as u32));
        self.display
            .fill_contiguous(&screen_area, self.buffer.iter().copied())
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<(), Box<dyn Error>> {
        let duty_cycle = (brightness as f64) / 255.0;

        self.bl
            .set_duty_cycle(duty_cycle)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }
}

impl<C: St7789Config, const N: usize> DrawTarget for St7789Display<C, N> {
    type Color = Rgb565;
    type Error = Box<dyn Error>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let mut fb = FrameBuf::new(&mut *self.buffer, C::WIDTH as usize, C::HEIGHT as usize);
        fb.draw_iter(pixels)
            .map_err(|_| Box::<dyn Error>::from("Framebuffer draw error"))
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let mut fb = FrameBuf::new(&mut *self.buffer, C::WIDTH as usize, C::HEIGHT as usize);
        fb.fill_solid(area, color)
            .map_err(|_| Box::<dyn Error>::from("Framebuffer fill error"))
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let mut fb = FrameBuf::new(&mut *self.buffer, C::WIDTH as usize, C::HEIGHT as usize);
        fb.fill_contiguous(area, colors)
            .map_err(|_| Box::<dyn Error>::from("Framebuffer fill error"))
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let mut fb = FrameBuf::new(&mut *self.buffer, C::WIDTH as usize, C::HEIGHT as usize);
        fb.clear(color)
            .map_err(|_| Box::<dyn Error>::from("Framebuffer clear error"))
    }
}

impl<C: St7789Config, const N: usize> OriginDimensions for St7789Display<C, N> {
    fn size(&self) -> Size {
        Size::new(C::WIDTH as u32, C::HEIGHT as u32)
    }
}

// Aliases for convenience
pub type St7789_135_240 = St7789Display<Display135x240, { 135 * 240 }>;
pub type St7789_240_240 = St7789Display<Display240x240, { 240 * 240 }>;
