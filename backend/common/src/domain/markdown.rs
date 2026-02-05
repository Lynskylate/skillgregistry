use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub metadata: Option<Value>,
    pub body: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_raw_frontmatter_requires_opening_delimiter() {
        let err = split_raw_frontmatter("name: x\n---\nbody").unwrap_err();
        assert!(err.to_string().contains("Missing frontmatter"));
    }

    #[test]
    fn parse_markdown_frontmatter_returns_none_when_missing() -> Result<()> {
        let parsed = parse_markdown_frontmatter("# Title")?;
        assert!(parsed.metadata.is_none());
        assert_eq!(parsed.body, "# Title");
        Ok(())
    }

    #[test]
    fn parse_markdown_frontmatter_parses_yaml_when_present() -> Result<()> {
        let parsed = parse_markdown_frontmatter("---\nname: a\n---\n# Body")?;
        assert_eq!(
            parsed
                .metadata
                .as_ref()
                .and_then(|m| m.get("name"))
                .and_then(|v| v.as_str()),
            Some("a")
        );
        assert_eq!(parsed.body, "# Body");
        Ok(())
    }
}
