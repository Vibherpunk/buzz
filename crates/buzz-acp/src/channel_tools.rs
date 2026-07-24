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
use std::path::Path;

use uuid::Uuid;

use crate::acp::{EnvVar, McpServer};

/// One channel's tool policy as written in TOML.
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

/// Resolved channel→MCP-server policy, keyed by UUID and by lowercased name.
#[derive(Debug, Default)]
pub struct ChannelTools {
    by_uuid: HashMap<Uuid, Vec<McpServer>>,
    by_name: HashMap<String, Vec<McpServer>>,
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
            let servers = entry.to_servers(key, default_harbor)?;
            match key.parse::<Uuid>() {
                Ok(uuid) => {
                    tools.by_uuid.insert(uuid, servers);
                }
                Err(_) => {
                    tools
                        .by_name
                        .insert(key.trim_start_matches('#').to_ascii_lowercase(), servers);
                }
            }
        }
        Ok(tools)
    }

    /// Resolve the policy for a channel: UUID match first, then name match.
    /// `None` means no entry — the caller keeps the global default.
    pub fn resolve(&self, id: &Uuid, name: Option<&str>) -> Option<&Vec<McpServer>> {
        self.by_uuid.get(id).or_else(|| {
            name.and_then(|n| {
                self.by_name
                    .get(&n.trim_start_matches('#').to_ascii_lowercase())
            })
        })
    }

    /// Number of channel entries in the policy (for startup logging).
    pub fn entry_count(&self) -> usize {
        self.by_uuid.len() + self.by_name.len()
    }
}

impl ChannelToolsEntry {
    fn to_servers(&self, key: &str, default_harbor: &str) -> Result<Vec<McpServer>, String> {
        if self.room.is_none() && self.mcp.is_empty() {
            return Err(format!(
                "channel-tools entry '{key}' declares neither `room` nor `mcp`"
            ));
        }
        let mut servers = Vec::new();
        if let Some(room) = &self.room {
            if room.trim().is_empty() {
                return Err(format!("channel-tools entry '{key}' has an empty `room`"));
            }
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
        Ok(servers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(s: &str) -> Result<ChannelTools, String> {
        let dir = std::env::temp_dir().join(format!("chtools-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{:x}.toml", md5ish(s)));
        std::fs::write(&path, s).unwrap();
        ChannelTools::load(&path)
    }

    fn md5ish(s: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
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
}
