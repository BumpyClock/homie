# Provider Accounts Section Design Doc

> **Bead**: remotely-o1v.4 | **Date**: 2026-02-16
> **Parent**: `settings-provider-login.md` (sections 3, 6, 10)

---

## Overview

- **Goals**: Enable users to view provider connection status, initiate device-code auth flows, and manage linked AI provider accounts from the web Settings panel.
- **Primary users**: Developers using Homie web console who need to authenticate with AI providers (OpenAI Codex, GitHub Copilot, Claude Code).
- **Success criteria**:
  - User can see which providers are available and their connection status
  - User can initiate and complete device-code auth without leaving the Settings panel
  - Auth state changes are announced to screen readers
  - All interactions meet 44px touch target minimum

---

## Inputs and Constraints

### Platform Targets
- Web (desktop-first, responsive down to 320px width)
- Settings panel slide-over: 480px max-width on desktop, full-width on mobile (<=640px)

### Design System
- **Reuse existing tokens**: `--surface-0`, `--surface-1`, `--border`, `--text-primary`, `--text-secondary`, `--text-tertiary`, `--accent-dim`, `--success`, `--success-dim`, `--danger`, `--danger-dim`, `--warning`, `--warning-dim`
- **Reuse existing animations**: `.homie-dots`, `.homie-fade-in`, `--duration-standard` (200ms), `--duration-fast` (140ms), `--ease-enter`, `--ease-move`
- **Reuse existing components**: `StatusDot` (from `@/components/status-dot`)
- **Icon library**: Lucide React (already in use: `Wifi`, `KeyRound`, `SlidersHorizontal`, `Info`)

### Content Requirements
- Provider list sourced from `chatClient.listAccounts()` returning `ChatAccountProviderStatus[]`
- Device code session data: `verificationUrl`, `userCode`, `deviceCode`, `intervalSecs`, `expiresAt`
- Provider display names via `modelProviderLabel()` from `@homie/shared`

### Technical Constraints
- Must integrate with `useProviderAuth` hook from `@homie/shared`
- Must call `refreshAccountProviders` on successful auth
- Providers only available when gateway status is "connected"

---

## Information Architecture

```
Provider Accounts Section
├── Section header (title + description)
└── Provider list
    ├── Provider row (idle/connected/error)
    │   └── Device code card (expanded when polling)
    ├── Provider row
    └── ...
```

### User Flow

```
[View providers] → [Click "Connect"] → [See device code]
                                            ↓
                                    [Open URL in new tab]
                                            ↓
                                    [Enter code on provider site]
                                            ↓
                    [Polling...]  ←─────────┘
                         ↓
                [Authorization result]
                    ↓           ↓
              [Success]     [Denied/Expired]
                 ↓              ↓
           [Update UI]    [Show error + retry]
```

---

## Design System Strategy

### Existing Tokens Reused
| Token | Usage |
|-------|-------|
| `--surface-0` | Card/row background |
| `--surface-1` | Hover state, device code card background |
| `--border` | Row borders, card borders |
| `--text-primary` | Provider names, headings |
| `--text-secondary` | Descriptions, labels |
| `--text-tertiary` | Timestamps, hints |
| `--success` | Connected status indicator |
| `--success-dim` | Connected status pill background |
| `--danger` | Error text |
| `--danger-dim` | Error background |
| `--accent-dim` | Active/focus states |

### Existing Components Reused
| Component | Usage |
|-----------|-------|
| `StatusDot` | Provider connection status indicator |

### New Components Needed
| Component | Purpose |
|-----------|---------|
| `ProviderAccountsSection` | Section container with header and provider list |
| `ProviderRow` | Single provider with status, actions, expandable device code |
| `DeviceCodeInline` | Device code verification UI with URL, code, countdown |
| `StatusPill` | Small badge showing connected/error state |
| `CopyButton` | Icon button to copy code to clipboard |

