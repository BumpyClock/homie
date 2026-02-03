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
  const handshakeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const binaryListeners = useRef<Set<(data: ArrayBuffer) => void>>(new Set());
  const binaryBacklogRef = useRef<ArrayBuffer[]>([]);
  const binaryBacklogBytesRef = useRef(0);
  const eventListeners = useRef<Set<(event: { topic: string; params?: unknown }) => void>>(new Set());
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

  const clearBinaryBacklog = useCallback(() => {
    binaryBacklogRef.current = [];
    binaryBacklogBytesRef.current = 0;
  }, []);

  const clearHandshakeTimeout = useCallback(() => {
    if (handshakeTimeoutRef.current) {
      clearTimeout(handshakeTimeoutRef.current);
      handshakeTimeoutRef.current = null;
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    shouldReconnect.current = true;
    retryCount.current = 0;
    debug.current = readDebugFlag();

    if (!url) {
      const cleanup = () => {
        mounted.current = false;
        shouldReconnect.current = false;
        clearHandshakeTimeout();
        clearBinaryBacklog();
        const ws = wsRef.current;
        wsRef.current = null;
        handshakeCompleted.current = false;
        for (const pending of pendingRequests.current.values()) {
          pending.reject(new Error("Connection closed"));
        }
        pendingRequests.current.clear();
        if (ws) {
          try {
            ws.close();
          } catch {
            // ignore
          }
        }
      };
      setTimeout(() => {
        if (!mounted.current) return;
        setStatus("disconnected");
        setError(null);
        setRejection(null);
        setServerHello(null);
      }, 0);
      return cleanup;
    }

    const connect = () => {
        if (!mounted.current) return;

        if (wsRef.current) {
          const existing = wsRef.current;
          const state = existing.readyState;
          if (state === WebSocket.OPEN) {
            log("skip connect: socket open");
            return;
          }
          // Treat a stuck CONNECTING socket as replaceable (common in dev StrictMode).
          if (state === WebSocket.CONNECTING || state === WebSocket.CLOSING) {
            try {
              existing.close();
            } catch {
              // ignore
            }
          }
          wsRef.current = null;
          handshakeCompleted.current = false;
          clearHandshakeTimeout();
          clearBinaryBacklog();
        }
        
        setStatus("connecting");
        setError(null);
        setRejection(null);
        setServerHello(null);
        log("connecting", { url, auth: authToken ? "set" : "none" });

        try {
            const ws = new WebSocket(url);
            ws.binaryType = "arraybuffer";
            wsRef.current = ws;
            handshakeCompleted.current = false;
            clearHandshakeTimeout();
            clearBinaryBacklog();

            const dispatchBinary = (buffer: ArrayBuffer) => {
              const listenerCount = binaryListeners.current.size;
              if (listenerCount > 0) {
                binaryListeners.current.forEach((listener) => listener(buffer));
                return;
              }

              // UI not mounted yet: keep a small backlog so early PTY output isn't lost.
              const MAX_BACKLOG_BYTES = 1024 * 1024; // 1MB
              binaryBacklogRef.current.push(buffer);
              binaryBacklogBytesRef.current += buffer.byteLength;
              while (
                binaryBacklogRef.current.length > 0 &&
                binaryBacklogBytesRef.current > MAX_BACKLOG_BYTES
              ) {
                const dropped = binaryBacklogRef.current.shift();
                if (dropped) binaryBacklogBytesRef.current -= dropped.byteLength;
              }

              log("binary buffered", {
                bytes: buffer.byteLength,
                backlogFrames: binaryBacklogRef.current.length,
                backlogBytes: binaryBacklogBytesRef.current,
              });
            };

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
                    capabilities: ["terminal", "chat"],
                };

                log("send hello", { protocol: clientHello.protocol, client_id: clientHello.client_id });
                ws.send(JSON.stringify(clientHello));

                clearHandshakeTimeout();
                handshakeTimeoutRef.current = setTimeout(() => {
                  // If we never receive ServerHello, force a reconnect.
                  if (wsRef.current === ws && !handshakeCompleted.current) {
                    log("handshake timeout; closing");
                    try {
                      ws.close();
                    } catch {
                      // ignore
                    }
                  }
                }, 5000);
            };

            ws.onmessage = (event) => {
                if (!mounted.current) return;

                if (event.data instanceof Blob) {
                    event.data.arrayBuffer().then((buffer) => {
                        dispatchBinary(buffer);
                    });
                    return;
                }
                
                if (event.data instanceof ArrayBuffer) {
                    dispatchBinary(event.data);
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
                        clearHandshakeTimeout();
                        clearBinaryBacklog();
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
                         if (typeof data.topic === "string") {
                           eventListeners.current.forEach((listener) =>
                             listener({ topic: data.topic, params: data.params })
                           );
                         }
                    }
                }
            };

            ws.onerror = (e) => {
                console.error("WebSocket error", e);
                if (mounted.current) {
                  setError(e);
                  setStatus("error");
                }
                log("error", e);
                // Some browsers may not reliably fire `onclose` after an error; force teardown.
                try {
                  ws.close();
                } catch {
                  // ignore
                }
            };

            ws.onclose = (event) => {
                if (wsRef.current !== ws) {
                  return;
                }
                wsRef.current = null;
                handshakeCompleted.current = false;
                clearHandshakeTimeout();
                clearBinaryBacklog();

                // Clear pending requests
                for (const pending of pendingRequests.current.values()) {
                    pending.reject(new Error("Connection closed"));
                }
                pendingRequests.current.clear();

                if (!mounted.current) return;

                log("close", { code: event.code, reason: event.reason, wasClean: event.wasClean });
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

    const pendingRequestsRef = pendingRequests.current;
    return () => {
      mounted.current = false;
      shouldReconnect.current = false;
      clearHandshakeTimeout();
      clearBinaryBacklog();
      if (wsRef.current) {
        wsRef.current.close();
      }
      wsRef.current = null;
      handshakeCompleted.current = false;
      for (const pending of pendingRequestsRef.values()) {
        pending.reject(new Error("Connection closed"));
      }
      pendingRequestsRef.clear();
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
    };
  }, [url, authToken, log, clearHandshakeTimeout, clearBinaryBacklog]);

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
      // Flush any buffered frames so the terminal renders initial output.
      if (binaryBacklogRef.current.length > 0) {
        const buffered = binaryBacklogRef.current;
        binaryBacklogRef.current = [];
        binaryBacklogBytesRef.current = 0;
        for (const buffer of buffered) {
          callback(buffer);
        }
        log("binary backlog flushed", { frames: buffered.length });
      }
      return () => {
          binaryListeners.current.delete(callback);
      };
  }, [log]);

  const onEvent = useCallback((callback: (event: { topic: string; params?: unknown }) => void) => {
    eventListeners.current.add(callback);
    return () => {
      eventListeners.current.delete(callback);
    };
  }, []);

  return { status, serverHello, rejection, error, call, sendBinary, onBinaryMessage, onEvent };
}
