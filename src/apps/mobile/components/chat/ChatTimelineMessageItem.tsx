import * as Haptics from 'expo-haptics';
import {
  Check,
  CheckCircle2,
  ClipboardList,
  Copy,
  Cpu,
  Ellipsis,
  FileDiff,
  Globe,
  Loader,
  MessageCircleDashed,
  Terminal,
  TriangleAlert,
  XCircle,
} from 'lucide-react-native';
import { memo, useEffect, useRef, useState } from 'react';
import {
  type AccessibilityActionEvent,
  Platform,
  Pressable,
  ScrollView,
  Text,
  View,
} from 'react-native';

import { palettes, type AppPalette } from '@/theme/tokens';
import { ChatMarkdown } from './ChatMarkdown';
import {
  approvalStatusLabel,
  avatarInitial,
  bodyForItem,
} from './chat-timeline-helpers';
import { styles } from './chat-timeline-styles';
import type { ChatItem } from '@homie/shared';

/* ── shared card prop type ─────────────────────────────── */

interface CardProps {
  item: ChatItem;
  palette: AppPalette;
}

/* ── streaming debounce hook ──────────────────────────── */

const STREAMING_IDLE_MS = 300;

/**
 * During active streaming, returns true so callers can render cheap plain text.
 * After STREAMING_IDLE_MS of no content changes *or* when streaming ends,
 * returns false so callers switch to full ChatMarkdown rendering.
 */
function useStreamingDebounce(isStreaming: boolean, content: string): boolean {
  const [usePlainText, setUsePlainText] = useState(isStreaming);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!isStreaming) {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = null;
      setUsePlainText(false);
      return;
    }
    // Streaming: show plain text, schedule markdown switch on idle
    setUsePlainText(true);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setUsePlainText(false);
      timerRef.current = null;
    }, STREAMING_IDLE_MS);
  }, [isStreaming, content]);

  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    },
    [],
  );

  return usePlainText;
}

/* ── per-kind card components ──────────────────────────── */

const ChatTimelineReasoningCard = memo(function ChatTimelineReasoningCard({
  item,
  palette,
}: CardProps) {
  const detail = [...(item.summary ?? []), ...(item.content ?? [])].join('\n');
  const text = detail || 'Reasoning update';

  return (
    <View
      accessibilityRole="summary"
      accessibilityLabel={`Reasoning: ${text}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <MessageCircleDashed size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>Reasoning</Text>
        </View>
      </View>
      <Text style={[styles.messageBody, { color: palette.textSecondary }]}>{text}</Text>
    </View>
  );
});

const ChatTimelineCommandCard = memo(function ChatTimelineCommandCard({
  item,
  palette,
}: CardProps) {
  const status = item.status ?? 'running';
  const pillColors = commandStatusColors(palette, status);

  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`Command: ${item.command ?? 'command'}. Status: ${status}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <Terminal size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>Command</Text>
        </View>
        <View
          style={[
            styles.statusPill,
            { borderColor: pillColors.foreground, backgroundColor: pillColors.background },
          ]}>
          <CommandStatusIcon status={status} color={pillColors.foreground} />
          <Text style={[styles.statusPillLabel, { color: pillColors.foreground }]}>
            {status}
          </Text>
        </View>
      </View>

      {item.command ? (
        <Text style={[styles.monoBlock, { color: palette.text }]}>{`$ ${item.command}`}</Text>
      ) : null}

      {item.output ? (
        <ScrollView
          style={{ maxHeight: 160 }}
          nestedScrollEnabled
          accessibilityLabel="Command output">
          <Text style={[styles.monoBlock, { color: palette.textSecondary }]}>{item.output}</Text>
        </ScrollView>
      ) : null}
    </View>
  );
});

const ChatTimelineFileCard = memo(function ChatTimelineFileCard({
  item,
  palette,
}: CardProps) {
  const paths = item.changes?.map((c) => c.path) ?? [];

  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`File changes: ${paths.length} file${paths.length === 1 ? '' : 's'}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <FileDiff size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>
            File changes
          </Text>
        </View>
      </View>
      {paths.length > 0 ? (
        paths.map((p) => (
          <Text key={p} style={[styles.filePathText, { color: palette.text }]}>
            {p}
          </Text>
        ))
      ) : (
        <Text style={[styles.messageBody, { color: palette.textSecondary }]}>File changes</Text>
      )}
    </View>
  );
});

const ChatTimelinePlanCard = memo(function ChatTimelinePlanCard({
  item,
  palette,
}: CardProps) {
  const text = item.text || '';

  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`Plan: ${text}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <ClipboardList size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>Plan</Text>
        </View>
      </View>
      <Text style={[styles.monoBlock, { color: palette.text }]}>{text}</Text>
    </View>
  );
});

