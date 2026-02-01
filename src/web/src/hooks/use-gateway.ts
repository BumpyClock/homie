import { useEffect, useRef, useState, useCallback } from "react";
import {
  type ClientHello,
  type HandshakeResponse,
  PROTOCOL_VERSION,
  type ServerHello,
  type HelloReject
} from "@/lib/protocol";

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
  
  const pendingRequests = useRef<Map<string, { resolve: (val: unknown) => void; reject: (err: unknown) => void }>>(new Map());

  useEffect(() => {
    mounted.current = true;
    shouldReconnect.current = true;
    retryCount.current = 0;

    const connect = () => {
        if (!mounted.current) return;
        
        setStatus("connecting");
        // Ensure we don't have multiple connections
        if (wsRef.current) {
            wsRef.current.close();
        }

        try {
            const ws = new WebSocket(url);
            wsRef.current = ws;
            handshakeCompleted.current = false;

            ws.onopen = () => {
                if (!mounted.current) return;
                setStatus("handshaking");
                retryCount.current = 0;

                const clientHello: ClientHello = {
                    protocol: { min: PROTOCOL_VERSION, max: PROTOCOL_VERSION },
                    client_id: "homie-web/0.0.1",
                    auth_token: authToken,
                    capabilities: ["terminal"],
                };

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
                     return;
                }

                if (!handshakeCompleted.current) {
                    const response = data as HandshakeResponse;
                    if (response.type === "hello") {
                        handshakeCompleted.current = true;
                        setServerHello(response);
                        setStatus("connected");
                    } else if (response.type === "reject") {
                        shouldReconnect.current = false;
                        setRejection(response);
                        setStatus("rejected");
                        ws.close(); 
                    }
                } else {
                    // RPC Response handling
                    if (data.id) {
                         const pending = pendingRequests.current.get(data.id);
                         if (pending) {
                             if (data.error) pending.reject(data.error);
                             else pending.resolve(data.result);
                             pendingRequests.current.delete(data.id);
                         }
                    }
                }
            };

            ws.onerror = (e) => {
                if (!mounted.current) return;
                console.error("WebSocket error", e);
                setError(e);
                setStatus("error");
            };

            ws.onclose = () => {
                if (!mounted.current) return;

                // Clear pending requests
                for (const pending of pendingRequests.current.values()) {
                    pending.reject(new Error("Connection closed"));
                }
                pendingRequests.current.clear();

                if (handshakeCompleted.current) {
                     setStatus("disconnected");
                }

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
  }, [url, authToken]);

  const call = useCallback((method: string, params?: unknown) => {
      return new Promise((resolve, reject) => {
          if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN || !handshakeCompleted.current) {
              reject(new Error("Not connected"));
              return;
          }
          const id = crypto.randomUUID();
          pendingRequests.current.set(id, { resolve, reject });
          wsRef.current.send(JSON.stringify({ id, method, params }));
      });
  }, []);

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
