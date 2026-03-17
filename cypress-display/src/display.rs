// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait TargetDisplay: Send {
    async fn turn_on(&mut self) -> Result<(), Box<dyn Error>>;
    async fn turn_off(&mut self) -> Result<(), Box<dyn Error>>;
    async fn flush(&mut self) -> Result<(), Box<dyn Error>>;
    async fn set_brightness(&mut self, brightness: u8) -> Result<(), Box<dyn Error>>;
}
