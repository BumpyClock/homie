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

export type HandshakeResponse =
  | ({ type: "hello" } & ServerHello)
  | ({ type: "reject" } & HelloReject);

export const PROTOCOL_VERSION = 1;

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

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isHandshakeResponse(value: unknown): value is HandshakeResponse {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (value.type === "hello") {
    return typeof value.protocol_version === "number" && typeof value.server_id === "string";
  }

  if (value.type === "reject") {
    return typeof value.code === "string" && typeof value.reason === "string";
  }

  return false;
}

export function isRpcResponse(value: unknown): value is RpcResponse {
  if (!isObjectRecord(value) || value.type !== "response") {
    return false;
  }

  return typeof value.id === "string";
}

export function isRpcEvent(value: unknown): value is RpcEvent {
  if (!isObjectRecord(value) || value.type !== "event") {
    return false;
  }

  return typeof value.topic === "string";
}
