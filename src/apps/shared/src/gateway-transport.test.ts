import { afterEach, describe, expect, it, vi } from "vitest";

import {
  GatewayTransport,
  type GatewayCloseEventLike,
  type GatewaySocketLike,
} from "./gateway-transport";

class FakeGatewaySocket implements GatewaySocketLike {
  readyState = 0;
  binaryType?: "blob" | "arraybuffer";
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: unknown) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: GatewayCloseEventLike) => void) | null = null;
  readonly sent: Array<string | ArrayBuffer | ArrayBufferView | Blob> = [];
  sendError: unknown = null;
  closeError: unknown = null;
  closeCount = 0;

  open(): void {
    this.readyState = 1;
    this.onopen?.({});
  }

  reject(): void {
    this.onmessage?.({
      data: JSON.stringify({
        type: "reject",
        code: "unauthorized",
        reason: "bad token",
      }),
    });
  }

  accept(): void {
    this.onmessage?.({
      data: JSON.stringify({
        type: "hello",
        protocol_version: 1,
        server_id: "server",
        services: [],
      }),
    });
  }

  send(data: string | ArrayBuffer | ArrayBufferView | Blob): void {
    if (this.sendError) {
      throw this.sendError;
    }
    this.sent.push(data);
  }

  close(_code?: number, _reason?: string): void {
    this.closeCount += 1;
    if (this.closeError) {
      throw this.closeError;
    }
    this.readyState = 3;
    this.onclose?.({ code: 1006, reason: "closed", wasClean: false });
  }
}

class RejectingBlob extends Blob {
  constructor(private readonly decodeError: unknown) {
    super(["unreadable"]);
  }

  override arrayBuffer(): Promise<ArrayBuffer> {
    return Promise.reject(this.decodeError);
  }
}

describe("GatewayTransport", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("records a diagnostic, closes, and reconnects when hello send throws", () => {
    vi.useFakeTimers();
    const sockets: FakeGatewaySocket[] = [];
    const sendError = new Error("send failed");
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnectBackoff: { baseDelayMs: 10 },
      createWebSocket: () => {
        const socket = new FakeGatewaySocket();
        sockets.push(socket);
        return socket;
      },
    });

    transport.start();
    sockets[0].sendError = sendError;
    sockets[0].open();

    expect(sockets[0].closeCount).toBe(1);
    expect(transport.getState()).toMatchObject({
      status: "disconnected",
      error: {
        message: "hello send failed",
        error: sendError,
      },
    });

    vi.advanceTimersByTime(10);

    expect(sockets).toHaveLength(2);
  });

  it("does not reset retry count until hello succeeds", () => {
    vi.useFakeTimers();
    const sockets: FakeGatewaySocket[] = [];
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnectBackoff: { baseDelayMs: 10 },
      createWebSocket: () => {
        if (sockets.length === 0) {
          sockets.push(new FakeGatewaySocket());
          throw new Error("connect failed");
        }

        const socket = new FakeGatewaySocket();
        sockets.push(socket);
        return socket;
      },
    });

    transport.start();
    vi.advanceTimersByTime(10);
    sockets[1].sendError = new Error("send failed");
    sockets[1].open();
    vi.advanceTimersByTime(10);

    expect(sockets).toHaveLength(2);

    vi.advanceTimersByTime(10);

    expect(sockets).toHaveLength(3);
  });

  it("resets retry count after hello succeeds", () => {
    vi.useFakeTimers();
    const sockets: FakeGatewaySocket[] = [];
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnectBackoff: { baseDelayMs: 10 },
      createWebSocket: () => {
        if (sockets.length === 0) {
          sockets.push(new FakeGatewaySocket());
          throw new Error("connect failed");
        }

        const socket = new FakeGatewaySocket();
        sockets.push(socket);
        return socket;
      },
    });

    transport.start();
    vi.advanceTimersByTime(10);
    sockets[1].open();
    sockets[1].accept();
    sockets[1].close();
    vi.advanceTimersByTime(10);

    expect(sockets).toHaveLength(3);
  });

  it("does not reconnect after hello reject", () => {
    vi.useFakeTimers();
    const sockets: FakeGatewaySocket[] = [];
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnectBackoff: { baseDelayMs: 10 },
      createWebSocket: () => {
        const socket = new FakeGatewaySocket();
        sockets.push(socket);
        return socket;
      },
    });

    transport.start();
    sockets[0].open();
    sockets[0].reject();
    vi.advanceTimersByTime(100);

    expect(transport.getState()).toMatchObject({
      status: "rejected",
      rejection: { code: "unauthorized", reason: "bad token" },
    });
    expect(sockets).toHaveLength(1);
  });

  it("records a diagnostic when sending while disconnected", () => {
    const logger = vi.fn();
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnect: false,
      logger,
    });

    transport.sendBinary(new Uint8Array([1, 2, 3]));
    expect(transport.getState().error).toMatchObject({
      message: "sendBinary skipped",
      context: { reason: "not_connected" },
    });
    expect(logger).toHaveBeenCalledWith(
      "transport failure",
      expect.objectContaining({
        message: "sendBinary skipped",
        context: expect.objectContaining({ reason: "not_connected" }),
      }),
    );
  });

  it("records Blob arrayBuffer decode failures from incoming messages", async () => {
    const socket = new FakeGatewaySocket();
    const decodeError = new Error("decode failed");
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnect: false,
      createWebSocket: () => socket,
    });

    transport.start();
    socket.open();
    socket.onmessage?.({ data: new RejectingBlob(decodeError) });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(transport.getState().error).toMatchObject({
      message: "blob decode failed",
      error: decodeError,
    });
  });

  it("records a diagnostic when binary send throws", () => {
    const socket = new FakeGatewaySocket();
    const sendError = new Error("send failed");
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnect: false,
      createWebSocket: () => socket,
    });

    transport.start();
    socket.open();
    socket.onmessage?.({
      data: JSON.stringify({
        type: "hello",
        protocol_version: 1,
        server_id: "server",
        services: [],
      }),
    });
    socket.sendError = sendError;

    transport.sendBinary(new Uint8Array([1, 2, 3]));
    expect(transport.getState().error).toMatchObject({
      message: "sendBinary failed",
      error: sendError,
    });
  });

  it("records close failures during cleanup without throwing", () => {
    const logger = vi.fn();
    const socket = new FakeGatewaySocket();
    const closeError = new Error("close failed");
    const transport = new GatewayTransport({
      url: "ws://gateway.test",
      reconnect: false,
      createWebSocket: () => socket,
      logger,
    });

    transport.start();
    socket.open();
    socket.closeError = closeError;

    expect(() => transport.stop()).not.toThrow();
    expect(logger).toHaveBeenCalledWith(
      "transport failure",
      expect.objectContaining({
        message: "socket close failed",
        error: closeError,
        context: expect.objectContaining({ phase: "stop" }),
      }),
    );
  });
});
