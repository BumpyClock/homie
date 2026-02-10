# Homie Mobile UX Doc (Expo/RN, Chat-First)

## 1) Overview
- Goal: mobile chat primary surface; remote terminal secondary entry.
- Users: on-the-go operators; fast ask/approve/fix loops from phone/tablet.
- Principle: parity with current web chat semantics (`chat.*`, approvals, tool events), mobile-native ergonomics.
- Success bar: create/open thread <2 taps; send message <1.5s perceived feedback; approvals impossible to miss.

## 2) Constraints
- Stack: Expo + React Native; iOS/Android first; tablet support required.
- Backend contract fixed: no protocol changes for UX phase.
- Network volatility: frequent bg/fg, spotty mobile data, captive networks.
- Safe-area variance: notches, home indicator, dynamic island.
- Input complexity: long prompts, mentions, file tags, keyboard overlap.
- Performance target: 60fps scroll on mid-tier devices; first usable paint <1.2s warm start.

## 3) IA (Information Architecture)
- Top-level tabs (3): `Chat`, `Terminal`, `Settings`.
- Default route: `Chat`.
- Chat IA:
  - Thread List
  - Thread Detail
  - Thread Info (settings/tools/account)
  - Approval Queue (sheet + badge entry)
- Terminal IA:
  - Placeholder card + session jump entry (phase defer explicit)
- Settings IA:
  - Gateway target, account status, appearance, diagnostics.

Primary nav flows:
1. Open app -> Thread List -> Thread Detail -> Composer send.
2. Approval event -> global badge -> Approval sheet -> approve/deny -> return same scroll position.
3. Disconnect -> inline reconnect CTA -> retry/backoff -> restored thread context.

## 4) Design System Strategy
- Token source: shared semantic tokens with web; mobile-specific scale layer.
- Use semantic names, not raw values (`surface/base`, `text/muted`, `status/warn`).
- 4dp spacing grid; touch targets min 44x44dp.
- Radius system: 8 / 12 / 16 / 20dp (chips/cards/sheets/hero).
- Elevation tiers: 0 (flat), 1 (cards), 2 (floating composer), 3 (modal/sheet).
- Typography scale (system font for platform fidelity):
  - Display 28/34 semibold
  - Title 22/28 semibold
  - Body 16/22 regular
  - Body-sm 14/20 regular
  - Mono 13/18 medium (tool logs, inline code)
- Component ownership:
  - Shared logic/state hooks from protocol client.
  - RN presentation components isolated in `mobile/ui/chat/*`.

## 5) Layout + Responsive Behavior

Breakpoints:
- Phone compact: <=599dp width.
- Tablet/large: >=600dp width.

Phone layout:
- Thread list and detail as stacked routes.
- Header: 56dp height.
- Composer docked above keyboard; min 52dp, max 160dp before internal scroll.
- Message gutter: 12dp horizontal; 8dp vertical rhythm.
- Floating reconnect banner under header: 40dp height.

Tablet layout:
- Split view default in landscape and >=700dp portrait width.
- Left rail (thread list): 320dp fixed.
- Right pane (thread detail): fluid, max readable width 840dp; center when extra space.
- Composer width capped to 760dp, centered in detail pane.
- Approval queue opens as side sheet (420dp) instead of bottom sheet.

Orientation rules:
- Preserve scroll anchor + draft text on rotate.
- No full rerender; only layout recalc and terminal placeholder resize.

## 6) ASCII Layout

Phone - Thread List

+--------------------------------+
| Safe area                      |
| Chat                 [Search]  |
+--------------------------------+
| [Conn: Online]                 |
| Thread row                     |
| Thread row (badge 2)           |
| Thread row                     |
| ...                            |
+--------------------------------+
| Tabs: Chat | Terminal | Settings|
+--------------------------------+

Phone - Thread Detail

+--------------------------------+
| <- Threads   Thread name   ... |
+--------------------------------+
| day divider                     |
| user bubble                     |
| assistant card + tool chips     |
| approval card (sticky when new) |
| ...                             |
+--------------------------------+
| [ + ] composer text... [Send]   |
| keyline + safe-area inset       |
+--------------------------------+

Tablet - Split View

+----------------+-------------------------------------------+
| Chat           | Thread name                    status ... |
| search         +-------------------------------------------+
| thread list    | messages timeline                           |
| thread list    | messages timeline                           |
| ...            | ...                                        |
+----------------+-------------------------------------------+
| tabs (bottom)  | composer centered                           |
+----------------+-------------------------------------------+

