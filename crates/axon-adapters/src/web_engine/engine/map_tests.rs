use super::*;

#[test]
fn redirected_directory_seed_keeps_the_effective_trailing_slash() {
    assert_eq!(
        derive_map_scope_url(
            "https://dandavison.github.io/delta",
            "https://dandavison.github.io/delta/",
        )
        .as_deref(),
        Some("https://dandavison.github.io/delta/"),
    );
}
