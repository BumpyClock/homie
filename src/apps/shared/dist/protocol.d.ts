export interface VersionRange {
    min: number;
    max: number;
}
export interface ClientHello {
    protocol: VersionRange;
    client_id: string;
    auth_token?: string;
    capabilities: string[];
}
export interface ServiceCapability {
    service: string;
    version: string;
}
export interface ServerHello {
    protocol_version: number;
    server_id: string;
    identity?: string;
    services: ServiceCapability[];
}
export type HelloRejectCode = "version_mismatch" | "unauthorized" | "server_error";
export interface HelloReject {
    code: HelloRejectCode;
    reason: string;
}
export type HandshakeResponse = ({
    type: "hello";
} & ServerHello) | ({
    type: "reject";
} & HelloReject);
export declare const PROTOCOL_VERSION = 1;
export interface RpcError {
    code: number;
    message: string;
    data?: unknown;
}
export interface RpcRequest<TParams = unknown> {
    type: "request";
    id: string;
    method: string;
    params?: TParams;
}
export interface RpcResponse<TResult = unknown> {
    type: "response";
    id: string;
    result?: TResult;
    error?: RpcError;
}
export interface RpcEvent<TParams = unknown> {
    type: "event";
    topic: string;
    params?: TParams;
}
export type RpcEnvelope = RpcRequest | RpcResponse | RpcEvent;
export type GatewayEnvelope = HandshakeResponse | RpcEnvelope;
export interface SessionInfo {
    session_id: string;
    name?: string | null;
    shell: string;
    cols: number;
    rows: number;
    started_at: string;
    status: "active" | "exited" | "inactive";
    exit_code?: number;
}
export interface TmuxSessionInfo {
    name: string;
    windows: number;
    attached: boolean;
}
export interface TmuxListResponse {
    supported: boolean;
    sessions: TmuxSessionInfo[];
}
export interface SessionPreviewResponse {
    text: string;
}
export declare function isHandshakeResponse(value: unknown): value is HandshakeResponse;
export declare function isRpcResponse(value: unknown): value is RpcResponse;
export declare function isRpcEvent(value: unknown): value is RpcEvent;
//# sourceMappingURL=protocol.d.ts.map