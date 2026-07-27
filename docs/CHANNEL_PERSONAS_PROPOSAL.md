# Proposal: channel-scoped personas (extends `channel-tools.toml`)

> **Status: proposal — not built, not approved for implementation.** This touches
> shared `buzz-acp` core session-creation logic; do not implement without explicit
> sign-off and the testing bar below. Companion to the channel-scoped *tools*
> feature (`crates/buzz-acp/src/channel_tools.rs` + the desktop panel).

## Problem this solves

Today one Buzz agent = one Nostr identity = one fixed `system_prompt` (set in
`managed-agents.json`), used everywhere that identity appears. Each identity also
gets a separate, persistent runtime process **per relay/community it's in**
(`ManagedAgentRuntimeKey{pubkey, relay_url}`), so process count scales with
identities × communities.

Consolidating *personas* (writing fewer, broader prompts) would cut that overhead
— but at a real cost. A safety-conscious fleet depends on each domain having a
**narrow, non-diluted, individually-auditable prompt** with its own hard-coded
approval boundaries. Merging prompts risks blurring those boundaries — a real
regression for sensitive domains (e.g. finance), not just an efficiency tradeoff.

This proposal decouples the two: keep narrow, separate, fully-independent prompts
— but let **one identity/process serve several of them**, swapping which one
applies based on which channel a session is created in.

## Design: extend `ChannelToolsEntry`, don't build a parallel mechanism

`channel_tools.rs` already resolves a per-channel policy (by UUID, then by name)
at session-creation time in `pool.rs`, strict/replacing, fatal on load error.
Extend the same struct and the same resolution path rather than adding a second,
independently-loaded config file that must stay in sync with the first.

```rust
// channel_tools.rs — add one optional field to the existing entry
struct ChannelToolsEntry {
    room: Option<String>,
    harbor_command: Option<String>,
    mcp: Vec<McpToml>,
    persona: Option<String>,        // NEW: inline system_prompt override
    persona_file: Option<PathBuf>,  // NEW: load from file instead (long prompts)
    // mutually exclusive with `persona`; same validation pattern as
    // `cron`/`interval` in buzz-workflow's Schedule trigger
}
```

```toml
# ~/.buzz/channel-tools.toml — same file, new optional key per entry
[channels.bookkeeping]
room = "bookkeeping"
persona_file = "~/.buzz/personas/accountant.md"

[channels.marketing]
room = "marketing"
persona_file = "~/.buzz/personas/marketing.md"

[channels.support]
room = "support"
# no persona key — falls back to this identity's base managed-agents.json
# system_prompt, exactly like today. Backward compatible by default.
```

## Semantics (non-negotiable in v1 — matches the tool-scoping philosophy exactly)

- **Replacing, not additive.** A channel entry with `persona`/`persona_file` set
  **fully replaces** the base `system_prompt` for sessions created in that channel
  — not layered underneath, not appended, not merged. This is the load-bearing
  safety property: each channel's behavior stays a complete, self-contained,
  independently-reviewable prompt, identical in spirit to the separate-agent model.
  Do **not** build an additive "base + specialization layer" version — it
  reintroduces prompt-dilution risk and adds a fixed token cost to every turn (the
  shared base is paid for even when irrelevant to the channel).
- **Fatal on load error.** Missing `persona_file`, malformed TOML, or both
  `persona` and `persona_file` set → hard failure at startup, same as the existing
  "never silently degrade to no policy" rule. A channel silently running the wrong
  persona is worse than the process refusing to start.
- **No entry = today's behavior.** Channels without a persona key keep using the
  identity's normal `managed-agents.json` prompt. Nothing changes for channels that
  don't opt in.
- **Composes with existing tool-scoping.** `room`/`mcp` and `persona` can and should
  appear together in one entry — one channel definition should describe everything
  that channel does (tools *and* behavior), not require two separate config files.

## Explicitly out of scope for v1 (real gaps — flag for a future spec)

- **Channel names in the policy are not validated against live channels at load.**
  A missing/empty/unreadable `persona`/`persona_file` is fatal at load, but a channel
  *name* that matches no real channel is **not** caught: the entry silently never
  resolves and that channel keeps its base prompt. This is a pre-existing property of
  the tool-scoping resolver (a mistyped channel name silently fails to scope tools
  too), not new to personas — `buzz-acp` loads the policy with no live channel list,
  resolving per-session by UUID then name. Closing it needs a channel-list source at
  load time. Called out because it's the same failure shape as a plausible-but-wrong
  identifier that reads as normal: fatal-at-load covers the *unresolvable-persona*
  half, not this *wrong-but-resolvable-name* half.
- **`respond_to`/`respond_to_allowlist` stay per-identity, not per-channel.** If one
  channel's persona genuinely needs a different trust boundary than another channel
  on the same shared identity, v1 doesn't solve that. Some identity splits may exist
  *because* of a respond_to difference, not just a prompt difference.
- **Nostr profile (kind:0) stays one-per-identity.** A shared identity's display
  name/avatar is the same across channels with different personas — a persona named
  one thing can visually appear while behaving as another in a different channel.
  Not fatal, but decide it explicitly rather than discover it live.
- **Template variables in `persona`** (channel name, date, etc.) — punt unless a
  real need appears.

## Migration path (once built and approved)

1. Pick ONE low-stakes consolidation candidate first — two pure-advisory agents with
   no action-capable surface, where the safety argument doesn't apply to either.
2. Move each one's exact, current `system_prompt` **verbatim** into its own
   `persona_file`, unchanged — this migration moves *where the prompt lives*, not
   *what it says*.
3. Point both channels' entries at the SAME underlying identity (retire one pubkey,
   keep the other; repoint the retired one's channel memberships to the survivor).
4. Verify byte-for-byte behavior parity in the surviving channels before trimming
   anything else.
5. Only after that pilot holds, consider wider consolidation — treat any agent with
   a hard behavioral guarantee (e.g. "structurally cannot publish") as a "needs
   care" case, not a clean first candidate.

## Testing bar

Mirror the tool-scoping suite exactly (unit tests + clippy clean). Cover:
resolution precedence (UUID over name), fatal-load-error paths, the
mutual-exclusivity check on `persona`/`persona_file`, and a real **integration test
proving two different channels on one identity produce genuinely different,
non-bleeding system prompts in two concurrent sessions** — cross-channel state
bleeding on a shared process is the one truly new risk versus the tool-scoping
precedent, and nothing built so far exercises it.

## What this does and doesn't fix

- **Fixes:** process/relay-pair overhead, *if* identities get consolidated using
  this mechanism (necessary but not sufficient — someone still decides which
  identities to merge and does the migration above).
- **Does not fix:** every identity getting a runtime pair on every known community
  regardless of relevance. That's an orthogonal problem (relay-pair *scoping*, not
  persona-*count*) needing its own investigation into the app's pair-discovery.
- **Does not change per-turn token cost** in the replacing design — each turn still
  pays for one complete, non-diluted prompt. The savings are process/connection/
  memory overhead, not LLM API spend.
