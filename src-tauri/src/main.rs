// src-tauri/src/main.rs - НОВАЯ ВЕРСИЯ С AWS TERRAIN TILES - ЧАСТЬ 1
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use reqwest;
use tokio;

#[derive(Debug, Serialize, Deserialize)]
struct BoundingBox {
    min_lat: f64,
    min_lng: f64,
    max_lat: f64,
    max_lng: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenerationProgress {
    stage: String,
    progress: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OSMElement {
    id: i64,
    element_type: String,
    lat: Option<f64>,
    lon: Option<f64>,
    tags: std::collections::HashMap<String, String>,
    nodes: Option<Vec<i64>>,
}

#[tauri::command]
async fn generate_terrain(
    bbox: BoundingBox,
    output_path: String,
    window: tauri::Window,
) -> Result<String, String> {
    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Initializing".to_string(),
        progress: 0.0,
    });

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Downloading terrain data from AWS".to_string(),
        progress: 10.0,
    });
    
    let terrain_data = fetch_aws_terrain_tiles(&bbox).await
        .map_err(|e| format!("Failed to fetch AWS terrain: {}", e))?;

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Fetching OpenStreetMap data".to_string(),
        progress: 30.0,
    });
    
    let osm_data = fetch_osm_data(&bbox).await
        .map_err(|e| format!("Failed to fetch OSM data: {}", e))?;

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Processing terrain heightmap".to_string(),
        progress: 50.0,
    });
    
    let heightmap = process_terrain_data(&terrain_data, &bbox)?;

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Converting objects to BeamNG format".to_string(),
        progress: 70.0,
    });
    
    let (beamng_objects, road_network) = convert_osm_to_beamng(&osm_data, &bbox)?;

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Generating BeamNG map files".to_string(),
        progress: 85.0,
    });
    
    generate_beamng_files(&output_path, &heightmap, &beamng_objects, &road_network)?;

    let _ = window.emit("generation-progress", GenerationProgress {
        stage: "Complete".to_string(),
        progress: 100.0,
    });

    Ok(format!("Map generated successfully at: {}", output_path))
}

// НОВАЯ ФУНКЦИЯ: Загрузка напрямую из AWS Terrain Tiles
async fn fetch_aws_terrain_tiles(bbox: &BoundingBox) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // AWS Terrain Tiles доступны через несколько источников:
    // 1. Mapzen Terrarium format (открытый источник)
    // 2. Terrain-RGB от AWS
    
    // Используем Terrain Tiles из открытых источников
    // Zoom level: чем выше, тем детальнее (рекомендуется 10-14)
    let zoom = 12;
    let tiles = calculate_tiles(bbox, zoom);
    
    let mut all_terrain_data = Vec::new();
    let client = reqwest::Client::builder()
        .user_agent("BeamNG-Terrain-Generator/1.0")
        .build()?;

    println!("Downloading {} terrain tiles from AWS...", tiles.len());

    for (tile_x, tile_y) in tiles {
        // Используем открытый источник AWS Terrain Tiles
        // Terrarium format: https://registry.opendata.aws/terrain-tiles/
        let url = format!(
            "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
            zoom, tile_x, tile_y
        );
        
        println!("Fetching tile: {}/{}/{}", zoom, tile_x, tile_y);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let bytes = response.bytes().await?;
                    all_terrain_data.extend_from_slice(&bytes);
                    println!("✓ Downloaded tile {}/{}", tile_x, tile_y);
                } else {
                    eprintln!("Failed to download tile {}/{}: {}", tile_x, tile_y, response.status());
                    // Создаём пустой тайл если не удалось загрузить
                    all_terrain_data.extend_from_slice(&create_empty_tile());
                }
            }
            Err(e) => {
                eprintln!("Error downloading tile {}/{}: {}", tile_x, tile_y, e);
                all_terrain_data.extend_from_slice(&create_empty_tile());
            }
        }
    }

    if all_terrain_data.is_empty() {
        return Err("No terrain data downloaded".into());
    }

    Ok(all_terrain_data)
}

