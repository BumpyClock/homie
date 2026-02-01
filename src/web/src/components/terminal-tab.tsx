
import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

interface TerminalTabProps {
  sessionId: string;
  initialCols?: number;
  initialRows?: number;
  onInput: (data: string) => void;
  onResize: (cols: number, rows: number) => void;
  registerDataListener: (listener: (data: Uint8Array) => void) => () => void;
  active: boolean;
  theme?: "light" | "dark";
}

export function TerminalTab({
  sessionId,
  onInput,
  onResize,
  registerDataListener,
  active,
  theme = "dark",
}: TerminalTabProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 14,
      theme: theme === "light" 
        ? { background: "#ffffff", foreground: "#000000", cursor: "#333" } 
        : { background: "#1a1b26", foreground: "#a9b1d6", cursor: "#c0caf5" }, // Tokyo Night-ish
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
  }, [sessionId, onInput, onResize, registerDataListener, theme]); // Re-init if sessionId changes (shouldn't happen for same component instance usually)

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
