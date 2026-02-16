import { useEffect, useMemo, useRef, useState } from "react";
import * as Select from "@radix-ui/react-select";
import {
  Brain,
  Check,
  ChevronDown,
  Code2,
  Gauge,
  Globe,
  ListTodo,
  Search,
  ShieldCheck,
  ShieldQuestion,
  Zap,
} from "lucide-react";
import { modelProviderLabel } from "@homie/shared";
import type {
  ChatSettings,
  ChatWebToolName,
  CollaborationModeOption,
  ModelOption,
  ThreadTokenUsage,
} from "@/lib/chat-utils";

type SelectOption = {
  value: string;
  label: string;
  description?: string;
  icon?: React.ReactNode;
  group?: string;
};

interface ChatSelectProps {
  label: string;
  value: string;
  icon: React.ReactNode;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  searchable?: boolean;
  searchPlaceholder?: string;
}

function ChatSelect({
  label,
  value,
  icon,
  options,
  onChange,
  disabled,
  searchable = false,
  searchPlaceholder = "Search...",
}: ChatSelectProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const active = options.find((opt) => opt.value === value) ?? options[0];
  const normalizedQuery = query.trim().toLowerCase();
  const filteredOptions = useMemo(() => {
    if (!searchable || !normalizedQuery) return options;
    return options.filter((opt) => {
      const haystack = `${opt.label} ${opt.description ?? ""} ${opt.value}`.toLowerCase();
      return haystack.includes(normalizedQuery);
    });
  }, [normalizedQuery, options, searchable]);
  const hasGroups = useMemo(
    () => options.some((opt) => Boolean(opt.group?.trim())),
    [options],
  );
  const groupedOptions = useMemo(() => {
    const groups = new Map<string, SelectOption[]>();
    for (const opt of filteredOptions) {
      const label = opt.group?.trim() || "Options";
      const existing = groups.get(label);
      if (existing) {
        existing.push(opt);
      } else {
        groups.set(label, [opt]);
      }
    }
    return Array.from(groups.entries());
  }, [filteredOptions]);

  useEffect(() => {
    if (!open) {
      setQuery("");
      return;
    }
    if (!searchable) return;
    const id = window.requestAnimationFrame(() => {
      searchInputRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(id);
  }, [open, searchable]);

  return (
    <Select.Root value={value} onValueChange={onChange} disabled={disabled} open={open} onOpenChange={setOpen}>
      <Select.Trigger
        className="inline-flex items-center gap-2 min-h-[40px] min-w-[124px] max-w-[220px] px-2.5 rounded-lg border border-border/60 bg-card/30 text-sm text-foreground hover:bg-muted/40 data-[state=open]:bg-muted/40 transition-colors motion-reduce:transition-none disabled:opacity-60"
        aria-label={label}
      >
        <span className="text-muted-foreground">{icon}</span>
        <span className="truncate text-left flex-1">{active?.label ?? label}</span>
        <Select.Icon className="text-muted-foreground">
          <ChevronDown className="w-4 h-4" />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content
          sideOffset={8}
          position="popper"
          collisionPadding={12}
          className="z-50 w-[var(--radix-select-trigger-width)] max-w-[min(420px,calc(100vw-24px))] rounded-lg border border-border/80 bg-popover shadow-lg homie-popover overflow-hidden"
        >
          {searchable && (
            <div className="sticky top-0 z-10 bg-popover border-b border-border/60 p-2">
              <label className="relative block">
                <Search className="w-3.5 h-3.5 text-muted-foreground absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                <input
                  ref={searchInputRef}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    event.stopPropagation();
                  }}
                  placeholder={searchPlaceholder}
                  className="w-full h-8 rounded-md border border-border/70 bg-background pl-8 pr-2 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary/50"
                  aria-label={`${label} search`}
                />
              </label>
            </div>
          )}
          <Select.Viewport
            className="p-1 overflow-y-auto chat-scroll"
            style={{ maxHeight: "min(22rem, var(--radix-select-content-available-height))" }}
          >
            {groupedOptions.map(([groupLabel, groupItems]) => (
              <Select.Group key={groupLabel}>
                {hasGroups ? (
                  <Select.Label className="px-3 py-1.5 text-[11px] uppercase tracking-wide text-muted-foreground">
                    {groupLabel}
                  </Select.Label>
                ) : null}
                {groupItems.map((opt) => (
                  <Select.Item
                    key={opt.value}
                    value={opt.value}
                    className="relative w-full text-left px-3 py-2 rounded-md min-h-[40px] transition-colors motion-reduce:transition-none text-foreground data-[highlighted]:bg-muted/50 data-[state=checked]:bg-muted/70 outline-none cursor-default pr-9"
                  >
                    <div className="flex items-center gap-2">
                      {opt.icon && <span className="text-muted-foreground">{opt.icon}</span>}
                      <span className="text-sm font-medium">
                        <Select.ItemText>{opt.label}</Select.ItemText>
                      </span>
                    </div>
                    {opt.description && <div className="text-xs text-muted-foreground mt-1">{opt.description}</div>}
                    <Select.ItemIndicator className="absolute right-2 top-1/2 -translate-y-1/2 text-primary">
                      <Check className="w-4 h-4" />
                    </Select.ItemIndicator>
                  </Select.Item>
                ))}
              </Select.Group>
            ))}
            {filteredOptions.length === 0 && (
              <div className="px-3 py-2 text-xs text-muted-foreground">No results.</div>
            )}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}

interface ChatComposerBarProps {
  models: ModelOption[];
  collaborationModes: CollaborationModeOption[];
  settings: ChatSettings;
  enabledWebTools: ChatWebToolName[];
  webToolsAvailable: boolean;
  tokenUsage?: ThreadTokenUsage;
  running: boolean;
  queuedHint?: boolean;
  disabled?: boolean;
  onChangeSettings: (updates: Partial<ChatSettings>) => void;
}

function ContextRing({ usage }: { usage: ThreadTokenUsage }) {
  const window = usage.modelContextWindow ?? 0;
  const used = usage.last.totalTokens || usage.total.totalTokens;
  const percent = window > 0 ? Math.min(used / window, 1) : 0;
  const label = window > 0 ? `${used.toLocaleString()}/${window.toLocaleString()}` : "—";

  return (
    <div className="min-w-[120px]">
      <div className="flex items-center justify-between gap-2 text-[11px] uppercase tracking-wide text-muted-foreground">
        <span>Context</span>
        <span className="font-medium text-foreground tabular-nums normal-case">{label}</span>
      </div>
      <div className="mt-1 h-1.5 rounded-full bg-muted/50 overflow-hidden" aria-hidden="true">
        <div
          className="h-full rounded-full bg-primary transition-[width] duration-200 ease-out motion-reduce:transition-none"
          style={{ width: `${Math.max(4, percent * 100)}%` }}
        />
      </div>
    </div>
  );
}

function WebToolsIndicator({
  enabledTools,
  available,
}: {
  enabledTools: ChatWebToolName[];
  available: boolean;
}) {
  const enabledSet = useMemo(() => new Set(enabledTools), [enabledTools]);

  return (
    <div className="flex items-center gap-1.5 min-h-[24px]">
      <div className="text-[11px] uppercase tracking-wide text-muted-foreground inline-flex items-center gap-1">
        <Globe className="w-3.5 h-3.5" />
        Web
      </div>
      <div className="flex items-center gap-1.5">
        {(["web_fetch", "web_search", "browser"] as const).map((toolName) => {
          const enabled = available && enabledSet.has(toolName);
          const label =
            toolName === "web_fetch" ? "Fetch" : toolName === "web_search" ? "Search" : "Browser";
          return (
            <span
              key={toolName}
              className={`inline-flex items-center justify-center rounded-full border px-2 py-0.5 text-[10px] leading-none font-medium transition-colors motion-reduce:transition-none ${
                enabled
                  ? "border-emerald-500/50 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
                  : "border-border bg-muted/40 text-muted-foreground"
              }`}
              title={available ? `${toolName} ${enabled ? "enabled" : "disabled"}` : "Web tools unavailable"}
            >
              {label}
            </span>
          );
        })}
      </div>
    </div>
  );
}

export function ChatComposerBar({
  models,
  collaborationModes,
  settings,
  enabledWebTools,
  webToolsAvailable,
  tokenUsage,
  running,
  queuedHint,
  disabled,
  onChangeSettings,
}: ChatComposerBarProps) {
  const selectedModel =
    models.find((model) => model.model === settings.model || model.id === settings.model) ??
    models.find((model) => model.isDefault) ??
    models[0];

  const modelOptions = useMemo<SelectOption[]>(() => {
    if (!models.length) {
      return [{ value: settings.model ?? "default", label: settings.model ?? "Default model" }];
    }
    return models.map((model) => ({
      value: model.model || model.id,
      label: model.displayName || model.model || model.id,
      description: model.description,
      icon: <Brain className="w-4 h-4" />,
      group: modelProviderLabel(model),
    }));
  }, [models, settings.model]);

  const effortOptions = useMemo<SelectOption[]>(() => {
    const supported = selectedModel?.supportedReasoningEfforts ?? [];
    const entries = supported.length
      ? supported.map((effort) => ({
          value: effort.reasoningEffort,
          label: effort.reasoningEffort.toUpperCase(),
          description: effort.description,
        }))
      : [
          { value: "low", label: "LOW", description: "Faster, lighter reasoning." },
          { value: "medium", label: "MEDIUM", description: "Balanced default." },
          { value: "high", label: "HIGH", description: "Deeper reasoning." },
        ];
    return [
      {
        value: "auto",
        label: "Auto",
        description: selectedModel?.defaultReasoningEffort
          ? `Default: ${selectedModel.defaultReasoningEffort.toUpperCase()}`
          : "Model default effort.",
        icon: <Gauge className="w-4 h-4" />,
      },
      ...entries.map((entry) => ({
        ...entry,
        icon: <Gauge className="w-4 h-4" />,
      })),
    ];
  }, [selectedModel]);

  const permissionOptions: SelectOption[] = [
    {
      value: "explore",
      label: "Explore",
      description: "Always ask for approval.",
      icon: <ShieldQuestion className="w-4 h-4" />,
    },
    {
      value: "ask",
      label: "Ask",
      description: "Ask for untrusted actions.",
      icon: <ShieldCheck className="w-4 h-4" />,
    },
    {
      value: "execute",
      label: "Execute",
      description: "Run without approval prompts.",
      icon: <Zap className="w-4 h-4" />,
    },
  ];

  const agentModes = useMemo(() => {
    const normalized = collaborationModes
      .map((mode) => {
        const value = (mode.mode ?? mode.id).toLowerCase();
        if (!value) return null;
        return {
          value,
          label: value === "plan" ? "Plan" : value === "code" ? "Code" : mode.label,
        };
      })
      .filter(Boolean) as Array<{ value: string; label: string }>;
    return normalized.filter((mode) => mode.value === "code" || mode.value === "plan");
  }, [collaborationModes]);

  const showAgentModeToggle = agentModes.length >= 2;

  return (
    <div className="rounded-lg border border-border/70 bg-card/40 px-2.5 py-2 flex flex-wrap items-center gap-2.5">
      {showAgentModeToggle && (
        <div className="flex items-center gap-2">
          <div className="text-[11px] uppercase tracking-wide text-muted-foreground">Agent</div>
          <div className="flex items-center bg-muted/50 rounded-md p-1 min-h-[40px]">
            {agentModes.map((mode) => {
              const isActive = settings.agentMode === mode.value;
              const icon = mode.value === "plan" ? <ListTodo className="w-4 h-4" /> : <Code2 className="w-4 h-4" />;
              return (
                <button
                  key={mode.value}
                  type="button"
                  disabled={disabled}
                  aria-pressed={isActive}
                  onClick={() => onChangeSettings({ agentMode: mode.value as ChatSettings["agentMode"] })}
                  className={`inline-flex items-center gap-2 px-2.5 py-1.5 min-h-[36px] rounded-md text-xs font-medium transition-colors motion-reduce:transition-none ${
                    isActive
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {icon}
                  {mode.label}
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <ChatSelect
          label="Model"
          value={settings.model ?? modelOptions[0]?.value ?? "default"}
          icon={<Brain className="w-4 h-4" />}
          options={modelOptions}
          searchable
          searchPlaceholder="Search models..."
          disabled={disabled}
          onChange={(value) => onChangeSettings({ model: value })}
        />
        <ChatSelect
          label="Effort"
          value={settings.effort}
          icon={<Gauge className="w-4 h-4" />}
          options={effortOptions}
          disabled={disabled}
          onChange={(value) => onChangeSettings({ effort: value as ChatSettings["effort"] })}
        />
        <ChatSelect
          label="Permission"
          value={settings.permission}
          icon={<ShieldCheck className="w-4 h-4" />}
          options={permissionOptions}
          disabled={disabled}
          onChange={(value) => onChangeSettings({ permission: value as ChatSettings["permission"] })}
        />
      </div>

      <div className="ml-auto flex items-center gap-3">
        <WebToolsIndicator enabledTools={enabledWebTools} available={webToolsAvailable} />
        {queuedHint ? (
          <div className="text-xs text-muted-foreground">Queued for next step</div>
        ) : running ? (
          <div className="text-xs text-muted-foreground">Applies next message</div>
        ) : null}
        {tokenUsage && <ContextRing usage={tokenUsage} />}
      </div>
    </div>
  );
}