// Альтернативная функция: Использование Mapzen Terrarium (бесплатно)
async fn fetch_mapzen_terrarium(bbox: &BoundingBox) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let zoom = 12;
    let tiles = calculate_tiles(bbox, zoom);
    
    let mut all_terrain_data = Vec::new();
    let client = reqwest::Client::builder()
        .user_agent("BeamNG-Terrain-Generator/1.0")
        .build()?;

    for (tile_x, tile_y) in tiles {
        // Mapzen Terrarium - открытый источник elevation data
        let url = format!(
            "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
            zoom, tile_x, tile_y
        );
        
        let response = client.get(&url).send().await?;
        let bytes = response.bytes().await?;
        all_terrain_data.extend_from_slice(&bytes);
    }

    Ok(all_terrain_data)
}

// Альтернативная функция: Использование OpenTopoData API (бесплатно, но медленнее)
async fn fetch_opentopo_data(bbox: &BoundingBox) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    // OpenTopoData API - бесплатный источник elevation данных
    // Ограничение: 100 точек за запрос
    
    let resolution = 100; // Точек по каждой оси
    let mut heightmap = vec![vec![0.0; resolution]; resolution];
    
    let lat_step = (bbox.max_lat - bbox.min_lat) / resolution as f64;
    let lng_step = (bbox.max_lng - bbox.min_lng) / resolution as f64;
    
    let client = reqwest::Client::new();
    
    // Собираем точки батчами по 100
    let mut locations = Vec::new();
    for i in 0..resolution {
        for j in 0..resolution {
            let lat = bbox.min_lat + (i as f64 * lat_step);
            let lng = bbox.min_lng + (j as f64 * lng_step);
            locations.push(format!("{},{}", lat, lng));
            
            // Когда набралось 100 точек или это последняя итерация
            if locations.len() >= 100 || (i == resolution - 1 && j == resolution - 1) {
                let locations_param = locations.join("|");
                let url = format!(
                    "https://api.opentopodata.org/v1/aster30m?locations={}",
                    locations_param
                );
                
                let response = client.get(&url).send().await?;
                let json: serde_json::Value = response.json().await?;
                
                if let Some(results) = json["results"].as_array() {
                    for (idx, result) in results.iter().enumerate() {
                        if let Some(elevation) = result["elevation"].as_f64() {
                            let original_idx = (i * resolution + j) - (locations.len() - 1 - idx);
                            let row = original_idx / resolution;
                            let col = original_idx % resolution;
                            if row < resolution && col < resolution {
                                heightmap[row][col] = elevation as f32;
                            }
                        }
                    }
                }
                
                locations.clear();
                
                // Rate limiting: подождать между запросами
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
    
    Ok(heightmap)
}

fn create_empty_tile() -> Vec<u8> {
    // Создаём пустой PNG тайл 256x256 с нулевой высотой
    vec![0u8; 256 * 256 * 3]
}

async fn fetch_osm_data(bbox: &BoundingBox) -> Result<Vec<OSMElement>, Box<dyn std::error::Error>> {
    let query = format!(
        r#"[out:json][timeout:180];
        (
          way["building"]({},{},{},{});
          way["highway"]({},{},{},{});
          node["natural"="tree"]({},{},{},{});
          way["natural"="tree_row"]({},{},{},{});
          node["highway"="bus_stop"]({},{},{},{});
          way["amenity"]({},{},{},{});
        );
        out body;
        >;
        out skel qt;"#,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
        bbox.min_lat, bbox.min_lng, bbox.max_lat, bbox.max_lng,
    );

    let client = reqwest::Client::new();
    let response = client
        .post("https://overpass-api.de/api/interpreter")
        .body(query)
        .send()
        .await?;

    let osm_json: serde_json::Value = response.json().await?;
    let elements: Vec<OSMElement> = serde_json::from_value(osm_json["elements"].clone())?;
    
    Ok(elements)
}

fn calculate_tiles(bbox: &BoundingBox, zoom: u32) -> Vec<(u32, u32)> {
    let min_tile = lat_lng_to_tile(bbox.min_lat, bbox.min_lng, zoom);
    let max_tile = lat_lng_to_tile(bbox.max_lat, bbox.max_lng, zoom);
    
    let mut tiles = Vec::new();
    for x in min_tile.0..=max_tile.0 {
        for y in min_tile.1..=max_tile.1 {
            tiles.push((x, y));
        }
    }
    tiles
}

fn lat_lng_to_tile(lat: f64, lng: f64, zoom: u32) -> (u32, u32) {
    let n = 2_f64.powi(zoom as i32);
    let x = ((lng + 180.0) / 360.0 * n) as u32;
    let y = ((1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI) / 2.0 * n) as u32;
    (x, y)
}

fn process_terrain_data(data: &[u8], bbox: &BoundingBox) -> Result<Vec<Vec<f32>>, String> {
    // Terrarium format decoding
    // Height = (R * 256 + G + B / 256) - 32768
    
    let img = image::load_from_memory(data)
        .map_err(|e| format!("Failed to load terrain image: {}", e))?;
    
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    
    let mut heightmap = vec![vec![0.0; width as usize]; height as usize];
    
    for y in 0..height {
        for x in 0..width {
            let pixel = rgb.get_pixel(x, y);
            let r = pixel[0] as f32;
            let g = pixel[1] as f32;
            let b = pixel[2] as f32;
            
            // Terrarium format: height = (R * 256 + G + B / 256) - 32768
            let height_meters = (r * 256.0 + g + b / 256.0) - 32768.0;
            heightmap[y as usize][x as usize] = height_meters;
        }
    }
    
    Ok(heightmap)
}

#[derive(Debug, Serialize, Clone)]
struct BeamNGObject {
    obj_type: String,
    position: (f32, f32, f32),
    properties: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
struct RoadNode {
    id: String,
    position: (f32, f32, f32),
    width: f32,
    road_type: String,
}

#[derive(Debug, Serialize, Clone)]
struct RoadSegment {
    id: String,
    start_node: String,
    end_node: String,
    width: f32,
    lanes: u32,
    road_type: String,
    one_way: bool,
}

#[derive(Debug, Serialize)]
struct RoadNetwork {
    nodes: Vec<RoadNode>,
    segments: Vec<RoadSegment>,
}

fn convert_osm_to_beamng(
    elements: &[OSMElement],
    bbox: &BoundingBox,
) -> Result<(Vec<BeamNGObject>, RoadNetwork), String> {
    let mut objects = Vec::new();
    let mut road_nodes = Vec::new();
    let mut road_segments = Vec::new();
    let mut node_positions: std::collections::HashMap<i64, (f64, f64)> = std::collections::HashMap::new();
    
    for element in elements {
        if element.element_type == "node" {
            if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                node_positions.insert(element.id, (lat, lon));
            }
        }
    }
    
    for element in elements {
        let tags = &element.tags;
        
        if tags.contains_key("building") {
            if let Some(nodes) = &element.nodes {
                if let Some(&first_node_id) = nodes.first() {
                    if let Some(&(lat, lon)) = node_positions.get(&first_node_id) {
                        objects.push(BeamNGObject {
                            obj_type: "building".to_string(),
                            position: latlon_to_beamng(lat, lon, bbox),
                            properties: tags.clone(),
                        });
                    }
                }
            }
        }
        
        if tags.get("natural") == Some(&"tree".to_string()) {
            if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                objects.push(BeamNGObject {
                    obj_type: "tree".to_string(),
                    position: latlon_to_beamng(lat, lon, bbox),
                    properties: tags.clone(),
                });
            }
        }
        
        if tags.get("highway") == Some(&"bus_stop".to_string()) {
            if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                objects.push(BeamNGObject {
                    obj_type: "bus_stop".to_string(),
                    position: latlon_to_beamng(lat, lon, bbox),
                    properties: tags.clone(),
                });
            }
        }
        
        if tags.contains_key("highway") {
            if let Some(nodes) = &element.nodes {
                let highway_type = tags.get("highway").unwrap_or(&"road".to_string()).clone();
                let lanes = parse_lanes(tags.get("lanes"));
                let width = calculate_road_width(&highway_type, lanes);
                let one_way = tags.get("oneway") == Some(&"yes".to_string());
                
                for (i, &node_id) in nodes.iter().enumerate() {
                    if let Some(&(lat, lon)) = node_positions.get(&node_id) {
                        let node_pos = latlon_to_beamng(lat, lon, bbox);
                        
                        road_nodes.push(RoadNode {
                            id: format!("node_{}_{}", element.id, node_id),
                            position: node_pos,
                            width,
                            road_type: highway_type.clone(),
                        });
                        
                        if i > 0 {
                            let prev_node_id = nodes[i - 1];
                            road_segments.push(RoadSegment {
                                id: format!("segment_{}_{}_{}", element.id, prev_node_id, node_id),
                                start_node: format!("node_{}_{}", element.id, prev_node_id),
                                end_node: format!("node_{}_{}", element.id, node_id),
                                width,
                                lanes,
                                road_type: highway_type.clone(),
                                one_way,
                            });
                        }
                    }
                }
            }
        }
    }
    
    let road_network = RoadNetwork {
        nodes: road_nodes,
        segments: road_segments,
    };
    
    Ok((objects, road_network))
}

