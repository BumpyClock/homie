import type { ChatItem, ConnectionStatus } from '@homie/shared';

import type { AppPalette } from '@/theme/tokens';

export type StatusTone = 'accent' | 'success' | 'warning';

export function bodyForItem(item: ChatItem): string {
  if (item.kind === 'reasoning') {
    const detail = [...(item.summary ?? []), ...(item.content ?? [])].join('\n');
    return detail || 'Reasoning update';
  }
  if (item.kind === 'command') {
    const command = item.command ? `$ ${item.command}` : '';
    const output = item.output ?? '';
    return [command, output].filter(Boolean).join('\n');
  }
  if (item.kind === 'file') {
    if (!item.changes || item.changes.length === 0) return 'File changes';
    return item.changes.map((change) => change.path).join('\n');
  }
  if (item.kind === 'approval') {
    if (item.command) return `Approval required: ${item.command}`;
    return 'Approval required';
  }
  if (item.kind === 'tool') return item.text || 'Tool call';
  return item.text || '';
}

export function labelForItem(item: ChatItem): string {
  if (item.kind === 'user') return 'You';
  if (item.kind === 'assistant') return 'Gateway';
  if (item.kind === 'plan') return 'Plan';
  if (item.kind === 'reasoning') return 'Reasoning';
  if (item.kind === 'command') return 'Command';
  if (item.kind === 'file') return 'Files';
  if (item.kind === 'approval') return 'Approval';
  if (item.kind === 'diff') return 'Diff';
  if (item.kind === 'tool') return 'Tool';
  return 'System';
}

export function approvalStatusLabel(status: string): string {
  if (status === 'accept' || status === 'accept_for_session') return 'Accepted';
  if (status === 'decline' || status === 'cancel') return 'Declined';
  return 'Pending';
}

export function avatarInitial(item: ChatItem): string {
  if (item.kind === 'user') return 'Y';
  return 'G';
}

export function statusForConnection(status: ConnectionStatus): { label: string; tone: StatusTone } {
  if (status === 'connected') return { label: 'Connected', tone: 'success' };
  if (status === 'connecting' || status === 'handshaking') {
    return { label: 'Connecting', tone: 'accent' };
  }
  if (status === 'rejected') return { label: 'Rejected', tone: 'warning' };
  if (status === 'error') return { label: 'Error', tone: 'warning' };
  return { label: 'Disconnected', tone: 'warning' };
}

export function colorsForTone(palette: AppPalette, tone: StatusTone) {
  if (tone === 'success') {
    return { foreground: palette.success, background: palette.successDim };
  }
  if (tone === 'warning') {
    return { foreground: palette.warning, background: palette.warningDim };
  }
  return { foreground: palette.accent, background: palette.accentDim };
}
