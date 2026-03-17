// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use crate::display::TargetDisplay;

use async_trait::async_trait;
use display_interface_spi::SPIInterface;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use linux_embedded_hal::Delay;
use rppal::{
    gpio::{Gpio, OutputPin},
    spi::{Bus, Mode, SimpleHalSpiDevice, SlaveSelect, Spi},
};
use ssd1351::display::display::Ssd1351 as Ssd1351Driver;
use std::error::Error;

pub struct Ssd1351 {
    driver: Ssd1351Driver<SPIInterface<SimpleHalSpiDevice, OutputPin>>,
    rst: OutputPin,
}

impl Ssd1351 {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 19660800, Mode::Mode0)?;
        let gpio = Gpio::new()?;
        let dc = gpio.get(25)?.into_output();
        let rst = gpio.get(27)?.into_output();

        let spii = SPIInterface::new(SimpleHalSpiDevice::new(spi), dc);
        let driver = Ssd1351Driver::new(spii);

        Ok(Self { driver, rst })
    }
}

#[async_trait]
impl TargetDisplay for Ssd1351 {
    async fn turn_on(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .reset(&mut self.rst, &mut Delay)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;
        self.driver
            .turn_on()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn turn_off(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .reset(&mut self.rst, &mut Delay)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))?;
        self.driver
            .turn_off()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver
            .flush()
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }

    async fn set_brightness(&mut self, brightness: u8) -> Result<(), Box<dyn Error>> {
        self.driver
            .set_brightness(brightness)
            .map_err(|e| Box::<dyn Error>::from(format!("{:?}", e)))
    }
}

impl DrawTarget for Ssd1351 {
    type Color = Rgb565;
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

impl OriginDimensions for Ssd1351 {
    fn size(&self) -> Size {
        self.driver.size()
    }
}
