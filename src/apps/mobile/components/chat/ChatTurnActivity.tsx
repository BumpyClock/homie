// ABOUTME: Collapsible card showing grouped tool calls per turn in the chat timeline.
// ABOUTME: Expandable rows with tool names, status, structured detail cards, and tool-type counters.
// ABOUTME: Opens ToolDetailSheet for web tools; handles pill overflow with flexWrap.

import { ChevronDown, ChevronUp } from 'lucide-react-native';
import { memo, useCallback, useMemo, useState } from 'react';
import {
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import Animated, { LinearTransition } from 'react-native-reanimated';

import {
  friendlyToolLabelFromItem,
  normalizeChatToolName,
  rawToolNameFromItem,
  type ChatItem,
} from '@homie/shared';
import { ToolDetailSheet } from '@/components/chat/ToolDetailSheet';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { radius, spacing, touchTarget, type AppPalette, typography } from '@/theme/tokens';

interface ChatTurnActivityProps {
  toolItems: ChatItem[];
  turnId?: string;
  activeTurnId?: string;
  running: boolean;
  palette: AppPalette;
}

function statusLabel(value: string | undefined): string | null {
  if (!value) return null;
  if (value === 'completed') return 'Completed';
  if (value === 'in_progress') return 'Running';
  if (value === 'pending') return 'Pending';
  if (value === 'failed') return 'Failed';
  return value;
}

function callsLabel(count: number): string {
  return count === 1 ? '1 call' : `${count} calls`;
}

/** Build structured tool type counts for pill display. */
interface ToolTypeCount {
  label: string;
  count: number;
}

function toolTypeCounts(items: ChatItem[]): ToolTypeCount[] {
  const counts = new Map<string, number>();
  for (const item of items) {
    const label = friendlyToolLabelFromItem(item, 'Tool call');
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return Array.from(counts.entries()).map(([label, count]) => ({ label, count }));
}

/** Last completed tool step preview for collapsed state. */
function lastStepPreview(items: ChatItem[]): string | null {
  for (let i = items.length - 1; i >= 0; i -= 1) {
    const item = items[i];
    if (item.status === 'in_progress') {
      const label = friendlyToolLabelFromItem(item, 'Tool call');
      return `Running: ${label}…`;
    }
  }
  for (let i = items.length - 1; i >= 0; i -= 1) {
    const item = items[i];
    if (item.status === 'completed') {
      const label = friendlyToolLabelFromItem(item, 'Tool call');
      return `Last: ${label}`;
    }
  }
  return null;
}

/** Check if a tool item is a web tool (browser, web_search, web_fetch). */
function isWebTool(item: ChatItem): boolean {
  const raw = rawToolNameFromItem(item);
  const normalized = normalizeChatToolName(raw);
  return normalized === 'web_search' || normalized === 'web_fetch' || normalized === 'browser';
}

/** Tool count pill component for summary row. */
interface ToolCountPillProps {
  label: string;
  count: number;
  palette: AppPalette;
}

function ToolCountPill({ label, count, palette }: ToolCountPillProps) {
  const displayText = count > 1 ? `${label} x${count}` : label;
  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`${label}, ${count} ${count === 1 ? 'call' : 'calls'}`}
      style={[styles.toolCountPill, { backgroundColor: palette.surface2 }]}>
      <Text
        style={[styles.toolCountPillText, { color: palette.textSecondary }]}
        numberOfLines={1}>
        {displayText}
      </Text>
    </View>
  );
}

