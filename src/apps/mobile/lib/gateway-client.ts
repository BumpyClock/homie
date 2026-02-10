import { GatewayTransport } from "@homie/shared";

export interface MobileGatewayClientOptions {
  url: string;
  authToken?: string;
}

export function createMobileGatewayClient(options: MobileGatewayClientOptions) {
  return new GatewayTransport({
    url: options.url,
    authToken: options.authToken,
    clientId: "homie-mobile/0.1.0",
    capabilities: ["chat", "terminal"],
    reconnect: true,
  });
}
