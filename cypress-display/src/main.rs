// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

mod cedar_client;
mod display;
mod prefs;
mod renderer;
mod ssd1306;
mod ssd1309;
mod ssd1351;
mod st7789;
mod web;

use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};

use cedar_client::{CedarClient, CedarResponse, ResponseStatus, ServerMode, ServerState};
use display::TargetDisplay;
use embedded_graphics::{
    draw_target::DrawTarget, geometry::OriginDimensions, pixelcolor::PixelColor,
};
use renderer::{DrawState, RotatedDisplay, Rotation, draw_ui};
use rppal::gpio::{Event, Gpio, InputPin, Trigger};
use simple_signal::{self, Signal};
use tokio::time::sleep;
use web::{Framebuffer, ServerContext};

struct FakeStateProvider {
    tilt: f64,
    rot: f64,
    angle: f64,
    has_solution: bool,
    is_alt_az: bool,
}

impl FakeStateProvider {
    fn new() -> Self {
        FakeStateProvider {
            tilt: 123.8,
            rot: -45.3,
            angle: 0.0,
            has_solution: true,
            is_alt_az: true,
        }
    }

    fn get_next_response(&mut self) -> CedarResponse {
        self.angle = (self.angle + 9.0) % 360.0;
        let delta = if self.has_solution { 1.17 } else { 10.0 };
        self.tilt = self.tilt + delta;
        if self.tilt > 180.0 {
            self.tilt = -179.2;
        }
        self.rot = self.rot - delta * 2.0;
        if self.rot < -180.0 {
            self.rot = 179.7;
            if !self.has_solution {
                self.is_alt_az = !self.is_alt_az;
            }
            self.has_solution = !self.has_solution;
        }
        CedarResponse {
            status: ResponseStatus::Success,
            server_state: Some(ServerState {
                server_mode: ServerMode::Operating,
                is_alt_az: self.is_alt_az,
                has_slew_request: self.has_solution,
                rotation_target_distance: self.rot,
                tilt_target_distance: self.tilt,
                target_angle: self.angle,
                has_solution: self.has_solution,
            }),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = pico_args::Arguments::from_env();

    let mirror_enabled = args.contains("--mirror");
    let test_mode = args.contains("--test");
    let buttons_enabled = args.contains("--buttons");

    let cli_brightness = match args.opt_value_from_str::<_, u32>("--brightness")? {
        Some(val) if (1..=255).contains(&val) => Some(val as u8),
        Some(_) => return Err("Brightness must be between 1 and 255".into()),
        None => None,
    };

    let cli_rotation = match args.opt_value_from_str::<_, u16>("--rotation")? {
        Some(val) if val == 0 || val == 90 || val == 180 || val == 270 => Some(val),
        Some(_) => return Err("Rotation must be one of 0, 90, 180, or 270".into()),
        None => None,
    };

    let display_type = match args.opt_value_from_str::<_, u32>("--type")? {
        Some(val) if (1..=6).contains(&val) => val,
        Some(_) => return Err("Type must be between 1 and 6".into()),
        None => 1,
    };

    let file_brightness = prefs::load_brightness();
    let initial_brightness = cli_brightness.unwrap_or(file_brightness);

    let file_rotation = prefs::load_rotation();
    let initial_rotation = cli_rotation.unwrap_or(file_rotation);
    let current_rotation = Rotation::from_degrees(initial_rotation);

    let shared_brightness = Arc::new(AtomicU8::new(initial_brightness));
    let shared_rotation = Arc::new(AtomicU16::new(initial_rotation));

    // Initialize shared frame only if the mirror is enabled to save memory
    let shared_frame = Arc::new(RwLock::new(if mirror_enabled {
        vec![0u8; 256 * 256 * 2]
    } else {
        Vec::new()
    }));

    let server_ctx = ServerContext {
        brightness: shared_brightness.clone(),
        rotation: shared_rotation.clone(),
        frame: shared_frame.clone(),
    };

    // Store button handles to keep pins/interrupts alive
    let _button_handles = if buttons_enabled {
        start_button_monitor(shared_brightness.clone(), shared_rotation.clone())
    } else {
        Vec::new()
    };

    web::start_server(server_ctx)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    simple_signal::set_handler(&[Signal::Int, Signal::Term], move |signal_rec| {
        println!("Signal received : '{:?}'", signal_rec);
        r.store(false, Ordering::SeqCst);
    });

    // Initialize and use specific display wrapper
    match display_type {
        1 => {
            let raw_disp = ssd1351::Ssd1351::new()?;
            let disp = RotatedDisplay::new_rgb_128_128(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
        2 => {
            let raw_disp = ssd1306::Ssd1306_128_64::new()?;
            let disp = RotatedDisplay::new_binary_128_64(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
        3 => {
            let raw_disp = ssd1306::Ssd1306_128_32::new()?;
            let disp = RotatedDisplay::new_binary_128_32(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
        4 => {
            let raw_disp = ssd1309::Ssd1309::new()?;
            let disp = RotatedDisplay::new_binary_128_64(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
        5 => {
            let raw_disp = st7789::St7789_135_240::new()?;
            let disp = RotatedDisplay::new_rgb_135_240(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
        _ => {
            let raw_disp = st7789::St7789_240_240::new()?;
            let disp = RotatedDisplay::new_rgb_240_240(raw_disp, current_rotation);
            run_display(
                disp,
                running,
                shared_brightness,
                shared_rotation,
                test_mode,
                mirror_enabled,
                display_type,
                shared_frame,
            )
            .await?
        }
    };
    Ok(())
}

async fn run_display<D, C>(
    mut disp: RotatedDisplay<D, C>,
    running: Arc<AtomicBool>,
    shared_brightness: Arc<AtomicU8>,
    shared_rotation: Arc<AtomicU16>,
    test_mode: bool,
    mirror_enabled: bool,
    display_type: u32,
    shared_frame: Arc<RwLock<Vec<u8>>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    D: TargetDisplay + DrawTarget<Color = C> + OriginDimensions,
    C: PixelColor,
    D::Error: std::fmt::Debug,
{
    disp.parent.turn_on().await?;

    let initial_brightness = shared_brightness.load(Ordering::Relaxed);
    let mut current_brightness = initial_brightness;
    disp.parent.set_brightness(current_brightness).await?;

    let mut current_rotation = Rotation::from_degrees(shared_rotation.load(Ordering::Relaxed));

    // Virtual framebuffer for web rendering
    let mut web_fb = if mirror_enabled {
        Some(match display_type {
            1 => RotatedDisplay::new_rgb_128_128(
                Framebuffer::new_with_pixel_doubling(),
                current_rotation,
            ),
            3 => RotatedDisplay::new_rgb_128_32(
                Framebuffer::new_with_pixel_doubling(),
                current_rotation,
            ),
            5 => RotatedDisplay::new_rgb_135_240(Framebuffer::new(), current_rotation),
            6 => RotatedDisplay::new_rgb_240_240(Framebuffer::new(), current_rotation),
            _ => RotatedDisplay::new_rgb_128_64(
                Framebuffer::new_with_pixel_doubling(),
                current_rotation,
            ),
        })
    } else {
        None
    };

    let mut client = CedarClient::new();
    let mut last_slew: Option<ServerState> = None;
    let mut stale_angle = 0;

    let mut fake_provider = FakeStateProvider::new();

    let mut update_time = Instant::now();

    while running.load(Ordering::SeqCst) {
        let target_brightness = shared_brightness.load(Ordering::Relaxed);
        if target_brightness != current_brightness {
            println!("Updating display brightness to {}", target_brightness);
            disp.parent.set_brightness(target_brightness).await?;
            current_brightness = target_brightness;
        }

        let target_rotation_deg = shared_rotation.load(Ordering::Relaxed);
        let target_rotation = Rotation::from_degrees(target_rotation_deg);
        if target_rotation != current_rotation {
            println!("Updating display rotation to {}", target_rotation_deg);
            disp.set_rotation(target_rotation);
            if let Some(fb) = &mut web_fb {
                fb.set_rotation(target_rotation);
            }
            current_rotation = target_rotation;
        }

        let resp = if test_mode {
            fake_provider.get_next_response()
        } else {
            client.get_state().await
        };
        let draw_state = if resp.status != ResponseStatus::Success {
            DrawState::Message(format!("{:?}", resp.status))
        } else if let Some(state) = &resp.server_state {
            match state.server_mode {
                ServerMode::Operating => {
                    if !state.has_slew_request {
                        if state.has_solution {
                            last_slew = None;
                        }
                        if let Some(slew) = &last_slew {
                            let state = DrawState::Operating(slew, Some(stale_angle));
                            stale_angle = (stale_angle + 9) % 360;
                            state
                        } else {
                            DrawState::Message("No Target".to_string())
                        }
                    } else {
                        last_slew = Some(state.clone());
                        DrawState::Operating(state, None)
                    }
                }
                ServerMode::Calibrating => DrawState::Message("Calibrating".to_string()),
                _ => DrawState::Message("Setup Mode".to_string()),
            }
        } else {
            DrawState::Message("...".to_string())
        };

        // Draw to physical display
        disp.clear();
        draw_ui(&mut disp, &draw_state);
        let _ = disp.parent.flush().await;

        // Draw to virtual framebuffer
        if mirror_enabled {
            if let Some(fb) = &mut web_fb {
                fb.clear();
                draw_ui(fb, &draw_state);

                if let Ok(mut lock) = shared_frame.write() {
                    lock.copy_from_slice(fb.parent.as_bytes());
                }
            }
        }

        let elapsed = update_time.elapsed().as_millis() as u64;
        if test_mode {
            println!("Rendered frame in {} ms", elapsed);
        }
        if elapsed < 100 {
            sleep(Duration::from_millis(100 - elapsed)).await;
        }
        update_time = Instant::now();
    }

    disp.parent.turn_off().await?;
    Ok(())
}

// Returned Vec<InputPin> must be kept alive for interrupts to function
fn start_button_monitor(brightness: Arc<AtomicU8>, rotation: Arc<AtomicU16>) -> Vec<InputPin> {
    let mut kept_pins = Vec::new();

    let gpio = match Gpio::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("GPIO initialization failed (Buttons disabled): {}", e);
            return Vec::new();
        }
    };

    // Helper to configure a pin, set async interrupt, and store it
    let mut setup_pin =
        |pin_num: u8, callback: Box<dyn FnMut(Event) + Send + 'static>| match gpio.get(pin_num) {
            Ok(p) => {
                let mut pin = p.into_input_pullup();
                match pin.set_async_interrupt(
                    Trigger::FallingEdge,
                    Some(Duration::from_millis(50)),
                    callback,
                ) {
                    Ok(_) => {
                        println!("Button enabled on Pin {}", pin_num);
                        kept_pins.push(pin);
                    }
                    Err(e) => eprintln!("Failed to set interrupt for Pin {}: {}", pin_num, e),
                }
            }
            Err(e) => eprintln!("Failed to get Pin {}: {}", pin_num, e),
        };

    // Pin 21: Increase Brightness
    let b_inc = brightness.clone();
    setup_pin(
        21,
        Box::new(move |_| {
            println!("Button: Brightness Up");
            let current = b_inc.load(Ordering::Relaxed);
            let new_val = ((current + 8) % 255).max(5);
            b_inc.store(new_val, Ordering::Relaxed);
            prefs::save_brightness(new_val);
        }),
    );

    // Pin 16: Decrease Brightness
    let b_dec = brightness.clone();
    setup_pin(
        16,
        Box::new(move |_| {
            println!("Button: Brightness Down");
            let current = b_dec.load(Ordering::Relaxed);
            let new_val = ((current - 8) % 255).max(5);
            b_dec.store(new_val, Ordering::Relaxed);
            prefs::save_brightness(new_val);
        }),
    );

    // Pin 20: Cycle Rotation
    let r_cycle = rotation.clone();
    setup_pin(
        20,
        Box::new(move |_| {
            println!("Button: Rotation");
            let current = r_cycle.load(Ordering::Relaxed);
            let new_val = match current {
                0 => 90,
                90 => 180,
                180 => 270,
                _ => 0,
            };
            r_cycle.store(new_val, Ordering::Relaxed);
            prefs::save_rotation(new_val);
        }),
    );

    kept_pins
}
