// Copyright (c) 2026 Omair Kamil
// See LICENSE file in root directory for license terms.

use async_trait::async_trait;
use canonical_error::CanonicalError;
use cedar_elements::cedar_common::CelestialCoord;
use cedar_elements::cedar_sky::{
    CatalogDescription, CatalogEntry, CatalogEntryKey, Constellation, ObjectType, Ordering,
    SelectedCatalogEntry,
};
use cedar_elements::cedar_sky_trait::{CedarSkyTrait, LocationInfo};
use rusqlite::{Connection, OpenFlags, params};
use std::sync::Arc;
use std::time::SystemTime;
use tiny_http::{Header, Response, Server};
use tokio::sync::Mutex;

pub struct CypressCatalog {
    db_path: String,
    db: Arc<Mutex<Option<Connection>>>,
    catalog_descriptions: std::sync::Mutex<Vec<CatalogDescription>>,
    object_types: std::sync::Mutex<Vec<ObjectType>>,
    constellations: std::sync::Mutex<Vec<Constellation>>,
    bsp_path: Option<String>,
}

impl CypressCatalog {
    pub fn new(db_path: &str, bsp_path: Option<&str>) -> Self {
        Self {
            db_path: db_path.to_string(),
            bsp_path: bsp_path.map(|s| s.to_string()),
            db: Arc::new(Mutex::new(None)),
            catalog_descriptions: std::sync::Mutex::new(Vec::new()),
            object_types: std::sync::Mutex::new(Vec::new()),
            constellations: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub async fn open(&self) -> Result<(), CanonicalError> {
        let mut db_lock = self.db.lock().await;
        if db_lock.is_none() {
            let conn =
                Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                    .map_err(|e| {
                        canonical_error::internal_error(&format!("Failed to open db: {}", e))
                    })?;

            conn.create_scalar_function(
                "SIN",
                1,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8
                    | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
                move |ctx| {
                    let val: f64 = ctx.get::<f64>(0)?;
                    Ok(val.sin())
                },
            )
            .map_err(|e| canonical_error::internal_error(&format!("Failed to add SIN: {}", e)))?;

            conn.create_scalar_function(
                "COS",
                1,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8
                    | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
                move |ctx| {
                    let val: f64 = ctx.get::<f64>(0)?;
                    Ok(val.cos())
                },
            )
            .map_err(|e| canonical_error::internal_error(&format!("Failed to add COS: {}", e)))?;

            // Populate metadata
            if let Ok(mut stmt) =
                conn.prepare("SELECT catalog_label, description FROM catalog_descriptions")
            {
                let mut descs = Vec::new();
                if let Ok(mut rows) = stmt.query([]) {
                    while let Ok(Some(row)) = rows.next() {
                        let label: String = row.get(0).unwrap_or_default();
                        let desc: String = row.get(1).unwrap_or_default();
                        descs.push(CatalogDescription {
                            label: label.clone(),
                            name: desc.clone(),
                            description: desc,
                            source: String::new(),
                            copyright: None,
                            license: None,
                        });
                    }
                }
                *self.catalog_descriptions.lock().unwrap() = descs;
            }

            if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT object_type, broad_category FROM catalog WHERE object_type IS NOT NULL") {
                let mut types = Vec::new();
                if let Ok(mut rows) = stmt.query([]) {
                    while let Ok(Some(row)) = rows.next() {
                        let label: String = row.get(0).unwrap_or_default();
                        let broad: String = row.get(1).unwrap_or_default();
                        types.push(ObjectType {
                            label,
                            broad_category: broad,
                        });
                    }
                }
                *self.object_types.lock().unwrap() = types;
            }

            if let Ok(mut stmt) = conn.prepare(
                "SELECT DISTINCT constellation FROM catalog WHERE constellation IS NOT NULL",
            ) {
                let mut consts = Vec::new();
                if let Ok(mut rows) = stmt.query([]) {
                    while let Ok(Some(row)) = rows.next() {
                        let label: String = row.get(0).unwrap_or_default();
                        consts.push(Constellation {
                            label: label.clone(),
                            name: label,
                        });
                    }
                }
                *self.constellations.lock().unwrap() = consts;
            }

            *db_lock = Some(conn);
        }
        Ok(())
    }

    pub fn start_comet_upload_server(&self, port: u16) {
        let db_path = self.db_path.clone();
        std::thread::spawn(move || {
            let server = Server::http(format!("0.0.0.0:{}", port)).unwrap();
            for mut request in server.incoming_requests() {
                eprintln!(
                    "Incoming tiny_http request: {} {}",
                    request.method().as_str(),
                    request.url()
                );
                if request.url() == "/update-comets" && request.method().as_str() == "OPTIONS" {
                    let mut response = Response::empty(200);
                    response.add_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );
                    response.add_header(
                        Header::from_bytes(
                            &b"Access-Control-Allow-Methods"[..],
                            &b"POST, OPTIONS"[..],
                        )
                        .unwrap(),
                    );
                    response.add_header(
                        Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"*"[..])
                            .unwrap(),
                    );
                    let _ = request.respond(response);
                } else if request.url() == "/update-comets" && request.method().as_str() == "POST" {
                    eprintln!("Received POST /update-comets");
                    let mut content = String::new();
                    match request.as_reader().read_to_string(&mut content) {
                        Ok(len) => eprintln!("Read {} bytes from request", len),
                        Err(e) => eprintln!("Error reading request: {:?}", e),
                    }

                    if let Ok(mut conn) = Connection::open(&db_path) {
                        eprintln!("Opened connection to {}", db_path);
                        let tx = conn.transaction();
                        if let Ok(tx) = tx {
                            eprintln!("Started transaction");
                            let _ = tx.execute("DELETE FROM comets", []);
                            eprintln!("Deleted old comets");

                            let mut inserted = 0;
                            for (i, line) in content.lines().enumerate() {
                                if line.trim().is_empty() {
                                    continue;
                                }
                                if line.len() < 102 {
                                    eprintln!(
                                        "Skipping line {} because length {} < 102",
                                        i,
                                        line.len()
                                    );
                                    continue;
                                }
                                let h_str = line[91..96].trim();
                                let k_str = line[96..101].trim();
                                if h_str.is_empty() || k_str.is_empty() {
                                    eprintln!("Skipping line {} due to empty H/G params", i);
                                    continue;
                                }

                                let catalog_id = line[0..12].trim().replace("  ", " ");
                                let year: i32 = line[14..18].trim().parse().unwrap_or(0);
                                let month: i32 = line[19..21].trim().parse().unwrap_or(0);
                                let day_frac: f64 = line[22..29].trim().parse().unwrap_or(0.0);

                                let mut y = year;
                                let mut m = month;
                                if m <= 2 {
                                    y -= 1;
                                    m += 12;
                                }
                                let a = y / 100;
                                let b = 2 - a + (a / 4);
                                let t_peri = (365.25 * (y as f64 + 4716.0)) as i32 as f64
                                    + (30.6001 * (m as f64 + 1.0)) as i32 as f64
                                    + day_frac
                                    + b as f64
                                    - 1524.5;

                                let q: f64 = line[30..39].trim().parse().unwrap_or(0.0);
                                let e: f64 = line[40..49].trim().parse().unwrap_or(0.0);
                                let w: f64 = line[50..59].trim().parse().unwrap_or(0.0);
                                let node: f64 = line[60..69].trim().parse().unwrap_or(0.0);
                                let incl: f64 = line[70..79].trim().parse().unwrap_or(0.0);
                                let h: f64 = h_str.parse().unwrap_or(0.0);
                                let k: f64 = k_str.parse().unwrap_or(0.0);

                                let common_name = if line.len() > 102 {
                                    let mut name_part = &line[102..];
                                    if let Some(idx) = name_part.find("  ") {
                                        name_part = &name_part[..idx];
                                    }
                                    name_part.trim()
                                } else {
                                    ""
                                };

                                let _ = tx.execute(
                                    "INSERT INTO comets (catalog_id, common_name, h, k, t_peri, q, e, w, node, incl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                    params![catalog_id, common_name, h, k, t_peri, q, e, w, node, incl]
                                );
                                inserted += 1;
                            }
                            match tx.commit() {
                                Ok(_) => eprintln!(
                                    "Committed transaction. Inserted {} comets.",
                                    inserted
                                ),
                                Err(e) => eprintln!("Failed to commit transaction: {:?}", e),
                            }
                        } else {
                            eprintln!("Failed to start transaction");
                        }
                    } else {
                        eprintln!("Failed to open DB connection to {}", db_path);
                    }

                    eprintln!("Sending 200 OK response");
                    let mut response = Response::from_string("OK");
                    response.add_header(
                        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
                    );
                    if let Err(e) = request.respond(response) {
                        eprintln!("Failed to send response: {:?}", e);
                    }
                } else {
                    let _ =
                        request.respond(Response::from_string("Not Found").with_status_code(404));
                }
            }
        });
    }
}

