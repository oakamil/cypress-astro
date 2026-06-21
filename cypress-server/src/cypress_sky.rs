// Required Notice: Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use async_trait::async_trait;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use canonical_error::CanonicalError;
use cedar_elements::cedar_common::CelestialCoord;
use cedar_elements::cedar_sky::{
    CatalogDescription, CatalogEntry, CatalogEntryKey, Constellation, ObjectType, Ordering,
    SelectedCatalogEntry,
};
use cedar_elements::cedar_sky_trait::{CedarSkyTrait, LocationInfo};

use starfield::planetlib::{Body, Ephemeris};
use starfield::time::Time;

use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};

// Constants for mobile screen rendering limits and decrowding
pub const DEFAULT_DECROWD_DISTANCE_ARCSEC: f64 = 60.0; // 1 arcminute for mobile readability
pub const DEFAULT_PAGINATION_LIMIT: usize = 100; // Prevent overwhelming mobile UI
pub const DEFAULT_FAINTEST_MAGNITUDE_MOBILE: i32 = 8; // Fainter objects might clutter small screens

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCatalogEntry {
    pub catalog_label: String,
    pub catalog_entry: String,
    pub ra: f64,
    pub dec: f64,
    pub constellation: Option<String>,
    pub object_type: String,
    pub broad_category: String,
    pub magnitude: Option<f64>,
    pub angular_size: Option<String>,
    pub common_name: Option<String>,
    pub copyright: Option<String>,
}

impl RTreeObject for DbCatalogEntry {
    type Envelope = AABB<[f64; 3]>;
    fn envelope(&self) -> Self::Envelope {
        let ra_rad = self.ra.to_radians();
        let dec_rad = self.dec.to_radians();
        let x = dec_rad.cos() * ra_rad.cos();
        let y = dec_rad.cos() * ra_rad.sin();
        let z = dec_rad.sin();
        AABB::from_point([x, y, z])
    }
}

pub struct CypressSky {
    rtree: RTree<DbCatalogEntry>,
    ephemeris: Option<Ephemeris>,
    solar_system_cache: Vec<CatalogEntry>,
}

impl CypressSky {
    pub fn new() -> Self {
        // Load the serialized R-Tree from disk
        let rtree = match File::open(
            "/Users/oakamil/projects/cypress-astro/cypress-server/data/catalog.bin",
        ) {
            Ok(f) => {
                let reader = BufReader::new(f);
                bincode::deserialize_from(reader).unwrap_or_else(|_| RTree::new())
            }
            Err(_) => {
                println!("Warning: catalog.bin not found. Sky will be empty.");
                RTree::new()
            }
        };

        Self {
            rtree,
            ephemeris: None, // Requires SPK file to be loaded in a real environment
            solar_system_cache: Vec::new(),
        }
    }

    /// Helper to compute angular distance between two celestial coordinates
    fn angular_distance_arcsec(c1: &CelestialCoord, c2: &CelestialCoord) -> f64 {
        let d_ra = (c1.ra - c2.ra) * c1.dec.to_radians().cos();
        let d_dec = c1.dec - c2.dec;
        ((d_ra * d_ra + d_dec * d_dec).sqrt()) * 3600.0
    }
}

#[async_trait]
impl CedarSkyTrait for CypressSky {
    async fn initialize_solar_system(&mut self, timestamp: SystemTime) {
        if let Some(ref mut eph) = self.ephemeris {
            self.solar_system_cache.clear();
            let bodies = [
                Body::Sun,
                Body::Mercury,
                Body::Venus,
                Body::Moon,
                Body::Mars,
                Body::Jupiter,
                Body::Saturn,
                Body::Uranus,
                Body::Neptune,
            ];

            let t = Time::from_unix(
                timestamp
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64(),
            );

            for &body in &bodies {
                if let Ok(_state) = eph.ecliptic_state(body, &t) {
                    self.solar_system_cache.push(CatalogEntry {
                        catalog_label: "SYS".to_string(),
                        catalog_entry: body.name().to_string(),
                        coord: Some(CelestialCoord {
                            ra: 0.0,  // Placeholder: requires conversion from ecliptic state vector
                            dec: 0.0, // Placeholder
                            epoch: Some(2000.0),
                        }),
                        constellation: None,
                        object_type: Some(ObjectType {
                            label: "planet".to_string(),
                            broad_category: "solar_system".to_string(),
                        }),
                        magnitude: Some(0.0), // Computed from distance/phase
                        angular_size: None,
                        common_name: Some(body.name().to_string()),
                        notes: None,
                    });
                }
            }
        }
    }

