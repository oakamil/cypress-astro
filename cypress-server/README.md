# cypress-server
Server binary that includes the Cedar™ plate-solving system and the solver from `cypress-solver`.

## Building

Refer to the build instructions on the `cedar-server` [page](https://github.com/smroid/cedar-server/blob/main/building.md#building-from-source).

The following repos will need to be cloned instead of the ones listed on the `cedar-server` page:

```
git clone https://github.com/smroid/asi_camera2.git
git clone https://github.com/smroid/cedar-aim.git
git clone https://github.com/smroid/cedar-camera.git
git clone https://github.com/smroid/cedar-detect.git
git clone https://github.com/smroid/cedar-server.git
git clone https://github.com/smroid/tetra3_server.git
git clone https://github.com/oakamil/cypress-astro.git
```

To build the cypress-server binary from the `cypress-astro` root:

```
./build.sh
```

The binary is built into `dist/cypress-server`.

## Running

Place `cypress-server` into the same location as `cedar-box-server`, typically `$HOME/cedar/bin`. `cedar-box-server` may be removed if it is already there.

Ensure that the capabilities for the binary are set. The build script above does this automatically, but if the binary is copied to a different system the capabilities have to be set again.

```
caps="cap_sys_time,cap_dac_override,cap_chown,cap_fowner,cap_net_bind_service+ep"
sudo setcap "$caps" $HOME/cedar/bin/cypress-server
```

Run the binary without activating any Python environment as there is no dependency on the Python-based `cedar-solve`.

```
cd $HOME/run
$HOME/cedar/bin/cypress-server
```

### IMU Integration

If an IMU is available `cypress-server` will utilize the IMU in standard 9-axis mode by default. To use a different mode include the -i or --imu-rotation-mode argument.

```
$HOME/cedar/bin/cypress-server -i 2
```

| Mode | Description | Measured Error |
| ---| --- | --- |
| 1 | Standard 9-axis rotation vector | 2.3 deg |
| 2 | Game rotation vector (6-axis, magnetometer is ignored) | 3.5 deg |
| 3 | AR/VR stabilized 9-axis rotation vector | 2.4 deg |
| 4 | AR/VR stabilized game rotation vector (6-axis) | 3.6 deg |

Standard mode is recommended unless magnetic interference is present in the environment. The measured error above is from ~15 short to medium slews after calibration.

## License

This project is licensed under the Apache License 2.0.

See `LICENSE.md` file for full details.

### Third-Party Licenses

While `cypress-server` itself is licensed under the Apache License 2.0, it integrates with and depends on several external projects (including but not limited to `cedar-server` and `olive-solve`). Each of these third-party projects is governed by its own respective licensing terms. Users are responsible for reviewing and complying with the individual licenses of any integrated components, tools, or dependencies.

## Disclaimer

All product names, trademarks and registered trademarks are property of their respective owners. All company, product and service names used in this website are for identification purposes only. Use of these names, trademarks and brands does not imply endorsement.

`cypress-astro` and `cypress-server` are not affiliated with, endorsed by, or sponsored by Clear Skies Astro.

Cedar™ is a trademark of Clear Skies Astro, registered in the U.S. and other countries.
