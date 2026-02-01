
import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { useTheme } from "@/hooks/use-theme";

interface TerminalTabProps {
  sessionId: string;
  initialCols?: number;
  initialRows?: number;
  onInput: (data: string) => void;
  onResize: (cols: number, rows: number) => void;
  registerDataListener: (listener: (data: Uint8Array) => void) => () => void;
  active: boolean;
}

// Helper to get HSL color string from CSS variable
function getThemeColor(variable: string): string {
  const root = document.documentElement;
  const value = getComputedStyle(root).getPropertyValue(variable);
  if (!value) return "#000000";
  return `hsl(${value})`;
}

export function TerminalTab({
  sessionId,
  onInput,
  onResize,
  registerDataListener,
  active,
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
    // const selection = getThemeColor("--primary"); // with opacity usually, but xterm handles selection style separately

    terminalRef.current.options.theme = {
        background: bg,
        foreground: fg,
        cursor: cursor,
        selectionBackground: resolvedTheme === 'dark' ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.3)',
    };
  }, [resolvedTheme, colorScheme]);

  useEffect(() => {
    if (!containerRef.current) return;

    // Initial theme setup
    const bg = getThemeColor("--background");
    const fg = getThemeColor("--foreground");
    const cursor = getThemeColor("--primary");

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 14,
      theme: {
        background: bg,
        foreground: fg,
        cursor: cursor,
        selectionBackground: resolvedTheme === 'dark' ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.3)',
      },
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);

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

    const cleanup = registerDataListener((data) => {
      term.write(data);
    });

    return () => {
      cleanup();
      term.dispose();
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