function ChatTurnActivityCard({
  toolItems,
  turnId,
  activeTurnId,
  running,
  palette,
}: ChatTurnActivityProps) {
  const reducedMotion = useReducedMotion();
  const [expanded, setExpanded] = useState(false);
  const [openToolId, setOpenToolId] = useState<string | null>(null);
  const [sheetItem, setSheetItem] = useState<ChatItem | null>(null);

  const standardLayoutTransition = useMemo(
    () => (reducedMotion ? undefined : LinearTransition.duration(motion.duration.standard)),
    [reducedMotion],
  );
  const fastLayoutTransition = useMemo(
    () => (reducedMotion ? undefined : LinearTransition.duration(motion.duration.fast)),
    [reducedMotion],
  );

  const isTurnActive =
    running &&
    ((turnId && activeTurnId === turnId) || (!turnId && !activeTurnId));

  const counts = useMemo(() => toolTypeCounts(toolItems), [toolItems]);
  const collapsedPreview = useMemo(
    () => (expanded ? null : lastStepPreview(toolItems)),
    [expanded, toolItems],
  );

  const toggleExpanded = useCallback(() => {
    triggerMobileHaptic(motion.haptics.activityToggle);
    setExpanded((current) => {
      if (current) setOpenToolId(null);
      return !current;
    });
  }, []);

  const toggleToolDetail = useCallback((item: ChatItem) => {
    const itemId = item.id;
    const nextOpen = openToolId === itemId ? null : itemId;

    // For web tools, open bottom sheet instead of inline expansion
    if (nextOpen && isWebTool(item)) {
      triggerMobileHaptic(motion.haptics.activityDetail);
      setSheetItem(item);
      return;
    }

    triggerMobileHaptic(
      nextOpen
        ? motion.haptics.activityDetail
        : motion.haptics.activityToggle,
    );
    setOpenToolId((current) => (current === itemId ? null : itemId));
  }, [openToolId]);

  const closeSheet = useCallback(() => {
    setSheetItem(null);
  }, []);

  return (
    <>
      <Animated.View
        layout={standardLayoutTransition}
        style={[
          styles.card,
          {
            backgroundColor: palette.surface1,
            borderColor: palette.border,
          },
        ]}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={`${expanded ? 'Collapse' : 'Expand'} agent activity, ${callsLabel(toolItems.length)}${isTurnActive ? ', running' : ''}`}
          accessibilityHint="Shows grouped tool calls for this turn."
          onPress={toggleExpanded}
          style={({ pressed }) => [styles.headerPressable, { opacity: pressed ? 0.9 : 1 }]}>
          <View style={styles.headerRow}>
            <Text style={[styles.title, { color: palette.text }]}>Agent activity</Text>
            {expanded ? (
              <ChevronUp size={14} color={palette.textSecondary} />
            ) : (
              <ChevronDown size={14} color={palette.textSecondary} />
            )}
          </View>

          {/* Tool type pills with flexWrap for overflow */}
          <View style={styles.summaryPillsRow}>
            {counts.map(({ label, count }) => (
              <ToolCountPill
                key={label}
                label={label}
                count={count}
                palette={palette}
              />
            ))}
          </View>

          <View style={styles.metaRow}>
            <Text style={[styles.metaText, { color: palette.textSecondary }]}>
              {callsLabel(toolItems.length)}
            </Text>
            {isTurnActive ? (
              <View style={[styles.runningPill, { borderColor: palette.success }]}>
                <View style={[styles.runningDot, { backgroundColor: palette.success }]} />
                <Text style={[styles.runningText, { color: palette.success }]}>Running</Text>
              </View>
            ) : null}
          </View>
          {collapsedPreview ? (
            <Text
              numberOfLines={1}
              style={[styles.collapsedPreview, { color: palette.textTertiary }]}>
              {collapsedPreview}
            </Text>
          ) : null}
        </Pressable>
        {expanded ? (
          <Animated.View
            layout={standardLayoutTransition}
            style={[styles.list, { borderTopColor: palette.border }]}>
            {toolItems.map((item, index) => {
              const open = openToolId === item.id;
              const status = statusLabel(item.status);
              const label = friendlyToolLabelFromItem(item, 'Tool call');
              const useWebToolSheet = isWebTool(item);
              const statusColor = item.status === 'failed' ? palette.danger : palette.textSecondary;
              return (
                <Animated.View
                  layout={fastLayoutTransition}
                  key={item.id}
                  style={[
                    styles.toolRow,
                    {
                      backgroundColor: palette.surface0,
                      borderColor: palette.border,
                    },
                  ]}>
                  <Pressable
                    accessibilityRole="button"
                    accessibilityLabel={
                      useWebToolSheet
                        ? `View ${label} details`
                        : open
                          ? `Hide ${label} details`
                          : `Show ${label} details`
                    }
                    accessibilityHint={
                      useWebToolSheet
                        ? 'Opens details in a sheet.'
                        : 'Opens request and result details for this tool call.'
                    }
                    onPress={() => {
                      toggleToolDetail(item);
                    }}
                    style={({ pressed }) => [styles.toolPressable, { opacity: pressed ? 0.86 : 1 }]}>
                    <View style={styles.toolHeader}>
                      <Text
                        style={[styles.toolLabel, { color: palette.text }]}
                        numberOfLines={1}>
                        {`${index + 1}. ${label}`}
                      </Text>
                      {useWebToolSheet ? null : open ? (
                        <ChevronUp size={13} color={palette.textSecondary} />
                      ) : (
                        <ChevronDown size={13} color={palette.textSecondary} />
                      )}
                    </View>
                    {status ? (
                      <Text style={[styles.toolStatus, { color: statusColor }]}>
                        {status}
                      </Text>
                    ) : null}
                  </Pressable>
                  {/* Non-web tools: inline expansion */}
                  {open && !useWebToolSheet ? (
                    <View style={[styles.inlinePayload, { borderTopColor: palette.border }]}>
                      <Text
                        style={[styles.payload, { color: palette.textSecondary }]}
                        selectable>
                        {payloadForToolItem(item)}
                      </Text>
                    </View>
                  ) : null}
                </Animated.View>
              );
            })}
          </Animated.View>
        ) : null}
      </Animated.View>

      {/* Bottom sheet for web tool details */}
      <ToolDetailSheet
        visible={sheetItem !== null}
        item={sheetItem}
        palette={palette}
        onClose={closeSheet}
      />
    </>
  );
}

