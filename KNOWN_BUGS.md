# Known Bugs

## personas-in-rooms — known defect, confuses agents

**Status:** Rolled back. Do not re-land.
**Introduced by:** `a9217e6b1` — *feat(buzz-acp): channel-scoped personas*
**Rolled back to:** `924515dd8` — *docs: proposal — channel-scoped personas* (the
last commit before the persona code landed in `crates/buzz-acp/src/channel_tools.rs`
+ `pool.rs` and the desktop persona panel/commands).

### What it did

The "channel-scoped personas" feature auto-derived a Buzz channel's persona
(system prompt) from whatever persona file lived in the mapped Harbor room,
resolved per-channel at agent spawn time.

### Why it's a bug

It **confuses the agents**. A channel's system prompt stopped being a single,
owned, reviewable instruction set — it was silently overridden/replaced by
room state the agent could not see or reason about. Behavior became
inconsistent depending on room contents, and prompt provenance was opaque.

### Remediation applied

- Fork rolled back to `924515dd8`; built and installed (no persona code in
  `buzz-acp` or `buzz-desktop`).
- Harbor room persona files relocated out of Harbor and staged as first-class
  **actual agents** (owner-reviewed identities with explicit system prompts)
  instead of room-derived persona overrides.
- All other Buzz installs on the machine removed; only the rolled-back build
  remains.

### Scope

Only the **personas-in-rooms** surface is reverted. The fork's other addition —
**channel-scoped tools** (skills + MCP servers per channel via
`channel-tools.toml`) — is preserved; it predates `a9217e6b1` and does not
confuse agents.

### Recommendation

If per-channel instructions are needed, model them as **actual agents**
(owner-reviewed identities with explicit system prompts), never as
room-derived persona overrides.
