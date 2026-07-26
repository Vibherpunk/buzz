# Channel-scoped tools (this fork's addition)

> **What this fork adds over upstream [`block/buzz`](https://github.com/block/buzz):**
> the ability to scope an agent's skills and MCP tools **per channel**, so the
> *channel* — not the agent — is the unit of capability. Everything else in this
> repo is unmodified upstream Buzz.

## The idea

In stock Buzz, whatever skills and extensions an agent is configured with travel
with that agent into every channel — the same everywhere, ungoverned per channel.
This fork makes a **channel** map to a **[Harbor](https://github.com/TDH-Labs/Harbor)
room**; the room is the source of truth for the skills and MCP servers agents may
use when they work in that channel. Scope a channel once, and *every* agent that
responds there — now or later — is confined to exactly that toolset, enforced
server-side and audited. Nothing leaks in from the harness; nothing leaks across
channels.

A `#legal` channel exposes the legal room's skills; a `#billing` channel
exposes the billing room's; an agent moving between them picks up each
channel's tools as it goes. Multiple agents in one channel all share that
channel's scope.

## How it works

A small policy file, `~/.buzz/channel-tools.toml`, maps each channel (by name or
UUID) to a Harbor room:

```toml
harbor_command = "harbor"           # binary used for `room` entries

[channels.legal]
room = "legal"                      # → expands to: harbor mcp-server --room=legal

[channels.billing]
room = "billing"
```

When `buzz-acp` creates a session for a channel, it looks the channel up in that
file and **replaces** the session's MCP servers with the channel's scope:

```
channel has an entry?  →  session gets exactly `harbor mcp-server --room=<room>`
no entry               →  session keeps the harness's own default extensions
```

That single MCP server exposes the room's skills as `list_skills` / `read_skill`,
gated by room and token budget. The lookup keys on the **channel**, not the
agent — so five agents answering in `#legal` all get the `legal` room's skills,
and none can load a skill from another room.

Because scoping happens where the agent *runs* (your machine's `buzz-acp` reading
your local policy), it applies in **any** community the agent works in —
self-hosted or Block-hosted. The relay only moves messages.

## Harnesses: scoping replaces the tool set (which affects Goose specifically)

The override is a full **replace** of the session's MCP servers, and the two
supported harnesses react to that differently — by design:

- **Claude** — its shell and file tools are built-in *agent* capabilities, not
  MCP servers, so the server list can't take them away. A scoped Claude agent
  keeps its built-in shell and gets the channel's tools on top.
- **Goose** — its shell (and its other tools) come from its own local
  `config.yaml` extensions. Goose treats an explicit MCP server list as a full
  replacement of its tool set and **stops loading those extensions**. So scoping
  a channel intentionally overrides Goose's default toolset down to just the
  channel's tools — that's the point (confine per channel).

To keep every scoped agent — any harness — able to *execute*, declare an
execution baseline in the policy:

```toml
baseline_mcp = ["buzz-dev-mcp"]     # appended to every scoped channel
```

Those commands are appended (deduped) to **every scoped channel**, harness-agnostic,
so a scoped Goose agent has **the room's skills + the baseline** (e.g. shell via
`buzz-dev-mcp`) instead of an empty toolbox. Confinement stays intact — the
baseline is a bounded, explicit set, *not* the harness's whole config — and
unscoped/persona-only channels get nothing. Claude gets the baseline too (a
redundant shell — harmless). An agent that needs more than the baseline in a
given channel gets those tools in its **room**, still scoped.

To instead let a Goose agent keep its *full* local functionality in a channel,
don't scope that channel — an unscoped channel keeps the harness's own default
extensions untouched.

**Skills vs personas — only tools scope the harness.** Skills live *inside* the
room's MCP server, so they're part of the scoped tool set: having a room (to have
skills) is what triggers the override. **Personas do not affect tools at all** —
a persona is only the session's system prompt, resolved on a separate path, so a
channel with a persona but no room/tools leaves the harness's tools completely
untouched (Goose keeps its full `config.yaml`). See
[Channel personas](CHANNEL_PERSONAS.md).

## What changed vs. upstream

**Enforcement — `crates/buzz-acp/`:**
| File | Change |
|------|--------|
| `src/channel_tools.rs` | **new** — `ChannelTools::load/resolve` reads the policy file and expands a `room` entry to `harbor mcp-server --room=<room>` (or an explicit inline server list) |
| `src/config.rs` | `--channel-tools` flag / `BUZZ_ACP_CHANNEL_TOOLS` env → policy path |
| `src/lib.rs` | loads the policy at startup |
| `src/pool.rs` | the per-session MCP **replace** override |

**Management GUI — `desktop/`:**
| File | Change |
|------|--------|
| `src-tauri/src/commands/channel_tools.rs` | **new** — Tauri commands that shell to `harbor` to read a channel's tools, add/remove skills & MCP servers, and scope a channel on the fly |
| `src/features/channels/ui/ChannelToolsDialog.tsx` | **new** — the "Channel tools" panel |
| `src/features/channels/ui/ChannelMembersBar.tsx` | the **Channel tools** button in the channel header |
| `src/shared/api/tauriChannelTools.ts` | **new** — the typed API bridge |

## Using it

1. **Install Harbor** (the control plane): see
   [TDH-Labs/Harbor](https://github.com/TDH-Labs/Harbor). It provides the
   `harbor` command this fork calls.
2. **Point `buzz-acp` at a policy file** via `--channel-tools ~/.buzz/channel-tools.toml`
   (or `BUZZ_ACP_CHANNEL_TOOLS`).
3. **Manage per channel from the GUI:** open a channel → **Channel tools** in the
   header → add or remove skills and MCP servers. The first add scopes the
   channel automatically (no hand-edited config). Or use the CLI:
   `harbor channel-tools <channel> --map`, `harbor channel-tools <channel>`.

Full Harbor-side guide: **[Harbor → Buzz](https://github.com/TDH-Labs/Harbor/blob/main/docs/BUZZ.md)**.

## Notes

- **Scopes are keyed by channel name, globally on your machine.** A channel named
  `legal` in two communities gets the same scope; key by UUID to differ them.
- **The scope replaces, it doesn't merge** — a scoped channel exposes exactly the
  room's tools, so the room must contain everything that channel's work needs.
- Harbor is one provider of the scoped command; the policy format is
  provider-agnostic (`harbor_command` plus an explicit `mcp` list are both
  supported), which is the intended shape for upstreaming.
