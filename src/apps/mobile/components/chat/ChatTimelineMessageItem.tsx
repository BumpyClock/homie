import * as Haptics from 'expo-haptics';
import {
  Check,
  Copy,
  Cpu,
  Ellipsis,
  TriangleAlert,
} from 'lucide-react-native';
import { memo } from 'react';
import {
  type AccessibilityActionEvent,
  Platform,
  Pressable,
  Text,
  View,
} from 'react-native';

import { palettes, type AppPalette } from '@/theme/tokens';
import { ChatMarkdown } from './ChatMarkdown';
import {
  approvalStatusLabel,
  avatarInitial,
  bodyForItem,
  labelForItem,
} from './chat-timeline-helpers';
import { styles } from './chat-timeline-styles';
import type { ChatItem } from '@homie/shared';

interface ChatTimelineMessageItemProps {
  item: ChatItem;
  palette: AppPalette;
  isCopied: boolean;
  showActions: boolean;
  approvalStatusForItem?: string;
  approvalResponding?: boolean;
  hasApprovalHandler: boolean;
  onToggleActions: (itemId: string) => void;
  onOpenMenu: (itemId: string, body: string) => void;
  onCopy: (itemId: string, body: string) => void;
  onApprovalDecision: (
    item: ChatItem,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => void;
}

function ChatTimelineMessageItemBase({
  item,
  palette,
  isCopied,
  showActions,
  approvalStatusForItem,
  approvalResponding = false,
  hasApprovalHandler,
  onToggleActions,
  onOpenMenu,
  onCopy,
  onApprovalDecision,
}: ChatTimelineMessageItemProps) {
  if (item.kind === 'approval') {
    return (
      <ApprovalItem
        item={item}
        palette={palette}
        statusValue={approvalStatusForItem}
        responding={approvalResponding}
        hasApprovalHandler={hasApprovalHandler}
        onApprovalDecision={onApprovalDecision}
      />
    );
  }

  const body = bodyForItem(item);
  if (!body.trim()) return null;
  const isUser = item.kind === 'user';
  const bodySnippet = body.length > 120 ? `${body.slice(0, 120)}...` : body;

  const onAccessibilityAction = (event: AccessibilityActionEvent) => {
    if (event.nativeEvent.actionName === 'activate') {
      onToggleActions(item.id);
      return;
    }
    if (event.nativeEvent.actionName === 'longpress') {
      onOpenMenu(item.id, body);
      return;
    }
    if (event.nativeEvent.actionName === 'copy') {
      onCopy(item.id, body);
    }
  };

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`${labelForItem(item)}: ${bodySnippet}`}
      accessibilityHint="Double tap to toggle actions. Long press for more message actions."
      accessibilityActions={[
        { name: 'activate', label: 'Toggle message actions' },
        { name: 'longpress', label: 'Open message action menu' },
        { name: 'copy', label: 'Copy message text' },
      ]}
      onAccessibilityAction={onAccessibilityAction}
      delayLongPress={260}
      onPress={() => {
        onToggleActions(item.id);
      }}
      onLongPress={() => {
        onToggleActions(item.id);
        onOpenMenu(item.id, body);
      }}
      style={({ pressed }) => [
        styles.messageRow,
        {
          backgroundColor: pressed ? palette.surface1 : 'transparent',
        },
      ]}>
      <View
        style={[
          styles.avatar,
          {
            backgroundColor: isUser ? palette.accent : palette.surface1,
          },
        ]}>
        {isUser ? (
          <Text style={styles.avatarText}>{avatarInitial(item)}</Text>
        ) : (
          <Cpu size={14} color={palette.textSecondary} />
        )}
      </View>

      <View style={styles.messageContent}>
        <Text style={[styles.senderName, { color: palette.text }]}>{labelForItem(item)}</Text>
        {isUser ? (
          <Text style={[styles.messageBody, { color: palette.text }]}>{body}</Text>
        ) : (
          <ChatMarkdown content={body} itemKind={item.kind} palette={palette} />
        )}

