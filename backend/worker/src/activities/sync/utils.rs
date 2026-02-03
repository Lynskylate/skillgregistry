use super::domain::{ParsedMarkdown, SkillFrontmatter};
use anyhow::Result;
use md5;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;

pub fn verify_skill(_expected_name: &str, frontmatter_str: &str) -> Result<SkillFrontmatter> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML: {}", e))?;

    let obj = yaml_value
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("Frontmatter is not a YAML mapping"))?;

    let allowed_keys = [
        "name",
        "description",
        "license",
        "compatibility",
        "allowed-tools",
        "metadata",
    ];
    for key in obj.keys() {
        if let Some(k) = key.as_str() {
            if !allowed_keys.contains(&k) {
                return Err(anyhow::anyhow!("Unexpected key in frontmatter: {}", k));
            }
        }
    }

    let frontmatter: SkillFrontmatter = serde_yaml::from_value(yaml_value)?;

    let n = &frontmatter.name;
    if n.is_empty() || n.len() > 64 {
        return Err(anyhow::anyhow!("Skill name must be 1-64 characters"));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow::anyhow!(
            "Skill name must only contain lowercase letters, numbers, and hyphens"
        ));
    }
    if n.starts_with('-') || n.ends_with('-') {
        return Err(anyhow::anyhow!(
            "Skill name must not start or end with a hyphen"
        ));
    }
    if n.contains("--") {
        return Err(anyhow::anyhow!(
            "Skill name must not contain consecutive hyphens"
        ));
    }

    if frontmatter.description.is_empty() || frontmatter.description.len() > 1024 {
        return Err(anyhow::anyhow!("Description must be 1-1024 characters"));
    }

    Ok(frontmatter)
}

pub fn package_skill(file_map: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
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

pub fn parse_markdown_frontmatter(input: &str) -> Result<ParsedMarkdown> {
    let mut lines = input.lines();
    let first = lines.next().unwrap_or_default();
    if first.trim_end() != "---" {
        return Ok(ParsedMarkdown {
            metadata: None,
            body: input.to_string(),
        });
    }

    let mut fm_lines = Vec::new();
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            break;
        }
        fm_lines.push(line);
    }
    let frontmatter_str = fm_lines.join("\n");
    let body = lines.collect::<Vec<_>>().join("\n");

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&frontmatter_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML frontmatter: {}", e))?;
    let metadata = serde_json::to_value(yaml_value)?;

    Ok(ParsedMarkdown {
        metadata: Some(metadata),
        body,
    })
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

pub fn parse_boolish(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => matches!(s.as_str(), "true" | "1" | "yes"),
        _ => false,
    }
}

pub fn json_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|v| v.as_str()).map(|s| s.to_string())
}

pub fn split_raw_frontmatter(input: &str) -> Result<(String, String)> {
    let mut lines = input.lines();
    let first = lines.next().unwrap_or_default();
    if first.trim_end() != "---" {
        return Err(anyhow::anyhow!("Missing frontmatter"));
    }
    let mut fm_lines = Vec::new();
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            break;
        }
        fm_lines.push(line);
    }
    let fm = fm_lines.join("\n");
    let body = lines.collect::<Vec<_>>().join("\n");
    Ok((fm, body))
}
