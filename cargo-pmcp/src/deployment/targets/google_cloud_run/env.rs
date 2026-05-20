//! Cloud Run environment-variable rendering helpers.
//!
//! Splits out the shared utility from `deploy.rs` so both `deploy.rs`
//! (gcloud invocation) and `dockerfile.rs` (cloudbuild.yaml emission)
//! can call it without crossing into the deploy module's API surface.

use std::collections::HashMap;

/// Render an `[environment]` table as a `gcloud run deploy --set-env-vars`
/// argument value.
///
/// The output is sorted by key to keep the value deterministic across runs
/// (important so re-running deploy with no schema change does not produce
/// a different Cloud Run revision purely because HashMap iteration order
/// shifted). Values are passed verbatim — gcloud accepts `KEY=VAL` where
/// VAL may contain spaces; commas inside values are not supported by
/// gcloud's `--set-env-vars` flag, and we surface the same limitation.
pub(super) fn render_set_env_vars(env: &HashMap<String, String>) -> String {
    if env.is_empty() {
        return String::new();
    }
    let mut entries: Vec<(&String, &String)> = env.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_set_env_vars_is_deterministic() {
        let mut env = HashMap::new();
        env.insert("ZEBRA".to_string(), "z".to_string());
        env.insert("ALPHA".to_string(), "a".to_string());
        env.insert("MIKE".to_string(), "m".to_string());
        let rendered = render_set_env_vars(&env);
        assert_eq!(rendered, "ALPHA=a,MIKE=m,ZEBRA=z");
    }

    #[test]
    fn render_set_env_vars_empty_is_empty_string() {
        let env = HashMap::new();
        assert_eq!(render_set_env_vars(&env), "");
    }

    #[test]
    fn render_set_env_vars_handles_single_entry() {
        let mut env = HashMap::new();
        env.insert(
            "EXPECTED_AUDIENCE".to_string(),
            "abc.apps.googleusercontent.com".to_string(),
        );
        assert_eq!(
            render_set_env_vars(&env),
            "EXPECTED_AUDIENCE=abc.apps.googleusercontent.com"
        );
    }
}