        {showActions ? (
          <View style={[styles.messageActions, { borderTopColor: palette.border }]}> 
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={isCopied ? 'Message copied' : 'Copy message'}
              hitSlop={6}
              onPress={() => {
                onCopy(item.id, body);
              }}
              style={({ pressed }) => [
                styles.messageActionButton,
                {
                  backgroundColor: palette.surface1,
                  borderColor: palette.border,
                  opacity: pressed ? 0.82 : 1,
                },
              ]}>
              {isCopied ? (
                <Check size={13} color={palette.textSecondary} />
              ) : (
                <Copy size={13} color={palette.textSecondary} />
              )}
              <Text style={[styles.messageActionLabel, { color: palette.textSecondary }]}>Copy</Text>
            </Pressable>

            <Pressable
              accessibilityRole="button"
              accessibilityLabel="More message actions"
              hitSlop={6}
              onPress={() => {
                onOpenMenu(item.id, body);
              }}
              style={({ pressed }) => [
                styles.messageActionButton,
                {
                  backgroundColor: palette.surface1,
                  borderColor: palette.border,
                  opacity: pressed ? 0.82 : 1,
                },
              ]}>
              <Ellipsis size={13} color={palette.textSecondary} />
              <Text style={[styles.messageActionLabel, { color: palette.textSecondary }]}>More</Text>
            </Pressable>
          </View>
        ) : null}
      </View>
    </Pressable>
  );
}

interface ApprovalItemProps {
  item: ChatItem;
  palette: AppPalette;
  statusValue?: string;
  responding: boolean;
  hasApprovalHandler: boolean;
  onApprovalDecision: (
    item: ChatItem,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => void;
}

function ApprovalItem({
  item,
  palette,
  statusValue: statusValueProp,
  responding,
  hasApprovalHandler,
  onApprovalDecision,
}: ApprovalItemProps) {
  const statusValue = statusValueProp ?? item.status ?? 'pending';
  const resolved = statusValue !== 'pending';
  const canRespond = !resolved && !responding && item.requestId !== undefined && hasApprovalHandler;

  return (
    <View
      style={[
        styles.approvalCard,
        {
          backgroundColor: palette.warningDim,
          borderColor: palette.warning,
        },
      ]}>
      <View style={styles.approvalHeader}>
        <View style={styles.approvalTitleRow}>
          <TriangleAlert size={13} color={palette.warning} />
          <Text style={[styles.approvalTitle, { color: palette.warning }]}>Approval Required</Text>
        </View>
        <Text style={[styles.approvalStatus, { color: palette.textSecondary }]}>
          {approvalStatusLabel(statusValue)}
        </Text>
      </View>

      {item.reason ? (
        <Text style={[styles.messageBody, { color: palette.text }]}>{item.reason}</Text>
      ) : null}

      {item.command ? (
        <View
          style={[
            styles.commandCard,
            {
              backgroundColor: palette.surface0,
              borderColor: palette.border,
            },
          ]}>
          <Text style={[styles.commandText, { color: palette.text }]}>{`$ ${item.command}`}</Text>
        </View>
      ) : null}

      {!resolved ? (
        <View style={styles.approvalActions}>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Approve this request"
            accessibilityState={{ disabled: !canRespond }}
            disabled={!canRespond}
            onPress={() => {
              onApprovalDecision(item, 'accept');
            }}
            style={({ pressed }) => [
              styles.approvalButton,
              {
                backgroundColor: palette.success,
                borderColor: palette.success,
                opacity: pressed ? 0.84 : canRespond ? 1 : 0.58,
              },
            ]}>
            <Text style={[styles.approvalLabel, { color: palettes.light.surface0 }]}>
              {responding ? 'Working...' : 'Approve'}
            </Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Always approve for this session"
            accessibilityState={{ disabled: !canRespond }}
            disabled={!canRespond}
            onPress={() => {
              onApprovalDecision(item, 'accept_for_session');
            }}
            style={({ pressed }) => [
              styles.approvalButton,
              {
                backgroundColor: palette.accent,
                borderColor: palette.accent,
                opacity: pressed ? 0.84 : canRespond ? 1 : 0.58,
              },
            ]}>
            <Text style={[styles.approvalLabel, { color: palettes.light.surface0 }]}>Always</Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Decline this request"
            accessibilityState={{ disabled: !canRespond }}
            disabled={!canRespond}
            onPress={() => {
              onApprovalDecision(item, 'decline');
            }}
            style={({ pressed }) => [
              styles.approvalButton,
              {
                backgroundColor: palette.surface0,
                borderColor: palette.danger,
                opacity: pressed ? 0.84 : canRespond ? 1 : 0.58,
              },
            ]}>
            <Text style={[styles.approvalLabel, { color: palette.danger }]}>Decline</Text>
          </Pressable>
        </View>
      ) : null}
    </View>
  );
}

export const ChatTimelineMessageItem = memo(ChatTimelineMessageItemBase);

export function triggerMessageHaptic() {
  if (Platform.OS !== 'web') {
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
  }
}
