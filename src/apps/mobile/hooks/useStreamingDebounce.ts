// ABOUTME: Local RN implementation of streaming debounce hook.
// ABOUTME: Compiled @homie/shared hooks cause duplicate-React crashes in Metro;
// ABOUTME: this re-implements the logic using the app's own React instance.

import { useEffect, useRef, useState } from 'react';

const DEFAULT_IDLE_MS = 300;

/**
 * During active streaming, returns `true` so callers can render cheap plain text.
 * After {@link idleMs} of no content changes **or** when streaming ends,
 * returns `false` so callers switch to full markdown rendering.
 */
export function useStreamingDebounce(
  isStreaming: boolean | undefined,
  content: string,
  idleMs = DEFAULT_IDLE_MS,
): boolean {
  const [usePlainText, setUsePlainText] = useState(!!isStreaming);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!isStreaming) {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = null;
      setUsePlainText(false);
      return;
    }
    setUsePlainText(true);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setUsePlainText(false);
      timerRef.current = null;
    }, idleMs);
  }, [isStreaming, content, idleMs]);

  useEffect(
    () => () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    },
    [],
  );

  return usePlainText;
}
