export const DEFAULT_PREVIEW_LINES = 120;

export interface SessionPreview {
  text: string;
  capturedAt: number;
}

export function previewKey(namespace: string, sessionId: string) {
  return `homie:preview:${namespace}:${sessionId}`;
}

export function savePreview(namespace: string, sessionId: string, text: string) {
  if (typeof window === "undefined") return;
  try {
    const payload: SessionPreview = { text, capturedAt: Date.now() };
    window.localStorage.setItem(previewKey(namespace, sessionId), JSON.stringify(payload));
  } catch {
    // ignore storage failures
  }
}

export function loadPreview(namespace: string, sessionId: string): SessionPreview | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(previewKey(namespace, sessionId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as SessionPreview;
    if (typeof parsed?.text !== "string") return null;
    return parsed;
  } catch {
    return null;
  }
}

export function removePreview(namespace: string, sessionId: string) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(previewKey(namespace, sessionId));
  } catch {
    // ignore
  }
}
