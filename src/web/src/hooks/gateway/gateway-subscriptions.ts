import type { GatewayTransport } from "@homie/shared";

export interface GatewayTopicEvent {
  topic: string;
  params?: unknown;
}

function noopUnsubscribe() {
  return () => undefined;
}

export function subscribeGatewayBinary(
  transport: GatewayTransport | null,
  callback: (data: ArrayBuffer) => void,
) {
  if (!transport) {
    return noopUnsubscribe();
  }
  return transport.onBinaryMessage(callback);
}

export function subscribeGatewayEvents(
  transport: GatewayTransport | null,
  callback: (event: GatewayTopicEvent) => void,
) {
  if (!transport) {
    return noopUnsubscribe();
  }
  return transport.onEvent((event) => {
    callback({ topic: event.topic, params: event.params });
  });
}
