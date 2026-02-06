use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_skill_rejects_unexpected_key() {
        let fm = "name: ok\ndescription: d\nunexpected: x\n";
        let err = verify_skill("ignored", fm).unwrap_err();
        assert!(err.to_string().contains("Unexpected key"));
    }

    #[test]
    fn verify_skill_rejects_invalid_name_characters() {
        let fm = "name: Bad_Name\ndescription: d\n";
        let err = verify_skill("ignored", fm).unwrap_err();
        assert!(err.to_string().contains("lowercase"));
    }

    #[test]
    fn verify_skill_accepts_valid_minimal_frontmatter() -> Result<()> {
        let fm = "name: good-name\ndescription: test\n";
        let ok = verify_skill("ignored", fm)?;
        assert_eq!(ok.name, "good-name");
        Ok(())
    }
}
