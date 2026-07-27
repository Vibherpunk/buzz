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

/** An MCP server exposed in a channel: room-configured, or declared inline in the policy. */
export type ChannelMcpServer = {
  name: string;
  source: "room" | "explicit";
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
