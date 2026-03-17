// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use prost::Message;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use tetra3_server::tetra3_server::{SolveRequest, SolveResult, SolveStatus as ProtoSolveStatus};

// --- Serialization Data Transfer Objects (DTOs) ---

#[derive(Serialize, Deserialize, Debug)]
pub struct SolveOptionsDto {
    pub fov_estimate: Option<f64>,
    pub fov_max_error: Option<f64>,
    pub match_radius: f64,
    pub match_threshold: f64,
    pub solve_timeout_ms: Option<f64>,
    pub distortion: Option<f64>,
    pub match_max_error: f64,
    pub return_matches: bool,
    pub return_catalog: bool,
    pub return_rotation_matrix: bool,
    pub target_pixel: Option<Vec<[f64; 2]>>,
    pub target_sky_coord: Option<Vec<[f64; 2]>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolveInputDto {
    pub centroids: Vec<[f64; 2]>,
    pub image_height: f64,
    pub image_width: f64,
    pub options: SolveOptionsDto,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolutionDto {
    pub ra: Option<f64>,
    pub dec: Option<f64>,
    pub roll: Option<f64>,
    pub fov: Option<f64>,
    pub distortion: Option<f64>,
    pub rmse: Option<f64>,
    pub p90e: Option<f64>,
    pub maxe: Option<f64>,
    pub matches: Option<usize>,
    pub prob: Option<f64>,
    pub epoch_equinox: Option<f64>,
    pub epoch_proper_motion: Option<f64>,
    pub status: String,
    pub t_solve_ms: f64,
    pub rotation_matrix: Option<Vec<f64>>,
    pub target_ra: Option<Vec<f64>>,
    pub target_dec: Option<Vec<f64>>,
    pub target_y: Option<Vec<Option<f64>>>,
    pub target_x: Option<Vec<Option<f64>>>,
    pub matched_centroids: Option<Vec<[f64; 2]>>,
    pub matched_stars: Option<Vec<[f64; 3]>>,
    pub matched_cat_id: Option<Vec<Vec<u32>>>,
    pub catalog_stars: Option<Vec<(f64, f64, f64, f64, f64)>>,
}

// --- Tests ---

#[test]
#[ignore]
// Run intentionally via: cargo test generate_solver_test_fixtures --release -- --ignored
fn generate_solver_test_fixtures() {
    let proto_zip_path = Path::new("data/testdata.zip");

    if !proto_zip_path.exists() {
        println!("Required data files not found. Skipping fixture generation.");
        return;
    }

    let proto_zip_file = File::open(proto_zip_path).expect("Failed to open testdata.zip");
    let mut read_archive = ZipArchive::new(proto_zip_file).expect("Failed to open zip archive");

    // Prepare the output zip
    let fixtures_dir = Path::new("output");
    fs::create_dir_all(&fixtures_dir).unwrap();
    let out_zip_path = fixtures_dir.join("solver_fixtures.zip");
    let out_zip_file = File::create(&out_zip_path).unwrap();
    let mut zip_writer = ZipWriter::new(out_zip_file);

    let iterations = 738;

    for x in 0..iterations {
        // Read Request Proto
        let req_filename = format!("solve_request_{}.pb", x);
        let mut req_buffer = Vec::new();
        {
            let mut req_file = read_archive
                .by_name(&req_filename)
                .unwrap_or_else(|_| panic!("Entry {} not found in zip", req_filename));
            req_file.read_to_end(&mut req_buffer).unwrap();
        }
        let request = SolveRequest::decode(Cursor::new(req_buffer))
            .expect("Failed to decode SolveRequest proto");

        // Read Result Proto
        let res_filename = format!("solve_result_{}.pb", x);
        let mut res_buffer = Vec::new();
        {
            let mut res_file = read_archive
                .by_name(&res_filename)
                .unwrap_or_else(|_| panic!("Entry {} not found in zip", res_filename));
            res_file.read_to_end(&mut res_buffer).unwrap();
        }
        let result_proto = SolveResult::decode(Cursor::new(res_buffer))
            .expect("Failed to decode SolveResult proto");

        // Map Protobuf Request data to our Rust Input DTO
        let width = request.image_width as f64;
        let height = request.image_height as f64;

        let mut flat_cents = Vec::with_capacity(request.star_centroids.len());
        for c in &request.star_centroids {
            flat_cents.push([c.y as f64, c.x as f64]);
        }

        let target_pixel = if request.target_pixels.is_empty() {
            None
        } else {
            let mut flat = Vec::with_capacity(request.target_pixels.len());
            for tp in &request.target_pixels {
                flat.push([tp.y as f64, tp.x as f64]);
            }
            Some(flat)
        };

        let target_sky_coord = if request.target_sky_coords.is_empty() {
            None
        } else {
            let mut flat = Vec::with_capacity(request.target_sky_coords.len());
            for tsc in &request.target_sky_coords {
                flat.push([tsc.ra, tsc.dec]);
            }
            Some(flat)
        };

        let input_dto = SolveInputDto {
            centroids: flat_cents,
            image_height: height,
            image_width: width,
            options: SolveOptionsDto {
                fov_estimate: request.fov_estimate,
                fov_max_error: request.fov_max_error.or(Some(0.1)),
                match_radius: request.match_radius.unwrap_or(0.01),
                match_threshold: request.match_threshold.unwrap_or(1e-4),
                solve_timeout_ms: Some(5000.0),
                distortion: request.distortion,
                match_max_error: request.match_max_error.unwrap_or(0.005),
                return_matches: false,
                return_catalog: false,
                return_rotation_matrix: false,
                target_pixel,
                target_sky_coord,
            },
        };

        // Map Protobuf Result data to our Rust Output DTO
        let status_str = if result_proto.status == Some(ProtoSolveStatus::MatchFound as i32) {
            "MatchFound"
        } else if result_proto.status == Some(ProtoSolveStatus::Timeout as i32) {
            "Timeout"
        } else if result_proto.status == Some(ProtoSolveStatus::Cancelled as i32) {
            "Cancelled"
        } else if result_proto.status == Some(ProtoSolveStatus::TooFew as i32) {
            "TooFew"
        } else {
            "NoMatch"
        };

        let (ra, dec) = if let Some(coords) = result_proto.image_center_coords {
            (Some(coords.ra), Some(coords.dec))
        } else {
            (None, None)
        };

        let solution_dto = SolutionDto {
            ra,
            dec,
            roll: result_proto.roll,
            fov: result_proto.fov,
            distortion: None, // Derived fields not populated explicitly in the ground truth
            rmse: None,
            p90e: None,
            maxe: None,
            matches: None,
            prob: None,
            epoch_equinox: None,
            epoch_proper_motion: None,
            status: status_str.to_string(),
            t_solve_ms: 0.0, // Solver bypassed, set execution time to zero
            rotation_matrix: None,
            target_ra: None,
            target_dec: None,
            target_y: None,
            target_x: None,
            matched_centroids: None,
            matched_stars: None,
            matched_cat_id: None,
            catalog_stars: None,
        };

        let zip_options = FileOptions::default().compression_method(CompressionMethod::Deflated);

        // Write input JSON
        let input_filename = format!("input_{}.json", x + 1);
        zip_writer.start_file(&input_filename, zip_options).unwrap();
        let input_json = serde_json::to_vec_pretty(&input_dto).unwrap();
        zip_writer.write_all(&input_json).unwrap();

        // Write output JSON
        let output_filename = format!("output_{}.json", x + 1);
        zip_writer
            .start_file(&output_filename, zip_options)
            .unwrap();
        let output_json = serde_json::to_vec_pretty(&solution_dto).unwrap();
        zip_writer.write_all(&output_json).unwrap();
    }

    zip_writer.finish().unwrap();
    println!("Successfully generated solver_fixtures.zip");
}