    fn get_catalog_descriptions(&self) -> Vec<CatalogDescription> {
        vec![
            CatalogDescription {
                label: "NGC".to_string(),
                name: "New General Catalogue".to_string(),
                description: "The Complete New General Catalogue".to_string(),
                source: "Sky Publishing Corporation".to_string(),
                copyright: Some("Sky Publishing Corporation".to_string()),
                license: None,
            },
            CatalogDescription {
                label: "IC".to_string(),
                name: "Index Catalogue".to_string(),
                description: "Index Catalogue".to_string(),
                source: "Sky Publishing Corporation".to_string(),
                copyright: Some("Sky Publishing Corporation".to_string()),
                license: None,
            },
            CatalogDescription {
                label: "M".to_string(),
                name: "Messier".to_string(),
                description: "Messier catalog".to_string(),
                source: "Public Domain".to_string(),
                copyright: None,
                license: None,
            },
            CatalogDescription {
                label: "C".to_string(),
                name: "Caldwell".to_string(),
                description: "Caldwell catalog".to_string(),
                source: "Public Domain".to_string(),
                copyright: None,
                license: None,
            },
            CatalogDescription {
                label: "H".to_string(),
                name: "Herschel 400".to_string(),
                description: "Herschel 400 catalogue".to_string(),
                source: "Public Domain".to_string(),
                copyright: None,
                license: None,
            },
            CatalogDescription {
                label: "Str".to_string(),
                name: "Named Bright Stars".to_string(),
                description: "Named Bright Stars".to_string(),
                source: "Public Domain".to_string(),
                copyright: None,
                license: None,
            },
            CatalogDescription {
                label: "WDS".to_string(),
                name: "Washington Double Stars".to_string(),
                description: "WDS double star catalog".to_string(),
                source: "USNO".to_string(),
                copyright: None,
                license: None,
            },
            CatalogDescription {
                label: "SYS".to_string(),
                name: "Solar System".to_string(),
                description: "Real-time ephemeris of the solar system".to_string(),
                source: "JPL DE440".to_string(),
                copyright: None,
                license: None,
            },
        ]
    }

    fn get_object_types(&self) -> Vec<ObjectType> {
        vec![
            ObjectType {
                label: "star".to_string(),
                broad_category: "star".to_string(),
            },
            ObjectType {
                label: "planet".to_string(),
                broad_category: "solar_system".to_string(),
            },
            ObjectType {
                label: "galaxy".to_string(),
                broad_category: "galaxy".to_string(),
            },
            ObjectType {
                label: "cluster".to_string(),
                broad_category: "cluster".to_string(),
            },
            ObjectType {
                label: "nebula".to_string(),
                broad_category: "nebula".to_string(),
            },
        ]
    }

