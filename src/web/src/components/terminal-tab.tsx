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

function buildTheme(resolvedTheme: string) {
  return {
    background: getThemeColor("--background"),
    foreground: getThemeColor("--foreground"),
    cursor: getThemeColor("--primary"),
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
  const { resolvedTheme } = useTheme();

  useEffect(() => {
    if (!terminalRef.current) return;
    terminalRef.current.options.theme = buildTheme(resolvedTheme);
  }, [resolvedTheme]);

  useEffect(() => {
    if (!containerRef.current) return;

    let cleanup: (() => void) | null = null;

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 14,
      theme: buildTheme(resolvedTheme),
      allowProposedApi: false,
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

    // Ensure server knows our initial size.
    onResize(term.cols, term.rows);

    cleanup = registerDataListener((data) => {
      term.write(data);
    });

    return () => {
      try {
        const snapshot = snapshotTerminal(term, DEFAULT_PREVIEW_LINES);
        if (snapshot.trim().length > 0) {
          savePreview(previewNamespace, sessionId, snapshot);
        }
      } catch {
        // ignore
      }

      cleanup?.();
      cleanup = null;
      term.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, onInput, onResize, registerDataListener, previewNamespace]);

  useEffect(() => {
    if (!active || !fitAddonRef.current || !terminalRef.current) return;
    requestAnimationFrame(() => {
      fitAddonRef.current?.fit();
      terminalRef.current?.focus();
    });
  }, [active]);

  useEffect(() => {
    const handleResize = () => {
      if (active && fitAddonRef.current) fitAddonRef.current.fit();
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [active]);

  return (
    <div
      ref={containerRef}
      className={`w-full h-full overflow-hidden ${active ? "block" : "hidden"}`}
      style={{ minHeight: "0" }}
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

