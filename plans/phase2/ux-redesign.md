# Homie Mobile — UX Redesign Plan

> Precision + Utility with technical warmth. Linear meets Raycast, on mobile.

---

## Table of Contents

1. [Design Direction](#1-design-direction)
2. [Navigation Architecture](#2-navigation-architecture)
3. [Screen-by-Screen Redesign](#3-screen-by-screen-redesign)
4. [Component Inventory](#4-component-inventory)
5. [Interaction Patterns](#5-interaction-patterns)
6. [Empty States & Onboarding](#6-empty-states--onboarding)
7. [Connection & Status System](#7-connection--status-system)
8. [Responsive Strategy](#8-responsive-strategy)

---

## 1. Design Direction

### 1.1 Personality

Homie is a **precision tool for operators**. It should feel like a command center you can trust — not a consumer chat app. The personality sits at the intersection of:

- **Linear** — Information-dense, keyboard-driven, monochrome with accent pops
- **Raycast** — Speed-oriented, contextual, dark-first, minimal decoration
- **Things 3** — Tactile microinteractions, confident whitespace, considered motion

Every pixel must earn its place. No decorative borders unless they communicate hierarchy. No color unless it carries semantic meaning.

### 1.2 Color Foundation

**Dark palette (primary):**

```
Background      #0B1018    – True dark, not pure black (OLED-friendly, avoids smearing)
Surface-0       #111921    – Card/panel base
Surface-1       #18222D    – Elevated cards, drawer panel
Surface-2       #1F2B38    – Active states, hover, raised elements
Surface-3       #273545    – Input fields, wells

Text-Primary    #E8EDF3    – High contrast, not pure white
Text-Secondary  #7B8A9C    – De-emphasized labels, timestamps
Text-Tertiary   #4A5768    – Disabled, placeholder

Accent          #4FA4FF    – Primary action, active states, links
Accent-Dim      rgba(79, 164, 255, 0.12) – Accent backgrounds, subtle highlights

Success         #43C38A
Success-Dim     rgba(67, 195, 138, 0.12)

Warning         #F0B44D
Warning-Dim     rgba(240, 180, 77, 0.12)

Danger          #F06A80
Danger-Dim      rgba(240, 106, 128, 0.12)

Border          rgba(255, 255, 255, 0.06)
Border-Active   rgba(255, 255, 255, 0.12)
```

**Light palette (secondary, system-driven):**

```
Background      #F5F7FA
Surface-0       #FFFFFF
Surface-1       #F0F2F6
Surface-2       #E8ECF2
Surface-3       #DDE2EA

Text-Primary    #0F1720
Text-Secondary  #5B6878
Text-Tertiary   #97A3B3

Accent          #0A78E8
Accent-Dim      rgba(10, 120, 232, 0.08)
```

**Key rules:**
- Surface layers create depth via value shift, not via borders or shadows
- Borders are used only to separate interactive regions (inputs, cards that need tap targets)
- Accent color is used sparingly: active nav items, primary buttons, focus rings
- Status colors (success/warning/danger) only appear in StatusPill, badges, and inline alerts — never as decoration

### 1.3 Typography

Expand the type scale for more granular hierarchy:

```
Display     28 / 34  -0.4 tracking  Bold (700)     — Screen titles only
Heading     22 / 28  -0.3 tracking  Semibold (600) — Section headers  (NEW)
Title       17 / 24  -0.2 tracking  Semibold (600) — Card titles, drawer header
Body        15 / 22   0.0 tracking  Regular (400)  — Message text, descriptions
Body-Medium 15 / 22   0.0 tracking  Medium (500)   — Emphasized body text
Caption     13 / 18   0.1 tracking  Medium (500)   — Timestamps, metadata
Label       12 / 16   0.3 tracking  Semibold (600) — Pill text, section labels
Mono        13 / 18   0.0 tracking  Regular (400)  — Terminal data, code, IDs
                                                      Font: SpaceMono
```

**Rules:**
- Only `Display` uses Bold; everything else is Semibold or lighter
- UPPERCASE is reserved for: section labels in drawer, status pill text, nothing else
- Letter-spacing > 0.2 only on text < 13pt
- Line height ratio: 1.35-1.47x for readability on small screens

### 1.4 Depth & Elevation Strategy

**No shadows.** Depth is communicated through surface value stepping:

```
Layer 0: Background (#0B1018)  — Full bleed behind everything
Layer 1: Surface-0 (#111921)   — Drawer panel, content cards
Layer 2: Surface-1 (#18222D)   — Active thread row, composer card
Layer 3: Surface-2 (#1F2B38)   — Input fields, pressed states
Layer 4: Surface-3 (#273545)   — Overlays, bottom sheets
```

The only shadow in the entire app: the drawer panel on phone, using a 24px blur at 20% opacity black, to create separation from dimmed content.

### 1.5 Iconography

- **Icon set:** Migrate fully to `lucide-react-native` (consistent with Feather but larger set, active maintenance)
- **Icon size:** 18px standard, 14px inline/small, 22px for nav items
- **Icon color:** defaults to `Text-Secondary`; active/selected uses `Accent`; destructive actions use `Danger`
- **No filled icons** — outline only, 1.5px stroke weight

### 1.6 Spacing System

Tighten the scale for information density:

```
2     — Micro (icon gaps, inline element nudges)
4     — XS   (intra-component gaps)
6     — SM   (pill padding, tight card gutters)
8     — MD   (inter-element gaps within a card)
12    — LG   (card internal padding, section gaps)
16    — XL   (between cards, drawer section gaps)
24    — 2XL  (screen-level sections, major separators)
32    — 3XL  (top safe-area to content)
```

### 1.7 Corner Radius

```
4     — Micro (inline badges, small pills)
8     — SM   (buttons, input fields, thread rows)
12    — MD   (cards, drawer panel)
16    — LG   (bottom sheets, modals)
999   — Pill  (status pills, tags)
```

---

## 2. Navigation Architecture

### 2.1 Current Problem

The current routing is broken:
- `(tabs)/_layout.tsx` declares a `Stack` (not Tabs), yet the group is called `(tabs)`
- `index.tsx` is a 805-line monolith that renders 3 different "sections" via an internal `section` state variable
- `terminals.tsx` and `settings.tsx` exist as route files but are never navigated to — they're orphaned stubs
- The drawer is hand-rolled with `PanResponder`, not using any navigation library

### 2.2 Proposed Architecture

```
app/
  _layout.tsx              ← Root layout: providers, global state
  (app)/
    _layout.tsx            ← AppShell: drawer + content frame
    index.tsx              ← Chat screen (default route)
    chat/
      [threadId].tsx       ← Deep-link to specific thread
    terminals/
      index.tsx            ← Terminal list + detail split
      [sessionId].tsx      ← Deep-link to specific session
    settings/
      index.tsx            ← Settings screen
      gateway.tsx          ← Gateway configuration (dedicated)
      appearance.tsx       ← Theme/appearance settings
      about.tsx            ← App info, version, diagnostics
    onboarding/
      index.tsx            ← First-run gateway setup
```

**Key decisions:**

1. **Rename `(tabs)` → `(app)`** — This group has never been tabs; naming it correctly prevents confusion.

2. **Extract `AppShell` into `(app)/_layout.tsx`** — The drawer, header bar, and section management become a layout component, not screen-level code. This layout:
   - Owns the drawer open/close state and gesture handlers
   - Renders the left panel (nav + sublists)
   - Renders the right content area via `<Slot />`
   - Provides drawer context to children via React Context

3. **Each section becomes its own route** — Chat, Terminals, Settings are real routes navigated via `router.replace()`. No more internal `section` state switching.

4. **Thread deep-linking** — `chat/[threadId].tsx` enables push notifications to open a specific thread, and URL-based sharing.

5. **Delete orphaned stubs** — Remove current `terminals.tsx` and `settings.tsx` from `(tabs)`, replace with proper route files.

### 2.3 Navigation Flow Diagram

```
┌─────────────────────────────────────────────────────┐
│                     AppShell                         │
│  ┌──────────────┐  ┌────────────────────────────┐   │
│  │  Left Panel  │  │       Content Area          │   │
│  │              │  │                              │   │
│  │  ┌────────┐  │  │  ┌──────────────────────┐   │   │
│  │  │Identity│  │  │  │    Header Bar         │   │   │
│  │  │ Badge  │  │  │  │ [≡] Title    [Status] │   │   │
│  │  └────────┘  │  │  └──────────────────────┘   │   │
│  │              │  │                              │   │
│  │  ┌────────┐  │  │  ┌──────────────────────┐   │   │
│  │  │ Chat   │◄─┼──┼──│     <Slot />          │   │   │
│  │  │ Term   │  │  │  │  (route content)      │   │   │
│  │  │ Settng │  │  │  │                        │   │   │
│  │  └────────┘  │  │  └──────────────────────┘   │   │
│  │              │  │                              │   │
│  │  ── ── ── ── │  │                              │   │
│  │              │  │                              │   │
│  │  ┌────────┐  │  │                              │   │
│  │  │Sub-list│  │  │                              │   │
│  │  │Threads │  │  │                              │   │
│  │  │or Sess │  │  │                              │   │
│  │  └────────┘  │  │                              │   │
│  └──────────────┘  └────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

### 2.4 Drawer Behavior

**Phone:**
- Drawer hidden by default
- Open: edge swipe (left 24px) or tap header menu button
- Close: swipe left on panel, tap backdrop, or tap any nav item
- Width: `min(320, 86vw)`
- Backdrop: `rgba(0, 0, 0, 0.45)` with animated opacity

**Tablet (≥600dp):**
- Drawer always visible, persistent left panel
- Width: 300dp fixed
- No backdrop, no swipe gestures
- 1px right border separating panel from content

### 2.5 Gesture Handler Migration

Replace `PanResponder` with `react-native-gesture-handler`'s `Gesture.Pan()` API:
- More reliable on both platforms
- Proper gesture composition with ScrollView inside drawer
- Native driver animations via Reanimated worklets
- Better handling of simultaneous gestures (scroll + swipe)

---

## 3. Screen-by-Screen Redesign

### 3.1 Chat Screen (Default — `index.tsx`)

The primary screen. This is where 80%+ of user time is spent.

```
PHONE LAYOUT
┌─────────────────────────────────┐
│ ≡   Chat               ● conn  │  ← Compact header: 48px
├─────────────────────────────────┤
│                                 │
│  ┌─ G ── Gateway ── 2:14 ───┐  │  ← Turn group: avatar + label + time
│  │  Response text rendered   │  │
│  │  as markdown with code    │  │
│  │  blocks and lists...      │  │
│  └───────────────────────────┘  │
│                                 │
│  ┌─ Y ── You ── 2:13 ───────┐  │
│  │  User message plain text  │  │
│  └───────────────────────────┘  │
│                                 │
│  ┌── ⚠ APPROVAL REQUIRED ───┐  │  ← Distinct approval card
│  │  rm -rf /tmp/build        │  │
│  │                           │  │
│  │  [Approve]   [Deny]       │  │  ← Full-width button row
│  │  [Approve for session]    │  │
│  └───────────────────────────┘  │
│                                 │
│  ●●● Typing...                  │  ← Streaming indicator (NEW)
│                                 │
├─────────────────────────────────┤
│ ⬡ claude-4     ↕ high          │  ← Pills row
│ ┌───────────────────────── ↑ ┐  │  ← Input + send
│ │ Message…                    │  │
│ └─────────────────────────────┘  │
└─────────────────────────────────┘
```

**Key changes from current:**

1. **Compact header** — Remove "Gateway" eyebrow. Single line: menu icon + "Chat" title + StatusPill. 48px total height. The eyebrow was adding noise for zero information gain since the user knows they're in the Homie app.

2. **Streaming indicator** — When `thread.running` is true and the last turn is from the assistant, show a pulsing dot row: `● ● ●` with a staggered fade animation. Text shows "Thinking…" for reasoning, "Running command…" for tool use, "Typing…" for normal response.

3. **Approval cards are visually distinct** — Amber left border (4px), warning-dim background, monospace command display, full-width action buttons instead of inline text. The card should INTERRUPT the visual flow clearly.

4. **Turn groups are tighter** — Remove redundant vertical padding between items within a turn. Avatar + label + timestamp in a single compact row. The avatar is 24px circle with a single letter, not large.

5. **Jump to latest** — Appears as a floating mini-FAB at bottom-right when scroll position is > 2 screens from latest message. Circular, 36px, with down-arrow icon.

6. **Copy action is inline** — Long-press shows a context menu (native `ActionSheet`). Single-tap on code blocks copies directly with haptic feedback.

### 3.2 Chat Screen — Empty State (No Active Thread)

```
┌─────────────────────────────────┐
│ ≡   Chat               ● conn  │
├─────────────────────────────────┤
│                                 │
│                                 │
│         ┌─────────────┐         │
│         │     ◇       │         │  ← Geometric icon, not illustration
│         │   Homie     │         │
│         └─────────────┘         │
│                                 │
│       Select a thread from      │
│       the sidebar, or start     │
│       a new conversation.       │
│                                 │
│       ┌─ + New Chat ──────┐     │  ← Primary action CTA
│       └───────────────────┘     │
│                                 │
│                                 │
├─────────────────────────────────┤
│ (composer disabled, grayed)     │
└─────────────────────────────────┘
```

### 3.3 Terminal List Screen (`terminals/index.tsx`)

```
PHONE LAYOUT
┌─────────────────────────────────┐
│ ≡   Terminals           ● conn │
├─────────────────────────────────┤
│                                 │
│  ┌─ Active Sessions ─────────┐  │
│  │                            │  │
│  │  ┌ zsh ── deimos ── 4m ─┐ │  │  ← Session card
│  │  │ 120x40  ● running     │ │  │
│  │  └────────────────────────┘ │  │
│  │                            │  │
│  │  ┌ bash ── phobos ── 2h ┐ │  │
│  │  │ 80x24  ● running      │ │  │
│  │  └────────────────────────┘ │  │
│  │                            │  │
│  └────────────────────────────┘  │
│                                 │
│  ┌─ Recent (Closed) ─────────┐  │  ← Grouped by status (NEW)
│  │                            │  │
│  │  ┌ zsh ── deimos ── 1d ─┐ │  │
│  │  │ 80x24  ○ closed       │ │  │
│  │  └────────────────────────┘ │  │
│  │                            │  │
│  └────────────────────────────┘  │
│                                 │
└─────────────────────────────────┘
```

**Key design decisions:**

1. **Section grouping** — Sessions grouped by status: "Active Sessions" and "Recent (Closed)". Active sessions show a green dot, closed show a hollow dot.

2. **Machine identity** — Each session card shows the machine name (e.g., "deimos") alongside the shell name. This is critical for multi-machine users.

3. **Tap to view** — Tapping a session navigates to `terminals/[sessionId]` which shows the terminal detail (and eventually the full terminal renderer).

4. **Pull to refresh** — Standard pull-to-refresh gesture refreshes the session list. No explicit "Refresh" button in the UI.

### 3.4 Terminal Detail Screen (`terminals/[sessionId].tsx`)

```
┌─────────────────────────────────┐
│ ←   zsh@deimos          ● live  │  ← Back button + session title
├─────────────────────────────────┤
│                                 │
│  ┌──────────────────────────┐   │
│  │ $ ls -la                 │   │  ← Terminal viewport (scrollable)
│  │ total 42                 │   │
│  │ drwxr-xr-x  5 user ...  │   │
│  │ -rw-r--r--  1 user ...  │   │
│  │                          │   │
│  │                          │   │
│  │                          │   │
│  │ $  █                     │   │
│  └──────────────────────────┘   │
│                                 │
├─────────────────────────────────┤
│ Session info                    │
│ Shell: /bin/zsh                 │
│ Resolution: 120 × 40           │
│ Started: 4 minutes ago         │
│ Machine: deimos                 │
└─────────────────────────────────┘
```

For Phase 2, the terminal viewport is a read-only scrollable view of the session's latest output snapshot. Full interactive terminal rendering (with keyboard input) is Phase 3.

The bottom section shows session metadata in a compact key-value layout.

### 3.5 Settings Screen (`settings/index.tsx`)

```
┌─────────────────────────────────┐
│ ≡   Settings                    │
├─────────────────────────────────┤
│                                 │
│  GATEWAY                        │  ← Section label (uppercase caption)
│  ┌──────────────────────────┐   │
│  │ Target URL               │   │  ← Tappable row → settings/gateway
│  │ ws://10.0.1.5:8080  ›    │   │
│  ├──────────────────────────┤   │
│  │ Status                   │   │
│  │ ● Connected    ›         │   │
│  └──────────────────────────┘   │
│                                 │
│  IDENTITY                       │
│  ┌──────────────────────────┐   │
│  │ Device Name              │   │  ← (NEW) User's device identifier
│  │ iPhone 15 Pro            │   │
│  ├──────────────────────────┤   │
│  │ Connected Machine        │   │  ← (NEW) Remote machine info
│  │ deimos (Arch Linux)      │   │
│  └──────────────────────────┘   │
│                                 │
│  APPEARANCE                     │
│  ┌──────────────────────────┐   │
│  │ Theme                    │   │
│  │ System ›                 │   │  ← System / Dark / Light
│  ├──────────────────────────┤   │
│  │ Reduced Motion           │   │
│  │ System           [toggle]│   │
│  └──────────────────────────┘   │
│                                 │
│  ABOUT                          │
│  ┌──────────────────────────┐   │
│  │ Version                  │   │
│  │ 0.1.0 (42)              │   │
│  ├──────────────────────────┤   │
│  │ Diagnostics        ›    │   │  ← Connection logs, debug info
│  └──────────────────────────┘   │
│                                 │
└─────────────────────────────────┘
```

**Key design decisions:**

1. **Grouped list pattern** — iOS Settings-style grouped rows with section labels. Each group is a single card with internal dividers (`Surface-0` background, `Border` dividers).

2. **Gateway config is a sub-screen** — Tapping "Target URL" navigates to `settings/gateway.tsx` with full-screen URL input, test connection button, and clear/reset.

3. **Identity section** — New section showing what device is connected and which remote machine is on the other end. This gives the user spatial awareness of "where am I connected to?"

4. **Appearance control** — Theme picker (System/Dark/Light) and a toggle for reduced motion override.

5. **Diagnostics** — Sub-screen showing connection logs, WebSocket status history, last error details. Useful for debugging without leaving the app.

### 3.6 Gateway Configuration Sub-screen (`settings/gateway.tsx`)

```
┌─────────────────────────────────┐
│ ←   Gateway                     │
├─────────────────────────────────┤
│                                 │
│  Enter the WebSocket URL of     │
│  your Homie gateway.            │
│                                 │
│  ┌──────────────────────────┐   │
│  │ ws://                    │   │  ← Text input with monospace font
│  └──────────────────────────┘   │
│                                 │
│  Hint: ws://10.0.1.5:8080      │  ← Auto-detected hint if available
│                                 │
│  ┌── Test Connection ───────┐   │  ← Secondary action
│  └──────────────────────────┘   │
│                                 │
│  ┌══ Save Target ═══════════┐   │  ← Primary action (accent fill)
│  └══════════════════════════┘   │
│                                 │
│  ┌── Clear Target ──────────┐   │  ← Danger action (only if has target)
│  └──────────────────────────┘   │
│                                 │
└─────────────────────────────────┘
```

### 3.7 Drawer Panel — Left Panel Content

```
PHONE (DRAWER OPEN)
┌──────────────────┬──────────────┐
│                  │              │
│  ◇ Homie         │  dimmed      │
│  deimos · ●      │  content     │
│                  │              │
│  ┌──────────┐   │              │
│  │ ◈ Chat   │   │              │  ← Active section: accent left bar
│  │ ◆ Term   │   │              │  ← Inactive: no bar
│  │ ◆ Settngs│   │              │
│  └──────────┘   │              │
│                  │              │
│  ── threads ──── │              │  ← Section label divider
│                  │              │
│  ┌ + New Chat ─┐ │              │  ← Compact action button
│  └─────────────┘ │              │
│                  │              │
│  ┌─────────────┐ │              │
│  │ Debug proxy │ │              │  ← Thread row
│  │ 2m ago      │ │              │
│  ├─────────────┤ │              │
│  │ Deploy fix ●│ │              │  ← Running indicator (green dot)
│  │ 14m ago     │ │              │
│  ├─────────────┤ │              │
│  │ Auth setup  │ │              │
│  │ 1h ago      │ │              │
│  └─────────────┘ │              │
│                  │              │
└──────────────────┴──────────────┘
```

**Changes from current:**

1. **Identity badge at top** — Shows app name + connected machine name + connection dot. Replaces generic "Homie" text. This tells the user: "You are connected to deimos and it's online."

2. **Nav items use active indicator bar** — Instead of border+fill, active section gets a 3px accent-colored left bar and text in `Text-Primary`. Inactive items are text-only with `Text-Secondary`. This is visually lighter and more scalable.

3. **Thread list is flush** — Threads are not cards-in-a-list; they're flat rows with subtle bottom dividers. This increases density and reduces visual noise.

4. **Thread rows are compact** — Title (14px semibold) + timestamp (12px secondary) on one line. Optional second line showing preview text (13px, secondary, 1-line truncate). Running dot inline with title. No card borders.

5. **"New Chat" is a compact row** — Styled as a subtle ghost button with `+` icon, not a full-width accent-colored button. The accent button was too heavy for a drawer action.

6. **Search/filter** — At top of thread list: a compact search field (appears on focus, collapses to icon when empty). Filters threads by title text match.

### 3.8 Drawer — Terminals Sub-list

When "Terminals" section is active, the sub-list shows terminal sessions instead of threads:

```
│  ── sessions ─── │
│                  │
│  ┌ ↻ Refresh ──┐ │
│  └─────────────┘ │
│                  │
│  ┌─────────────┐ │
│  │ zsh@deimos  │ │
│  │ ● 4m ago    │ │
│  ├─────────────┤ │
│  │ bash@phobos │ │
│  │ ● 2h ago    │ │
│  └─────────────┘ │
```

### 3.9 Drawer — Settings

When "Settings" is active, no sub-list. The drawer shows only the nav items and a minimal info block:

```
│  ── info ─────── │
│                  │
│  Connected to    │
│  ws://10.0.1.5   │
│  ● Online        │
```

---

## 4. Component Inventory

### 4.1 New Components

| Component | Purpose | File |
|-----------|---------|------|
| `AppShell` | Root layout: drawer state, gesture handlers, left panel + content frame | `components/shell/AppShell.tsx` |
| `DrawerPanel` | Left panel container: identity badge, nav, sublists | `components/shell/DrawerPanel.tsx` |
| `NavRail` | Vertical nav items (Chat/Terminals/Settings) with active indicator | `components/shell/NavRail.tsx` |
| `IdentityBadge` | Machine name + connection dot at top of drawer | `components/shell/IdentityBadge.tsx` |
| `ThreadRow` | Compact thread row for drawer (replaces card-in-card pattern) | `components/chat/ThreadRow.tsx` |
| `ThreadSearchBar` | Collapsible search/filter for thread list | `components/chat/ThreadSearchBar.tsx` |
| `StreamingIndicator` | Staggered dot pulsing + status text during AI response | `components/chat/StreamingIndicator.tsx` |
| `ApprovalCard` | Distinct amber-bordered card for approval requests | `components/chat/ApprovalCard.tsx` |
| `JumpToLatest` | Floating mini-FAB for scroll-to-bottom in timeline | `components/chat/JumpToLatest.tsx` |
| `SessionRow` | Compact terminal session row for drawer | `components/shell/SessionRow.tsx` |
| `SettingsGroup` | Grouped list rows with section label (iOS Settings pattern) | `components/settings/SettingsGroup.tsx` |
| `SettingsRow` | Individual row within a SettingsGroup | `components/settings/SettingsRow.tsx` |
| `HeaderBar` | Compact header: menu button + title + status pill | `components/shell/HeaderBar.tsx` |
| `GhostButton` | Subtle, borderless button for secondary actions | `components/ui/GhostButton.tsx` |
| `SectionLabel` | Uppercase caption text for section dividers | `components/ui/SectionLabel.tsx` |
| `EmptyState` | Centered empty state with geometric icon + message + CTA | `components/ui/EmptyState.tsx` |
| `ConnectionBanner` | Full-width reconnection banner (NEW) | `components/ui/ConnectionBanner.tsx` |

### 4.2 Updated Components

| Component | Changes |
|-----------|---------|
| `StatusPill` | Add `danger` tone, add pulsing dot indicator for "connecting" state, reduce font to 11px |
| `ChatTimeline` | Extract approval rendering to `ApprovalCard`, add `StreamingIndicator` at bottom, add `JumpToLatest` overlay |
| `ChatComposer` | Remove outer padding (parent handles it), add disabled visual state with skeleton pulse, refine pill styling |
| `ThreadList` | Replace with flat `ThreadRow` list using `FlatList` for virtualization, add `ThreadSearchBar` |
| `PrimarySectionMenu` | Replace with `NavRail` — vertical items with left accent bar instead of bordered buttons |
| `TerminalSessionList` | Replace with flat `SessionRow` list, group by status |
| `ScreenSurface` | Simplify — remove animated fade-in (AppShell handles transitions) |
| `ThreadActionSheet` | Update styling to match new bottom sheet aesthetic, use `Surface-3` background |
| `ModelPickerSheet` | Update styling — taller rows, more touch target, clearer active state |
| `EffortPickerSheet` | Same styling update as ModelPickerSheet |

### 4.3 Removed Components

| Component | Reason |
|-----------|--------|
| `PrimarySectionMenu` | Replaced by `NavRail` |
| `GatewayTargetForm` (inline in index) | Moved to dedicated `settings/gateway.tsx` screen |
| `SettingRow` (defined in index.tsx) | Replaced by `SettingsRow` component |

### 4.4 Component Detail: ApprovalCard

The approval card is one of the most critical UI elements — it requests permission for potentially destructive operations. It must be impossible to miss and impossible to accidentally act on.

```
┌─ ┃ ─────────────────────────────┐
│  ┃  ⚠  APPROVAL REQUIRED       │  ← Warning icon + label
│  ┃                              │
│  ┃  ┌────────────────────────┐  │
│  ┃  │ $ rm -rf /tmp/build   │  │  ← Mono, Surface-3 well
│  ┃  └────────────────────────┘  │
│  ┃                              │
│  ┃  ┌─────────┐  ┌──────────┐  │
│  ┃  │ Approve │  │  Deny    │  │  ← Button row
│  ┃  └─────────┘  └──────────┘  │
│  ┃  ┌─────────────────────────┐ │
│  ┃  │  Approve for session    │ │  ← Full-width secondary
│  ┃  └─────────────────────────┘ │
└──┃──────────────────────────────┘
   ┃ ← 4px amber left border
```

- Background: `Warning-Dim` (12% opacity amber)
- Left border: 4px solid `Warning`
- "Approve" button: `Success` fill, white text
- "Deny" button: `Surface-2` fill, `Text-Primary` text
- "Approve for session": Ghost button, `Text-Secondary`
- Minimum vertical spacing between buttons: 8px
- Buttons are 48px tall for confident tapping
- After decision: card collapses to single line "✓ Approved" or "✗ Denied" with 200ms transition

### 4.5 Component Detail: StreamingIndicator

```
● ● ●  Thinking…
```

- Three dots, 6px diameter each, `Accent` color
- Staggered opacity animation: each dot fades 0.3→1.0→0.3 with 200ms offset
- Total cycle: 900ms
- Text label changes based on content type:
  - `Thinking…` — reasoning stream
  - `Running…` — tool/command execution
  - `Typing…` — normal text generation
  - `Planning…` — plan items being generated
- When `reducedMotion` is true: static dots at full opacity, no animation
- Height: 32px, left-aligned with message content

### 4.6 Component Detail: HeaderBar

```
┌─────────────────────────────────┐
│ [≡]   Chat               [●··] │
└─────────────────────────────────┘
  │      │                   │
  │      Title (17px SB)     StatusPill
  Menu button (22px icon)
```

- Height: 48px (tighter than current)
- Menu button: 44×44 tap target, icon only (no "Menu" text label), ghost style
- Title: `Title` weight, uses section name directly
- No "Gateway" eyebrow — saves 20px vertical space
- StatusPill: right-aligned, compact
- On tablet: menu button hidden, left padding adjusts

---

## 5. Interaction Patterns

### 5.1 Gesture Map

| Gesture | Location | Action |
|---------|----------|--------|
| Edge swipe right (24px) | Screen left edge | Open drawer |
| Swipe left on panel | Drawer panel | Close drawer |
| Tap backdrop | Drawer backdrop | Close drawer |
| Long press | Thread row | Open thread action sheet |
| Long press | Message bubble | Open message context menu (copy/share) |
| Pull down | Thread list, Session list | Refresh data |
| Swipe left on thread row | Drawer thread list | Reveal archive action (destructive swipe) |
| Tap code block | Chat timeline | Copy to clipboard (with haptic) |
| Tap | Jump-to-latest FAB | Scroll to bottom |

### 5.2 Haptic Feedback Map

| Action | Haptic Type | Platform |
|--------|-------------|----------|
| Tap nav item | `selectionAsync()` | iOS/Android |
| Tap thread row | `selectionAsync()` | iOS/Android |
| Open drawer (swipe complete) | `impactAsync(Light)` | iOS/Android |
| Send message | `impactAsync(Medium)` | iOS/Android |
| Approval: Approve | `notificationAsync(Success)` | iOS/Android |
| Approval: Deny | `notificationAsync(Warning)` | iOS/Android |
| Long press menu open | `impactAsync(Heavy)` | iOS/Android |
| Copy to clipboard | `impactAsync(Light)` | iOS/Android |
| Error / connection lost | `notificationAsync(Error)` | iOS/Android |
| Pull-to-refresh trigger | `impactAsync(Light)` | iOS/Android |
| Destructive swipe reveal | `impactAsync(Medium)` | iOS/Android |

**Key change:** The current app uses `Light` for everything. The redesign matches haptic intensity to action severity — light for navigation, medium for actions, heavy for destructive reveals, notification types for outcomes.

### 5.3 Transition Specifications

| Transition | Duration | Easing | Property |
|------------|----------|--------|----------|
| Drawer open/close | 250ms | `Easing.out(Easing.cubic)` | translateX + backdrop opacity |
| Section content swap | 150ms | `Easing.out(Easing.quad)` | opacity (crossfade) |
| New thread row appear | 200ms | `Easing.out(Easing.cubic)` | opacity + translateY(8→0) |
| Approval card collapse | 200ms | `Easing.out(Easing.cubic)` | height + opacity |
| Status pill tone change | 300ms | `linear` | backgroundColor |
| Jump-to-latest appear | 180ms | `Easing.out(Easing.back)` | scale(0→1) + opacity |
| Streaming dots | 900ms cycle | `Easing.inOut(Easing.sine)` | opacity per dot |
| Bottom sheet enter | 300ms | `Easing.out(Easing.cubic)` | translateY + backdrop |
| Bottom sheet exit | 200ms | `Easing.in(Easing.cubic)` | translateY + backdrop |
| Connection banner slide | 250ms | `Easing.out(Easing.cubic)` | translateY + height |

**Reduced motion override:**
- All `transform` animations → instant (0ms)
- All `opacity` animations → 80ms max
- Streaming dots → static at full opacity
- No `scale` or `translateY` transitions

### 5.4 Keyboard Handling

- `ChatComposer` uses `KeyboardStickyView` (current approach, keep it)
- When keyboard opens: composer sticks above keyboard, timeline shrinks
- When switching sections: dismiss keyboard before section transition
- Settings text inputs: keyboard-aware scroll view
- Gateway URL input: `keyboardType="url"`, auto-capitalization off

---

## 6. Empty States & Onboarding

### 6.1 First-Run Flow

When the app launches with no saved gateway target, the user enters the onboarding flow.

```
SCREEN 1: Welcome
┌─────────────────────────────────┐
│                                 │
│                                 │
│           ◇                     │
│         Homie                   │  ← App icon + name
│                                 │
│    Connect to your machines     │  ← Tagline
│    from anywhere.               │
│                                 │
│                                 │
│   ┌─────────────────────────┐   │
│   │ Enter gateway URL       │   │  ← URL input, monospace
│   └─────────────────────────┘   │
│                                 │
│   Detected: ws://10.0.1.5:8080 │  ← Auto-detected hint
│                                 │
│   ┌══ Connect ══════════════┐   │  ← Primary CTA
│   └═════════════════════════┘   │
│                                 │
│   What is a gateway? ›         │  ← Help link (opens brief explainer)
│                                 │
└─────────────────────────────────┘

SCREEN 2: Connecting (animated)
┌─────────────────────────────────┐
│                                 │
│                                 │
│           ◇                     │
│                                 │
│   Connecting to gateway…        │  ← Spinner + status text
│   ws://10.0.1.5:8080           │
│                                 │
│   ┌ ● Resolving host     ✓ ┐   │  ← Step checklist
│   │ ● WebSocket handshake ● │   │
│   │ ○ Authentication      ○ │   │
│   │ ○ Ready               ○ │   │
│   └─────────────────────────┘   │
│                                 │
│   ┌── Cancel ───────────────┐   │
│   └─────────────────────────┘   │
│                                 │
└─────────────────────────────────┘

SCREEN 3: Connected (auto-dismisses after 1.5s)
┌─────────────────────────────────┐
│                                 │
│                                 │
│           ✓                     │  ← Green checkmark
│                                 │
│   Connected to deimos           │
│   Arch Linux · 4 terminals      │
│                                 │
│                                 │
└─────────────────────────────────┘
→ Auto-transitions to Chat screen
```

**Key design decisions:**

1. **Single-screen input** — URL input is the only thing on the welcome screen. No multi-step wizard — users who need Homie know what a gateway URL is.

2. **Connection progress is visual** — The step checklist gives confidence that something is happening and helps diagnose where failures occur.

3. **Auto-detected hint** — If the backend provides mDNS/Bonjour discovery, show the detected gateway as a tappable suggestion.

4. **Auto-advance** — On successful connection, brief success screen (1.5s) then auto-navigate to Chat. No unnecessary "Continue" button.

### 6.2 Empty State Designs

Each empty state follows the same pattern: **Geometric icon → Message → CTA**

**No threads (Chat section):**
```
        ◇
  No conversations yet.
  Start one to begin working
  with your gateway.

  [ + New Chat ]
```

**No terminal sessions:**
```
        ▣
  No running terminals.
  Start a session from your
  desktop or web client first.

  [ ↻ Refresh ]
```

**Disconnected state (any screen):**
```
┌─────────────────────────────────┐
│  ⚡ Connection lost             │  ← Full-width amber banner
│  Reconnecting to deimos…       │
│  ┌── Retry Now ───┐            │
│  └────────────────┘            │
└─────────────────────────────────┘
```

**Error state:**
```
        △
  Something went wrong.
  {error message}

  [ Retry ]  [ Settings ]
```

### 6.3 Icon Language for Empty States

All empty state icons are geometric, not illustrative. They're constructed from basic shapes and rendered in `Text-Tertiary` at 48px:

- Chat: `◇` (diamond) — represents conversation nodes  
- Terminals: `▣` (dotted square) — represents a terminal window  
- Error: `△` (triangle) — universal warning  
- Success: `✓` (checkmark) — completion
- Search no results: `◎` (bullseye) — target not found

---

## 7. Connection & Status System

### 7.1 Status States

| State | StatusPill | Color | Dot | Banner |
|-------|-----------|-------|-----|--------|
| Connected | "Online" | Success | ● solid green | None |
| Connecting | "Connecting" | Warning | ● pulsing amber | None |
| Reconnecting | "Reconnecting" | Warning | ● pulsing amber | Amber banner: "Reconnecting to {machine}…" |
| Disconnected | "Offline" | Danger | ● solid red | Red banner: "Connection lost. Tap to retry." |
| No target | "Setup" | Warning | None | None (show onboarding) |

### 7.2 StatusPill Redesign

```
Current:   ┌─────────────┐
           │  CONNECTED  │     ← Solid fill, hard to read
           └─────────────┘

Proposed:  ┌─────────────────┐
           │ ●  Online       │  ← Dot + label, subtle background
           └─────────────────┘
```

- Size: 28px height, auto width
- Background: status color at 12% opacity (dim variant)
- Text: status color at 100%
- Dot: 6px solid circle, status color, left of text
- For "Connecting" state: dot pulses (opacity 0.4→1.0, 800ms cycle)
- Font: `Label` 11px

### 7.3 Connection Banner

Shows below the header when connection state is warning/danger:

```
┌─────────────────────────────────────┐
│ ⚡ Reconnecting to deimos… [Retry] │
└─────────────────────────────────────┘
```

- Height: 36px
- Background: `Warning-Dim` (reconnecting) or `Danger-Dim` (disconnected)
- Icon: ⚡ bolt, colored to match state
- Text: `Caption` weight, `Text-Primary`
- "Retry" button: right-aligned, text-only, accent color
- Animates in with 250ms translateY slide-down
- Auto-dismisses when connection restores (200ms slide-up)

### 7.4 Machine Identity Display

The drawer header shows the connected machine identity:

```
┌──────────────────┐
│  ◇ Homie         │  ← App wordmark
│  deimos · ●      │  ← Machine name + status dot
│  Arch Linux      │  ← OS name (secondary)
└──────────────────┘
```

When no connection:
```
┌──────────────────┐
│  ◇ Homie         │
│  Not connected   │  ← Gray text
└──────────────────┘
```

This replaces the current generic "Homie" header and gives users constant awareness of _which machine_ they're controlling — critical for users with multiple machines.

---

## 8. Responsive Strategy

### 8.1 Breakpoints

```
Compact     < 600dp     Phone portrait (default)
Medium      600-900dp   Phone landscape, small tablet
Expanded    > 900dp     Tablet portrait/landscape
```

### 8.2 Layout Adaptations

| Element | Compact | Medium | Expanded |
|---------|---------|--------|----------|
| Drawer | Hidden, swipe/button | Persistent, 280dp | Persistent, 320dp |
| Drawer backdrop | Yes (45% black) | No | No |
| Content padding | 16dp | 20dp | 24dp |
| Chat timeline max-width | 100% | 100% | 680dp centered |
| Composer max-width | 100% | 100% | 680dp centered |
| Header menu button | Visible | Hidden | Hidden |
| Settings layout | Full width | Full width | 2-column (nav + detail) |
| Terminal detail | Full screen | Split (list + detail) | Split (list + detail) |
| Thread search | Above thread list | Above thread list | Always visible in drawer |
| Bottom sheets | Full width | Max 420dp centered | Max 420dp centered |

### 8.3 Phone Layout

The primary layout. Everything is full-bleed, single-column:

```
┌─────────────────────────────────┐
│          HeaderBar              │
├─────────────────────────────────┤
│        (ConnectionBanner)       │  ← Conditional
├─────────────────────────────────┤
│                                 │
│         Route Content           │
│         (full width)            │
│                                 │
├─────────────────────────────────┤
│         Composer                │  ← Chat only
└─────────────────────────────────┘
```

### 8.4 Tablet Layout

Split view with persistent left panel:

```
┌──────────────┬──────────────────────────────┐
│              │         HeaderBar             │
│  DrawerPanel │──────────────────────────────│
│  (300dp)     │    (ConnectionBanner)         │
│              │──────────────────────────────│
│  NavRail     │                              │
│  ──────────  │      Route Content            │
│  ThreadList  │      (max-width: 680dp)       │
│  or Sessions │      (centered)               │
│              │                              │
│              │──────────────────────────────│
│              │         Composer              │
└──────────────┴──────────────────────────────┘
```

- Left panel: 300dp fixed, `Surface-0` background, 1px right border
- Content area: centered with max-width 680dp for readability
- HeaderBar: no menu button, title still shows

### 8.5 Tablet: Terminal Split View

On tablets, the terminal section shows a master-detail split:

```
┌──────────────┬────────────┬─────────────────┐
│              │  Sessions  │  Terminal Detail │
│  DrawerPanel │  (list)    │  (viewport)     │
│              │            │                  │
│              │  zsh@dei●  │  $ ls -la       │
│              │  bash@pho  │  total 42       │
│              │            │  drwxr-xr-x ... │
│              │            │                  │
└──────────────┴────────────┴─────────────────┘
```

This three-pane layout only appears on expanded (>900dp) screens.

### 8.6 Safe Area Handling

- Top: `insets.top` + 8dp padding → HeaderBar
- Bottom: `insets.bottom` piped into ChatComposer via `KeyboardStickyView`
- Left/Right: on iOS with notch in landscape, respect `insets.left` and `insets.right` for drawer and content
- Drawer: full height, accounts for top safe area internally

### 8.7 Orientation Handling

- Phone portrait: default layout
- Phone landscape: same as portrait, but composer anchors to bottom, timeline gets more horizontal space
- Tablet portrait: persistent drawer + content
- Tablet landscape: persistent drawer + wider content, terminal split available

---

## Appendix A: Token Migration Checklist

Changes to `theme/tokens.ts`:

```
NEW TOKENS:
- Surface-2, Surface-3 (additional depth layers)
- Text-Tertiary (disabled/placeholder)
- Accent-Dim, Success-Dim, Warning-Dim, Danger-Dim
- Border-Active

MODIFIED TOKENS:
- background: #0D131B → #0B1018 (darker for more OLED savings)
- surface: #121C27 → #111921 (Surface-0)
- surfaceAlt: #1A2735 → #18222D (Surface-1)

NEW SPACING:
- 2 (micro)
- 6 (sm, tighter than current 8)

NEW RADIUS:
- 4 (micro, for inline badges)
- 16 (lg, for bottom sheets)

NEW TYPOGRAPHY:
- heading: 22/28, SB, -0.3
- bodyRegular: 15/22, Regular (current body is Medium)
- caption: 13/18, Medium, 0.1
```

## Appendix B: Component Dependency Map

```
AppShell
├── DrawerPanel
│   ├── IdentityBadge
│   ├── NavRail
│   ├── SectionLabel
│   ├── ThreadSearchBar
│   ├── ThreadRow (via FlatList)
│   ├── SessionRow (via FlatList)
│   └── GhostButton (New Chat, Refresh)
├── HeaderBar
│   ├── GhostButton (menu)
│   └── StatusPill
├── ConnectionBanner
└── <Slot /> (route content)
    ├── ChatScreen
    │   ├── ChatTimeline
    │   │   ├── ChatTurnGroup
    │   │   │   ├── ChatMarkdown
    │   │   │   ├── ChatTurnActivity
    │   │   │   └── ApprovalCard
    │   │   ├── StreamingIndicator
    │   │   └── JumpToLatest
    │   └── ChatComposer
    │       ├── ModelPickerSheet
    │       └── EffortPickerSheet
    ├── TerminalListScreen
    │   └── SessionRow (via SectionList)
    ├── TerminalDetailScreen
    ├── SettingsScreen
    │   ├── SettingsGroup
    │   └── SettingsRow
    └── OnboardingScreen
```

## Appendix C: File Count Estimate

| Category | Current Files | After Redesign | Delta |
|----------|--------------|----------------|-------|
| Routes (app/) | 5 | 10 | +5 |
| Shell components | 3 | 7 | +4 |
| Chat components | 7 | 11 | +4 |
| UI components | 2 | 7 | +5 |
| Settings components | 0 | 3 | +3 |
| Theme files | 2 | 2 | 0 |
| Hooks | 5 | 6 | +1 |
| **Total** | **24** | **46** | **+22** |

The increase is intentional — smaller, focused files replace the 805-line monolith. Average file size target: 80-150 lines.

## Appendix D: Implementation Priority

Phase 1: Foundation (blocks everything else)
1. Token migration (new palette, spacing, radius, typography)
2. AppShell + DrawerPanel + NavRail + HeaderBar
3. Route restructuring (delete stubs, create proper routes)
4. IdentityBadge + StatusPill redesign

Phase 2: Chat Excellence
5. ApprovalCard extraction and redesign
6. StreamingIndicator
7. ThreadRow (flat drawer row)
8. ThreadSearchBar
9. JumpToLatest
10. ConnectionBanner

Phase 3: Terminals & Settings
11. SettingsGroup + SettingsRow
12. Settings screen (grouped list)
13. Gateway configuration sub-screen
14. Terminal list screen (with grouping)
15. Terminal detail screen (read-only viewport)

Phase 4: Onboarding & Polish
16. Onboarding flow (welcome → connecting → connected)
17. Empty state components
18. Haptic differentiation pass
19. Tablet layout refinements
20. Reduced motion audit

---

*Design document authored for Homie Mobile v2 UX redesign.*
*Last updated: 2026-02-10*
