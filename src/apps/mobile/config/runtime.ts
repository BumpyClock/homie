const defaultGatewayUrl = 'ws://127.0.0.1:9800/ws';

export const runtimeConfig = {
  gatewayUrl: process.env.EXPO_PUBLIC_HOMIE_GATEWAY_URL ?? defaultGatewayUrl,
};
