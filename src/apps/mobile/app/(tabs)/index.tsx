import { useMemo, useState } from 'react';
import { KeyboardStickyView } from 'react-native-keyboard-controller';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatTimeline } from '@/components/chat/ChatTimeline';
import { ThreadList } from '@/components/chat/ThreadList';
import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { ThreadActionSheet } from '@/components/shell/ThreadActionSheet';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export default function ChatTabScreen() {
  const { palette } = useAppTheme();
  const insets = useSafeAreaInsets();
  const [actioningThreadId, setActioningThreadId] = useState<string | null>(null);
  const [busyThreadAction, setBusyThreadAction] = useState(false);

  const {
    loadingTarget,
    hasTarget,
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
    models,
    selectedModel,
    selectedEffort,
    setSelectedModel,
    setSelectedEffort,
    selectThread,
    refreshThreads,
    createChat,
    sendMessage,
    renameThread,
    archiveThread,
    respondApproval,
  } = useMobileShellData();

  const actioningThread = useMemo(
    () => threads.find((thread) => thread.chatId === actioningThreadId) ?? null,
    [actioningThreadId, threads],
  );

  const canCreateChat = hasTarget && status === 'connected' && !creatingChat;
  const canRefreshThreads = hasTarget && status === 'connected' && !loadingThreads;

  return (
    <>
      <AppShell
        section="chat"
        hasTarget={hasTarget}
        loadingTarget={loadingTarget}
        error={error}
        statusBadge={statusBadge}
        renderDrawerActions={({ closeDrawer }) => (
          <>
            <Pressable
              accessibilityRole="button"
              disabled={!canCreateChat}
              onPress={() => {
                void createChat().then(closeDrawer);
              }}
              style={({ pressed }) => [
                styles.actionButton,
                styles.primaryAction,
                {
                  backgroundColor: palette.accent,
                  borderColor: palette.accent,
                  opacity: pressed ? 0.86 : canCreateChat ? 1 : 0.58,
                },
              ]}>
              <Text style={[styles.actionLabel, { color: palette.surface0 }]}> 
                {creatingChat ? 'Creating...' : 'New Chat'}
              </Text>
            </Pressable>
            <Pressable
              accessibilityRole="button"
              disabled={!canRefreshThreads}
              onPress={() => {
                void refreshThreads();
              }}
              style={({ pressed }) => [
                styles.actionButton,
                {
                  backgroundColor: palette.surface1,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : canRefreshThreads ? 1 : 0.58,
                },
              ]}>
              <Text style={[styles.actionLabel, { color: palette.text }]}> 
                {loadingThreads ? 'Refreshing...' : 'Refresh'}
              </Text>
            </Pressable>
          </>
        )}
        renderDrawerContent={({ closeDrawer }) => (
          <ThreadList
            threads={threads}
            activeChatId={activeChatId}
            loading={loadingThreads}
            onSelect={(chatId) => {
              selectThread(chatId);
              closeDrawer();
            }}
            onLongPressThread={(thread) => {
              setActioningThreadId(thread.chatId);
            }}
          />
        )}>
        <View style={styles.chatPane}>
          <ChatTimeline
            thread={hasTarget ? activeThread : null}
            loading={loadingMessages && hasTarget}
            onApprovalDecision={respondApproval}
          />
          <KeyboardStickyView offset={{ closed: 0, opened: -insets.bottom }}>
            <ChatComposer
              disabled={status !== 'connected' || !activeThread || !hasTarget}
              sending={sendingMessage}
              bottomInset={insets.bottom}
              models={models}
              selectedModel={selectedModel}
              selectedEffort={selectedEffort}
              onSelectModel={setSelectedModel}
              onSelectEffort={setSelectedEffort}
              onSend={sendMessage}
            />
          </KeyboardStickyView>
        </View>
      </AppShell>

      <ThreadActionSheet
        thread={actioningThread}
        busy={busyThreadAction}
        onClose={() => {
          if (busyThreadAction) return;
          setActioningThreadId(null);
        }}
        onRename={async (title) => {
          if (!actioningThread) return;
          setBusyThreadAction(true);
          try {
            await renameThread(actioningThread.chatId, title);
            setActioningThreadId(null);
          } finally {
            setBusyThreadAction(false);
          }
        }}
        onArchive={async () => {
          if (!actioningThread) return;
          setBusyThreadAction(true);
          try {
            await archiveThread(actioningThread.chatId);
            setActioningThreadId(null);
          } finally {
            setBusyThreadAction(false);
          }
        }}
      />
    </>
  );
}

const styles = StyleSheet.create({
  chatPane: {
    flex: 1,
    minHeight: 0,
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
});