    fn get_constellations(&self) -> Vec<Constellation> {
        starfield::constellationlib::ConstellationFinder::all_names()
            .iter()
            .map(|(abbr, name)| Constellation {
                label: abbr.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    async fn query_catalog_entries(
        &self,
        max_distance: Option<f64>,
        _min_elevation: Option<f64>,
        faintest_magnitude: Option<i32>,
        match_catalog_label: bool,
        catalog_label: &[String],
        match_object_type_label: bool,
        object_type_label: &[String],
        text_search: Option<String>,
        _ordering: Option<Ordering>,
        decrowd_distance: Option<f64>,
        limit_result: Option<usize>,
        sky_location: Option<CelestialCoord>,
        _location_info: Option<LocationInfo>,
    ) -> Result<(Vec<SelectedCatalogEntry>, usize), CanonicalError> {
        let faint_limit = faintest_magnitude.unwrap_or(DEFAULT_FAINTEST_MAGNITUDE_MOBILE) as f64;

        // 1. Fetch from R-Tree spatially or globally
        let mut raw_entries: Vec<CatalogEntry> = self.solar_system_cache.clone();

        let found_objects: Vec<&DbCatalogEntry> =
            if let (Some(max_dist_deg), Some(target)) = (max_distance, &sky_location) {
                // Convert to 3D Cartesian space for mathematically perfect spherical queries
                let target_ra_rad = target.ra.to_radians();
                let target_dec_rad = target.dec.to_radians();
                let tx = target_dec_rad.cos() * target_ra_rad.cos();
                let ty = target_dec_rad.cos() * target_ra_rad.sin();
                let tz = target_dec_rad.sin();

                // The max Euclidean distance between two unit vectors for a given angular distance theta is:
                // d = 2 * sin(theta / 2)
                let max_euclidean_dist = 2.0 * (max_dist_deg.to_radians() / 2.0).sin();
                let max_euclidean_sq = max_euclidean_dist * max_euclidean_dist;

                self.rtree
                    .locate_within_distance([tx, ty, tz], max_euclidean_sq)
                    .collect()
            } else {
                // Global fallback for text-only searches
                self.rtree.iter().collect()
            };

        for db_entry in found_objects {
            if let Some(mag) = db_entry.magnitude {
                if mag > faint_limit {
                    continue;
                }
            }

            if match_catalog_label && !catalog_label.contains(&db_entry.catalog_label) {
                continue;
            }

            if match_object_type_label && !object_type_label.contains(&db_entry.broad_category) {
                continue;
            }

            if let Some(ref text) = text_search {
                let text_lower = text.to_lowercase();
                let mut matched = false;
                if db_entry.catalog_label.to_lowercase().contains(&text_lower) {
                    matched = true;
                }
                if db_entry.catalog_entry.to_lowercase().contains(&text_lower) {
                    matched = true;
                }
                if let Some(cn) = &db_entry.common_name {
                    if cn.to_lowercase().contains(&text_lower) {
                        matched = true;
                    }
                }
                if !matched {
                    continue;
                }
            }

            raw_entries.push(CatalogEntry {
                catalog_label: db_entry.catalog_label.clone(),
                catalog_entry: db_entry.catalog_entry.clone(),
                coord: Some(CelestialCoord {
                    ra: db_entry.ra,
                    dec: db_entry.dec,
                    epoch: Some(2000.0),
                }),
                constellation: db_entry.constellation.as_ref().map(|c| Constellation {
                    label: c.clone(),
                    name: starfield::constellationlib::ConstellationFinder::full_name(c)
                        .unwrap_or("")
                        .to_string(),
                }),
                object_type: Some(ObjectType {
                    label: db_entry.object_type.clone(),
                    broad_category: db_entry.broad_category.clone(),
                }),
                magnitude: db_entry.magnitude,
                angular_size: db_entry.angular_size.clone(),
                common_name: db_entry.common_name.clone(),
                notes: db_entry.copyright.clone(),
            });
        }

        // 2. DECROWDING LOGIC
        let decrowd_dist = decrowd_distance.unwrap_or(DEFAULT_DECROWD_DISTANCE_ARCSEC);
        let mut selected_entries: Vec<SelectedCatalogEntry> = Vec::new();

        raw_entries.sort_by(|a, b| {
            let mag_a = a.magnitude.unwrap_or(100.0);
            let mag_b = b.magnitude.unwrap_or(100.0);
            mag_a
                .partial_cmp(&mag_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in raw_entries {
            let coord = entry.coord.as_ref().unwrap();

            let mut crowded = false;
            for selected in &mut selected_entries {
                let sel_coord = selected.entry.as_ref().unwrap().coord.as_ref().unwrap();
                if Self::angular_distance_arcsec(coord, sel_coord) < decrowd_dist {
                    selected.decrowded_entries.push(entry.clone());
                    crowded = true;
                    break;
                }
            }

            if !crowded {
                selected_entries.push(SelectedCatalogEntry {
                    entry: Some(entry),
                    deduped_entries: vec![],
                    decrowded_entries: vec![],
                    altitude: None,
                    azimuth: None,
                });
            }
        }

        // 3. PAGINATION
        let limit = limit_result.unwrap_or(DEFAULT_PAGINATION_LIMIT);
        let truncated_count = if selected_entries.len() > limit {
            selected_entries.len() - limit
        } else {
            0
        };

        selected_entries.truncate(limit);

        Ok((selected_entries, truncated_count))
    }

    async fn get_catalog_entry(
        &mut self,
        entry_key: CatalogEntryKey,
        _timestamp: SystemTime,
    ) -> Result<CatalogEntry, CanonicalError> {
        if entry_key.cat_label == "SYS" {
            if let Some(entry) = self
                .solar_system_cache
                .iter()
                .find(|e| e.catalog_entry == entry_key.entry)
            {
                return Ok(entry.clone());
            }
        }

        // Direct search across the entire R-Tree for the specific ID
        // Note: For O(1) lookups by ID, a separate HashMap alongside the RTree is recommended,
        // but for now an O(N) iteration over the RTree leaves works.
        if let Some(db_entry) = self
            .rtree
            .iter()
            .find(|e| e.catalog_label == entry_key.cat_label && e.catalog_entry == entry_key.entry)
        {
            return Ok(CatalogEntry {
                catalog_label: db_entry.catalog_label.clone(),
                catalog_entry: db_entry.catalog_entry.clone(),
                coord: Some(CelestialCoord {
                    ra: db_entry.ra,
                    dec: db_entry.dec,
                    epoch: Some(2000.0),
                }),
                constellation: db_entry.constellation.as_ref().map(|c| Constellation {
                    label: c.clone(),
                    name: starfield::constellationlib::ConstellationFinder::full_name(c)
                        .unwrap_or("")
                        .to_string(),
                }),
                object_type: Some(ObjectType {
                    label: db_entry.object_type.clone(),
                    broad_category: db_entry.broad_category.clone(),
                }),
                magnitude: db_entry.magnitude,
                angular_size: db_entry.angular_size.clone(),
                common_name: db_entry.common_name.clone(),
                notes: db_entry.copyright.clone(),
            });
        }

        Err(CanonicalError::not_found(format!(
            "Entry not found: {} {}",
            entry_key.cat_label, entry_key.entry
        )))
    }
}
