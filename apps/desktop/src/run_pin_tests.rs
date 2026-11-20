use devpit_rpc::EnvVar;

use super::*;

fn var(name: &str, value: &str) -> EnvVar {
    EnvVar {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn profile(env: Vec<EnvVar>) -> Declared {
    Declared {
        id: "glm".to_owned(),
        label: "GLM".to_owned(),
        base: "claude".to_owned(),
        command: String::new(),
        args: Vec::new(),
        env,
        models: Vec::new(),
    }
}

#[test]
fn the_pin_names_the_account_gateway_and_model() {
    let declared = profile(vec![
        var("CLAUDE_CONFIG_DIR", "/home/aron/.claude-2"),
        var(
            "ANTHROPIC_BASE_URL",
            "https://user:pw@api.z.ai/api/anthropic",
        ),
        var("ANTHROPIC_MODEL", "glm-4.6"),
    ]);
    let pinned = pin(&declared, None, "/home/aron");
    assert_eq!(pinned.id, "glm");
    assert_eq!(pinned.label, "GLM");
    assert_eq!(pinned.agent, "claude");
    assert_eq!(pinned.config_dir.as_deref(), Some("~/.claude-2"));
    assert_eq!(
        pinned.base_url.as_deref(),
        Some("https://api.z.ai/api/anthropic")
    );
    assert_eq!(pinned.requested_model.as_deref(), Some("glm-4.6"));
}

#[test]
fn the_step_model_wins_then_the_profile_then_none() {
    let with = profile(vec![var("ANTHROPIC_MODEL", "glm-4.6")]);
    assert_eq!(
        pin(&with, Some("sonnet"), "/h").requested_model.as_deref(),
        Some("sonnet")
    );
    assert_eq!(
        pin(&with, None, "/h").requested_model.as_deref(),
        Some("glm-4.6")
    );
    assert_eq!(pin(&profile(Vec::new()), None, "/h").requested_model, None);
}

#[test]
fn a_secret_valued_variable_never_reaches_the_json() {
    let fake = "sk-fake-0123456789";
    let declared = profile(vec![
        var("ANTHROPIC_API_KEY", fake),
        var("ANTHROPIC_AUTH_TOKEN", fake),
        var("GH_TOKEN", fake),
        // Even named like a model or a gateway, a secret name is refused.
        var("SECRET_MODEL_TOKEN", fake),
        var(
            "ANTHROPIC_BASE_URL",
            &format!("https://u:{fake}@gw.example/v1"),
        ),
    ]);
    let json = serde_json::to_string(&pin(&declared, None, "/home/aron")).expect("json");
    assert!(!json.contains(fake), "{json}");
    assert!(json.contains("https://gw.example/v1"), "{json}");
}

#[test]
fn a_variable_the_sync_keeps_home_is_not_read() {
    // The same filter as the sync: a name it does not share is not pinned.
    let declared = profile(vec![var("MY_BASE", "x"), var("OTHER_MODELISH", "y")]);
    let pinned = pin(&declared, None, "/h");
    assert_eq!(pinned.requested_model, None);
    assert_eq!(pinned.base_url, None);
}

#[test]
fn a_gateway_variable_the_sync_refuses_is_skipped() {
    // Shaped like a gateway, but named like a secret: the sync's filter says no.
    let declared = profile(vec![
        var("AUTH_BASE_URL", "https://leak.example"),
        var("ANTHROPIC_BASE_URL", "https://gw.example"),
    ]);
    let pinned = pin(&declared, None, "/h");
    assert_eq!(pinned.base_url.as_deref(), Some("https://gw.example"));
}
