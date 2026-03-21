#!/bin/bash

cargo build --release --bin cypress-server
cp target/release/cypress-server dist/.
sudo setcap "cap_sys_time,cap_dac_override,cap_chown,cap_fowner,cap_net_bind_service+ep" dist/cypress-server