fn parse_lanes(lanes_str: Option<&String>) -> u32 {
    lanes_str
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(2)
}

fn calculate_road_width(highway_type: &str, lanes: u32) -> f32 {
    let lane_width = 3.5;
    
    match highway_type {
        "motorway" | "trunk" => lanes as f32 * lane_width + 2.0,
        "primary" => lanes as f32 * lane_width + 1.5,
        "secondary" => lanes as f32 * lane_width + 1.0,
        "tertiary" => lanes as f32 * lane_width + 0.5,
        "residential" | "service" => lanes as f32 * 3.0,
        "path" | "footway" | "cycleway" => 2.0,
        _ => lanes as f32 * lane_width,
    }
}

fn latlon_to_beamng(lat: f64, lon: f64, bbox: &BoundingBox) -> (f32, f32, f32) {
    let x = ((lon - bbox.min_lng) / (bbox.max_lng - bbox.min_lng) * 2048.0) as f32;
    let y = 0.0;
    let z = ((lat - bbox.min_lat) / (bbox.max_lat - bbox.min_lat) * 2048.0) as f32;
    (x, y, z)
}

fn generate_beamng_files(
    output_path: &str,
    heightmap: &[Vec<f32>],
    objects: &[BeamNGObject],
    road_network: &RoadNetwork,
) -> Result<(), String> {
    use std::fs;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::ZipWriter;
    
    let path = PathBuf::from(output_path);
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    
    let mod_name = "generated_map";
    let mod_path = path.join(mod_name);
    let levels_path = mod_path.join("levels");
    let level_path = levels_path.join(mod_name);
    let art_path = level_path.join("art");
    let art_terrains_path = art_path.join("terrains");
    
    fs::create_dir_all(&level_path).map_err(|e| e.to_string())?;
    fs::create_dir_all(&art_terrains_path).map_err(|e| e.to_string())?;
    
    generate_mod_info(&mod_path, mod_name)?;
    generate_main_level(&level_path, mod_name)?;
    generate_items_level(&level_path, objects)?;
    generate_road_files(&level_path, road_network)?;
    
    let heightmap_path = art_terrains_path.join("terrain.png");
    save_heightmap_as_png(heightmap, &heightmap_path)?;
    
    generate_terrain_files(&art_terrains_path, heightmap)?;
    generate_preview_image(&level_path)?;
    
    let zip_path = path.join(format!("{}.zip", mod_name));
    create_mod_zip(&mod_path, &zip_path)?;
    
    Ok(())
}

