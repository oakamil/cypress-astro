# cypress-imu

Hardware integration of the BNO085 IMU (Inertial Measurement Unit) for the [Cedar™](https://github.com/smroid/cedar-server) telescope control system.

This crate is part of the broader `cypress-astro` workspace.

## Overview

The `cypress-imu` package provides an implementation of the Cedar `ImuTrait` using the BNO085 sensor. This allows the Cedar™ telescope control system to estimate positioning between camera plate-solves.

`cypress-imu` supports both standard rotation vector mode (9-axis) and game rotation vector mode (6-axis - the magnetometer is disabled).

## Getting Started

### Prerequisites

* [Rust / Cargo](https://rustup.rs/)
* A connected BNO085 IMU hardware module via I2C. Only 4 physical connections are needed as the sensor's interrupt and reset pins aren't used.

### Building

You can build the crate directly using cargo:

```
cargo build --release
```

### Testing

A test binary is included to verify the sensor is properly connected and transmitting data. To run the IMU test binary:

```
cargo run --bin test_imu
```

## License

This project is licensed under the Apache License 2.0.

See `LICENSE.md` file for full details.

### Third-Party Licenses

While `cypress-imu` itself is licensed under the Apache License 2.0, it integrates with and depends on several external projects (including but not limited to `cedar-server`). Each of these third-party projects is governed by its own respective licensing terms. Users are responsible for reviewing and complying with the individual licenses of any integrated components, tools, or dependencies.

## Disclaimer

All product names, trademarks and registered trademarks are property of their respective owners. All company, product and service names used in this website are for identification purposes only. Use of these names, trademarks and brands does not imply endorsement.

`cypress-astro` is not affiliated with, endorsed by, or sponsored by Clear Skies Astro.

Cedar™ is a trademark of Clear Skies Astro, registered in the U.S. and other countries.
