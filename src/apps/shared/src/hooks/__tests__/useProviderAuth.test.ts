import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useProviderAuth } from "../useProviderAuth.js";
import type {
  ChatDeviceCodeSession,
  ChatDeviceCodePollResult,
} from "../../chat-client.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSession(
  provider = "github",
  intervalSecs = 5,
): ChatDeviceCodeSession {
  return {
    provider,
    verificationUrl: "https://example.com/device",
    userCode: "ABCD-1234",
    deviceCode: "device_abc",
    intervalSecs,
    expiresAt: new Date(Date.now() + 900_000).toISOString(),
  };
}

function makePollResult(
  status: ChatDeviceCodePollResult["status"],
  intervalSecs?: number,
): ChatDeviceCodePollResult {
  return { status, ...(intervalSecs != null ? { intervalSecs } : {}) };
}

function createMocks() {
  return {
    startLogin:
      vi.fn<(p: string) => Promise<ChatDeviceCodeSession>>(),
    pollLogin:
      vi.fn<
        (
          p: string,
          s: ChatDeviceCodeSession,
        ) => Promise<ChatDeviceCodePollResult>
      >(),
    onAuthorized: vi
      .fn<() => Promise<void>>()
      .mockResolvedValue(undefined),
  };
}

/** Flush microtasks so that resolved promises propagate through React. */
async function flushMicrotasks(): Promise<void> {
  await act(async () => {});
}

