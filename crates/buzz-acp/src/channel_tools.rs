//! Channel-scoped tool policy: map channels to the MCP servers (tools/skills)
//! an agent may use when responding in that channel.
//!
//! Loaded from the TOML file named by `--channel-tools` / `BUZZ_ACP_CHANNEL_TOOLS`.
//! Keys are channel UUIDs or channel names (names match case-insensitively);
//! values declare either a harbor `room` (shorthand for
//! `harbor mcp-server --room=<room>`) or an explicit `mcp` server list.
//!
//! Scoping is strict, not additive: when a channel has an entry, its server
//! list *replaces* the global `--mcp-command` default for sessions created on
//! that channel — and because the agent runtime skips its own configured
//! extensions whenever explicit MCP servers are passed, a scoped channel
//! exposes exactly the listed servers and nothing else. Channels without an
//! entry keep today's behavior.
//!
//! ```toml
//! # ~/.buzz/channel-tools.toml
//! harbor_command = "harbor"                        # default for `room` entries
//!
//! [channels.bookkeeping]          # channel name…
//! room = "bookkeeping"
//!
//! [channels."6f8199d3-479f-4dce-8851-000a4b0a2586"]   # …or channel UUID
//! room = "bookkeeping"
//!
//! [channels.devops]
//! room = "devops"
//! [[channels.devops.mcp]]         # extra explicit servers are additive to `room`
//! name = "system-monitoring"
//! command = "/usr/local/bin/sysmon-mcp"
//! args = ["--stdio"]
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::acp::{EnvVar, McpServer};

/// One channel's policy as written in TOML: which tools it exposes and,
/// optionally, which persona (system prompt) sessions in it run under.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelToolsEntry {
    /// Harbor room shorthand: expands to `<harbor_command> mcp-server --room=<room>`.
    #[serde(default)]
    room: Option<String>,
    /// Per-entry harbor binary override.
    #[serde(default)]
    harbor_command: Option<String>,
    /// Explicit MCP servers for this channel (additive with `room`).
    #[serde(default)]
    mcp: Vec<McpToml>,
    /// Inline system-prompt override for sessions created in this channel.
    /// REPLACES the agent's base `system_prompt` — never layered or merged.
    /// Mutually exclusive with `persona_file`.
    #[serde(default)]
    persona: Option<String>,
    /// Load the persona from a file instead (for long prompts). Mutually
    /// exclusive with `persona`. `~` is expanded to `$HOME`.
    #[serde(default)]
    persona_file: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct McpToml {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelToolsFile {
    /// Default harbor binary for `room` entries. Defaults to `"harbor"` (PATH).
    #[serde(default)]
    harbor_command: Option<String>,
    #[serde(default)]
    channels: HashMap<String, ChannelToolsEntry>,
}

/// Resolved channel policy, keyed by UUID and by lowercased name. Tools and
/// personas are stored separately: a channel may scope tools, override the
/// persona, both, or (for personas) neither — resolution of each is independent.
#[derive(Debug, Default)]
pub struct ChannelTools {
    by_uuid: HashMap<Uuid, Vec<McpServer>>,
    by_name: HashMap<String, Vec<McpServer>>,
    persona_by_uuid: HashMap<Uuid, String>,
    persona_by_name: HashMap<String, String>,
    /// Total number of channel entries (tool- and/or persona-bearing).
    entry_count: usize,
}

impl ChannelTools {
    /// Load and validate a policy file. Errors are fatal by design: a tool
    /// policy that fails to parse must never silently degrade to "no policy".
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read channel-tools file {}: {e}", path.display()))?;
        let file: ChannelToolsFile = toml::from_str(&content)
            .map_err(|e| format!("invalid channel-tools file {}: {e}", path.display()))?;

        let default_harbor = file.harbor_command.as_deref().unwrap_or("harbor");
        let mut tools = ChannelTools::default();

        for (key, entry) in &file.channels {
            entry.validate(key)?;
            let servers = entry.to_servers(default_harbor);
            let persona = entry.read_persona(key)?;
            match key.parse::<Uuid>() {
                Ok(uuid) => {
                    if !servers.is_empty() {
                        tools.by_uuid.insert(uuid, servers);
                    }
                    if let Some(p) = persona {
                        tools.persona_by_uuid.insert(uuid, p);
                    }
                }
                Err(_) => {
                    let name_key = key.trim_start_matches('#').to_ascii_lowercase();
                    if !servers.is_empty() {
                        tools.by_name.insert(name_key.clone(), servers);
                    }
                    if let Some(p) = persona {
                        tools.persona_by_name.insert(name_key, p);
                    }
                }
            }
            tools.entry_count += 1;
        }
        Ok(tools)
    }

    /// Resolve the tool policy for a channel: UUID match first, then name match.
    /// `None` means no tool entry — the caller keeps the global default.
    pub fn resolve(&self, id: &Uuid, name: Option<&str>) -> Option<&Vec<McpServer>> {
        self.by_uuid.get(id).or_else(|| {
            name.and_then(|n| {
                self.by_name
                    .get(&n.trim_start_matches('#').to_ascii_lowercase())
            })
        })
    }

    /// Resolve the persona (system-prompt override) for a channel: UUID match
    /// first, then name match. `None` means no persona entry — the caller keeps
    /// the agent's base `system_prompt`. Independent of tool resolution: a
    /// channel may override tools, the persona, both, or neither.
    pub fn resolve_persona(&self, id: &Uuid, name: Option<&str>) -> Option<&str> {
        self.persona_by_uuid
            .get(id)
            .map(String::as_str)
            .or_else(|| {
                name.and_then(|n| {
                    self.persona_by_name
                        .get(&n.trim_start_matches('#').to_ascii_lowercase())
                        .map(String::as_str)
                })
            })
    }

    /// Number of channel entries in the policy (for startup logging).
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }
}

