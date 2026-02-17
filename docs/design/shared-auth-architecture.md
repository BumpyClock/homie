# Shared-first provider auth architecture

> **Bead**: remotely-o1v.5.2 | **Date**: 2026-02-16

## Overview

Provider authentication uses a **shared-first** architecture: a single `useProviderAuth` hook in `@homie/shared` owns the device-code state machine and polling logic. Web and mobile platforms provide thin adapter components that wire the hook into their respective UI frameworks. Unified UI copy lives in a shared `AUTH_COPY` constant to ensure consistent messaging across platforms.

## Architecture diagram

```
@homie/shared
├── useProviderAuth()        ← state machine + polling logic
├── AUTH_COPY                ← unified UI copy
└── provider-auth-copy.ts   ← copy constants
     │
     ├── Web adapter (src/web/)
     │   ├── SettingsPanel → ProviderAccountsSection
     │   └── ChatPanel → AuthRedirectBanner
     │
     └── Mobile adapter (src/apps/mobile/)
         ├── SettingsScreen → ProviderAccountsSection
         └── ChatTimeline → AuthRedirectBanner
```

## useProviderAuth API

```typescript
interface ProviderAuthState {
  status: "idle" | "starting" | "polling" | "authorized" | "denied" | "expired" | "error";
  session?: { verificationUrl: string; userCode: string };
  error?: string;
}

function useProviderAuth(opts: {
  startLogin: (provider: string) => Promise<ChatDeviceCodeSession>;
  pollLogin: (provider: string, session: ChatDeviceCodeSession) => Promise<ChatDeviceCodePollResult>;
  onAuthorized: () => Promise<void>;
}): {
  authStates: Record<string, ProviderAuthState>;
  connect: (providerId: string) => void;
  cancel: (providerId: string) => void;
}
```

- **`authStates`** — map of provider ID to current auth state
- **`connect(providerId)`** — start device-code flow; cancels any in-flight flow for the same provider
- **`cancel(providerId)`** — cancel in-flight flow and reset to idle

## State machine

Per-provider state transitions:

```
idle ──connect()──→ starting ──session──→ polling
                                           │
                      ┌────────────────────┼────────────────┐
                      ▼                    ▼                ▼
                 authorized            denied/expired     error
                      │                    │                │
                      ▼                    ▼                ▼
                 onAuthorized()      error message      error message
```

- **idle** — no flow in progress
- **starting** — `chat.account.login.start` RPC in flight
- **polling** — device code issued; polling `chat.account.login.poll` at server-specified interval
- **authorized** — user approved; credentials stored; `onAuthorized` callback fires
- **denied** / **expired** / **error** — terminal states; UI shows error message; user can retry

RFC 8628 compliance: on `slow_down`, interval increases by 5 seconds per spec.

## Platform responsibilities

| Concern | Shared (`@homie/shared`) | Platform-owned |
|---------|--------------------------|----------------|
| State machine + polling | `useProviderAuth` | — |
| UI copy / labels | `AUTH_COPY` | — |
| Provider list rendering | — | `ProviderAccountsSection` |
| Single provider row | — | `ProviderRow` |
| Device-code verification UI | — | `DeviceCodeInline` (web) / `DeviceCodeCard` (mobile) |
| Auth redirect banner | — | `AuthRedirectBanner` |
| URL opening | — | `window.open` (web) / `Linking.openURL` (mobile) |
| Navigation / routing | — | Platform router |
| Theming / tokens | — | Platform theme system |
| Animations / haptics | — | CSS / Reanimated |

## File map

| File | Role |
|------|------|
| `src/apps/shared/src/hooks/useProviderAuth.ts` | Shared auth state machine hook |
| `src/apps/shared/src/provider-auth-copy.ts` | Shared UI copy constants (`AUTH_COPY`) |
| `src/apps/shared/src/hooks/index.ts` | Hook barrel export |
| `src/web/src/components/settings/ProviderAccountsSection.tsx` | Web provider list + auth |
| `src/web/src/components/settings/ProviderRow.tsx` | Web single provider row |
| `src/web/src/components/settings/DeviceCodeInline.tsx` | Web device-code verification card |
| `src/web/src/components/AuthRedirectBanner.tsx` | Web chat auth redirect banner |
| `src/apps/mobile/components/settings/ProviderAccountsSection.tsx` | Mobile provider list + auth |
| `src/apps/mobile/components/settings/ProviderRow.tsx` | Mobile single provider row |
| `src/apps/mobile/components/settings/DeviceCodeCard.tsx` | Mobile device-code verification card |
| `src/apps/mobile/components/chat/AuthRedirectBanner.tsx` | Mobile chat auth redirect banner |
