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
