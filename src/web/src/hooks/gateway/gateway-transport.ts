import { GatewayTransport, type GatewayTransportOptions } from "@homie/shared";

export interface UseGatewayTransportOptions {
  url: string;
  authToken?: string;
  logger?: (...args: unknown[]) => void;
}

export function readGatewayDebugFlag() {
  if (import.meta.env.VITE_DEBUG_WS === "1") return true;
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem("homie-debug") === "1";
  } catch {
    return false;
  }
}

export function createGatewayTransport({
  url,
  authToken,
  logger,
}: UseGatewayTransportOptions) {
  const options: GatewayTransportOptions = {
    url: url || "",
    authToken,
    clientId: "homie-web/0.0.1",
    capabilities: ["terminal", "chat"],
    logger,
    reconnect: true,
  };

  return new GatewayTransport(options);
}
