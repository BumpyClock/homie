# Settings Redesign: Sectioned Navigation + Provider Login

> **Bead**: remotely-o1v | **Date**: 2026-02-16
> **Disciplines applied**: UX Design, Web Animation Design, Design Engineering

---

## 1. Current State Analysis

### Mobile (`settings.tsx` — 483 LOC, single scroll)
- **Connection Card** — gateway status hero
- **Gateway Target Card** — URL form
- **App Defaults & Help Card** — theme, model, model list, provider auth, tips (everything dumped here)
- Provider auth buried 3 levels deep inside "App Defaults & Help"
- Drawer has "Quick Links" but they don't scroll-to-section
- Wide layout (≥1080px) splits into 2 columns, but sections aren't independently navigable

### Web (no settings page)
- Settings scattered: target selector + theme toggle in `GatewayHeader`
- Gateway details in modal (`GatewayDetailsModal`)
- **Zero** provider auth UI — no way to login from web
- No dedicated settings surface at all

### Problems
| Problem | Impact |
|---------|--------|
| Provider auth buried in "Defaults & Help" | Users can't find login flow |
| Mobile settings is one long scroll | Cognitive overload, no wayfinding |
| Web has no settings page | Can't configure providers on web |
| Auth logic lives in mobile `settings.tsx` | Can't reuse on web |
| No auth state machine | Polling loops are ad-hoc, error states unclear |

---

## 2. Information Architecture

### Settings Sections (both platforms)

```
Settings
├── Connection        — gateway status, target URL, transport state
├── Provider Accounts — per-provider login status + device-code auth
├── Preferences       — theme, default model, effort, permissions
└── About             — version, tips, links
```

**Rationale**: Four sections map to distinct user mental models:
1. "Am I connected?" (Connection)
2. "Which AI providers are linked?" (Provider Accounts)
3. "How should the app behave?" (Preferences)
4. "What version / help?" (About)

### Navigation Pattern

| Platform | Pattern | Rationale |
|----------|---------|-----------|
| **Web** | Vertical sidebar tabs (left rail) | Desktop has horizontal space; sidebar is scan-friendly, keeps content stable |
| **Mobile (phone)** | Segmented control (top) or scrollable chip bar | Compact, thumb-reachable, familiar iOS/Android pattern |
| **Mobile (tablet/wide)** | Sidebar + content pane (master-detail) | Reuse web pattern on wide screens |

---

## 3. Web Settings Design

### Layout: Settings Panel (slide-over or dedicated route)

**Entry point**: Gear icon in `GatewayHeader` (right side, next to theme selector).

**Panel structure** (slide-over from right, 480px max-width on desktop; full-width on ≤640px):

```
┌─────────────────────────────────────────┐
│  ← Settings                        [×]  │  ← sticky header
├────────┬────────────────────────────────┤
│        │                                │
│ ● Conn │  [Active section content]      │
│ ● Prov │                                │
│ ● Pref │                                │
│ ● Abou │                                │
│        │                                │
├────────┴────────────────────────────────┤
│  (footer: version)                      │
└─────────────────────────────────────────┘
```

On narrow viewports (≤640px), collapse sidebar into a horizontal tab bar at the top:

```
┌─────────────────────────────────────────┐
│  ← Settings                        [×]  │
│  [Conn] [Providers] [Prefs] [About]     │
├─────────────────────────────────────────┤
│                                         │
│  [Active section content]               │
│                                         │
└─────────────────────────────────────────┘
```

### Provider Accounts Section (Web)

```
Provider Accounts
─────────────────────────────────────
Each provider row:
┌─────────────────────────────────────┐
│  [Icon]  OpenAI Codex     ● Connected │  ← StatusPill
│          Scopes: chat, code           │
│          Expires: 2026-03-15          │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  [Icon]  GitHub Copilot   [Connect] │  ← action button
│          Not connected              │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  [Icon]  Claude Code      ● Imported│
│          Via CLI credentials        │
└─────────────────────────────────────┘
```

**Device-code flow inline** (expands under provider row on "Connect"):
```
┌─────────────────────────────────────┐
│  [Icon]  GitHub Copilot             │
│                                     │
│  ┌─ Device Code ──────────────────┐ │
│  │  Visit: github.com/login/device│ │
│  │  Code:  ABCD-1234              │ │
│  │  ─────── ● ● ● ────────────── │ │  ← progress dots
│  │  Waiting for authorization...  │ │
│  └────────────────────────────────┘ │
└─────────────────────────────────────┘
```

