# Homie Mobile Left-Edge Navigation UX Design Doc

## Overview
- Goals
  - Replace bottom tab bar with left-edge menu shell.
  - Keep chat as primary workflow; terminals/settings secondary.
  - Keep approvals visible and actionable inside chat detail.
- Primary users
  - On-the-go operators managing remote tasks from phone/tablet.
- Success criteria
  - Open any thread in <=2 taps.
  - Approvals visible in-thread and from menu list.
  - No bottom tab UI remains.

## Inputs and Constraints
- Platform targets
  - React Native via latest Expo, iOS + Android first, tablet-aware.
- Breakpoints
  - Phone compact: <=599dp
  - Tablet: >=600dp
- Design system or component library
  - Existing `src/apps/mobile/theme/tokens.ts` and `src/apps/mobile/theme/motion.ts`.
- Content requirements
  - Primary sections: Chat, Terminals, Settings.
  - Conversation list nested under primary sections in left menu.
- Technical constraints
  - Preserve current gateway protocol and chat hooks.
  - Respect reduced motion setting.

## Information Architecture
- App shell (new)
  - Left-edge menu (drawer on phone, persistent panel on tablet).
  - Content region (active section).
- Left menu hierarchy
  - Primary section group
    - Chat
    - Terminals
    - Settings
  - Divider
  - Section detail group (below divider)
    - Chat: conversations list (search/filter + thread rows with status/unread/approval count)
    - Terminals: running terminal sessions list
    - Settings: no sublist, opens settings content page directly
- Chat flows
  - Menu -> Chat -> Thread select -> Detail + composer
  - Incoming approval -> thread badge + inline approval card -> approve/deny
  - Thread create -> list prepend -> open detail

## Design System Strategy
- Reuse
  - Existing palette, spacing, radius, typography, motion tokens.
  - Existing components: `ThreadList`, `ChatTimeline`, `ChatComposer`, `StatusPill`.
- New components needed
  - `AppShell` (left edge frame)
  - `PrimaryNavSectionList`
  - `ConversationsPane`
  - `SectionHeaderBar`
- Token usage rules
  - No raw hex in components.
  - 44dp minimum tap targets.
  - Keep menu widths fixed to avoid layout shift.

## Layout and Responsive Behavior
- Phone
  - Menu hidden by default.
  - Open via top-left menu button or edge swipe.
  - Drawer width: 320dp max (or 86% viewport if smaller).
  - Content always full-screen behind drawer.
- Tablet
  - Menu persistent on left.
  - Primary section list + conversations in one scrollable column.
  - Fixed width: 340dp for v1.
  - TODO tracked for future user-resizable width.
- Chat detail
  - Header stays in content pane.
  - Timeline + composer remains current architecture.
- No bottom tab bar
  - Remove `Tabs` usage for runtime navigation.

## ASCII Layout
```text
Phone (Drawer Closed)
+--------------------------------+
| [menu]  Chat            [status]|
+--------------------------------+
| Thread detail timeline         |
| ...                            |
| [approval card inline]         |
+--------------------------------+
| composer                       |
+--------------------------------+

Phone (Drawer Open)
+------------------+-------------+
| Chat             | dimmed      |
| Terminals        | content     |
| Settings         |             |
|------------------|             |
| Conversations    |             |
| - Thread A       |             |
| - Thread B (2)   |             |
| - Thread C (!)   |             |
+------------------+-------------+

Tablet (Persistent Left Menu)
+----------------------+-----------------------------+
| Chat                 | Thread detail               |
| Terminals            | timeline                    |
| Settings             | ...                         |
|----------------------|                             |
| Conversations        |                             |
| - Thread A           |                             |
| - Thread B           |                             |
+----------------------+-----------------------------+
```

## Component Inventory
- `AppShell`
  - Purpose: one source of truth for section + drawer state.
  - States: drawer open/closed, reduced-motion, tablet persistent.
- `PrimaryNavSectionList`
  - Purpose: switch Chat/Terminals/Settings.
  - States: active/inactive, badge count (optional).
