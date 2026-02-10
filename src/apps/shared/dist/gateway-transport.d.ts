import { type HelloReject, type RpcEvent, type ServerHello } from "./protocol";
export type ConnectionStatus = "disconnected" | "connecting" | "handshaking" | "connected" | "error" | "rejected";
export interface GatewayTransportState {
    status: ConnectionStatus;
    serverHello: ServerHello | null;
    rejection: HelloReject | null;
    error: unknown;
}
export interface ReconnectBackoffOptions {
    baseDelayMs?: number;
    maxDelayMs?: number;
}
export interface GatewayCloseEventLike {
    code?: number;
    reason?: string;
    wasClean?: boolean;
}
export interface GatewaySocketLike {
    readyState: number;
    binaryType?: "blob" | "arraybuffer";
    onopen: ((event: unknown) => void) | null;
    onmessage: ((event: unknown) => void) | null;
    onerror: ((event: unknown) => void) | null;
    onclose: ((event: GatewayCloseEventLike) => void) | null;
    send(data: string | ArrayBuffer | ArrayBufferView | Blob): void;
    close(code?: number, reason?: string): void;
}
export interface GatewayTransportOptions {
    url: string;
    authToken?: string;
    protocolVersion?: number;
    clientId?: string;
    capabilities?: string[];
    handshakeTimeoutMs?: number;
    maxBinaryBacklogBytes?: number;
    reconnect?: boolean;
    reconnectBackoff?: ReconnectBackoffOptions;
    createRequestId?: () => string;
    createWebSocket?: (url: string) => GatewaySocketLike;
    logger?: (...args: unknown[]) => void;
}
export type Unsubscribe = () => void;
export declare const DEFAULT_HANDSHAKE_TIMEOUT_MS = 5000;
export declare const DEFAULT_MAX_BINARY_BACKLOG_BYTES: number;
export declare const DEFAULT_BASE_RECONNECT_DELAY_MS = 1000;
export declare const DEFAULT_MAX_RECONNECT_DELAY_MS = 30000;
export declare function createRequestId(): string;
export declare function getReconnectDelayMs(retryCount: number, options?: ReconnectBackoffOptions): number;
export declare class GatewayTransport {
    private readonly options;
    private socket;
    private handshakeTimeout;
    private reconnectTimeout;
    private handshakeCompleted;
    private retryCount;
    private running;
    private shouldReconnect;
    private status;
    private serverHello;
    private rejection;
    private error;
    private readonly pendingRequests;
    private readonly binaryListeners;
    private readonly eventListeners;
    private readonly stateListeners;
    private binaryBacklog;
    private binaryBacklogBytes;
    constructor(options: GatewayTransportOptions);
    start(): void;
    stop(): void;
    setConnection(url: string, authToken?: string): void;
    getState(): GatewayTransportState;
    onStateChange(listener: (state: GatewayTransportState) => void): Unsubscribe;
    onBinaryMessage(listener: (data: ArrayBuffer) => void): Unsubscribe;
    onEvent(listener: (event: RpcEvent) => void): Unsubscribe;
    call<TResult = unknown>(method: string, params?: unknown): Promise<TResult>;
    sendBinary(data: Uint8Array | ArrayBuffer): void;
    private connect;
    private scheduleReconnect;
    private dispatchBinary;
    private clearBinaryBacklog;
    private clearHandshakeTimeout;
    private clearReconnectTimeout;
    private setStatus;
    private resetState;
    private emitState;
    private log;
}
//# sourceMappingURL=gateway-transport.d.ts.map