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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::FileOptions;

    fn create_zip(files: Vec<(&str, &[u8])>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for (path, content) in files {
                zip.start_file(path, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn strips_single_root_prefix_like_github_zipball() -> Result<()> {
        let zip = create_zip(vec![(
            "repo-sha/skill-a/SKILL.md",
            b"---\nname: a\ndescription: b\n---\n",
        )]);
        let m = zip_to_file_map(&zip)?;
        assert!(m.contains_key("skill-a/SKILL.md"));
        assert!(!m.contains_key("repo-sha/skill-a/SKILL.md"));
        Ok(())
    }

    #[test]
    fn does_not_strip_when_multiple_roots_present() -> Result<()> {
        let zip = create_zip(vec![("a/file.txt", b"x"), ("b/file.txt", b"y")]);
        let m = zip_to_file_map(&zip)?;
        assert!(m.contains_key("a/file.txt"));
        assert!(m.contains_key("b/file.txt"));
        Ok(())
    }
}