#[async_trait]
impl CedarSkyTrait for CypressCatalog {
    async fn initialize_solar_system(&mut self, timestamp: SystemTime) {
        let bsp_path = if let Some(p) = &self.bsp_path {
            p.clone()
        } else {
            return;
        };

        let db_lock_clone = self.db.clone();
        tokio::task::spawn_blocking(move || {
            use chrono::TimeZone;
            use starfield::jplephem_ext::SpiceKernelExt;

            let ts = starfield::time::Timescale::default();
            let time = match timestamp.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => {
                    let dt = chrono::Utc.timestamp_millis_opt(d.as_millis() as i64).single();
                    if let Some(dt) = dt {
                        use chrono::{Datelike, Timelike};
                        ts.utc((
                            dt.year(),
                            dt.month() as _,
                            dt.day() as _,
                            dt.hour() as _,
                            dt.minute() as _,
                            dt.second() as f64 + dt.nanosecond() as f64 / 1_000_000_000.0,
                        ))
                    } else {
                        return;
                    }
                },
                Err(_) => return,
            };

            let mut kernel = if let Ok(k) = starfield::jplephem::kernel::SpiceKernel::open(&bsp_path) {
                k
            } else {
                return;
            };

            let observer_loc = starfield::toposlib::WGS84.latlon(0.0, 0.0, 0.0);
            let earth = if let Ok(e) = observer_loc.at(&time, &mut kernel) {
                e
            } else {
                return;
            };

            let targets = [
                ("MERCURY BARYCENTER", "Mercury"),
                ("VENUS BARYCENTER", "Venus"),
                ("MOON", "Moon"),
                ("MARS BARYCENTER", "Mars"),
                ("JUPITER BARYCENTER", "Jupiter"),
                ("SATURN BARYCENTER", "Saturn"),
                ("URANUS BARYCENTER", "Uranus"),
                ("NEPTUNE BARYCENTER", "Neptune"),
                ("PLUTO BARYCENTER", "Pluto"),
            ];

            let mut planets_data = Vec::new();

            for (name, common_name) in targets {
                if let Ok(astrometric) = earth.observe(name, &mut kernel, &time) {
                    let (ra_hours, dec_deg, _) = astrometric.radec(None);
                    let mag = starfield::magnitudelib::planetary_magnitude(&astrometric, &time).ok();
                    planets_data.push((
                        "Planet".to_string(),
                        common_name.to_string(),
                        ra_hours * 15.0,
                        dec_deg,
                        "Planet".to_string(),
                        "planet".to_string(),
                        mag,
                        Some(common_name.to_string()),
                        Some(format!("The planet {}", common_name)),
                    ));
                }
            }

            let mut asteroids = vec![];
            {
                let mut conn_guard = db_lock_clone.blocking_lock();
                if let Some(conn) = conn_guard.as_mut() {
                    if let Ok(mut stmt) = conn.prepare("SELECT catalog_id, common_name, h, g, epoch, m, peri, node, incl, e, a FROM asteroids") {
                        let asteroid_iter = stmt.query_map([], |row| {
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, f64>(2)?,
                                row.get::<_, f64>(3)?,
                                row.get::<_, f64>(4)?,
                                row.get::<_, f64>(5)?,
                                row.get::<_, f64>(6)?,
                                row.get::<_, f64>(7)?,
                                row.get::<_, f64>(8)?,
                                row.get::<_, f64>(9)?,
                                row.get::<_, f64>(10)?,
                            ))
                        });
                        if let Ok(iter) = asteroid_iter {
                            for ast in iter {
                                if let Ok(a) = ast {
                                    asteroids.push(a);
                                }
                            }
                        }
                    }
                }
            }

