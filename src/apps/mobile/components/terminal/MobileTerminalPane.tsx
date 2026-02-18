import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Animated, Keyboard, Platform, Pressable, StyleSheet, Text, View } from 'react-native';
import { ChevronDown, ChevronUp, Info } from 'lucide-react-native';
import { WebView } from 'react-native-webview';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

import { parseBinaryFrame, StreamType } from './terminal-binary';

interface MobileTerminalPaneProps {
  sessionId: string | null;
  shellLabel: string | null;
  sessionCols: number | null;
  sessionRows: number | null;
  sessionStatus: string | null;
  connected: boolean;
  onAttach: (sessionId: string, options?: { replay?: boolean; maxBytes?: number }) => Promise<void>;
  onBinaryMessage: (callback: (data: ArrayBuffer) => void) => () => void;
  onInput: (sessionId: string, data: string) => Promise<void>;
  onResize: (sessionId: string, cols: number, rows: number) => Promise<void>;
}

type WebTerminalEvent =
  | { type: 'ready' }
  | { type: 'input'; data: string }
  | { type: 'resize'; cols: number; rows: number }
  | { type: 'error'; message: string };

function fallbackDecodeUtf8(payload: Uint8Array): string {
  const chunkSize = 2048;
  let output = '';
  for (let index = 0; index < payload.length; index += chunkSize) {
    const segment = payload.subarray(index, index + chunkSize);
    output += String.fromCharCode(...segment);
  }
  return output;
}

type Decoder = (payload: ArrayBuffer | Uint8Array) => string;

const ACCESSORY_KEYS = [
  { id: 'tab', label: 'Tab', data: '\t' },
  { id: 'ctrl_c', label: '^C', data: '\u0003' },
  { id: 'ctrl_v', label: '^V', data: '\u0016' },
  { id: 'ctrl_b', label: '^B', data: '\u0002' },
  { id: 'esc', label: 'Esc', data: '\u001b' },
  { id: 'up', label: '↑', data: '\u001b[A' },
  { id: 'down', label: '↓', data: '\u001b[B' },
] as const;

const TERMINAL_HTML = `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no" />
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm/css/xterm.css" />
    <style>
      html, body {
        width: 100%;
        height: 100%;
        margin: 0;
        padding: 0;
        overflow: hidden;
        background: #0b1020;
      }
      #terminal {
        width: 100%;
        height: 100%;
        padding: 8px;
        box-sizing: border-box;
      }
      .xterm-viewport {
        overscroll-behavior: none;
      }
    </style>
  </head>
  <body>
    <div id="terminal"></div>
    <script>
      if (typeof globalThis === 'undefined') {
        window.globalThis = window;
      }
    </script>
    <script>
      window.__homiePostError = (message, detail) => {
        if (!window.ReactNativeWebView) {
          return;
        }
        const payload = {
          type: 'error',
          message: detail ? message + ': ' + detail : message,
        };
        window.ReactNativeWebView.postMessage(JSON.stringify(payload));
      };
    </script>
    <script
      src="https://cdn.jsdelivr.net/npm/@xterm/xterm/lib/xterm.js"
      onerror="window.__homiePostError && window.__homiePostError('xterm.js failed to load')"
    ></script>
    <script
      src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit/lib/addon-fit.js"
      onerror="window.__homiePostError && window.__homiePostError('xterm addon-fit failed to load')"
    ></script>
    <script>
      (function() {
        const post = (payload) => {
          if (window.ReactNativeWebView) {
            window.ReactNativeWebView.postMessage(JSON.stringify(payload));
          }
        };

        const postError = (message, detail) => {
          post({
            type: 'error',
            message: detail ? message + ': ' + detail : message,
          });
        };

        window.addEventListener('error', (event) => {
          const message = event.message || 'webview runtime error';
          const detail = event.error && event.error.message
            ? event.error.message
            : event.filename
              ? event.filename + ':' + event.lineno
              : '';
          postError(message, detail);
        });

        window.addEventListener('unhandledrejection', (event) => {
          const reason = event.reason;
          const message = reason && reason.message ? reason.message : String(reason);
          postError('Unhandled promise rejection', message);
        });

        try {
          const terminal = new window.Terminal({
            convertEol: true,
            cursorBlink: true,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            fontSize: 13,
            lineHeight: 1.25,
            theme: {
              background: '#0b1020',
              foreground: '#e6edf7',
              cursor: '#8aa4ff',
            },
          });
          const fitAddon = new window.FitAddon.FitAddon();
          terminal.loadAddon(fitAddon);
          terminal.open(document.getElementById('terminal'));

          const notifyResize = () => {
            fitAddon.fit();
            post({ type: 'resize', cols: terminal.cols, rows: terminal.rows });
          };

          let resizeTimer = null;
          window.addEventListener('resize', () => {
            clearTimeout(resizeTimer);
            resizeTimer = setTimeout(notifyResize, 40);
          });

          terminal.onData((data) => {
            post({ type: 'input', data });
          });

          window.__homieTerminal = {
            write(data) {
              if (typeof data === 'string' && data.length) {
                terminal.write(data);
              }
            },
            clear() {
              terminal.clear();
              terminal.reset();
            },
            focus() {
              terminal.focus();
            },
            fit() {
              notifyResize();
            },
          };

          notifyResize();
          terminal.focus();
          post({ type: 'ready' });
        } catch (error) {
          post({ type: 'error', message: error?.message || String(error) });
        }
      })();
    </script>
  </body>
</html>`;

