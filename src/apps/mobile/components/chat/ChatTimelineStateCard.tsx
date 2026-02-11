import {
  CircleAlert,
  Clock3,
  Link2,
  LoaderCircle,
  MessageCircle,
  type LucideIcon,
  WifiOff,
} from 'lucide-react-native';
import { Pressable, Text, View } from 'react-native';

import type { AppPalette } from '@/theme/tokens';

import { colorsForTone, type StatusTone } from './chat-timeline-helpers';
import { styles } from './chat-timeline-styles';

interface ChatTimelineStateCardProps {
  icon: LucideIcon | string;
  title: string;
  body: string;
  palette: AppPalette;
  tone?: StatusTone;
  actionLabel?: string;
  onAction?: () => void;
}

const ICON_NAME_MAP: Record<string, LucideIcon> = {
  'link-2': Link2,
  loader: LoaderCircle,
  'wifi-off': WifiOff,
  'message-circle': MessageCircle,
  clock: Clock3,
};

export function ChatTimelineStateCard({
  icon,
  title,
  body,
  palette,
  tone = 'accent',
  actionLabel,
  onAction,
}: ChatTimelineStateCardProps) {
  const toneColors = colorsForTone(palette, tone);
  const Icon = typeof icon === 'string' ? ICON_NAME_MAP[icon] ?? CircleAlert : icon;

  return (
    <View style={[styles.stateCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
      <Icon size={28} color={toneColors.foreground} />
      <Text style={[styles.stateTitle, { color: palette.text }]}>{title}</Text>
      <Text style={[styles.stateBody, { color: palette.textSecondary }]}>{body}</Text>
      {actionLabel && onAction ? (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={actionLabel}
          onPress={onAction}
          style={({ pressed }) => [
            styles.stateAction,
            {
              backgroundColor: palette.surface1,
              borderColor: palette.border,
              opacity: pressed ? 0.84 : 1,
            },
          ]}>
          <Text style={[styles.stateActionLabel, { color: palette.text }]}>{actionLabel}</Text>
        </Pressable>
      ) : null}
    </View>
  );
}
