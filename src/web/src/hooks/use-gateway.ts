import { useEffect, useRef, useState, useCallback } from "react";
import {
  type ClientHello,
  type HandshakeResponse,
  PROTOCOL_VERSION,
  type ServerHello,
  type HelloReject
} from "@/lib/protocol";
import { uuid } from "@/lib/uuid";

export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "handshaking"
  | "connected"
  | "error"
  | "rejected";

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
  
  const wsRef = useRef<WebSocket | null>(null);
  const binaryListeners = useRef<Set<(data: ArrayBuffer) => void>>(new Set());
  const retryCount = useRef(0);
  const mounted = useRef(true);
  const handshakeCompleted = useRef(false);
  const shouldReconnect = useRef(true);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const debug = useRef(false);
  
  const pendingRequests = useRef<Map<string, { resolve: (val: unknown) => void; reject: (err: unknown) => void }>>(new Map());

  const log = useCallback((...args: unknown[]) => {
    if (debug.current) {
      console.debug("[gateway]", ...args);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    shouldReconnect.current = true;
    retryCount.current = 0;
    debug.current = readDebugFlag();

    if (!url) {
      setStatus("disconnected");
      setError(null);
      setRejection(null);
      setServerHello(null);
      return () => {
        mounted.current = false;
        shouldReconnect.current = false;
      };
    }

    const connect = () => {
        if (!mounted.current) return;

        if (wsRef.current) {
          const state = wsRef.current.readyState;
          if (state === WebSocket.OPEN || state === WebSocket.CONNECTING) {
            log("skip connect: socket active");
            return;
          }
        }
        
        setStatus("connecting");
        setError(null);
        setRejection(null);
        setServerHello(null);
        log("connecting", { url, auth: authToken ? "set" : "none" });
        if (wsRef.current && wsRef.current.readyState === WebSocket.CLOSING) {
            wsRef.current.close();
        }

        try {
            const ws = new WebSocket(url);
            ws.binaryType = "arraybuffer";
            wsRef.current = ws;
            handshakeCompleted.current = false;

            ws.onopen = () => {
                if (!mounted.current) return;
                setStatus("handshaking");
                retryCount.current = 0;
                log("open");
                if (reconnectTimeoutRef.current) {
                  clearTimeout(reconnectTimeoutRef.current);
                  reconnectTimeoutRef.current = null;
                }

                const clientHello: ClientHello = {
                    protocol: { min: PROTOCOL_VERSION, max: PROTOCOL_VERSION },
                    client_id: "homie-web/0.0.1",
                    auth_token: authToken,
                    capabilities: ["terminal"],
                };

                log("send hello", { protocol: clientHello.protocol, client_id: clientHello.client_id });
                ws.send(JSON.stringify(clientHello));
            };

            ws.onmessage = (event) => {
                if (!mounted.current) return;

                if (event.data instanceof Blob) {
                    event.data.arrayBuffer().then((buffer) => {
                        binaryListeners.current.forEach((listener) => listener(buffer));
                    });
                    return;
                }
                
                if (event.data instanceof ArrayBuffer) {
                    binaryListeners.current.forEach((listener) => listener(event.data));
                    return;
                }

                let data;
                try {
                     data = JSON.parse(event.data);
                } catch (e) {
                     console.error("Failed to parse message", e);
                     log("parse failed", event.data);
                     return;
                }

                if (!handshakeCompleted.current) {
                    const response = data as HandshakeResponse;
                    if (response.type === "hello") {
                        handshakeCompleted.current = true;
                        setServerHello(response);
                        setStatus("connected");
                        log("handshake ok", { server_id: response.server_id, version: response.protocol_version });
                    } else if (response.type === "reject") {
                        shouldReconnect.current = false;
                        setRejection(response);
                        setStatus("rejected");
                        log("handshake rejected", response);
                        ws.close(); 
                    }
                    return;
                } else {
                    if (data?.type === "response" && data.id) {
                         const id = typeof data.id === "string" ? data.id : String(data.id);
                         const pending = pendingRequests.current.get(id);
                         if (pending) {
                             if (data.error) pending.reject(data.error);
                             else pending.resolve(data.result);
                             pendingRequests.current.delete(id);
                         }
                         log("response", { id, ok: !data.error });
                    } else if (data?.type === "event") {
                         log("event", { topic: data.topic });
                    }
                }
            };

            ws.onerror = (e) => {
                if (!mounted.current) return;
                console.error("WebSocket error", e);
                setError(e);
                setStatus("error");
                log("error", e);
            };

            ws.onclose = (event) => {
                if (!mounted.current) return;
                if (wsRef.current !== ws) {
                  return;
                }
                log("close", { code: event.code, reason: event.reason, wasClean: event.wasClean });
                wsRef.current = null;

                // Clear pending requests
                for (const pending of pendingRequests.current.values()) {
                    pending.reject(new Error("Connection closed"));
                }
                pendingRequests.current.clear();
                handshakeCompleted.current = false;

                setStatus((prev) => (prev === "rejected" ? prev : "disconnected"));

                if (shouldReconnect.current) {
                    const delay = Math.min(1000 * Math.pow(2, retryCount.current), 30000);
                    retryCount.current++;
                    
                    reconnectTimeoutRef.current = setTimeout(() => {
                        if (mounted.current) connect();
                    }, delay);
                }
            };
        } catch (err) {
            console.error("Failed to create WebSocket", err);
            setStatus("error");
            log("create failed", err);
            const delay = Math.min(1000 * Math.pow(2, retryCount.current), 30000);
            retryCount.current++;
            reconnectTimeoutRef.current = setTimeout(() => {
                if (mounted.current) connect();
            }, delay);
        }
    };

    connect();

    return () => {
        mounted.current = false;
        shouldReconnect.current = false;
        if (wsRef.current) {
            wsRef.current.close();
        }
        if (reconnectTimeoutRef.current) {
            clearTimeout(reconnectTimeoutRef.current);
        }
    };
  }, [url, authToken, log]);

  const call = useCallback((method: string, params?: unknown) => {
      return new Promise((resolve, reject) => {
          if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN || !handshakeCompleted.current) {
              reject(new Error("Not connected"));
              return;
          }
          const id = uuid();
          pendingRequests.current.set(id, { resolve, reject });
          log("request", { id, method });
          wsRef.current.send(JSON.stringify({ type: "request", id, method, params }));
      });
  }, [log]);

  const sendBinary = useCallback((data: Uint8Array | ArrayBuffer) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN && handshakeCompleted.current) {
        wsRef.current.send(data);
    }
  }, []);

  const onBinaryMessage = useCallback((callback: (data: ArrayBuffer) => void) => {
      binaryListeners.current.add(callback);
      return () => {
          binaryListeners.current.delete(callback);
      };
  }, []);

  return { status, serverHello, rejection, error, call, sendBinary, onBinaryMessage };
}
