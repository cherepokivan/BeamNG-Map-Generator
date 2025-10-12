// bnggen - Rust CLI prototype
use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;
use zip::write::FileOptions;

fn println_progress(p: u8, text: &str) {
    let obj = serde_json::json!({"progress": p, "text": text});
    println!("{}", obj.to_string());
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 {
        eprintln!("Usage: bnggen min_lat min_lon max_lat max_lon outdir");
        std::process::exit(1);
    }
    let min_lat = &args[1];
    let min_lon = &args[2];
    let max_lat = &args[3];
    let max_lon = &args[4];
    let outdir = Path::new(&args[5]);
    create_dir_all(outdir)?;

    println_progress(5, "Fetching OSM data from Overpass API");
    let bbox = format!("{},{},{},{}", min_lat, min_lon, max_lat, max_lon);
    let client = Client::new();
    let overpass = "https://overpass-api.de/api/interpreter";
    let query = format!("[out:json][timeout:120];(way({});relation({});node({}););out body;>;out skel qt;", bbox, bbox, bbox);
    let resp = client.post(overpass).form(&[("data", &query)]).send()?;
    let v: Value = resp.json()?;
    let raw = outdir.join("osm_overpass.json");
    let mut rf = File::create(&raw)?;
    rf.write_all(serde_json::to_string_pretty(&v)?.as_bytes())?;

    println_progress(30, "Parsing OSM and generating simple assets");
    let models = outdir.join("models"); create_dir_all(&models)?;
    let tex = models.join("textures"); create_dir_all(&tex)?;
    std::fs::write(tex.join("asphalt.png"), b"PNG_PLACEHOLDER")?;
    std::fs::write(tex.join("roof.png"), b"PNG_PLACEHOLDER")?;

    std::fs::write(models.join("building_placeholder.dae"), b"<COLLADA/>")?;
    std::fs::write(outdir.join("buildings.json"), b"{}")?;
    std::fs::write(outdir / "trees.json", b"{}")?;

    println_progress(70, "Packaging mod into zip");
    let zip_path = outdir.join("osm_generated_mod.zip");
    let zip_file = File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = FileOptions::default();
    zip.start_file("levels/level/metadata.json", options)?;
    zip.write_all(b"{}")?;
    zip.start_file("levels/level/models/building_placeholder.dae", options)?;
    zip.write_all(b"<COLLADA/>")?;
    zip.finish()?;

    println_progress(100, "Done");
    println!("OUTPUT:{}", zip_path.display());
    Ok(())
}
