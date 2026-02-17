import { useCallback, useMemo, useState } from 'react';
import { KeyboardStickyView } from 'react-native-keyboard-controller';
import { StyleSheet, View, useWindowDimensions } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatTimeline } from '@/components/chat/ChatTimeline';
import { ThreadList } from '@/components/chat/ThreadList';
import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { ThreadActionSheet } from '@/components/shell/ThreadActionSheet';
import { ActionButton } from '@/components/ui/ActionButton';

export default function ChatTabScreen() {
  const { width } = useWindowDimensions();
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
    skills,
    collaborationModes,
    selectedModel,
    selectedEffort,
    selectedPermission,
    selectedCollaborationMode,
    setSelectedModel,
    setSelectedEffort,
    setSelectedPermission,
    setSelectedCollaborationMode,
    selectThread,
    refreshThreads,
    createChat,
    sendMessage,
    renameThread,
    archiveThread,
    respondApproval,
    stopChat,
    activePendingApprovalCount,
    queuedMessage,
    clearQueuedMessage,
  } = useMobileShellData();

  const actioningThread = useMemo(
    () => threads.find((thread) => thread.chatId === actioningThreadId) ?? null,
    [actioningThreadId, threads],
  );
  const getApprovalCount = useCallback(
    (chatId: string) => (chatId === activeChatId ? activePendingApprovalCount : 0),
    [activeChatId, activePendingApprovalCount],
  );

  const canCreateChat = hasTarget && status === 'connected' && !creatingChat;
  const canRefreshThreads = hasTarget && status === 'connected' && !loadingThreads;
  const wideChatLayout = width >= 1100;

  return (
    <>
      <AppShell
        section="chat"
        topBarTitle={hasTarget ? activeThread?.title ?? 'Chat' : 'Chat'}
        hasTarget={hasTarget}
        loadingTarget={loadingTarget}
        error={error}
        statusBadge={statusBadge}
        renderDrawerActions={({ closeDrawer }) => (
          <>
            <ActionButton
              disabled={!canCreateChat}
              flex
              label={creatingChat ? 'Creating...' : 'New Chat'}
              onPress={() => {
                void createChat().then(closeDrawer);
              }}
              variant="primary"
            />
            <ActionButton
              disabled={!canRefreshThreads}
              label={loadingThreads ? 'Refreshing...' : 'Refresh'}
              onPress={() => {
                void refreshThreads();
              }}
            />
          </>
        )}
        renderDrawerContent={({ closeDrawer }) => (
          <ThreadList
            threads={threads}
            activeChatId={activeChatId}
            loading={loadingThreads}
            getApprovalCount={getApprovalCount}
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
          <View style={[styles.chatFrame, wideChatLayout ? styles.chatFrameWide : null]}>
            <ChatTimeline
              thread={hasTarget ? activeThread : null}
              loading={loadingMessages && hasTarget}
              status={status}
              hasTarget={hasTarget}
              error={error}
              onRetry={() => {
                void refreshThreads();
              }}
              onApprovalDecision={respondApproval}
            />
            <KeyboardStickyView offset={{ closed: 0, opened: -insets.bottom }}>
              <ChatComposer
                disabled={status !== 'connected' || !activeThread || !hasTarget}
                sending={sendingMessage}
                isRunning={activeThread?.running ?? false}
                bottomInset={insets.bottom}
                models={models}
                skills={skills}
                collaborationModes={collaborationModes}
                selectedModel={selectedModel}
                selectedEffort={selectedEffort}
                selectedPermission={selectedPermission}
                selectedCollaborationMode={selectedCollaborationMode}
                onSelectModel={setSelectedModel}
                onSelectEffort={setSelectedEffort}
                onSelectPermission={setSelectedPermission}
                onSelectCollaborationMode={setSelectedCollaborationMode}
                queuedMessage={queuedMessage}
                onClearQueue={clearQueuedMessage}
                onSend={sendMessage}
                onStop={stopChat}
              />
            </KeyboardStickyView>
          </View>
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
  chatFrame: {
    flex: 1,
    minHeight: 0,
    width: '100%',
  },
  chatFrameWide: {
    alignSelf: 'center',
    maxWidth: 980,
  },
});
