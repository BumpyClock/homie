import { Pressable, StyleSheet, Text, View } from 'react-native';

import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatTimeline } from '@/components/chat/ChatTimeline';
import { ThreadList } from '@/components/chat/ThreadList';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useGatewayChat } from '@/hooks/useGatewayChat';
import { useGatewayTarget } from '@/hooks/useGatewayTarget';
import { radius, spacing, typography } from '@/theme/tokens';
import { useState } from 'react';

export default function ChatTabScreen() {
  const { palette } = useAppTheme();
  const [savingTarget, setSavingTarget] = useState(false);
  const {
    loading: loadingTarget,
    targetUrl,
    hasTarget,
    targetHint,
    error: targetError,
    saveTarget,
  } = useGatewayTarget();
  const {
    status,
    statusBadge,
    threads,
    activeChatId,
    activeThread,
    error,
    loadingThreads,
    loadingMessages,
    creatingChat,
    sendingMessage,
    selectThread,
    refreshThreads,
    createChat,
    sendMessage,
  } = useGatewayChat(targetUrl ?? '');

  const handleSaveTarget = async (value: string) => {
    setSavingTarget(true);
    try {
      await saveTarget(value);
    } finally {
      setSavingTarget(false);
    }
  };

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}>
        <View style={styles.headerRow}>
          <View>
            <Text style={[styles.eyebrow, { color: palette.textSecondary }]}>Gateway</Text>
            <Text style={[styles.title, { color: palette.text }]}>Chat</Text>
          </View>
          <StatusPill
            label={hasTarget ? statusBadge.label : 'Setup'}
            tone={hasTarget ? statusBadge.tone : 'warning'}
          />
        </View>

        {loadingTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Checking saved gateway configuration...
            </Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Add a gateway URL to enable chat on this device.
            </Text>
            <GatewayTargetForm
              initialValue={targetHint}
              hintValue={targetHint}
              saving={savingTarget}
              saveLabel="Save and Connect"
              onSave={handleSaveTarget}
            />
            {targetError ? (
              <Text style={[styles.setupHint, { color: palette.textSecondary }]}>{targetError}</Text>
            ) : null}
          </View>
        ) : null}

        {hasTarget ? (
          <View style={[styles.targetRow, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text numberOfLines={1} style={[styles.targetValue, { color: palette.textSecondary }]}>
              {targetUrl}
            </Text>
          </View>
        ) : null}

        <View style={styles.actionsRow}>
          <Pressable
            accessibilityRole="button"
            disabled={creatingChat || status !== 'connected' || !hasTarget}
            onPress={() => {
              void createChat();
            }}
            style={({ pressed }) => [
              styles.actionButton,
              styles.primaryAction,
              {
                backgroundColor: palette.accent,
                borderColor: palette.accent,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <Text style={[styles.actionLabel, { color: palette.surface }]}>
              {creatingChat ? 'Creating...' : 'New Chat'}
            </Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            disabled={loadingThreads || status !== 'connected' || !hasTarget}
            onPress={() => {
              void refreshThreads();
            }}
            style={({ pressed }) => [
              styles.actionButton,
              {
                backgroundColor: palette.surface,
                borderColor: palette.border,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <Text style={[styles.actionLabel, { color: palette.text }]}>
              {loadingThreads ? 'Refreshing...' : 'Refresh'}
            </Text>
          </Pressable>
        </View>

        {error ? (
          <View style={[styles.errorCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
          </View>
        ) : null}

        {hasTarget ? (
          <ThreadList
            threads={threads}
            activeChatId={activeChatId}
            loading={loadingThreads}
            onSelect={selectThread}
          />
        ) : null}

        <View style={styles.chatPane}>
          <ChatTimeline thread={hasTarget ? activeThread : null} loading={loadingMessages && hasTarget} />
          <ChatComposer
            disabled={status !== 'connected' || !activeThread || !hasTarget}
            sending={sendingMessage}
            onSend={sendMessage}
          />
        </View>
      </View>
    </ScreenSurface>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    paddingTop: spacing.xxl,
    paddingHorizontal: spacing.lg,
    gap: spacing.lg,
  },
  headerRow: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  eyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  title: {
    ...typography.display,
  },
  setupCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.sm,
  },
  setupTitle: {
    ...typography.title,
    fontSize: 20,
  },
  setupBody: {
    ...typography.body,
    fontWeight: '400',
  },
  setupHint: {
    ...typography.data,
    fontSize: 12,
  },
  targetRow: {
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
  },
  targetValue: {
    ...typography.data,
  },
  actionsRow: {
    flexDirection: 'row',
    gap: spacing.sm,
  },
  actionButton: {
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
  },
  primaryAction: {
    flex: 1,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 13,
  },
  errorCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    padding: spacing.sm,
  },
  errorText: {
    ...typography.body,
    fontWeight: '500',
    fontSize: 13,
  },
  chatPane: {
    flex: 1,
    minHeight: 0,
    gap: spacing.md,
  },
});
