import { type ChatItem } from '@homie/shared';
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { Image, Linking, Pressable, StyleSheet, Text, View } from 'react-native';
import Markdown, { Renderer, type RendererInterface } from 'react-native-marked';

import type { AppPalette } from '@/theme/tokens';
import { radius, spacing, typography } from '@/theme/tokens';

interface ChatMarkdownProps {
  content: string;
  itemKind: ChatItem['kind'];
  palette: AppPalette;
}

class ChatMarkdownRenderer extends Renderer implements RendererInterface {
  constructor(private readonly palette: AppPalette) {
    super();
  }

  private createImageNode(uri: string, alt?: string, title?: string) {
    return (
      <MarkdownImage
        key={this.getKey()}
        uri={uri}
        alt={alt || title || 'Image'}
        palette={this.palette}
      />
    );
  }

  image(uri: string, alt?: string, _style?: unknown, title?: string) {
    return this.createImageNode(uri, alt, title);
  }

  linkImage(href: string, imageUrl: string, alt?: string, _style?: unknown, title?: string | null) {
    const imageNode = this.createImageNode(imageUrl, alt, title || undefined);
    return (
      <Pressable
        accessibilityRole="link"
        accessibilityHint="Opens in browser"
        key={this.getKey()}
        onPress={() => {
          void Linking.openURL(href);
        }}
        style={({ pressed }) => ({ opacity: pressed ? 0.9 : 1 })}>
        {imageNode}
      </Pressable>
    );
  }
}

const MarkdownImage = React.memo(function MarkdownImage({ uri, alt, palette }: { uri: string; alt: string; palette: AppPalette }) {
  const [aspectRatio, setAspectRatio] = useState<number>(16 / 10);
  const [failed, setFailed] = useState(false);

  const handleRatio = useCallback((nextRatio: number) => {
    if (nextRatio > 0 && Number.isFinite(nextRatio)) setAspectRatio(nextRatio);
  }, []);

  const handleError = useCallback(() => setFailed(true), []);

  if (failed) {
    return (
      <View style={[styles.imageFallback, { borderColor: palette.border, backgroundColor: palette.surface0 }]}> 
        <Text style={[styles.imageFallbackLabel, { color: palette.textSecondary }]}>Image unavailable</Text>
        <Text numberOfLines={2} style={[styles.imageFallbackAlt, { color: palette.text }]}>
          {alt}
        </Text>
      </View>
    );
  }

  return (
    <View style={[styles.imageWrap, { borderColor: palette.border, backgroundColor: palette.surface0 }]}> 
      <View
        accessibilityRole="image"
        accessibilityLabel={alt}
        style={[styles.imageInner, { aspectRatio }]}
      >
        <View style={[styles.imageFill, { backgroundColor: palette.surface0 }]}>
          <MarkdownImageNative
            uri={uri}
            alt={alt}
            onRatio={handleRatio}
            onError={handleError}
          />
        </View>
      </View>
    </View>
  );
});

const MarkdownImageNative = React.memo(function MarkdownImageNative({
  uri,
  alt,
  onRatio,
  onError,
}: {
  uri: string;
  alt: string;
  onRatio: (ratio: number) => void;
  onError: () => void;
}) {
  const sourceRef = useRef({ uri });
  // Keep source object stable to prevent Image from reloading.
  if (sourceRef.current.uri !== uri) {
    sourceRef.current = { uri };
  }

  const handleLoad = useCallback(
    (event: { nativeEvent: { source?: { width: number; height: number } } }) => {
      const source = event.nativeEvent?.source;
      if (!source || source.width <= 0 || source.height <= 0) return;
      onRatio(source.width / source.height);
    },
    [onRatio],
  );

  return (
    <Image
      accessibilityLabel={alt}
      resizeMode="contain"
      source={sourceRef.current}
      style={styles.nativeImage}
      onError={onError}
      onLoad={handleLoad}
    />
  );
});

const FLAT_LIST_PROPS = { scrollEnabled: false } as const;

export function ChatMarkdown({ content, itemKind, palette }: ChatMarkdownProps) {
  const renderer = useMemo(() => new ChatMarkdownRenderer(palette), [palette]);
  const isAgentContent =
    itemKind === 'assistant' ||
    itemKind === 'reasoning' ||
    itemKind === 'tool' ||
    itemKind === 'plan' ||
    itemKind === 'diff';

  const markdownStyles = useMemo(
    () => ({
      text: {
        ...styles.itemBody,
        color: palette.text,
      },
      paragraph: {
        marginBottom: spacing.xs,
      },
      code: {
        backgroundColor: palette.surface0,
        borderColor: palette.border,
        borderRadius: radius.sm,
        borderWidth: 1,
        padding: spacing.xs,
      },
      codespan: {
        ...styles.commandText,
        backgroundColor: palette.surface0,
        color: palette.text,
      },
      link: {
        color: palette.accent,
        textDecorationLine: 'underline' as const,
      },
      blockquote: {
        borderLeftColor: palette.border,
        borderLeftWidth: 3,
        paddingLeft: spacing.sm,
      },
      list: {
        marginBottom: spacing.xs,
      },
      li: {
        ...styles.itemBody,
        color: palette.text,
      },
      h1: {
        ...styles.itemBody,
        color: palette.text,
        fontSize: 18,
        fontWeight: '700' as const,
      },
      h2: {
        ...styles.itemBody,
        color: palette.text,
        fontSize: 16,
        fontWeight: '700' as const,
      },
      h3: {
        ...styles.itemBody,
        color: palette.text,
        fontSize: 15,
        fontWeight: '600' as const,
      },
    }),
    [palette],
  );

  if (!isAgentContent) {
    return <Text style={[styles.itemBody, { color: palette.text }]}>{content}</Text>;
  }

  return (
    <Markdown
      value={content}
      renderer={renderer}
      styles={markdownStyles}
      flatListProps={FLAT_LIST_PROPS}
    />
  );
}

const styles = StyleSheet.create({
  itemBody: {
    ...typography.body,
    fontSize: 14,
    fontWeight: '400',
  },
  commandText: {
    ...typography.data,
    fontSize: 12,
  },
  imageWrap: {
    borderRadius: 7,
    borderWidth: 1,
    marginBottom: spacing.sm,
    marginTop: spacing.xs,
    overflow: 'hidden',
    width: '100%',
  },
  imageInner: {
    maxHeight: 350,
    minHeight: 120,
    width: '100%',
  },
  imageFill: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
  },
  nativeImage: {
    height: '100%',
    width: '100%',
  },
  imageFallback: {
    borderRadius: 7,
    borderWidth: 1,
    gap: spacing.xs,
    marginBottom: spacing.sm,
    marginTop: spacing.xs,
    minHeight: 96,
    padding: spacing.sm,
    width: '100%',
  },
  imageFallbackLabel: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
  imageFallbackAlt: {
    ...typography.body,
    fontSize: 14,
    fontWeight: '400',
  },
});
