import { extractLastMessage, truncateText, type ChatItem, type ChatThreadSummary } from "./chat-types";

/** Returns true if stop button should be shown (running but not actively sending). */
export function shouldShowStop(running: boolean, sending: boolean): boolean {
  return running && !sending;
}

const PREVIEW_LIMIT = 96;

/** Extract a short preview string from the last user/assistant message. */
export function previewFromItems(items: ChatItem[]): string {
  const text = extractLastMessage(items);
  return text ? truncateText(text, PREVIEW_LIMIT) : "";
}

/** Sort thread summaries by most-recent activity first. */
export function sortThreads(threads: ChatThreadSummary[]): ChatThreadSummary[] {
  return [...threads].sort((left, right) => {
    const leftValue = left.lastActivityAt ?? 0;
    const rightValue = right.lastActivityAt ?? 0;
    return rightValue - leftValue;
  });
}
