import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Blocks,
  Loader2,
  Plug,
  Puzzle,
  Search,
  UserRound,
  X,
} from "lucide-react";
import * as React from "react";

import {
  addExistingSkillToChannel,
  addMcpToChannel,
  addNewSkillToChannel,
  getChannelPersona,
  getChannelTools,
  getPoolSkills,
  removeChannelPersona,
  removeMcpFromChannel,
  removeSkillFromChannel,
  setChannelPersonaFile,
  setChannelPersonaInline,
  type ChannelPersona,
  type ChannelTools,
  type PoolSkill,
} from "@/shared/api/tauriChannelTools";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";

type ChannelToolsDialogProps = {
  /** Channel policy key — its name (matched case-insensitively) or UUID. */
  channelKey: string;
  channelTitle: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

const channelToolsKey = (channel: string) =>
  ["channel-tools", channel] as const;
const poolSkillsKey = ["channel-tools", "pool-skills"] as const;
const channelPersonaKey = (channel: string) =>
  ["channel-persona", channel] as const;

export function ChannelToolsDialog({
  channelKey,
  channelTitle,
  open,
  onOpenChange,
}: ChannelToolsDialogProps) {
  const toolsQuery = useQuery({
    queryKey: channelToolsKey(channelKey),
    queryFn: () => getChannelTools(channelKey),
    enabled: open,
  });

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Channel tools</DialogTitle>
          <DialogDescription>
            Skills and MCP servers available to agents in{" "}
            <span className="font-medium text-foreground">{channelTitle}</span>.
            Anything you add is available in this channel first.
          </DialogDescription>
        </DialogHeader>

        {toolsQuery.isLoading ? (
          <div className="flex items-center justify-center gap-2 py-10 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading channel tools…
          </div>
        ) : toolsQuery.isError ? (
          <ErrorNote
            message={
              toolsQuery.error instanceof Error
                ? toolsQuery.error.message
                : "Failed to load channel tools."
            }
          />
        ) : toolsQuery.data ? (
          <ChannelToolsBody channelKey={channelKey} tools={toolsQuery.data} />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

function ChannelToolsBody({
  channelKey,
  tools,
}: {
  channelKey: string;
  tools: ChannelTools;
}) {
  const queryClient = useQueryClient();
  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: channelToolsKey(channelKey) });

  const removeSkill = useMutation({
    mutationFn: (skill: string) => removeSkillFromChannel(channelKey, skill),
    onSuccess: invalidate,
  });
  const removeMcp = useMutation({
    mutationFn: (name: string) => removeMcpFromChannel(channelKey, name),
    onSuccess: invalidate,
  });
  const removeError =
    (removeSkill.error instanceof Error && removeSkill.error.message) ||
    (removeMcp.error instanceof Error && removeMcp.error.message) ||
    null;

  return (
    <div className="flex max-h-[60vh] flex-col gap-5 overflow-y-auto pr-1">
      {removeError ? <ErrorNote message={removeError} /> : null}
      <PersonaSection channelKey={channelKey} />

      <section className="flex flex-col gap-2">
        <SectionHeading
          icon={<Puzzle className="h-4 w-4" />}
          title="Skills"
          count={tools.skills.length}
        />
        {tools.skills.length === 0 ? (
          <EmptyRow>No skills in this channel yet.</EmptyRow>
        ) : (
          <ul className="flex flex-col gap-1">
            {tools.skills.map((skill) => (
              <li
                className="flex items-start justify-between gap-2 rounded-md border border-border/50 bg-muted/30 px-3 py-2"
                key={skill.name}
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium">
                      {skill.name}
                    </span>
                    {!skill.present ? (
                      <Badge
                        variant="outline"
                        className="shrink-0 text-2xs text-amber-600"
                      >
                        unavailable
                      </Badge>
                    ) : null}
                  </div>
                  {skill.description ? (
                    <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                      {skill.description}
                    </p>
                  ) : null}
                </div>
                <RemoveButton
                  label={`Remove ${skill.name}`}
                  pending={
                    removeSkill.isPending &&
                    removeSkill.variables === skill.name
                  }
                  onClick={() => removeSkill.mutate(skill.name)}
                />
              </li>
            ))}
          </ul>
        )}
        <AddSkillControls channelKey={channelKey} present={tools.skills} />
      </section>

      <section className="flex flex-col gap-2">
        <SectionHeading
          icon={<Plug className="h-4 w-4" />}
          title="MCP servers"
          count={tools.mcpServers.length}
        />
        {tools.mcpServers.length === 0 ? (
          <EmptyRow>No MCP servers in this channel yet.</EmptyRow>
        ) : (
          <ul className="flex flex-col gap-1">
            {tools.mcpServers.map((server) => (
              <li
                className="flex items-center justify-between gap-2 rounded-md border border-border/50 bg-muted/30 px-3 py-2"
                key={`${server.source}:${server.name}`}
              >
                <span className="truncate text-sm font-medium">
                  {server.name}
                </span>
                <RemoveButton
                  label={`Remove ${server.name}`}
                  pending={
                    removeMcp.isPending && removeMcp.variables === server.name
                  }
                  onClick={() => removeMcp.mutate(server.name)}
                />
              </li>
            ))}
          </ul>
        )}
        <AddMcpControls channelKey={channelKey} />
      </section>
    </div>
  );
}

function personaSourceLabel(source: string): string {
  if (source === "override-inline") return "custom";
  if (source === "override-file") return "custom file";
  return "from room";
}

/**
 * The channel's persona (system prompt). Auto-derived from the channel's room —
 * exactly like skills/MCP — with the option to pick a different room persona,
 * write a custom one, or remove the override.
 */
function PersonaSection({ channelKey }: { channelKey: string }) {
  const queryClient = useQueryClient();
  const personaQuery = useQuery({
    queryKey: channelPersonaKey(channelKey),
    queryFn: () => getChannelPersona(channelKey),
  });
  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: channelPersonaKey(channelKey) });

  const setFile = useMutation({
    mutationFn: (file: string) => setChannelPersonaFile(channelKey, file),
    onSuccess: invalidate,
  });
  const setInline = useMutation({
    mutationFn: (text: string) => setChannelPersonaInline(channelKey, text),
    onSuccess: invalidate,
  });
  const remove = useMutation({
    mutationFn: () => removeChannelPersona(channelKey),
    onSuccess: invalidate,
  });

  const [mode, setMode] = React.useState<null | "pick" | "custom">(null);
  const [customText, setCustomText] = React.useState("");

  const data: ChannelPersona | undefined = personaQuery.data;
  const busy = setFile.isPending || setInline.isPending || remove.isPending;
  const mutationError =
    (setFile.error instanceof Error && setFile.error.message) ||
    (setInline.error instanceof Error && setInline.error.message) ||
    (remove.error instanceof Error && remove.error.message) ||
    null;

  return (
    <section className="flex flex-col gap-2">
      <SectionHeading
        icon={<UserRound className="h-4 w-4" />}
        title="Persona"
        count={data?.effective ? 1 : 0}
      />
      {personaQuery.isLoading ? (
        <div className="flex items-center gap-2 px-1 py-2 text-xs text-muted-foreground">
          <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading persona…
        </div>
      ) : personaQuery.isError ? (
        <ErrorNote
          message={
            personaQuery.error instanceof Error
              ? personaQuery.error.message
              : "Failed to load persona."
          }
        />
      ) : data ? (
        <React.Fragment>
          {data.effective ? (
            <div className="flex items-start justify-between gap-2 rounded-md border border-border/50 bg-muted/30 px-3 py-2">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium">
                    {data.effective.name}
                  </span>
                  <Badge
                    variant="outline"
                    className="shrink-0 text-2xs text-muted-foreground"
                  >
                    {personaSourceLabel(data.effective.source)}
                  </Badge>
                </div>
                {data.effective.preview ? (
                  <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                    {data.effective.preview}
                  </p>
                ) : null}
              </div>
              {data.overridden ? (
                <RemoveButton
                  label="Remove persona"
                  pending={remove.isPending}
                  onClick={() => remove.mutate()}
                />
              ) : null}
            </div>
          ) : data.ambiguous ? (
            <EmptyRow>
              This channel’s room has several personas — pick the one this
              channel should use.
            </EmptyRow>
          ) : (
            <EmptyRow>
              No persona — agents here use their own base prompt.
            </EmptyRow>
          )}

          {mode === "pick" ? (
            <div className="flex flex-col gap-1 rounded-md border border-border/50 p-2">
              {data.roomOptions.length === 0 ? (
                <EmptyRow>This channel’s room has no personas.</EmptyRow>
              ) : (
                <ul className="flex max-h-40 flex-col gap-1 overflow-y-auto">
                  {data.roomOptions.map((opt) => (
                    <li
                      className="flex items-center justify-between gap-2 rounded px-2 py-1.5 hover:bg-muted/50"
                      key={opt.path}
                    >
                      <div className="min-w-0">
                        <div className="truncate text-sm">{opt.name}</div>
                        {opt.preview ? (
                          <div className="truncate text-2xs text-muted-foreground">
                            {opt.preview}
                          </div>
                        ) : null}
                      </div>
                      <Button
                        disabled={busy}
                        onClick={() =>
                          setFile.mutate(opt.path, {
                            onSuccess: () => setMode(null),
                          })
                        }
                        size="sm"
                        type="button"
                        variant="ghost"
                      >
                        Use
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
              <button
                className="self-start text-2xs text-muted-foreground hover:text-foreground"
                onClick={() => setMode(null)}
                type="button"
              >
                Cancel
              </button>
            </div>
          ) : mode === "custom" ? (
            <div className="flex flex-col gap-2 rounded-md border border-border/50 p-2">
              <textarea
                className="min-h-[120px] w-full resize-y rounded-md border border-border/50 bg-background px-3 py-2 text-sm"
                onChange={(e) => setCustomText(e.target.value)}
                placeholder="Write this channel’s system prompt…"
                value={customText}
              />
              <div className="flex items-center justify-end gap-2">
                <button
                  className="text-2xs text-muted-foreground hover:text-foreground"
                  onClick={() => setMode(null)}
                  type="button"
                >
                  Cancel
                </button>
                <Button
                  disabled={busy || customText.trim() === ""}
                  onClick={() =>
                    setInline.mutate(customText, {
                      onSuccess: () => {
                        setCustomText("");
                        setMode(null);
                      },
                    })
                  }
                  size="sm"
                  type="button"
                  variant="default"
                >
                  {setInline.isPending ? (
                    <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
                  ) : null}
                  Save persona
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex gap-2">
              {data.roomOptions.length > 0 ? (
                <Button
                  onClick={() => setMode("pick")}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  <UserRound className="mr-1.5 h-4 w-4" />
                  {data.effective ? "Change persona" : "Choose room persona"}
                </Button>
              ) : null}
              <Button
                onClick={() => setMode("custom")}
                size="sm"
                type="button"
                variant="outline"
              >
                Write custom
              </Button>
            </div>
          )}

          {mutationError ? <ErrorNote message={mutationError} /> : null}
        </React.Fragment>
      ) : null}
    </section>
  );
}

/** Add a skill — pick an existing pool skill, or install a new one from a path. */
function AddSkillControls({
  channelKey,
  present,
}: {
  channelKey: string;
  present: ChannelTools["skills"];
}) {
  const queryClient = useQueryClient();
  const [mode, setMode] = React.useState<null | "existing" | "new">(null);
  const [filter, setFilter] = React.useState("");
  const [source, setSource] = React.useState("");

  const presentNames = React.useMemo(
    () => new Set(present.map((s) => s.name)),
    [present],
  );
  const poolQuery = useQuery({
    queryKey: poolSkillsKey,
    queryFn: getPoolSkills,
    enabled: mode === "existing",
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: channelToolsKey(channelKey) });

  const addExisting = useMutation({
    mutationFn: (skill: string) => addExistingSkillToChannel(channelKey, skill),
    onSuccess: () => {
      void invalidate();
      setFilter("");
    },
  });
  const addNew = useMutation({
    mutationFn: (path: string) => addNewSkillToChannel(channelKey, path),
    onSuccess: () => {
      void invalidate();
      setSource("");
      setMode(null);
    },
  });

  const candidates = React.useMemo(() => {
    const all = (poolQuery.data ?? []).filter((s) => !presentNames.has(s.name));
    const q = filter.trim().toLowerCase();
    const matched = q
      ? all.filter(
          (s) =>
            s.name.toLowerCase().includes(q) ||
            s.description.toLowerCase().includes(q),
        )
      : all;
    return matched.slice(0, 50);
  }, [poolQuery.data, presentNames, filter]);

  const pending = addExisting.isPending || addNew.isPending;

  if (mode === null) {
    return (
      <div className="flex gap-2">
        <Button
          onClick={() => setMode("existing")}
          size="sm"
          type="button"
          variant="outline"
        >
          <Blocks className="mr-1.5 h-4 w-4" />
          Add existing skill
        </Button>
        <Button
          onClick={() => setMode("new")}
          size="sm"
          type="button"
          variant="outline"
        >
          <Puzzle className="mr-1.5 h-4 w-4" />
          Install new skill
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 rounded-md border border-border/50 p-2">
      {mode === "existing" ? (
        <React.Fragment>
          <div className="relative">
            <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              autoFocus
              className="pl-7"
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Search skills elsewhere in Buzz…"
              value={filter}
            />
          </div>
          {poolQuery.isLoading ? (
            <div className="flex items-center gap-2 px-1 py-2 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading skills…
            </div>
          ) : (
            <ul className="flex max-h-40 flex-col gap-1 overflow-y-auto">
              {candidates.length === 0 ? (
                <EmptyRow>No matching skills.</EmptyRow>
              ) : (
                candidates.map((skill: PoolSkill) => (
                  <li
                    className="flex items-center justify-between gap-2 rounded px-2 py-1.5 hover:bg-muted/50"
                    key={skill.name}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-sm">{skill.name}</div>
                      {skill.description ? (
                        <div className="truncate text-2xs text-muted-foreground">
                          {skill.description}
                        </div>
                      ) : null}
                    </div>
                    <Button
                      disabled={pending}
                      onClick={() => addExisting.mutate(skill.name)}
                      size="sm"
                      type="button"
                      variant="ghost"
                    >
                      Add
                    </Button>
                  </li>
                ))
              )}
            </ul>
          )}
        </React.Fragment>
      ) : (
        <React.Fragment>
          <Input
            autoFocus
            onChange={(e) => setSource(e.target.value)}
            placeholder="Path to a skill directory or SKILL.md"
            value={source}
          />
          <div className="flex justify-end gap-2">
            <Button
              disabled={pending || source.trim() === ""}
              onClick={() => addNew.mutate(source.trim())}
              size="sm"
              type="button"
              variant="default"
            >
              {addNew.isPending ? (
                <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
              ) : null}
              Install & add
            </Button>
          </div>
        </React.Fragment>
      )}

      {(addExisting.error || addNew.error) && (
        <ErrorNote
          message={
            (addExisting.error instanceof Error && addExisting.error.message) ||
            (addNew.error instanceof Error && addNew.error.message) ||
            "Failed to add skill."
          }
        />
      )}

      <button
        className="self-start text-2xs text-muted-foreground hover:text-foreground"
        onClick={() => setMode(null)}
        type="button"
      >
        Cancel
      </button>
    </div>
  );
}

/** Add an MCP server (name + command) to the channel. */
function AddMcpControls({ channelKey }: { channelKey: string }) {
  const queryClient = useQueryClient();
  const [open, setOpen] = React.useState(false);
  const [name, setName] = React.useState("");
  const [command, setCommand] = React.useState("");
  const [args, setArgs] = React.useState("");

  const addMcp = useMutation({
    mutationFn: () =>
      addMcpToChannel(
        channelKey,
        name.trim(),
        command.trim(),
        args.trim() || undefined,
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: channelToolsKey(channelKey),
      });
      setName("");
      setCommand("");
      setArgs("");
      setOpen(false);
    },
  });

  if (!open) {
    return (
      <Button
        className="self-start"
        onClick={() => setOpen(true)}
        size="sm"
        type="button"
        variant="outline"
      >
        <Plug className="mr-1.5 h-4 w-4" />
        Add MCP server
      </Button>
    );
  }

  const canSubmit =
    name.trim() !== "" && command.trim() !== "" && !addMcp.isPending;

  return (
    <div className="flex flex-col gap-2 rounded-md border border-border/50 p-2">
      <Input
        autoFocus
        onChange={(e) => setName(e.target.value)}
        placeholder="Server name (e.g. grafana)"
        value={name}
      />
      <Input
        onChange={(e) => setCommand(e.target.value)}
        placeholder="Command (e.g. grafana-mcp)"
        value={command}
      />
      <Input
        onChange={(e) => setArgs(e.target.value)}
        placeholder="Args, comma-separated (optional)"
        value={args}
      />
      {addMcp.error ? (
        <ErrorNote
          message={
            addMcp.error instanceof Error
              ? addMcp.error.message
              : "Failed to add MCP server."
          }
        />
      ) : null}
      <div className="flex justify-end gap-2">
        <button
          className="text-2xs text-muted-foreground hover:text-foreground"
          onClick={() => setOpen(false)}
          type="button"
        >
          Cancel
        </button>
        <Button
          disabled={!canSubmit}
          onClick={() => addMcp.mutate()}
          size="sm"
          type="button"
          variant="default"
        >
          {addMcp.isPending ? (
            <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
          ) : null}
          Add server
        </Button>
      </div>
    </div>
  );
}

function RemoveButton({
  label,
  pending,
  onClick,
}: {
  label: string;
  pending: boolean;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className="shrink-0 rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
      disabled={pending}
      onClick={onClick}
      title={label}
      type="button"
    >
      {pending ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <X className="h-4 w-4" />
      )}
    </button>
  );
}

function SectionHeading({
  icon,
  title,
  count,
}: {
  icon: React.ReactNode;
  title: string;
  count: number;
}) {
  return (
    <div className="flex items-center gap-2 text-sm font-semibold">
      <span className="text-muted-foreground">{icon}</span>
      {title}
      <span className="text-xs font-normal text-muted-foreground">{count}</span>
    </div>
  );
}

function EmptyRow({ children }: { children: React.ReactNode }) {
  return (
    <p className="rounded-md border border-dashed border-border/50 px-3 py-2 text-xs text-muted-foreground">
      {children}
    </p>
  );
}

function ErrorNote({ message }: { message: string }) {
  return (
    <div className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
      <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
      <span className="break-words">{message}</span>
    </div>
  );
}
