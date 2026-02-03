use anyhow::Result;
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};
use zip::ZipArchive;

pub fn zip_to_file_map(zip_data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut archive = ZipArchive::new(Cursor::new(zip_data))?;
    let mut saw_no_slash = false;
    let mut first_segments: HashSet<String> = HashSet::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name();
        if let Some((first, _)) = name.split_once('/') {
            first_segments.insert(first.to_string());
        } else {
            saw_no_slash = true;
        }
    }

    let root_prefix = if !saw_no_slash && first_segments.len() == 1 {
        Some(format!("{}/", first_segments.into_iter().next().unwrap()))
    } else {
        None
    };

    let mut archive = ZipArchive::new(Cursor::new(zip_data))?;
    let mut files = BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let full_name = file.name().to_string();

        let rel_name = if let Some(prefix) = &root_prefix {
            full_name
                .strip_prefix(prefix)
                .unwrap_or(full_name.as_str())
                .to_string()
        } else {
            full_name
        };

        let rel_name = rel_name.trim_start_matches('/').to_string();
        if rel_name.is_empty() {
            continue;
        }

        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        files.insert(rel_name, content);
    }

    Ok(files)
}
