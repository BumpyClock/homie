// ABOUTME: Collapsible card showing grouped tool calls per turn in the chat timeline.
// ABOUTME: Expandable rows with tool names, status, and JSON payload detail views.

import { ChevronDown, ChevronUp } from 'lucide-react-native';
import { memo, useCallback, useMemo, useState } from 'react';
import {
  LayoutAnimation,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';

import { friendlyToolLabelFromItem, type ChatItem } from '@homie/shared';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { radius, spacing, type AppPalette, typography } from '@/theme/tokens';

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

function payloadForToolItem(item: ChatItem): string {
  const fallback = item.text?.trim() || 'No payload details available';
  if (!item.raw || typeof item.raw !== 'object') return fallback;
  try {
    const serialized = JSON.stringify(item.raw, null, 2);
    return serialized || fallback;
  } catch {
    return fallback;
  }
}

function callsLabel(count: number): string {
  return count === 1 ? '1 call' : `${count} calls`;
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

  const runLayoutTransition = useCallback((duration: number) => {
    const resolved = reducedMotion ? 0 : duration;
    LayoutAnimation.configureNext({
      duration: resolved,
      create: {
        type: LayoutAnimation.Types.easeInEaseOut,
        property: LayoutAnimation.Properties.opacity,
        duration: resolved,
      },
      update: { type: LayoutAnimation.Types.easeInEaseOut, duration: resolved },
      delete: {
        type: LayoutAnimation.Types.easeInEaseOut,
        property: LayoutAnimation.Properties.opacity,
        duration: resolved,
      },
    });
  }, [reducedMotion]);

  const isTurnActive =
    running &&
    ((turnId && activeTurnId === turnId) || (!turnId && !activeTurnId));

  const summary = useMemo(() => {
    const labels = Array.from(
      new Set(toolItems.map((item) => friendlyToolLabelFromItem(item, 'Tool call'))),
    );
    if (labels.length === 0) return 'Tool calls';
    if (labels.length <= 2) return labels.join(' • ');
    return `${labels.slice(0, 2).join(' • ')} +${labels.length - 2}`;
  }, [toolItems]);

  const toggleExpanded = useCallback(() => {
    runLayoutTransition(expanded ? motion.duration.fast : motion.duration.standard);
    triggerMobileHaptic(motion.haptics.activityToggle);
    setExpanded((current) => {
      if (current) setOpenToolId(null);
      return !current;
    });
  }, [expanded, runLayoutTransition]);

  const toggleToolDetail = useCallback((itemId: string) => {
    runLayoutTransition(motion.duration.fast);
    const nextOpen = openToolId === itemId ? null : itemId;
    triggerMobileHaptic(
      nextOpen
        ? motion.haptics.activityDetail
        : motion.haptics.activityToggle,
    );
    setOpenToolId((current) => (current === itemId ? null : itemId));
  }, [openToolId, runLayoutTransition]);

  return (
    <View
      style={[
        styles.card,
        {
          backgroundColor: palette.surface1,
          borderColor: palette.border,
        },
      ]}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={expanded ? 'Collapse agent activity' : 'Expand agent activity'}
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
        <Text style={[styles.summary, { color: palette.textSecondary }]}>{summary}</Text>
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
      </Pressable>
      {expanded ? (
        <View style={[styles.list, { borderTopColor: palette.border }]}>
          {toolItems.map((item, index) => {
            const open = openToolId === item.id;
            const status = statusLabel(item.status);
            const label = friendlyToolLabelFromItem(item, 'Tool call');
            return (
              <View
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
                  accessibilityLabel={open ? `Hide ${label} payload` : `Show ${label} payload`}
                  onPress={() => {
                    toggleToolDetail(item.id);
                  }}
                  style={({ pressed }) => [styles.toolPressable, { opacity: pressed ? 0.86 : 1 }]}>
                  <View style={styles.toolHeader}>
                    <Text style={[styles.toolLabel, { color: palette.text }]}>
                      {`${index + 1}. ${label}`}
                    </Text>
                    {open ? (
                      <ChevronUp size={13} color={palette.textSecondary} />
                    ) : (
                      <ChevronDown size={13} color={palette.textSecondary} />
                    )}
                  </View>
                  {status ? <Text style={[styles.toolStatus, { color: palette.textSecondary }]}>{status}</Text> : null}
                </Pressable>
                {open ? (
                  <Text style={[styles.payload, { color: palette.textSecondary }]}>
                    {payloadForToolItem(item)}
                  </Text>
                ) : null}
              </View>
            );
          })}
        </View>
      ) : null}
    </View>
  );
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
  summary: {
    ...typography.body,
    fontSize: 13,
    fontWeight: '500',
  },
  metaRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
    justifyContent: 'space-between',
  },
  metaText: {
    ...typography.data,
    fontSize: 12,
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
    gap: spacing.xs,
    padding: spacing.xs,
  },
  toolPressable: {
    gap: spacing.xs,
  },
  toolHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  toolLabel: {
    ...typography.label,
    flex: 1,
    fontSize: 12,
  },
  toolStatus: {
    ...typography.data,
    fontSize: 11,
  },
  payload: {
    ...typography.data,
    fontSize: 11,
    lineHeight: 15,
    marginTop: spacing.xs,
    paddingTop: spacing.xs,
  },
});
