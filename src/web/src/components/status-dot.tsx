import type { ConnectionStatus } from "@/hooks/use-gateway";

export function StatusDot({ status, className }: { status: ConnectionStatus; className?: string }) {
  const color =
    status === "connected"
      ? "bg-green-500"
      : status === "connecting" || status === "handshaking"
        ? "bg-yellow-500"
        : "bg-red-500";

  const shouldPulse = status === "connecting" || status === "handshaking";

  return (
    <span
      className={`inline-block rounded-full ${color} ${shouldPulse ? "animate-pulse motion-reduce:animate-none" : ""} ${className ?? "h-2.5 w-2.5"}`}
      role="img"
      aria-label={`Connection status: ${status}`}
      title={`Connection: ${status}`}
    />
  );
}
