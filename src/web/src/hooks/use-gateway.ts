import { useCallback, useEffect, useRef, useState } from "react";
import {
  GatewayTransport,
  type ConnectionStatus,
  type HelloReject,
  type RpcEvent,
  type ServerHello,
} from "@homie/shared";

interface UseGatewayOptions {
  url: string;
  authToken?: string;
}

function readDebugFlag() {
  if (import.meta.env.VITE_DEBUG_WS === "1") return true;
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem("homie-debug") === "1";
  } catch {
    return false;
  }
}

export function useGateway({ url, authToken }: UseGatewayOptions) {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [serverHello, setServerHello] = useState<ServerHello | null>(null);
  const [rejection, setRejection] = useState<HelloReject | null>(null);
  const [error, setError] = useState<Event | null>(null);

  const transportRef = useRef<GatewayTransport | null>(null);
  const debugRef = useRef(false);

  const log = useCallback((...args: unknown[]) => {
    if (debugRef.current) {
      console.debug("[gateway]", ...args);
    }
  }, []);

  useEffect(() => {
    debugRef.current = readDebugFlag();

    const transport = new GatewayTransport({
      url: url || "",
      authToken,
      clientId: "homie-web/0.0.1",
      capabilities: ["terminal", "chat"],
      logger: (...args: unknown[]) => log(...args),
      reconnect: true,
    });

    transportRef.current = transport;

    const unsubscribeState = transport.onStateChange((nextState) => {
      setStatus(nextState.status);
      setServerHello(nextState.serverHello);
      setRejection(nextState.rejection);
      setError((nextState.error as Event | null) ?? null);
    });

    transport.start();

    return () => {
      unsubscribeState();
      transport.stop();
      transportRef.current = null;
    };
  }, [authToken, log, url]);

  const call = useCallback((method: string, params?: unknown) => {
    const transport = transportRef.current;
    if (!transport) {
      return Promise.reject(new Error("Not connected"));
    }
    return transport.call(method, params);
  }, []);

  const sendBinary = useCallback((data: Uint8Array | ArrayBuffer) => {
    const transport = transportRef.current;
    if (!transport) return;
    transport.sendBinary(data);
  }, []);

  const onBinaryMessage = useCallback((callback: (data: ArrayBuffer) => void) => {
    const transport = transportRef.current;
    if (!transport) {
      return () => undefined;
    }
    return transport.onBinaryMessage(callback);
  }, []);

  const onEvent = useCallback((callback: (event: { topic: string; params?: unknown }) => void) => {
    const transport = transportRef.current;
    if (!transport) {
      return () => undefined;
    }
    return transport.onEvent((event: RpcEvent) => {
      callback({ topic: event.topic, params: event.params });
    });
  }, []);

  return { status, serverHello, rejection, error, call, sendBinary, onBinaryMessage, onEvent };
}