function tryParseWebEvent(raw: string): WebTerminalEvent | null {
  try {
    const parsed = JSON.parse(raw) as WebTerminalEvent;
    if (!parsed || typeof parsed !== 'object' || typeof parsed.type !== 'string') return null;
    return parsed;
  } catch {
    return null;
  }
}

export function MobileTerminalPane({
  sessionId,
  shellLabel,
  sessionCols,
  sessionRows,
  sessionStatus,
  connected,
  onAttach,
  onBinaryMessage,
  onInput,
  onResize,
}: MobileTerminalPaneProps) {
  const { palette } = useAppTheme();
  const webViewRef = useRef<WebView>(null);
  const decoderRef = useRef<TextDecoder | null>(
    typeof TextDecoder === 'function' ? new TextDecoder() : null,
  );
  const decodePayload = useRef<Decoder>((payload: ArrayBuffer | Uint8Array): string => {
    if (decoderRef.current) {
      return decoderRef.current.decode(payload, { stream: true });
    }
    return fallbackDecodeUtf8(payload instanceof Uint8Array ? payload : new Uint8Array(payload));
  });
  const activeSessionRef = useRef<string | null>(null);
  const terminalReadyRef = useRef(false);
  const pendingWritesRef = useRef<string[]>([]);
  const lastKnownSizeRef = useRef<{ cols: number; rows: number } | null>(null);

  const [paneError, setPaneError] = useState<string | null>(null);
  const [attachedSessionId, setAttachedSessionId] = useState<string | null>(null);
  const [infoExpanded, setInfoExpanded] = useState(false);
  const [keyboardHeight, setKeyboardHeight] = useState(0);
  const [keyboardVisible, setKeyboardVisible] = useState(false);
  const infoAnim = useRef(new Animated.Value(0)).current;
  const accessoryAnim = useRef(new Animated.Value(0)).current;

  const clearTerminal = useCallback(() => {
    webViewRef.current?.injectJavaScript('window.__homieTerminal && window.__homieTerminal.clear(); true;');
  }, []);

  const flushWrites = useCallback(() => {
    if (!terminalReadyRef.current) return;
    if (!pendingWritesRef.current.length) return;
    const batch = pendingWritesRef.current.join('');
    pendingWritesRef.current = [];
    webViewRef.current?.injectJavaScript(
      `window.__homieTerminal && window.__homieTerminal.write(${JSON.stringify(batch)}); true;`,
    );
  }, []);

  const writeToTerminal = useCallback(
    (text: string) => {
      if (!text) return;
      if (!terminalReadyRef.current) {
        pendingWritesRef.current.push(text);
        return;
      }
      webViewRef.current?.injectJavaScript(
        `window.__homieTerminal && window.__homieTerminal.write(${JSON.stringify(text)}); true;`,
      );
    },
    [],
  );

  useEffect(() => {
    const unsubscribe = onBinaryMessage((raw) => {
      let frame;
      try {
        frame = parseBinaryFrame(raw);
      } catch {
        return;
      }
      if (!activeSessionRef.current || frame.sessionId !== activeSessionRef.current) return;
      if (frame.stream !== StreamType.Stdout && frame.stream !== StreamType.Stderr) return;
      const chunk = decodePayload.current(frame.payload);
      writeToTerminal(chunk);
    });
    return unsubscribe;
  }, [onBinaryMessage, writeToTerminal]);

  useEffect(() => {
    activeSessionRef.current = sessionId;
    setAttachedSessionId(null);
    setPaneError(null);
    setInfoExpanded(false);
    infoAnim.setValue(0);
    pendingWritesRef.current = [];
    decoderRef.current = typeof TextDecoder === 'function' ? new TextDecoder() : null;
    if (!sessionId) {
      clearTerminal();
    }
  }, [clearTerminal, infoAnim, sessionId]);

  useEffect(() => {
    if (!connected || !sessionId) return;
    let cancelled = false;

    const attach = async () => {
      try {
        await onAttach(sessionId, { replay: true, maxBytes: 256_000 });
        if (cancelled) return;
        setAttachedSessionId(sessionId);
        setPaneError(null);

        if (lastKnownSizeRef.current) {
          const { cols, rows } = lastKnownSizeRef.current;
          await onResize(sessionId, cols, rows);
        }
      } catch (error) {
        if (cancelled) return;
        const message = error instanceof Error ? error.message : 'Unable to attach terminal session';
        setPaneError(message);
      }
    };

    void attach();
    return () => {
      cancelled = true;
    };
  }, [connected, onAttach, onResize, sessionId]);

  useEffect(() => {
    const showEvent = Platform.OS === 'ios' ? 'keyboardWillShow' : 'keyboardDidShow';
    const hideEvent = Platform.OS === 'ios' ? 'keyboardWillHide' : 'keyboardDidHide';

    const showSubscription = Keyboard.addListener(showEvent, (event) => {
      setKeyboardVisible(true);
      setKeyboardHeight(event.endCoordinates?.height ?? 0);
      Animated.timing(accessoryAnim, {
        toValue: 1,
        duration: 180,
        useNativeDriver: true,
      }).start();
    });

    const hideSubscription = Keyboard.addListener(hideEvent, () => {
      setKeyboardVisible(false);
      Animated.timing(accessoryAnim, {
        toValue: 0,
        duration: 150,
        useNativeDriver: true,
      }).start(() => {
        setKeyboardHeight(0);
      });
    });

    return () => {
      showSubscription.remove();
      hideSubscription.remove();
    };
  }, [accessoryAnim]);

  const onWebMessage = useCallback(
    (raw: string) => {
      const event = tryParseWebEvent(raw);
      if (!event) return;

      if (event.type === 'ready') {
        terminalReadyRef.current = true;
        clearTerminal();
        flushWrites();
        webViewRef.current?.injectJavaScript('window.__homieTerminal && window.__homieTerminal.focus(); true;');
        return;
      }

      if (event.type === 'error') {
        setPaneError(event.message || 'Terminal runtime failed');
        return;
      }

      if (!sessionId || !connected) return;

      if (event.type === 'input') {
        void onInput(sessionId, event.data);
        return;
      }

      if (event.type === 'resize') {
        const cols = Math.max(2, Math.floor(event.cols));
        const rows = Math.max(2, Math.floor(event.rows));
        lastKnownSizeRef.current = { cols, rows };
        void onResize(sessionId, cols, rows);
      }
    },
    [clearTerminal, connected, flushWrites, onInput, onResize, sessionId],
  );

  const stateLabel = useMemo(() => {
    if (!connected) return 'Gateway disconnected';
    if (!sessionId) return 'No active session';
    if (paneError) return `Terminal error: ${paneError}`;
    if (attachedSessionId === sessionId) return shellLabel ? `Attached: ${shellLabel}` : 'Attached';
    return 'Attaching session...';
  }, [attachedSessionId, connected, paneError, sessionId, shellLabel]);

  const toggleInfoPanel = useCallback(() => {
    const next = !infoExpanded;
    setInfoExpanded(next);
    Animated.timing(infoAnim, {
      toValue: next ? 1 : 0,
      duration: 180,
      useNativeDriver: true,
    }).start();
  }, [infoAnim, infoExpanded]);

  const infoPanelStyle = useMemo(
    () => ({
      opacity: infoAnim,
      transform: [
        {
          translateY: infoAnim.interpolate({
            inputRange: [0, 1],
            outputRange: [-10, 0],
          }),
        },
      ],
    }),
    [infoAnim],
  );

  const accessoryStyle = useMemo(
    () => ({
      opacity: accessoryAnim,
      transform: [
        {
          translateY: accessoryAnim.interpolate({
            inputRange: [0, 1],
            outputRange: [10, 0],
          }),
        },
      ],
    }),
    [accessoryAnim],
  );

  const sendAccessoryInput = useCallback(
    (data: string) => {
      if (!sessionId || !connected) return;
      void onInput(sessionId, data);
    },
    [connected, onInput, sessionId],
  );

  return (
    <View style={[styles.container, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
      <View style={styles.webContainer}>
        <WebView
          ref={webViewRef}
          source={{ html: TERMINAL_HTML }}
          originWhitelist={['*']}
          onMessage={(event) => {
            onWebMessage(event.nativeEvent.data);
          }}
          javaScriptEnabled
          domStorageEnabled
          scrollEnabled={false}
          bounces={false}
          automaticallyAdjustContentInsets={false}
          style={styles.webview}
        />
      </View>

      <View style={styles.overlayRoot} pointerEvents="box-none">
        <Pressable
          accessibilityLabel={infoExpanded ? 'Hide terminal session info' : 'Show terminal session info'}
          onPress={toggleInfoPanel}
          style={({ pressed }) => [
            styles.infoToggle,
            {
              backgroundColor: palette.surface0,
              borderColor: palette.border,
              opacity: pressed ? 0.82 : 1,
            },
          ]}>
          <Info size={13} color={palette.textSecondary} />
          <Text style={[styles.infoToggleLabel, { color: palette.text }]}>Session info</Text>
          {infoExpanded ? (
            <ChevronUp size={13} color={palette.textSecondary} />
          ) : (
            <ChevronDown size={13} color={palette.textSecondary} />
          )}
        </Pressable>

        <Animated.View
          pointerEvents={infoExpanded ? 'auto' : 'none'}
          style={[
            styles.infoPanel,
            {
              backgroundColor: palette.surface0,
              borderColor: palette.border,
            },
            infoPanelStyle,
          ]}>
          <Text style={[styles.infoLinePrimary, { color: palette.text }]}>
            {shellLabel || 'Terminal session'}
          </Text>
          <Text style={[styles.infoLine, { color: paneError ? palette.danger : palette.textSecondary }]}>
            {stateLabel}
          </Text>
          <Text style={[styles.infoLine, { color: palette.textSecondary }]}>
            {`Resolution: ${sessionCols ?? '--'} x ${sessionRows ?? '--'}`}
          </Text>
          <Text style={[styles.infoLine, { color: palette.textSecondary }]}>
            {`Status: ${sessionStatus ?? 'unknown'}`}
          </Text>
        </Animated.View>
      </View>

      <Animated.View
        pointerEvents={keyboardVisible ? 'auto' : 'none'}
        style={[
          styles.accessoryBar,
          {
            backgroundColor: palette.surface0,
            borderColor: palette.border,
            bottom: keyboardHeight,
          },
          accessoryStyle,
        ]}>
        {ACCESSORY_KEYS.map((key) => (
          <Pressable
            key={key.id}
            disabled={!connected || !sessionId}
            onPress={() => {
              sendAccessoryInput(key.data);
            }}
            style={({ pressed }) => [
              styles.accessoryButton,
              {
                backgroundColor: pressed ? palette.surface2 : palette.surface1,
                borderColor: palette.border,
                opacity: connected && sessionId ? 1 : 0.55,
              },
            ]}>
            <Text style={[styles.accessoryLabel, { color: palette.text }]}>{key.label}</Text>
          </Pressable>
        ))}
      </Animated.View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    borderRadius: radius.lg,
    borderWidth: 1,
    minHeight: 280,
    overflow: 'hidden',
  },
  webContainer: {
    flex: 1,
    minHeight: 260,
  },
  webview: {
    flex: 1,
    backgroundColor: '#0b1020',
  },
  overlayRoot: {
    position: 'absolute',
    top: spacing.sm,
    left: spacing.sm,
    right: spacing.sm,
  },
  infoToggle: {
    alignItems: 'center',
    alignSelf: 'flex-start',
    borderRadius: radius.pill,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
  },
  infoToggleLabel: {
    ...typography.caption,
    fontWeight: '600',
  },
  infoPanel: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    marginTop: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  infoLinePrimary: {
    ...typography.caption,
    fontWeight: '600',
  },
  infoLine: {
    ...typography.caption,
    fontWeight: '500',
  },
  accessoryBar: {
    position: 'absolute',
    left: 0,
    right: 0,
    borderTopWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
    zIndex: 4,
  },
  accessoryButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 32,
    minWidth: 36,
    paddingHorizontal: spacing.sm,
  },
  accessoryLabel: {
    ...typography.caption,
    fontWeight: '600',
  },
});
