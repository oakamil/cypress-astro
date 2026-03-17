// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional");

    tonic_build::configure().compile_with_config(config, &["proto/tetra3.proto"], &["proto"])?;
    Ok(())
}
