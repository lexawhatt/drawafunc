use std::fs;
use std::path::Path;

use crate::model::{DrawStroke, Project};

pub(crate) const PROJECT_FILE: &str = "drawafunc_project.json";

pub(crate) fn save_strokes(strokes: &[DrawStroke]) -> Result<(), String> {
    let project = Project::from_strokes(strokes);
    serde_json::to_string_pretty(&project)
        .map_err(|err| err.to_string())
        .and_then(|json| fs::write(PROJECT_FILE, json.as_bytes()).map_err(|err| err.to_string()))
}

pub(crate) fn load_project() -> Result<Project, String> {
    let path = Path::new(PROJECT_FILE);
    fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|json| serde_json::from_str::<Project>(&json).map_err(|err| err.to_string()))
}

pub(crate) fn project_hash(strokes: &[DrawStroke]) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for stroke in strokes {
        hash ^= stroke.points.len() as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
        for point in &stroke.points {
            hash ^= point.x.to_bits() as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
            hash ^= point.y.to_bits() as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
    }
    hash
}