## 7) Component Inventory
- App shell: safe-area scaffold, tab bar, global toasts, global connection badge.
- Thread list: search field, filter chips (`All`, `Running`, `Needs approval`), thread rows, unread badge.
- Thread row: title, last message preview, relative time, status dot, approval count.
- Thread detail header: back/title, model pill, connection state, overflow actions.
- Message primitives: user bubble, assistant card, tool-step row, reasoning collapse row, error row.
- Approval card: risk icon, action summary, affected paths/command, `Approve` + `Deny` + `View details`.
- Composer: multiline input, mention/file token chips, attachment entry, send/stop button, keyboard accessory.
- Inline banners: reconnecting, offline queued, sync complete.
- Sheets/modals: thread info, approval queue, destructive confirm, model/settings picker.
- Terminal placeholder: explainer card + CTA to tracked roadmap issue.

## 8) Interaction + State Matrix

| Area | Idle | Loading | Streaming | Needs approval | Error | Offline |
|---|---|---|---|---|---|---|
| Thread list | rows visible | skeleton rows (6) | n/a | badge increment live | inline retry row | cached rows + stale badge |
| Thread detail | timeline static | shimmer blocks, keep header | token/tool append at 1 frame batches | sticky approval card + haptic light | error card + resend | queued message chips |
| Composer | send enabled | disabled until thread ready | morph send->stop | input disabled only when approval modal forced | keep draft + error text | send becomes queue action |
| Approval sheet | hidden | n/a | can open during stream | top priority focus trap | deny fallback always available | local decision disabled until online |
| Connection banner | hidden | connecting pulse | hidden | hidden | visible warning | persistent offline state |

State rules:
- Never lose draft on route change, bg/fg, rotate, reconnect.
- Only one blocking modal at once; approval sheet preempts non-critical sheets.
- Streaming autoscroll only if user within 120dp of bottom.

## 9) Visual System
- Tone: utilitarian + calm; dark-first for terminal-adjacent context, full light theme parity.
- Color roles:
  - Primary action: blue 600 (light) / blue 400 (dark).
  - Success: green 600/400.
  - Warning/approval: amber 600/400.
  - Danger: red 600/400.
  - Neutral surfaces: 1/2/3 layered contrast (min 1.2 delta each layer).
- Message contrast:
  - User bubble stronger fill; assistant card elevated neutral.
  - Tool steps low-emphasis mono text with icon channel color.
- Icon size: 18dp default, 22dp for primary actions.
- Motion guidance:
  - Screen push/pop: 240ms, standard decel.
  - List item enter stagger: 20ms step, max 6 visible items.
  - Stream token fade/slide: 120ms ease-out, batched.
  - Approval card appear: 180ms scale+fade; subtle haptic on arrival.
  - Reduce motion: disable slide/scale; keep opacity only <=100ms.

## 10) Accessibility
- WCAG 2.2 AA contrast targets for text + controls.
- Dynamic type support up to 200%; composer and cards reflow, no truncation on actions.
- Screen reader:
  - Clear labels for thread rows, status, unread count, approval urgency.
  - Streaming updates announced in throttled chunks (not every token).
  - Approval actions exposed with consequence hint.
- Focus order deterministic: header -> timeline -> composer -> tabs.
- Touch:
  - Min hit area 44x44dp.
  - 8dp minimum spacing between adjacent destructive/confirm buttons.
- Haptics: light for send success; warning for approval; error for failed action.

## 11) Content Notes
- Voice: concise operator language; no marketing phrasing.
- Thread titles: generated from first user intent, editable.
- Status copy examples:
  - Online
  - Reconnecting...
  - Offline - messages queue locally
  - Approval required
  - Action failed - retry
- Approval copy:
  - Lead with action + scope (`Run command in /repo`, `Edit 3 files`).
  - Show risk hints plain language.
- Empty states:
  - Threads: `No chats yet. Start a thread to run tasks remotely.`
  - Terminal tab: `Terminal renderer planned. Use chat tools for now.`
- Time format: relative in lists; absolute timestamp in detail on long-press.

## 12) Handoff Checklist (Design -> Build)
- Final token map (light/dark + semantic roles).
- Component spec sheets with min/max sizes and state variants.
- Motion spec table with durations/easing and reduce-motion fallbacks.
- Accessibility annotation pass (labels, traits, announce behavior).
- Device QA matrix: iPhone SE, iPhone Pro Max, Pixel A-series, iPad 11in, Android tablet 10in.