impl ChannelToolsEntry {
    /// Validate one entry's shape before it's split into tool/persona maps.
    /// Errors are fatal (never silently degrade), matching the load contract.
    fn validate(&self, key: &str) -> Result<(), String> {
        if self.persona.is_some() && self.persona_file.is_some() {
            return Err(format!(
                "channel-tools entry '{key}' sets both `persona` and `persona_file` (mutually exclusive)"
            ));
        }
        if self.room.is_none()
            && self.mcp.is_empty()
            && self.persona.is_none()
            && self.persona_file.is_none()
        {
            return Err(format!(
                "channel-tools entry '{key}' declares none of `room`, `mcp`, `persona`, `persona_file`"
            ));
        }
        if let Some(room) = &self.room {
            if room.trim().is_empty() {
                return Err(format!("channel-tools entry '{key}' has an empty `room`"));
            }
        }
        if let Some(p) = &self.persona {
            if p.trim().is_empty() {
                return Err(format!(
                    "channel-tools entry '{key}' has an empty `persona`"
                ));
            }
        }
        Ok(())
    }

    /// Build the MCP servers for this entry. Empty when it scopes no tools (e.g.
    /// a persona-only entry) — the caller then leaves the channel at the default
    /// tool set. Assumes `validate` has already run.
    fn to_servers(&self, default_harbor: &str) -> Vec<McpServer> {
        let mut servers = Vec::new();
        if let Some(room) = &self.room {
            servers.push(McpServer {
                name: format!("harbor-{room}"),
                command: self
                    .harbor_command
                    .as_deref()
                    .unwrap_or(default_harbor)
                    .to_string(),
                args: vec!["mcp-server".into(), format!("--room={room}")],
                env: vec![],
            });
        }
        for m in &self.mcp {
            servers.push(McpServer {
                name: m.name.clone(),
                command: m.command.clone(),
                args: m.args.clone(),
                env: m
                    .env
                    .iter()
                    .map(|(k, v)| EnvVar {
                        name: k.clone(),
                        value: v.clone(),
                    })
                    .collect(),
            });
        }
        servers
    }

    /// Resolve this entry's persona string: inline `persona`, or the contents of
    /// `persona_file` (fatal on read error / empty file), or `None`.
    fn read_persona(&self, key: &str) -> Result<Option<String>, String> {
        if let Some(p) = &self.persona {
            return Ok(Some(p.clone()));
        }
        if let Some(pf) = &self.persona_file {
            let path = expand_tilde(pf);
            let content = std::fs::read_to_string(&path).map_err(|e| {
                format!(
                    "channel-tools entry '{key}': cannot read persona_file {}: {e}",
                    path.display()
                )
            })?;
            if content.trim().is_empty() {
                return Err(format!(
                    "channel-tools entry '{key}': persona_file {} is empty",
                    path.display()
                ));
            }
            return Ok(Some(content));
        }
        Ok(None)
    }
}

