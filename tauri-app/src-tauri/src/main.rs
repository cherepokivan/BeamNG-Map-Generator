#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::Deserialize;
use tauri::Manager;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead};
use std::path::PathBuf;

#[derive(Deserialize)]
struct BBox { south: f64, west: f64, north: f64, east: f64 }

#[tauri::command]
fn generate_map(app: tauri::AppHandle, bbox: BBox) -> Result<String, String> {
    let outdir = std::env::temp_dir().join("bng_out");
    let outdir_s = outdir.to_string_lossy().to_string();
    let bnggen_path = if cfg!(debug_assertions) {
        std::env::current_dir().unwrap().join("../bnggen/target/debug/bnggen")
    } else {
        std::env::current_exe().unwrap().parent().unwrap().join("bnggen.exe")
    };
    let mut child = Command::new(bnggen_path)
        .arg(bbox.south.to_string())
        .arg(bbox.west.to_string())
        .arg(bbox.north.to_string())
        .arg(bbox.east.to_string())
        .arg(&outdir_s)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn bnggen: {}", e))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = app.emit_all("bng_progress", l.clone());
            }
        }
    }
    let status = child.wait().map_err(|e| format!("bnggen wait failed: {}", e))?;
    if !status.success() { return Err("bnggen failed".into()); }
    Ok(outdir_s)
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![generate_map])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
