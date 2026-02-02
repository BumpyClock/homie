
import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { useTheme } from "@/hooks/use-theme";
import { DEFAULT_PREVIEW_LINES, savePreview } from "@/lib/session-previews";

interface TerminalTabProps {
  sessionId: string;
  initialCols?: number;
  initialRows?: number;
  onInput: (data: string) => void;
  onResize: (cols: number, rows: number) => void;
  registerDataListener: (listener: (data: Uint8Array) => void) => () => void;
  active: boolean;
  previewNamespace: string;
}

// Helper to get HSL color string from CSS variable
function getThemeColor(variable: string): string {
  const root = document.documentElement;
  const value = getComputedStyle(root).getPropertyValue(variable).trim();
  if (!value) return "#000000";
  return `hsl(${value})`;
}

function getThemeColorAlpha(variable: string, alpha: number): string {
  const root = document.documentElement;
  const value = getComputedStyle(root).getPropertyValue(variable).trim();
  if (!value) return `rgba(0, 0, 0, ${alpha})`;
  return `hsl(${value} / ${alpha})`;
}

export function TerminalTab({
  sessionId,
  onInput,
  onResize,
  registerDataListener,
  active,
  previewNamespace,
}: TerminalTabProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const { resolvedTheme, colorScheme } = useTheme();

  // Update theme when it changes
  useEffect(() => {
    if (!terminalRef.current) return;

    const bg = getThemeColor("--background");
    const fg = getThemeColor("--foreground");
    const cursor = getThemeColor("--primary");

    const theme = {
      background: bg,
      foreground: fg,
      cursor: cursor,
      selectionBackground: getThemeColorAlpha(
        "--primary",
        resolvedTheme === "dark" ? 0.25 : 0.18
      ),

      black: getThemeColor("--term-black"),
      red: getThemeColor("--term-red"),
      green: getThemeColor("--term-green"),
      yellow: getThemeColor("--term-yellow"),
      blue: getThemeColor("--term-blue"),
      magenta: getThemeColor("--term-magenta"),
      cyan: getThemeColor("--term-cyan"),
      white: getThemeColor("--term-white"),

      brightBlack: getThemeColor("--term-bright-black"),
      brightRed: getThemeColor("--term-bright-red"),
      brightGreen: getThemeColor("--term-bright-green"),
      brightYellow: getThemeColor("--term-bright-yellow"),
      brightBlue: getThemeColor("--term-bright-blue"),
      brightMagenta: getThemeColor("--term-bright-magenta"),
      brightCyan: getThemeColor("--term-bright-cyan"),
      brightWhite: getThemeColor("--term-bright-white"),
    };

    terminalRef.current.options.theme = theme;
  }, [resolvedTheme, colorScheme]);

  useEffect(() => {
    if (!containerRef.current) return;

    let disposed = false;
    let cleanup: (() => void) | null = null;

    const start = async () => {
      if (disposed || !containerRef.current) return;

      // Initial theme setup
      const bg = getThemeColor("--background");
      const fg = getThemeColor("--foreground");
      const cursor = getThemeColor("--primary");

      const theme = {
        background: bg,
        foreground: fg,
        cursor: cursor,
        cursorAccent: bg,
        selectionBackground: getThemeColorAlpha(
          "--primary",
          resolvedTheme === "dark" ? 0.25 : 0.18
        ),

        black: getThemeColor("--term-black"),
        red: getThemeColor("--term-red"),
        green: getThemeColor("--term-green"),
        yellow: getThemeColor("--term-yellow"),
        blue: getThemeColor("--term-blue"),
        magenta: getThemeColor("--term-magenta"),
        cyan: getThemeColor("--term-cyan"),
        white: getThemeColor("--term-white"),

        brightBlack: getThemeColor("--term-bright-black"),
        brightRed: getThemeColor("--term-bright-red"),
        brightGreen: getThemeColor("--term-bright-green"),
        brightYellow: getThemeColor("--term-bright-yellow"),
        brightBlue: getThemeColor("--term-bright-blue"),
        brightMagenta: getThemeColor("--term-bright-magenta"),
        brightCyan: getThemeColor("--term-bright-cyan"),
        brightWhite: getThemeColor("--term-bright-white"),
      };

      const term = new Terminal({
        cursorBlink: true,
        fontFamily: 'Menlo, Monaco, "Courier New", monospace',
        fontSize: 14,
        theme,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);

      term.open(containerRef.current);
      fitAddon.fit();

      term.onData((data) => {
        onInput(data);
      });

      term.onResize((size) => {
        onResize(size.cols, size.rows);
      });

      terminalRef.current = term;
      fitAddonRef.current = fitAddon;

      // Report initial size
      onResize(term.cols, term.rows);

      cleanup = registerDataListener((data) => {
        term.write(data);
      });
    };

    void start();

    return () => {
      disposed = true;
      try {
        const term = terminalRef.current;
        if (term) {
          const snapshot = snapshotTerminal(term, DEFAULT_PREVIEW_LINES);
          if (snapshot.trim().length > 0) {
            savePreview(previewNamespace, sessionId, snapshot);
          }
        }
      } catch {
        // ignore snapshot failures
      }
      cleanup?.();
      cleanup = null;
      terminalRef.current?.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, onInput, onResize, registerDataListener]); // Removed theme deps from init to avoid re-creation

  // Handle resizing and visibility
  useEffect(() => {
    if (active && fitAddonRef.current && terminalRef.current) {
      // Need a slight delay or requestAnimationFrame to ensure container is visible/sized
      requestAnimationFrame(() => {
        fitAddonRef.current?.fit();
        terminalRef.current?.focus();
      });
    }
  }, [active]);

  // Handle window resize
  useEffect(() => {
    const handleResize = () => {
      if (active && fitAddonRef.current) {
        fitAddonRef.current.fit();
      }
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [active]);

  return (
    <div 
      ref={containerRef} 
      className={`w-full h-full overflow-hidden ${active ? 'block' : 'hidden'}`}
      style={{ minHeight: '0' }}
    />
  );
}

function snapshotTerminal(term: Terminal, maxLines: number): string {
  const buffer = term.buffer.active;
  const total = buffer.length;
  const start = Math.max(0, total - maxLines);
  const lines: string[] = [];
  for (let i = start; i < total; i += 1) {
    const line = buffer.getLine(i);
    if (!line) continue;
    lines.push(line.translateToString(true));
  }
  return lines.join("\n").trimEnd();
}
