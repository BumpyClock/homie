import { useMemo, useState } from "react";

type MentionKind = "file" | "folder" | "skill";

interface MentionToken {
  kind: MentionKind;
  label: string;
  raw: string;
}

interface ChatComposerInputProps {
  value: string;
  disabled?: boolean;
  placeholder?: string;
  inputRef: React.RefObject<HTMLTextAreaElement | null>;
  onChange: (value: string, cursor: number) => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onKeyUp: (event: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onClick: (event: React.MouseEvent<HTMLTextAreaElement>) => void;
}

const TOKEN_REGEX = /\[(file|folder|skill):([^\]\n]+)\]/g;

function parseMentions(value: string): Array<string | MentionToken> {
  const tokens: Array<string | MentionToken> = [];
  let lastIndex = 0;
  for (const match of value.matchAll(TOKEN_REGEX)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      tokens.push(value.slice(lastIndex, index));
    }
    const kind = match[1] as MentionKind;
    const raw = match[2] ?? "";
    const label = raw.split("/").filter(Boolean).pop() ?? raw;
    tokens.push({ kind, label, raw });
    lastIndex = index + match[0].length;
  }
  if (lastIndex < value.length) {
    tokens.push(value.slice(lastIndex));
  }
  return tokens;
}

export function ChatComposerInput({
  value,
  disabled,
  placeholder,
  inputRef,
  onChange,
  onKeyDown,
  onKeyUp,
  onClick,
}: ChatComposerInputProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const tokens = useMemo(() => parseMentions(value), [value]);

  return (
    <div className="relative flex-1 min-h-[56px] max-h-[180px] resize-y rounded-md border border-border bg-background">
      <div className="pointer-events-none absolute inset-0 overflow-hidden px-3 py-2 text-sm text-foreground">
        <div
          className="whitespace-pre-wrap break-words"
          style={{ transform: `translateY(-${scrollTop}px)` }}
        >
          {tokens.map((token, index) => {
            if (typeof token === "string") {
              return <span key={`text-${index}`}>{token}</span>;
            }
            return (
              <span
                key={`mention-${index}-${token.raw}`}
                className="inline-flex items-center gap-1 rounded-[5px] border border-border/60 bg-muted/60 px-1.5 py-0.5 text-[12px] text-foreground align-middle"
                data-mention="true"
              >
                <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
                  {token.kind}
                </span>
                <span className="max-w-[220px] truncate">{token.label}</span>
              </span>
            );
          })}
        </div>
      </div>
      <textarea
        ref={inputRef}
        value={value}
        onChange={(e) => {
          const next = e.target.value;
          const cursor = e.target.selectionStart ?? next.length;
          onChange(next, cursor);
        }}
        onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        onClick={onClick}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        disabled={disabled}
        placeholder={placeholder}
        style={{ caretColor: "var(--color-foreground)" }}
        className={`relative z-10 w-full h-full min-h-[56px] max-h-[180px] resize-y bg-transparent px-3 py-2 text-sm focus:outline-none focus:border-primary disabled:opacity-60 ${
          value.length ? "text-transparent" : "text-foreground"
        } placeholder:text-muted-foreground/60`}
      />
    </div>
  );
}
