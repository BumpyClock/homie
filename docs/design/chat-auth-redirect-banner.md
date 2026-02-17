# Chat Auth Redirect Banner Design Doc

## Overview

- **Goals**: Replace inline provider auth flows in chat surfaces with a minimal, non-intrusive banner that redirects users to the Settings panel for provider authentication
- **Primary users**: Users who encounter unauthorized providers while in an active chat session
- **Success criteria**:
  - Banner appears only when provider authorization is required
  - Single tap/click navigates to Settings > Providers section
  - Banner is dismissible but re-appears if auth state hasn't changed
  - Zero disruption to chat flow - content remains scrollable behind/below banner

## Inputs and Constraints

- **Platform targets**: Web (ChatPanel), Mobile (ChatTimeline)
- **Breakpoints**: Web responsive (sm: 640px+), Mobile native
- **Design system**:
  - Web: Tailwind + CSS variables (index.css)
  - Mobile: React Native StyleSheet + tokens.ts
- **Component library**: Lucide icons (both platforms)
- **Technical constraints**:
  - Must integrate above existing chat content scroll area
  - Must not block message input or interfere with keyboard
  - Must animate in/out with existing motion system

## Information Architecture

- **Page hierarchy**: Banner sits at top of chat content area, below thread header
- **Navigation model**: Single action routes to Settings > Providers section
- **Key user flow**:
  1. User opens chat with unauthorized provider
  2. Banner appears with provider context
  3. User taps "Open Settings" / "Go to Settings"
  4. App navigates to Settings with Providers section active
  5. User completes auth flow
  6. Returns to chat; banner no longer appears

## Design System Strategy

### Existing tokens/components to reuse

**Web (index.css)**:
- `--warning` / `--warning-dim` for banner background tint
- `--border` for subtle border
- `--radius-md` (8px) for container
- `--duration-fast` (140ms) for enter animation
- `.homie-fade-in` animation class

**Mobile (tokens.ts)**:
- `palette.warning` / `palette.warningDim` for tint
- `palette.border` for border
- `radius.sm` (8px) for container
- `spacing.md` (8px), `spacing.lg` (12px) for padding
- `typography.caption` for text
- `motion.duration.fast` for animation

### Discovery notes

- Existing `errorBanner` style in chat-timeline-styles.ts provides a pattern but uses danger tones
- Web chat-panel.tsx has inline `!accountStatus.ok` block (lines 463-508) - this is what we're replacing
- Mobile ChatTimeline has state cards but those are centered empty states, not inline banners

### New tokens/components needed

- No new tokens required - using existing warning semantic colors
- New component: `AuthRedirectBanner` (one per platform, same API shape)

## Layout and Responsive Behavior

### Web
- Full width of chat content area
- Fixed height, does not scroll with content
- Positioned above scroll container, below thread header
- Collapses/expands with animation

### Mobile
- Full width minus horizontal padding
- Fixed height, does not scroll with content
- Positioned at top of FlatList area (above inverted content)
- Uses Reanimated for enter/exit

## ASCII Layout

```text
Web - Desktop/Tablet
+--------------------------------------------------+
| Thread Header                                    |
+--------------------------------------------------+
| [!] Provider authorization needed. [Go to Settings]|  <- NEW BANNER
+--------------------------------------------------+
| Chat content scroll area                         |
| [Message bubbles...]                             |
|                                                  |
+--------------------------------------------------+
| Composer bar                                     |
+--------------------------------------------------+

Web - Mobile (<640px)
+------------------------------+
| Thread Header                |
+------------------------------+
| [!] Provider auth needed.    |  <- NEW BANNER
| [Go to Settings]             |
+------------------------------+
| Chat content                 |
| [Messages...]                |
+------------------------------+
| Composer                     |
+------------------------------+

Mobile (React Native)
+------------------------------+
| Header (Tabs)                |
+------------------------------+
| [!] Sign in required         |  <- NEW BANNER
| [Open Settings]              |
+------------------------------+
| ChatTimeline (FlatList)      |
| [Messages inverted...]       |
+------------------------------+
| Composer                     |
+------------------------------+
```

## Component Inventory

### AuthRedirectBanner

**Purpose**: Display contextual authorization prompt with navigation action

**Props**:
| Prop | Type | Required | Description |
|------|------|----------|-------------|
| `visible` | boolean | yes | Controls visibility/animation |
| `message` | string | yes | Primary message text |
| `actionLabel` | string | no | Button text (default: "Go to Settings") |
| `onAction` | () => void | yes | Navigation callback |
| `onDismiss` | () => void | no | Optional dismiss callback |

