# cypress-solver

Integration of the fast `tetra-solve-rs` algorithms for the [Cedar™](https://github.com/smroid/cedar-server) telescope control system.

This crate is part of the broader `cypress-astro` workspace.

## Getting Started

### Prerequisites

* [Rust / Cargo](https://rustup.rs/) 
* Python 3 (Optional, required only for running the Python test suite)
* [cedar-server](https://github.com/smroid/cedar-server) cloned into the same parent directory as `cypress-astro`

### Building

To build the package, run the following from within the `cypress-solver` directory or the workspace root:

```bash
cargo build --release
```

### Testing

A set of real-world test data is provided in this repository for validating the algorithm.

#### Python Tetra3 Solver Validation

To run the Python interoperability tests, ensure that you have the following repositories cloned into the same parent directory as `cypress-astro`:

* [cedar-solve](https://github.com/smroid/cedar-solve)
* [tetra3_server](https://github.com/smroid/tetra3_server)

In the `cedar-solve` repository, ensure that the `setup.sh` script has been run to configure the Python environment. Then, execute the tests against the Python solver:

```bash
./run_python_tests.sh
```

#### Rust Solver Validation

Ensure that the `tetra3_server` repository is cloned to the same parent directory as `cypress-astro`.

Run the native Rust tests:

```bash
cargo test --release tetra3_solver -- --nocapture
```

## License

This project is licensed under the GNU General Public License v3.0.

See the root `LICENSE.md` file for full details.

### Third-Party Licenses

While cypress-solver itself is licensed under the GNU General Public License v3.0, it integrates with and depends on several external projects (including but not limited to `cedar-server`, `tetra3_server`, and `olive-solve`). Each of these third-party projects is governed by its own respective licensing terms. Users are responsible for reviewing and complying with the individual licenses of any integrated components, tools, or dependencies.

## Disclaimer

All product names, trademarks, and registered trademarks are the property of their respective owners. All company, product, and service names used in this repository are for identification purposes only. Use of these names, trademarks, and brands does not imply endorsement.

`cypress-astro` and `cypress-solver` are not affiliated with, endorsed by, or sponsored by Clear Skies Astro.

Cedar™ is a trademark of Clear Skies Astro, registered in the U.S. and other countries.
