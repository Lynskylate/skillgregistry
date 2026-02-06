use anyhow::Result;
use md5;
use std::collections::BTreeMap;
use std::io::Write;

pub fn package_zip(file_map: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut new_zip_buffer = Vec::new();
    {
        let mut zip_writer = zip::ZipWriter::new(std::io::Cursor::new(&mut new_zip_buffer));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for (path, content) in file_map {
            zip_writer.start_file(path, options)?;
            zip_writer.write_all(content)?;
        }
        zip_writer.finish()?;
    }
    Ok(new_zip_buffer)
}

pub fn normalize_dir_prefix(prefix: &str) -> String {
    let mut p = prefix
        .trim()
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if p.is_empty() {
        return p;
    }
    if !p.ends_with('/') {
        p.push('/');
    }
    p
}

pub fn subtree_file_map(
    all_files: &BTreeMap<String, Vec<u8>>,
    dir_prefix: &str,
) -> BTreeMap<String, Vec<u8>> {
    let prefix = normalize_dir_prefix(dir_prefix);
    let mut out = BTreeMap::new();
    for (path, bytes) in all_files {
        if prefix.is_empty() {
            out.insert(path.clone(), bytes.clone());
            continue;
        }
        if path.starts_with(&prefix) {
            let rel = path
                .trim_start_matches(&prefix)
                .trim_start_matches('/')
                .to_string();
            if !rel.is_empty() {
                out.insert(rel, bytes.clone());
            }
        }
    }
    out
}

pub fn compute_hash(file_map: &BTreeMap<String, Vec<u8>>) -> String {
    let mut context = md5::Context::new();
    for (path, content) in file_map {
        context.consume(path.as_bytes());
        context.consume(content);
    }
    format!("{:x}", context.compute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn compute_hash_changes_when_content_changes() {
        let mut a = BTreeMap::new();
        a.insert("a.txt".to_string(), b"one".to_vec());
        let h1 = compute_hash(&a);
        a.insert("a.txt".to_string(), b"two".to_vec());
        let h2 = compute_hash(&a);
        assert_ne!(h1, h2);
    }

    #[test]
    fn package_zip_contains_expected_files() -> Result<()> {
        let mut m = BTreeMap::new();
        m.insert("dir/a.txt".to_string(), b"hello".to_vec());
        m.insert("b.txt".to_string(), b"world".to_vec());
        let zip_bytes = package_zip(&m)?;

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))?;
        assert_eq!(archive.len(), 2);

        let mut file = archive.by_name("dir/a.txt")?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        assert_eq!(buf, "hello");

        Ok(())
    }
}
