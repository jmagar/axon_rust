use super::super::*;

#[test]
fn dotenv_parser_ignores_comments_blank_and_malformed_lines() {
    let parsed = parse_dotenv_content(
        r#"
        # comment
        NO_EQUALS
        =missing_key
        FOO=bar

        BAZ = qux
        "#,
    );
    assert_eq!(parsed.get("FOO").map(String::as_str), Some("bar"));
    assert_eq!(parsed.get("BAZ").map(String::as_str), Some("qux"));
    assert!(!parsed.contains_key("NO_EQUALS"));
}

#[test]
fn dotenv_parser_unquotes_single_and_double_quoted_values() {
    let parsed = parse_dotenv_content(
        r#"
        A="value one"
        B='value two'
        C=plain
        "#,
    );
    assert_eq!(parsed.get("A").map(String::as_str), Some("value one"));
    assert_eq!(parsed.get("B").map(String::as_str), Some("value two"));
    assert_eq!(parsed.get("C").map(String::as_str), Some("plain"));
}

#[test]
fn dotenv_parser_keeps_inner_equals_and_last_value_wins() {
    let parsed = parse_dotenv_content(
        r#"
        AXON_PG_URL=postgresql://u:p@localhost:5432/db?sslmode=disable
        FOO=first
        FOO=second
        "#,
    );
    assert_eq!(
        parsed.get("AXON_PG_URL").map(String::as_str),
        Some("postgresql://u:p@localhost:5432/db?sslmode=disable")
    );
    assert_eq!(parsed.get("FOO").map(String::as_str), Some("second"));
}
