import {
  CircleHelp,
  Link2,
  SlidersHorizontal,
  Wifi,
} from 'lucide-react-native';
import { useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import Animated, {
  FadeInRight,
  FadeOutLeft,
  useReducedMotion,
} from 'react-native-reanimated';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { AboutSection } from '@/components/settings/AboutSection';
import { ConnectionSection } from '@/components/settings/ConnectionSection';
import { PreferencesSection } from '@/components/settings/PreferencesSection';
import { ProviderAccountsSection } from '@/components/settings/ProviderAccountsSection';
import {
  SettingsSegmentedControl,
  type SettingsSection,
} from '@/components/settings/SettingsSegmentedControl';
import { useAppTheme } from '@/hooks/useAppTheme';
import { motion } from '@/theme/motion';
import { radius, spacing, touchTarget, typography } from '@/theme/tokens';

const SECTIONS: SettingsSection[] = [
  { key: 'connection', label: 'Connection' },
  { key: 'providers', label: 'Providers' },
  { key: 'preferences', label: 'Preferences' },
  { key: 'about', label: 'About' },
];

export default function SettingsTabScreen() {
  const { width } = useWindowDimensions();
  const { palette, mode } = useAppTheme();
  const reducedMotion = useReducedMotion();
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
    startProviderLogin,
    pollProviderLogin,
    refreshAccountProviders,
  } = useMobileShellData();

  const [activeSection, setActiveSection] = useState('connection');
  // Track previous section for exit direction (not rendered — drives animation only)
  const prevSectionRef = useRef(activeSection);
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

  const handleSectionChange = (key: string) => {
    prevSectionRef.current = activeSection;
    setActiveSection(key);
  };

  const enterAnimation = reducedMotion
    ? undefined
    : FadeInRight.duration(motion.duration.fast).withInitialValues({ opacity: 0, transform: [{ translateX: 16 }] });
  const exitAnimation = reducedMotion
    ? undefined
    : FadeOutLeft.duration(motion.duration.fast).withInitialValues({ opacity: 1, transform: [{ translateX: 0 }] });

  const renderActiveSection = () => {
    switch (activeSection) {
      case 'connection':
        return (
          <ConnectionSection
            status={status}
            loadingTarget={loadingTarget}
            targetUrl={targetUrl}
            targetHint={targetHint}
            hasTarget={hasTarget}
            targetError={targetError}
            savingTarget={savingTarget}
            saveGatewayTarget={saveGatewayTarget}
            clearGatewayTarget={clearGatewayTarget}
            statusBadge={statusBadge}
            error={error}
          />
        );
      case 'providers':
        return (
          <ProviderAccountsSection
            accountProviders={accountProviders}
            startProviderLogin={startProviderLogin}
            pollProviderLogin={pollProviderLogin}
            refreshAccountProviders={refreshAccountProviders}
          />
        );
      case 'preferences':
        return (
          <PreferencesSection
            mode={mode}
            selectedModelLabel={selectedModelLabel}
            availableModelLabels={availableModelLabels}
          />
        );
      case 'about':
        return <AboutSection />;
      default:
        return null;
    }
  };

  const activeSectionDef = SECTIONS.find((s) => s.key === activeSection);

  const sidebarNav = (
    <View
      accessible
      accessibilityRole="tablist"
      accessibilityLabel="Settings sections"
      style={[styles.sidebarNav, { borderColor: palette.border, backgroundColor: palette.surface0 }]}
    >
      {SECTIONS.map((section) => {
        const isActive = section.key === activeSection;
        return (
          <Pressable
            key={section.key}
            accessibilityRole="tab"
            accessibilityState={{ selected: isActive }}
            accessibilityLabel={`${section.label} tab`}
            onPress={() => handleSectionChange(section.key)}
            style={({ pressed }) => [
              styles.sidebarItem,
              isActive && { backgroundColor: palette.accentDim },
              pressed && !isActive && { opacity: 0.7 },
            ]}
          >
            <Text
              style={[
                styles.sidebarLabel,
                { color: isActive ? palette.accent : palette.textSecondary },
              ]}
            >
              {section.label}
            </Text>
          </Pressable>
        );
      })}
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
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Go to Connection section"
            onPress={() => handleSectionChange('connection')}
            style={styles.drawerItem}
          >
            <Wifi size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Connection Status</Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Go to Connection section"
            onPress={() => handleSectionChange('connection')}
            style={styles.drawerItem}
          >
            <Link2 size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Gateway Target</Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Go to Preferences section"
            onPress={() => handleSectionChange('preferences')}
            style={styles.drawerItem}
          >
            <SlidersHorizontal size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Preferences</Text>
          </Pressable>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Go to About section"
            onPress={() => handleSectionChange('about')}
            style={styles.drawerItem}
          >
            <CircleHelp size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>About</Text>
          </Pressable>
          <Text style={[styles.drawerNote, { color: palette.textSecondary }]}>
            Configure once, then switch targets anytime from this screen.
          </Text>
        </View>
      )}
    >
      {wideLayout ? (
        <View style={styles.columns}>
          {sidebarNav}
          <View
            accessible
            accessibilityRole="summary"
            accessibilityLabel={`${activeSectionDef?.label ?? 'Settings'} section content`}
            style={styles.contentPane}
          >
            {renderActiveSection()}
          </View>
        </View>
      ) : (
        <View style={styles.narrowLayout}>
          <SettingsSegmentedControl
            sections={SECTIONS}
            activeSection={activeSection}
            onSectionChange={handleSectionChange}
          />
          <Animated.View
            key={activeSection}
            entering={enterAnimation}
            exiting={exitAnimation}
            accessible
            accessibilityRole="summary"
            accessibilityLabel={`${activeSectionDef?.label ?? 'Settings'} section content`}
            style={styles.sectionContent}
          >
            {renderActiveSection()}
          </Animated.View>
        </View>
      )}
    </AppShell>
  );
}

const styles = StyleSheet.create({
  narrowLayout: {
    flex: 1,
  },
  sectionContent: {
    flex: 1,
    padding: spacing.md,
  },
  columns: {
    flex: 1,
    flexDirection: 'row',
    gap: spacing.lg,
  },
  sidebarNav: {
    alignSelf: 'flex-start',
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.xs,
    margin: spacing.md,
    padding: spacing.sm,
    width: 200,
  },
  sidebarItem: {
    borderRadius: radius.md,
    minHeight: touchTarget.min,
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  sidebarLabel: {
    ...typography.bodyMedium,
    fontSize: 14,
  },
  contentPane: {
    flex: 1,
    paddingRight: spacing.md,
    paddingVertical: spacing.md,
  },
  drawerCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.sm,
    marginHorizontal: spacing.md,
    marginTop: spacing.sm,
    padding: spacing.md,
  },
  drawerEyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  drawerItem: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: touchTarget.min,
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