            let sun = kernel.at("sun", &time).unwrap_or(earth.clone());
            for (catalog_id, common_name, h, g, epoch, m, peri, node, incl, e, a) in asteroids {
                let orbit = starfield::keplerlib::mpcorb_orbit(
                    a, e, incl, node, peri, m,
                    &ts.tt_jd(epoch, None),
                    starfield::constants::GM_SUN,
                    common_name.as_deref(),
                );

                let mut target_helio = orbit.at(&time);
                let mut target_bary_pos = target_helio.position + sun.position;
                let mut target_bary_vel = target_helio.velocity + sun.velocity;
                let mut distance_au = (target_bary_pos - earth.position).norm();
                let mut light_time0 = 0.0;

                for _ in 0..5 {
                    let light_time = distance_au / starfield::constants::C_AUDAY;
                    let delta_t = light_time - light_time0;
                    if delta_t.abs() < 1e-7 {
                        break;
                    }
                    let retarded_time = ts.tdb_jd(time.tdb() - light_time);
                    target_helio = orbit.at(&retarded_time);
                    let sun_ret = kernel
                        .at("sun", &retarded_time)
                        .unwrap_or_else(|_| sun.clone());
                    target_bary_pos = target_helio.position + sun_ret.position;
                    target_bary_vel = target_helio.velocity + sun_ret.velocity;
                    distance_au = (target_bary_pos - earth.position).norm();
                    light_time0 = light_time;
                }

                let vector = target_bary_pos - earth.position;
                let velocity = target_bary_vel - earth.velocity;

                let astrometric = starfield::positions::Position::astrometric(vector, velocity, &earth, -1, light_time0);
                let (ra_hours, dec_deg, _) = astrometric.radec(None);

                let r_vec = -target_helio.position;
                let delta_vec = earth.position - target_bary_pos;
                let r = r_vec.norm();
                let delta = delta_vec.norm();
                let ph_ang = r_vec.angle(&delta_vec);
                let ta = (ph_ang / 2.0).tan();
                let phi1 = (-3.33 * ta.powf(0.63)).exp();
                let phi2 = (-1.87 * ta.powf(1.22)).exp();
                let mag = h + 5.0 * (r * delta).log10()
                    - 2.5 * ((1.0 - g) * phi1 + g * phi2).log10();

                planets_data.push((
                    "Asteroid".to_string(),
                    catalog_id.unwrap_or_default(),
                    ra_hours * 15.0,
                    dec_deg,
                    "Asteroid".to_string(),
                    "asteroid".to_string(),
                    Some(mag),
                    common_name.clone(),
                    common_name.map(|n| format!("The asteroid {}", n)),
                ));
            }