### Token Naming Conventions
- Follow existing pattern: `--{category}-{modifier}` (e.g., `--text-secondary`)
- New animation classes: `homie-{feature}-{action}` (e.g., `homie-row-expand`)

---

## Layout and Responsive Behavior

### Desktop (>=640px)
- Provider rows: full width within 480px panel
- Device code card: full width within row, 16px padding
- Copy button: positioned inline after code

### Mobile (<640px)
- Same layout, panel becomes full-width
- Device code card: 12px padding (tighter)
- URL may truncate with ellipsis

---

## ASCII Layout

```text
Provider Accounts Section
+--------------------------------------------------+
| Provider Accounts                                |
| Manage your AI provider connections              |  ← section header
+--------------------------------------------------+
|                                                  |
| ┌──────────────────────────────────────────────┐ |
| │ [●] OpenAI Codex               ● Connected   │ |  ← StatusPill (green)
| │     Scopes: chat, code                       │ |
| │     Expires: Mar 15, 2026                    │ |
| └──────────────────────────────────────────────┘ |
|                                                  |
| ┌──────────────────────────────────────────────┐ |
| │ [○] GitHub Copilot               [Connect]   │ |  ← action button
| │     Not connected                            │ |
| └──────────────────────────────────────────────┘ |
|                                                  |
| ┌──────────────────────────────────────────────┐ |
| │ [○] GitHub Copilot               [Cancel]    │ |  ← polling state
| │                                              │ |
| │  ┌ Verify your account ─────────────────────┐│ |
| │  │                                          ││ |
| │  │  1. Open github.com/login/device         ││ |  ← clickable link
| │  │                                          ││ |
| │  │  2. Enter code:                          ││ |
| │  │                                          ││ |
| │  │     ABCD-1234              [📋]          ││ |  ← large mono + copy
| │  │                                          ││ |
| │  │  ● ● ●  Waiting for approval...          ││ |  ← animated dots
| │  │  Expires in 8:42                         ││ |
| │  └──────────────────────────────────────────┘│ |
| └──────────────────────────────────────────────┘ |
|                                                  |
| ┌──────────────────────────────────────────────┐ |
| │ [○] Claude Code                   ● Imported │ |  ← StatusPill (muted)
| │     Via CLI credentials                      │ |
| └──────────────────────────────────────────────┘ |
|                                                  |
+--------------------------------------------------+

Mobile (<640px) - same structure, full-width panel
```

---

## Component Inventory

### ProviderAccountsSection

**Purpose**: Container for provider list with section header.

**Props**:
```typescript
interface ProviderAccountsSectionProps {
  status: ConnectionStatus;
  accountProviders: ChatAccountProviderStatus[];
  onRefresh: () => void;
  startLogin: (provider: string) => Promise<ChatDeviceCodeSession>;
  pollLogin: (provider: string, session: ChatDeviceCodeSession) => Promise<ChatDeviceCodePollResult>;
}
```

**Structure**:
- Section title: "Provider Accounts"
- Description text: "Manage your AI provider connections"
- Provider list: maps `accountProviders` to `ProviderRow` components
- Empty state: "No providers available" when `accountProviders.length === 0`
- Disconnected state: "Connect to gateway to manage providers" when `status !== "connected"`

**States**:
- Loading: spinner while initial fetch
- Empty: helpful message when no providers
- Disconnected: message explaining gateway requirement
- Populated: list of provider rows

---

### ProviderRow

**Purpose**: Single provider with status display, action button, and expandable device code UI.

**Props**:
```typescript
interface ProviderRowProps {
  provider: ChatAccountProviderStatus;
  authState: ProviderAuthState;
  onConnect: () => void;
  onCancel: () => void;
}
```

**Variants/States**:

