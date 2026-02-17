# Tool Detail Card System Design Doc

## Overview

- **Goals**: Provide structured, scannable previews of tool activity in the chat timeline with progressive disclosure into full tool results via bottom sheet modals
- **Primary users**: Developers and power users monitoring AI agent tool activity on mobile devices
- **Success criteria**:
  - Users can quickly scan tool activity counts without expanding
  - Last-step preview provides context without full expansion
  - Bottom sheet modal shows complete, structured tool results with accessible touch targets
  - Visual consistency with existing chat timeline cards

## Inputs and Constraints

- **Platform targets**: React Native (iOS/Android)
- **Breakpoints**: Mobile-first; no tablet/desktop variants in this spec
- **Design system**: Existing token system in `/src/apps/mobile/theme/tokens.ts`
- **Component library**: React Native + lucide-react-native icons
- **Content requirements**:
  - Browser: action, target, URL, message, excerpt
  - Web Search: query, provider, result cards (title, URL, snippet)
  - Web Fetch: title, URL, truncation status, excerpt
- **Technical constraints**:
  - 44pt minimum touch targets (per `touchTarget.min`)
  - Accessible labels required for all interactive elements
  - Use `surface1` background, `border` tokens, `radius.sm` for cards

## Design System Strategy

### Existing Tokens/Components to Reuse

From `/src/apps/mobile/theme/tokens.ts`:

| Token | Value | Usage |
|-------|-------|-------|
| `palettes.*.surface1` | `#F0F2F6` (light) / `#18222D` (dark) | Card backgrounds |
| `palettes.*.border` | `rgba(15,23,32,0.08)` / `rgba(255,255,255,0.06)` | Card borders |
| `palettes.*.text` | Primary text | Headlines, labels |
| `palettes.*.textSecondary` | Secondary text | Metadata, counts |
| `palettes.*.textTertiary` | Tertiary text | Timestamps, hints |
| `palettes.*.accent` | `#0A78E8` / `#4FA4FF` | Action pills, links |
| `palettes.*.accentDim` | `rgba(10,120,232,0.08)` / `rgba(79,164,255,0.12)` | Pill backgrounds |
| `radius.sm` | `8px` | Card corners |
| `radius.pill` | `999px` | Status/action pills |
| `spacing.xs` | `4px` | Tight gaps |
| `spacing.sm` | `6px` | Standard card padding |
| `spacing.md` | `8px` | Comfortable padding |
| `spacing.lg` | `12px` | Section padding |
| `typography.label` | 12px/600 | Card headers, pill text |
| `typography.mono` | 13px/SpaceMono | URLs, queries, excerpts |
| `typography.monoSmall` | 11px/SpaceMono | Compact data |
| `touchTarget.min` | `44px` | Minimum hit area |

### Existing Components to Reuse

| Component | Path | Usage |
|-----------|------|-------|
| `StatusPill` | `ui/StatusPill.tsx` | Provider pill, truncation badge |
| `LabeledValueRow` | `ui/LabeledValueRow.tsx` | Key-value fields in detail cards |
| `ChatTurnActivity` | `chat/ChatTurnActivity.tsx` | Pattern reference for collapsible cards |
| `ChatToolDetailCard` | `chat/ChatToolDetailCard.tsx` | Existing detail card (to extend) |
| Sheet pattern | `chat/*PickerSheet.tsx` | Modal structure reference |

### New Components Needed

| Component | Purpose |
|-----------|---------|
| `ToolCountChip` | Inline pill showing tool type + count |
| `ToolPreviewRow` | Collapsed "Last step" preview line |
| `ToolDetailSheet` | Bottom sheet modal container |
| `BrowserToolCard` | Structured browser result card |
| `WebSearchToolCard` | Web search result card with result list |
| `WebFetchToolCard` | Web fetch result card |

## Information Architecture

```
ChatTurnActivity (summary card - collapsed)
├── Header: "Agent activity" + chevron
├── Summary line: "Run command x3 . Search web x1"
├── Meta row: "4 calls" + [Running] pill (if active)
└── Preview row: "[Last] Read file config.yaml" (optional)

ChatTurnActivity (expanded)
├── Header
├── Summary
├── Tool list
│   ├── Tool row 1 -> tap opens ToolDetailSheet
│   ├── Tool row 2 -> tap opens ToolDetailSheet
│   └── ...
└── Preview row

ToolDetailSheet (modal)
├── Handle bar
├── Header: Tool type icon + label + close button
└── Content area (scrollable)
    ├── BrowserToolCard | WebSearchToolCard | WebFetchToolCard
    └── Generic tool card (fallback)
```

