// ABOUTME: Structured detail view for web-related tool calls (browser, web_search, web_fetch).
// ABOUTME: Shows tool-specific fields like URL, query, and truncated output in a readable card format.
// ABOUTME: Handles URL truncation, output capping (720 chars), empty states, and accessible touch targets.

import { Globe, Search, AlertCircle } from 'lucide-react-native';
import { memo, useMemo } from 'react';
import {
  Linking,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';

import type { ChatItem } from '@homie/shared';
import { normalizeChatToolName } from '@homie/shared';
import { radius, spacing, touchTarget, type AppPalette, typography } from '@/theme/tokens';

/** Maximum characters for output preview before truncation. */
const OUTPUT_PREVIEW_LIMIT = 720;

/** Maximum URL display length before middle-truncation. */
const URL_DISPLAY_LIMIT = 60;

/* ── Type guards ─────────────────────────────────────────── */

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

/* ── Field extraction helpers ────────────────────────────── */

function extractField(raw: unknown, ...keys: string[]): string | undefined {
  if (!isRecord(raw)) return undefined;
  for (const key of keys) {
    const value = raw[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
}

function extractInputField(raw: unknown, ...keys: string[]): string | undefined {
  if (!isRecord(raw)) return undefined;
  const input = isRecord(raw.input) ? raw.input : raw;
  return extractField(input, ...keys);
}

/* ── Text truncation helpers ─────────────────────────────── */

function truncateOutput(text: string): { value: string; truncated: boolean } {
  if (text.length <= OUTPUT_PREVIEW_LIMIT) return { value: text, truncated: false };
  return { value: `${text.slice(0, OUTPUT_PREVIEW_LIMIT)}…`, truncated: true };
}

/**
 * Middle-truncates a URL for display while keeping protocol and end visible.
 * Example: "https://example.com/very/long/path/to/resource.html"
 *       -> "https://example.com/very/…/resource.html"
 */
function truncateUrl(url: string, maxLength = URL_DISPLAY_LIMIT): { display: string; full: string } {
  if (url.length <= maxLength) return { display: url, full: url };

  // Find protocol end
  const protocolEnd = url.indexOf('://');
  const protocolLength = protocolEnd > 0 ? protocolEnd + 3 : 0;

  // Reserve space for protocol + ellipsis + end portion
  const reservedStart = Math.min(protocolLength + 20, Math.floor(maxLength * 0.4));
  const reservedEnd = Math.floor(maxLength * 0.3);

  const start = url.slice(0, reservedStart);
  const end = url.slice(-reservedEnd);

  return { display: `${start}…${end}`, full: url };
}

/* ── Field types ─────────────────────────────────────────── */

interface ToolDetailField {
  label: string;
  value: string;
  fullValue?: string; // For URLs that were truncated
  mono?: boolean;
  truncated?: boolean;
  isUrl?: boolean;
}

interface SearchResult {
  title: string;
  url: string;
  snippet?: string;
}

/* ── Tool-specific field extractors ──────────────────────── */

function extractSearchResults(raw: unknown): SearchResult[] {
  if (!isRecord(raw)) return [];

  // Try multiple common result array keys
  const resultsArray = raw.results ?? raw.items ?? raw.data ?? raw.web_results;
  if (!Array.isArray(resultsArray)) return [];

  return resultsArray
    .filter(isRecord)
    .slice(0, 5) // Limit to 5 results for mobile display
    .map((item) => ({
      title: String(item.title ?? item.name ?? 'Untitled'),
      url: String(item.url ?? item.link ?? item.href ?? ''),
      snippet: item.snippet ?? item.description ?? item.excerpt
        ? String(item.snippet ?? item.description ?? item.excerpt)
        : undefined,
    }))
    .filter((r) => r.url); // Only include results with URLs
}

function fieldsForWebSearch(item: ChatItem): { fields: ToolDetailField[]; results: SearchResult[] } {
  const fields: ToolDetailField[] = [];
  const query = extractInputField(item.raw, 'query', 'search_query', 'q');
  if (query) fields.push({ label: 'Query', value: query });

  // Extract provider if available
  const provider = extractInputField(item.raw, 'provider', 'engine', 'source');
  if (provider) fields.push({ label: 'Provider', value: provider });

  // Try to extract structured results first
  const results = extractSearchResults(item.raw);

  // Fall back to text output if no structured results
  if (results.length === 0) {
    const output = item.output ?? extractField(item.raw, 'output', 'result', 'text');
    if (output) {
      const { value, truncated } = truncateOutput(output);
      fields.push({ label: 'Results', value, mono: true, truncated });
    }
  }

  return { fields, results };
}

function fieldsForWebFetch(item: ChatItem): ToolDetailField[] {
  const fields: ToolDetailField[] = [];

  // Title if available
  const title = extractInputField(item.raw, 'title', 'page_title');
  if (title) fields.push({ label: 'Title', value: title });

  // URL with truncation handling
  const url = extractInputField(item.raw, 'url', 'uri', 'href');
  if (url) {
    const { display, full } = truncateUrl(url);
    fields.push({
      label: 'URL',
      value: display,
      fullValue: full !== display ? full : undefined,
      mono: true,
      isUrl: true,
    });
  }

  // Prompt/instruction
  const prompt = extractInputField(item.raw, 'prompt', 'instruction');
  if (prompt) fields.push({ label: 'Prompt', value: prompt });

  // Response content
  const output = item.output ?? extractField(item.raw, 'output', 'result', 'text', 'content');
  if (output) {
    const { value, truncated } = truncateOutput(output);
    fields.push({ label: 'Response', value, mono: true, truncated });
  }

  return fields;
}

function fieldsForBrowser(item: ChatItem): ToolDetailField[] {
  const fields: ToolDetailField[] = [];

  // Action and target as paired fields
  const action = extractInputField(item.raw, 'action', 'command');
  const target = extractInputField(item.raw, 'target', 'selector', 'element');
  if (action) fields.push({ label: 'Action', value: action });
  if (target) fields.push({ label: 'Target', value: target });

  // URL with truncation
  const url = extractInputField(item.raw, 'url', 'uri', 'href');
  if (url) {
    const { display, full } = truncateUrl(url);
    fields.push({
      label: 'URL',
      value: display,
      fullValue: full !== display ? full : undefined,
      mono: true,
      isUrl: true,
    });
  }

  // Message/instruction
  const message = extractInputField(item.raw, 'message', 'instruction');
  if (message) fields.push({ label: 'Message', value: message });

  // Output/excerpt
  const output = item.output ?? extractField(item.raw, 'output', 'result', 'text', 'excerpt');
  if (output) {
    const { value, truncated } = truncateOutput(output);
    fields.push({ label: 'Output', value, mono: true, truncated });
  }

  return fields;
}

function fieldsForGenericTool(item: ChatItem): ToolDetailField[] {
  const fields: ToolDetailField[] = [];
  const text = item.text?.trim();
  if (text) fields.push({ label: 'Tool', value: text });
  if (item.raw) {
    try {
      const serialized = JSON.stringify(item.raw, null, 2);
      if (serialized) {
        const { value, truncated } = truncateOutput(serialized);
        fields.push({ label: 'Payload', value, mono: true, truncated });
      }
    } catch {
      // skip
    }
  }
  return fields;
}

/* ── Icon helper ─────────────────────────────────────────── */

function iconForTool(toolName: string | undefined, palette: AppPalette) {
  if (toolName === 'web_search') return <Search size={14} color={palette.accent} />;
  if (toolName === 'web_fetch' || toolName === 'browser') return <Globe size={14} color={palette.accent} />;
  return null;
}

function toolTypeLabel(toolName: string | undefined): string {
  if (toolName === 'web_search') return 'Web Search';
  if (toolName === 'web_fetch') return 'Fetch Page';
  if (toolName === 'browser') return 'Browser';
  return 'Tool';
}

/* ── Sub-components ──────────────────────────────────────── */

interface SearchResultCardProps {
  result: SearchResult;
  palette: AppPalette;
  last: boolean;
}

function SearchResultCard({ result, palette, last }: SearchResultCardProps) {
  const handlePress = () => {
    if (result.url) {
      void Linking.openURL(result.url);
    }
  };

  const { display: urlDisplay } = truncateUrl(result.url, 50);

  return (
    <Pressable
      accessibilityRole="link"
      accessibilityLabel={`${result.title}, ${result.url}`}
      accessibilityHint="Opens link in browser"
      onPress={handlePress}
      style={({ pressed }) => [
        styles.resultCard,
        {
          backgroundColor: palette.surface1,
          borderColor: palette.border,
          opacity: pressed ? 0.8 : 1,
          marginBottom: last ? 0 : spacing.xs,
        },
      ]}>
      <Text
        style={[styles.resultTitle, { color: palette.text }]}
        numberOfLines={2}>
        {result.title}
      </Text>
      <Text
        style={[styles.resultUrl, { color: palette.accent }]}
        numberOfLines={1}>
        {urlDisplay}
      </Text>
      {result.snippet ? (
        <Text
          style={[styles.resultSnippet, { color: palette.textSecondary }]}
          numberOfLines={3}>
          {result.snippet}
        </Text>
      ) : null}
    </Pressable>
  );
}

interface FieldRowProps {
  field: ToolDetailField;
  palette: AppPalette;
}

function FieldRow({ field, palette }: FieldRowProps) {
  const handleUrlPress = () => {
    if (field.isUrl && field.fullValue) {
      void Linking.openURL(field.fullValue);
    } else if (field.isUrl && field.value) {
      void Linking.openURL(field.value);
    }
  };

  const valueStyle = field.mono ? styles.fieldValueMono : styles.fieldValue;

  return (
    <View style={styles.fieldRow}>
      <View style={styles.fieldLabelRow}>
        <Text style={[styles.fieldLabel, { color: palette.textSecondary }]}>
          {field.label}
        </Text>
        {field.truncated ? (
          <View style={[styles.truncatedBadge, { backgroundColor: palette.warningDim }]}>
            <Text style={[styles.truncatedBadgeText, { color: palette.warning }]}>
              Truncated
            </Text>
          </View>
        ) : null}
      </View>
      {field.isUrl ? (
        <Pressable
          accessibilityRole="link"
          accessibilityLabel={`URL: ${field.fullValue ?? field.value}`}
          accessibilityHint="Opens link in browser"
          onPress={handleUrlPress}
          style={({ pressed }) => [
            styles.urlPressable,
            { opacity: pressed ? 0.7 : 1 },
          ]}>
          <Text
            style={[valueStyle, { color: palette.accent }]}
            numberOfLines={2}
            selectable>
            {field.value}
          </Text>
        </Pressable>
      ) : (
        <Text style={[valueStyle, { color: palette.text }]} selectable>
          {field.value}
        </Text>
      )}
    </View>
  );
}

interface EmptyStateProps {
  palette: AppPalette;
}

function EmptyState({ palette }: EmptyStateProps) {
  return (
    <View style={styles.emptyState}>
      <AlertCircle size={16} color={palette.textTertiary} />
      <Text style={[styles.emptyStateText, { color: palette.textTertiary }]}>
        No details available
      </Text>
    </View>
  );
}

/* ── Main component ──────────────────────────────────────── */

interface ChatToolDetailCardProps {
  item: ChatItem;
  palette: AppPalette;
  /** Enable nested scrolling for use inside FlatList */
  nestedScrollEnabled?: boolean;
  /** Maximum height before scrolling (for bottom sheet context) */
  maxHeight?: number;
}

function ChatToolDetailCardInner({
  item,
  palette,
  nestedScrollEnabled = true,
  maxHeight,
}: ChatToolDetailCardProps) {
  const rawName = isRecord(item.raw) && typeof item.raw.tool === 'string' ? item.raw.tool : item.text;
  const toolName = normalizeChatToolName(rawName ?? undefined);

  const { fields, results } = useMemo(() => {
    if (toolName === 'web_search') {
      return fieldsForWebSearch(item);
    }
    if (toolName === 'web_fetch') {
      return { fields: fieldsForWebFetch(item), results: [] };
    }
    if (toolName === 'browser') {
      return { fields: fieldsForBrowser(item), results: [] };
    }
    return { fields: fieldsForGenericTool(item), results: [] };
  }, [item, toolName]);

  const icon = iconForTool(toolName, palette);
  const hasContent = fields.length > 0 || results.length > 0;

  const content = (
    <View style={styles.contentInner}>
      {icon ? (
        <View style={styles.iconRow}>
          {icon}
          <Text style={[styles.toolType, { color: palette.accent }]}>
            {toolTypeLabel(toolName)}
          </Text>
        </View>
      ) : null}

      {!hasContent ? (
        <EmptyState palette={palette} />
      ) : (
        <>
          {fields.map((field, index) => (
            <FieldRow
              key={`${field.label}-${index}`}
              field={field}
              palette={palette}
            />
          ))}

          {results.length > 0 ? (
            <View style={styles.resultsSection}>
              <Text style={[styles.fieldLabel, { color: palette.textSecondary }]}>
                Results ({results.length})
              </Text>
              <View style={styles.resultsList}>
                {results.map((result, index) => (
                  <SearchResultCard
                    key={`result-${index}-${result.url}`}
                    result={result}
                    palette={palette}
                    last={index === results.length - 1}
                  />
                ))}
              </View>
            </View>
          ) : null}
        </>
      )}
    </View>
  );

  // Use ScrollView for long content in bottom sheet context
  const needsScroll = maxHeight !== undefined;

  return (
    <View
      style={[
        styles.card,
        {
          backgroundColor: palette.surface0,
          borderColor: palette.border,
          maxHeight,
        },
      ]}>
      {needsScroll ? (
        <ScrollView
          nestedScrollEnabled={nestedScrollEnabled}
          showsVerticalScrollIndicator={false}
          keyboardShouldPersistTaps="handled"
          contentContainerStyle={styles.scrollContent}>
          {content}
        </ScrollView>
      ) : (
        content
      )}
    </View>
  );
}

export const ChatToolDetailCard = memo(ChatToolDetailCardInner);

/* ── Styles ──────────────────────────────────────────────── */

const styles = StyleSheet.create({
  card: {
    borderRadius: radius.sm,
    borderWidth: 1,
    overflow: 'hidden',
  },
  scrollContent: {
    padding: spacing.sm,
  },
  contentInner: {
    gap: spacing.sm,
    padding: spacing.sm,
  },
  iconRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.xs,
  },
  toolType: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
  fieldRow: {
    gap: 2,
  },
  fieldLabelRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  fieldLabel: {
    ...typography.label,
    fontSize: 10,
    textTransform: 'uppercase',
  },
  fieldValue: {
    ...typography.body,
    fontSize: 13,
  },
  fieldValueMono: {
    ...typography.mono,
    fontSize: 11,
    lineHeight: 15,
  },
  urlPressable: {
    minHeight: touchTarget.min,
    justifyContent: 'center',
  },
  truncatedBadge: {
    borderRadius: radius.micro,
    paddingHorizontal: spacing.xs,
    paddingVertical: 2,
  },
  truncatedBadgeText: {
    ...typography.overline,
    fontSize: 9,
    textTransform: 'uppercase',
  },
  resultsSection: {
    gap: spacing.xs,
  },
  resultsList: {
    gap: spacing.xs,
  },
  resultCard: {
    borderRadius: radius.sm,
    borderWidth: 1,
    padding: spacing.sm,
    minHeight: touchTarget.min,
  },
  resultTitle: {
    ...typography.bodyMedium,
    fontSize: 13,
    marginBottom: 2,
  },
  resultUrl: {
    ...typography.monoSmall,
    marginBottom: spacing.xs,
  },
  resultSnippet: {
    ...typography.caption,
    fontSize: 12,
  },
  emptyState: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: spacing.xs,
    paddingVertical: spacing.lg,
  },
  emptyStateText: {
    ...typography.caption,
    fontStyle: 'italic',
  },
});