| State | Visual | Action Button |
|-------|--------|---------------|
| `idle` (not logged in) | Muted icon, "Not connected" | "Connect" (primary) |
| `starting` | Same as idle | "Connecting..." (disabled, subtle pulse) |
| `polling` | Expanded with device code card | "Cancel" (secondary) |
| `authorized` | Green StatusDot, StatusPill "Connected" | None (or "Disconnect" if supported) |
| `denied` | Warning icon, danger text | "Try Again" (primary) |
| `expired` | Warning icon, danger text | "Try Again" (primary) |
| `error` | Warning icon, danger text | "Try Again" (primary) |

**Composition**:
```
┌─────────────────────────────────────────────────┐
│ [Icon]  Provider Name           [StatusPill/Btn]│
│         Status text / scopes / expiry           │
│ ┌ Device Code Card (conditionally rendered) ───┐│
│ │                                              ││
│ └──────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

**Animation**:
- Height expansion: CSS Grid `grid-template-rows: 0fr → 1fr`, 200ms `ease-move`
- Status changes: color crossfade 200ms

---

### DeviceCodeInline

**Purpose**: Device code verification UI shown when polling for authorization.

**Props**:
```typescript
interface DeviceCodeInlineProps {
  session: { verificationUrl: string; userCode: string };
  expiresAt: string;
}
```

**Structure**:
1. **Header**: "Verify your account" (text-secondary, uppercase tracking)
2. **Step 1**: "Open [URL]" — clickable link, opens new tab
3. **Step 2**: "Enter code:" label + large monospace code + copy button
4. **Waiting indicator**: Animated dots + "Waiting for approval..."
5. **Countdown**: "Expires in MM:SS" — updates every second

**Animation**:
- Entry: `homie-fade-in` (translateY(4px→0), opacity, 160ms)
- Dots: existing `.homie-dots` animation

---

### StatusPill

**Purpose**: Small badge showing connection state.

**Props**:
```typescript
interface StatusPillProps {
  status: "connected" | "imported" | "error";
  children: React.ReactNode;
}
```

**Variants**:
| Status | Background | Text | Dot |
|--------|------------|------|-----|
| `connected` | `success-dim` | `success` | Green |
| `imported` | `surface-1` | `text-secondary` | None |
| `error` | `danger-dim` | `danger` | None |

**Styling**:
- Padding: 4px 8px
- Border radius: 4px
- Font: 11px, 500 weight, uppercase tracking
- Fixed min-width per variant to prevent layout shift

---

### CopyButton

**Purpose**: Icon button to copy verification code to clipboard.

**Props**:
```typescript
interface CopyButtonProps {
  text: string;
  "aria-label": string;
}
```

**States**:
- Default: Copy icon (clipboard)
- Copied: Checkmark icon, reverts after 2s

**Styling**:
- Size: 32x32px (touch target padded to 44px)
- Icon: 16x16px
- Background: transparent, hover: `surface-1`
- Focus ring: 2px primary

---

## Interaction and State Matrix

### Primary Actions

| Action | Trigger | Result |
|--------|---------|--------|
| Connect | Click "Connect" button | Start device code flow, expand row |
| Cancel | Click "Cancel" button | Cancel polling, collapse row |
| Retry | Click "Try Again" button | Restart device code flow |
| Copy code | Click copy button | Copy to clipboard, show confirmation |
| Open URL | Click verification link | Open in new tab |

### State Transitions

```
idle ─────────► starting ─────────► polling
  ▲                                   │ │ │
  │                                   │ │ │
  └───────────────────────────────────┘ │ │
              (cancel)                  │ │
                                        │ │
  ┌─────────────────────────────────────┘ │
  │ (success)                             │
  ▼                                       │
authorized ◄────────────────────┐         │
                                │         │
                                │         │
  ┌─────────────────────────────┴─────────┘
  │ (denied/expired/error)
  ▼
