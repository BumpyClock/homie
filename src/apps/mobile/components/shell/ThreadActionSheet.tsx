import { type ChatThreadSummary } from '@homie/shared';
import { useEffect, useState } from 'react';
import { Modal, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ThreadActionSheetProps {
  thread: ChatThreadSummary | null;
  busy: boolean;
  onClose: () => void;
  onRename: (title: string) => Promise<void>;
  onArchive: () => Promise<void>;
}

export function ThreadActionSheet({
  thread,
  busy,
  onClose,
  onRename,
  onArchive,
}: ThreadActionSheetProps) {
  const { palette } = useAppTheme();
  const [title, setTitle] = useState('');
  const [confirmArchive, setConfirmArchive] = useState(false);

  useEffect(() => {
    setTitle(thread?.title ?? '');
    setConfirmArchive(false);
  }, [thread?.chatId, thread?.title]);

  if (!thread) return null;

  return (
    <Modal
      animationType="fade"
      transparent
      visible
      onRequestClose={onClose}>
      <View style={styles.overlay}>
        <Pressable style={StyleSheet.absoluteFillObject} onPress={onClose} />
        <View style={[styles.sheet, { backgroundColor: palette.surface, borderColor: palette.border }]}>
          <Text style={[styles.title, { color: palette.text }]}>Thread Actions</Text>
          <Text style={[styles.subtitle, { color: palette.textSecondary }]}>
            {thread.title}
          </Text>
          <TextInput
            value={title}
            onChangeText={setTitle}
            editable={!busy}
            placeholder="Rename thread"
            placeholderTextColor={palette.textSecondary}
            style={[
              styles.input,
              { color: palette.text, borderColor: palette.border, backgroundColor: palette.surfaceAlt },
            ]}
          />
          <View style={styles.row}>
            <Pressable
              accessibilityRole="button"
              disabled={busy || !title.trim()}
              onPress={() => {
                setConfirmArchive(false);
                void onRename(title.trim());
              }}
              style={({ pressed }) => [
                styles.button,
                {
                  backgroundColor: palette.accent,
                  borderColor: palette.accent,
                  opacity: pressed ? 0.86 : busy || !title.trim() ? 0.58 : 1,
                },
              ]}>
              <Text style={[styles.buttonLabel, { color: palette.surface }]}>
                {busy ? 'Saving...' : 'Rename'}
              </Text>
            </Pressable>
            <Pressable
              accessibilityRole="button"
              disabled={busy}
              onPress={() => {
                if (!confirmArchive) {
                  setConfirmArchive(true);
                  return;
                }
                void onArchive();
              }}
              style={({ pressed }) => [
                styles.button,
                {
                  backgroundColor: confirmArchive ? palette.danger : palette.surface,
                  borderColor: palette.danger,
                  opacity: pressed ? 0.86 : busy ? 0.58 : 1,
                },
              ]}>
              <Text style={[styles.buttonLabel, { color: confirmArchive ? palette.surface : palette.danger }]}>
                {confirmArchive ? 'Confirm Archive' : 'Archive'}
              </Text>
            </Pressable>
          </View>
          {confirmArchive ? (
            <Text style={[styles.confirmText, { color: palette.danger }]}>
              Tap archive again to confirm permanent removal from the list.
            </Text>
          ) : null}
          <Pressable
            accessibilityRole="button"
            disabled={busy}
            onPress={onClose}
            style={({ pressed }) => [
              styles.cancelButton,
              {
                backgroundColor: palette.surfaceAlt,
                borderColor: palette.border,
                opacity: pressed ? 0.86 : busy ? 0.58 : 1,
              },
            ]}>
            <Text style={[styles.buttonLabel, { color: palette.text }]}>Cancel</Text>
          </Pressable>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  overlay: {
    alignItems: 'center',
    backgroundColor: 'rgba(8, 12, 18, 0.34)',
    flex: 1,
    justifyContent: 'flex-end',
    padding: spacing.lg,
  },
  sheet: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.sm,
    maxWidth: 420,
    padding: spacing.lg,
    width: '100%',
  },
  title: {
    ...typography.title,
    fontSize: 18,
  },
  subtitle: {
    ...typography.body,
    fontSize: 13,
    fontWeight: '400',
  },
  input: {
    ...typography.body,
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    paddingHorizontal: spacing.sm,
  },
  row: {
    flexDirection: 'row',
    gap: spacing.sm,
  },
  button: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    flex: 1,
    justifyContent: 'center',
    minHeight: 44,
  },
  cancelButton: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 44,
  },
  buttonLabel: {
    ...typography.label,
    fontSize: 13,
  },
  confirmText: {
    ...typography.data,
    fontSize: 12,
  },
});
