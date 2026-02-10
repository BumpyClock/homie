import {
  PROTOCOL_VERSION,
  type ClientHello,
  type HelloReject,
  type RpcEvent,
  type RpcRequest,
  type RpcResponse,
  type ServerHello,
  isHandshakeResponse,
  isRpcEvent,
  isRpcResponse,
} from "./protocol";
import { RequestMap } from "./request-map";

export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "handshaking"
  | "connected"
  | "error"
  | "rejected";

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

const WS_CONNECTING = 0;
const WS_OPEN = 1;
const WS_CLOSING = 2;

export const DEFAULT_HANDSHAKE_TIMEOUT_MS = 5000;
export const DEFAULT_MAX_BINARY_BACKLOG_BYTES = 1024 * 1024;
export const DEFAULT_BASE_RECONNECT_DELAY_MS = 1000;
export const DEFAULT_MAX_RECONNECT_DELAY_MS = 30000;

export function createRequestId(): string {
  const maybeCrypto = globalThis.crypto;
  if (maybeCrypto && typeof maybeCrypto.randomUUID === "function") {
    return maybeCrypto.randomUUID();
  }

  const random = Math.random().toString(16).slice(2);
  return `req_${Date.now()}_${random}`;
}

export function getReconnectDelayMs(
  retryCount: number,
  options: ReconnectBackoffOptions = {}
): number {
  const base = options.baseDelayMs ?? DEFAULT_BASE_RECONNECT_DELAY_MS;
  const max = options.maxDelayMs ?? DEFAULT_MAX_RECONNECT_DELAY_MS;
  return Math.min(base * Math.pow(2, retryCount), max);
}