denied/expired/error ─► idle (on retry)
```

### Hover/Focus/Active/Disabled

| Element | Hover | Focus | Active | Disabled |
|---------|-------|-------|--------|----------|
| Connect button | bg: `surface-1` | ring: 2px primary | bg: `surface-2` | opacity: 0.5, no pointer |
| Cancel button | bg: `surface-1` | ring: 2px primary | bg: `surface-2` | — |
| Copy button | bg: `surface-1` | ring: 2px primary | scale: 0.95 | — |
| URL link | underline | ring: 2px primary | — | — |
| Provider row | — | outline: none (not focusable) | — | — |

### Loading/Empty/Error States

| State | Component | Visual |
|-------|-----------|--------|
| Gateway disconnected | ProviderAccountsSection | "Connect to gateway to manage providers" message |
| No providers | ProviderAccountsSection | "No providers configured" message |
| Auth error | ProviderRow | Danger-colored message, "Try Again" button |
| Auth denied | ProviderRow | "Access denied" text in danger color |
| Auth expired | ProviderRow | "Code expired" text in danger color |

### Validation and Inline Feedback

- Countdown timer: Updates every second, shows remaining time in MM:SS
- Copy feedback: Icon changes to checkmark for 2s, then reverts
- Auth success: Row collapses, status updates to "Connected" with animation

---

## Visual System

### Color Roles

| Role | Light Mode | Dark Mode | Usage |
|------|------------|-----------|-------|
| Surface | `--surface-0` | `--surface-0` | Card/row background |
| Surface elevated | `--surface-1` | `--surface-1` | Device code card, hover states |
| Text primary | `--text-primary` | `--text-primary` | Provider names |
| Text secondary | `--text-secondary` | `--text-secondary` | Descriptions, labels |
| Text tertiary | `--text-tertiary` | `--text-tertiary` | Timestamps, hints |
| Success | `--success` | `--success` | Connected indicator |
| Danger | `--danger` | `--danger` | Error text |
| Border | `--border` | `--border` | Row/card borders |

### Typography Scale

| Element | Size | Weight | Tracking | Font |
|---------|------|--------|----------|------|
| Provider name | 14px | 500 | normal | System |
| Status text | 13px | 400 | normal | System |
| Device code | 18px | 600 | 0.05em | Mono (`font-mono`) |
| Step labels | 13px | 400 | normal | System |
| Section header | 14px | 600 | -0.02em | System |
| StatusPill | 11px | 500 | 0.03em | System |
| Countdown | 12px | 400 | normal | Mono |

### Spacing and Sizing

| Element | Spacing |
|---------|---------|
| Section padding | 0 (within panel's 16px padding) |
| Row padding | 16px |
| Row gap (between rows) | 8px |
| Device code card margin-top | 12px |
| Device code card padding | 16px (desktop), 12px (mobile) |
| Button min-height | 44px |
| Icon size | 16px (provider), 20px (status dot) |

### Iconography

| Icon | Usage | Source |
|------|-------|--------|
| Provider icons | Per-provider (fallback: KeyRound) | Lucide or custom |
| Copy | Copy button default | `lucide-react/Clipboard` |
| Check | Copy success | `lucide-react/Check` |
| AlertCircle | Error state | `lucide-react/AlertCircle` |
| ExternalLink | URL link decoration | `lucide-react/ExternalLink` |

---

## Accessibility

### Keyboard Navigation

| Key | Action |
|-----|--------|
| Tab | Move between interactive elements (buttons, links) |
| Enter/Space | Activate button or link |
| Escape | Close settings panel (handled by parent) |

### Focus Order

1. Section content starts
2. First provider row's action button (if any)
3. Second provider row's action button
4. ... (continue for each provider)
5. When device code expanded: URL link → Copy button → Cancel button

### Focus States

- All interactive elements: 2px ring in `--ring` color
- Buttons: ring offset 2px
- Links: underline + ring

### Contrast Targets

| Element | Minimum Ratio |
|---------|---------------|
| Text primary on surface-0 | 7:1 |
| Text secondary on surface-0 | 4.5:1 |
| Success on success-dim | 4.5:1 |
| Danger on danger-dim | 4.5:1 |

### ARIA Notes

| Element | ARIA |
|---------|------|
| Section container | `role="tabpanel"`, `aria-labelledby="settings-tab-providers"` |
| Provider list | `role="list"` (implicit via semantic HTML) |
| Provider row | `role="listitem"` (implicit) |
| Device code | `aria-live="assertive"` — announce code when displayed |
| Status changes | `aria-live="polite"` — announce connection success/failure |
| Copy button | `aria-label="Copy verification code"` |
| URL link | `target="_blank"`, `rel="noopener noreferrer"` |
| StatusPill | Text content is the label; no additional aria needed |

### Screen Reader Announcements

- When device code appears: "Verification code: ABCD-1234"
- When authorized: "[Provider] connected successfully"
- When denied: "[Provider] access denied"
- When expired: "[Provider] code expired"

---

## Content Notes

### Copy Tone

- Neutral, instructional
- No jargon — "Connect" not "Authenticate"
- Error messages: brief, actionable

### Copy Examples

| Context | Copy |
|---------|------|
| Section description | "Manage your AI provider connections" |
| Not connected | "Not connected" |
| Connected | "Connected" |
| Imported | "Imported via CLI" |
| Step 1 | "Open [URL]" |
| Step 2 | "Enter code:" |
| Waiting | "Waiting for approval..." |
| Countdown | "Expires in 8:42" |
| Error: denied | "Access denied" |
| Error: expired | "Code expired" |
| Error: generic | "Connection failed" |
| Retry button | "Try Again" |
| Gateway disconnected | "Connect to gateway to manage providers" |
| No providers | "No providers configured" |

### Empty State Copy

**Gateway disconnected**:
```
Connect to gateway to manage providers
```

**No providers available**:
```
No providers configured

