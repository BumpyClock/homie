import { FileText, Folder, Sparkles } from "lucide-react";
import type { FileOption, SkillOption } from "@/lib/chat-utils";

type TriggerType = "slash" | "mention";

interface ChatInlineMenuProps {
  trigger: { type: TriggerType } | null;
  visible: boolean;
  menuIndex: number;
  position: { x: number; y: number };
  skillOptions: SkillOption[];
  mentionSkillOptions: SkillOption[];
  mentionOptions: FileOption[];
  attachedFolder?: string | null;
  onSelectSkill: (skill: SkillOption) => void;
  onSelectFile: (file: FileOption) => void;
  onSelectFolder: (file: FileOption) => void;
  onHoverIndex: (index: number) => void;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

export function ChatInlineMenu({
  trigger,
  visible,
  menuIndex,
  position,
  skillOptions,
  mentionSkillOptions,
  mentionOptions,
  attachedFolder,
  onSelectSkill,
  onSelectFile,
  onSelectFolder,
  onHoverIndex,
}: ChatInlineMenuProps) {
  if (!trigger) return null;
  if (typeof window === "undefined") return null;

  const baseWidth = trigger.type === "mention" ? 360 : 300;
  const maxWidth = clamp(baseWidth, 220, window.innerWidth - 24);
  const left = clamp(Math.round(position.x) - 12, 12, window.innerWidth - maxWidth - 12);
  const bottomRaw = window.innerHeight - Math.round(position.y) + 8;
  const bottom = clamp(bottomRaw, 12, window.innerHeight - 12);

  return (
    <div
      className={`fixed z-50 overflow-hidden rounded-lg border border-border bg-background shadow-lg transition-[opacity,transform] duration-150 ease-out origin-bottom-left motion-reduce:transition-none ${
        visible ? "opacity-100 translate-y-0 scale-100" : "opacity-0 translate-y-2 scale-[0.98]"
      }`}
      style={{ left, bottom, width: maxWidth, maxWidth }}
    >
      <div className="max-h-[240px] overflow-y-auto py-1">
        {trigger.type === "slash" && (
          <>
            <div className="px-3 py-1.5 text-[11px] uppercase tracking-wide text-muted-foreground border-b border-border/60">
              Skills
            </div>
            {skillOptions.length === 0 && (
              <div className="px-3 py-2 text-xs text-muted-foreground">No skills found.</div>
            )}
            {skillOptions.map((skill, idx) => (
              <button
                key={skill.name}
                type="button"
                onClick={() => onSelectSkill(skill)}
                onMouseEnter={() => onHoverIndex(idx)}
                className={`w-full text-left px-3 py-2 text-sm flex items-center gap-2 ${
                  idx === menuIndex ? "bg-muted/60" : "hover:bg-muted/40"
                }`}
              >
                <Sparkles className="h-4 w-4 text-muted-foreground" />
                <div className="min-w-0">
                  <div className="truncate">{skill.name}</div>
                  {skill.description && (
                    <div className="text-xs text-muted-foreground truncate">{skill.description}</div>
                  )}
                </div>
              </button>
            ))}
          </>
        )}
        {trigger.type === "mention" && (
          <>
            <div className="px-3 py-1.5 text-[11px] uppercase tracking-wide text-muted-foreground border-b border-border/60">
              Skills
            </div>
            {mentionSkillOptions.length === 0 && (
              <div className="px-3 py-2 text-xs text-muted-foreground">No skills found.</div>
            )}
            {mentionSkillOptions.map((skill, idx) => (
              <button
                key={skill.name}
                type="button"
                onClick={() => onSelectSkill(skill)}
                onMouseEnter={() => onHoverIndex(idx)}
                className={`w-full text-left px-3 py-2 text-sm flex items-center gap-2 ${
                  idx === menuIndex ? "bg-muted/60" : "hover:bg-muted/40"
                }`}
              >
                <Sparkles className="h-4 w-4 text-muted-foreground" />
                <div className="min-w-0">
                  <div className="truncate">{skill.name}</div>
                  {skill.description && (
                    <div className="text-xs text-muted-foreground truncate">{skill.description}</div>
                  )}
                </div>
              </button>
            ))}
            <div className="px-3 py-1.5 text-[11px] uppercase tracking-wide text-muted-foreground border-b border-border/60">
              Files
            </div>
            {!attachedFolder && (
              <div className="px-3 py-2 text-xs text-muted-foreground">
                Attach a folder to mention files.
              </div>
            )}
            {attachedFolder && mentionOptions.length === 0 && (
              <div className="px-3 py-2 text-xs text-muted-foreground">No matching files.</div>
            )}
            {mentionOptions.map((file, idx) => {
              const absoluteIndex = mentionSkillOptions.length + idx;
              return (
                <button
                  key={file.path}
                  type="button"
                  onClick={() => {
                    if (file.type === "directory") {
                      onSelectFolder(file);
                    } else {
                      onSelectFile(file);
                    }
                  }}
                  onMouseEnter={() => onHoverIndex(absoluteIndex)}
                  className={`w-full text-left px-3 py-2 text-sm flex items-center gap-2 ${
                    absoluteIndex === menuIndex ? "bg-muted/60" : "hover:bg-muted/40"
                  }`}
                >
                  {file.type === "directory" ? (
                    <Folder className="h-4 w-4 text-muted-foreground" />
                  ) : (
                    <FileText className="h-4 w-4 text-muted-foreground" />
                  )}
                  <div className="min-w-0">
                    <div className="truncate">{file.name}</div>
                    <div className="text-xs text-muted-foreground truncate">{file.relativePath}</div>
                  </div>
                </button>
              );
            })}
          </>
        )}
      </div>
      <div className="border-t border-border px-3 py-2 text-[11px] text-muted-foreground">
        {trigger.type === "slash" ? "Use / to insert skills" : "Use @ for skills and files"}
      </div>
    </div>
  );
}