/// Expand a leading `~` to `$HOME` (persona_file paths in the policy use it).
fn expand_tilde(p: &Path) -> PathBuf {
    if let Ok(rest) = p.strip_prefix("~") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(s: &str) -> Result<ChannelTools, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        // Unique filename per call — a hash-of-content name makes two tests with
        // the same TOML share a file and race (truncate mid-read) under the
        // default parallel test runner.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!("chtools-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("t{n}.toml"));
        std::fs::write(&path, s).unwrap();
        ChannelTools::load(&path)
    }

    #[test]
    fn room_entry_expands_to_harbor_server() {
        let t = load_str("[channels.bookkeeping]\nroom = \"bookkeeping\"\n").unwrap();
        let id = Uuid::nil();
        let servers = t.resolve(&id, Some("bookkeeping")).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "harbor-bookkeeping");
        assert_eq!(servers[0].command, "harbor");
        assert_eq!(servers[0].args, vec!["mcp-server", "--room=bookkeeping"]);
    }

    #[test]
    fn uuid_key_matches_by_uuid_and_wins_over_name() {
        let id = "6f8199d3-479f-4dce-8851-000a4b0a2586";
        let toml = format!(
            "[channels.\"{id}\"]\nroom = \"legal\"\n[channels.general]\nroom = \"devops\"\n"
        );
        let t = load_str(&toml).unwrap();
        let uuid: Uuid = id.parse().unwrap();
        let servers = t.resolve(&uuid, Some("general")).unwrap();
        assert_eq!(servers[0].args[1], "--room=legal");
    }

    #[test]
    fn name_match_is_case_insensitive_and_hash_insensitive() {
        let t = load_str("[channels.\"#BookKeeping\"]\nroom = \"bookkeeping\"\n").unwrap();
        assert!(t.resolve(&Uuid::nil(), Some("bookkeeping")).is_some());
        assert!(t.resolve(&Uuid::nil(), Some("#Bookkeeping")).is_some());
        assert!(t.resolve(&Uuid::nil(), Some("other")).is_none());
    }

    #[test]
    fn unmatched_channel_resolves_none() {
        let t = load_str("[channels.bookkeeping]\nroom = \"bookkeeping\"\n").unwrap();
        assert!(t.resolve(&Uuid::nil(), Some("marketing")).is_none());
        assert!(t.resolve(&Uuid::nil(), None).is_none());
    }

    #[test]
    fn explicit_mcp_servers_carry_args_and_env() {
        let t = load_str(
            "[channels.devops]\nroom = \"devops\"\n[[channels.devops.mcp]]\nname = \"sysmon\"\ncommand = \"/bin/sysmon\"\nargs = [\"--stdio\"]\nenv = { FOO = \"bar\" }\n",
        )
        .unwrap();
        let servers = t.resolve(&Uuid::nil(), Some("devops")).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[1].name, "sysmon");
        assert_eq!(servers[1].env[0].name, "FOO");
        assert_eq!(servers[1].env[0].value, "bar");
    }

    #[test]
    fn custom_harbor_command_overrides_default() {
        let t = load_str(
            "harbor_command = \"/opt/harbor\"\n[channels.a]\nroom = \"x\"\n[channels.b]\nroom = \"y\"\nharbor_command = \"/special/harbor\"\n",
        )
        .unwrap();
        assert_eq!(
            t.resolve(&Uuid::nil(), Some("a")).unwrap()[0].command,
            "/opt/harbor"
        );
        assert_eq!(
            t.resolve(&Uuid::nil(), Some("b")).unwrap()[0].command,
            "/special/harbor"
        );
    }

    #[test]
    fn entry_without_room_or_mcp_is_rejected() {
        assert!(load_str("[channels.empty]\n").is_err());
    }

    #[test]
    fn empty_room_is_rejected() {
        assert!(load_str("[channels.a]\nroom = \"  \"\n").is_err());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(load_str("[channels.a]\nroom = \"x\"\ntypo_key = 1\n").is_err());
    }

    // ---- channel-scoped personas ----

    fn write_tmp(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("chtools-persona-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn inline_persona_resolves() {
        let t = load_str("[channels.support]\npersona = \"You are support.\"\n").unwrap();
        assert_eq!(
            t.resolve_persona(&Uuid::nil(), Some("support")),
            Some("You are support.")
        );
    }

    #[test]
    fn persona_file_is_loaded_and_composes_with_tools() {
        let pf = write_tmp("acct.md", "You are the accountant persona.\n");
        let toml = format!(
            "[channels.bookkeeping]\nroom = \"bookkeeping\"\npersona_file = \"{}\"\n",
            pf.display()
        );
        let t = load_str(&toml).unwrap();
        assert_eq!(
            t.resolve_persona(&Uuid::nil(), Some("bookkeeping")),
            Some("You are the accountant persona.\n")
        );
        // tools and persona compose on the one entry
        assert_eq!(
            t.resolve(&Uuid::nil(), Some("bookkeeping")).unwrap()[0].name,
            "harbor-bookkeeping"
        );
    }

    #[test]
    fn persona_only_entry_scopes_no_tools() {
        let t = load_str("[channels.chat]\npersona = \"friendly greeter\"\n").unwrap();
        assert_eq!(
            t.resolve_persona(&Uuid::nil(), Some("chat")),
            Some("friendly greeter")
        );
        // no room/mcp → tools stay at the default (resolve returns None)
        assert!(t.resolve(&Uuid::nil(), Some("chat")).is_none());
    }

    #[test]
    fn persona_uuid_key_wins_over_name() {
        let id = "6f8199d3-479f-4dce-8851-000a4b0a2586";
        let toml = format!(
            "[channels.\"{id}\"]\npersona = \"by uuid\"\n[channels.general]\npersona = \"by name\"\n"
        );
        let t = load_str(&toml).unwrap();
        let uuid: Uuid = id.parse().unwrap();
        assert_eq!(t.resolve_persona(&uuid, Some("general")), Some("by uuid"));
    }

    #[test]
    fn unmatched_channel_has_no_persona() {
        let t = load_str("[channels.support]\npersona = \"x\"\n").unwrap();
        assert!(t.resolve_persona(&Uuid::nil(), Some("other")).is_none());
        assert!(t.resolve_persona(&Uuid::nil(), None).is_none());
    }

    #[test]
    fn persona_and_persona_file_both_set_is_fatal() {
        let pf = write_tmp("both.md", "x");
        let toml = format!(
            "[channels.a]\npersona = \"inline\"\npersona_file = \"{}\"\n",
            pf.display()
        );
        assert!(load_str(&toml).is_err());
    }

    #[test]
    fn missing_persona_file_is_fatal() {
        assert!(load_str("[channels.a]\npersona_file = \"/no/such/persona/file.md\"\n").is_err());
    }

    #[test]
    fn empty_persona_is_rejected() {
        assert!(load_str("[channels.a]\npersona = \"   \"\n").is_err());
    }

    #[test]
    fn empty_persona_file_is_rejected() {
        let pf = write_tmp("empty.md", "\n  \n");
        let toml = format!("[channels.a]\npersona_file = \"{}\"\n", pf.display());
        assert!(load_str(&toml).is_err());
    }

    /// The one risk the tool-scoping precedent never exercised: a single shared
    /// process (one identity) serving different personas per channel must never
    /// cross prompts between concurrent sessions. Resolution is a pure lookup
    /// over immutable maps and the session build uses it in a local — so there
    /// is no shared mutable state to bleed. This hammers that property under
    /// contention to prove it.
    #[test]
    fn concurrent_resolution_never_bleeds_across_channels() {
        use std::sync::Arc;
        let t = Arc::new(
            load_str(
                "[channels.alpha]\npersona = \"I am ALPHA and only ALPHA.\"\n\
                 [channels.beta]\npersona = \"I am BETA and only BETA.\"\n",
            )
            .unwrap(),
        );
        let mut handles = Vec::new();
        for i in 0..64 {
            let t = Arc::clone(&t);
            handles.push(std::thread::spawn(move || {
                let (ch, want) = if i % 2 == 0 {
                    ("alpha", "I am ALPHA and only ALPHA.")
                } else {
                    ("beta", "I am BETA and only BETA.")
                };
                for _ in 0..2000 {
                    assert_eq!(t.resolve_persona(&Uuid::nil(), Some(ch)), Some(want));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }
}
