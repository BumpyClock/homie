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

// RPC Types

export interface JsonRpcRequest {
  id: string;
  method: string;
  params?: unknown;
}

export interface JsonRpcResponse {
  id: string;
  result?: unknown;
  error?: {
    code: number;
    message: string;
  };
}

export interface SessionInfo {
  session_id: string;
  shell: string;
  cols: number;
  rows: number;
  started_at: string;
  status: "Active" | "Exited" | "Inactive";
  exit_code?: number;
}
