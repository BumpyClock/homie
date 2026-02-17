import { useState, useEffect, useRef } from "react";

const DEFAULT_IDLE_MS = 300;

/**
 * During active streaming, returns true so callers can render cheap plain text.
 * After idleMs of no content changes OR when streaming ends,
 * returns false so callers switch to full markdown rendering.
 *
 * @note WEB-ONLY: React Native cannot use hooks from compiled packages due to
 * multiple React instances. Mobile apps should implement this hook locally.
 * @see src/apps/mobile/components/chat/ChatTimelineMessageItem.tsx for RN implementation
 */
export function useStreamingDebounce(
  isStreaming: boolean,
  content: string,
  idleMs = DEFAULT_IDLE_MS,
): boolean {
  const [usePlainText, setUsePlainText] = useState(isStreaming);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!isStreaming) {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = null;
      setUsePlainText(false);
      return;
    }
    // Streaming: show plain text, schedule markdown switch on idle
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
