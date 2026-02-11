export const StreamType = {
  Stdout: 0,
  Stderr: 1,
  Stdin: 2,
} as const;

export type StreamType = (typeof StreamType)[keyof typeof StreamType];

const BINARY_HEADER_SIZE = 17;

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

function uuidFromBytes(bytes: Uint8Array): string {
  const hex = Array.from(bytes).map((value) => value.toString(16).padStart(2, '0'));
  return `${hex.slice(0, 4).join('')}-${hex.slice(4, 6).join('')}-${hex.slice(6, 8).join('')}-${hex.slice(8, 10).join('')}-${hex.slice(10).join('')}`;
}
