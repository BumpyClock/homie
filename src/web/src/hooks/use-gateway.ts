import { useEffect, useRef, useState } from "react";
import {
  ClientHello,
  HandshakeResponse,
  PROTOCOL_VERSION,
  ServerHello,
  HelloReject
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
  const retryCount = useRef(0);
  const mounted = useRef(true);
  const handshakeCompleted = useRef(false);
  const shouldReconnect = useRef(true);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

                if (handshakeCompleted.current) {
                    return;
                }

                try {
                    const response: HandshakeResponse = JSON.parse(event.data);
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
                } catch (e) {
                    console.error("Failed to parse handshake response", e);
                    setStatus("error");
                    ws.close();
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

  return { status, serverHello, rejection, error };
}
