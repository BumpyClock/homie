import { useRef, useCallback } from "react";
import { Wifi, KeyRound, SlidersHorizontal, Info } from "lucide-react";

export type SettingsSection = "connection" | "providers" | "preferences" | "about";

const SECTIONS: { id: SettingsSection; label: string; icon: typeof Wifi }[] = [
  { id: "connection", label: "Connection", icon: Wifi },
  { id: "providers", label: "Providers", icon: KeyRound },
  { id: "preferences", label: "Preferences", icon: SlidersHorizontal },
  { id: "about", label: "About", icon: Info },
];

interface SettingsNavProps {
  activeSection: SettingsSection;
  onSectionChange: (section: SettingsSection) => void;
}

export function SettingsNav({ activeSection, onSectionChange }: SettingsNavProps) {
  const tabsRef = useRef<(HTMLButtonElement | null)[]>([]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent, index: number) => {
      const isVertical = window.matchMedia("(min-width: 640px)").matches;
      const prevKey = isVertical ? "ArrowUp" : "ArrowLeft";
      const nextKey = isVertical ? "ArrowDown" : "ArrowRight";

      let nextIndex: number | null = null;
      if (e.key === nextKey) {
        nextIndex = (index + 1) % SECTIONS.length;
      } else if (e.key === prevKey) {
        nextIndex = (index - 1 + SECTIONS.length) % SECTIONS.length;
      } else if (e.key === "Home") {
        nextIndex = 0;
      } else if (e.key === "End") {
        nextIndex = SECTIONS.length - 1;
      }

      if (nextIndex !== null) {
        e.preventDefault();
        const section = SECTIONS[nextIndex];
        onSectionChange(section.id);
        tabsRef.current[nextIndex]?.focus();
      }
    },
    [onSectionChange],
  );

  return (
    <nav
      role="tablist"
      aria-label="Settings sections"
      aria-orientation="vertical"
      className="
        flex sm:flex-col gap-1
        overflow-x-auto sm:overflow-x-visible
        pb-2 sm:pb-0 sm:pr-2
        border-b sm:border-b-0 sm:border-r border-border
        -mx-4 px-4 sm:mx-0 sm:px-0
        sm:min-w-[160px] sm:py-1
      "
    >
      {SECTIONS.map((section, i) => {
        const Icon = section.icon;
        const isActive = activeSection === section.id;
        return (
          <button
            key={section.id}
            ref={(el) => { tabsRef.current[i] = el; }}
            role="tab"
            id={`settings-tab-${section.id}`}
            aria-selected={isActive}
            aria-controls={`settings-panel-${section.id}`}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onSectionChange(section.id)}
            onKeyDown={(e) => handleKeyDown(e, i)}
            className={`
              flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md text-sm whitespace-nowrap
              transition-colors duration-[140ms]
              ${isActive
                ? "bg-accent-dim text-text-primary font-medium"
                : "text-text-secondary hover:text-text-primary hover:bg-surface-1"
              }
            `}
          >
            <Icon className="w-4 h-4 shrink-0" />
            <span>{section.label}</span>
          </button>
        );
      })}
    </nav>
  );
}