### Component Inventory (Web — new)

| Component | Purpose |
|-----------|---------|
| `SettingsPanel` | Slide-over container with sidebar/tab nav |
| `SettingsNav` | Sidebar tabs (desktop) / horizontal tabs (mobile) |
| `SettingsSection` | Generic section wrapper with title |
| `ConnectionSection` | Gateway status + target form (extract from header) |
| `ProviderAccountsSection` | Provider list + auth flows |
| `ProviderRow` | Single provider: icon, name, status, action |
| `DeviceCodeInline` | Expandable device-code verification UI |
| `PreferencesSection` | Theme, model, effort, permissions |
| `AboutSection` | Version, tips |

---

## 4. Mobile Settings Design

### Layout: Segmented Sections

Replace single scroll with a **segmented control** at the top + section content below.

```
┌─────────────────────────────────────┐
│  Settings                    [≡]    │  ← AppShell header + drawer
│                                     │
│  [Connection] [Providers] [Prefs]   │  ← SegmentedControl (scrollable)
│                                     │
│  ┌─────────────────────────────────┐│
│  │                                 ││
│  │  [Active section content]       ││  ← ScrollView per section
│  │                                 ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

Wide layout (≥1080px): Switch to sidebar-style master-detail (same as web pattern).

### Provider Accounts Section (Mobile)

Same row structure as web, adapted to React Native:
- `Pressable` rows with 44px min touch targets
- `StatusPill` for connected state
- Device-code flow expands inline with `LayoutAnimation` or `Reanimated` height transition
- Verification URL opens via `Linking.openURL`
- User code displayed in large mono font for easy copying
- Copy-to-clipboard button next to user code

### Component Inventory (Mobile — new/refactored)

| Component | Purpose |
|-----------|---------|
| `SettingsSegmentedControl` | Top tab bar for section switching |
| `ConnectionSection` | Extract from current `connectionCard` + `targetCard` |
| `ProviderAccountsSection` | Extract + enhance from current auth card |
| `ProviderRow` | Single provider row (shared layout logic) |
| `DeviceCodeCard` | Inline device-code verification |
| `PreferencesSection` | Extract from current `defaultsCard` |
| `AboutSection` | Tips + version (extract from current help card) |

---

## 5. Shared Architecture

### Shared Auth Controller (`src/apps/shared/src/hooks/useProviderAuth.ts`)

Extract the polling logic from mobile `settings.tsx` into a shared hook:

```typescript
interface ProviderAuthState {
  status: 'idle' | 'starting' | 'polling' | 'authorized' | 'denied' | 'expired' | 'error';
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

**State machine** (per provider):
```
idle → starting → polling ⇄ polling (slow_down)
                  polling → authorized → idle (after refresh)
                  polling → denied → idle
                  polling → expired → idle
     → error → idle (on retry)
```

### Shared Section Types (`src/apps/shared/src/settings-types.ts`)

```typescript
type SettingsSection = 'connection' | 'providers' | 'preferences' | 'about';
```

---

## 6. Animation & Motion Spec

### Design Principles
- **Purposeful**: Every animation communicates state change or spatial relationship
- **Fast**: Enter 200ms ease-out, exit 160ms ease-in (already in motion.ts)
- **Reduced-motion**: All animations respect `prefers-reduced-motion`

### Web Animations

| Element | Trigger | Animation | Duration | Easing |
|---------|---------|-----------|----------|--------|
| Settings panel | Open | Slide from right + fade | 200ms | `ease-out` (cubic-bezier(0, 0, 0.2, 1)) |
| Settings panel | Close | Slide to right + fade | 160ms | `ease-in` (cubic-bezier(0.4, 0, 1, 1)) |
| Section switch | Tab click | Content crossfade | 140ms | `ease-out` |
| Provider row expand | Connect click | Height reveal (grid-template-rows 0fr→1fr) | 200ms | `ease-move` (cubic-bezier(0.4, 0, 0.2, 1)) |
| Device code appear | Session received | Fade-in + translateY(4px→0) | 160ms | `ease-enter` |
| Status pill change | Auth complete | Background color crossfade | 200ms | `ease-move` |
| Overlay | Panel open/close | Opacity 0→0.3 / 0.3→0 | 200ms / 160ms | `linear` |

**CSS implementation** (extend existing `index.css` patterns):
```css
@keyframes homie-slide-in-right {
  from { opacity: 0; transform: translateX(16px); }
  to   { opacity: 1; transform: translateX(0); }
}
.homie-settings-enter {
  animation: homie-slide-in-right var(--duration-standard) var(--ease-enter) both;
}

@keyframes homie-slide-out-right {
  from { opacity: 1; transform: translateX(0); }
  to   { opacity: 0; transform: translateX(16px); }
}
.homie-settings-exit {
  animation: homie-slide-out-right var(--duration-fast) var(--ease-exit) both;
}
```

### Mobile Animations

| Element | Trigger | Animation | Config |
|---------|---------|-----------|--------|
| Section content | Tab switch | `FadeInRight` / `FadeOutLeft` | `duration.fast` (140ms), `easing.enter` |
| Provider row expand | Connect press | `withTiming` height | `duration.standard` (220ms), `easing.move` |
| Device code card | Session received | `FadeIn` + `SlideInDown(4)` | `duration.fast`, `easing.enter` |
| Status pill | Auth state change | Spring color transition | `spring.snappy` |
| Segmented control indicator | Tab switch | `withSpring` translateX | `spring.responsive` |
| Section initial load | Mount | Stagger children | `stagger.tight` (30ms) |

**Haptics** (extend `motion.haptics`):
```typescript
settingsTabSwitch: { kind: 'selection' },
providerConnect:   { kind: 'impact', style: ImpactFeedbackStyle.Light },
providerAuthorized:{ kind: 'notification', style: NotificationFeedbackType.Success },
providerDenied:    { kind: 'notification', style: NotificationFeedbackType.Warning },
```

### Reduced Motion
- Web: All `--duration-*` vars already zero'd in `@media (prefers-reduced-motion: reduce)`
- Mobile: Check `AccessibilityInfo.isReduceMotionEnabled()` or use Reanimated's `useReducedMotion()` — skip spring/timing, apply instant layout

---

## 7. Accessibility Spec

### Touch Targets
- All interactive elements: min 44×44px (already in `touchTarget.min`)
- Provider "Connect" button: 44px height, generous horizontal padding

### Keyboard Navigation (Web)
- Settings panel: focus trap when open
- Tab order: nav tabs → section content → action buttons
- `Escape` closes panel, returns focus to gear trigger
- Arrow keys navigate sidebar tabs
- `Enter`/`Space` activates tab or button

### Screen Reader
- Settings panel: `role="dialog"`, `aria-label="Settings"`
- Nav tabs: `role="tablist"` + `role="tab"` + `aria-selected`
- Section content: `role="tabpanel"` + `aria-labelledby`
- Provider status: `aria-live="polite"` on status pill container
- Device code: announce "Verification code: ABCD-1234" via `aria-live="assertive"`
- Auth state changes: announce "OpenAI Codex connected" via live region

### No Layout Shift
- Provider rows have fixed min-height whether collapsed or expanded
- Device-code expansion uses CSS Grid `grid-template-rows` for smooth reveal (no content jump)
- Status pill has fixed width per state (prevent text-width layout thrashing)

---

## 8. Implementation Plan

### Phase 1: Shared Foundation
1. **`useProviderAuth` hook** in `src/apps/shared/src/hooks/` — extract polling state machine
2. **Settings section types** in `src/apps/shared/src/settings-types.ts`
3. Update bead `remotely-o1v.1` (shared controller)

### Phase 2: Web Settings Panel
1. **`SettingsPanel`** slide-over component with overlay
2. **`SettingsNav`** sidebar tabs / horizontal tabs
3. **`ConnectionSection`** — extract target management from header
4. **`ProviderAccountsSection`** + **`ProviderRow`** + **`DeviceCodeInline`**
5. **`PreferencesSection`** — theme selector (extract from header)
6. **`AboutSection`** — version/tips
7. Gear icon trigger in `GatewayHeader`
8. CSS animations for panel + section transitions
9. Update bead `remotely-o1v.2` (web settings)

### Phase 3: Mobile Settings Refactor
1. **`SettingsSegmentedControl`** component
2. Split `settings.tsx` into section components
3. **`ProviderAccountsSection`** using shared `useProviderAuth`
4. Wire up Reanimated transitions + haptics
5. Update bead `remotely-o1v.3` (mobile settings)

### Phase 4: Chat Cleanup
1. Remove auth blocks from chat surfaces
2. Add "Go to Settings" redirect when unauthorized provider is selected
3. Update bead `remotely-o1v.4` (chat cleanup)

### Phase 5: Polish & Tests
1. Keyboard/screen-reader audit (web)
2. VoiceOver/TalkBack audit (mobile)
3. Reduced-motion verification
4. Smoke tests for device-code flow
5. Update bead `remotely-o1v.5` (docs + tests)

---

## 9. File Map (Planned)

```
src/apps/shared/src/
  hooks/
    useProviderAuth.ts          ← NEW: shared auth state machine
  settings-types.ts             ← NEW: section types

src/web/src/
  components/
    settings/
      SettingsPanel.tsx          ← NEW: slide-over container
      SettingsNav.tsx            ← NEW: sidebar/horizontal tabs
      ConnectionSection.tsx      ← NEW: gateway status + target
      ProviderAccountsSection.tsx← NEW: provider list + auth
      ProviderRow.tsx            ← NEW: single provider row
      DeviceCodeInline.tsx       ← NEW: device-code verification
      PreferencesSection.tsx     ← NEW: theme, model defaults
      AboutSection.tsx           ← NEW: version, tips
  index.css                      ← EDIT: add settings animations

src/apps/mobile/
  app/(tabs)/settings.tsx        ← REFACTOR: split into sections
  components/settings/
    SettingsSegmentedControl.tsx  ← NEW: top tab bar
    ConnectionSection.tsx        ← NEW: extract from settings.tsx
    ProviderAccountsSection.tsx  ← NEW: extract + enhance
    ProviderRow.tsx              ← NEW: single provider row
    DeviceCodeCard.tsx           ← NEW: inline verification
    PreferencesSection.tsx       ← NEW: extract from settings.tsx
    AboutSection.tsx             ← NEW: tips + version
  theme/motion.ts                ← EDIT: add settings haptics
```

---

## 10. Visual Reference: Provider Row States

```
┌─ idle ──────────────────────────────────┐
│  [○]  GitHub Copilot          [Connect] │
│       Not connected                     │
└─────────────────────────────────────────┘

┌─ connecting ────────────────────────────┐
│  [○]  GitHub Copilot      [Connecting…] │  ← button disabled, subtle pulse
│       Starting device code flow...      │
└─────────────────────────────────────────┘

┌─ polling (device-code visible) ─────────┐
│  [○]  GitHub Copilot        [Cancel]    │
│                                         │
│  ┌ Verify your account ───────────────┐ │
│  │                                    │ │
│  │  1. Open  github.com/login/device  │ │  ← tappable/clickable link
│  │  2. Enter code:                    │ │
│  │                                    │ │
│  │     ABCD-1234           [Copy]     │ │  ← large mono, copy button
│  │                                    │ │
│  │  ● ● ●  Waiting for approval...   │ │  ← animated dots
│  │  Expires in 8:42                   │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘

┌─ authorized ────────────────────────────┐
│  [●]  GitHub Copilot      ● Connected   │  ← green StatusPill
│       Scopes: chat · code               │
│       Expires: 2026-03-15               │
└─────────────────────────────────────────┘

┌─ error (denied/expired) ────────────────┐
│  [○]  GitHub Copilot       [Try Again]  │
│       ⚠ Access denied.                  │  ← danger color
└─────────────────────────────────────────┘
```

---

## 11. Decision Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Settings surface (web) | Slide-over panel | Doesn't require route change; stays in context; matches utility-density direction |
| Settings surface (mobile) | Segmented control in existing tab | Already has settings tab; segmented control adds wayfinding without new navigation |
| Section count | 4 (Connection, Providers, Preferences, About) | Minimal viable sections; each serves distinct mental model |
| Auth state machine | Shared hook, not Redux/Zustand | Project uses hooks + context; no global store; hook is portable |
| Device-code UX | Inline expand (not modal) | Modal interrupts; inline expand keeps spatial context |
| Wide mobile layout | Sidebar master-detail | Matches web pattern; reduces code divergence |
