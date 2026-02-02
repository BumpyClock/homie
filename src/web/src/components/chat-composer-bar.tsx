import { useEffect, useMemo, useRef, useState } from "react";
import {
  Brain,
  ChevronDown,
  Code2,
  Gauge,
  ListTodo,
  ShieldCheck,
  ShieldQuestion,
  Zap,
} from "lucide-react";
import type {
  ChatSettings,
  CollaborationModeOption,
  ModelOption,
  ThreadTokenUsage,
} from "@/lib/chat-utils";

type SelectOption = {
  value: string;
  label: string;
  description?: string;
  icon?: React.ReactNode;
};

interface ChatSelectProps {
  label: string;
  value: string;
  icon: React.ReactNode;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
}

function ChatSelect({ label, value, icon, options, onChange, disabled }: ChatSelectProps) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const active = options.find((opt) => opt.value === value) ?? options[0];

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (!target) return;
      if (wrapperRef.current && !wrapperRef.current.contains(target)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, []);

  return (
    <div ref={wrapperRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        disabled={disabled}
        className="flex items-center gap-2 min-h-[44px] min-w-[160px] px-3 rounded-md border border-border bg-background text-sm text-foreground hover:bg-muted/50 transition-colors motion-reduce:transition-none disabled:opacity-60"
        aria-label={label}
      >
        <span className="text-muted-foreground">{icon}</span>
        <span className="truncate">{active?.label ?? label}</span>
        <ChevronDown className="w-4 h-4 text-muted-foreground" />
      </button>
      {open && !disabled && (
        <div className="absolute z-20 mt-2 w-64 rounded-md border border-border bg-popover shadow-lg">
          <div className="p-1">
            {options.map((opt) => (
              <button
                key={opt.value}
                type="button"
                onClick={() => {
                  onChange(opt.value);
                  setOpen(false);
                }}
                className={`w-full text-left px-3 py-2 rounded-md min-h-[44px] transition-colors motion-reduce:transition-none ${
                  opt.value === value
                    ? "bg-muted/70 text-foreground"
                    : "hover:bg-muted/50 text-foreground"
                }`}
              >
                <div className="flex items-center gap-2">
                  {opt.icon && <span className="text-muted-foreground">{opt.icon}</span>}
                  <span className="text-sm font-medium">{opt.label}</span>
                </div>
                {opt.description && (
                  <div className="text-xs text-muted-foreground mt-1">{opt.description}</div>
                )}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

interface ChatComposerBarProps {
  models: ModelOption[];
  collaborationModes: CollaborationModeOption[];
  settings: ChatSettings;
  tokenUsage?: ThreadTokenUsage;
  running: boolean;
  disabled?: boolean;
  onChangeSettings: (updates: Partial<ChatSettings>) => void;
}

function ContextRing({ usage }: { usage: ThreadTokenUsage }) {
  const window = usage.modelContextWindow ?? 0;
  const used = usage.last.totalTokens || usage.total.totalTokens;
  const percent = window > 0 ? Math.min(used / window, 1) : 0;
  const radius = 16;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - percent);
  const label = window > 0 ? `${used.toLocaleString()}/${window.toLocaleString()}` : "—";

  return (
    <div className="flex items-center gap-3">
      <div className="relative h-10 w-10">
        <svg className="h-10 w-10 -rotate-90" viewBox="0 0 40 40" aria-hidden="true">
          <circle
            cx="20"
            cy="20"
            r={radius}
            stroke="currentColor"
            strokeWidth="4"
            fill="none"
            className="text-muted-foreground/40"
          />
          <circle
            cx="20"
            cy="20"
            r={radius}
            stroke="currentColor"
            strokeWidth="4"
            fill="none"
            strokeDasharray={circumference}
            strokeDashoffset={offset}
            className="text-primary transition-[stroke-dashoffset] duration-200 ease-out motion-reduce:transition-none"
          />
        </svg>
      </div>
      <div>
        <div className="text-xs uppercase tracking-wide text-muted-foreground">Context</div>
        <div className="text-sm font-medium tabular-nums">{label}</div>
      </div>
    </div>
  );
}

export function ChatComposerBar({
  models,
  collaborationModes,
  settings,
  tokenUsage,
  running,
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
    <div className="border border-border rounded-md bg-background/70 px-3 py-2 flex flex-wrap items-center gap-3">
      {showAgentModeToggle && (
        <div className="flex items-center gap-2">
          <div className="text-xs uppercase tracking-wide text-muted-foreground">Agent</div>
          <div className="flex items-center bg-muted/50 rounded-md p-1 min-h-[44px]">
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
                  className={`inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md text-xs font-medium transition-colors motion-reduce:transition-none ${
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

      <div className="ml-auto flex items-center gap-4">
        {running && (
          <div className="text-xs text-muted-foreground">Applies next message</div>
        )}
        {tokenUsage && <ContextRing usage={tokenUsage} />}
      </div>
    </div>
  );
}