## Layout and Responsive Behavior

### Mobile (Primary)

All layouts are single-column, full-width minus timeline indentation.

## ASCII Layout

```text
ChatTurnActivity (Collapsed State)
+------------------------------------------------------------------+
| AGENT ACTIVITY                                           [v]     |  <- Header row
+------------------------------------------------------------------+
| Run command x3 . Search web x1 . Fetch page x2                   |  <- Summary (pills)
+------------------------------------------------------------------+
| 6 calls                                        [Running]         |  <- Meta row
+------------------------------------------------------------------+
| [Last] Read file config.yaml                                     |  <- Preview row
+------------------------------------------------------------------+

ChatTurnActivity (Expanded State)
+------------------------------------------------------------------+
| AGENT ACTIVITY                                           [^]     |
+------------------------------------------------------------------+
| Run command x3 . Search web x1 . Fetch page x2                   |
+------------------------------------------------------------------+
| 6 calls                                        [Running]         |
+------------------------------------------------------------------+
| +--------------------------------------------------------------+ |
| | 1. Run command                                          [v]  | |  <- Tool row (44pt min)
| | Completed                                                    | |
| +--------------------------------------------------------------+ |
| +--------------------------------------------------------------+ |
| | 2. Search web                                           [v]  | |  <- Tappable -> opens sheet
| | Completed                                                    | |
| +--------------------------------------------------------------+ |
| +--------------------------------------------------------------+ |
| | 3. Browser                                              [v]  | |
| | Running                                                      | |
| +--------------------------------------------------------------+ |
+------------------------------------------------------------------+

ToolDetailSheet (Bottom Sheet Modal)
+------------------------------------------------------------------+
|                         [====]                                   |  <- Handle bar
+------------------------------------------------------------------+
| [Icon] WEB SEARCH                                          [X]   |  <- Header
+------------------------------------------------------------------+
| +--------------------------------------------------------------+ |
| | QUERY                                                        | |  <- Label
| | react native bottom sheet modal                              | |  <- Value (mono)
| +--------------------------------------------------------------+ |
| | PROVIDER                                                     | |
| | [Brave Search]                                               | |  <- Pill
| +--------------------------------------------------------------+ |
| | RESULTS                                                      | |
| +--------------------------------------------------------------+ |
| | +----------------------------------------------------------+ | |
| | | React Native Bottom Sheet                                | | |  <- Result card
| | | https://github.com/gorhom/react-native-bottom-sheet     | | |
| | | A performant interactive bottom sheet with fully...     | | |
| | +----------------------------------------------------------+ | |
| | +----------------------------------------------------------+ | |
| | | React Native Modal Presentation                          | | |
| | | https://reactnative.dev/docs/modal                       | | |
| | | The Modal component is a basic way to present...         | | |
| | +----------------------------------------------------------+ | |
+------------------------------------------------------------------+

BrowserToolCard (Inside Sheet)
+------------------------------------------------------------------+
| ACTION   TARGET                                                  |
| [click]  [Submit button]                                         |  <- Paired pills
+------------------------------------------------------------------+
| URL                                                              |
| https://example.com/checkout                                     |  <- Mono, selectable
+------------------------------------------------------------------+
| MESSAGE                                                          |
| Successfully clicked the submit button and navigated to...       |
+------------------------------------------------------------------+
| EXCERPT                                                          |
| Order confirmed. Your order #12345 has been placed...            |  <- Mono, truncated
+------------------------------------------------------------------+

WebFetchToolCard (Inside Sheet)
+------------------------------------------------------------------+
| TITLE                                                            |
| API Documentation - Authentication                               |
+------------------------------------------------------------------+
| URL                                                  [Truncated] |  <- Badge if truncated
| https://docs.example.com/api/auth                                |
+------------------------------------------------------------------+
| EXCERPT                                                          |
| Authentication is handled via Bearer tokens. Include the...      |
+------------------------------------------------------------------+
```

## Component Inventory

### ToolCountChip

- **Purpose**: Inline pill showing tool type label + count
- **Variants**: Default (textSecondary), highlighted (accent when > 0)
- **States**: Normal
- **Composition**: `View` + `Text` with pill styling

```
Props:
- label: string          // "Run command"
- count: number          // 3
- tone?: 'default' | 'accent'
```

### ToolPreviewRow

