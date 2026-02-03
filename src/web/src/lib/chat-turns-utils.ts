import type { ChatItem } from "@/lib/chat-utils";

interface TurnGroup {
  id: string;
  turnId?: string;
  items: ChatItem[];
}

function stripMarkdown(text: string) {
  return text.replace(/[#_*`>~-]/g, "").replace(/\s+/g, " ").trim();
}

export function groupTurns(items: ChatItem[]): TurnGroup[] {
  const order: string[] = [];
  const map = new Map<string, TurnGroup>();
  items.forEach((item) => {
    const key = item.turnId ?? item.id;
    if (!map.has(key)) {
      map.set(key, { id: key, turnId: item.turnId, items: [] });
      order.push(key);
    }
    map.get(key)?.items.push(item);
  });
  return order.map((id) => map.get(id)!).filter(Boolean);
}

export function previewFromTurn(turn: TurnGroup, isStreaming: boolean) {
  const assistant = turn.items.find((item) => item.kind === "assistant");
  if (assistant?.text) return stripMarkdown(assistant.text).slice(0, 140);
  const reasoning = turn.items.find((item) => item.kind === "reasoning");
  if (reasoning?.summary?.length) return stripMarkdown(reasoning.summary[0]).slice(0, 140);
  const command = turn.items.find((item) => item.kind === "command");
  if (command?.command) return `Command: ${stripMarkdown(command.command).slice(0, 100)}`;
  if (isStreaming) return "Thinking…";
  return "Steps completed";
}

export function getReasoningPreview(item?: ChatItem) {
  if (!item) return "";
  const summary = item.summary?.filter(Boolean) ?? [];
  if (summary.length > 0) return summary[0];
  const content = item.content?.filter(Boolean) ?? [];
  if (content.length > 0) return content[0];
  return "";
}

export function getActivityPreview(item: ChatItem) {
  switch (item.kind) {
    case "approval":
      return item.reason || item.command || "Approval required";
    case "reasoning":
      return getReasoningPreview(item) || "Reasoning update";
    case "command":
      return item.command ? `Command: ${item.command}` : "Command execution";
    case "file":
      return item.changes?.[0]?.path ? `File: ${item.changes[0].path}` : "File changes";
    case "plan":
      return item.text ? stripMarkdown(item.text).slice(0, 120) : "Plan update";
    case "diff":
      return item.text ? stripMarkdown(item.text).slice(0, 120) : "Diff update";
    case "tool":
      return item.text || "Tool call";
    case "system":
      return item.text || "System update";
    default:
      return item.text || "Update";
  }
}

