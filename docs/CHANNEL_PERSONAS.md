# Channel-scoped personas

**Status: shipped.** A Buzz channel's persona (system prompt) is auto-derived
from its mapped Harbor room — the same room-is-source-of-truth model the
channel-scoped *tools* feature uses — and managed from the **Channel tools**
panel or the `harbor` CLI. This is a reference for the feature as built.

## What it does

One Buzz agent = one Nostr identity = one base `system_prompt` (in
`managed-agents.json`), used everywhere that identity appears. Channel-scoped
personas let **one identity behave as a different, self-contained persona per
channel** — the persona is bound to the channel, not the agent, so whichever
agent works there adopts it.

Because the persona **replaces** the agent's base prompt (never layers on top),
each channel's behavior stays a complete, independently-reviewable prompt — the
same safety property as running a separate agent per domain, without the separate
process per domain.

## How a channel gets its persona

At session creation, `buzz-acp` resolves, in order:

1. **Explicit override** — a `persona`/`persona_file` key on the channel's entry
   in `~/.buzz/channel-tools.toml`.
2. **Auto-derived from the room** — the channel's mapped Harbor room holds its
   persona(s) as markdown at `<rooms>/<room>/agents/<name>.md`. If exactly one
   matches the channel, it's used automatically (no import step):
   - a room with **one** persona → that persona;
   - a **name match** — since a room's channels are typically
     `<room>-<specialization>` and every persona shares the room name, matching
     keys off the *distinctive* token (`region-eu-sales` → `eu-sales`,
     `team-billing` → `billing-clerk`);
   - a room with several personas and **no** clear match → ambiguous; the panel
     shows a picker.
3. **Neither** → the agent's own base `system_prompt` (unchanged from stock Buzz).

**Precedence:** when a channel resolves to a persona (1 or 2), it **replaces** the
agent's own for sessions in that channel. The agent's base prompt is only the
fallback (case 3). It is replacing, not additive — the room persona is the single
system prompt sent to the agent's harness for that session.

## Managing personas

**In the app** — the **Channel tools** panel (channel header → *Channel tools*)
shows the effective persona next to the channel's skills and MCP servers, with:

- **Change** — pick a different persona the room offers;
- **Write custom** — a channel-specific override (stored under `~/.buzz/personas/`);
- **Edit** — open the current persona's full text and edit in place. Editing a
  *room* persona rewrites the canonical `<rooms>/<room>/agents/<name>.md`, so the
  change applies to every channel that uses it (the editor says so);
- **Remove** — clear the override, reverting to the room-derived persona or none.

**From the CLI** (Harbor, the source of truth):

```bash
harbor channel-persona <channel> --json        # the persona a channel runs under
harbor channel-persona <channel> --sync        # auto-apply the room persona (if unambiguous)
harbor channel-persona <channel> --set-file P   # override with a file
harbor channel-persona <channel> --set-inline T # override with text
harbor channel-persona <channel> --remove       # clear the override
harbor room-personas <room> --json              # personas a room offers
harbor room-persona <room> <name> --json        # a persona's full text
harbor room-persona <room> <name> --set-body T  # edit the canonical persona
```

## Where personas live

- **Room personas** (source of truth): `<rooms>/<room>/agents/<name>.md`. A
  channel's `persona_file` points at the live room file — a reference, not a copy,
  so editing the room file updates every channel that uses it.
- **Custom (channel-specific) personas**: `~/.buzz/personas/<channel>.md`,
  referenced by `persona_file` on that channel's entry.
- **The mapping**: `~/.buzz/channel-tools.toml` — the same file the channel's
  skills/MCP scoping lives in, so one channel entry describes everything that
  channel does (tools *and* behavior).

## Semantics

- **Replacing, not additive.** A resolved persona fully replaces the base
  `system_prompt` for sessions in that channel — not layered, appended, or merged.
- **Fatal on load error.** A configured `persona_file` that is missing/empty, or
  both `persona` and `persona_file` set, is a hard failure at startup — a channel
  silently running the wrong persona is worse than the process refusing to start.
- **Opt-in.** A channel with no persona (explicit or room-derived) behaves exactly
  as stock Buzz.
- **Orthogonal to tools.** A persona is only the session's system prompt — it never
  changes the channel's MCP servers. A channel with a persona but no room/tools
  leaves the harness's own tools completely untouched (including a Goose agent's
  full local `config.yaml`); only [channel-scoped *tools*](CHANNEL_SCOPED_TOOLS.md)
  replace the harness's tool set.

## Not this: `harbor buzz-pack`

`harbor buzz-pack` emits Block's *native* Persona Packs (`buzz-persona`) with
synthesized generic bodies — a separate translator, useful for exporting rooms to
stock Buzz. Channel-scoped personas are the fork's own path: they use the **rich
room persona files verbatim**, per-channel, with the GUI and edit flow above.

## Known limitations

- **Channel names in the policy aren't validated against live channels at load.** A
  `persona_file` that can't load is fatal, but a channel *name* that matches no real
  channel silently never resolves (that channel keeps its base prompt). Pre-existing
  property of the tool-scoping resolver — `buzz-acp` loads the policy with no live
  channel list — not new to personas.
- **`respond_to`/`respond_to_allowlist` stay per-identity, not per-channel.** If one
  channel's persona needs a different trust boundary than another on the same shared
  identity, that isn't expressed here.
- **Nostr profile (kind:0) stays one-per-identity.** A shared identity's display
  name/avatar is the same across channels with different personas.
- **No template variables** in a persona (channel name, date, etc.).
