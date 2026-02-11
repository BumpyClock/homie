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
  action?: React.ReactNode;
}

const TOKEN_REGEX = /\[(file|folder|skill):([^\]\n]+)\]|(^|[\s(])\$([A-Za-z][\w-]*)/gm;

function parseMentions(value: string): Array<string | MentionToken> {
  const tokens: Array<string | MentionToken> = [];
  let lastIndex = 0;
  for (const match of value.matchAll(TOKEN_REGEX)) {
    const index = match.index ?? 0;
    if (index > lastIndex) {
      tokens.push(value.slice(lastIndex, index));
    }
    if (match[1] && match[2]) {
      const kind = match[1] as MentionKind;
      const raw = match[2] ?? "";
      const label = raw.split("/").filter(Boolean).pop() ?? raw;
      tokens.push({ kind, label, raw });
    } else {
      const prefix = match[3] ?? "";
      const raw = match[4] ?? "";
      if (prefix) {
        tokens.push(prefix);
      }
      tokens.push({ kind: "skill", label: raw, raw });
    }
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
  action,
}: ChatComposerInputProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const tokens = useMemo(() => parseMentions(value), [value]);

  return (
    <div className="relative flex-1 min-h-[44px] max-h-[180px] rounded-xl border border-border/60 bg-card/30 transition-[border-color,box-shadow] duration-150 ease-out focus-within:border-primary/50 focus-within:shadow-[0_0_0_3px_hsl(var(--primary)/0.12)] motion-reduce:transition-none">
      <div className="pointer-events-none absolute inset-0 overflow-hidden px-3 py-2 text-sm text-foreground">
        <div
          className="whitespace-pre-wrap break-words sm:pr-12"
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
        className={`relative z-10 w-full h-full min-h-[44px] max-h-[180px] resize-none bg-transparent px-3 py-2 sm:pr-12 text-sm focus:outline-none disabled:opacity-60 ${
          value.length ? "text-transparent" : "text-foreground"
        } placeholder:text-muted-foreground/60`}
      />
      {action ? <div className="absolute right-2 bottom-2 z-20 hidden sm:flex">{action}</div> : null}
    </div>
  );
}
