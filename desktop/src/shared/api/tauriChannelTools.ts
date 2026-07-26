/**
 * tauriChannelTools.ts — typed bridge to the `channel_tools_*` Rust commands.
 *
 * A channel maps to a Harbor *room*; the room is the source of truth for the
 * skills and MCP servers that channel exposes, enforced server-side by Harbor's
 * gate. These wrappers shell (in Rust) to the same `harbor` binary buzz-acp
 * uses, so the GUI panel, the CLI, and enforcement all agree on one policy.
 *
 * Reads return `harbor ... --json` verbatim; mutations wrap Harbor's own
 * audited verbs and return its stdout so the panel can surface what happened.
 */
import { invokeTauri } from "@/shared/api/tauri";

/** A skill exposed in a channel (via its room). `present:false` = room lists it but the pool lacks it. */
export type ChannelSkill = {
  name: string;
  description: string;
  present: boolean;
};

/** An MCP server exposed in a channel: room-configured, declared inline, or the policy baseline. */
export type ChannelMcpServer = {
  name: string;
  source: "room" | "explicit" | "baseline";
};

/** The fully-resolved toolset a channel exposes (shape of `harbor channel-tools <c> --json`). */
export type ChannelTools = {
  channel: string;
  /** Null when the channel maps to no room. */
  room: string | null;
  /** False when the channel has no policy entry (not Harbor-scoped). */
  scoped: boolean;
  skills: ChannelSkill[];
  mcpServers: ChannelMcpServer[];
};

/** One pool skill, for the "add a skill from elsewhere in Buzz" picker. */
export type PoolSkill = {
  name: string;
  description: string;
  room: string;
};

/** Resolve one channel to the skills + MCP servers it exposes. */
export function getChannelTools(channel: string): Promise<ChannelTools> {
  return invokeTauri<ChannelTools>("channel_tools_get", { channel });
}

/** Every skill in the Harbor pool (for selecting an existing one to add). */
export function getPoolSkills(): Promise<PoolSkill[]> {
  return invokeTauri<PoolSkill[]>("channel_tools_pool_skills");
}

/** Add an existing skill (from elsewhere in Buzz) to this channel. Scopes it on the fly. */
export function addExistingSkillToChannel(
  channel: string,
  skill: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_add_existing_skill", {
    channel,
    skill,
  });
}

/** Install a brand-new skill from a path and make it available in this channel first. */
export function addNewSkillToChannel(
  channel: string,
  source: string,
  name?: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_add_new_skill", {
    channel,
    source,
    name,
  });
}

/** Remove a skill from this channel (unregister it from the channel's room). */
export function removeSkillFromChannel(
  channel: string,
  skill: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_remove_skill", { channel, skill });
}

/** Remove an MCP server from this channel. */
export function removeMcpFromChannel(
  channel: string,
  name: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_remove_mcp", { channel, name });
}

/** Add an MCP server to this channel. Scopes it on the fly. */
export function addMcpToChannel(
  channel: string,
  name: string,
  command: string,
  args?: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_add_mcp", {
    channel,
    name,
    command,
    args,
  });
}

/** One persona a channel's room offers (for the picker). */
export type RoomPersona = {
  name: string;
  path: string;
  preview: string;
};

/** The persona a channel effectively runs under (shape of `harbor channel-persona <c> --json`). */
export type ChannelPersona = {
  channel: string;
  room: string | null;
  /** The persona in effect, or null when the channel has none (uses base prompt). */
  effective: {
    name: string;
    path: string | null;
    inline: string | null;
    source: "override-file" | "override-inline" | "room";
    preview: string;
    /** The full prompt text, for in-place editing. */
    body: string;
  } | null;
  /** Every persona the mapped room offers. */
  roomOptions: RoomPersona[];
  /** True when the room has several personas and none matches the channel name. */
  ambiguous: boolean;
  /** True when an explicit override is set (vs auto-derived from the room). */
  overridden: boolean;
};

/** Result of auto-applying a channel's room persona. */
export type SyncPersonaResult = {
  channel: string;
  synced: boolean;
  path?: string;
  reason?: string;
};

/** Resolve the persona a channel runs under (auto-derived from its room, or overridden). */
export function getChannelPersona(channel: string): Promise<ChannelPersona> {
  return invokeTauri<ChannelPersona>("channel_tools_get_persona", { channel });
}

/** Auto-apply the channel's room persona when unambiguous (no-op with a reason otherwise). */
export function syncChannelPersona(
  channel: string,
): Promise<SyncPersonaResult> {
  return invokeTauri<SyncPersonaResult>("channel_tools_sync_persona", {
    channel,
  });
}

/** Override a channel's persona with a specific file (e.g. a room persona from the picker). */
export function setChannelPersonaFile(
  channel: string,
  file: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_set_persona_file", {
    channel,
    file,
  });
}

/** Override a channel's persona with custom text. */
export function setChannelPersonaInline(
  channel: string,
  text: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_set_persona_inline", {
    channel,
    text,
  });
}

/** Edit the canonical room persona's full text (applies to every channel that uses it). */
export function editRoomPersona(
  room: string,
  name: string,
  text: string,
): Promise<string> {
  return invokeTauri<string>("channel_tools_edit_room_persona", {
    room,
    name,
    text,
  });
}

/** Remove a channel's persona override (revert to room-derived, or none). */
export function removeChannelPersona(channel: string): Promise<string> {
  return invokeTauri<string>("channel_tools_remove_persona", { channel });
}
