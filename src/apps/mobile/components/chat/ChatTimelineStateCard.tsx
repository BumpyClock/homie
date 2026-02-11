import { Feather } from '@expo/vector-icons';
import { Pressable, Text, View } from 'react-native';

import type { AppPalette } from '@/theme/tokens';

import { colorsForTone, type StatusTone } from './chat-timeline-helpers';
import { styles } from './chat-timeline-styles';

interface ChatTimelineStateCardProps {
  icon: keyof typeof Feather.glyphMap;
  title: string;
  body: string;
  palette: AppPalette;
  tone?: StatusTone;
  actionLabel?: string;
  onAction?: () => void;
}

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

  return (
    <View style={[styles.stateCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
      <Feather name={icon} size={28} color={toneColors.foreground} />
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
