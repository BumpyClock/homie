import { Moon, Sun, Monitor } from "lucide-react";
import { useTheme, type Theme, type ColorScheme, COLOR_SCHEMES } from "@/hooks/use-theme";
import { PREVIEW_OPTIONS, type PreviewRefresh } from "@/lib/session-utils";

interface PreferencesSectionProps {
  previewRefresh: PreviewRefresh;
  onPreviewRefresh: (value: PreviewRefresh) => void;
}

const THEME_MODES: { value: Theme; icon: typeof Sun; label: string }[] = [
  { value: "light", icon: Sun, label: "Light" },
  { value: "dark", icon: Moon, label: "Dark" },
  { value: "system", icon: Monitor, label: "System" },
];

const SCHEME_OPTIONS: { value: ColorScheme; label: string; color: string }[] = [
  { value: "default", label: "Default", color: "bg-zinc-500" },
  { value: "monokai", label: "Monokai", color: "bg-pink-500" },
  { value: "one-dark", label: "One Dark", color: "bg-blue-500" },
  { value: "flexoki", label: "Flexoki", color: "bg-amber-500" },
  { value: "dracula", label: "Dracula", color: "bg-violet-500" },
  { value: "catppuccin", label: "Catppuccin", color: "bg-purple-500" },
];

export function PreferencesSection({ previewRefresh, onPreviewRefresh }: PreferencesSectionProps) {
  const { theme, setTheme, colorScheme, setColorScheme } = useTheme();

  return (
    <div
      role="tabpanel"
      id="settings-panel-preferences"
      aria-labelledby="settings-tab-preferences"
      className="space-y-6"
    >
      <h2 className="text-sm font-semibold text-text-primary">Preferences</h2>

      {/* Theme mode */}
      <div className="space-y-2">
        <label className="text-xs uppercase tracking-wide text-text-secondary font-medium">Theme Mode</label>
        <div className="flex bg-surface-1 rounded-lg p-1 gap-1">
          {THEME_MODES.map((t) => {
            const Icon = t.icon;
            return (
              <button
                key={t.value}
                type="button"
                onClick={() => setTheme(t.value)}
                className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 min-h-[44px] rounded-md text-sm transition-colors duration-[140ms] ${
                  theme === t.value
                    ? "bg-surface-0 text-text-primary shadow-sm font-medium"
                    : "text-text-secondary hover:text-text-primary"
                }`}
                aria-pressed={theme === t.value}
                aria-label={t.label}
              >
                <Icon className="w-4 h-4" />
                <span className="hidden sm:inline">{t.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Color scheme */}
      <div className="space-y-2">
        <label className="text-xs uppercase tracking-wide text-text-secondary font-medium">Color Scheme</label>
        <div className="grid grid-cols-3 gap-2">
          {SCHEME_OPTIONS.map((s) => (
            <button
              key={s.value}
              type="button"
              onClick={() => setColorScheme(s.value)}
              className={`flex flex-col items-center gap-1.5 p-2.5 min-h-[44px] rounded-md border-2 transition-colors duration-[140ms] ${
                colorScheme === s.value
                  ? "border-primary bg-accent-dim"
                  : "border-transparent hover:bg-surface-1"
              }`}
              aria-pressed={colorScheme === s.value}
              aria-label={s.label}
            >
              <div className={`w-5 h-5 rounded-full ${s.color}`} />
              <span className="text-xs text-text-secondary">{s.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Preview refresh */}
      <div className="space-y-2">
        <label htmlFor="preview-refresh-select" className="text-xs uppercase tracking-wide text-text-secondary font-medium">
          Preview Refresh Rate
        </label>
        <select
          id="preview-refresh-select"
          value={previewRefresh}
          onChange={(e) => onPreviewRefresh(e.target.value as PreviewRefresh)}
          className="w-full bg-surface-0 border border-border rounded-md px-3 py-2 min-h-[44px] text-sm text-text-primary focus:outline-none focus:border-primary"
        >
          {PREVIEW_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