fn generate_mod_info(mod_path: &PathBuf, mod_name: &str) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    
    let info = serde_json::json!({
        "name": format!("Generated Map - {}", mod_name),
        "version": "1.0",
        "author": "BeamNG Terrain Generator",
        "description": "Automatically generated map from real-world data using OpenStreetMap and AWS Terrain Tiles",
        "gameVersion": "0.32",
        "modType": "level"
    });
    
    let info_path = mod_path.join("info.json");
    let mut file = File::create(info_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&info).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn generate_main_level(level_path: &PathBuf, mod_name: &str) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    
    let main_level = serde_json::json!({
        "main": {
            "levelName": format!("Generated - {}", mod_name),
            "title": "Generated Map",
            "description": "Map generated from real-world OpenStreetMap data",
            "authors": "BeamNG Terrain Generator",
            "biome": "Urban",
            "previews": ["preview.jpg"],
            "previewPosition": {
                "pos": [1024.0, 1024.0, 100.0],
                "rot": [0, 0, 1, 0]
            }
        },
        "spawn": {
            "defaultSpawnPoint": "spawn_0",
            "spawnPoints": [
                {
                    "objectname": "spawn_0",
                    "pos": [1024.0, 1024.0, 105.0],
                    "rot": [0, 0, 1, 0],
                    "rotationMatrix": [[1,0,0],[0,1,0],[0,0,1]]
                }
            ]
        },
        "sun": {
            "azimuth": 0.0,
            "elevation": 45.0,
            "shadowDistance": 1000.0,
            "shadowSoftness": 0.15
        }
    });
    
    let main_path = level_path.join("main.level.json");
    let mut file = File::create(main_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&main_level).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn generate_items_level(level_path: &PathBuf, objects: &[BeamNGObject]) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    
    let items = serde_json::json!({
        "objects": objects.iter().enumerate().map(|(i, obj)| {
            serde_json::json!({
                "class": get_beamng_object_class(&obj.obj_type),
                "persistentId": format!("{}_{}", obj.obj_type, i),
                "position": [obj.position.0, obj.position.1, obj.position.2],
                "rotation": [0, 0, 1, 0],
                "scale": [1, 1, 1],
                "__gameObjectId": i + 1000
            })
        }).collect::<Vec<_>>()
    });
    
    let items_path = level_path.join("items.level.json");
    let mut file = File::create(items_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&items).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn get_beamng_object_class(obj_type: &str) -> &str {
    match obj_type {
        "building" => "TSStatic",
        "tree" => "Forest",
        "bus_stop" => "TSStatic",
        _ => "TSStatic",
    }
}

