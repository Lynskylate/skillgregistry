use anyhow::Result;
use common::domain::{archive, json, markdown, skill};
use serde_json::Value;
use std::collections::BTreeMap;

pub use common::domain::markdown::ParsedMarkdown;
pub use common::domain::skill::SkillFrontmatter;

pub fn verify_skill(expected_name: &str, frontmatter_str: &str) -> Result<SkillFrontmatter> {
    skill::verify_skill(expected_name, frontmatter_str)
}

pub fn package_skill(file_map: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    archive::package_zip(file_map)
}

pub fn parse_markdown_frontmatter(input: &str) -> Result<ParsedMarkdown> {
    markdown::parse_markdown_frontmatter(input)
}

pub fn normalize_dir_prefix(prefix: &str) -> String {
    archive::normalize_dir_prefix(prefix)
}

pub fn subtree_file_map(
    all_files: &BTreeMap<String, Vec<u8>>,
    dir_prefix: &str,
) -> BTreeMap<String, Vec<u8>> {
    archive::subtree_file_map(all_files, dir_prefix)
}

pub fn compute_hash(file_map: &BTreeMap<String, Vec<u8>>) -> String {
    archive::compute_hash(file_map)
}

pub fn parse_boolish(value: Option<&Value>) -> bool {
    json::parse_boolish(value)
}

pub fn json_string(value: Option<&Value>) -> Option<String> {
    json::json_string(value)
}

pub fn split_raw_frontmatter(input: &str) -> Result<(String, String)> {
    markdown::split_raw_frontmatter(input)
}