function defaultCreateWebSocket(url: string): GatewaySocketLike {
  if (typeof WebSocket === "undefined") {
    throw new Error("WebSocket is not available");
  }

  return new WebSocket(url) as unknown as GatewaySocketLike;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function extractMessagePayload(value: unknown): unknown {
  if (!isObjectRecord(value)) {
    return value;
  }

  if ("data" in value) {
    return value.data;
  }

  return value;
}

function isBlobValue(value: unknown): value is Blob {
  return typeof Blob !== "undefined" && value instanceof Blob;
}

function toArrayBuffer(view: ArrayBufferView): ArrayBuffer {
  const bytes = new Uint8Array(view.byteLength);
  bytes.set(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
  return bytes.buffer;
}

function toRequestId(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }

  if (typeof value === "number" || typeof value === "bigint") {
    return String(value);
  }

  return null;
}

export class GatewayTransport {
  private readonly options: Required<
    Omit<
      GatewayTransportOptions,
      "authToken" | "createWebSocket" | "logger" | "createRequestId" | "reconnectBackoff"
    >
  > & {
    authToken?: string;
    createWebSocket: (url: string) => GatewaySocketLike;
    logger?: (...args: unknown[]) => void;
    createRequestId: () => string;
    reconnectBackoff: ReconnectBackoffOptions;
  };

  private socket: GatewaySocketLike | null = null;
  private handshakeTimeout: ReturnType<typeof setTimeout> | null = null;
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
  private handshakeCompleted = false;
  private retryCount = 0;
  private running = false;
  private shouldReconnect = true;

  private status: ConnectionStatus = "disconnected";
  private serverHello: ServerHello | null = null;
  private rejection: HelloReject | null = null;
  private error: unknown = null;

  private readonly pendingRequests = new RequestMap();
  private readonly binaryListeners = new Set<(data: ArrayBuffer) => void>();
  private readonly eventListeners = new Set<(event: RpcEvent) => void>();
  private readonly stateListeners = new Set<(state: GatewayTransportState) => void>();

  private binaryBacklog: ArrayBuffer[] = [];
  private binaryBacklogBytes = 0;

  constructor(options: GatewayTransportOptions) {
    this.options = {
      url: options.url,
      authToken: options.authToken,
      protocolVersion: options.protocolVersion ?? PROTOCOL_VERSION,
      clientId: options.clientId ?? "homie-client/0.0.1",
      capabilities: options.capabilities ?? ["terminal", "chat"],
      handshakeTimeoutMs: options.handshakeTimeoutMs ?? DEFAULT_HANDSHAKE_TIMEOUT_MS,
      maxBinaryBacklogBytes: options.maxBinaryBacklogBytes ?? DEFAULT_MAX_BINARY_BACKLOG_BYTES,
      reconnect: options.reconnect ?? true,
      reconnectBackoff: options.reconnectBackoff ?? {},
      createRequestId: options.createRequestId ?? createRequestId,
      createWebSocket: options.createWebSocket ?? defaultCreateWebSocket,
      logger: options.logger,
    };
    this.shouldReconnect = this.options.reconnect;
  }

  start(): void {
    if (this.running) {
      return;
    }

    this.running = true;
    this.shouldReconnect = this.options.reconnect;
    this.retryCount = 0;

    if (!this.options.url) {
      this.resetState("disconnected");
      return;
    }

    this.connect();
  }

  stop(): void {
    this.running = false;
    this.shouldReconnect = false;
    this.retryCount = 0;
    this.clearReconnectTimeout();
    this.clearHandshakeTimeout();

    const socket = this.socket;
    this.socket = null;
    this.handshakeCompleted = false;
    this.clearBinaryBacklog();
    this.pendingRequests.rejectAll(new Error("Connection closed"));

    if (socket) {
      try {
        socket.close();
      } catch {
        // ignore
      }
    }

    this.resetState("disconnected");
  }

  setConnection(url: string, authToken?: string): void {
    const sameUrl = this.options.url === url;
    const sameAuth = this.options.authToken === authToken;
    if (sameUrl && sameAuth) {
      return;
    }

    this.options.url = url;
    this.options.authToken = authToken;
    this.shouldReconnect = this.options.reconnect;
    this.retryCount = 0;
    this.clearReconnectTimeout();

    const socket = this.socket;
    this.socket = null;
    this.handshakeCompleted = false;
    this.clearHandshakeTimeout();
    this.clearBinaryBacklog();
    this.pendingRequests.rejectAll(new Error("Connection closed"));

    if (socket) {
      try {
        socket.close();
      } catch {
        // ignore
      }
    }

    if (!url) {
      this.resetState("disconnected");
      return;
    }

    if (!this.running) {
      this.resetState("disconnected");
      return;
    }

    this.connect();
  }

  getState(): GatewayTransportState {
    return {
      status: this.status,
      serverHello: this.serverHello,
      rejection: this.rejection,
      error: this.error,
    };
  }

  onStateChange(listener: (state: GatewayTransportState) => void): Unsubscribe {
    this.stateListeners.add(listener);
    listener(this.getState());
    return () => {
      this.stateListeners.delete(listener);
    };
  }

  onBinaryMessage(listener: (data: ArrayBuffer) => void): Unsubscribe {
    this.binaryListeners.add(listener);

    if (this.binaryBacklog.length > 0) {
      const buffered = this.binaryBacklog;
      this.binaryBacklog = [];
      this.binaryBacklogBytes = 0;
      for (const frame of buffered) {
        listener(frame);
      }
      this.log("binary backlog flushed", { frames: buffered.length });
    }

    return () => {
      this.binaryListeners.delete(listener);
    };
  }

  onEvent(listener: (event: RpcEvent) => void): Unsubscribe {
    this.eventListeners.add(listener);
    return () => {
      this.eventListeners.delete(listener);
    };
  }

  call<TResult = unknown>(method: string, params?: unknown): Promise<TResult> {
    return new Promise<TResult>((resolve, reject) => {
      const socket = this.socket;
      if (!socket || socket.readyState !== WS_OPEN || !this.handshakeCompleted) {
        reject(new Error("Not connected"));
        return;
      }

      const id = this.options.createRequestId();
      const request: RpcRequest = {
        type: "request",
        id,
        method,
        params,
      };

      this.pendingRequests.set(id, { resolve, reject });
      this.log("request", { id, method });

      try {
        socket.send(JSON.stringify(request));
      } catch (error) {
        this.pendingRequests.reject(id, error);
      }
    });
  }

  sendBinary(data: Uint8Array | ArrayBuffer): void {
    const socket = this.socket;
    if (!socket || socket.readyState !== WS_OPEN || !this.handshakeCompleted) {
      return;
    }

    socket.send(data);
  }

  private connect(): void {
    if (!this.running || !this.options.url) {
      return;
    }

    if (this.socket) {
      const existing = this.socket;
      const state = existing.readyState;
      if (state === WS_OPEN) {
        this.log("skip connect: socket open");
        return;
      }

      if (state === WS_CONNECTING || state === WS_CLOSING) {
        try {
          existing.close();
        } catch {
          // ignore
        }
      }

      this.socket = null;
      this.handshakeCompleted = false;
      this.clearHandshakeTimeout();
      this.clearBinaryBacklog();
    }

    this.setStatus("connecting");
    this.error = null;
    this.rejection = null;
    this.serverHello = null;
    this.emitState();
    this.log("connecting", { url: this.options.url, auth: this.options.authToken ? "set" : "none" });

    let socket: GatewaySocketLike;
    try {
      socket = this.options.createWebSocket(this.options.url);
    } catch (error) {
      this.error = error;
      this.setStatus("error");
      this.scheduleReconnect();
      return;
    }

    try {
      socket.binaryType = "arraybuffer";
    } catch {
      // ignore
    }
    this.socket = socket;
    this.handshakeCompleted = false;
    this.clearHandshakeTimeout();
    this.clearBinaryBacklog();

    socket.onopen = () => {
      if (!this.running || this.socket !== socket) {
        return;
      }

      this.setStatus("handshaking");
      this.retryCount = 0;
      this.log("open");
      this.clearReconnectTimeout();

      const clientHello: ClientHello = {
        protocol: {
          min: this.options.protocolVersion,
          max: this.options.protocolVersion,
        },
        client_id: this.options.clientId,
        auth_token: this.options.authToken,
        capabilities: this.options.capabilities,
      };

      this.log("send hello", {
        protocol: clientHello.protocol,
        client_id: clientHello.client_id,
      });

      socket.send(JSON.stringify(clientHello));

      this.clearHandshakeTimeout();
      this.handshakeTimeout = setTimeout(() => {
        if (this.socket === socket && !this.handshakeCompleted) {
          this.log("handshake timeout; closing");
          try {
            socket.close();
          } catch {
            // ignore
          }
        }
      }, this.options.handshakeTimeoutMs);
    };

    socket.onmessage = (event) => {
      if (!this.running || this.socket !== socket) {
        return;
      }

      const payload = extractMessagePayload(event);

      if (isBlobValue(payload)) {
        void payload
          .arrayBuffer()
          .then((buffer) => {
            if (!this.running || this.socket !== socket) {
              return;
            }
            this.dispatchBinary(buffer);
          })
          .catch(() => {
            // ignore
          });
        return;
      }

      if (payload instanceof ArrayBuffer) {
        this.dispatchBinary(payload);
        return;
      }

      if (ArrayBuffer.isView(payload)) {
        this.dispatchBinary(toArrayBuffer(payload));
        return;
      }

      if (typeof payload !== "string") {
        return;
      }

      let decoded: unknown;
      try {
        decoded = JSON.parse(payload);
      } catch (error) {
        this.log("parse failed", error);
        return;
      }

      if (!this.handshakeCompleted) {
        if (!isHandshakeResponse(decoded)) {
          return;
        }

        if (decoded.type === "hello") {
          this.handshakeCompleted = true;
          this.clearHandshakeTimeout();
          this.clearBinaryBacklog();
          this.serverHello = decoded;
          this.setStatus("connected");
          this.log("handshake ok", {
            server_id: decoded.server_id,
            version: decoded.protocol_version,
          });
        } else {
          this.shouldReconnect = false;
          this.rejection = decoded;
          this.setStatus("rejected");
          this.log("handshake rejected", decoded);
          socket.close();
        }
        return;
      }

      if (isRpcResponse(decoded)) {
        const id = toRequestId(decoded.id);
        if (!id) {
          return;
        }

        const response = decoded as RpcResponse;
        if (response.error) {
          this.pendingRequests.reject(id, response.error);
        } else {
          this.pendingRequests.resolve(id, response.result);
        }
        this.log("response", { id, ok: !response.error });
        return;
      }

      if (isRpcEvent(decoded)) {
        this.log("event", { topic: decoded.topic });
        for (const listener of this.eventListeners) {
          listener(decoded);
        }
      }
    };

    socket.onerror = (event) => {
      if (!this.running || this.socket !== socket) {
        return;
      }

      this.error = event;
      this.setStatus("error");
      this.log("error", event);

      try {
        socket.close();
      } catch {
        // ignore
      }
    };

    socket.onclose = (event) => {
      if (this.socket !== socket) {
        return;
      }

      this.socket = null;
      this.handshakeCompleted = false;
      this.clearHandshakeTimeout();
      this.clearBinaryBacklog();
      this.pendingRequests.rejectAll(new Error("Connection closed"));

      if (!this.running) {
        return;
      }

      this.log("close", {
        code: event.code,
        reason: event.reason,
        wasClean: event.wasClean,
      });

      if (this.status !== "rejected") {
        this.setStatus("disconnected");
      }

      this.scheduleReconnect();
    };
  }

  private scheduleReconnect(): void {
    if (!this.running || !this.shouldReconnect) {
      return;
    }

    const delay = getReconnectDelayMs(this.retryCount, this.options.reconnectBackoff);
    this.retryCount += 1;

    this.clearReconnectTimeout();
    this.reconnectTimeout = setTimeout(() => {
      this.reconnectTimeout = null;
      this.connect();
    }, delay);
  }

  private dispatchBinary(buffer: ArrayBuffer): void {
    if (this.binaryListeners.size > 0) {
      for (const listener of this.binaryListeners) {
        listener(buffer);
      }
      return;
    }

    this.binaryBacklog.push(buffer);
    this.binaryBacklogBytes += buffer.byteLength;

    while (
      this.binaryBacklog.length > 0 &&
      this.binaryBacklogBytes > this.options.maxBinaryBacklogBytes
    ) {
      const dropped = this.binaryBacklog.shift();
      if (dropped) {
        this.binaryBacklogBytes -= dropped.byteLength;
      }
    }

    this.log("binary buffered", {
      bytes: buffer.byteLength,
      backlogFrames: this.binaryBacklog.length,
      backlogBytes: this.binaryBacklogBytes,
    });
  }

  private clearBinaryBacklog(): void {
    this.binaryBacklog = [];
    this.binaryBacklogBytes = 0;
  }

  private clearHandshakeTimeout(): void {
    if (this.handshakeTimeout) {
      clearTimeout(this.handshakeTimeout);
      this.handshakeTimeout = null;
    }
  }

  private clearReconnectTimeout(): void {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
  }

  private setStatus(status: ConnectionStatus): void {
    if (this.status === status) {
      return;
    }

    this.status = status;
    this.emitState();
  }

  private resetState(status: ConnectionStatus): void {
    this.status = status;
    this.serverHello = null;
    this.rejection = null;
    this.error = null;
    this.emitState();
  }

  private emitState(): void {
    if (this.stateListeners.size === 0) {
      return;
    }

    const snapshot = this.getState();
    for (const listener of this.stateListeners) {
      listener(snapshot);
    }
  }

  private log(...args: unknown[]): void {
    this.options.logger?.(...args);
  }
}
