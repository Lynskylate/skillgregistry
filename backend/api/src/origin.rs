use std::collections::HashSet;

fn normalize_origin(value: &str) -> Option<String> {
    let normalized = value.trim().trim_end_matches("/");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub fn parse_frontend_origins(raw: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    let Some(raw) = raw else {
        return out;
    };

    for candidate in raw.split(",") {
        if let Some(origin) = normalize_origin(candidate) {
            if seen.insert(origin.clone()) {
                out.push(origin);
            }
        }
    }

    out
}

pub fn is_origin_allowed(allowed_origins: &[String], request_origin: &str) -> bool {
    let Some(origin) = normalize_origin(request_origin) else {
        return false;
    };

    allowed_origins.iter().any(|allowed| allowed == &origin)
}

#[cfg(test)]
mod tests {
    use super::{is_origin_allowed, parse_frontend_origins};

    #[test]
    fn parse_frontend_origins_supports_multiple_values() {
        let origins = parse_frontend_origins(Some("https://a.example, https://b.example"));
        assert_eq!(origins, vec!["https://a.example", "https://b.example"]);
    }

    #[test]
    fn parse_frontend_origins_deduplicates_and_trims() {
        let origins = parse_frontend_origins(Some(
            " https://a.example/ , https://a.example , , https://b.example/ ",
        ));
        assert_eq!(origins, vec!["https://a.example", "https://b.example"]);
    }

    #[test]
    fn is_origin_allowed_matches_normalized_value() {
        let allowed = parse_frontend_origins(Some("https://a.example,https://b.example"));
        assert!(is_origin_allowed(&allowed, "https://a.example/"));
        assert!(!is_origin_allowed(&allowed, "https://c.example"));
    }
}
