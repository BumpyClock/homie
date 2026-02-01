
export const StreamType = {
  Stdout: 0,
  Stderr: 1,
  Stdin: 2,
} as const;

export type StreamType = typeof StreamType[keyof typeof StreamType];

export const BINARY_HEADER_SIZE = 17;

export interface BinaryFrame {
  sessionId: string;
  stream: StreamType;
  payload: Uint8Array;
}

export function parseBinaryFrame(data: ArrayBuffer): BinaryFrame {
  const view = new DataView(data);
  const sessionBytes = new Uint8Array(data, 0, 16);
  const sessionId = uuidFromBytes(sessionBytes);
  const stream = view.getUint8(16) as StreamType;
  const payload = new Uint8Array(data, BINARY_HEADER_SIZE);

  return { sessionId, stream, payload };
}

export function createBinaryFrame(sessionId: string, stream: StreamType, payload: Uint8Array | string): Uint8Array {
  const payloadBytes = typeof payload === 'string' ? new TextEncoder().encode(payload) : payload;
  const buffer = new Uint8Array(BINARY_HEADER_SIZE + payloadBytes.length);
  const view = new DataView(buffer.buffer);
  
  // UUID to bytes
  const sessionBytes = uuidToBytes(sessionId);
  buffer.set(sessionBytes, 0);
  
  view.setUint8(16, stream);
  buffer.set(payloadBytes, BINARY_HEADER_SIZE);
  
  return buffer;
}

function uuidFromBytes(bytes: Uint8Array): string {
    // Simple implementation or use a library if available. 
    // Format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    const hex = Array.from(bytes).map(b => b.toString(16).padStart(2, '0'));
    return `${hex.slice(0, 4).join('')}-${hex.slice(4, 6).join('')}-${hex.slice(6, 8).join('')}-${hex.slice(8, 10).join('')}-${hex.slice(10).join('')}`;
}

function uuidToBytes(uuid: string): Uint8Array {
    const hex = uuid.replace(/-/g, '');
    const bytes = new Uint8Array(16);
    for (let i = 0; i < 16; i++) {
        bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
    }
    return bytes;
}
