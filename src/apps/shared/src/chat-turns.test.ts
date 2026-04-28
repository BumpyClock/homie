import { describe, expect, it } from "vitest";

import type { ChatItem } from "./chat-types";
import { partitionChatTurnItems } from "./chat-turns";

describe("partitionChatTurnItems", () => {
  it("preserves partition order and tracks latest activity fields", () => {
    const user: ChatItem = { id: "user", kind: "user", text: "Build this" };
    const approval: ChatItem = {
      id: "approval",
      kind: "approval",
      reason: "Run command?",
      requestId: 1,
    };
    const firstReasoning: ChatItem = {
      id: "reasoning-1",
      kind: "reasoning",
      summary: ["Checking files"],
    };
    const tool: ChatItem = {
      id: "tool",
      kind: "tool",
      text: "exec",
      raw: { tool: "exec" },
    };
    const assistant: ChatItem = { id: "assistant", kind: "assistant", text: "Done" };
    const latestReasoning: ChatItem = {
      id: "reasoning-2",
      kind: "reasoning",
      summary: ["Verifying"],
    };

    const partition = partitionChatTurnItems([
      user,
      approval,
      firstReasoning,
      tool,
      assistant,
      latestReasoning,
    ]);

    expect(partition.userItems).toEqual([user]);
    expect(partition.assistantItems).toEqual([assistant]);
    expect(partition.activityItems).toEqual([approval, firstReasoning, tool, latestReasoning]);
    expect(partition.approvalItems).toEqual([approval]);
    expect(partition.nonApprovalActivityItems).toEqual([firstReasoning, tool, latestReasoning]);
    expect(partition.toolItems).toEqual([tool]);
    expect(partition.lastActivity).toBe(latestReasoning);
    expect(partition.latestReasoningItem).toBe(latestReasoning);
    expect(partition.hasAssistant).toBe(true);
  });

  it("uses the latest approval as last activity when there are no non-approval activities", () => {
    const firstApproval: ChatItem = { id: "approval-1", kind: "approval", requestId: 1 };
    const latestApproval: ChatItem = { id: "approval-2", kind: "approval", requestId: 2 };

    const partition = partitionChatTurnItems([firstApproval, latestApproval]);

    expect(partition.lastActivity).toBe(latestApproval);
    expect(partition.latestReasoningItem).toBeUndefined();
    expect(partition.hasAssistant).toBe(true);
  });
});