- **Purpose**: Shows last executed tool step in collapsed state
- **Variants**: None
- **States**: Normal
- **Composition**: `[Last]` badge + tool label + truncated detail

```
Props:
- toolName: string       // "Read file"
- detail?: string        // "config.yaml"
```

### ToolDetailSheet

- **Purpose**: Bottom sheet modal container for structured tool results
- **Variants**: By tool type (browser, web_search, web_fetch, generic)
- **States**: Open, closed
- **Composition**: Modal + backdrop + sheet container + header + scrollable content

```
Props:
- visible: boolean
- item: ChatItem
- onClose: () => void
```

### BrowserToolCard

- **Purpose**: Structured display of browser tool results
- **Variants**: With/without excerpt, with/without message
- **States**: Normal
- **Composition**: Action/target pill row + URL field + message field + excerpt field

```
Props:
- action?: string        // "click"
- target?: string        // "Submit button"
- url?: string
- message?: string
- excerpt?: string
```

### WebSearchToolCard

- **Purpose**: Display web search query and results
- **Variants**: With/without provider, 0-N results
- **States**: Normal
- **Composition**: Query field + provider pill + result cards list

```
Props:
- query: string
- provider?: string
- results: Array<{ title: string; url: string; snippet?: string }>
```

### WebFetchToolCard

- **Purpose**: Display web fetch results
- **Variants**: With/without truncation badge
- **States**: Normal
- **Composition**: Title field + URL field + truncation badge + excerpt field

```
Props:
- title?: string
- url: string
- truncated?: boolean
- excerpt?: string
```

### SearchResultCard

- **Purpose**: Individual result card within WebSearchToolCard
- **Variants**: With/without snippet
- **States**: Normal
- **Composition**: Title + URL (mono) + snippet

```
Props:
- title: string
- url: string
- snippet?: string
```

## Interaction and State Matrix

### ChatTurnActivity Summary Card

| Element | Tap | Long Press | Swipe |
|---------|-----|------------|-------|
| Header row | Toggle expanded/collapsed | - | - |
| Tool row (expanded) | Open ToolDetailSheet | - | - |
| Preview row | Toggle expanded + scroll to last | - | - |

### ToolDetailSheet

| Element | Tap | Long Press | Swipe |
|---------|-----|------------|-------|
| Backdrop | Close sheet | - | - |
| Close button | Close sheet | - | - |
| Handle bar | - | - | Swipe down closes |
| URL field | Select text | Copy URL | - |
| Result card | - | Copy URL | - |

### States

| State | Visual Treatment |
|-------|------------------|
| Collapsed | Summary pills + meta row + preview row visible |
| Expanded | Tool list visible, chevron flipped |
| Loading | Tool row shows "Running" status |
| Error | Tool row shows "Failed" status in danger color |
| Empty results | Show "No results" in textTertiary |

### Haptic Feedback

Reuse existing motion tokens from `@/theme/motion`:
- `motion.haptics.activityToggle` - expand/collapse
- `motion.haptics.activityDetail` - open sheet

## Visual System

### Color Roles

| Role | Token | Usage |
|------|-------|-------|
| Card background | `surface1` | All card backgrounds |
| Card border | `border` | 1px card outlines |
| Primary text | `text` | Labels, titles |
| Secondary text | `textSecondary` | Metadata, counts |
| Muted text | `textTertiary` | Hints, timestamps |
| Accent | `accent` | Action pills, links |
| Accent dim | `accentDim` | Pill backgrounds |
| Success | `success` | Running status |
| Danger | `danger` | Failed status |

### Typography Scale

| Style | Size | Weight | Usage |
|-------|------|--------|-------|
| `typography.label` | 11-12px | 600 | Card headers, pill labels |
| `typography.body` | 15px | 400 | Body text, messages |
| `typography.caption` | 13px | 500 | Summaries |
| `typography.mono` | 13px | 400 | URLs, queries, excerpts |
| `typography.monoSmall` | 11px | 400 | Compact data fields |

### Spacing System

| Token | Value | Usage |
|-------|-------|-------|
| `spacing.xs` (4px) | Pill internal padding, tight gaps |
| `spacing.sm` (6px) | Card padding, standard gaps |
| `spacing.md` (8px) | Comfortable section padding |
| `spacing.lg` (12px) | Sheet header padding |

### Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `radius.sm` (8px) | All cards, tool rows, result cards |
| `radius.pill` (999px) | Action/status pills |
| `radius.lg` (16px) | Sheet top corners |

### Iconography

