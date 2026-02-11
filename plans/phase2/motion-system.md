# Homie Mobile — Motion System Specification

> Motion is communication, not decoration. Every animation must answer "what changed?" faster than static UI alone.

---

## Table of Contents

1. [Design Principles](#1-design-principles)
2. [Motion Token System](#2-motion-token-system)
3. [Screen Transitions](#3-screen-transitions)
4. [Component Animations](#4-component-animations)
5. [Gesture Choreography](#5-gesture-choreography)
6. [Micro-interactions](#6-micro-interactions)
7. [Accessibility & Reduced Motion](#7-accessibility--reduced-motion)
8. [Performance Rules](#8-performance-rules)
9. [Haptic Integration](#9-haptic-integration)
10. [Quick Reference Tables](#10-quick-reference-tables)

---

## 1. Design Principles

### 1.1 Motion Philosophy

Homie is a **precision tool for operators**, not a consumer social app. Motion must feel:

- **Purposeful** — Every animation communicates spatial relationship, state change, or cause-and-effect. No gratuitous bounce or overshoot.
- **Fast** — Operators don't wait. Most transitions complete under 250ms. Nothing exceeds 500ms.
- **Interruptible** — Gesture-driven animations can be grabbed mid-flight. No animation should lock out user input.
- **Consistent** — The same interaction type always produces the same motion profile. Drawer always uses the same spring. Messages always enter the same way.
- **Quiet** — Ambient animations (streaming dots, connection pulse) run at low visual intensity. They communicate "alive" without demanding attention.

### 1.2 Motion Budget

Each screen should have a **maximum of one emphasis-level animation active at a time**. Multiple simultaneous attention-grabbing animations create chaos. When an approval card enters with emphasis timing, other elements should use standard or micro timing.

### 1.3 Spatial Model

Motion implies physical space:

- **Down = new content arriving** — Messages, notifications, and new items slide in from above or fade in place.
- **Left/Right = navigation hierarchy** — Drawer slides from left. Push transitions move content left (going deeper) or right (going back).
- **Up = user action rising** — Send button launches upward. Compose expands upward. Bottom sheets rise from below.
- **Scale = attention** — Elements that demand focus scale up slightly on entrance. Dismissed elements scale down to nothing.

---

## 2. Motion Token System

### 2.1 Duration Scale

| Token | Value | Use Case |
|-------|-------|----------|
| `duration.micro` | 80ms | Opacity-only transitions, state color changes, reduced-motion fallback ceiling |
| `duration.fast` | 140ms | Button press feedback, icon swaps, tooltip appear, send button opacity |
| `duration.standard` | 220ms | Default for most transitions: enter/exit, section crossfade, drawer settle |
| `duration.emphasis` | 320ms | Attention-worthy moments: approval card entrance, connection banner, bottom sheet enter |
| `duration.dramatic` | 450ms | Reserved for onboarding hero moments, first-connection success, empty→populated state. Maximum 1 per screen. |

**Rule:** If you can't decide which duration, use `standard`. If it still feels slow, drop to `fast`. Never add a duration above `dramatic`.

### 2.2 Easing Curves

| Token | Definition | Character | Use Case |
|-------|-----------|-----------|----------|
| `easing.enter` | `cubic-bezier(0.0, 0.0, 0.2, 1.0)` | Decelerating — fast start, gentle stop | Elements appearing: fade-in, slide-in, scale-up |
| `easing.exit` | `cubic-bezier(0.4, 0.0, 1.0, 1.0)` | Accelerating — slow start, fast finish | Elements leaving: fade-out, slide-out, dismiss |
| `easing.move` | `cubic-bezier(0.4, 0.0, 0.2, 1.0)` | Symmetric ease-in-out | Position changes, layout shifts, reorder |
| `easing.linear` | `cubic-bezier(0.0, 0.0, 1.0, 1.0)` | Constant velocity | Color transitions, opacity loops (streaming dots), progress bars |
| `easing.overshoot` | `cubic-bezier(0.175, 0.885, 0.32, 1.275)` | Slight overshoot past target | Playful reveals: FAB appear, pill pop-in, toast entrance |

**Reanimated mapping:**

| Token | Reanimated Code |
|-------|----------------|
| `easing.enter` | `Easing.out(Easing.cubic)` |
| `easing.exit` | `Easing.in(Easing.cubic)` |
| `easing.move` | `Easing.inOut(Easing.cubic)` |
| `easing.linear` | `Easing.linear` |
| `easing.overshoot` | `Easing.out(Easing.back(1.7))` |

### 2.3 Spring Configurations

Springs are used for gesture-driven and physics-based animations. Each config is tuned for its interaction feel.

| Token | Damping | Stiffness | Mass | Character | Use Case |
|-------|---------|-----------|------|-----------|----------|
| `spring.snappy` | 20 | 300 | 0.5 | Crisp, minimal overshoot | Button scale, toggle snap, pill selection |
| `spring.responsive` | 18 | 220 | 0.6 | Light bounce, quick settle | Send button, gesture release snap |
| `spring.drawer` | 22 | 180 | 0.8 | Smooth, dampened | Drawer open/close, panel slides |
| `spring.sheet` | 24 | 200 | 1.0 | Controlled, weighty | Bottom sheet snap points |
| `spring.gentle` | 26 | 120 | 1.0 | Slow, elegant settle | Onboarding transitions, hero moments |

**Velocity sensitivity:** All springs used with gestures should incorporate the gesture's final velocity via `withSpring(target, config, { velocity: gestureVelocity })`.

### 2.4 Naming Convention

All motion tokens live under the `motion` namespace:

```
motion.duration.micro        → 80
motion.duration.fast         → 140
motion.duration.standard     → 220
motion.duration.emphasis     → 320
motion.duration.dramatic     → 450

motion.easing.enter          → Easing.out(Easing.cubic)
motion.easing.exit           → Easing.in(Easing.cubic)
motion.easing.move           → Easing.inOut(Easing.cubic)
motion.easing.linear         → Easing.linear
motion.easing.overshoot      → Easing.out(Easing.back(1.7))

motion.spring.snappy         → { damping: 20, stiffness: 300, mass: 0.5 }
motion.spring.responsive     → { damping: 18, stiffness: 220, mass: 0.6 }
motion.spring.drawer         → { damping: 22, stiffness: 180, mass: 0.8 }
motion.spring.sheet          → { damping: 24, stiffness: 200, mass: 1.0 }
motion.spring.gentle         → { damping: 26, stiffness: 120, mass: 1.0 }
```

### 2.5 Stagger

When multiple items animate in sequence (thread list population, onboarding step checklist):

| Token | Value | Use Case |
|-------|-------|----------|
| `stagger.tight` | 30ms | Items in a dense list (thread rows, session rows) |
| `stagger.standard` | 60ms | Card groups, settings rows |
| `stagger.relaxed` | 100ms | Onboarding steps, empty-state elements |

**Cap:** Maximum 8 items staggered (30ms × 8 = 240ms total spread). Beyond 8, items appear instantly.

---

## 3. Screen Transitions

### 3.1 Section Switching (Chat ↔ Terminals ↔ Settings)

These are peer-level sections, not hierarchical. Use a **crossfade**, not a slide.

| Property | Value |
|----------|-------|
| Outgoing section | opacity 1→0, duration: `fast` (140ms), easing: `exit` |
| Incoming section | opacity 0→1, duration: `standard` (220ms), easing: `enter` |
| Overlap | 60ms — outgoing begins exiting, incoming starts after 80ms delay |
| Transform | None. No translateX/Y. Crossfade only. |
| Notes | Outgoing exits faster than incoming enters. This creates a brief "breath" moment that prevents visual collision. |

**On drawer item tap:**
1. Drawer closes (if phone) with `spring.drawer`
2. After 80ms delay (drawer is mid-travel), outgoing section begins fade-out
3. Incoming section fades in once outgoing reaches opacity ≤ 0.3

### 3.2 Thread Selection → Chat Detail

Selecting a thread from the drawer loads a new conversation. This is a **content swap within the same section**, not navigation.

| Property | Value |
|----------|-------|
| Timeline content | opacity 1→0→1 during data swap, duration: `fast` (140ms each direction) |
| Skeleton | If data not immediate, show skeleton shimmer at opacity 1 for up to 600ms |
| Message entrance | Once data arrives, messages enter via staggered `FadeInUp` |
| Drawer behavior | On phone: drawer closes. On tablet: drawer stays, thread row highlights |

### 3.3 Push/Pop Navigation (Settings → Gateway, Terminal List → Detail)

Hierarchical navigation uses a **horizontal slide with parallax layering**.

**Push (going deeper):**

| Property | Value |
|----------|-------|
| Current screen | translateX: 0 → -30%, opacity: 1→0.3, duration: `emphasis` (320ms), easing: `move` |
| New screen | translateX: 100% → 0, opacity: 0.8→1, duration: `emphasis` (320ms), easing: `enter` |
| Parallax ratio | Current screen moves at 30% of new screen's travel distance |
| Back button | Fades in with `fast` timing, 100ms delay after push begins |

**Pop (going back):**

| Property | Value |
|----------|-------|
| Current screen | translateX: 0 → 100%, opacity: 1→0.8, duration: `standard` (220ms), easing: `exit` |
| Previous screen | translateX: -30% → 0, opacity: 0.3→1, duration: `standard` (220ms), easing: `enter` |
| Notes | Exit (pop) is faster than entrance (push). Leaving should feel instant. |

**Gesture-driven pop:** On iOS, the edge-swipe-to-go-back gesture drives the same translateX values interactively. Release snaps to 0 or 100% via `spring.drawer`.

### 3.4 Onboarding Flow (Welcome → Connecting → Connected → Chat)

The onboarding is a **vertical journey** — each step rises into view, implying forward progress.

| Transition | Animation |
|------------|-----------|
| Welcome → Connecting | Welcome slides up + fades out (translateY: 0→-20, opacity→0, `standard`). Connecting fades in from below (translateY: 12→0, opacity 0→1, `emphasis`) |
| Connecting → Connected | Step checklist items animate to completion. Icon morphs from spinner to checkmark via scale pulse (1.0→1.15→1.0, `spring.gentle`). Connected screen crossfades in with `emphasis` timing. |
| Connected → Chat | Connected content scales down subtly (1.0→0.97) + fades out over `standard`. Chat screen enters with standard `ScreenSurface` animation. `notificationAsync(Success)` haptic fires. |

---

## 4. Component Animations

### 4.1 Chat Messages

#### User Message (Sent)

The user's own message should feel **launched** — immediate, confident.

| Phase | Animation |
|-------|-----------|
| Appear | translateY: 16→0, opacity: 0→1, duration: `fast` (140ms), easing: `enter` |
| Timing | Appears immediately after send button press, no delay |
| Stagger | N/A — single message at a time |
| Context | Runs concurrently with send button's scale_down→reset animation and input clearing |

#### AI Message (Received)

AI messages stream in over time. The **container** enters once, then **content fills progressively**.

| Phase | Animation |
|-------|-----------|
| Container appear | opacity: 0→1, translateY: 8→0, duration: `standard` (220ms), easing: `enter` |
| Streaming text | Text appears character-by-character (or chunk-by-chunk) with no per-character animation. Plain render. The cursor/caret blinks at the insertion point via opacity loop (0→1→0, 500ms cycle, `easing.linear`). |
| Complete | Cursor fades out over `micro` (80ms). No other transition on stream end. |
| Code block | Code blocks within a streamed message appear as a unit once the closing fence is detected. Fade in with `fast` timing. Syntax highlighting renders immediately (no progressive highlighting). |

#### Message Group (Historical Load)

When loading a full thread (switching threads, pull-to-refresh):

| Property | Value |
|----------|-------|
| Animation | Staggered `FadeInUp`, each message: translateY: 8→0, opacity: 0→1 |
| Duration | `fast` (140ms) per item |
| Stagger | `stagger.tight` (30ms) between items |
| Cap | First 12 visible messages animate. Rest appear instantly. |
| Direction | Bottom-up: newest message animates first, older messages follow |

### 4.2 Streaming Indicator

Three dots indicating AI is processing.

| Property | Value |
|----------|-------|
| Dot size | 6px diameter, `Accent` color |
| Dot spacing | 6px gap between dots |
| Animation | Each dot: opacity oscillates 0.3→1.0→0.3. `easing.linear`, 900ms total cycle. |
| Stagger | Dot 1 starts at 0ms, Dot 2 at 200ms, Dot 3 at 400ms |
| Container enter | opacity: 0→1, translateY: 4→0, duration: `fast` (140ms), easing: `enter` |
| Container exit | opacity: 1→0, duration: `micro` (80ms), easing: `exit` |
| Label animation | Text label ("Thinking…", "Running…") crossfades when status changes: outgoing opacity→0 over `micro`, incoming opacity→1 over `micro`, 40ms overlap |

### 4.3 Approval Cards

Approval cards require **interruption-level entrance** — they must be noticed.

#### Entrance

| Property | Value |
|----------|-------|
| Container | translateY: 12→0, opacity: 0→1, duration: `emphasis` (320ms), easing: `enter` |
| Left border | scaleY: 0→1 (grows from top), duration: `emphasis` (320ms), easing: `enter`, 80ms delay after container starts |
| Buttons | Staggered opacity+translateY entrance: `standard` duration, `stagger.standard` (60ms) between Approve → Deny → Approve Session |
| Haptic | `notificationAsync(Warning)` fires when card fully enters |

#### Decision (Approve/Deny)

| Action | Animation |
|--------|-----------|
| Tap Approve | Button scales 1.0→0.95→1.0 (`spring.snappy`). Card crossfades to result state: "✓ Approved" single line. Other buttons fade out (`fast`). Card height collapses via `LayoutAnimation` preset `easeInEaseOut`, 200ms. `notificationAsync(Success)` haptic. |
| Tap Deny | Same button scale feedback. Card crossfades to "✗ Denied". Other buttons fade out. Same collapse. `notificationAsync(Warning)` haptic. |
| After collapse | Result line persists for 3 seconds, then fades to 40% opacity over `standard` to recede into timeline history. |

### 4.4 Thread List Items (Drawer)

#### Initial Population

| Property | Value |
|----------|-------|
| Animation | Staggered `FadeIn` (opacity only, no translateY — the drawer slides in which provides enough motion) |
| Duration | `fast` (140ms) per item |
| Stagger | `stagger.tight` (30ms) |
| Cap | First 10 items animate. Rest appear instantly. |

#### New Thread Appears (Real-time)

| Property | Value |
|----------|-------|
| Position | Inserted at top of list |
| Animation | height: 0→auto (via `LayoutAnimation`), opacity: 0→1, duration: `standard` (220ms), easing: `enter` |
| Existing items | Shift down via `LayoutAnimation` with `standard` timing |

#### Selection State Change

| Property | Value |
|----------|-------|
| Active indicator (left bar) | scaleY: 0→1, origin: center, duration: `fast` (140ms), easing: `overshoot` |
| Background | backgroundColor transition to active color, duration: `micro` (80ms), easing: `linear` |
| Deselected row | Active indicator scaleY: 1→0, background transition, both `fast` timing |

#### Thread Row Swipe-to-Archive

| Phase | Animation |
|-------|-----------|
| Swipe reveal | Row content translateX follows finger. Red "Archive" action surface revealed underneath. |
| Threshold | 35% of row width. Below threshold: spring back to 0 on release. |
| Commit | Once past threshold and released: row slides fully left, height collapses (200ms), `impactAsync(Medium)` haptic at threshold. |
| Undo toast | If undo is supported: toast appears from bottom with `emphasis` timing, auto-dismisses after 4 seconds. |

### 4.5 Drawer

#### Open (Programmatic — Button Tap)

| Property | Value |
|----------|-------|
| Panel | translateX: -drawerWidth → 0, animation: `withSpring(0, motion.spring.drawer)` |
| Backdrop | opacity: 0→0.45, driven by panel progress (interpolated, not separately timed) |
| Content area | No transform. Stays in place. Only backdrop overlays it. |

#### Open (Gesture-Driven)

| Property | Value |
|----------|-------|
| Panel | translateX follows finger position directly (1:1 tracking from edge) |
| Tracking zone | Left 24px of screen initiates. Horizontal movement > vertical by factor of 1.5 required. |
| Release above threshold | If progress > 35% OR velocity > 300pt/s: snap open via `withSpring(0, motion.spring.drawer, { velocity })` |
| Release below threshold | If progress ≤ 35% AND velocity ≤ 300pt/s: snap closed via `withSpring(-drawerWidth, motion.spring.drawer, { velocity })` |

#### Close (Gesture-Driven)

| Property | Value |
|----------|-------|
| Panel | Swipe left on panel body. Panel translateX follows finger 1:1. |
| Release threshold | Close if progress < 65% OR leftward velocity > 300pt/s |
| Backdrop tap | Triggers programmatic close with `spring.drawer` |

#### Overdrag Behavior

| Direction | Behavior |
|----------|----------|
| Overdrag right (past fully open) | Rubber-band resistance: finger moves 3px, panel moves 1px. Max overdrag: 24px. Springs back to 0 on release with `spring.snappy`. |
| Overdrag left (past fully closed) | Not possible — clamped at closed position. |

#### Drawer Panel Content Entrance

When drawer opens, internal content animates:

| Element | Animation |
|---------|-----------|
| Identity badge | Already visible (no entrance animation) |
| Nav rail items | Already visible |
| Thread/session list | Items stagger in with `FadeIn`, `stagger.tight`, starting 100ms after drawer begins opening |
| Search bar | Fades in from 0 opacity, `standard` timing, 80ms delay |

### 4.6 Status Changes (Connection State)

#### Connected → Disconnected

| Property | Value |
|----------|-------|
| StatusPill | backgroundColor crossfades from `Success-Dim` to `Danger-Dim`, text changes, duration: `emphasis` (320ms), easing: `linear` |
| StatusPill dot | Color transitions from green to red, same timing |
| Connection banner | Slides in from above: translateY: -36→0, opacity: 0→1, duration: `emphasis` (320ms), easing: `enter`. Page content shifts down via `LayoutAnimation`. |
| Haptic | `notificationAsync(Error)` |

#### Disconnected → Connecting (Reconnecting)

| Property | Value |
|----------|-------|
| Banner text | Crossfade: "Connection lost" → "Reconnecting…", duration: `fast` (140ms) |
| Banner color | `Danger-Dim` → `Warning-Dim`, duration: `standard` (220ms), easing: `linear` |
| StatusPill | Dot begins pulsing: opacity 0.4→1.0, 800ms cycle, `easing.linear` |

#### Reconnecting → Connected

| Property | Value |
|----------|-------|
| StatusPill | backgroundColor → `Success-Dim`, dot stops pulsing (snap to full opacity), text → "Online" |
| Connection banner | Briefly shows "Connected ✓" in green for 1200ms, then slides out: translateY: 0→-36, opacity→0, `standard` timing. Content shifts back up via `LayoutAnimation`. |
| Haptic | `notificationAsync(Success)` |

### 4.7 Empty States

Empty states should feel **alive but calm** — a subtle ambient quality that says "the app is ready."

| Element | Animation |
|---------|-----------|
| Geometric icon | Slow, continuous subtle scale pulse: 1.0→1.03→1.0, 3000ms cycle, `easing.linear`. Imperceptible unless you stare. |
| Text content | Static — no animation |
| CTA button | Static — no animation on idle. Standard button press feedback on interaction. |
| Entrance (first render) | Icon: opacity 0→1 + scale 0.9→1.0, `emphasis` timing, `easing.enter`. Text: opacity 0→1, `standard` timing, 100ms delay. Button: opacity 0→1, `standard` timing, 200ms delay. |

### 4.8 Error States

Errors must grab attention without inducing panic.

| Element | Animation |
|---------|-----------|
| Error card entrance | translateY: -8→0 (slides down from above, unusual direction = "something wrong"), opacity: 0→1, duration: `emphasis` (320ms), easing: `enter` |
| Error icon | Single shake: translateX cycle 0→-4→4→-2→2→0, duration: 400ms, `easing.move`. Fires once on entrance, not looping. |
| Retry button | Standard micro-interaction (see §6.1) |
| Dismiss | Opacity 1→0, `fast` timing. Or replaced by success content with crossfade. |

### 4.9 Send Button

The send button has a **three-phase animation loop** tied to the compose lifecycle.

| Phase | Trigger | Animation |
|-------|---------|-----------|
| **1. Dormant** | Input empty | scale: 0.6, opacity: 0.35, via `spring.responsive` |
| **2. Ready** | Input has content | scale: 1.0, opacity: 1.0, via `spring.responsive` |
| **3. Pressed** | Touch down on button | scale: 0.85, opacity: 0.7, duration: `micro` (80ms), easing: `exit` |
| **4. Release/Send** | Touch up + message dispatched | scale: 0.85→1.15→1.0, via `spring.snappy`. Concurrent haptic `impactAsync(Medium)`. |
| **5. Sending** | Awaiting network | scale stays 1.0, opacity pulses 0.5→1.0→0.5, 600ms cycle, `easing.linear`. Stops when ack received. |

### 4.10 Model & Effort Pills

#### Selection Change

| Property | Value |
|----------|-------|
| Outgoing label | opacity: 1→0, duration: `micro` (80ms) |
| Incoming label | opacity: 0→1, duration: `fast` (140ms), 40ms delay |
| Pill container | Gentle width animation via `LayoutAnimation` to accommodate different label lengths, `standard` timing |
| Haptic | `selectionAsync()` on picker row tap |

#### Picker Sheet

See §4.12 Bottom Sheets for enter/exit spec.

Active item in picker: checkmark fades in with `fast` timing, row background transitions to `Accent-Dim` over `micro`.

### 4.11 Jump-to-Latest FAB

| State | Animation |
|-------|-----------|
| Appear (scroll > 2 screens from bottom) | scale: 0→1, opacity: 0→1, duration: `standard` (220ms), easing: `overshoot` |
| Disappear (scroll near bottom, or tapped) | scale: 1→0.8, opacity: 1→0, duration: `fast` (140ms), easing: `exit` |
| Tap | Triggers smooth scrollToEnd. FAB fades out once scroll completes. |

### 4.12 Bottom Sheets (ModelPicker, EffortPicker, ActionSheet)

| Phase | Animation |
|-------|-----------|
| **Enter** | Backdrop: opacity 0→0.45, `emphasis` (320ms), `easing.enter`. Sheet: translateY: screenHeight→snapPoint, `withSpring(snapPoint, motion.spring.sheet)`. |
| **Exit** | Sheet: translateY→screenHeight + 50, `withSpring(target, { ...motion.spring.sheet, stiffness: 250 })`. Backdrop: opacity→0, `standard` (220ms), `easing.exit`. |
| **Drag** | Sheet translateY follows finger 1:1. Overdrag up: rubber-band (3:1 ratio). Release: snap to nearest snap point via `spring.sheet`. |
| **Dismiss threshold** | Downward velocity > 500pt/s OR sheet pulled below 50% of snap height |

### 4.13 Connection Onboarding Step Checklist

During the "Connecting" onboarding screen, steps animate as they complete:

| State | Animation |
|-------|-----------|
| Pending | Dot is hollow circle, `Text-Tertiary` color. Static. |
| Active | Dot pulses: opacity 0.4→1.0, 600ms cycle, `Accent` color. Label becomes `Text-Primary`. |
| Complete | Dot morphs to filled circle with checkmark. Scale: 1.0→1.2→1.0 (`spring.snappy`). Color transitions to `Success`. Next step becomes active (200ms delay). |
| Failed | Dot morphs to × icon. Color transitions to `Danger`. Shake animation same as error icon (§4.8). |

---

## 5. Gesture Choreography

### 5.1 Drawer Swipe

| Parameter | Value |
|-----------|-------|
| Edge activation zone | 24px from left screen edge |
| Directionality gate | `abs(dx) > abs(dy) * 1.5` (strongly horizontal) |
| Minimum movement | 8px before gesture is recognized |
| Tracking | 1:1 — panel translateX matches finger dx exactly |
| Snap open threshold | Progress > 35% OR rightward velocity > 300 pt/s |
| Snap close threshold | Progress < 65% OR leftward velocity > 300 pt/s |
| Spring on release | `motion.spring.drawer` with captured release velocity |
| Overdrag resistance | Beyond fully-open: 3:1 resistance ratio, max 24px |
| Cancel (onPanResponderTerminate) | Spring to last stable state (open if was opening, closed if was closing) |
| Platform | Migrate from PanResponder to `react-native-gesture-handler` `Gesture.Pan()` for native thread performance |

### 5.2 Message Long-Press

| Phase | Animation |
|-------|-----------|
| Touch start | No immediate animation. Wait 150ms. |
| Long-press recognition (500ms) | Message container scales: 1.0→0.97, opacity: 1.0→0.92, duration: 150ms, easing: `exit`. Background darkens slightly (Surface-0 → Surface-1). `impactAsync(Heavy)` haptic. |
| Menu appears | Native ActionSheet/context menu opens. Message stays scaled down. |
| Menu dismissed | Message springs back: scale 0.97→1.0, opacity→1.0, `spring.snappy`. Background transitions back. |

### 5.3 Pull-to-Refresh

| Property | Value |
|----------|-------|
| Indicator style | Platform-native (UIRefreshControl on iOS, SwipeRefreshLayout on Android). Do not custom-animate. |
| Haptic | `impactAsync(Light)` when pull threshold is crossed |
| Content behavior | FlatList handles the overscroll natively |
| Post-refresh | If new items loaded, they animate in via standard staggered entrance (§4.1 Message Group or §4.4 Thread List) |

### 5.4 Thread Row Swipe-to-Archive

| Parameter | Value |
|-----------|-------|
| Activation | Horizontal swipe left on a thread row, `abs(dx) > abs(dy) * 2`, minimum 12px |
| Tracking | Row content translateX follows finger. Red background with "Archive" icon revealed underneath. |
| Visual feedback at threshold | At 35% width: row snaps slightly left (4px), `impactAsync(Medium)` haptic, archive icon scales up 1.0→1.1 |
| Release above threshold | Row slides fully off-screen left, height collapses to 0 via `LayoutAnimation` (200ms) |
| Release below threshold | Row springs back to translateX: 0 via `spring.snappy` |
| Undo | If supported, toast enters from bottom (see §4.8 patterns) |

### 5.5 Bottom Sheet Drag

| Parameter | Value |
|-----------|-------|
| Drag area | Sheet handle area (top 44px) or full sheet body |
| Upward overdrag | Rubber-band: 3:1 resistance ratio, max 30px past top snap point |
| Downward drag | 1:1 tracking until below lowest snap point, then 3:1 rubber-band |
| Dismiss velocity | Downward velocity > 500pt/s |
| Dismiss position | Sheet center below 50% of collapsed snap height |
| Snap physics | `withSpring(snapPoint, motion.spring.sheet, { velocity })` |

---

## 6. Micro-interactions

### 6.1 Button Press Feedback

All tappable elements (buttons, pills, rows) share a consistent press feedback profile.

| Element Type | Press State | Release State |
|-------------|-------------|---------------|
| **Primary button** (filled) | scale: 0.97, opacity: 0.85, duration: `micro` (80ms) | scale: 1.0, opacity: 1.0, via `spring.snappy` |
| **Ghost button** | scale: 0.97, opacity: 0.6, duration: `micro` (80ms) | spring back via `spring.snappy` |
| **Icon button** | scale: 0.88, opacity: 0.7, duration: `micro` (80ms) | spring back via `spring.snappy` |
| **List row** | backgroundColor transitions to `Surface-2`, duration: `micro` (80ms) | backgroundColor transitions back, duration: `fast` (140ms) |
| **Pill/chip** | scale: 0.95, backgroundColor to `Surface-3`, duration: `micro` | spring back, backgroundColor fades |

### 6.2 Toggle/Switch

| Phase | Animation |
|-------|-----------|
| Thumb position | translateX from off→on position, via `spring.snappy` |
| Track color | backgroundColor crossfades: off-color → `Accent`, duration: `fast` (140ms) |
| Haptic | `selectionAsync()` on state change |
| Press feedback | Thumb scales 1.0→1.15 while pressed (indicates grabbability), springs back on release |

### 6.3 Copy Confirmation

When user taps a code block or uses the copy action:

| Phase | Animation |
|-------|-----------|
| Tap | `impactAsync(Light)` haptic |
| Visual | Brief flash overlay on the copied content: `Accent-Dim` background fades in then out, 400ms total (`fast` in + `standard` out) |
| Indicator | Small "Copied" toast or inline label appears near the tap point: opacity 0→1, translateY: 4→0, `fast` timing. Auto-dismisses after 1500ms with `micro` fade-out. |

### 6.4 Skeleton Shimmer (Loading States)

For loading states (thread list loading, message history loading):

| Property | Value |
|----------|-------|
| Shape | Rounded rectangles matching the expected content layout (message bubbles, thread rows) |
| Base color | `Surface-1` |
| Shimmer highlight | Linear gradient sweep at 30° angle, highlight color: `Surface-2` at 40% opacity |
| Sweep speed | 1200ms per full sweep, `easing.linear`, continuous loop |
| Sweep width | 40% of container width |
| Entrance | Skeleton container fades in with `fast` timing on mount |
| Exit | Skeleton fades out (`fast`) while real content fades in (`standard`), 60ms overlap |

### 6.5 Loading Spinner

For inline loading states (send pending, connection testing):

| Property | Value |
|----------|-------|
| Type | Simple rotating arc (270° arc of a circle) |
| Size | 16px (inline), 24px (centered/empty-state) |
| Color | `Accent` (default), or contextual (`Text-Secondary` for subtle) |
| Speed | 800ms per full rotation, `easing.linear`, continuous |
| Entrance | opacity 0→1, `fast` (140ms) |
| Exit | opacity 1→0, `micro` (80ms) |

### 6.6 Text Input Focus Ring

| Phase | Animation |
|-------|-----------|
| Focus | Border color transitions from `Border` to `Accent`, duration: `fast` (140ms), easing: `linear`. Subtle glow effect if supported: `Accent` at 6% opacity outer shadow. |
| Blur | Border color transitions back, duration: `standard` (220ms). Glow fades. |

---

## 7. Accessibility & Reduced Motion

### 7.1 Detection

Use the existing `useReducedMotion` hook which reads from `AccessibilityInfo.isReduceMotionEnabled()` and listens for changes. All animated components must respect this.

### 7.2 Reduced Motion Behavior Matrix

| Animation Category | Normal | Reduced Motion |
|-------------------|--------|----------------|
| **Opacity transitions** | Full duration | Capped at `duration.micro` (80ms) |
| **translateX/Y** | Full duration + easing | Instant (0ms) — element appears at final position |
| **Scale transforms** | Springs/timing | Instant (0ms) — no scale animation |
| **Springs** | Full physics simulation | Replaced with `withTiming(target, { duration: 0 })` |
| **LayoutAnimation** | Animated height/position | `LayoutAnimation.configureNext(LayoutAnimation.Presets.easeInEaseOut)` with 0ms duration |
| **Color/background transitions** | Full duration | Capped at `duration.micro` (80ms) |
| **Continuous loops** (streaming dots, spinner, skeleton shimmer) | Full animation | Static — dots at full opacity, spinner hidden (show text "Loading…"), skeleton at base color without shimmer |
| **Empty state ambient pulse** | Subtle 3s cycle | Static — no pulse |
| **Gestures** | Full tracking + spring | Tracking still works (essential for function), but release snap is instant instead of spring |
| **Stagger delays** | 30-100ms per item | 0ms — all items appear simultaneously |
| **Haptics** | Full haptic map | Preserved — haptics are NOT motion. Keep all haptic feedback. |

### 7.3 What to ALWAYS Preserve (Even in Reduced Motion)

1. **Opacity transitions ≤ 80ms** — Brief fades prevent jarring pop-in without triggering motion sensitivity.
2. **Color transitions** — Background color changes at `micro` duration are not problematic for vestibular disorders.
3. **Gesture tracking** — The 1:1 finger tracking for drawer and bottom sheets is user-controlled motion and must remain functional.
4. **Haptic feedback** — Haptics are tactile, not visual motion. Always fire.
5. **Layout changes** — Height insertions/removals should still occur (structurally necessary) but without animation.
6. **Focus rings** — Border color changes on focus are accessibility features themselves.

### 7.4 What to ALWAYS Remove (In Reduced Motion)

1. **translateX/Y on entrance/exit** — The primary trigger for vestibular discomfort.
2. **Scale bouncing/overshoot** — Use `easing.overshoot` → no animation.
3. **Parallax** — Push/pop parallax layers → instant swap.
4. **Continuous ambient animation** — Empty state pulse, loading shimmer sweep.
5. **Spring physics** — Replace with instant position snaps.
6. **Shake/wobble** — Error icon shake → no shake.

### 7.5 Screen Reader Announcement Timing

| Event | Announcement Timing | Content |
|-------|---------------------|---------|
| New message received | 300ms after content renders (let screen reader finish current utterance) | "New message from Gateway: {first 100 chars}" |
| Approval card appeared | 500ms after entrance animation completes | "Approval required: {command}. Approve, Deny, or Approve for session." |
| Approval decision | Immediately on action | "Approved" or "Denied" |
| Connection state change | 200ms after state transition | "Connection lost" / "Reconnecting" / "Connected to {machine}" |
| Drawer opened | After spring settles (use `runOnJS` callback from spring completion) | "Navigation drawer opened" |
| Drawer closed | After spring settles | "Navigation drawer closed" |
| Section switched | After crossfade completes | "{Section name} selected" |
| Send message | Immediately on send | "Message sent" |
| Copy action | Immediately on copy | "Copied to clipboard" |

---

## 8. Performance Rules

### 8.1 Animated Properties — Safe List

**ONLY animate these properties via `react-native-reanimated` worklets on the UI thread:**

| Property | Notes |
|----------|-------|
| `transform: [{ translateX }]` | GPU-composited, no layout recalc |
| `transform: [{ translateY }]` | GPU-composited, no layout recalc |
| `transform: [{ scale }]` | GPU-composited, no layout recalc |
| `transform: [{ rotate }]` | For spinner only |
| `opacity` | GPU-composited |

These properties are handled by the native compositor and never trigger layout or paint passes.

### 8.2 Avoid Animating

| Property | Why | Alternative |
|----------|-----|-------------|
| `width` / `height` | Triggers full layout recalculation | Use `scale` transform or `LayoutAnimation` for one-off layout changes |
| `margin` / `padding` | Layout recalc | Use `transform: translateX/Y` |
| `borderRadius` | Paint recalc | Animate `opacity` of two overlapping views with different radii |
| `backgroundColor` on complex views | Can be expensive in deep trees | Animate `opacity` of an overlay view with the target color |
| `fontSize` / `lineHeight` | Text layout is expensive | Crossfade between two text elements |
| `shadowOpacity` / `shadowRadius` | Expensive on Android | Avoid shadows entirely (per design system) |

### 8.3 Worklet vs JS Thread

| Use Worklets (UI Thread) | Use JS Thread |
|--------------------------|---------------|
| Gesture-following animations (drawer, sheet drag) | `LayoutAnimation` calls |
| `withSpring`, `withTiming` on transform/opacity | Timer-based delays (`setTimeout` before animation) |
| `useAnimatedStyle` for derived animated values | State changes that trigger re-renders before animation |
| `interpolate` for mapping gesture progress to multiple properties | Analytics/logging on animation events |
| `runOnJS` for callbacks at animation milestones | Haptic triggers (must be called from JS) |

**Rule:** If an animation responds to a gesture or another animation's progress, it MUST run on the UI thread via worklets. If it's triggered by a discrete event (button tap, data arrival), it can use either, but prefer worklets for smoothness.

### 8.4 Memory Considerations for Long Chat Timelines

| Concern | Mitigation |
|---------|------------|
| Shared values accumulate | Shared values for entering animations should be scoped to the component. When a message scrolls off-screen and is recycled by FlatList, its shared values are garbage collected. |
| Entering/exiting animations on recycled views | Use `entering` and `exiting` props only on views near the viewport edges. For views deep in the scroll buffer, skip entrance animations entirely. |
| FlatList `windowSize` | Set `windowSize` to 7 (default 21) for chat timelines. Keeps ~7 screens worth of rendered items. |
| `removeClippedSubviews` | Enable on Android for chat timelines. Not needed on iOS (already optimized). |
| Staggered animations cap | Cap at 12 items (§4.1). Beyond that, items render without animation. |
| Continuous animations in off-screen items | Streaming dots and connection pulse should only run when the component is visible (use `useIsFocused` or intersection observer pattern). |
| Animated style dependencies | Keep `useAnimatedStyle` dependency count low (1-2 shared values). Avoid creating new animated styles on every render. |

### 8.5 Frame Budget

Target: **60 FPS** (16.6ms per frame) on all devices.

| Operation | Budget |
|-----------|--------|
| Gesture tracking callback | < 4ms |
| Animated style recalculation | < 2ms |
| Layout pass (triggered by LayoutAnimation) | < 8ms |
| Total per frame during animation | < 12ms (leaving 4ms headroom) |

**Profiling rule:** If any animation drops below 45 FPS on an iPhone 12 (baseline test device) or a mid-range Android (Pixel 6a equivalent), the animation must be simplified or removed.

---

## 9. Haptic Integration

### 9.1 Haptic Intensity Ladder

Haptics should match the **weight of the action** — light for navigation, medium for actions, heavy for destructive reveals.

| Intensity | Haptic Call | Actions |
|-----------|------------|---------|
| **Subtle** | `selectionAsync()` | Nav item tap, thread row tap, toggle switch, picker row tap, effort/model pill tap |
| **Light** | `impactAsync(Light)` | Drawer open complete, copy to clipboard, pull-to-refresh trigger, code block tap |
| **Medium** | `impactAsync(Medium)` | Send message, swipe-to-archive threshold, destructive swipe reveal |
| **Heavy** | `impactAsync(Heavy)` | Long-press menu activation |
| **Success** | `notificationAsync(Success)` | Approval: Approve, connection established, onboarding complete |
| **Warning** | `notificationAsync(Warning)` | Approval: Deny, approval card entrance, entering reconnecting state |
| **Error** | `notificationAsync(Error)` | Connection lost, error state entrance |

### 9.2 Haptic Timing Rules

1. **Fire on commitment, not on gesture start.** — The drawer haptic fires when the spring animation begins (release), not when the finger first touches.
2. **Never double-fire.** — If a tap triggers both a selection haptic and a navigation transition, fire only the selection haptic.
3. **Sync with visual.** — The haptic should fire within ±16ms of the corresponding visual change (same frame or adjacent frame).
4. **No haptics during reduced motion.** — Wait, actually: haptics ARE preserved during reduced motion mode (§7.2). They're tactile, not visual.

---

## 10. Quick Reference Tables

### 10.1 Complete Duration Table

| Token | Value | Examples |
|-------|-------|---------|
| `micro` | 80ms | Color transitions, opacity-only fades, reduced-motion ceiling |
| `fast` | 140ms | Button feedback, icon swap, send opacity, per-item entrance |
| `standard` | 220ms | Crossfade, drawer settle, default enter/exit, layout shift |
| `emphasis` | 320ms | Approval entrance, bottom sheet, connection banner, push nav |
| `dramatic` | 450ms | Onboarding hero, first-connection, empty→populated. Max 1/screen. |

### 10.2 Complete Easing Table

| Token | Bezier | Reanimated | Personality |
|-------|--------|------------|-------------|
| `enter` | (0, 0, 0.2, 1) | `Easing.out(Easing.cubic)` | Fast start, gentle land |
| `exit` | (0.4, 0, 1, 1) | `Easing.in(Easing.cubic)` | Slow start, fast exit |
| `move` | (0.4, 0, 0.2, 1) | `Easing.inOut(Easing.cubic)` | Symmetrical travel |
| `linear` | (0, 0, 1, 1) | `Easing.linear` | Constant rate |
| `overshoot` | (0.175, 0.885, 0.32, 1.275) | `Easing.out(Easing.back(1.7))` | Pop with slight bounce |

### 10.3 Complete Spring Table

| Token | D | S | M | Feel | Use |
|-------|---|---|---|------|-----|
| `snappy` | 20 | 300 | 0.5 | Crisp, almost no overshoot | Buttons, toggles, pills |
| `responsive` | 18 | 220 | 0.6 | Light bounce | Send button, gesture snap |
| `drawer` | 22 | 180 | 0.8 | Smooth, dampened | Drawer, panel slides |
| `sheet` | 24 | 200 | 1.0 | Controlled, weighty | Bottom sheets |
| `gentle` | 26 | 120 | 1.0 | Slow, elegant | Onboarding, hero moments |

### 10.4 Component → Animation Quick Map

| Component | Enter | Exit | Interact | Spring |
|-----------|-------|------|----------|--------|
| User message | translateY↑ + fade, `fast` | — | Long-press scale | — |
| AI message container | translateY↑ + fade, `standard` | — | Long-press scale | — |
| Streaming indicator | fade + translateY↑, `fast` | fade, `micro` | — | — |
| Approval card | translateY↑ + fade, `emphasis` | Height collapse, `standard` | Button scale | `snappy` |
| Thread row | Staggered fade, `fast` | Swipe + collapse | Press bg change | `snappy` |
| Drawer panel | — | — | Swipe tracking | `drawer` |
| Bottom sheet | translateY↑, spring | translateY↓, spring | Drag tracking | `sheet` |
| Status pill | — | — | Color crossfade, `emphasis` | — |
| Connection banner | translateY↓, `emphasis` | translateY↑, `standard` | — | — |
| Send button | scale up from 0.6, spring | — | Press scale | `responsive` |
| Model/effort pill | — | — | Label crossfade, `micro`→`fast` | — |
| Jump-to-latest FAB | scale+fade, `standard` | scale+fade, `fast` | Tap → scroll | — |
| Empty state | Staggered fade+scale, `emphasis` | — | Ambient pulse (3s) | — |
| Error state | translateY↓ + fade, `emphasis` | fade, `fast` | Icon shake (400ms) | — |
| Toggle switch | — | — | Thumb translateX | `snappy` |
| Skeleton shimmer | fade, `fast` | fade, `fast` | Sweep loop (1200ms) | — |

### 10.5 Haptic Quick Map

| Action | Haptic |
|--------|--------|
| Nav tap | `selection` |
| Thread tap | `selection` |
| Drawer open | `impact(Light)` |
| Send message | `impact(Medium)` |
| Long-press menu | `impact(Heavy)` |
| Copy | `impact(Light)` |
| Approve | `notification(Success)` |
| Deny | `notification(Warning)` |
| Connection lost | `notification(Error)` |
| Connection restored | `notification(Success)` |
| Pull-to-refresh | `impact(Light)` |
| Swipe threshold | `impact(Medium)` |
| Toggle | `selection` |
| Picker row | `selection` |

---

## Appendix A: Migration from Current System

### Current → New Token Mapping

| Current Token | New Token | Notes |
|---------------|-----------|-------|
| `motion.duration.quick` (140ms) | `motion.duration.fast` (140ms) | Renamed for clarity. Same value. |
| `motion.duration.regular` (220ms) | `motion.duration.standard` (220ms) | Renamed. Same value. |
| `motion.easing.enterExit` | Split into `motion.easing.enter` and `motion.easing.exit` | Enter uses ease-out (decelerate into rest). Exit uses ease-in (accelerate away). |
| `motion.easing.move` | `motion.easing.move` | Same curve, same name. |
| `SEND_SPRING` (d:14, s:200, m:0.6) | `motion.spring.responsive` (d:18, s:220, m:0.6) | Slightly more dampened for refinement. Update existing ChatComposer. |
| Drawer `withTiming` + `easing.enterExit` | `withSpring` + `motion.spring.drawer` | Migrate from timing to spring for more natural settle. |

### Breaking Changes

1. `motion.easing.enterExit` is removed. All usages must choose `enter` or `exit`.
2. Drawer animation moves from `withTiming` to `withSpring`. The drawer progress value still interpolates 0→1, but the driving function changes.
3. `ScreenSurface` mount animation is removed (AppShell handles section transitions). The component becomes a static container.
4. `PanResponder` is replaced with `react-native-gesture-handler` `Gesture.Pan()`. All gesture code must be rewritten.

### Migration Priority

1. Update `theme/motion.ts` with new token definitions
2. Migrate ChatComposer's `SEND_SPRING` to use `motion.spring.responsive`
3. Split `enterExit` usages in ChatTimeline into `enter`/`exit`
4. Convert drawer from `withTiming` → `withSpring`
5. Migrate `PanResponder` → `Gesture.Pan()`
6. Add new components with new tokens from the start

---

*Motion system specification for Homie Mobile.*
*Companion to: [UX Redesign Plan](./ux-redesign.md)*
*Last updated: 2026-02-10*
