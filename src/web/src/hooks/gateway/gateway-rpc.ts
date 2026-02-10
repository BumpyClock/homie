import type { GatewayTransport } from "@homie/shared";

export function callGatewayRpc<TResult = unknown>(
  transport: GatewayTransport | null,
  method: string,
  params?: unknown,
): Promise<TResult> {
  if (!transport) {
    return Promise.reject(new Error("Not connected"));
  }
  return transport.call<TResult>(method, params);
}

export function sendGatewayBinary(
  transport: GatewayTransport | null,
  data: Uint8Array | ArrayBuffer,
) {
  if (!transport) return;
  transport.sendBinary(data);
}