Use **lucide-react-native** (already in use):
- `Globe` (14px) - browser, web_fetch
- `Search` (14px) - web_search
- `X` (20px) - close button
- `ChevronDown` / `ChevronUp` (14px) - expand/collapse

## Accessibility

### Keyboard Navigation

N/A for mobile - focus on touch and screen reader support.

### Focus Order and States

For screen readers:
1. Card header (summary) - actionable, announces tool counts
2. Each tool row when expanded - announces tool name and status
3. Sheet header when open
4. Each field within sheet content

### Touch Targets

All interactive elements must meet `touchTarget.min` (44pt):
- Tool rows: `minHeight: 44`
- Close button: `hitSlop: 12` (extends 44pt hit area)
- Result cards: `minHeight: 44`

### Contrast Targets

Tokens already meet WCAG AA (4.5:1 for text):
- Light mode: `#0F1720` on `#F0F2F6` = 11.2:1
- Dark mode: `#E8EDF3` on `#18222D` = 10.8:1

### ARIA/Accessibility Labels

| Element | accessibilityRole | accessibilityLabel |
|---------|-------------------|-------------------|
| Summary header | `button` | `"Expand agent activity, {n} calls{running ? ', running' : ''}"` |
| Tool row | `button` | `"View {toolName} details"` |
| Close button | `button` | `"Close tool details"` |
| URL field | `text` | `"URL: {url}"` |
| Result card | `text` | `"{title}, {url}"` |
| Status pill | `text` | `"Status: {status}"` |

## Content Notes

### Copy Tone and Hierarchy

- **Headers**: Uppercase labels (`AGENT ACTIVITY`, `QUERY`, `URL`)
- **Tool labels**: Sentence case (`Run command`, `Search web`)
- **Status**: Sentence case (`Running`, `Completed`, `Failed`)

### Summary Line Format

Use `friendlyToolLabelFromItem()` for labels, aggregate by type:
```
"Run command x3 . Search web x1 . Fetch page x2"
```

Separator: ` . ` (interpunct with spaces for visual rhythm)

### Empty-State Copy

| State | Copy |
|-------|------|
| No results (web_search) | "No search results found" |
| No excerpt (web_fetch) | "Page content unavailable" |
| Failed tool | "Tool execution failed" |

### Error Messaging

- Tool failure shows status pill in `danger` color
- Error reason (if available) shown below status in `textTertiary`

## Implementation Notes

### File Structure (Implemented)

```
src/apps/mobile/components/chat/
├── ChatTurnActivity.tsx          # Summary pills (flexWrap), preview row, sheet integration
├── ChatToolDetailCard.tsx        # Structured cards for browser/web_search/web_fetch
├── ToolDetailSheet.tsx           # React Native Modal bottom sheet
```

Key implementation decisions:
- **No separate component files** for ToolCountChip, BrowserToolCard, etc. - integrated into parent components to reduce file count
- **React Native Modal** used instead of third-party bottom sheet library for simplicity
- **Nested scrolling** handled via `nestedScrollEnabled` prop on ScrollView inside FlatList context

### Data Flow

1. `ChatTurnActivity` receives `toolItems: ChatItem[]`
2. On web tool row tap, set `sheetItem` state and open `ToolDetailSheet`
3. On non-web tool row tap, toggle inline expansion via `openToolId` state
4. `ToolDetailSheet` renders `ChatToolDetailCard` with tool-specific field extraction
5. `ChatToolDetailCard` extracts structured results (web_search) or fields (browser/web_fetch)

### Edge Cases Handled

- **Long URLs**: Middle-truncated with protocol preserved (`https://example.com/ver…/file.html`)
- **Tool output capping**: 720 character limit with truncation badge
- **Empty payload**: Shows "No details available" state with icon
- **Pill overflow**: `flexWrap: 'wrap'` on summary pills row
- **Touch targets**: Minimum 44pt on all interactive elements
- **Safe area**: Bottom sheet respects `useSafeAreaInsets()` for notch/home indicator
- **Keyboard avoiding**: `KeyboardAvoidingView` wraps sheet content
- **Failed status**: Shows in `danger` color

### Extraction Helpers

All helpers live in `ChatToolDetailCard.tsx`:
- `extractField(raw, ...keys)` - extract string from raw object
- `extractInputField(raw, ...keys)` - extract from `raw.input` or `raw`
- `truncateOutput(text)` - cap at 720 chars with ellipsis
- `truncateUrl(url)` - middle-truncate long URLs
- `extractSearchResults(raw)` - parse results array for web_search