fn generate_road_files(level_path: &PathBuf, road_network: &RoadNetwork) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    
    let road_nodes_json = serde_json::json!({
        "nodes": road_network.nodes.iter().map(|node| {
            serde_json::json!({
                "id": node.id,
                "position": node.position,
                "width": node.width,
                "roadType": node.road_type
            })
        }).collect::<Vec<_>>(),
        "segments": road_network.segments.iter().map(|seg| {
            serde_json::json!({
                "id": seg.id,
                "startNode": seg.start_node,
                "endNode": seg.end_node,
                "width": seg.width,
                "lanes": seg.lanes,
                "roadType": seg.road_type,
                "oneWay": seg.one_way
            })
        }).collect::<Vec<_>>()
    });
    
    let road_nodes_path = level_path.join("road_nodes.json");
    let mut file = File::create(road_nodes_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&road_nodes_json).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    let decal_road_json = generate_decal_road_format(road_network);
    let decal_path = level_path.join("decalRoad.json");
    let mut file = File::create(decal_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&decal_road_json).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn generate_decal_road_format(road_network: &RoadNetwork) -> serde_json::Value {
    let mut decal_roads = Vec::new();
    
    for segment in &road_network.segments {
        let start_node = road_network.nodes.iter()
            .find(|n| n.id == segment.start_node);
        let end_node = road_network.nodes.iter()
            .find(|n| n.id == segment.end_node);
        
        if let (Some(start), Some(end)) = (start_node, end_node) {
            decal_roads.push(serde_json::json!({
                "class": "DecalRoad",
                "persistentId": segment.id,
                "position": start.position,
                "detail": 4,
                "breakAngle": 3.0,
                "textureLength": 5.0,
                "Material": get_road_material(&segment.road_type),
                "nodes": [
                    {
                        "pos": [start.position.0, start.position.1, start.position.2],
                        "width": segment.width,
                        "widthLeft": segment.width / 2.0,
                        "widthRight": segment.width / 2.0
                    },
                    {
                        "pos": [end.position.0, end.position.1, end.position.2],
                        "width": segment.width,
                        "widthLeft": segment.width / 2.0,
                        "widthRight": segment.width / 2.0
                    }
                ]
            }));
        }
    }
    
    serde_json::json!({
        "decalRoads": decal_roads
    })
}

