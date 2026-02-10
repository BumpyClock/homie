import {
  itemsFromThread,
  truncateText,
  type ChatItem,
} from "@homie/shared";

export * from "@homie/shared";

export function extractLastMessage(items: ChatItem[]): string {
  let last = "";
  for (const item of items) {
    if ((item.kind === "user" || item.kind === "assistant") && item.text) {
      last = item.text;
    }
  }
  return last;
}

export function deriveTitleFromThread(thread: Record<string, unknown>, fallback: string) {
  const preview = typeof thread.preview === "string" ? thread.preview : "";
  if (preview.trim().length > 0) return truncateText(preview, 42);
  const items = itemsFromThread(thread);
  const firstMessage = items.find((item) => item.kind === "user" && item.text);
  if (firstMessage?.text) return truncateText(firstMessage.text, 42);
  return fallback;
}
