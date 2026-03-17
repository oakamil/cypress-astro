// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use cedar_elements::cedar::{
    FrameRequest, MountType, OperatingMode, cedar_client::CedarClient as GrpcClient,
};
use log::{debug, warn};
use tonic::transport::Channel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStatus {
    Success,
    Disconnected,
    RpcFailed,
    NoState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMode {
    Unknown,
    Setup,
    Calibrating,
    Operating,
}

#[derive(Debug, Clone)]
pub struct ServerState {
    pub server_mode: ServerMode,
    pub is_alt_az: bool,
    pub has_slew_request: bool,
    pub rotation_target_distance: f64,
    pub tilt_target_distance: f64,
    pub target_angle: f64,
    pub has_solution: bool,
}

#[derive(Debug, Clone)]
pub struct CedarResponse {
    pub status: ResponseStatus,
    pub server_state: Option<ServerState>,
}

pub struct CedarClient {
    client: Option<GrpcClient<Channel>>,
}

impl CedarClient {
    pub fn new() -> Self {
        CedarClient { client: None }
    }

    // This function tries to (re-)connect to the Cedar gRPC service if
    // disconnected.
    pub async fn get_state(&mut self) -> CedarResponse {
        if self.client.is_none() {
            self.try_to_connect().await;
        }
        if self.client.is_none() {
            return CedarResponse {
                status: ResponseStatus::Disconnected,
                server_state: None,
            };
        }
        let client = self.client.as_mut().unwrap();
        let resp = Self::get_state_impl(client).await;
        debug!("Generated response: {:?}", resp);
        resp
    }

    // Connects to the main Cedar gRPC server
    async fn try_to_connect(&mut self) {
        let client = GrpcClient::connect("http://localhost:80").await;
        match client {
            Ok(c) => {
                self.client = Some(c);
            }
            Err(e) => {
                warn!("Unable to connect go Cedar server: {}", e);
            }
        }
    }

    async fn get_state_impl(client: &mut GrpcClient<Channel>) -> CedarResponse {
        let request = FrameRequest {
            non_blocking: Some(true),
            ..Default::default()
        };

        match client.get_frame(request).await {
            Ok(response) => {
                let frame = response.into_inner();

                if !frame.has_result.unwrap_or(false) {
                    return CedarResponse {
                        status: ResponseStatus::NoState,
                        server_state: None,
                    };
                }

                let mut server_mode = ServerMode::Unknown;
                if frame.calibrating {
                    server_mode = ServerMode::Calibrating;
                } else if let Some(op_settings) = &frame.operation_settings {
                    if let Some(mode) = op_settings.operating_mode {
                        if mode == OperatingMode::Setup as i32 {
                            server_mode = ServerMode::Setup;
                        } else if mode == OperatingMode::Operate as i32 {
                            server_mode = ServerMode::Operating;
                        }
                    }
                }

                let mut is_alt_az = false;
                if let Some(prefs) = &frame.preferences {
                    if let Some(mount) = prefs.mount_type {
                        if mount == MountType::AltAz as i32 {
                            is_alt_az = true;
                        }
                    }
                }

                let mut has_slew_request = false;
                let mut rotation_dist = 0.0;
                let mut tilt_dist = 0.0;
                let mut target_angle = 0.0;

                if let Some(slew) = &frame.slew_request {
                    has_slew_request = true;
                    rotation_dist = slew.offset_rotation_axis.unwrap_or(0.0);
                    tilt_dist = slew.offset_tilt_axis.unwrap_or(0.0);
                    target_angle = slew.target_angle.unwrap_or(0.0);
                }

                let state = ServerState {
                    server_mode,
                    is_alt_az,
                    has_slew_request,
                    rotation_target_distance: rotation_dist,
                    tilt_target_distance: tilt_dist,
                    target_angle,
                    has_solution: frame.plate_solution.is_some(),
                };

                CedarResponse {
                    status: ResponseStatus::Success,
                    server_state: Some(state),
                }
            }
            Err(e) => {
                warn!("Failed to get frame: {}", e);
                CedarResponse {
                    status: ResponseStatus::RpcFailed,
                    server_state: None,
                }
            }
        }
    }
}