/** Maximum characters for inline payload preview. */
const INLINE_PAYLOAD_LIMIT = 720;

function payloadForToolItem(item: ChatItem): string {
  const fallback = item.text?.trim() || 'No payload details available';
  if (!item.raw || typeof item.raw !== 'object') return fallback;
  try {
    const serialized = JSON.stringify(item.raw, null, 2);
    if (!serialized) return fallback;
    // Cap long output for inline display
    if (serialized.length > INLINE_PAYLOAD_LIMIT) {
      return `${serialized.slice(0, INLINE_PAYLOAD_LIMIT)}…`;
    }
    return serialized;
  } catch {
    return fallback;
  }
}

export const ChatTurnActivity = memo(ChatTurnActivityCard);

const styles = StyleSheet.create({
  card: {
    borderRadius: radius.sm,
    borderWidth: 1,
    marginLeft: 36,
    overflow: 'hidden',
  },
  headerPressable: {
    gap: spacing.xs,
    padding: spacing.sm,
    minHeight: touchTarget.min,
  },
  headerRow: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  title: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
  summaryPillsRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: spacing.xs,
    marginTop: spacing.xs,
  },
  toolCountPill: {
    borderRadius: radius.micro,
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
  },
  toolCountPillText: {
    ...typography.caption,
    fontSize: 11,
  },
  metaRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
    justifyContent: 'space-between',
    marginTop: spacing.xs,
  },
  metaText: {
    ...typography.mono,
    fontSize: 12,
  },
  collapsedPreview: {
    ...typography.caption,
    fontSize: 12,
    fontStyle: 'italic',
    marginTop: spacing.xs,
  },
  runningPill: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
  },
  runningDot: {
    borderRadius: 3,
    height: 6,
    width: 6,
  },
  runningText: {
    ...typography.label,
    fontSize: 10,
    textTransform: 'uppercase',
  },
  list: {
    borderTopWidth: 1,
    gap: spacing.xs,
    padding: spacing.sm,
    paddingTop: spacing.xs,
  },
  toolRow: {
    borderRadius: radius.sm,
    borderWidth: 1,
    overflow: 'hidden',
  },
  toolPressable: {
    gap: spacing.xs,
    padding: spacing.xs,
    minHeight: touchTarget.min,
  },
  toolHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  toolLabel: {
    ...typography.label,
    flex: 1,
    fontSize: 12,
  },
  toolStatus: {
    ...typography.mono,
    fontSize: 11,
  },
  inlinePayload: {
    borderTopWidth: 1,
    padding: spacing.xs,
  },
  payload: {
    ...typography.mono,
    fontSize: 11,
    lineHeight: 15,
  },
});
