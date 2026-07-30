# Cypress Display

`cypress-display` provides a display controller designed to work alongside the [Cedar™](https://github.com/smroid/cedar) telescope control system. 

`cypress-display` drives a hardware display, consuming data from Cedar™ server to display its PushTo guidance.

![Cypress Demo Small](https://github.com/user-attachments/assets/1374c194-d611-47f5-b0d9-2200074225a2)

This crate is part of the broader `cypress-astro` workspace.

## Hardware Requirements

This software is intended to run on Linux-based embedded hardware, specifically the **Raspberry Pi**, as it utilizes hardware-specific HALs (e.g. `rppal`, `linux-embedded-hal`).

### Supported Displays

| Type | Size | Color | Resolution | Driver | Interface | Wiring | Tested Sample | Draw Time* |
| ---| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1.5" | RGB | 128x128 | SSD1351 | SPI | [Waveshare wiring diagram](https://www.waveshare.com/img/devkit/LCD/1.5inch-RGB-OLED-Module/1.5inch-RGB-OLED-Module-details-5.jpg) | [Waveshare](https://www.waveshare.com/1.5inch-rgb-oled-module.htm) | 24 ms |
| 2 | 0.96" | White | 128x64 | SSD1306 | I2C | Connect VCC on the display to pin 1 on the Raspberry Pi, GND to pin 6, DIN (SDA) to pin 3, and CLK (SCL) to pin 5. | [Makerfocus](https://www.makerfocus.com/products/2pcs-i2c-oled-display-module-0-91-inch-i2c-ssd1306-oled-display-module-1?variant=31333400608845) | 45 ms |
| 3 | 0.91" | White | 128x32 | SSD1306 | I2C | Same as above | [HiLetgo](https://a.co/d/04xFymht) | 22 ms |
| 4 | 1.54" | White | 128x64 | SSD1309 | I2C | Same as above | [Coliao](https://a.co/d/0aAaMkcY) | 50 ms |
| 5 | 1.14" | RGB (LCD) | 135x240 | ST7789 | SPI | [Wiring diagram](https://www.waveshare.com/wiki/1.14inch_LCD_Module), but move BL\*\* to Board 32 on a Raspberry Pi 5 | [JESSINIE](https://a.co/d/0iwukeR2) | 15 ms | 
| 6 | 1.3" | RGB (LCD) | 240x240 | ST7789 | SPI | Same as #5\*\*\* | [Waveshare](https://a.co/d/07qvCxcw) | 20 ms |

\* Draw time is the typical time it takes to draw a frame as measured on a Raspberry Pi Zero 2W. Observed draw times were ~40% shorter on a Raspberry Pi 5 for I2C and ~20% shorter on SPI.

\*\* Enable hardware PWM. On the Raspberry Pi 5 add "dtoverlay=pwm,pin=12,func=4" to /boot/firmware/config.txt. On older Pi models add "dtoverlay=pwm".

\*\*\* When using the LCD hat BL must be physically redirected to board pin 32 (Pi 5) or 12 (older Pis) instead of board pin 18.

Note that SPI/I2C needs to enabled via `raspi-config`.

Red film is recommended to preserve night adaptation when using white OLED or LCD displays. The white OLED displays are typically much brighter than RGB displays at the lowest brightness settings, so multiple layers of film are advised to reduce the brightness.

## Building

### Software Prerequisites

* **Rust**: Stable toolchain (edition 2024).
* **Cedar™ Server**: This application expects `cedar-server` to be running on `localhost:80`.

### Build Instructions

You can build the project using the provided build script.

```Bash
./build.sh
```

This will place the binary and the web content into the directory `out/cypress/bin`.

## Usage

### cypress-display

The display driver daemon. It connects to the hardware display and queries Cedar™ server to render the UI.

```Bash
cd out/cypress/bin
./cypress-display --brightness 128
```
* `--brightness`: (Optional) Set physical display brightness (1-255). Default is 128 (50%).
* `--buttons`: (Optional) Enable buttons for rotation and brightness.
* `--mirror`: (Optional) Mirror the physical display to the web UI.
* `--rotate`: (Optional) Set physical display clockwise rotation (0, 90, 180, or 270). Default is 0.
* `--type`: (Optional) Set physical display type. 1 = RGB 128x128, 2 = Mono 128x64 (SSD1306), 3 = Mono 128x32 (SSD1306), 4 = Mono 128x64 (SSD1309), 5 = RGB 135x240 (ST7789), 6 = RGB 240x240 (ST7789). Default is 1.
* `--test`: (Optional) Test mode that cycles through various guidance values.

### Brightness and Rotation Control

The brightness and rotation can be updated in the field by connecting to the e-finder's WiFi network and accessing `cypress-display`'s control page at `https://192.168.4.1:6030`.

<img width="209" height="195" alt="cypress-control" src="https://github.com/user-attachments/assets/62f27993-ff80-49a5-b918-38d10ef4caed" />

Physical buttons can be used for control as well. Pins match the Waveshare 1.3 LCD hat:

* GPIO21 (Board 40) - brightness up (KEY1)
* GPIO20 (Board 38) - rotation (KEY2)
* GPIO16 (Board 36) - brightness down (KEY3)

### Display Mirror

`cypress-display` includes the ability to mirror the displayed output to the web UI.

```Bash
./cypress-display.sh --mirror
```

The mirrored display is available at `https://192.168.4.1:6030/mirror`. The mirrored display can be used without the presence of a physical screen as long as SPI is enabled on the e-finder device.

## Installation

A distribution zipfile is provided [here](https://github.com/oakamil/cypress-astro/raw/refs/heads/main/cypress-display/dist/cypress-display.zip).

### Download Instructions

If your Cedar™ e-finder device has internet access the distribution archive can be downloaded directly:

```Bash
wget https://github.com/oakamil/cypress-astro/raw/refs/heads/main/cypress-display/dist/cypress-display.zip
```

Otherwise you can download the file to a computer and use scp to transfer it to the e-finder device after connecting to its WiFi network:

```Bash
scp ~/Downloads/cypress-display.zip cedar@192.168.4.1:.
```

### Enabling SPI

SPI must be enabled on the e-finder device to use an RGB screen.

```Bash
sudo raspi-config
```

1. Select `3 Interface Options`
2. Select `I4 SPI`
3. Respond `<Yes>` to enable SPI
4. Continue by selecting `<OK>`
5. Select `<Finish>` to exit

### Enabling I2C

I2C must be enabled on the e-finder device to use a binary screen.

```Bash
sudo raspi-config
```

1. Select `3 Interface Options`
2. Select `I5 I2C`
3. Respond `<Yes>` to enable I2C
4. Continue by selecting `<OK>`
5. Select `<Finish>` to exit

For better performance increase the I2C clock speed by adding the following line in /boot/firmware/config.txt and rebooting:

```
dtparam=i2c_arm_baudrate=400000
```

### Install Script

The provided distribution archive includes a script to install `cypress-display` as a service to automatically start when the e-finder boots.

```Bash
unzip cypress-display.zip
./install.sh
```

The installation will set up `cypress-display` for use with a 128x128 RGB display. To use a different display update `cypress-display.service` to add the `--type` argument to the end of `ExecStart`.

## License

This project is licensed under the GNU General Public License v3.0.

See the root `LICENSE.md` file for full details.

### Third-Party Licenses

While `cypress-display` itself is licensed under the GNU General Public License v3.0, it integrates with and depends on several external projects (including but not limited to `cedar-server`). Each of these third-party projects is governed by its own respective licensing terms. Users are responsible for reviewing and complying with the individual licenses of any integrated components, tools, or dependencies.

## Disclaimer

All product names, trademarks and registered trademarks are property of their respective owners. All company, product and service names used in this website are for identification purposes only. Use of these names, trademarks and brands does not imply endorsement.

`cypress-astro` and `cypress-display` are not affiliated with, endorsed by, or sponsored by Clear Skies Astro.

Cedar™ is a trademark of Clear Skies Astro, registered in the U.S. and other countries.