fn get_road_material(road_type: &str) -> &str {
    match road_type {
        "motorway" | "trunk" => "road_asphalt_highway",
        "primary" | "secondary" => "road_asphalt",
        "tertiary" | "residential" => "road_asphalt_residential",
        "service" => "road_concrete",
        "path" | "footway" | "cycleway" => "road_gravel",
        _ => "road_asphalt",
    }
}

fn generate_terrain_files(art_terrains_path: &PathBuf, heightmap: &[Vec<f32>]) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;
    
    let ter_json = serde_json::json!({
        "terrainSize": 2048,
        "squareSize": 1.0,
        "heightScale": 256.0,
        "heightMap": "terrain.png"
    });
    
    let ter_path = art_terrains_path.join("terrain.ter.json");
    let mut file = File::create(ter_path).map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&ter_json).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

fn generate_preview_image(level_path: &PathBuf) -> Result<(), String> {
    let img = image::ImageBuffer::from_fn(512, 512, |x, y| {
        let r = ((x as f32 / 512.0) * 255.0) as u8;
        let g = ((y as f32 / 512.0) * 255.0) as u8;
        let b = 128;
        image::Rgb([r, g, b])
    });
    
    let preview_path = level_path.join("preview.jpg");
    img.save(preview_path).map_err(|e| e.to_string())?;
    
    Ok(())
}

fn save_heightmap_as_png(heightmap: &[Vec<f32>], path: &PathBuf) -> Result<(), String> {
    let height = heightmap.len() as u32;
    let width = heightmap[0].len() as u32;
    
    let mut img = image::GrayImage::new(width, height);
    
    let min_h = heightmap.iter().flatten().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_h = heightmap.iter().flatten().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let range = max_h - min_h;
    
    for y in 0..height {
        for x in 0..width {
            let h = heightmap[y as usize][x as usize];
            let normalized = ((h - min_h) / range * 255.0) as u8;
            img.put_pixel(x, y, image::Luma([normalized]));
        }
    }
    
    img.save(path).map_err(|e| e.to_string())?;
    Ok(())
}

// src-tauri/src/main.rs - ЧАСТЬ 3 (финальная часть)
// ВАЖНО: Вставьте эту часть ПОСЛЕ части 2!

fn create_mod_zip(mod_path: &PathBuf, zip_path: &PathBuf) -> Result<(), String> {
    use std::fs::File;
    use walkdir::WalkDir;
    
    let file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    for entry in WalkDir::new(mod_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = path.strip_prefix(mod_path.parent().unwrap()).unwrap();
        
        if path.is_file() {
            zip.start_file(name.to_string_lossy().into_owned(), options)
                .map_err(|e| e.to_string())?;
            let content = std::fs::read(path).map_err(|e| e.to_string())?;
            use std::io::Write;
            zip.write_all(&content).map_err(|e| e.to_string())?;
        } else if !name.as_os_str().is_empty() {
            zip.add_directory(name.to_string_lossy().into_owned(), options)
                .map_err(|e| e.to_string())?;
        }
    }
    
    zip.finish().map_err(|e| e.to_string())?;
    
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![generate_terrain])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}