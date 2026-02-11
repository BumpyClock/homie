import {
  CircleHelp,
  Link2,
  Server,
  SlidersHorizontal,
  Wifi,
} from 'lucide-react-native';
import { useMemo, useState } from 'react';
import { Linking, Pressable, ScrollView, StyleSheet, Text, View, useWindowDimensions } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { LabeledValueRow } from '@/components/ui/LabeledValueRow';
import { StatusPill } from '@/components/ui/StatusPill';
import { runtimeConfig } from '@/config/runtime';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

function statusDetails(status: string, hasTarget: boolean, loadingTarget: boolean): string {
  if (loadingTarget) return 'Loading saved gateway target from device storage.';
  if (!hasTarget) return 'No gateway target set yet. Add one below to start syncing.';
  if (status === 'connected') return 'Live connection is healthy. Chat and terminals are ready.';
  if (status === 'connecting' || status === 'handshaking') return 'Connecting to gateway now. This should settle in a moment.';
  if (status === 'rejected') return 'Gateway rejected the session. Confirm auth and URL path.';
  if (status === 'error') return 'Connection error from gateway. Check URL, network, and certificates.';
  return 'Disconnected from gateway. Reconnect by confirming target URL.';
}

export default function SettingsTabScreen() {
  const { width } = useWindowDimensions();
  const { palette, mode } = useAppTheme();
  const {
    status,
    loadingTarget,
    targetUrl,
    targetHint,
    hasTarget,
    targetError,
    savingTarget,
    saveGatewayTarget,
    clearGatewayTarget,
    statusBadge,
    error,
    models,
    selectedModel,
    accountProviders,
    refreshAccountProviders,
    startProviderLogin,
    pollProviderLogin,
  } = useMobileShellData();
  const [authBusyByProvider, setAuthBusyByProvider] = useState<Record<string, boolean>>({});
  const [authSessionByProvider, setAuthSessionByProvider] = useState<
    Record<string, { verificationUrl: string; userCode: string } | undefined>
  >({});
  const [authErrorByProvider, setAuthErrorByProvider] = useState<Record<string, string | undefined>>({});
  const activeTarget = targetUrl ?? targetHint ?? 'Not set';
  const wideLayout = width >= 1080;
  const selectedModelOption =
    models.find((model) => model.model === selectedModel || model.id === selectedModel) ??
    models.find((model) => model.isDefault) ??
    models[0] ??
    null;
  const selectedModelLabel =
    selectedModelOption?.displayName || selectedModelOption?.model || selectedModelOption?.id || 'Loading...';
  const availableModelLabels = models
    .map((model) => model.displayName || model.model || model.id)
    .filter((label): label is string => Boolean(label && label.trim()));
  const providerLabels = useMemo<Record<string, string>>(
    () => ({
      'openai-codex': 'OpenAI Codex',
      'github-copilot': 'GitHub Copilot',
      'claude-code': 'Claude Code',
    }),
    [],
  );

  const connectProvider = async (provider: string) => {
    setAuthErrorByProvider((prev) => ({ ...prev, [provider]: undefined }));
    setAuthBusyByProvider((prev) => ({ ...prev, [provider]: true }));
    try {
      const session = await startProviderLogin(provider);
      setAuthSessionByProvider((prev) => ({
        ...prev,
        [provider]: {
          verificationUrl: session.verificationUrl,
          userCode: session.userCode,
        },
      }));
      let delaySeconds = Math.max(1, session.intervalSecs || 5);
      for (let i = 0; i < 120; i += 1) {
        await new Promise((resolve) => setTimeout(resolve, delaySeconds * 1000));
        const result = await pollProviderLogin(provider, session);
        if (result.status === 'authorized') {
          setAuthSessionByProvider((prev) => ({ ...prev, [provider]: undefined }));
          await refreshAccountProviders();
          return;
        }
        if (result.status === 'pending' || result.status === 'slow_down') {
          delaySeconds = Math.max(1, result.intervalSecs ?? delaySeconds);
          continue;
        }
        setAuthErrorByProvider((prev) => ({
          ...prev,
          [provider]: result.status === 'denied' ? 'Access denied.' : 'Device code expired.',
        }));
        return;
      }
      setAuthErrorByProvider((prev) => ({ ...prev, [provider]: 'Authorization timed out.' }));
    } catch (nextError) {
      const message = nextError instanceof Error ? nextError.message : 'Login failed.';
      setAuthErrorByProvider((prev) => ({ ...prev, [provider]: message }));
    } finally {
      setAuthBusyByProvider((prev) => ({ ...prev, [provider]: false }));
    }
  };

  const connectionCard = (
    <View style={[styles.heroCard, { backgroundColor: palette.surface1, borderColor: palette.border }]}> 
      <View style={styles.heroHeader}>
        <Text style={[styles.heroEyebrow, { color: palette.textSecondary }]}>Connection</Text>
        <StatusPill compact label={statusBadge.label} tone={statusBadge.tone} />
      </View>
      <Text style={[styles.heroTitle, { color: palette.text }]}>Gateway Status</Text>
      <Text style={[styles.heroBody, { color: palette.textSecondary }]}>
        {statusDetails(status, hasTarget, loadingTarget)}
      </Text>
      <LabeledValueRow label="Current target" value={activeTarget} mono />
      <LabeledValueRow label="Transport state" value={status} mono last />
      {targetError ? (
        <View style={[styles.inlineAlert, { backgroundColor: palette.dangerDim, borderColor: palette.danger }]}> 
          <Text style={[styles.inlineAlertText, { color: palette.danger }]}>{targetError}</Text>
        </View>
      ) : null}
    </View>
  );

  const targetCard = (
    <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
      <View style={styles.cardHeader}>
        <Server size={14} color={palette.accent} />
        <Text style={[styles.cardTitle, { color: palette.text }]}>Gateway Target</Text>
      </View>
      <Text style={[styles.cardBody, { color: palette.textSecondary }]}>
        Set the URL used by mobile chat + terminal sessions.
      </Text>
      <GatewayTargetForm
        initialValue={targetUrl ?? targetHint}
        hintValue={targetHint}
        saving={savingTarget || loadingTarget}
        saveLabel={hasTarget ? 'Update Target' : 'Save Target'}
        onSave={saveGatewayTarget}
        onClear={hasTarget ? clearGatewayTarget : undefined}
      />
      {!targetHint ? (
        <Text style={[styles.meta, { color: palette.textSecondary }]}>No env fallback. Set target manually.</Text>
      ) : (
        <Text style={[styles.meta, { color: palette.textSecondary }]}>Env fallback available: {runtimeConfig.gatewayUrl}</Text>
      )}
    </View>
  );

  const defaultsCard = (
    <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
      <View style={styles.cardHeader}>
        <SlidersHorizontal size={14} color={palette.accent} />
        <Text style={[styles.cardTitle, { color: palette.text }]}>App Defaults & Help</Text>
      </View>
      <LabeledValueRow label="Theme" value={mode} />
      <LabeledValueRow label="Selected model" value={selectedModelLabel} />
      <LabeledValueRow
        label="Available models"
        value={availableModelLabels.length > 0 ? `${availableModelLabels.length}` : 'Loading...'}
      />
      {availableModelLabels.length > 0 ? (
        <View style={[styles.modelList, { borderColor: palette.border, backgroundColor: palette.surface1 }]}>
          {availableModelLabels.map((label) => (
            <Text key={label} numberOfLines={1} style={[styles.modelListItem, { color: palette.textSecondary }]}>
              {`\u2022 ${label}`}
            </Text>
          ))}
        </View>
      ) : null}
      <LabeledValueRow label="Gateway path" value="/ws" mono last />
      <View style={[styles.authCard, { borderColor: palette.border, backgroundColor: palette.surface1 }]}>
        <Text style={[styles.authTitle, { color: palette.text }]}>Provider Authentication</Text>
        {accountProviders.filter((provider) => provider.enabled).map((provider) => {
          const busy = !!authBusyByProvider[provider.id];
          const session = authSessionByProvider[provider.id];
          const authError = authErrorByProvider[provider.id];
          return (
            <View key={provider.id} style={[styles.authRow, { borderColor: palette.border }]}>
              <View style={styles.authHeader}>
                <Text style={[styles.authProviderName, { color: palette.text }]}>
                  {providerLabels[provider.id] ?? provider.id}
                </Text>
                {provider.loggedIn ? (
                  <StatusPill compact label="Connected" tone="success" />
                ) : (
                  <Pressable
                    disabled={busy}
                    onPress={() => {
                      void connectProvider(provider.id);
                    }}
                    style={({ pressed }) => [
                      styles.authButton,
                      {
                        borderColor: palette.border,
                        backgroundColor: pressed ? palette.surface2 : palette.surface0,
                        opacity: busy ? 0.7 : 1,
                      },
                    ]}>
                    <Text style={[styles.authButtonText, { color: palette.text }]}>
                      {busy ? 'Connecting...' : 'Connect'}
                    </Text>
                  </Pressable>
                )}
              </View>
              {!provider.loggedIn && session ? (
                <View style={styles.authSession}>
                  <Text style={[styles.authHint, { color: palette.textSecondary }]}>
                    Open verification URL and enter code:
                  </Text>
                  <Pressable onPress={() => { void Linking.openURL(session.verificationUrl); }}>
                    <Text style={[styles.authLink, { color: palette.accent }]}>{session.verificationUrl}</Text>
                  </Pressable>
                  <Text style={[styles.authCode, { color: palette.text }]}>{session.userCode}</Text>
                </View>
              ) : null}
              {authError ? (
                <Text style={[styles.authError, { color: palette.danger }]}>{authError}</Text>
              ) : null}
            </View>
          );
        })}
      </View>
      <View style={[styles.helpCard, { backgroundColor: palette.surface1, borderColor: palette.border }]}> 
        <Text style={[styles.helpTitle, { color: palette.text }]}>Tips</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Prefer `wss://` for remote access.</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Include `/ws` at the end of your gateway URL.</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Update target here any time to switch machines.</Text>
      </View>
    </View>
  );

  return (
    <AppShell
      section="settings"
      hasTarget={hasTarget}
      loadingTarget={loadingTarget}
      error={error}
      statusBadge={statusBadge}
      renderDrawerContent={() => (
        <View style={[styles.drawerCard, { borderColor: palette.border, backgroundColor: palette.surface1 }]}> 
          <Text style={[styles.drawerEyebrow, { color: palette.textSecondary }]}>Quick Links</Text>
          <View style={styles.drawerItem}>
            <Wifi size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Connection Status</Text>
          </View>
          <View style={styles.drawerItem}>
            <Link2 size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Gateway Target</Text>
          </View>
          <View style={styles.drawerItem}>
            <CircleHelp size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Defaults & Help</Text>
          </View>
          <Text style={[styles.drawerNote, { color: palette.textSecondary }]}>Configure once, then switch targets anytime from this screen.</Text>
        </View>
      )}>
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        showsVerticalScrollIndicator={false}>
        {wideLayout ? (
          <View style={styles.columns}>
            <View style={styles.primaryColumn}>
              {connectionCard}
              {targetCard}
            </View>
            <View style={styles.secondaryColumn}>{defaultsCard}</View>
          </View>
        ) : (
          <>
            {connectionCard}
            {targetCard}
            {defaultsCard}
          </>
        )}
      </ScrollView>
    </AppShell>
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
  columns: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: spacing.lg,
  },
  primaryColumn: {
    flex: 1.4,
    gap: spacing.md,
    minWidth: 0,
  },
  secondaryColumn: {
    flex: 1,
    minWidth: 0,
  },
  heroCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.sm,
  },
  heroHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  heroEyebrow: {
    ...typography.overline,
    textTransform: 'uppercase',
  },
  heroTitle: {
    ...typography.heading,
    fontSize: 20,
  },
  heroBody: {
    ...typography.body,
    fontSize: 14,
  },
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.md,
  },
  cardHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  cardTitle: {
    ...typography.title,
    fontSize: 18,
  },
  cardBody: {
    ...typography.body,
    fontSize: 14,
  },
  inlineAlert: {
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  inlineAlertText: {
    ...typography.caption,
    fontWeight: '600',
  },
  meta: {
    ...typography.monoSmall,
    fontSize: 12,
  },
  helpCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    padding: spacing.md,
    gap: spacing.xs,
  },
  helpTitle: {
    ...typography.caption,
    textTransform: 'uppercase',
  },
  helpItem: {
    ...typography.caption,
    fontSize: 12,
    fontWeight: '500',
  },
  modelList: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  modelListItem: {
    ...typography.data,
    fontSize: 12,
  },
  authCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.sm,
    padding: spacing.md,
  },
  authTitle: {
    ...typography.caption,
    textTransform: 'uppercase',
  },
  authRow: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    padding: spacing.sm,
  },
  authHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  authProviderName: {
    ...typography.bodyMedium,
    fontSize: 13,
  },
  authButton: {
    borderRadius: radius.sm,
    borderWidth: 1,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
  },
  authButtonText: {
    ...typography.caption,
    fontWeight: '600',
  },
  authSession: {
    gap: spacing.xs,
  },
  authHint: {
    ...typography.caption,
    fontSize: 12,
  },
  authLink: {
    ...typography.caption,
    fontSize: 12,
    textDecorationLine: 'underline',
  },
  authCode: {
    ...typography.data,
    fontSize: 13,
  },
  authError: {
    ...typography.caption,
    fontSize: 12,
    fontWeight: '600',
  },
  drawerCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    marginHorizontal: spacing.md,
    marginTop: spacing.sm,
    padding: spacing.md,
    gap: spacing.sm,
  },
  drawerEyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  drawerItem: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  drawerItemLabel: {
    ...typography.bodyMedium,
    fontSize: 14,
  },
  drawerNote: {
    ...typography.caption,
    fontSize: 12,
  },
});