            let mut comets = vec![];
            {
                let mut conn_guard = db_lock_clone.blocking_lock();
                if let Some(conn) = conn_guard.as_mut() {
                    if let Ok(mut stmt) = conn.prepare("SELECT catalog_id, common_name, h, k, t_peri, q, e, w, node, incl FROM comets") {
                        let comet_iter = stmt.query_map([], |row| {
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, f64>(2)?,
                                row.get::<_, f64>(3)?,
                                row.get::<_, f64>(4)?,
                                row.get::<_, f64>(5)?,
                                row.get::<_, f64>(6)?,
                                row.get::<_, f64>(7)?,
                                row.get::<_, f64>(8)?,
                                row.get::<_, f64>(9)?,
                            ))
                        });
                        if let Ok(iter) = comet_iter {
                            for com in iter {
                                if let Ok(c) = com {
                                    comets.push(c);
                                }
                            }
                        }
                    }
                }
            }

            for (catalog_id, common_name, h, k, t_peri, q, e, w, node, incl) in comets {
                let epoch = ts.tt_jd(t_peri, None);
                let orbit = starfield::keplerlib::comet_orbit(
                    q, e, incl, node, w,
                    &epoch,
                    starfield::constants::GM_SUN,
                    common_name.as_deref(),
                );

                let mut target_helio = orbit.at(&time);
                let mut target_bary_pos = target_helio.position + sun.position;
                let mut target_bary_vel = target_helio.velocity + sun.velocity;
                let mut distance_au = (target_bary_pos - earth.position).norm();
                let mut light_time0 = 0.0;

                for _ in 0..5 {
                    let light_time = distance_au / starfield::constants::C_AUDAY;
                    let delta_t = light_time - light_time0;
                    if delta_t.abs() < 1e-7 {
                        break;
                    }
                    let retarded_time = ts.tdb_jd(time.tdb() - light_time);
                    target_helio = orbit.at(&retarded_time);
                    let sun_ret = kernel
                        .at("sun", &retarded_time)
                        .unwrap_or_else(|_| sun.clone());
                    target_bary_pos = target_helio.position + sun_ret.position;
                    target_bary_vel = target_helio.velocity + sun_ret.velocity;
                    distance_au = (target_bary_pos - earth.position).norm();
                    light_time0 = light_time;
                }

                let vector = target_bary_pos - earth.position;
                let velocity = target_bary_vel - earth.velocity;

                let astrometric = starfield::positions::Position::astrometric(vector, velocity, &earth, -1, light_time0);
                let (ra_hours, dec_deg, _) = astrometric.radec(None);

                let r_vec = -target_helio.position;
                let delta_vec = earth.position - target_bary_pos;
                let r = r_vec.norm();
                let delta = delta_vec.norm();

                let mag = h + 5.0 * delta.log10() + 2.5 * k * r.log10();

                planets_data.push((
                    "Comet".to_string(),
                    catalog_id.unwrap_or_default(),
                    ra_hours * 15.0,
                    dec_deg,
                    "Comet".to_string(),
                    "comet".to_string(),
                    Some(mag),
                    common_name.clone(),
                    common_name.map(|n| format!("The comet {}", n)),
                ));
            }

            let mut conn_guard = db_lock_clone.blocking_lock();
            if let Some(conn) = conn_guard.as_mut() {
                if let Ok(tx) = conn.transaction() {
                    let _ = tx.execute("DELETE FROM catalog WHERE catalog_label IN ('Planet', 'Asteroid', 'Comet')", []);

                    if let Ok(mut stmt) = tx.prepare(
                        "INSERT INTO catalog (catalog_label, catalog_entry, ra, dec, object_type, broad_category, magnitude, common_name, description) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    ) {
                        for (cl, ce, ra, dec, ot, bc, mag, cn, desc) in planets_data {
                            let _ = stmt.execute(rusqlite::params![cl, ce, ra, dec, ot, bc, mag, cn, desc]);
                        }
                    }
                    let _ = tx.commit();
                }
            }
        }).await.unwrap();
    }

    fn get_catalog_descriptions(&self) -> Vec<CatalogDescription> {
        self.catalog_descriptions.lock().unwrap().clone()
    }

    fn get_object_types(&self) -> Vec<ObjectType> {
        self.object_types.lock().unwrap().clone()
    }

    fn get_constellations(&self) -> Vec<Constellation> {
        self.constellations.lock().unwrap().clone()
    }

    async fn query_catalog_entries(
        &self,
        max_distance: Option<f64>,
        min_elevation: Option<f64>,
        faintest_magnitude: Option<i32>,
        match_catalog_label: bool,
        catalog_label: &[String],
        match_object_type_label: bool,
        object_type_label: &[String],
        text_search: Option<String>,
        ordering: Option<Ordering>,
        _decrowd_distance: Option<f64>,
        limit_result: Option<usize>,
        sky_location: Option<CelestialCoord>,
        location_info: Option<LocationInfo>,
    ) -> Result<(Vec<SelectedCatalogEntry>, usize), CanonicalError> {
        self.open().await?;

        let mut selection = String::from("1=1");
        let mut args: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(q) = text_search {
            let q_lower = q.trim().to_lowercase();
            let no_space_q = q_lower.replace(" ", "");

            let (prefix, num) = match q_lower.chars().position(|c| c.is_ascii_digit()) {
                Some(idx) => {
                    let p = q_lower[..idx].trim().to_string();
                    let n = q_lower[idx..].trim().to_string();
                    (p, n)
                }
                None => (q_lower.clone(), String::new()),
            };

            let catalog_aliases = [
                ("messier", "M"),
                ("m", "M"),
                ("caldwell", "C"),
                ("c", "C"),
                ("collinder", "Col"),
                ("col", "Col"),
                ("cr", "Col"),
                ("ngc", "NGC"),
                ("ic", "IC"),
                ("herschel", "H"),
                ("h", "H"),
                ("barnard", "B"),
                ("b", "B"),
                ("sharpless", "Sh2"),
                ("sh2", "Sh2"),
                ("sh", "Sh2"),
                ("abell", "Abl"),
                ("abl", "Abl"),
                ("arp", "Arp"),
            ];

            let label = catalog_aliases
                .iter()
                .find(|(k, _)| *k == prefix.as_str())
                .map(|(_, v)| v.to_string());

            let mut clause = String::from(
                "(catalog_label || ' ' || catalog_entry LIKE ? OR REPLACE(catalog_label || catalog_entry, ' ', '') LIKE ? OR common_name LIKE ? OR catalog_entry = ?)",
            );
            args.push(rusqlite::types::Value::Text(format!("%{}%", q_lower)));
            args.push(rusqlite::types::Value::Text(format!("%{}%", no_space_q)));
            args.push(rusqlite::types::Value::Text(format!("%{}%", q_lower)));
            args.push(rusqlite::types::Value::Text(q_lower.clone()));

            if let Some(lbl) = label {
                if !num.is_empty() {
                    clause = format!("({} OR (catalog_label = ? AND catalog_entry = ?))", clause);
                    args.push(rusqlite::types::Value::Text(lbl));
                    args.push(rusqlite::types::Value::Text(num));
                } else {
                    clause = format!("({} OR catalog_label = ?)", clause);
                    args.push(rusqlite::types::Value::Text(lbl));
                }
            }
            selection.push_str(&format!(" AND {}", clause));
        }

        if let Some(mag) = faintest_magnitude {
            selection.push_str(" AND (magnitude IS NULL OR magnitude <= ?)");
            args.push(rusqlite::types::Value::Real(mag as f64));
        }

        if !catalog_label.is_empty() {
            let mapped_labels: Vec<String> = catalog_label
                .iter()
                .map(|lbl| match lbl.to_uppercase().as_str() {
                    "IAU" => "Str".to_string(),
                    "PL" => "Planet".to_string(),
                    _ => lbl.clone(),
                })
                .collect();

            let in_clause = mapped_labels
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let op = if match_catalog_label { "IN" } else { "NOT IN" };
            selection.push_str(&format!(" AND LOWER(catalog_label) {} ({})", op, in_clause));
            for lbl in mapped_labels {
                args.push(rusqlite::types::Value::Text(lbl.to_lowercase()));
            }
        }

        if !object_type_label.is_empty() {
            let in_clause = object_type_label
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let op = if match_object_type_label {
                "IN"
            } else {
                "NOT IN"
            };
            selection.push_str(&format!(" AND LOWER(object_type) {} ({})", op, in_clause));
            for lbl in object_type_label {
                args.push(rusqlite::types::Value::Text(lbl.to_lowercase()));
            }
        }

        if let Some(dist) = max_distance {
            if let Some(target) = sky_location.as_ref() {
                let dist_rad = dist.to_radians();
                let cos_dist = dist_rad.cos();
                let target_ra_rad = target.ra.to_radians();
                let target_dec_rad = target.dec.to_radians();
                let sin_dec = target_dec_rad.sin();
                let cos_dec = target_dec_rad.cos();

                let dist_condition = format!(
                    "(SIN(dec * {}) * {} + COS(dec * {}) * {} * COS((ra * {}) - {})) >= {}",
                    std::f64::consts::PI / 180.0,
                    sin_dec,
                    std::f64::consts::PI / 180.0,
                    cos_dec,
                    std::f64::consts::PI / 180.0,
                    target_ra_rad,
                    cos_dist
                );
                selection.push_str(&format!(" AND {}", dist_condition));
            }
        }

        if let Some(min_elev) = min_elevation {
            if let Some(loc) = location_info.as_ref() {
                let lat = loc.observer_location.latitude;
                let lon = loc.observer_location.longitude;

                let time_ms = loc
                    .observing_time
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64;
                let days_since_j2000 = (time_ms - 946728000000.0) / 86400000.0;
                let mut gmst = (280.46061837 + 360.98564736629 * days_since_j2000) % 360.0;
                if gmst < 0.0 {
                    gmst += 360.0;
                }
                let mut lst = (gmst + lon) % 360.0;
                if lst < 0.0 {
                    lst += 360.0;
                }
                let lst_rad = lst.to_radians();

                let lat_rad = lat.to_radians();
                let sin_lat = lat_rad.sin();
                let cos_lat = lat_rad.cos();

                let min_elev_rad_sin = min_elev.to_radians().sin();

                let alt_condition = format!(
                    "(SIN(dec * {}) * {} + COS(dec * {}) * {} * COS({} - ra * {})) > {}",
                    std::f64::consts::PI / 180.0,
                    sin_lat,
                    std::f64::consts::PI / 180.0,
                    cos_lat,
                    lst_rad,
                    std::f64::consts::PI / 180.0,
                    min_elev_rad_sin
                );
                selection.push_str(&format!(" AND {}", alt_condition));
            }
        }

        // Assign numeric sort weights to catalog prefixes so that popular catalogs (Messier, Caldwell, NGC) appear first when sorting alphabetically.
        let priority_case = "CASE c.catalog_label \
            WHEN 'M' THEN 1 \
            WHEN 'C' THEN 2 \
            WHEN 'Col' THEN 3 \
            WHEN 'NGC' THEN 4 \
            WHEN 'IC' THEN 5 \
            WHEN 'H' THEN 6 \
            WHEN 'B' THEN 7 \
            WHEN 'Sh2' THEN 8 \
            WHEN 'Abl' THEN 9 \
            WHEN 'Arp' THEN 10 \
            ELSE 11 END";

        // HACK: cedar-server strictly filters the `ordering` protobuf enum to 1, 2, or 3.
        // To avoid maintaining a fork of cedar-server just to support Name/CatalogID sorting,
        // we smuggle the sort order through `_decrowd_distance` (which is otherwise unused).
        // -1.0 = Sort by Name
        // -2.0 = Sort by Catalog ID
        let order_clause = if _decrowd_distance == Some(-1.0) {
            "ORDER BY (CASE WHEN c.common_name IS NULL OR c.common_name = '' THEN 1 ELSE 0 END) ASC, c.common_name ASC, CAST(c.catalog_entry AS INTEGER) ASC NULLS LAST, c.catalog_entry ASC".to_string()
        } else if _decrowd_distance == Some(-2.0) {
            "ORDER BY (CASE c.catalog_label \
                WHEN 'Planet' THEN 0 \
                WHEN 'Moon' THEN 0 \
                WHEN 'Solar System' THEN 0 \
                WHEN 'Str' THEN 1 \
                WHEN 'M' THEN 2 \
                WHEN 'C' THEN 3 \
                WHEN 'Col' THEN 4 \
                WHEN 'NGC' THEN 5 \
                WHEN 'IC' THEN 6 \
                WHEN 'Asteroid' THEN 7 \
                WHEN 'Comet' THEN 8 \
                WHEN 'H' THEN 9 \
                WHEN 'B' THEN 10 \
                WHEN 'Sh2' THEN 11 \
                WHEN 'Abl' THEN 12 \
                WHEN 'Arp' THEN 13 \
                WHEN 'SaA' THEN 14 \
                WHEN 'SaM' THEN 15 \
                WHEN 'SaR' THEN 16 \
                WHEN 'Ta2' THEN 17 \
                WHEN 'Har' THEN 18 \
                WHEN 'RDS' THEN 19 \
                WHEN 'EGC' THEN 20 \
                ELSE 99 END) ASC, \
                CAST(c.catalog_entry AS INTEGER) ASC NULLS LAST, c.catalog_entry ASC"
                .to_string()
        } else {
            match ordering {
                Some(Ordering::Brightness) => {
                    format!("ORDER BY c.magnitude ASC NULLS LAST, {} ASC", priority_case)
                }
                Some(Ordering::SkyLocation) => {
                    if let Some(target) = sky_location.as_ref() {
                        let target_ra_rad = target.ra.to_radians();
                        let target_dec_rad = target.dec.to_radians();
                        let sin_dec = target_dec_rad.sin();
                        let cos_dec = target_dec_rad.cos();
                        format!(
                            "ORDER BY (SIN(dec * {}) * {} + COS(dec * {}) * {} * COS((ra * {}) - {})) DESC NULLS LAST",
                            std::f64::consts::PI / 180.0,
                            sin_dec,
                            std::f64::consts::PI / 180.0,
                            cos_dec,
                            std::f64::consts::PI / 180.0,
                            target_ra_rad
                        )
                    } else {
                        format!("ORDER BY c.magnitude ASC NULLS LAST, {} ASC", priority_case)
                    }
                }
                _ => format!("ORDER BY c.magnitude ASC NULLS LAST, {} ASC", priority_case),
            }
        };
        let limit = limit_result.unwrap_or(500);

        let query = format!(
            "SELECT c.catalog_entry, c.ra, c.dec, c.magnitude, c.catalog_label, c.object_type, c.broad_category, c.constellation, c.common_name, c.angular_size FROM catalog c WHERE {} {} LIMIT {}",
            selection, order_clause, limit
        );

        let conn_guard = self.db.lock().await;
        let conn = conn_guard.as_ref().unwrap();
        let mut stmt = conn.prepare(&query).map_err(|e| {
            println!("Query prepare error: {}", e);
            canonical_error::unknown_error("prepare failed")
        })?;

        let rows = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), |row| {
                let id: String = row.get(0)?;
                let ra: f64 = row.get(1)?;
                let dec: f64 = row.get(2)?;
                let magnitude: f64 = row.get(3).unwrap_or(99.0);
                let catalog_label: String = row.get(4).unwrap_or_default();
                let object_type: String = row.get(5).unwrap_or_default();
                let broad_category: String = row.get(6).unwrap_or_default();
                let constellation: String = row.get(7).unwrap_or_default();
                let common_name: String = row.get(8).unwrap_or_default();
                let angular_size: Option<String> = row.get(9).unwrap_or(None);

                let mut out_label = catalog_label.clone();
                let out_entry = id.clone();
                let mut out_common = if common_name.is_empty() {
                    None
                } else {
                    Some(common_name.clone())
                };

                let is_star_or_planet = out_label == "Str"
                    || out_label == "Planet"
                    || out_label == "Asteroid"
                    || out_label == "Comet";

                if let Some(cname) = out_common.take() {
                    let designation = if is_star_or_planet {
                        out_entry.clone()
                    } else {
                        format!("{}{}", out_label, out_entry)
                    };

                    let cname_norm = cname.replace(" ", "").to_lowercase();
                    let desig_norm = designation.replace(" ", "").to_lowercase();

                    // If the common name is just a spaced variant of the catalog designation (e.g. 'NGC 5340' vs 'NGC5340'), suppress the common name to prevent duplicate rendering in the frontend.
                    if cname_norm != desig_norm {
                        out_common = Some(cname);
                    }
                }

                if is_star_or_planet {
                    out_label = "".to_string();
                }

                let entry = CatalogEntry {
                    catalog_label: out_label,
                    catalog_entry: out_entry,
                    coord: Some(CelestialCoord {
                        ra,
                        dec,
                        epoch: Some(2000.0),
                    }),
                    constellation: if constellation.is_empty() {
                        None
                    } else {
                        Some(cedar_elements::cedar_sky::Constellation {
                            label: constellation,
                            name: String::new(),
                        })
                    },
                    object_type: Some(cedar_elements::cedar_sky::ObjectType {
                        label: object_type,
                        broad_category,
                    }),
                    magnitude: Some(magnitude),
                    angular_size,
                    common_name: out_common,
                    notes: None,
                };

                let sel_entry = SelectedCatalogEntry {
                    entry: Some(entry),
                    deduped_entries: Vec::new(),
                    decrowded_entries: Vec::new(),
                    altitude: None,
                    azimuth: None,
                };
                Ok(sel_entry)
            })
            .map_err(|e| {
                println!("Query exec error: {}", e);
                canonical_error::unknown_error("query failed")
            })?;

        let mut results: Vec<SelectedCatalogEntry> = Vec::new();
        let do_decrowd = _decrowd_distance.is_some() && _decrowd_distance.unwrap() > 0.0;
        let decrowd_distance_rad = if do_decrowd {
            _decrowd_distance.unwrap() / 3600.0 * (std::f64::consts::PI / 180.0)
        } else {
            0.0
        };

        for row in rows {
            if let Ok(sel_entry) = row {
                if do_decrowd {
                    let entry_coord = sel_entry.entry.as_ref().unwrap().coord.as_ref().unwrap();
                    let ra1 = entry_coord.ra.to_radians();
                    let dec1 = entry_coord.dec.to_radians();

                    let mut crowded_into = None;
                    for (i, final_entry) in results.iter_mut().enumerate() {
                        let f_coord = final_entry.entry.as_ref().unwrap().coord.as_ref().unwrap();
                        let ra2 = f_coord.ra.to_radians();
                        let dec2 = f_coord.dec.to_radians();

                        let cos_dist = (dec1.sin() * dec2.sin()
                            + dec1.cos() * dec2.cos() * (ra1 - ra2).cos())
                        .clamp(-1.0, 1.0);
                        let dist = cos_dist.acos();

                        if dist < decrowd_distance_rad {
                            crowded_into = Some(i);
                            break;
                        }
                    }

                    if let Some(idx) = crowded_into {
                        results[idx]
                            .decrowded_entries
                            .push(sel_entry.entry.unwrap());
                    } else {
                        results.push(sel_entry);
                    }
                } else {
                    results.push(sel_entry);
                }
            }
        }

        let skipped = 0;

        Ok((results, skipped))
    }

    async fn get_catalog_entry(
        &mut self,
        entry_key: CatalogEntryKey,
        _timestamp: SystemTime,
    ) -> Result<CatalogEntry, CanonicalError> {
        let conn_guard = self.db.lock().await;
        let conn = conn_guard.as_ref().unwrap();
        let mut stmt = conn.prepare(
            "SELECT catalog_entry, ra, dec, magnitude, catalog_label, object_type, broad_category, constellation, common_name, angular_size FROM catalog WHERE (catalog_entry = ? AND catalog_label = ?) OR common_name = ?"
        ).map_err(|_| canonical_error::unknown_error("prepare failed"))?;

        let mut rows = stmt
            .query_map(
                rusqlite::params![entry_key.entry, entry_key.cat_label, entry_key.entry],
                |row| {
                    let id: String = row.get(0)?;
                    let ra: f64 = row.get(1)?;
                    let dec: f64 = row.get(2)?;
                    let magnitude: f64 = row.get(3).unwrap_or(99.0);
                    let catalog_label: String = row.get(4).unwrap_or_default();
                    let object_type: String = row.get(5).unwrap_or_default();
                    let broad_category: String = row.get(6).unwrap_or_default();
                    let constellation: String = row.get(7).unwrap_or_default();
                    let common_name: String = row.get(8).unwrap_or_default();
                    let angular_size: Option<String> = row.get(9).unwrap_or(None);

                    let mut out_label = catalog_label.clone();
                    let out_entry = id.clone();
                    let mut out_common = if common_name.is_empty() {
                        None
                    } else {
                        Some(common_name.clone())
                    };

                    let is_star_or_planet = out_label == "Str"
                        || out_label == "Planet"
                        || out_label == "Asteroid"
                        || out_label == "Comet";

                    if let Some(cname) = out_common.take() {
                        let designation = if is_star_or_planet {
                            out_entry.clone()
                        } else {
                            format!("{}{}", out_label, out_entry)
                        };

                        let cname_norm = cname.replace(" ", "").to_lowercase();
                        let desig_norm = designation.replace(" ", "").to_lowercase();

                        // If the common name is just a spaced variant of the catalog designation (e.g. 'NGC 5340' vs 'NGC5340'), suppress the common name to prevent duplicate rendering in the frontend.
                        if cname_norm != desig_norm {
                            out_common = Some(cname);
                        }
                    }

                    if is_star_or_planet {
                        out_label = "".to_string();
                    }

                    Ok(CatalogEntry {
                        catalog_label: out_label,
                        catalog_entry: out_entry,
                        coord: Some(CelestialCoord {
                            ra,
                            dec,
                            epoch: Some(2000.0),
                        }),
                        constellation: if constellation.is_empty() {
                            None
                        } else {
                            Some(cedar_elements::cedar_sky::Constellation {
                                label: constellation,
                                name: String::new(),
                            })
                        },
                        object_type: Some(cedar_elements::cedar_sky::ObjectType {
                            label: object_type,
                            broad_category,
                        }),
                        magnitude: Some(magnitude),
                        angular_size,
                        common_name: out_common,
                        notes: None,
                    })
                },
            )
            .map_err(|_| canonical_error::unknown_error("query failed"))?;

        if let Some(Ok(entry)) = rows.next() {
            Ok(entry)
        } else {
            Err(canonical_error::not_found_error("entry not found"))
        }
    }
}
