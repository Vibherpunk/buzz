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

/// Ensure a channel is scoped, returning the room backing it. Called before an
/// add so an unmapped channel becomes usable in one click — Harbor creates the
/// room and records the mapping. The room name is an internal detail; the GUI
/// only ever deals in channels.
async fn ensure_channel_room(channel: &str) -> Result<String, String> {
    let out = run_harbor(vec![
        "channel-tools".into(),
        channel.to_string(),
        "--map".into(),
        "--json".into(),
    ])
    .await?;
    let value = json_output(out)?;
    value
        .get("room")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "could not scope this channel".to_string())
}

/// Resolve the room backing an already-scoped channel WITHOUT creating one
/// (read-only — used by the remove path, where scoping on the fly would be
/// wrong). Errors if the channel isn't scoped.
async fn channel_room(channel: &str) -> Result<String, String> {
    let out = run_harbor(vec![
        "channel-tools".into(),
        channel.to_string(),
        "--json".into(),
    ])
    .await?;
    let value = json_output(out)?;
    value
        .get("room")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "this channel has no tools to remove".to_string())
}

/// Remove a skill from this channel (unregister it from the channel's room).
#[tauri::command]
pub async fn channel_tools_remove_skill(channel: String, skill: String) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let skill = safe_arg("skill", &skill)?;
    let room = channel_room(&channel).await?;
    let out = run_harbor(vec![
        "skill-remove".into(),
        "--name".into(),
        skill,
        "--room".into(),
        room,
        "--yes".into(),
    ])
    .await?;
    text_output(out)
}

/// Remove an MCP server from this channel.
#[tauri::command]
pub async fn channel_tools_remove_mcp(channel: String, name: String) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let name = safe_arg("name", &name)?;
    let room = channel_room(&channel).await?;
    let out = run_harbor(vec![
        "mcp-remove".into(),
        "--room".into(),
        room,
        "--name".into(),
        name,
    ])
    .await?;
    text_output(out)
}

/// Add an existing skill (from elsewhere in Buzz) to this channel. Scopes the
/// channel on the fly if it isn't already.
#[tauri::command]
pub async fn channel_tools_add_existing_skill(
    channel: String,
    skill: String,
) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let skill = safe_arg("skill", &skill)?;
    let room = ensure_channel_room(&channel).await?;
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

/// Install a brand-new skill (from a directory or SKILL.md path) and make it
/// available in this channel first. Scopes the channel on the fly if needed.
#[tauri::command]
pub async fn channel_tools_add_new_skill(
    channel: String,
    source: String,
    name: Option<String>,
) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let source = safe_arg("source", &source)?;
    let room = ensure_channel_room(&channel).await?;
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

/// Resolve the persona a channel runs under: its room's persona (auto-derived,
/// like skills/MCP) or an explicit override. Shape mirrors `harbor
/// channel-persona --json`: `{ channel, room, effective, roomOptions[],
/// ambiguous, overridden }`.
#[tauri::command]
pub async fn channel_tools_get_persona(channel: String) -> Result<Value, String> {
    let channel = safe_arg("channel", &channel)?;
    let out = run_harbor(vec!["channel-persona".into(), channel, "--json".into()]).await?;
    json_output(out)
}

/// Auto-apply a channel's room persona when unambiguous (points `persona_file`
/// at the live room file). No-op with a reason when the channel has an override
/// or the room is ambiguous. Returns `{ channel, synced, path?, reason? }`.
#[tauri::command]
pub async fn channel_tools_sync_persona(channel: String) -> Result<Value, String> {
    let channel = safe_arg("channel", &channel)?;
    let out = run_harbor(vec![
        "channel-persona".into(),
        channel,
        "--sync".into(),
        "--json".into(),
    ])
    .await?;
    json_output(out)
}

/// Override a channel's persona with a specific file (typically one of the
/// room's persona files chosen from the picker).
#[tauri::command]
pub async fn channel_tools_set_persona_file(
    channel: String,
    file: String,
) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let file = file.trim();
    if file.is_empty() {
        return Err("file must not be empty".into());
    }
    // `--flag=value` (single argv) so a value is never mis-read as a flag.
    let out = run_harbor(vec![
        "channel-persona".into(),
        channel,
        format!("--set-file={file}"),
    ])
    .await?;
    text_output(out)
}

/// Override a channel's persona with custom text (stored under
/// `~/.buzz/personas/` and referenced by `persona_file`).
#[tauri::command]
pub async fn channel_tools_set_persona_inline(
    channel: String,
    text: String,
) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    if text.trim().is_empty() {
        return Err("persona text must not be empty".into());
    }
    let out = run_harbor(vec![
        "channel-persona".into(),
        channel,
        format!("--set-inline={text}"),
    ])
    .await?;
    text_output(out)
}

/// Edit the canonical room persona file (its full text). The change applies to
/// every channel that uses this persona — this edits the source of truth, not a
/// channel-specific copy.
#[tauri::command]
pub async fn channel_tools_edit_room_persona(
    room: String,
    name: String,
    text: String,
) -> Result<String, String> {
    let room = safe_arg("room", &room)?;
    let name = safe_arg("name", &name)?;
    if text.trim().is_empty() {
        return Err("persona text must not be empty".into());
    }
    let out = run_harbor(vec![
        "room-persona".into(),
        room,
        name,
        format!("--set-body={text}"),
    ])
    .await?;
    text_output(out)
}

/// Remove a channel's persona override, reverting to the room-derived persona
/// (or none).
#[tauri::command]
pub async fn channel_tools_remove_persona(channel: String) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let out = run_harbor(vec!["channel-persona".into(), channel, "--remove".into()]).await?;
    text_output(out)
}

/// Add an MCP server to this channel. Scopes the channel on the fly if needed.
#[tauri::command]
pub async fn channel_tools_add_mcp(
    channel: String,
    name: String,
    command: String,
    args: Option<String>,
) -> Result<String, String> {
    let channel = safe_arg("channel", &channel)?;
    let name = safe_arg("name", &name)?;
    let command = safe_arg("command", &command)?;
    let room = ensure_channel_room(&channel).await?;
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
