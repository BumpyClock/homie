import { ScrollView, StyleSheet, Text, View } from 'react-native';

import {
  type ChatAccountProviderStatus,
  type ChatDeviceCodePollResult,
  type ChatDeviceCodeSession,
  modelProviderLabel,
  useProviderAuth,
} from '@homie/shared';

import { ProviderRow } from '@/components/settings/ProviderRow';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ProviderAccountsSectionProps {
  accountProviders: ChatAccountProviderStatus[];
  startProviderLogin: (provider: string, profile?: string) => Promise<ChatDeviceCodeSession>;
  pollProviderLogin: (
    provider: string,
    session: ChatDeviceCodeSession,
    profile?: string,
  ) => Promise<ChatDeviceCodePollResult>;
  refreshAccountProviders: () => Promise<void>;
}

export function ProviderAccountsSection({
  accountProviders,
  startProviderLogin,
  pollProviderLogin,
  refreshAccountProviders,
}: ProviderAccountsSectionProps) {
  const { palette } = useAppTheme();
  const enabledProviders = accountProviders.filter((p) => p.enabled);

  const { authStates, connect, cancel } = useProviderAuth({
    startLogin: startProviderLogin,
    pollLogin: pollProviderLogin,
    onAuthorized: refreshAccountProviders,
  });

  if (enabledProviders.length === 0) {
    return (
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        showsVerticalScrollIndicator={false}
      >
        <View style={[styles.emptyCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
          <Text style={[styles.emptyTitle, { color: palette.text }]}>No Providers</Text>
          <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
            No account providers are enabled on your gateway. Configure providers in your gateway
            settings to connect here.
          </Text>
        </View>
      </ScrollView>
    );
  }

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={styles.scrollContent}
      showsVerticalScrollIndicator={false}
    >
      <Text style={[styles.sectionEyebrow, { color: palette.textSecondary }]}>
        Provider Accounts
      </Text>
      {enabledProviders.map((provider) => (
        <ProviderRow
          key={provider.id}
          provider={provider}
          providerLabel={modelProviderLabel({ model: '', provider: provider.key })}
          authState={authStates[provider.id]}
          onConnect={connect}
          onCancel={cancel}
        />
      ))}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: {
    flex: 1,
  },
  scrollContent: {
    gap: spacing.md,
    paddingBottom: spacing.xxxl,
  },
  sectionEyebrow: {
    ...typography.overline,
    textTransform: 'uppercase',
  },
  emptyCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.sm,
    padding: spacing.lg,
  },
  emptyTitle: {
    ...typography.title,
    fontSize: 18,
  },
  emptyBody: {
    ...typography.body,
    fontSize: 14,
  },
});
