//! Channel-scoped tools GUI bridge.
//!
//! Harbor is the source of truth for which skills + MCP servers a channel
//! exposes: buzz-acp maps a channel to a Harbor *room* via
//! `~/.buzz/channel-tools.toml`, and the room's skill/MCP allowlist is enforced
//! server-side by Harbor's gate. These commands are the GUI's window onto that
//! model — they shell out to the same `harbor` binary buzz-acp uses, so the
//! panel, the CLI, and enforcement all read and write ONE policy rather than
//! three divergent notions of "what's in this channel".
//!
//! Read commands (`*_get`, `*_pool_skills`) return `harbor ... --json` verbatim
//! for the frontend to render. Mutations (`*_add_*`) wrap Harbor's own audited
//! verbs (`skill-install` / `skill-room-add` / `mcp-add --room`); Harbor applies
//! its confirm policy and token budget, so this layer never bypasses the gate.
//!
//! The desktop Tauri crate does not enable tokio's `process` feature, so every
//! subprocess runs inside `spawn_blocking` over synchronous `std::process`,
//! mirroring `agent_model_process.rs`. Values are passed as distinct argv
//! entries (never a shell string), and any value that could be reinterpreted as
//! a flag is rejected up front.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use crate::managed_agents::login_shell_path;

/// The channel-tools policy file buzz-acp reads (env override mirrors
/// buzz-acp's `BUZZ_ACP_CHANNEL_TOOLS`, else `~/.buzz/channel-tools.toml`).
fn policy_path() -> PathBuf {
    if let Ok(p) = std::env::var("BUZZ_ACP_CHANNEL_TOOLS") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let mut base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.push(".buzz");
    base.push("channel-tools.toml");
    base
}

/// Resolve the harbor binary the same way buzz-acp does: the policy's
/// top-level `harbor_command`, else `harbor` on PATH.
fn harbor_binary(policy: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(policy) {
        if let Ok(toml::Value::Table(table)) = text.parse::<toml::Value>() {
            if let Some(toml::Value::String(cmd)) = table.get("harbor_command") {
                if !cmd.trim().is_empty() {
                    return cmd.clone();
                }
            }
        }
    }
    "harbor".to_string()
}

/// Reject a value that is empty or could be reinterpreted as a CLI flag. Values
/// reach harbor as separate argv entries (no shell), so the only injection
/// vector is a leading `-` being parsed as an option — guard exactly that.
fn safe_arg(label: &str, value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if v.starts_with('-') {
        return Err(format!("{label} must not start with '-'"));
    }
    Ok(v.to_string())
}

/// Run `harbor <args>` with the login-shell PATH so the binary resolves, off
/// the async runtime (the desktop crate lacks tokio's `process` feature).
async fn run_harbor(args: Vec<String>) -> Result<Output, String> {
    tokio::task::spawn_blocking(move || {
        let policy = policy_path();
        let binary = harbor_binary(&policy);
        let mut cmd = Command::new(&binary);
        if let Some(path) = login_shell_path() {
            cmd.env("PATH", path);
        }
        cmd.args(&args);
        cmd.output()
            .map_err(|e| format!("failed to run '{binary}': {e}"))
    })
    .await
    .map_err(|e| format!("harbor task failed: {e}"))?
}

/// Parse a read command's stdout as JSON, surfacing stderr on failure.
fn json_output(out: Output) -> Result<Value, String> {
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("harbor {}: {}", out.status, stderr.trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| {
        let preview = String::from_utf8_lossy(&out.stdout);
        format!(
            "harbor returned invalid JSON: {e} (got {} bytes)",
            preview.len()
        )
    })
}

/// Return a mutation's combined output, treating a non-zero exit as an error.
fn text_output(out: Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
        return Err(combined.trim().to_string());
    }
    Ok(stdout.trim().to_string())
}

/// Resolve one channel to the skills + MCP servers it exposes (its room's
/// allowlist). Shape: `{ channel, room, scoped, skills[], mcpServers[] }`.
#[tauri::command]
pub async fn channel_tools_get(channel: String) -> Result<Value, String> {
    let channel = safe_arg("channel", &channel)?;
    let out = run_harbor(vec!["channel-tools".into(), channel, "--json".into()]).await?;
    json_output(out)
}

/// Every skill in the Harbor pool (name/room/description), for the "add a skill
/// from elsewhere in Buzz" picker.
#[tauri::command]
pub async fn channel_tools_pool_skills() -> Result<Value, String> {
    let out = run_harbor(vec!["skills-list".into(), "--json".into()]).await?;
    json_output(out)
}

/// Grant an existing pool skill to this channel's room (additive).
#[tauri::command]
pub async fn channel_tools_add_existing_skill(
    room: String,
    skill: String,
) -> Result<String, String> {
    let room = safe_arg("room", &room)?;
    let skill = safe_arg("skill", &skill)?;
    let out = run_harbor(vec![
        "skill-room-add".into(),
        "--skill".into(),
        skill,
        "--room".into(),
        room,
    ])
    .await?;
    text_output(out)
}

/// Install a brand-new skill (from a directory or SKILL.md path) into the pool
/// and route it to this channel's room, so it is available here first.
#[tauri::command]
pub async fn channel_tools_add_new_skill(
    room: String,
    source: String,
    name: Option<String>,
) -> Result<String, String> {
    let room = safe_arg("room", &room)?;
    let source = safe_arg("source", &source)?;
    let mut args = vec![
        "skill-install".into(),
        "--source".into(),
        source,
        "--room".into(),
        room,
    ];
    if let Some(name) = name {
        if !name.trim().is_empty() {
            args.push("--name".into());
            args.push(safe_arg("name", &name)?);
        }
    }
    let out = run_harbor(args).await?;
    text_output(out)
}

/// Add an MCP server to this channel's room.
#[tauri::command]
pub async fn channel_tools_add_mcp(
    room: String,
    name: String,
    command: String,
    args: Option<String>,
) -> Result<String, String> {
    let room = safe_arg("room", &room)?;
    let name = safe_arg("name", &name)?;
    let command = safe_arg("command", &command)?;
    let mut argv = vec![
        "mcp-add".into(),
        "--room".into(),
        room,
        "--name".into(),
        name,
        "--command".into(),
        command,
    ];
    if let Some(extra) = args {
        if !extra.trim().is_empty() {
            argv.push("--args".into());
            argv.push(extra.trim().to_string());
        }
    }
    let out = run_harbor(argv).await?;
    text_output(out)
}
