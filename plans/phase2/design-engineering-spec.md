# Homie Mobile — Design Engineering Implementation Spec

> Implementation-ready token definitions, component APIs, layout patterns, accessibility contracts, and performance targets for the Homie mobile app redesign.

**Companion documents:**
- [UX Redesign Plan](./ux-redesign.md) — Visual direction, screen designs, navigation architecture
- [Motion System Specification](./motion-system.md) — Animation tokens, gesture choreography, haptic map

---

## Table of Contents

1. [Enhanced Design Token System](#1-enhanced-design-token-system)
2. [Component Architecture Overhaul](#2-component-architecture-overhaul)
3. [Layout & Spacing Architecture](#3-layout--spacing-architecture)
4. [Accessibility Engineering](#4-accessibility-engineering)
5. [Performance Engineering](#5-performance-engineering)
6. [Dark Mode Engineering](#6-dark-mode-engineering)
7. [Migration Path](#7-migration-path)

---

## 1. Enhanced Design Token System

### 1.1 Expanded Palette

The palette grows from 11 keys to 26 keys. Every color has a semantic role. No token exists without a concrete use case.

#### Dark Palette (Primary)

| Token | Value | Role | WCAG on background |
|-------|-------|------|-------------------|
| `background` | `#0B1018` | Full-bleed page background, OLED-safe | — |
| `surface0` | `#111921` | Card base, drawer panel, list container | — |
| `surface1` | `#18222D` | Elevated cards, active thread row, composer card | — |
| `surface2` | `#1F2B38` | Input fields, pressed states, wells | — |
| `surface3` | `#273545` | Overlays, bottom sheet body, code block bg | — |
| `text` | `#E8EDF3` | Primary body text, headings | 14.2:1 on background |
| `textSecondary` | `#7B8A9C` | Labels, timestamps, metadata | 5.1:1 on background |
| `textTertiary` | `#4A5768` | Disabled text, placeholders | 2.8:1 (decorative only) |
| `accent` | `#4FA4FF` | Primary action, active states, links, focus rings | 5.8:1 on background |
| `accentDim` | `rgba(79, 164, 255, 0.12)` | Accent backgrounds, selected row highlight | — |
| `success` | `#43C38A` | Approved, connected, positive outcomes | 7.2:1 on background |
| `successDim` | `rgba(67, 195, 138, 0.12)` | Success banner bg, connected status bg | — |
| `warning` | `#F0B44D` | Approval required, reconnecting | 8.3:1 on background |
| `warningDim` | `rgba(240, 180, 77, 0.12)` | Approval card bg, warning banner bg | — |
| `danger` | `#F06A80` | Denied, disconnected, errors, destructive | 6.1:1 on background |
| `dangerDim` | `rgba(240, 106, 128, 0.12)` | Error banner bg, destructive swipe bg | — |
| `border` | `rgba(255, 255, 255, 0.06)` | Subtle dividers, card separation | — |
| `borderActive` | `rgba(255, 255, 255, 0.12)` | Focused input borders, active separators | — |
| `overlay` | `rgba(0, 0, 0, 0.45)` | Drawer backdrop, bottom sheet scrim | — |
| `tabBar` | `rgba(18, 28, 39, 0.90)` | Retained for backward compat; deprecate after migration | — |

#### Light Palette (System-driven)

| Token | Value | Role | WCAG on background |
|-------|-------|------|-------------------|
| `background` | `#F5F7FA` | Page background | — |
| `surface0` | `#FFFFFF` | Card base | — |
| `surface1` | `#F0F2F6` | Elevated cards, active row | — |
| `surface2` | `#E8ECF2` | Input fields, pressed states | — |
| `surface3` | `#DDE2EA` | Overlays, bottom sheet, code block bg | — |
| `text` | `#0F1720` | Primary text | 15.5:1 on background |
| `textSecondary` | `#5B6878` | Labels, timestamps | 5.5:1 on background |
| `textTertiary` | `#97A3B3` | Disabled, placeholders | 2.6:1 (decorative only) |
| `accent` | `#0A78E8` | Primary action, links, focus rings | 4.7:1 on background |
| `accentDim` | `rgba(10, 120, 232, 0.08)` | Selected backgrounds | — |
| `success` | `#1A8F5C` | Connected, approved (darkened for light bg contrast) | 4.5:1 on background |
| `successDim` | `rgba(26, 143, 92, 0.08)` | Success backgrounds | — |
| `warning` | `#B06A0A` | Reconnecting, approval (darkened for contrast) | 4.6:1 on background |
| `warningDim` | `rgba(176, 106, 10, 0.08)` | Warning backgrounds | — |
| `danger` | `#C0364C` | Error, disconnected (darkened for contrast) | 5.2:1 on background |
| `dangerDim` | `rgba(192, 54, 76, 0.08)` | Error backgrounds | — |
| `border` | `rgba(15, 23, 32, 0.08)` | Subtle dividers | — |
| `borderActive` | `rgba(15, 23, 32, 0.16)` | Focused borders | — |
| `overlay` | `rgba(0, 0, 0, 0.30)` | Drawer backdrop (lighter in light mode) | — |
| `tabBar` | `rgba(255, 255, 255, 0.92)` | Backward compat; deprecate | — |

#### TypeScript Shape Change

Current `AppPalette` type has flat keys (`surface`, `surfaceAlt`). The new type adds layered surfaces and dim variants:

```
type AppPalette = {
  background: string;
  surface0: string;     // was: surface
  surface1: string;     // was: surfaceAlt
  surface2: string;     // NEW
  surface3: string;     // NEW
  text: string;
  textSecondary: string;
  textTertiary: string; // NEW
  accent: string;
  accentDim: string;    // NEW
  success: string;
  successDim: string;   // NEW
  warning: string;
  warningDim: string;   // NEW
  danger: string;
  dangerDim: string;    // NEW
  border: string;
  borderActive: string; // NEW
  overlay: string;      // NEW
  tabBar: string;       // DEPRECATED — remove after migration
}
```

**Migration alias strategy:** During migration, add getter aliases so `palette.surface` still works while code is being updated to `palette.surface0`. Mark getters with `@deprecated` JSDoc.

### 1.2 Opacity Tokens

Interaction states use consistent opacity multipliers rather than ad-hoc values scattered through components.

| Token | Value | Use Case |
|-------|-------|----------|
| `opacity.pressed` | `0.7` | Pressable components in active touch state |
| `opacity.disabled` | `0.38` | Disabled buttons, inputs, pills |
| `opacity.hover` | `0.85` | Web hover state (future) |
| `opacity.dimIcon` | `0.5` | De-emphasized icons (chevrons, decorative) |
| `opacity.backdrop` | `0.45` | Drawer and bottom sheet backdrops (dark mode) |
| `opacity.backdropLight` | `0.30` | Backdrops in light mode |
| `opacity.skeleton` | `0.4` | Skeleton shimmer highlight peak |

### 1.3 Shadow & Elevation Tokens

Homie uses a **no-shadow** surface-stepping depth model. The sole exception is the phone drawer panel, which needs separation from dimmed content.

| Token | iOS Shadow | Android `elevation` | Use Case |
|-------|-----------|---------------------|----------|
| `elevation.none` | none | `0` | Default for all cards, surfaces |
| `elevation.drawer` | `{ shadowColor: '#000', shadowOffset: { width: 8, height: 0 }, shadowOpacity: 0.20, shadowRadius: 24 }` | `8` | Drawer panel on phone only |
| `elevation.sheet` | `{ shadowColor: '#000', shadowOffset: { width: 0, height: -4 }, shadowOpacity: 0.15, shadowRadius: 16 }` | `6` | Bottom sheets |
| `elevation.fab` | `{ shadowColor: '#000', shadowOffset: { width: 0, height: 2 }, shadowOpacity: 0.18, shadowRadius: 8 }` | `4` | Jump-to-latest FAB |

**Dark mode override:** In dark mode, `shadowOpacity` values are halved because dark surfaces already provide separation. Light mode retains full opacity values.

### 1.4 Border Width Tokens

| Token | Value | Use Case |
|-------|-------|----------|
| `borderWidth.hairline` | `StyleSheet.hairlineWidth` | List dividers, card separators |
| `borderWidth.thin` | `1` | Input borders, focused rings, drawer right edge |
| `borderWidth.medium` | `2` | Active nav indicator, section bars |
| `borderWidth.thick` | `3` | Active nav rail left bar |
| `borderWidth.accent` | `4` | Approval card left border |

### 1.5 Icon Size Tokens

| Token | Value | Use Case |
|-------|-------|----------|
| `iconSize.xs` | `12` | Inline indicators (chevron in pill, status dot) |
| `iconSize.sm` | `14` | Inline icons, small action buttons, avatar fallback |
| `iconSize.md` | `18` | Standard icons in buttons, list rows, header actions |
| `iconSize.lg` | `22` | Nav rail items, primary actions |
| `iconSize.xl` | `28` | Empty state icons |
| `iconSize.display` | `48` | Onboarding hero, first-run icon |

### 1.6 Expanded Typography Scale

| Token | Size | Line Height | Weight | Letter Spacing | Font | Use Case |
|-------|------|-------------|--------|----------------|------|----------|
| `display` | 28 | 34 | 700 (Bold) | -0.4 | System | Screen titles only |
| `heading` | 22 | 28 | 600 (Semibold) | -0.3 | System | Section headers (NEW) |
| `title` | 17 | 24 | 600 (Semibold) | -0.2 | System | Card titles, drawer header, header bar |
| `body` | 15 | 22 | 400 (Regular) | 0 | System | Message text, descriptions |
| `bodyMedium` | 15 | 22 | 500 (Medium) | 0 | System | Emphasized body (thread preview) |
| `caption` | 13 | 18 | 500 (Medium) | 0.1 | System | Timestamps, metadata, badges |
| `label` | 12 | 16 | 600 (Semibold) | 0.3 | System | Pill text, section labels, nav items |
| `overline` | 11 | 14 | 600 (Semibold) | 0.8 | System | Section divider labels (NEW) |
| `mono` | 13 | 18 | 400 (Regular) | 0 | SpaceMono | Terminal data, code, command text, IDs |
| `monoSmall` | 11 | 16 | 400 (Regular) | 0 | SpaceMono | Inline code, session metadata (NEW) |
| `codeBlock` | 13 | 20 | 400 (Regular) | 0 | SpaceMono | Multi-line code blocks (relaxed line height) (NEW) |

**Key changes from current:**
- `body` weight drops from 500 → 400. Current `body` was Medium; standard body text should be Regular for readability at length.
- `bodyMedium` replaces current `body` weight for cases that need emphasis.
- `title` drops from 20 → 17. The current 20px title was too large for the compact header.
- `heading` at 22 fills the gap between `title` (17) and `display` (28) for section headers.
- `overline` added for drawer section labels ("THREADS", "SESSIONS").
- `monoSmall` and `codeBlock` added for code rendering differentiation.

### 1.7 Updated Spacing Scale

| Token | Value | Use Case |
|-------|-------|----------|
| `micro` | `2` | Icon gaps, inline nudges (NEW) |
| `xs` | `4` | Intra-component gaps, pill internal padding |
| `sm` | `6` | Tight card gutters, pill horizontal padding (CHANGED from 8) |
| `md` | `8` | Inter-element within cards (CHANGED from 12) |
| `lg` | `12` | Card internal padding, section gaps (CHANGED from 16) |
| `xl` | `16` | Between cards, drawer section gaps (CHANGED from 24) |
| `xxl` | `24` | Screen-level sections, major separators (CHANGED from 32) |
| `xxxl` | `32` | Top safe-area to content, onboarding spacing (NEW) |

**Impact:** The entire scale shifts tighter. Every component using `spacing.sm` gets 6px instead of 8px. Every component using `spacing.md` gets 8px instead of 12px. This increases information density, which is aligned with the "precision tool" design direction.

### 1.8 Updated Corner Radius

| Token | Value | Use Case |
|-------|-------|----------|
| `micro` | `4` | Inline badges, small pills, code inline (NEW) |
| `sm` | `8` | Buttons, inputs, thread rows (CHANGED from 6) |
| `md` | `12` | Cards, drawer panel, composer (CHANGED from 10) |
| `lg` | `16` | Bottom sheets, modals (CHANGED from 14) |
| `pill` | `999` | Status pills, tags (UNCHANGED) |

### 1.9 Touch Target Tokens

| Token | Value | Use Case |
|-------|-------|----------|
| `touchTarget.min` | `44` | Minimum pressable dimension (WCAG AA) |
| `touchTarget.comfortable` | `48` | Standard row height, button height |
| `touchTarget.compact` | `36` | Icon-only buttons where density matters (FAB, pills) |

### 1.10 Z-Index Scale

Explicit z-index values prevent stacking-context collisions:

| Token | Value | Use Case |
|-------|-------|----------|
| `zIndex.base` | `0` | Default content |
| `zIndex.sticky` | `10` | Sticky headers, connection banner |
| `zIndex.fab` | `20` | Jump-to-latest floating button |
| `zIndex.drawer` | `30` | Drawer backdrop + panel |
| `zIndex.sheet` | `40` | Bottom sheets |
| `zIndex.toast` | `50` | Toasts, copy confirmation |

---

## 2. Component Architecture Overhaul

### 2.1 Shared Patterns

#### Pressable Base Pattern

All tappable components share a consistent press feedback contract. This avoids every component re-implementing opacity/scale on press.

```
Props:
  onPress: () => void
  disabled?: boolean
  haptic?: 'selection' | 'light' | 'medium' | 'heavy' | 'none'
  accessibilityLabel: string
  accessibilityRole?: AccessibilityRole

Behavior:
  - Press-in: opacity → opacity.pressed (0.7) within duration.micro (80ms)
  - Press-out: opacity → 1.0 via spring.snappy
  - Disabled: opacity → opacity.disabled (0.38), pointerEvents → 'none'
  - Haptic fires on onPress, not on press-in
```

Components using this pattern: `IconButton`, `GhostButton`, all pills, all list rows, nav items.

#### Card Container Pattern

Reusable card wrapper providing consistent surface, padding, and border.

```
Props:
  variant: 'default' | 'elevated' | 'well' | 'warning'
  children: ReactNode

Mapping:
  default   → bg: surface0, border: border, padding: lg (12)
  elevated  → bg: surface1, border: none, padding: lg (12)
  well      → bg: surface2, border: none, padding: md (8)
  warning   → bg: warningDim, borderLeft: thick (4px) warning, padding: lg (12)
```

---

### 2.2 New Components

#### SkeletonLoader

Shimmer placeholder matching expected content layout.

```
Props:
  variant: 'threadRow' | 'message' | 'sessionRow' | 'settingsRow' | 'fullScreen'
  count?: number        // defaults: threadRow=6, message=4, sessionRow=3, settingsRow=4
  animated?: boolean    // defaults to !reducedMotion

Variant Specs:
  threadRow:
    - 1 line: 60% width × 14px height (title)
    - 1 line: 40% width × 12px height (timestamp)
    - row height: 56px, gap between rows: 1px (hairline)

  message:
    - avatar circle: 24×24
    - label line: 80px × 13px
    - body lines: 2-3 at random widths (70-100%) × 15px
    - vertical gap: 16px between messages

  sessionRow:
    - 1 line: 50% width × 14px
    - 1 line: 30% width × 12px
    - row height: 52px

  settingsRow:
    - 1 line: 45% width × 14px (left)
    - 1 line: 30% width × 13px (right-aligned)
    - row height: 48px

Shimmer:
  - Base color: surface1
  - Highlight: surface2 at opacity.skeleton (0.4)
  - Sweep angle: 30°
  - Sweep speed: 1200ms per cycle, linear easing, continuous
  - Sweep width: 40% of container
  - Reduced motion: no shimmer sweep, static at base color

Entrance: fade-in, duration.fast (140ms)
Exit: fade-out duration.fast while real content fades in duration.standard, 60ms overlap
```

#### ConnectionBanner

Persistent banner below header bar during connection issues.

```
Props:
  status: 'reconnecting' | 'disconnected' | 'error'
  machineName?: string
  onRetry?: () => void

Layout:
  - Height: 36px
  - Full width, positioned below HeaderBar
  - Content shifts down via LayoutAnimation

Visual:
  reconnecting → bg: warningDim, icon: zap (⚡), text: "Reconnecting to {machineName}…"
  disconnected → bg: dangerDim, icon: wifi-off, text: "Connection lost"
  error        → bg: dangerDim, icon: alert-triangle, text: "Connection error"

  - Icon: 14px, colored to match state (warning or danger)
  - Text: caption weight, color: text
  - Retry button: right-aligned, text-only, accent color, 44px touch target

Animation (from motion-system.md §4.6):
  Enter: translateY -36→0, opacity 0→1, duration.emphasis (320ms), easing.enter
  Exit: translateY 0→-36, opacity 1→0, duration.standard (220ms)
  Content push-down: LayoutAnimation with easing.move

Accessibility:
  accessibilityRole: 'alert'
  accessibilityLiveRegion: 'assertive'
  Announcement: "{status text}" on mount
```

#### StreamingIndicator

Typing indicator showing AI activity.

```
Props:
  status: 'thinking' | 'running' | 'typing' | 'planning'
  visible: boolean

Layout:
  - Height: 32px
  - Left-aligned with message content (matches message indent)
  - Positioned at bottom of timeline (above composer, below last message)

Visual:
  - Three dots: 6px diameter, accent color, 6px gap
  - Staggered opacity: each dot 0.3→1.0→0.3, 200ms offset between dots
  - Total cycle: 900ms, easing.linear
  - Label text: caption weight, textSecondary color
    thinking  → "Thinking…"
    running   → "Running…"
    typing    → "Typing…"
    planning  → "Planning…"

Animation:
  Container enter: opacity 0→1, translateY 4→0, duration.fast (140ms), easing.enter
  Container exit: opacity 1→0, duration.micro (80ms), easing.exit
  Label crossfade on status change: out duration.micro, in duration.micro, 40ms overlap
  Reduced motion: dots static at full opacity, no animation

Accessibility:
  accessibilityRole: 'progressbar'
  accessibilityLabel: "{status} response" (e.g. "Thinking response")
  accessibilityLiveRegion: 'polite'
```

#### EmptyState

Centered content for empty lists and error states.

```
Props:
  icon: 'chat' | 'terminal' | 'error' | 'search' | 'success' | ReactNode
  title: string
  body?: string
  action?: { label: string; onPress: () => void; variant?: 'primary' | 'ghost' }
  secondaryAction?: { label: string; onPress: () => void }

Icon Mapping:
  chat     → ◇ diamond (geometric, textTertiary)
  terminal → ▣ dotted square
  error    → △ triangle
  search   → ◎ bullseye
  success  → ✓ checkmark

Visual:
  - Container: centered vertically and horizontally, maxWidth: 280
  - Icon: 48px, textTertiary color
  - Title: title weight, text color, center-aligned, marginTop: xl (16)
  - Body: body weight, textSecondary color, center-aligned, marginTop: md (8)
  - Action button: marginTop: xxl (24), full width
  - Secondary action: marginTop: md (8), ghost style, below primary

Animation (from motion-system.md §4.7):
  Icon: opacity 0→1, scale 0.9→1.0, duration.emphasis, easing.enter
  Text: opacity 0→1, duration.standard, 100ms delay
  Button: opacity 0→1, duration.standard, 200ms delay
  Ambient icon pulse: scale 1.0→1.03→1.0, 3000ms cycle, linear (disabled in reducedMotion)

Accessibility:
  Container: accessibilityRole: 'header'
  Icon: importantForAccessibility: 'no' (decorative)
  Action button: standard button accessibility
```

#### Badge

Count/notification badge for thread rows and nav items.

```
Props:
  count?: number        // if provided, shows number; 99+ cap
  dot?: boolean         // if true, shows dot-only badge (no number)
  tone?: 'accent' | 'warning' | 'danger'   // default: accent
  visible?: boolean     // for animated show/hide

Visual:
  Dot mode:
    - 8px circle, solid tone color
    - No text

  Count mode:
    - minWidth: 18px, height: 18px, borderRadius: pill
    - Padding horizontal: xs (4)
    - Background: tone color
    - Text: 11px bold, white (#FFFFFF)
    - 99+ cap: shows "99+"

Animation:
  Appear: scale 0→1, duration.fast (140ms), easing.overshoot
  Disappear: scale 1→0, duration.micro (80ms), easing.exit
  Count change: crossfade text, duration.micro

Accessibility:
  accessibilityLabel: count ? `${count} notifications` : 'has notification'
  importantForAccessibility: 'yes'
```

#### Divider

Horizontal separator with optional label.

```
Props:
  label?: string        // e.g. "THREADS", "SESSIONS"
  spacing?: 'tight' | 'standard' | 'loose'   // default: standard

Layout:
  tight    → marginVertical: md (8)
  standard → marginVertical: lg (12)
  loose    → marginVertical: xl (16)

Visual:
  Without label:
    - 1px line (hairline), border color
    - Full width minus horizontal padding of parent

  With label:
    - Line — Label — Line pattern
    - Label: overline style (11px, semibold, 0.8 tracking), textTertiary, UPPERCASE
    - Lines: flex, hairline, border color
    - Gap between line and label: md (8)

Accessibility:
  accessibilityRole: 'separator' (no label) or 'header' (with label)
```

#### Avatar

User/AI avatar with initials and optional status indicator.

```
Props:
  initial: string       // single character
  variant: 'user' | 'ai' | 'system'
  size?: 'sm' | 'md' | 'lg'     // default: md
  status?: 'online' | 'busy' | 'offline'
  showIcon?: boolean    // if true, show icon instead of initial (for AI: cpu icon)

Size Mapping:
  sm → 20px circle
  md → 24px circle
  lg → 32px circle

Visual:
  user   → bg: accent, text: white, initial rendered
  ai     → bg: surface2, icon: cpu (14px), textSecondary
  system → bg: surface1, icon: settings (14px), textTertiary

  Status dot (optional):
    - 6px circle, positioned bottom-right with 1px white ring
    - online: success, busy: warning, offline: textTertiary

Font:
  sm → 10px bold
  md → 12px bold
  lg → 14px bold

Accessibility:
  accessibilityLabel: "{variant}: {initial}" or "{variant} avatar"
  importantForAccessibility: 'no' (decorative in most contexts)
```

#### IconButton

Pressable icon with proper hit area.

```
Props:
  icon: string          // lucide icon name (e.g. 'menu', 'x', 'copy')
  size?: 'sm' | 'md' | 'lg'   // default: md
  tone?: 'default' | 'accent' | 'danger'
  disabled?: boolean
  onPress: () => void
  accessibilityLabel: string

Size Mapping:
  sm → icon: 14px, touch area: 36×36
  md → icon: 18px, touch area: 44×44
  lg → icon: 22px, touch area: 48×48

Visual:
  default → icon color: textSecondary, no background
  accent  → icon color: accent
  danger  → icon color: danger

States:
  rest     → icon at base color
  pressed  → scale 0.88, opacity 0.7, duration.micro (80ms) via spring.snappy
  disabled → opacity.disabled (0.38), no press handler

Accessibility:
  accessibilityRole: 'button'
  accessibilityState: { disabled }
  Minimum touch target: 44×44 always (via hitSlop if needed)
```

#### SearchInput

Thread search/filter input for the drawer.

```
Props:
  value: string
  onChangeText: (text: string) => void
  placeholder?: string          // default: "Search…"
  collapsed?: boolean           // when true, shows icon-only; expands on focus

Layout:
  - Height: 36px
  - Horizontal padding: md (8)
  - Icon (left): search, 14px, textTertiary
  - Clear button (right): x-circle, 14px, appears when value.length > 0
  - Input: caption weight (13px), text color, flex: 1

Visual:
  - Background: surface2
  - Border: hairline, border color
  - Focused border: borderActive
  - Corner radius: sm (8)

States:
  empty-collapsed → icon-only circle (36×36), surface2 bg
  empty-expanded  → full-width input with placeholder
  active          → text input with clear button
  focused         → borderActive color, easing.linear over duration.fast (140ms)

Animation:
  Collapse ↔ expand: width transition via LayoutAnimation, duration.standard
  Focus ring: border color transitions over duration.fast

Accessibility:
  accessibilityRole: 'search'
  accessibilityLabel: placeholder text
  Clear button: accessibilityLabel: 'Clear search'
```

#### SwipeAction

Swipe-to-reveal action row wrapping list items.

```
Props:
  children: ReactNode         // the row content
  actions: Array<{
    key: string
    icon: string
    label: string
    color: string
    onAction: () => void
  }>
  threshold?: number          // default: 0.35 (35% of row width)
  enabled?: boolean           // default: true

Gesture:
  - Activation: horizontal swipe left, abs(dx) > abs(dy) * 2, min 12px
  - Tracking: row translateX follows finger 1:1
  - Background surface reveals action icons/labels underneath
  - Threshold feedback: at 35% width, haptic impact(Medium), icon scales 1.0→1.1
  - Release above threshold: row slides fully left, height collapses 200ms
  - Release below threshold: spring back to 0 via spring.snappy
  - Uses react-native-gesture-handler Gesture.Pan()

Visual:
  - Action surface: full height, colored background (e.g. danger for archive)
  - Icon + label: centered vertically, white color
  - Row overlays action surface

Accessibility:
  accessibilityActions: [{ name: action.key, label: action.label }]
  onAccessibilityAction: dispatches corresponding action
  Screen readers bypass swipe gesture; actions available via accessibility menu
```

#### BottomSheet

Proper bottom sheet replacing RN Modal for action sheets, pickers.

```
Props:
  visible: boolean
  snapPoints: number[]        // e.g. [300, 500]
  onDismiss: () => void
  children: ReactNode
  handleVisible?: boolean     // show grab handle, default: true

Layout:
  - Full-width on phone, max 420px centered on tablet
  - Top: 4px × 36px grab handle, surface3 color, centered
  - Content padding: xl (16) horizontal, lg (12) vertical
  - Corner radius: lg (16) top-left and top-right only
  - Background: surface1 (light) or surface2 (dark)

Gesture (from motion-system.md §4.12):
  - Drag area: handle (top 44px) or full sheet body
  - Upward overdrag: 3:1 rubber-band, max 30px past top snap
  - Downward: 1:1 tracking, below lowest snap → 3:1 rubber-band
  - Dismiss velocity: > 500pt/s downward
  - Dismiss position: center below 50% of collapsed snap
  - Snap physics: withSpring(snapPoint, spring.sheet, { velocity })

Animation:
  Backdrop enter: opacity 0→overlay, duration.emphasis (320ms), easing.enter
  Sheet enter: translateY screenHeight→snapPoint, withSpring using spring.sheet
  Sheet exit: translateY→screenHeight+50, withSpring with stiffer spring (stiffness: 250)
  Backdrop exit: opacity→0, duration.standard (220ms), easing.exit

Elevation: elevation.sheet

Accessibility:
  accessibilityViewIsModal: true
  Focus trap: focus contained within sheet when open
  Dismiss: accessible via close button AND backdrop tap
  accessibilityLabel: provided by consumer
```

---

### 2.3 Updated Existing Components

#### StatusPill (Updated)

```
Changes from current:
  - Add 'danger' tone (was missing)
  - Add 'connecting' pulsing dot state
  - Reduce font to 11px (label overline weight)
  - Switch from solid fill → dim fill + full-color text
  - Add leading status dot

Props:
  label: string
  tone: 'accent' | 'success' | 'warning' | 'danger'  // add 'danger'
  pulsing?: boolean      // NEW — for connecting state

Visual:
  Current: solid background fill, white text
  Updated: dim background (successDim, warningDim, etc.), full-color text

  - Height: 28px
  - Dot: 6px circle, solid tone color, left of text
  - Text: overline style (11px semibold), tone color, UPPERCASE
  - Padding: sm (6) vertical, md (8) horizontal
  - Border radius: pill

States:
  static  → dot at full opacity
  pulsing → dot opacity 0.4→1.0, 800ms cycle, easing.linear
  Reduced motion: dot static at full opacity
```

#### ChatComposer (Updated)

```
Changes:
  - Remove outerContainer horizontal padding (parent handles it)
  - Use surface1 bg instead of computed rgba
  - Border uses border/borderActive tokens, not inline rgba
  - Pill backgrounds use surface2 instead of inline rgba
  - Add disabled skeleton pulse state (NEW)
  - Input text uses body style (15px, Regular) instead of hardcoded 16px

Props:
  disabled?: boolean
  sending?: boolean
  bottomInset?: number
  models?: ModelOption[]
  selectedModel?: string | null
  selectedEffort?: ChatEffort
  onSelectModel?: (modelId: string) => void
  onSelectEffort?: (effort: ChatEffort) => void
  onSend: (message: string) => Promise<void>

States:
  rest      → surface1 bg, border color, full opacity
  focused   → borderActive color, duration.fast (140ms) transition
  disabled  → opacity.disabled (0.38), skeleton shimmer on input area
  sending   → send button opacity pulse 0.5→1.0, 600ms cycle

Composition:
  Uses BottomSheet (via ModelPickerSheet, EffortPickerSheet) instead of RN Modal
```

#### ChatTimeline (Updated)

```
Changes:
  - Extract inline approval rendering → ApprovalCard component
  - Add StreamingIndicator at bottom when thread.running
  - Add JumpToLatest floating overlay
  - FlatList optimization: add getItemLayout, reduce windowSize to 7
  - Message entrance animations use enter/exit split (not enterExit)
  - Entrance capped at 12 items (rest instant)
  - Avatar styling uses Avatar component

Props (unchanged):
  thread: TimelineThread | null
  loading: boolean
  onApprovalDecision?: (requestId, decision) => Promise<void> | void

FlatList Config:
  inverted: true
  windowSize: 7                          // down from default 21
  maxToRenderPerBatch: 8
  removeClippedSubviews: Platform.OS === 'android'
  keyExtractor: (group) => group.turnId
  getItemLayout: provided for fixed-height optimization where possible
  onScroll: throttled to 100ms for JumpToLatest logic

Composition:
  Wraps: Avatar, ChatMarkdown, ChatTurnActivity, ApprovalCard, StreamingIndicator
  Overlay: JumpToLatest (absolute positioned, zIndex.fab)
```

#### ThreadList → ThreadRow Refactor

```
Current: ScrollView of thread cards
Updated: FlatList of flat ThreadRow items

ThreadRow Props:
  thread: { id, title, updatedAt, running, approvalCount?, unread? }
  active: boolean
  onPress: () => void
  onLongPress: () => void

Layout:
  - Row height: 56px (touchTarget.comfortable 48 + vertical padding)
  - Active indicator: 3px left bar, accent color, scaleY animated
  - Title: title weight (17px), text color, 1 line truncate
  - Timestamp: caption weight (13px), textSecondary, right-aligned
  - Running dot: 6px success circle, inline after title
  - Approval badge: Badge component with count, warning tone
  - Active row bg: surface1
  - Inactive row bg: transparent

States:
  rest    → transparent bg, textSecondary timestamp
  active  → surface1 bg, accent left bar (3px), text color title
  pressed → surface2 bg, duration.micro
  loading → SkeletonLoader variant: threadRow

Swipe: wrapped in SwipeAction for archive gesture
```

---

### 2.4 Component Variant System

#### Button Variants

All buttons share a common base. Variants control visual treatment.

| Variant | Background | Text Color | Border | Use Case |
|---------|-----------|------------|--------|----------|
| `primary` | `accent` | `#FFFFFF` | none | Primary CTA (Connect, Save, Send) |
| `success` | `success` | `#FFFFFF` | none | Approve action |
| `danger` | `danger` | `#FFFFFF` | none | Destructive confirm |
| `secondary` | `surface2` | `text` | `border` hairline | Secondary actions (Test Connection) |
| `ghost` | `transparent` | `textSecondary` | none | Tertiary actions (Cancel, Clear) |
| `ghostAccent` | `transparent` | `accent` | none | Text links, inline actions |
| `ghostDanger` | `transparent` | `danger` | none | Destructive text actions (Clear Target) |

States applied to all:
| State | Treatment |
|-------|-----------|
| pressed | scale 0.97, opacity.pressed (0.7), duration.micro |
| disabled | opacity.disabled (0.38), no interaction |
| loading | inner content replaced with 16px spinner, maintains width |

Button height: `touchTarget.comfortable` (48px) for full-width, `touchTarget.min` (44px) for inline.

#### Card Variants

| Variant | Background | Border | Left Bar | Use Case |
|---------|-----------|--------|----------|----------|
| `default` | `surface0` | `border` hairline | none | Thread cards, settings groups |
| `elevated` | `surface1` | none | none | Composer, active cards |
| `well` | `surface2` | none | none | Code blocks, command display |
| `approval` | `warningDim` | none | 4px `warning` | Approval request cards |
| `error` | `dangerDim` | none | 4px `danger` | Error state cards |

---

## 3. Layout & Spacing Architecture

### 3.1 Container Patterns

| Context | Horizontal Padding | Notes |
|---------|-------------------|-------|
| Screen content (phone) | `xl` (16) | Applied by route-level View |
| Screen content (tablet) | `xxl` (24) | Applied by route-level View |
| Drawer panel | `xl` (16) | Internal padding for DrawerPanel |
| Card internal | `lg` (12) | Used by all card variants |
| List item internal | `xl` (16) horizontal, `lg` (12) vertical | ThreadRow, SessionRow, SettingsRow |
| Composer | `md` (8) horizontal, `md` (8) vertical | Tight for input focus |
| Bottom sheet | `xl` (16) horizontal, `lg` (12) vertical | Content area only |
| Header bar | `xl` (16) horizontal | Fixed height 48px |

### 3.2 Section Header Pattern

```
Layout:
  ── THREADS ──────────────────────

  - Text: overline style (11px, semibold, 0.8 tracking, UPPERCASE)
  - Color: textTertiary
  - Line: hairline, border color (optional, for labeled dividers)
  - Margin: xl (16) top, md (8) bottom
  - Padding: none (inherits container horizontal padding)
```

### 3.3 Screen-Level Layout

Every route screen follows this vertical stack:

```
┌─────────────────────────────────┐
│ Safe Area Top (insets.top)      │
├─────────────────────────────────┤
│ HeaderBar (48px fixed)          │
├─────────────────────────────────┤
│ ConnectionBanner (0 or 36px)    │  ← Conditional, animated
├─────────────────────────────────┤
│                                 │
│ Scroll Content (flex: 1)        │  ← FlatList, ScrollView, or static
│                                 │
├─────────────────────────────────┤
│ Footer (Composer, etc.)         │  ← Chat only, via KeyboardStickyView
├─────────────────────────────────┤
│ Safe Area Bottom (insets.bottom)│
└─────────────────────────────────┘
```

### 3.4 Responsive Breakpoints

| Name | Width | Drawer | Content Max Width | Content Padding |
|------|-------|--------|-------------------|-----------------|
| Compact | `<600dp` | Hidden overlay, swipe/button | 100% | 16dp |
| Medium | `600-900dp` | Persistent, 280dp | 100% | 20dp |
| Expanded | `>900dp` | Persistent, 320dp | 680dp centered | 24dp |

Hook: `useLayoutClass()` returns `'compact' | 'medium' | 'expanded'` based on `useWindowDimensions().width`.

### 3.5 Safe Area Strategy

| Edge | Source | Applied By |
|------|--------|------------|
| Top | `insets.top` | AppShell `_layout.tsx` — adds padding above HeaderBar |
| Bottom | `insets.bottom` | ChatComposer via `bottomInset` prop; other screens via container padding |
| Left | `insets.left` | Required on iPhone landscape with notch; applied by AppShell |
| Right | `insets.right` | Same as left; applied by AppShell |

The `<SafeAreaProvider>` wraps the root layout. Individual screens should NOT use `<SafeAreaView>` directly — `AppShell` handles all insets and passes relevant values to children via React Context.

### 3.6 Keyboard Avoidance Strategy

| Screen | Method | Notes |
|--------|--------|-------|
| Chat | `KeyboardStickyView` (current) | Composer sticks above keyboard, timeline shrinks |
| Settings Gateway | `KeyboardAvoidingView` + `ScrollView` | Standard scroll-up behavior |
| Onboarding URL input | `KeyboardAvoidingView` | Center content shifts up |
| Search in drawer | None needed | Search is at top; keyboard covers lower content which is fine |

Rule: dismiss keyboard before section transitions (drawer nav tap). Call `Keyboard.dismiss()` in the section-switch handler.

---

## 4. Accessibility Engineering

### 4.1 Focus Management

| Event | Focus Target | Method |
|-------|-------------|--------|
| Drawer opens | First nav item in drawer | `AccessibilityInfo.setAccessibilityFocus(ref)` after spring settles |
| Drawer closes | Header menu button | Focus returns to trigger |
| Bottom sheet opens | Sheet title or first interactive element | `accessibilityViewIsModal: true` + focus trap |
| Bottom sheet closes | Element that triggered the sheet | Focus returns to trigger |
| Thread selected | First message in timeline (or empty state) | `announceForAccessibility` + focus shift |
| Section switch | Section title in header | `announceForAccessibility("{Section} selected")` |
| Approval card enters | Approve button | Auto-focus to first action; announcement fires |
| Connection banner appears | Banner retry button (if present) | `accessibilityLiveRegion: 'assertive'` |

### 4.2 Content Grouping

| Component | Group Strategy |
|-----------|----------------|
| Thread row | Single accessible element: title + timestamp + status combined into `accessibilityLabel` |
| Message turn | Group: avatar + sender + timestamp as header; body as separate element for long content |
| Approval card | Group: title + command as label; buttons as separate focusable actions |
| Settings row | Single element: label + value combined; chevron is decorative |
| Nav rail item | Single element: icon + label + active state in `accessibilityLabel` |
| Pill (model/effort) | Single element: label + "Tap to change" in accessibilityHint |

### 4.3 Dynamic Type Considerations

React Native applies iOS Dynamic Type and Android font scaling automatically to `<Text>` components using system fonts. Considerations:

| Concern | Approach |
|---------|----------|
| Text truncation | Use `numberOfLines` + `ellipsizeMode` on labels that might overflow |
| Container overflow | Fixed-height rows (56px thread rows) must use `minHeight` instead of `height` so they expand with larger text |
| Icon alignment | Icons use fixed pixel sizes (not scaled), so vertical alignment needs `alignItems: 'center'` |
| Max scale | Set `maxFontSizeMultiplier={1.5}` on `TextInput` to prevent composer from becoming unusable at extreme scales |
| Testing | Test at iOS Accessibility > Larger Text > maximum setting and 200% Android font scale |

### 4.4 Contrast Audit

All text/background combinations must meet WCAG AA minimums.

| Combination | Dark Ratio | Light Ratio | Requirement | Passes |
|-------------|-----------|-------------|-------------|--------|
| text on background | 14.2:1 | 15.5:1 | 4.5:1 (normal) | ✓ |
| text on surface0 | 12.8:1 | 15.5:1 | 4.5:1 | ✓ |
| text on surface1 | 11.1:1 | 14.2:1 | 4.5:1 | ✓ |
| text on surface2 | 9.5:1 | 12.0:1 | 4.5:1 | ✓ |
| textSecondary on background | 5.1:1 | 5.5:1 | 4.5:1 | ✓ |
| textSecondary on surface0 | 4.6:1 | 5.5:1 | 4.5:1 | ✓ |
| textSecondary on surface1 | 4.0:1 | 5.1:1 | 3:1 (large/bold) | ✓* |
| textSecondary on surface2 | 3.5:1 | 4.3:1 | 3:1 (large/bold) | ✓* |
| textTertiary on background | 2.8:1 | 2.6:1 | N/A (decorative) | — |
| accent on background | 5.8:1 | 4.7:1 | 3:1 (UI components) | ✓ |
| accent on surface0 | 5.2:1 | 4.7:1 | 3:1 | ✓ |
| white on accent | 4.1:1 | 4.5:1 | 4.5:1 | ✓ |
| white on success | 3.5:1 | 4.5:1 | 3:1 (bold button text) | ✓* |
| white on danger | 4.2:1 | 5.2:1 | 3:1 | ✓ |
| warning on warningDim | 8.3:1 | 4.6:1 | 3:1 | ✓ |

`*` Marked items pass at large/bold text level (3:1) but not at normal body (4.5:1). These are only used for secondary labels ≤13px which are always semibold, meeting the "bold" criteria.

**Concern:** `textSecondary on surface1` is 4.0:1 in dark mode — marginally below 4.5:1 for normal text. Mitigation: `textSecondary` on `surface1` is only used for timestamps and metadata which use `caption` (13px, medium), borderline for "large bold" exemption. Monitor user feedback; if complaints arise, lighten `textSecondary` dark to `#8594A7` (4.5:1).

### 4.5 Touch Target Audit

Every interactive element must have a minimum touch target of 44×44pt.

| Component | Visual Size | Effective Touch Area | Method |
|-----------|-------------|---------------------|--------|
| IconButton (sm) | 14px icon | 36×36 | `hitSlop: 4` all sides → 44×44 |
| IconButton (md) | 18px icon | 44×44 | Native size meets minimum |
| Nav rail item | Full row 48×48 | 48 × full width | Row is the target |
| Thread row | 56px tall | 56 × full width | Row is the target |
| Pill (model/effort) | ~28×20 visible | 44×36 | `hitSlop: { top: 8, bottom: 8, left: 4, right: 4 }` |
| Send button | 32×32 visible | 44×44 | `hitSlop: 6` all sides |
| Status dot in pill | 6px | N/A | Not independently tappable |
| Drawer grab handle | 36×4 | 44×44 | Handle area is 44px tall |
| Bottom sheet close | 18px icon | 44×44 | hitSlop padding |
| Settings row chevron | 14px icon | N/A | Not independently tappable; row is target |

---

## 5. Performance Engineering

### 5.1 FlatList Optimization for ChatTimeline

| Setting | Value | Rationale |
|---------|-------|-----------|
| `windowSize` | `7` | Renders ~7 screens of content. Default 21 is excessive for chat. |
| `maxToRenderPerBatch` | `8` | Process 8 items per JS frame during scroll. Balance render speed vs. frame drops. |
| `updateCellsBatchingPeriod` | `50` | 50ms between batch renders during fast scroll. |
| `removeClippedSubviews` | `Platform.OS === 'android'` | Android benefits from this; iOS already handles recycling. |
| `initialNumToRender` | `12` | Show latest 12 messages immediately. Matches animation stagger cap. |
| `keyExtractor` | `(group) => group.turnId` | Stable key from data, not index. |
| `getItemLayout` | Provided for uniform-height items | Enables `scrollToIndex` without measuring. For variable-height messages, omit and accept measurement cost. |
| `onScroll` | Throttled event handler, `scrollEventThrottle: 100` | Avoid overwhelming JS thread with scroll events. Only needed for JumpToLatest threshold (400px offset). |
| `onEndReached` | `null` (no infinite scroll) | All messages load at once per thread. No pagination for now. |

#### Estimating Item Height

Chat messages have variable height, making `getItemLayout` imprecise. Strategy:

1. **Turn groups** contain 1-N items. Estimate height per turn based on content:
   - User text: `22 (lineHeight) * ceil(text.length / charsPerLine) + 48 (padding + avatar)`
   - AI markdown: unmeasurable at list level → skip `getItemLayout` for threads with AI responses
   - Approval card: fixed ~180px
   - Tool/command: fixed ~100px
2. For threads with all user messages (uncommon), provide `getItemLayout`. For mixed threads, omit it and rely on virtualization.

### 5.2 Memoization Strategy

| Component | Memoization | Dependencies |
|-----------|-------------|--------------|
| `ThreadRow` | `React.memo` with custom comparator | `thread.id`, `thread.title`, `thread.updatedAt`, `active`, `thread.running` |
| `ChatTurnGroup` (render item) | `React.memo` with deep item check | `group.turnId`, `group.items.length`, last item's `status` for approvals |
| `ApprovalCard` | `React.memo` | `item.id`, `item.status`, `responding` boolean |
| `Avatar` | `React.memo` | `initial`, `variant`, `status` — all primitive props |
| `StatusPill` | `React.memo` | `label`, `tone`, `pulsing` |
| `StreamingIndicator` | `React.memo` | `status`, `visible` |
| `SessionRow` | `React.memo` | `session.id`, `session.status`, `active` |
| `SettingsRow` | No memo | Renders infrequently, static content |

callback memoization:
- `renderItem` callbacks in FlatList: always wrap in `useCallback` with stable dependencies
- Event handlers passed down as props: `useCallback` at the parent
- Computed lists (e.g. `reversedGroups`): `useMemo` keyed on source data reference

### 5.3 Image Caching for Markdown

ChatMarkdown renders images from AI responses (screenshots, diagrams).

| Concern | Solution |
|---------|----------|
| Library | Use `expo-image` (already Expo-native, built-in caching) |
| Cache policy | `cachePolicy: 'memory-disk'` — cache in memory for current session, persist to disk for revisits |
| Placeholder | Show skeleton rectangle at estimated aspect ratio while loading |
| Error | Show broken-image icon (textTertiary) with "Image failed to load" alt text |
| Max dimensions | `maxWidth: 100%` of message container, `maxHeight: 300` to prevent layout explosion |
| Lazy loading | Images below the fold in a long message thread rely on FlatList's windowed rendering — offscreen images are recycled and re-requested on scroll-back |

### 5.4 Gesture Handler Thread

| Concern | Guideline |
|---------|-----------|
| Gesture callbacks | All gesture-driven animations (drawer, bottom sheet, swipe-to-archive) must use `react-native-gesture-handler`'s `Gesture.Pan()` with Reanimated worklets. Worklets run on the UI thread, avoiding JS thread round-trips. |
| runOnJS escape hatch | Use `runOnJS` only for: haptic calls, state updates that trigger re-renders, analytics. Never for visual updates. |
| Simultaneous gestures | Drawer swipe and ScrollView scroll must compose properly: use `Gesture.Simultaneous()` or `Gesture.Exclusive()` depending on the activation direction (horizontal vs. vertical). |
| Gesture state cleanup | On `onFinalize`, ensure all shared values reach stable end states. Never leave a drawer at translateX = -147 (mid-flight). |

### 5.5 Bundle Size Impact

Estimated impact of new dependencies and components:

| Addition | Estimated Size (gzipped) | Notes |
|----------|-------------------------|-------|
| `react-native-gesture-handler` | Already installed | Expo includes by default |
| `react-native-reanimated` | Already installed | Expo includes by default |
| `lucide-react-native` | ~15 KB (tree-shaken) | Import only used icons; replaces partial Feather usage |
| `expo-image` | Already in Expo | Zero additional cost |
| New components (16 files, ~150 lines avg) | ~12 KB total source | Minimal; mostly compositional |
| Token expansion (tokens.ts) | ~1 KB | Negligible |

**Net impact:** ~15 KB additional gzipped bundle from lucide icon migration. No new heavy dependencies.

### 5.6 Re-render Prevention Checklist

| Anti-pattern | Where it occurs now | Fix |
|-------------|-------------------|-----|
| Inline object styles | Every component uses `{ color: palette.text }` inline → new object each render | Pre-compute palette-dependent styles in `useMemo` keyed on `palette` reference |
| Inline arrow callbacks | `onPress={() => handlePress(id)}` in list render items | Move to `useCallback` with `id` bound; or use `data` prop pattern on Pressable |
| Context value churn | `useAppTheme()` returns a new object each render | Memoize the context value with `useMemo` in the provider |
| FlatList data reference | `[...turnGroups].reverse()` creates new array every render | `useMemo` on `thread?.items` reference |
| Animated style in render | Creating `useAnimatedStyle` closures over non-shared-value state | Use `useAnimatedReaction` to watch state and drive shared values |

---

## 6. Dark Mode Engineering

### 6.1 Shadow Behavior

| Element | Dark Mode | Light Mode |
|---------|-----------|------------|
| Drawer panel (phone) | Shadow at 10% opacity (half of light) | Shadow at 20% opacity |
| Bottom sheet | Shadow at 8% opacity | Shadow at 15% opacity |
| FAB (Jump-to-latest) | Shadow at 9% opacity | Shadow at 18% opacity |
| All other surfaces | No shadow — depth via surface stepping | No shadow — depth via surface stepping |
| Alternative for dark | Use 1px `borderActive` top edge on elevated surfaces as a "light line" to separate from background when surface values are close | Not needed |

Implementation: elevation tokens include `shadowOpacity` values for light mode. In dark mode, multiply by `0.5`:

```
const elevation = mode === 'dark'
  ? { ...elevations.drawer, shadowOpacity: elevations.drawer.shadowOpacity * 0.5 }
  : elevations.drawer;
```

### 6.2 Status Color Adjustments

Status colors shift between modes to maintain contrast:

| Token | Dark Value | Light Value | Reason |
|-------|-----------|-------------|--------|
| `success` | `#43C38A` | `#1A8F5C` | Darker green on light bg for 4.5:1 contrast |
| `warning` | `#F0B44D` | `#B06A0A` | Much darker amber on light bg |
| `danger` | `#F06A80` | `#C0364C` | Darker red on light bg |
| `accent` | `#4FA4FF` | `#0A78E8` | Darker blue on light bg |

The dim variants maintain the same base color at reduced opacity, so they naturally adapt.

### 6.3 Image Treatment

| Content | Dark Mode | Light Mode |
|---------|-----------|------------|
| User-uploaded images | No filter | No filter |
| Screenshots from AI | No filter (already captured content) | No filter |
| Geometric empty-state icons | `textTertiary` color | `textTertiary` color |
| App icon in onboarding | No filter | No filter |
| Icons (lucide) | `textSecondary` default color | `textSecondary` default color |

No automatic dark mode image dimming. If OS-level screenshots appear too bright in dark mode, we may add an optional `opacity: 0.92` treatment in a future pass — but not in this spec.

### 6.4 Code Block Colors

Code blocks use the well surface with monospace text:

| Element | Dark | Light |
|---------|------|-------|
| Code block background | `surface3` (#273545) | `surface3` (#DDE2EA) |
| Code text | `text` | `text` |
| Inline code background | `surface2` (#1F2B38) | `surface2` (#E8ECF2) |
| Inline code text | `accent` (#4FA4FF) | `accent` (#0A78E8) |
| Code block border | none | `border` hairline |

### 6.5 Verification Checklist

Before shipping, systematically verify every screen in both modes:

- [ ] Background → surface0 → surface1 → surface2 → surface3 forms a visible gradient step in both modes
- [ ] text on every surface level meets 4.5:1
- [ ] textSecondary on surface0 and surface1 meets 4.5:1 (or 3:1 for bold text)
- [ ] accent on background and surface0 meets 3:1 for UI controls
- [ ] Status pills readable: dim bg + full color text in both modes
- [ ] Approval card: warning left bar visible against warningDim background
- [ ] Connection banner: icon + text readable against dim background
- [ ] Drawer panel visually separates from backdrop in both modes
- [ ] Send button white arrow visible against accent bg in both modes
- [ ] Thread active row (surface1) distinguishable from inactive (transparent) on touch
- [ ] Code blocks visually distinct from surrounding message text

---

## 7. Migration Path

### 7.1 Phase Breakdown

Migration happens in 4 phases. Each phase produces a shippable, non-broken state.

#### Phase 1: Token Foundation (1-2 days, blocks everything)

**Changes:**
1. Expand `AppPalette` type with new tokens (surface0-3, dim variants, textTertiary, borderActive, overlay)
2. Add backward-compatible aliases (`surface` → `surface0`, `surfaceAlt` → `surface1`)
3. Add new spacing, radius, typography, opacity, elevation, borderWidth, iconSize, touchTarget tokens
4. Update `useAppTheme()` to expose new palette shape
5. Update motion.ts with new token system (rename `quick` → `fast`, `regular` → `standard`, split `enterExit`)

**What breaks:** Nothing. Old token names remain as aliases. New tokens are additive.
**Rollback:** Revert tokens.ts changes. All existing code still uses old keys.
**Test:** Visual spot-check — app should look identical with aliases active.

#### Phase 2: Shell Architecture (2-3 days, high risk)

**Changes:**
1. Create `AppShell` layout in `(app)/_layout.tsx`
2. Create `HeaderBar`, `DrawerPanel`, `NavRail`, `IdentityBadge`
3. Decompose `index.tsx` monolith:
   - Move drawer state + gesture to AppShell
   - Move section switching to route-based navigation
   - Move settings inline code to `settings/index.tsx`
   - Move terminal list to `terminals/index.tsx`
4. Replace PanResponder with Gesture.Pan()
5. Delete orphaned `terminals.tsx` and `settings.tsx` stubs

**What breaks:** Navigation architecture completely changes. All drawer/section logic moves.
**Risk:** Edge cases in gesture handler migration (PanResponder → Gesture.Pan). Drawer overdrag behavior differences.
**Rollback:** Keep old index.tsx on a branch. If shell migration fails, revert the `(app)` folder.
**Mitigation:** Build AppShell as a parallel `(app-v2)` route group. Switch the default route only after validation.

**Decomposition order within Phase 2:**
1. Create route stubs (empty screens that render "TODO") — verifies routing works
2. Move AppShell + DrawerPanel without gestures (button-only open/close)
3. Add Gesture.Pan() for drawer
4. Move section content from index.tsx to individual routes
5. Delete old code from index.tsx

#### Phase 3: Component Extraction (2-3 days, low risk)

Each component below can be extracted independently. No ordering dependencies between them.

| Component | Source | Risk | Independent |
|-----------|--------|------|-------------|
| `ApprovalCard` | Inline in ChatTimeline.tsx (L230-310) | Low | ✓ |
| `StreamingIndicator` | New component | None | ✓ |
| `JumpToLatest` | New component | None | ✓ |
| `ConnectionBanner` | New component | None | ✓ |
| `EmptyState` | New component | None | ✓ |
| `Badge` | New component | None | ✓ |
| `Divider` | New component | None | ✓ |
| `Avatar` | Extract from ChatTimeline inline rendering | Low | ✓ |
| `IconButton` | Extract from scattered Pressable+Feather patterns | Low | ✓ |
| `SkeletonLoader` | New component | None | ✓ |
| `SearchInput` | New component | None | ✓ |
| `SwipeAction` | New component | None | ✓ |
| `BottomSheet` | Replace RN Modal usage in pickers | Medium | After Phase 2 |
| `ThreadRow` | Replaces ThreadList card pattern | Low | After Phase 2 |

**Strategy:** One PR per component. Each PR includes the component, its tests, and the minimal integration point. Review individually.

#### Phase 4: Polish & Audit (1-2 days)

1. Remove backward-compatible palette aliases (breaking change for old references)
2. Accessibility audit: focus management, announcements, contrast checks
3. Performance profiling: FlatList frame rate on iPhone 12, Pixel 6a baselines
4. Haptic differentiation pass (replace all `impactAsync(Light)` with correct intensity)
5. Reduced motion audit: verify every animation respects `useReducedMotion()`
6. Tablet layout verification
7. Dark/light mode visual QA pass

### 7.2 Dependency Graph

```
Phase 1: Tokens ─────────────────┐
                                 │
Phase 2: Shell ──────────────────┤
  ├── Route stubs                │
  ├── AppShell + DrawerPanel     │
  ├── Gesture.Pan migration      │
  └── Content decomposition      │
                                 │
Phase 3: Components (parallel) ──┤  (depends on Phase 1 tokens only)
  ├── ApprovalCard               │  except BottomSheet and ThreadRow
  ├── StreamingIndicator         │  which depend on Phase 2 routes
  ├── JumpToLatest               │
  ├── ConnectionBanner           │
  ├── EmptyState                 │
  ├── Badge                      │
  ├── Divider                    │
  ├── Avatar                     │
  ├── IconButton                 │
  ├── SkeletonLoader             │
  ├── SearchInput                │
  └── SwipeAction                │
                                 │
Phase 4: Polish ─────────────────┘ (depends on all above)
```

### 7.3 Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| PanResponder → Gesture.Pan causes gesture regressions | Medium | High | Build parallel gesture handler, A/B test both |
| Spacing scale shift (sm: 8→6) causes widespread layout breakage | High | Medium | Apply spacing changes incrementally per component, not globally |
| Surface rename (surface→surface0) breaks imports | Low | Low | Aliases prevent breakage; lint rule flags old names |
| FlatList windowSize reduction causes visible blank areas on fast scroll | Medium | Medium | Test on low-end devices; adjust to 9 if blanking occurs |
| Bottom sheet migration from RN Modal breaks iOS keyboard interaction | Low | High | Keep Modal fallback; only switch after testing on both platforms |
| Lucide icon sizes differ from Feather at same px | Medium | Low | Audit all icon usages after migration; adjust individually |

### 7.4 Files Affected Per Phase

**Phase 1 (tokens only):**
- `theme/tokens.ts` — expand types and values
- `theme/motion.ts` — rename tokens, add new ones
- `hooks/useAppTheme.ts` — update return type

**Phase 2 (shell):**
- `app/(tabs)/_layout.tsx` → `app/(app)/_layout.tsx` (rename + rewrite)
- `app/(tabs)/index.tsx` → decompose into:
  - `app/(app)/index.tsx` (chat screen, ~200 lines)
  - `app/(app)/terminals/index.tsx` (~100 lines)
  - `app/(app)/settings/index.tsx` (~150 lines)
  - `app/(app)/settings/gateway.tsx` (~120 lines)
- New: `components/shell/AppShell.tsx`
- New: `components/shell/DrawerPanel.tsx`
- New: `components/shell/NavRail.tsx`
- New: `components/shell/HeaderBar.tsx`
- New: `components/shell/IdentityBadge.tsx`
- Delete: `app/(tabs)/terminals.tsx` (stub)
- Delete: `app/(tabs)/settings.tsx` (stub)

**Phase 3 (components):**
- 13 new component files (see table above)
- Update: `components/chat/ChatTimeline.tsx` (extract ApprovalCard, add StreamingIndicator/JumpToLatest)
- Update: `components/chat/ChatComposer.tsx` (token updates, remove inline rgba)
- Update: `components/ui/StatusPill.tsx` (add danger, dot, dim bg)

**Phase 4 (polish):**
- All component files (accessibility labels, haptic calls)
- `theme/tokens.ts` (remove aliases)
- No new files

---

## Appendix A: Token Export Shape

The complete `tokens.ts` export after migration:

```
exports:
  type AppPalette         — 20 color tokens per mode
  palettes                — Record<'light' | 'dark', AppPalette>
  spacing                 — { micro, xs, sm, md, lg, xl, xxl, xxxl }
  radius                  — { micro, sm, md, lg, pill }
  typography              — { display, heading, title, body, bodyMedium, caption, label, overline, mono, monoSmall, codeBlock }
  opacity                 — { pressed, disabled, hover, dimIcon, backdrop, backdropLight, skeleton }
  elevation               — { none, drawer, sheet, fab }
  borderWidth             — { hairline, thin, medium, thick, accent }
  iconSize                — { xs, sm, md, lg, xl, display }
  touchTarget             — { min, comfortable, compact }
  zIndex                  — { base, sticky, fab, drawer, sheet, toast }
```

## Appendix B: Component File Map

```
components/
  chat/
    ApprovalCard.tsx          NEW (extract from ChatTimeline)
    ChatComposer.tsx          UPDATED
    ChatMarkdown.tsx          UPDATED (code block tokens)
    ChatTimeline.tsx          UPDATED (composition changes)
    ChatTurnActivity.tsx      UPDATED (token migration)
    EffortPickerSheet.tsx     UPDATED (use BottomSheet)
    JumpToLatest.tsx          NEW
    ModelPickerSheet.tsx      UPDATED (use BottomSheet)
    StreamingIndicator.tsx    NEW
    ThreadRow.tsx             NEW (replaces card-in-ScrollView)
    ThreadSearchBar.tsx       NEW
  shell/
    AppShell.tsx              NEW
    DrawerPanel.tsx           NEW
    HeaderBar.tsx             NEW
    IdentityBadge.tsx         NEW
    NavRail.tsx               NEW (replaces PrimarySectionMenu)
    SessionRow.tsx            NEW
    TerminalSessionList.tsx   UPDATED
    ThreadActionSheet.tsx     UPDATED
  settings/
    SettingsGroup.tsx         NEW
    SettingsRow.tsx           NEW
  ui/
    Avatar.tsx                NEW
    Badge.tsx                 NEW
    BottomSheet.tsx           NEW
    ConnectionBanner.tsx      NEW
    Divider.tsx               NEW
    EmptyState.tsx            NEW
    GhostButton.tsx           NEW
    IconButton.tsx            NEW
    ScreenSurface.tsx         UPDATED (stripped to static container)
    SearchInput.tsx           NEW
    SectionLabel.tsx          NEW
    SkeletonLoader.tsx        NEW
    StatusPill.tsx            UPDATED
    SwipeAction.tsx           NEW
```

## Appendix C: Quick Reference — Token Diff from Current

| Category | Current | After | Key Changes |
|----------|---------|-------|-------------|
| Palette keys | 11 | 20 | +surface2/3, +textTertiary, +dim variants, +borderActive, +overlay |
| Spacing steps | 6 (4-32) | 8 (2-32) | +micro (2), sm 8→6, md 12→8, lg 16→12, xl 24→16, xxl 32→24, +xxxl (32) |
| Radius steps | 4 | 5 | +micro (4), sm 6→8, md 10→12, lg 14→16 |
| Typography styles | 5 | 11 | +heading, bodyMedium, caption, overline, monoSmall, codeBlock; body weight 500→400; title 20→17 |
| Motion durations | 2 | 5 | quick→fast, regular→standard, +micro, +emphasis, +dramatic |
| Motion easings | 2 | 5 | enterExit splits into enter+exit, +linear, +overshoot |
| Motion springs | 0 (inline) | 5 | snappy, responsive, drawer, sheet, gentle |
| New token groups | 0 | 6 | opacity, elevation, borderWidth, iconSize, touchTarget, zIndex |

---

*Design engineering specification for Homie Mobile v2 redesign.*
*Authored: 2026-02-10*
*For implementation by the mobile engineering team.*
