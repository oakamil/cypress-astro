#!/bin/bash
source ../../cedar-solve/.cedar_venv/bin/activate
# Generate python protobuf bindings
python -m grpc_tools.protoc -I ./proto --python_out=../../tetra3_server/python --grpc_python_out=../../tetra3_server/python ./proto/tetra3.proto
cargo test --release --test tetra3_server_py_test -- --nocapture
