use serde_json::Value;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_boolish_handles_bool_values() {
        assert!(parse_boolish(Some(&json!(true))));
        assert!(!parse_boolish(Some(&json!(false))));
    }

    #[test]
    fn parse_boolish_handles_i64_numbers() {
        assert!(parse_boolish(Some(&json!(42))));
        assert!(parse_boolish(Some(&json!(-1))));
        assert!(!parse_boolish(Some(&json!(0))));
    }

    #[test]
    fn parse_boolish_returns_false_for_non_i64_numbers() {
        assert!(!parse_boolish(Some(&json!(3.14))));
        assert!(!parse_boolish(Some(&json!(9223372036854775808u64))));
    }

    #[test]
    fn parse_boolish_accepts_only_documented_truthy_strings() {
        assert!(parse_boolish(Some(&json!("true"))));
        assert!(parse_boolish(Some(&json!("1"))));
        assert!(parse_boolish(Some(&json!("yes"))));

        assert!(!parse_boolish(Some(&json!("TRUE"))));
        assert!(!parse_boolish(Some(&json!("false"))));
        assert!(!parse_boolish(Some(&json!("0"))));
        assert!(!parse_boolish(Some(&json!("no"))));
        assert!(!parse_boolish(Some(&json!(""))));
    }

    #[test]
    fn parse_boolish_returns_false_for_non_boolish_values() {
        assert!(!parse_boolish(Some(&Value::Null)));
        assert!(!parse_boolish(Some(&json!([]))));
        assert!(!parse_boolish(Some(&json!({"k": "v"}))));
        assert!(!parse_boolish(None));
    }

    #[test]
    fn json_string_returns_owned_string_for_string_values() {
        assert_eq!(
            json_string(Some(&json!("hello"))),
            Some("hello".to_string())
        );
        assert_eq!(json_string(Some(&json!(""))), Some(String::new()));
    }

    #[test]
    fn json_string_returns_none_for_non_strings_or_missing() {
        assert_eq!(json_string(Some(&json!(true))), None);
        assert_eq!(json_string(Some(&json!(42))), None);
        assert_eq!(json_string(Some(&json!(3.14))), None);
        assert_eq!(json_string(Some(&Value::Null)), None);
        assert_eq!(json_string(Some(&json!([]))), None);
        assert_eq!(json_string(Some(&json!({"a": 1}))), None);
        assert_eq!(json_string(None), None);
    }
}
