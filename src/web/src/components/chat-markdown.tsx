import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import { FileText, Folder, Sparkles } from "lucide-react";

interface ChatMarkdownProps {
  content: string;
  className?: string;
  compact?: boolean;
}

export function ChatMarkdown({ content, className, compact }: ChatMarkdownProps) {
  if (!content) return null;
  const rendered = decorateMentions(content);
  return (
    <div className={className}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw]}
        components={{
          span: ({ children, ...props }) => {
            const mention = (props as Record<string, unknown>)["data-mention"];
            const labelRaw = (props as Record<string, unknown>)["data-label"];
            if (typeof mention === "string" && typeof labelRaw === "string") {
              return <MentionBadge kind={mention} label={labelRaw} />;
            }
            return <span {...props}>{children}</span>;
          },
          p: ({ children }) => (
            <p className={`${compact ? "mb-2" : "mb-3"} leading-relaxed last:mb-0`}>{children}</p>
          ),
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="underline decoration-muted-foreground/40 underline-offset-2 hover:text-foreground"
            >
              {children}
            </a>
          ),
          ul: ({ children }) => <ul className="mb-3 list-disc pl-5">{children}</ul>,
          ol: ({ children }) => <ol className="mb-3 list-decimal pl-5">{children}</ol>,
          li: ({ children }) => <li className="mb-1">{children}</li>,
          code: ({ className: codeClass, children }) => {
            if (codeClass) {
              return (
                <code className={`text-xs font-mono ${codeClass}`}>{children}</code>
              );
            }
            return (
              <code className="rounded bg-muted/60 px-1 py-0.5 text-xs font-mono">
                {children}
              </code>
            );
          },
          pre: ({ children }) => (
            <pre className="mb-3 overflow-x-auto rounded-md border border-border bg-muted/40 p-3 text-xs font-mono leading-relaxed">
              {children}
            </pre>
          ),
          blockquote: ({ children }) => (
            <blockquote className="border-l-2 border-border pl-3 text-muted-foreground">
              {children}
            </blockquote>
          ),
          h1: ({ children }) => <h1 className="mb-2 text-lg font-semibold">{children}</h1>,
          h2: ({ children }) => <h2 className="mb-2 text-base font-semibold">{children}</h2>,
          h3: ({ children }) => <h3 className="mb-2 text-sm font-semibold">{children}</h3>,
          hr: () => <hr className="my-3 border-border" />,
        }}
      >
        {rendered}
      </ReactMarkdown>
    </div>
  );
}

const MENTION_REGEX = /\[(file|folder|skill):([^\]\n]+)\]/g;

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function decorateMentions(value: string) {
  return value.replace(MENTION_REGEX, (_match, kind, rawLabel) => {
    const safeLabel = escapeHtml(String(rawLabel));
    return `<span data-mention="${kind}" data-label="${safeLabel}"></span>`;
  });
}

function formatMentionLabel(kind: string, label: string) {
  if (kind === "skill") {
    const segments = label.split(":").filter(Boolean);
    return segments[segments.length - 1] ?? label;
  }
  const parts = label.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? label;
}

function MentionBadge({ kind, label }: { kind: string; label: string }) {
  const display = formatMentionLabel(kind, label);
  const icon =
    kind === "folder" ? (
      <Folder className="h-3.5 w-3.5 text-muted-foreground" />
    ) : kind === "skill" ? (
      <Sparkles className="h-3.5 w-3.5 text-muted-foreground" />
    ) : (
      <FileText className="h-3.5 w-3.5 text-muted-foreground" />
    );
  return (
    <span
      className="inline-flex items-center gap-1 rounded-[5px] border border-border/60 bg-muted/50 px-1.5 py-0.5 text-[12px] text-foreground align-middle"
      title={label}
      data-mention="true"
    >
      {icon}
      <span className="max-w-[220px] truncate">{display}</span>
    </span>
  );
}