- `ConversationsPane`
  - Purpose: list/search/select chat threads.
  - States: loading, empty, error, active row.
- `TerminalSessionsPane`
  - Purpose: show running terminals list in left menu when Terminals is active.
  - States: loading, empty, active session.
- `ChatDetailPane`
  - Purpose: timeline + approval cards + composer.
  - States: disconnected, streaming, pending approval.

## Interaction and State Matrix
- Section switch
  - Tap section -> content swap; keep last-open chat for Chat section.
- Thread select
  - Tap thread -> set active chat; close drawer on phone.
- Thread row long-press
  - Open quick actions: rename, archive/delete confirm.
- Approval
  - Show inline card in timeline and badge in thread row.
  - Decision immediately updates row badge and card state.
- Reconnect
  - Keep drawer/section state, keep selected thread, show connection badge.
- Draft persistence
  - Preserve composer draft per thread across section switches.

## Motion Spec
- Open/close drawer
  - 180-220ms `ease-out`; translateX + backdrop opacity.
- Section/content swap
  - 140ms fade only (no large slide).
- New thread row insert
  - 120ms opacity + slight translateY.
- Approval badge pulse
  - 120ms single pulse, no loop.
- Reduced motion
  - Disable translate/scale; keep <=100ms opacity transitions.

## Visual System
- Direction
  - Precision + utility; low-noise surfaces, clear status accents.
- Surfaces
  - Menu: `surface`
  - Content: `background`
  - Active row: `surfaceAlt`
- Status colors
  - Approval badge: warning tone.
  - Connection: success/warning/danger via `StatusPill`.
- Typography
  - Keep current tokenized type scale; no font-family expansion in this phase.

## Markdown Rendering Strategy
- Goal
  - Render assistant and reasoning messages as markdown in `ChatTimeline` (headings, lists, inline code, links, code blocks).
- Recommended library
  - `react-native-marked`
- Why this path
  - Actively maintained for 2025/2026.
  - RN-native rendering path with good Expo compatibility.
  - Better fit for streaming chat content than markdown->HTML conversion stacks.
- Integration notes
  - Replace plain `<Text>` message body rendering with markdown component for assistant/reasoning/tool text bodies.
  - Keep user bubbles plain text for now.
  - Add link handling policy (open external links via platform linking).
  - Style code blocks with existing semantic tokens and monospaced typography token.
  - Add `react-native-svg` dependency required by the renderer.

## Accessibility
- Navigation controls
  - All menu toggles and icon buttons need explicit accessibility labels.
- Focus/order
  - Phone: menu button -> section list -> conversations -> content.
- Touch targets
  - 44dp minimum for section rows and thread rows.
- Announcements
  - Approval arrival: concise screen-reader announcement.
- Motion safety
  - Honor reduced motion hook globally in shell animations.

## Content Notes
- Section labels
  - Chat, Terminals, Settings.
- Empty threads copy
  - "No conversations yet. Start a chat to run tasks remotely."
- Approval copy
  - Lead with action + scope, keep to one line summary in row.

## Implementation Plan
1. Shell architecture
   - Replace `Tabs` layout (`src/apps/mobile/app/(tabs)/_layout.tsx`) with custom left-edge app shell.
   - Keep current screen modules, route through shell state first.
2. Navigation + list composition
   - Add `PrimaryNavSectionList` + `ConversationsPane`.
   - Wire section switching + thread selection + drawer behavior.
3. Chat UX integration
   - Keep existing chat detail components; connect badge and approval count into thread rows.
   - Preserve per-thread draft and selected thread state.
4. Motion + a11y pass
   - Apply tokenized motion durations/easing.
   - Add reduced-motion fallbacks and accessibility labels.
5. QA pass
   - Phone portrait/landscape + tablet split checks.
   - Verify no bottom tab remains.
6. Markdown rendering pass
   - Integrate markdown library in `ChatTimeline`.
   - Validate rendering parity for assistant/reasoning/tool messages.