**Variants/states**:
- `visible=true`: Banner rendered with enter animation
- `visible=false`: Banner exit animation, then unmounted
- `pressed` (action button): Slight opacity reduction (0.9)

**Composition notes**:
- Web: Tailwind classes, no dedicated stylesheet
- Mobile: StyleSheet object matching existing patterns

## Interaction and State Matrix

### Primary actions
| Action | Trigger | Result |
|--------|---------|--------|
| Go to Settings | Tap/click action button | Navigate to Settings > Providers |
| Dismiss | Tap X icon (if enabled) | Hide banner until next mount |

### States
| State | Visual | Behavior |
|-------|--------|----------|
| Entering | Fade + slide down (4px) | 140ms ease-out |
| Idle | Full opacity, static | Awaiting interaction |
| Button pressed | 90% opacity | 80ms micro duration |
| Exiting | Fade out | 80ms ease-out |

### Loading/empty/error
- Not applicable - banner only shows when auth status is known

### Validation
- Not applicable - no user input

## Visual System

### Color roles
| Role | Web Token | Mobile Token | Usage |
|------|-----------|--------------|-------|
| Background | `hsl(var(--warning-dim))` | `palette.warningDim` | Container fill |
| Border | `hsl(var(--warning) / 0.3)` | `palette.warning` @ 30% | Subtle edge |
| Icon | `hsl(var(--warning))` | `palette.warning` | AlertTriangle |
| Text primary | `hsl(var(--foreground))` | `palette.text` | Message |
| Text secondary | `hsl(var(--muted-foreground))` | `palette.textSecondary` | Sub-message |
| Action text | `hsl(var(--warning))` | `palette.warning` | Button label |

### Typography
| Element | Web | Mobile |
|---------|-----|--------|
| Message | text-sm (14px), font-medium | typography.caption (13px), weight 500 |
| Action | text-sm (14px), font-semibold | typography.label (12px), weight 600 |

### Spacing
| Dimension | Web | Mobile |
|-----------|-----|--------|
| Container padding | 12px | spacing.lg (12px) |
| Icon-to-text gap | 8px | spacing.md (8px) |
| Text-to-action gap | 12px | spacing.lg (12px) |
| Border radius | 8px | radius.sm (8px) |

### Iconography
- Icon: `AlertTriangle` from Lucide
- Size: 16px (both platforms)
- Color: warning foreground

## Accessibility

### Keyboard navigation
- Web: Action button is focusable, Enter/Space activates
- Banner container has `role="alert"` for screen reader announcement

### Focus order
1. Banner appears, screen reader announces message
2. Tab focuses action button
3. Tab moves to next focusable element in chat

### Contrast targets
- Warning text on warning-dim background: minimum 4.5:1
- Verified against both light/dark palettes in tokens

### ARIA notes
- Container: `role="alert"`, `aria-live="polite"`
- Action button: `role="button"`, `aria-label="Go to Settings to sign in to provider"`
- Dismiss button (if present): `aria-label="Dismiss authorization notice"`

## Content Notes

### Copy hierarchy
- **Primary message**: Concise, action-oriented
  - "Provider sign-in required to continue."
  - "Complete provider setup in Settings."
- **Action label**: Clear, imperative
  - Web: "Go to Settings"
  - Mobile: "Open Settings"

### Empty-state copy
- Not applicable

### Error messaging
- Not applicable - this IS the error state UI

## Implementation Notes

### Web integration point
Replace the existing inline auth block in `chat-panel.tsx` (lines 463-508) with:
```tsx
{!accountStatus.ok && (
  <AuthRedirectBanner
    visible={!accountStatus.ok}
    message={accountStatus.message}
    onAction={() => {/* navigate to settings, set activeSection="providers" */}}
  />
)}
```

### Mobile integration point
Add above the FlatList in `ChatTimeline.tsx`, inside the main container View:
```tsx
{!providerAuthOk && (
  <AuthRedirectBanner
    visible={!providerAuthOk}
    message="Provider sign-in required"
    onAction={() => router.push('/settings?section=providers')}
  />
)}
```

### State management
- Web: `accountStatus.ok` from `useChat` hook already available
- Mobile: Need to wire `useProviderAuth` hook to ChatTimeline props or context

---

## Quality Checklist

- [x] Requirements and constraints captured
- [x] Clear layout hierarchy for each breakpoint
- [x] ASCII layout diagram included
- [x] Components and states listed
- [x] Existing tokens/components reused
- [x] Accessibility guidance documented
- [x] Rationale provided for key decisions
