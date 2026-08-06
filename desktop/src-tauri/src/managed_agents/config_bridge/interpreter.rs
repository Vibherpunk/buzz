use super::types::{ExtensionEntry, RuntimeFileConfig};

/// Read Open Interpreter config from `~/.openinterpreter/config.toml`
/// (or `$CODEX_HOME/config.toml`, which Open Interpreter — a Codex fork —
/// resolves to `~/.openinterpreter` at runtime).
pub(super) fn read_config_file() -> Option<RuntimeFileConfig> {
    let path = interpreter_config_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    parse_interpreter_config(&raw)
}

fn parse_interpreter_config(toml_str: &str) -> Option<RuntimeFileConfig> {
    let table: toml::Table = toml_str.parse().ok()?;

    let model = toml_string(&table, "model");
    let model_provider = toml_string(&table, "model_provider");
    let approval_policy = toml_string(&table, "approval_policy");
    let sandbox_mode = toml_string(&table, "sandbox_mode");
    let reasoning_effort = toml_string(&table, "model_reasoning_effort");
    let context_window = toml_string(&table, "model_context_window");

    // Two-axis mode: approval_policy × sandbox_mode
    let mode = match (approval_policy.as_deref(), sandbox_mode.as_deref()) {
        (Some(ap), Some(sm)) => Some(format!("{ap}/{sm}")),
        (Some(ap), None) => Some(ap.to_string()),
        (None, Some(sm)) => Some(format!("default/{sm}")),
        (None, None) => None,
    };

    // MCP servers from [mcp_servers.<id>] tables
    let extensions = parse_mcp_servers(&table);

    // Config-driven extra fields — skip normalized keys to avoid double-counting.
    let config_json = toml_to_json(&toml::Value::Table(table));
    let skip = &[
        "model",
        "model_provider",
        "approval_policy",
        "sandbox_mode",
        "model_reasoning_effort",
        "model_context_window",
        "instructions",
        "mcp_servers",
        "model_providers",
    ];
    let mut extra = super::schema_walker::extract_config_fields(&config_json, skip);

    // Custom model providers from [model_providers.<id>] — surface as
    // "model_providers.<name> = configured" rather than flattening their internals.
    if let Some(serde_json::Value::Object(providers)) = config_json.get("model_providers") {
        for (name, _) in providers {
            extra.insert(format!("model_providers.{name}"), "configured".to_string());
        }
    }

    Some(RuntimeFileConfig {
        model,
        // Default to OpenAI when no provider is configured — that is Open
        // Interpreter's (Codex-derived) implicit provider.
        provider: model_provider.or_else(|| Some("openai".to_string())),
        mode,
        thinking_effort: reasoning_effort,
        max_output_tokens: None,
        context_limit: context_window,
        system_prompt: toml_to_json_string(&config_json, "instructions"),
        extensions,
        extra,
    })
}

fn parse_mcp_servers(table: &toml::Table) -> Vec<ExtensionEntry> {
    let servers = match table.get("mcp_servers").and_then(|v| v.as_table()) {
        Some(s) => s,
        None => return Vec::new(),
    };

    servers
        .iter()
        .map(|(name, _config)| ExtensionEntry {
            name: name.clone(),
            kind: "mcp".to_string(),
            enabled: true,
        })
        .collect()
}

/// Convert a TOML value to a serde_json Value.
fn toml_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map = tbl
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Extract a string value from a JSON object by key (mirrors toml_string).
fn toml_to_json_string(val: &serde_json::Value, key: &str) -> Option<String> {
    val.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn toml_string(table: &toml::Table, key: &str) -> Option<String> {
    table
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn interpreter_config_path() -> Option<std::path::PathBuf> {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        return Some(std::path::PathBuf::from(home).join("config.toml"));
    }
    let home = dirs::home_dir()?;
    Some(home.join(".openinterpreter").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_config() {
        let toml = r#"
model = "ollama/deepseek-v3"
model_provider = "ollama"
approval_policy = "unless-allow-listed"
sandbox_mode = "read-only"
"#;
        let cfg = parse_interpreter_config(toml).unwrap();
        assert_eq!(cfg.model.as_deref(), Some("ollama/deepseek-v3"));
        assert_eq!(cfg.provider.as_deref(), Some("ollama"));
        assert_eq!(cfg.mode.as_deref(), Some("unless-allow-listed/read-only"));
    }

    #[test]
    fn parses_custom_providers() {
        let toml = r#"
model = "my-model"
model_provider = "openrouter"

[model_providers.openrouter]
base_url = "https://openrouter.ai/api/v1"
"#;
        let cfg = parse_interpreter_config(toml).unwrap();
        assert_eq!(cfg.provider.as_deref(), Some("openrouter"));
        assert!(cfg.extra.contains_key("model_providers.openrouter"));
    }

    #[test]
    fn default_provider_is_openai() {
        let cfg = parse_interpreter_config("approval_policy = \"on-failure\"").unwrap();
        assert_eq!(cfg.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn path_respects_codex_home() {
        let orig = std::env::var_os("CODEX_HOME");
        std::env::set_var("CODEX_HOME", "/tmp/oi-home");
        let p = interpreter_config_path().unwrap();
        assert_eq!(p.to_string_lossy(), "/tmp/oi-home/config.toml");
        match orig {
            Some(v) => std::env::set_var("CODEX_HOME", v),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }
}
