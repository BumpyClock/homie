import { StyleSheet } from 'react-native';

import { elevation, palettes, radius, spacing, touchTarget, typography } from '@/theme/tokens';

export const styles = StyleSheet.create({
  container: {
    flex: 1,
    overflow: 'hidden',
  },
  header: {
    alignItems: 'center',
    borderBottomWidth: 1,
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: 64,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  headerMain: {
    flex: 1,
    gap: spacing.micro,
  },
  headerTitle: {
    ...typography.label,
    fontSize: 14,
  },
  headerMeta: {
    ...typography.caption,
    fontSize: 12,
  },
  headerPills: {
    alignItems: 'flex-end',
    gap: spacing.xs,
  },
  headerPill: {
    borderRadius: radius.pill,
    borderWidth: 1,
    minHeight: 24,
    justifyContent: 'center',
    paddingHorizontal: spacing.sm,
  },
  headerPillLabel: {
    ...typography.label,
    fontSize: 10,
    textTransform: 'uppercase',
  },
  errorBanner: {
    alignItems: 'center',
    borderBottomWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
  },
  errorText: {
    ...typography.body,
    flex: 1,
    fontSize: 13,
  },
  listArea: {
    flex: 1,
  },
  listContent: {
    paddingHorizontal: spacing.xs,
    paddingVertical: spacing.sm,
  },
  turnGroup: {
    gap: 0,
  },
  turnDivider: {
    height: 1,
    marginHorizontal: spacing.md,
    marginVertical: spacing.sm,
    opacity: 0.5,
  },
  messageRow: {
    alignItems: 'flex-start',
    borderRadius: radius.sm,
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: touchTarget.min,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
  },
  avatar: {
    alignItems: 'center',
    borderRadius: 14,
    height: 28,
    justifyContent: 'center',
    marginTop: 2,
    width: 28,
  },
  avatarText: {
    color: palettes.light.surface0,
    fontSize: 12,
    fontWeight: '600',
  },
  messageContent: {
    flex: 1,
    gap: spacing.xs,
    paddingBottom: spacing.xs,
  },
  senderName: {
    ...typography.label,
    fontSize: 13,
  },
  messageBody: {
    ...typography.body,
    fontSize: 14,
  },
  messageActions: {
    borderTopWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    marginTop: spacing.xs,
    paddingTop: spacing.xs,
  },
  messageActionButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    justifyContent: 'center',
    minHeight: touchTarget.min,
    minWidth: touchTarget.min,
    paddingHorizontal: spacing.sm,
  },
  messageActionLabel: {
    ...typography.label,
    fontSize: 12,
  },
  approvalCard: {
    borderRadius: radius.sm,
    borderWidth: 1,
    gap: spacing.sm,
    marginLeft: 36,
    padding: spacing.md,
  },
  approvalHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  approvalTitleRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.xs,
  },
  approvalTitle: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
  approvalStatus: {
    ...typography.caption,
    fontSize: 12,
  },
  commandCard: {
    borderRadius: radius.sm,
    borderWidth: 1,
    padding: spacing.sm,
  },
  commandText: {
    ...typography.monoSmall,
    fontSize: 12,
  },
  approvalActions: {
    flexDirection: 'row',
    gap: spacing.xs,
  },
  approvalButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    flex: 1,
    justifyContent: 'center',
    minHeight: touchTarget.min,
    paddingHorizontal: spacing.xs,
  },
  approvalLabel: {
    ...typography.label,
    fontSize: 12,
  },
  loadingWrap: {
    flex: 1,
    justifyContent: 'center',
    padding: spacing.lg,
  },
  emptyContainer: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    padding: spacing.xl,
  },
  stateCard: {
    alignItems: 'center',
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.xs,
    maxWidth: 420,
    minWidth: 260,
    padding: spacing.xl,
  },
  stateTitle: {
    ...typography.title,
    fontSize: 18,
    textAlign: 'center',
  },
  stateBody: {
    ...typography.body,
    textAlign: 'center',
  },
  stateAction: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    marginTop: spacing.xs,
    minHeight: touchTarget.min,
    minWidth: 160,
    paddingHorizontal: spacing.md,
  },
  stateActionLabel: {
    ...typography.label,
    fontSize: 13,
  },
  emptyListWrap: {
    alignItems: 'center',
    padding: spacing.lg,
  },
  emptyListLabel: {
    ...typography.body,
  },
  jumpButtonWrap: {
    bottom: spacing.md,
    position: 'absolute',
    right: spacing.md,
  },
  jumpButton: {
    alignItems: 'center',
    borderRadius: touchTarget.min / 2,
    borderWidth: 1,
    height: touchTarget.min,
    justifyContent: 'center',
    width: touchTarget.min,
    ...elevation.fab,
  },
});