Configure providers in your gateway settings.
```

### Error Messaging Guidelines

- Be specific: "Access denied" vs "Error"
- Be actionable: Always show "Try Again" button
- Keep it brief: One short sentence max
- Use danger color for error text, not for entire row

---

## Animation and Motion Spec

### Height Expansion (ProviderRow)

```css
.homie-row-expandable {
  display: grid;
  grid-template-rows: 0fr;
  transition: grid-template-rows var(--duration-standard) var(--ease-move);
}

.homie-row-expandable[data-expanded="true"] {
  grid-template-rows: 1fr;
}

.homie-row-expandable > div {
  overflow: hidden;
}
```

### Device Code Fade-In

Reuse existing `.homie-fade-in`:
```css
@keyframes homie-fade-in {
  from { opacity: 0; transform: translateY(4px); }
  to   { opacity: 1; transform: translateY(0); }
}
.homie-fade-in {
  animation: homie-fade-in 160ms cubic-bezier(0.25, 1, 0.5, 1) both;
}
```

### Status Pill Color Transition

```css
.homie-status-pill {
  transition: background-color var(--duration-standard) var(--ease-move),
              color var(--duration-standard) var(--ease-move);
}
```

### Waiting Dots

Reuse existing `.homie-dots`:
```css
.homie-dots span {
  animation: homie-dot 1200ms ease-in-out infinite;
}
.homie-dots span:nth-child(2) { animation-delay: 160ms; }
.homie-dots span:nth-child(3) { animation-delay: 320ms; }
```

### Copy Button Feedback

```css
.homie-copy-success {
  animation: homie-fade-in 160ms var(--ease-enter) both;
}
```

### Reduced Motion

All animations respect existing `prefers-reduced-motion` media query which zeroes duration vars.

---

## Quality Checklist

- [x] Requirements and constraints captured
- [x] Clear layout hierarchy for each breakpoint
- [x] ASCII layout diagram included
- [x] Components and states listed
- [x] Existing tokens/components reused
- [x] New components defined with clear purpose
- [x] Accessibility guidance documented
- [x] Rationale provided for key decisions
- [x] Animation spec matches existing system
- [x] Content/copy guidelines provided
