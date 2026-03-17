// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use crate::display::TargetDisplay;
use async_trait::async_trait;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use rppal::i2c::I2c;
use ssd1306::{I2CDisplayInterface, prelude::I2CInterface};
use ssd1309::{Builder, NoOutputPin, displayrotation::DisplayRotation, mode::GraphicsMode};
use std::convert::Infallible;
use std::error::Error;
use std::thread;
use std::time::Duration;

pub struct Ssd1309 {
    driver: GraphicsMode<I2CInterface<I2c>>,
}

// Simple delay implementation using std::thread::sleep
// required by the SSD1309 reset sequence.
struct Delay;

impl embedded_hal::delay::DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        thread::sleep(Duration::from_nanos(ns as u64));
    }
}

impl Ssd1309 {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let i2c = I2c::new()?;
        let interface = I2CDisplayInterface::new(i2c);

        let mut driver: GraphicsMode<_> = Builder::new()
            .with_rotation(DisplayRotation::Rotate0)
            .connect(interface)
            .into();

        let mut rst = NoOutputPin::<Infallible>::new();
        let mut delay = Delay;

        driver
            .reset(&mut rst, &mut delay)
            .map_err(|e| Box::<dyn Error>::from(format!("Reset failed: {:?}", e)))?;

        driver
            .init()
            .map_err(|e| Box::<dyn Error>::from(format!("Init failed: {:?}", e)))?;

        Ok(Self { driver })
    }
}

#[async_trait]
impl TargetDisplay for Ssd1309 {
    async fn turn_on(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .display_on(true)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn turn_off(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .display_on(false)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .flush()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<(), Box<dyn Error>> {
        self.driver
            .set_contrast(brightness)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }
}

impl DrawTarget for Ssd1309 {
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
}

impl OriginDimensions for Ssd1309 {
    fn size(&self) -> Size {
        self.driver.size()
    }
}