/** Advance fake timers by `ms` and flush resulting microtasks + React updates. */
async function advance(ms: number): Promise<void> {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  // Extra flush to let any resolved promises propagate
  await flushMicrotasks();
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

describe("useProviderAuth", () => {
  beforeEach(() => {
    vi.useFakeTimers({
      toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"],
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // -----------------------------------------------------------------------
  // 1. Happy path: connect → starting → polling → authorized → onAuthorized
  // -----------------------------------------------------------------------
  it("happy path: transitions starting → polling → authorized and calls onAuthorized", async () => {
    const mocks = createMocks();
    const session = makeSession();
    mocks.startLogin.mockResolvedValue(session);
    mocks.pollLogin.mockResolvedValue(makePollResult("authorized"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    // connect triggers starting
    act(() => {
      result.current.connect("github");
    });
    expect(result.current.authStates["github"]?.status).toBe("starting");

    // Flush microtasks → startLogin resolves → polling
    await flushMicrotasks();
    expect(result.current.authStates["github"]?.status).toBe("polling");
    expect(result.current.authStates["github"]?.session).toEqual({
      verificationUrl: "https://example.com/device",
      userCode: "ABCD-1234",
    });

    // Advance past the poll interval (5s) → pollLogin resolves → authorized
    await advance(5_000);

    expect(result.current.authStates["github"]?.status).toBe("authorized");
    expect(mocks.onAuthorized).toHaveBeenCalledOnce();
  });

  // -----------------------------------------------------------------------
  // 2. Denied: sets error "Access denied."
  // -----------------------------------------------------------------------
  it("sets denied status with error on denied poll result", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockResolvedValue(makeSession());
    mocks.pollLogin.mockResolvedValue(makePollResult("denied"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    await flushMicrotasks();
    await advance(5_000);

    expect(result.current.authStates["github"]?.status).toBe("denied");
    expect(result.current.authStates["github"]?.error).toBe("Access denied");
    expect(mocks.onAuthorized).not.toHaveBeenCalled();
  });

  // -----------------------------------------------------------------------
  // 3. Expired: sets error "Device code expired."
  // -----------------------------------------------------------------------
  it("sets expired status with error on expired poll result", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockResolvedValue(makeSession());
    mocks.pollLogin.mockResolvedValue(makePollResult("expired"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    await flushMicrotasks();
    await advance(5_000);

    expect(result.current.authStates["github"]?.status).toBe("expired");
    expect(result.current.authStates["github"]?.error).toBe("Code expired");
  });

  // -----------------------------------------------------------------------
  // 4. Slow down: interval increases by SLOW_DOWN_INCREASE_SECS (5s)
  // -----------------------------------------------------------------------
  it("increases poll interval by 5s on slow_down response (RFC 8628 §3.5)", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockResolvedValue(makeSession("github", 5));
    // First poll: slow_down, second poll: authorized
    mocks.pollLogin
      .mockResolvedValueOnce(makePollResult("slow_down"))
      .mockResolvedValueOnce(makePollResult("authorized"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    // Flush startLogin
    await flushMicrotasks();

    // First poll at 5s → returns slow_down
    await advance(5_000);
    expect(mocks.pollLogin).toHaveBeenCalledTimes(1);

    // After slow_down, interval = 5+5 = 10s. At 9s no new poll.
    await advance(9_000);
    expect(mocks.pollLogin).toHaveBeenCalledTimes(1);

    // At 10s (1 more second) → second poll fires → authorized
    await advance(1_000);
    expect(mocks.pollLogin).toHaveBeenCalledTimes(2);
    expect(result.current.authStates["github"]?.status).toBe("authorized");
  });

  // -----------------------------------------------------------------------
  // 5. Timeout: 120 iterations → "Authorization timed out."
  // -----------------------------------------------------------------------
  it("times out after MAX_POLL_ITERATIONS (120) with pending results", async () => {
    const mocks = createMocks();
    // Use minimum interval (1s) to keep the test fast
    mocks.startLogin.mockResolvedValue(makeSession("github", 1));
    mocks.pollLogin.mockResolvedValue(makePollResult("pending"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    await flushMicrotasks();

    // Advance through all 120 iterations (1s each)
    for (let i = 0; i < 120; i++) {
      await advance(1_000);
    }

    expect(result.current.authStates["github"]?.status).toBe("expired");
    expect(result.current.authStates["github"]?.error).toBe(
      "Authorization timed out.",
    );
    expect(mocks.pollLogin).toHaveBeenCalledTimes(120);
  }, 30_000);

  // -----------------------------------------------------------------------
  // 6. Network error: startLogin throws → error state
  // -----------------------------------------------------------------------
  it("sets error state when startLogin throws", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockRejectedValue(new Error("Network failure"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    // Flush → rejected promise propagates → error state
    await flushMicrotasks();

    expect(result.current.authStates["github"]?.status).toBe("error");
    expect(result.current.authStates["github"]?.error).toBe("Network failure");
  });

  // -----------------------------------------------------------------------
  // 7. Cancel: connect then cancel → resets to idle, stops polling
  // -----------------------------------------------------------------------
  it("cancel resets state to idle and stops polling", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockResolvedValue(makeSession());
    mocks.pollLogin.mockResolvedValue(makePollResult("pending"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    await flushMicrotasks();
    expect(result.current.authStates["github"]?.status).toBe("polling");

    // Cancel
    act(() => {
      result.current.cancel("github");
    });
    expect(result.current.authStates["github"]?.status).toBe("idle");

    // Advance timers — no more polls should happen
    const callsBefore = mocks.pollLogin.mock.calls.length;
    await advance(30_000);
    expect(mocks.pollLogin).toHaveBeenCalledTimes(callsBefore);
  });

  // -----------------------------------------------------------------------
  // 8. Re-connect: cancels previous flow, starts new
  // -----------------------------------------------------------------------
  it("re-connect cancels previous flow and starts a new one", async () => {
    const mocks = createMocks();
    const session1 = makeSession("github", 5);
    const session2 = makeSession("github", 5);
    session2.userCode = "NEW-CODE";

    mocks.startLogin
      .mockResolvedValueOnce(session1)
      .mockResolvedValueOnce(session2);
    mocks.pollLogin.mockResolvedValue(makePollResult("authorized"));

    const { result } = renderHook(() => useProviderAuth(mocks));

    // First connect
    act(() => {
      result.current.connect("github");
    });
    await flushMicrotasks();
    expect(result.current.authStates["github"]?.session?.userCode).toBe(
      "ABCD-1234",
    );

    // Re-connect before first flow polls
    act(() => {
      result.current.connect("github");
    });
    expect(result.current.authStates["github"]?.status).toBe("starting");

    // Flush second startLogin
    await flushMicrotasks();
    expect(result.current.authStates["github"]?.session?.userCode).toBe(
      "NEW-CODE",
    );

    // Advance past poll interval → authorized via second session
    await advance(5_000);
    expect(result.current.authStates["github"]?.status).toBe("authorized");
    expect(mocks.startLogin).toHaveBeenCalledTimes(2);
  });

  // -----------------------------------------------------------------------
  // 9. Unmount cleanup: cancels all tokens
  // -----------------------------------------------------------------------
  it("unmount cancels all in-flight flows", async () => {
    const mocks = createMocks();
    mocks.startLogin.mockResolvedValue(makeSession());
    mocks.pollLogin.mockResolvedValue(makePollResult("pending"));

    const { result, unmount } = renderHook(() => useProviderAuth(mocks));

    act(() => {
      result.current.connect("github");
    });

    await flushMicrotasks();
    expect(result.current.authStates["github"]?.status).toBe("polling");

    // Unmount
    unmount();

    // Advance timers — no more polls should fire
    const callsBefore = mocks.pollLogin.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(30_000);
    });
    expect(mocks.pollLogin).toHaveBeenCalledTimes(callsBefore);
  });

  // -----------------------------------------------------------------------
  // 10. Multiple providers: independent state
  // -----------------------------------------------------------------------
  it("multiple providers maintain independent state", async () => {
    const mocks = createMocks();

    const githubSession = makeSession("github", 5);
    const gitlabSession = makeSession("gitlab", 5);

    mocks.startLogin.mockImplementation(async (provider: string) => {
      if (provider === "github") return githubSession;
      return gitlabSession;
    });

    mocks.pollLogin.mockImplementation(
      async (
        _provider: string,
        session: ChatDeviceCodeSession,
      ) => {
        if (session.provider === "github")
          return makePollResult("authorized");
        return makePollResult("denied");
      },
    );

    const { result } = renderHook(() => useProviderAuth(mocks));

    // Connect both providers
    act(() => {
      result.current.connect("github");
      result.current.connect("gitlab");
    });

    // Flush startLogins
    await flushMicrotasks();
    expect(result.current.authStates["github"]?.status).toBe("polling");
    expect(result.current.authStates["gitlab"]?.status).toBe("polling");

    // Advance past poll interval
    await advance(5_000);

    // github authorized, gitlab denied — independent
    expect(result.current.authStates["github"]?.status).toBe("authorized");
    expect(result.current.authStates["github"]?.error).toBeUndefined();
    expect(result.current.authStates["gitlab"]?.status).toBe("denied");
    expect(result.current.authStates["gitlab"]?.error).toBe("Access denied");
  });
});
