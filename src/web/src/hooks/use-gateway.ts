import { useCallback, useEffect, useRef, useState } from "react";
import {
  type ConnectionStatus,
  type GatewayTransport,
  type GatewayTransportState,
  type HelloReject,
  type ServerHello,
} from "@homie/shared";
import { callGatewayRpc, sendGatewayBinary } from "@/hooks/gateway/gateway-rpc";
import {
  type GatewayTopicEvent,
  subscribeGatewayBinary,
  subscribeGatewayEvents,
} from "@/hooks/gateway/gateway-subscriptions";
import {
  createGatewayTransport,
  readGatewayDebugFlag,
} from "@/hooks/gateway/gateway-transport";

interface UseGatewayOptions {
  url: string;
  authToken?: string;
}

function applyGatewayState(
  nextState: GatewayTransportState,
  setStatus: (status: ConnectionStatus) => void,
  setServerHello: (value: ServerHello | null) => void,
  setRejection: (value: HelloReject | null) => void,
  setError: (value: Event | null) => void,
) {
  setStatus(nextState.status);
  setServerHello(nextState.serverHello);
  setRejection(nextState.rejection);
  setError((nextState.error as Event | null) ?? null);
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
    debugRef.current = readGatewayDebugFlag();

    const transport = createGatewayTransport({
      url,
      authToken,
      logger: (...args: unknown[]) => log(...args),
    });

    transportRef.current = transport;

    const unsubscribeState = transport.onStateChange((nextState) => {
      applyGatewayState(nextState, setStatus, setServerHello, setRejection, setError);
    });

    transport.start();

    return () => {
      unsubscribeState();
      transport.stop();
      transportRef.current = null;
    };
  }, [authToken, log, url]);

  const call = useCallback((method: string, params?: unknown) => {
    return callGatewayRpc(transportRef.current, method, params);
  }, []);

  const sendBinary = useCallback((data: Uint8Array | ArrayBuffer) => {
    sendGatewayBinary(transportRef.current, data);
  }, []);

  const onBinaryMessage = useCallback((callback: (data: ArrayBuffer) => void) => {
    return subscribeGatewayBinary(transportRef.current, callback);
  }, []);

  const onEvent = useCallback((callback: (event: GatewayTopicEvent) => void) => {
    return subscribeGatewayEvents(transportRef.current, callback);
  }, []);

  return { status, serverHello, rejection, error, call, sendBinary, onBinaryMessage, onEvent };
}

export type { ConnectionStatus, GatewayTopicEvent };
