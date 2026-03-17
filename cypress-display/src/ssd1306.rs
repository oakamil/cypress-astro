// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use crate::display::TargetDisplay;

use async_trait::async_trait;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use rppal::i2c::I2c;
use ssd1306::{
    I2CDisplayInterface, Ssd1306 as Ssd1306Driver, mode::BufferedGraphicsMode, prelude::*,
};
use std::error::Error;

// Configuration trait to specify display dimensions
pub trait Ssd1306Config: Send + Sync
where
    <Self::Size as DisplaySize>::Buffer: Send + Sync,
{
    type Size: DisplaySize + Send + Sync;
    fn create_size() -> Self::Size;
}

pub struct Config128x32;
impl Ssd1306Config for Config128x32 {
    type Size = DisplaySize128x32;
    fn create_size() -> Self::Size {
        DisplaySize128x32
    }
}

pub struct Config128x64;
impl Ssd1306Config for Config128x64 {
    type Size = DisplaySize128x64;
    fn create_size() -> Self::Size {
        DisplaySize128x64
    }
}

type Interface = I2CInterface<I2c>;

pub struct Ssd1306Display<C: Ssd1306Config>
where
    <C::Size as DisplaySize>::Buffer: Send + Sync,
{
    driver: Ssd1306Driver<Interface, C::Size, BufferedGraphicsMode<C::Size>>,
}

impl<C: Ssd1306Config> Ssd1306Display<C>
where
    <C::Size as DisplaySize>::Buffer: Send + Sync,
{
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let i2c = I2c::new()?;
        let interface = I2CDisplayInterface::new(i2c);

        let driver = Ssd1306Driver::new(interface, C::create_size(), DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

        Ok(Self { driver })
    }
}

#[async_trait]
impl<C: Ssd1306Config> TargetDisplay for Ssd1306Display<C>
where
    <C::Size as DisplaySize>::Buffer: Send + Sync,
{
    async fn turn_on(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .init()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;
        self.driver
            .set_display_on(true)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn turn_off(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .set_display_on(false)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .flush()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<(), Box<dyn Error>> {
        self.driver
            .set_brightness(Brightness::custom(1, brightness))
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }
}

impl<C: Ssd1306Config> DrawTarget for Ssd1306Display<C>
where
    <C::Size as DisplaySize>::Buffer: Send + Sync,
{
    type Color = BinaryColor;
    type Error = Box<dyn Error>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.driver
            .draw_iter(pixels)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.driver
            .clear(color)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }
}

impl<C: Ssd1306Config> OriginDimensions for Ssd1306Display<C>
where
    <C::Size as DisplaySize>::Buffer: Send + Sync,
{
    fn size(&self) -> Size {
        self.driver.size()
    }
}

pub type Ssd1306_128_32 = Ssd1306Display<Config128x32>;
pub type Ssd1306_128_64 = Ssd1306Display<Config128x64>;
