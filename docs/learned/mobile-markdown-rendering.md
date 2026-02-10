# Mobile Markdown Rendering (Expo/RN) - 2026-02-10

## Goal
- Render agent chat messages (assistant/reasoning/tool text) as markdown in `src/apps/mobile/components/chat/ChatTimeline.tsx`.

## Candidate libraries

### 1) `react-native-marked` (recommended)
- npm: https://www.npmjs.com/package/react-native-marked
- repo: https://github.com/gmsgowtham/react-native-marked
- latest publish: `8.0.0` (npm `time.modified` 2025-12-30)
- peer deps include `react-native-svg` and RN `>=0.76.0` (compatible with current RN `0.81.5`).
- Pros:
  - Actively maintained recently.
  - Built for RN (no WebView), tokenized style override support.
  - Good fit for chat message rendering and code blocks.
- Cons:
  - Adds `react-native-svg` dependency.
  - Needs a custom style map for visual parity with existing theme tokens.

### 2) `@ronradtke/react-native-markdown-display`
- npm: https://www.npmjs.com/package/@ronradtke/react-native-markdown-display
- repo: https://github.com/RonRadtke/react-native-markdown-display
- latest publish: `8.1.0` (npm `time.modified` 2025-01-21)
- Pros:
  - Mature and widely used in RN projects.
  - Straightforward markdown-it rendering model.
  - Simple to style individual node types.
- Cons:
  - Less recent release cadence than `react-native-marked`.
  - Streaming-heavy chat updates may need memoization to avoid frequent full re-render costs.

### 3) `react-native-render-html` (+ markdown preprocessing)
- npm: https://www.npmjs.com/package/react-native-render-html
- repo: https://github.com/meliorence/react-native-render-html
- latest publish on npm: `6.3.4` (2022-06-26)
- Pros:
  - Powerful renderer if app already standardizes on HTML content.
- Cons:
  - Not markdown-native; requires an extra markdown->HTML step.
  - Package release cadence is older; not ideal for new markdown-first chat integration.

## Recommendation
- Use `react-native-marked` for this project.
- Implementation scope:
  1. Render assistant/reasoning/tool text via markdown component.
  2. Keep user messages as plain text for this phase.
  3. Style headings/lists/links/code blocks using existing token system.
  4. Add link-open handler via RN Linking and accessibility labels for code/link blocks.

## Notes
- Expo docs also list markdown editor options (for editing use-cases), but chat requires read/render-first behavior:
  - https://docs.expo.dev/guides/editing-richtext/