const ChatTimelineDiffCard = memo(function ChatTimelineDiffCard({
  item,
  palette,
}: CardProps) {
  const text = item.text || '';

  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`Diff: ${text}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <FileDiff size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>Diff</Text>
        </View>
      </View>
      <ScrollView style={{ maxHeight: 200 }} nestedScrollEnabled accessibilityLabel="Diff content">
        <Text style={[styles.monoBlock, { color: palette.text }]}>{text}</Text>
      </ScrollView>
    </View>
  );
});

const ChatTimelineSystemCard = memo(function ChatTimelineSystemCard({
  item,
  palette,
}: CardProps) {
  const text = item.text || 'System event';

  return (
    <View
      accessibilityRole="text"
      accessibilityLabel={`System: ${text}`}
      style={[
        styles.cardContainer,
        { backgroundColor: palette.surface1, borderColor: palette.border },
      ]}>
      <View style={styles.cardHeader}>
        <View style={styles.cardHeaderLeft}>
          <Globe size={12} color={palette.textSecondary} />
          <Text style={[styles.cardHeaderLabel, { color: palette.textSecondary }]}>System</Text>
        </View>
      </View>
      <Text style={[styles.systemText, { color: palette.textSecondary }]}>{text}</Text>
    </View>
  );
});

/* ── shared message actions strip ──────────────────────── */

const MessageActions = memo(function MessageActions({
  itemId,
  body,
  palette,
  isCopied,
  onCopy,
  onOpenMenu,
}: {
  itemId: string;
  body: string;
  palette: AppPalette;
  isCopied: boolean;
  onCopy: (id: string, body: string) => void;
  onOpenMenu: (id: string, body: string) => void;
}) {
  return (
    <View style={[styles.messageActions, { borderTopColor: palette.border }]}>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={isCopied ? 'Message copied' : 'Copy message'}
        hitSlop={6}
        onPress={() => onCopy(itemId, body)}
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
        onPress={() => onOpenMenu(itemId, body)}
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
  );
});

/* ── approval (unchanged) ──────────────────────────────── */

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

const ApprovalItem = memo(function ApprovalItem({
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
});

/* ── helpers ───────────────────────────────────────────── */

function commandStatusColors(palette: AppPalette, status: string) {
  if (status === 'success') return { foreground: palette.success, background: palette.successDim };
  if (status === 'failed') return { foreground: palette.danger, background: palette.dangerDim };
  return { foreground: palette.accent, background: palette.accentDim };
}

function CommandStatusIcon({ status, color }: { status: string; color: string }) {
  if (status === 'success') return <CheckCircle2 size={10} color={color} />;
  if (status === 'failed') return <XCircle size={10} color={color} />;
  return <Loader size={10} color={color} />;
}

/* ── main dispatcher ───────────────────────────────────── */

function ChatTimelineUserBubble({
  item,
  palette,
  body,
  bodySnippet,
  isCopied,
  showActions,
  onToggleActions,
  onOpenMenu,
  onCopy,
}: CardProps & {
  body: string;
  bodySnippet: string;
  isCopied: boolean;
  showActions: boolean;
  onToggleActions: (id: string) => void;
  onOpenMenu: (id: string, body: string) => void;
  onCopy: (id: string, body: string) => void;
}) {
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
      accessibilityLabel={`You: ${bodySnippet}`}
      accessibilityHint="Double tap to toggle actions. Long press for more message actions."
      accessibilityActions={[
        { name: 'activate', label: 'Toggle message actions' },
        { name: 'longpress', label: 'Open message action menu' },
        { name: 'copy', label: 'Copy message text' },
      ]}
      onAccessibilityAction={onAccessibilityAction}
      delayLongPress={260}
      onPress={() => onToggleActions(item.id)}
      onLongPress={() => {
        onToggleActions(item.id);
        onOpenMenu(item.id, body);
      }}
      style={({ pressed }) => [
        styles.messageRow,
        { backgroundColor: pressed ? palette.surface1 : 'transparent' },
      ]}>
      <View style={[styles.avatar, { backgroundColor: palette.accent }]}>
        <Text style={styles.avatarText}>{avatarInitial(item)}</Text>
      </View>
      <View style={styles.messageContent}>
        <Text style={[styles.senderName, { color: palette.text }]}>You</Text>
        <Text style={[styles.messageBody, { color: palette.text }]}>{body}</Text>
        {showActions ? (
          <MessageActions
            itemId={item.id}
            body={body}
            palette={palette}
            isCopied={isCopied}
            onCopy={onCopy}
            onOpenMenu={onOpenMenu}
          />
        ) : null}
      </View>
    </Pressable>
  );
}

function ChatTimelineAssistantMessage({
  item,
  palette,
  body,
  bodySnippet,
  isCopied,
  showActions,
  isStreaming = false,
  onToggleActions,
  onOpenMenu,
  onCopy,
}: CardProps & {
  body: string;
  bodySnippet: string;
  isCopied: boolean;
  showActions: boolean;
  isStreaming?: boolean;
  onToggleActions: (id: string) => void;
  onOpenMenu: (id: string, body: string) => void;
  onCopy: (id: string, body: string) => void;
}) {
  const usePlainText = useStreamingDebounce(isStreaming, body);

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
      accessibilityLabel={`Gateway: ${bodySnippet}`}
      accessibilityHint="Double tap to toggle actions. Long press for more message actions."
      accessibilityActions={[
        { name: 'activate', label: 'Toggle message actions' },
        { name: 'longpress', label: 'Open message action menu' },
        { name: 'copy', label: 'Copy message text' },
      ]}
      onAccessibilityAction={onAccessibilityAction}
      delayLongPress={260}
      onPress={() => onToggleActions(item.id)}
      onLongPress={() => {
        onToggleActions(item.id);
        onOpenMenu(item.id, body);
      }}
      style={({ pressed }) => [
        styles.messageRow,
        { backgroundColor: pressed ? palette.surface1 : 'transparent' },
      ]}>
      <View style={[styles.avatar, { backgroundColor: palette.surface1 }]}>
        <Cpu size={14} color={palette.textSecondary} />
      </View>
      <View style={styles.messageContent}>
        <Text style={[styles.senderName, { color: palette.text }]}>Gateway</Text>
        <View accessibilityRole="text">
          {usePlainText ? (
            <Text style={[styles.messageBody, { color: palette.text }]}>{body}</Text>
          ) : (
            <ChatMarkdown content={body} itemKind={item.kind} palette={palette} />
          )}
        </View>
        {showActions ? (
          <MessageActions
            itemId={item.id}
            body={body}
            palette={palette}
            isCopied={isCopied}
            onCopy={onCopy}
            onOpenMenu={onOpenMenu}
          />
        ) : null}
      </View>
    </Pressable>
  );
}

/* ── main dispatcher ───────────────────────────────────── */

interface ChatTimelineMessageItemProps {
  item: ChatItem;
  palette: AppPalette;
  isCopied: boolean;
  showActions: boolean;
  isStreaming?: boolean;
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
  isStreaming = false,
  approvalStatusForItem,
  approvalResponding = false,
  hasApprovalHandler,
  onToggleActions,
  onOpenMenu,
  onCopy,
  onApprovalDecision,
}: ChatTimelineMessageItemProps) {
  /* ── card items (no copy/menu actions) ──────────────── */
  switch (item.kind) {
    case 'approval':
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
    case 'reasoning':
      return <ChatTimelineReasoningCard item={item} palette={palette} />;
    case 'command':
      return <ChatTimelineCommandCard item={item} palette={palette} />;
    case 'file':
      return <ChatTimelineFileCard item={item} palette={palette} />;
    case 'plan':
      return <ChatTimelinePlanCard item={item} palette={palette} />;
    case 'diff':
      return <ChatTimelineDiffCard item={item} palette={palette} />;
    case 'system':
      return <ChatTimelineSystemCard item={item} palette={palette} />;
    default:
      break;
  }

  /* ── user / assistant / tool — interactive bubbles ──── */
  const body = bodyForItem(item);
  if (!body.trim()) return null;
  const bodySnippet = body.length > 120 ? `${body.slice(0, 120)}...` : body;

  if (item.kind === 'user') {
    return (
      <ChatTimelineUserBubble
        item={item}
        palette={palette}
        body={body}
        bodySnippet={bodySnippet}
        isCopied={isCopied}
        showActions={showActions}
        onToggleActions={onToggleActions}
        onOpenMenu={onOpenMenu}
        onCopy={onCopy}
      />
    );
  }

  // assistant, tool, or any remaining kind
  return (
    <ChatTimelineAssistantMessage
      item={item}
      palette={palette}
      body={body}
      bodySnippet={bodySnippet}
      isCopied={isCopied}
      showActions={showActions}
      isStreaming={isStreaming}
      onToggleActions={onToggleActions}
      onOpenMenu={onOpenMenu}
      onCopy={onCopy}
    />
  );
}

/* ── exports ───────────────────────────────────────────── */

export const ChatTimelineMessageItem = memo(ChatTimelineMessageItemBase);

export function triggerMessageHaptic() {
  if (Platform.OS !== 'web') {
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
  }
}
