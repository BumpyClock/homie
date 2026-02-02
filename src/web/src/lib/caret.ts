const CARET_STYLE_PROPS = [
  "boxSizing",
  "width",
  "height",
  "overflowX",
  "overflowY",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "fontStyle",
  "fontVariant",
  "fontWeight",
  "fontStretch",
  "fontSize",
  "fontFamily",
  "lineHeight",
  "letterSpacing",
  "textTransform",
  "textIndent",
  "textRendering",
  "wordSpacing",
  "tabSize",
  "whiteSpace",
] as const;

export function getTextareaCaretPosition(
  textarea: HTMLTextAreaElement,
  value: string,
  cursor: number,
): { x: number; y: number } | null {
  if (typeof window === "undefined") return null;

  const style = window.getComputedStyle(textarea);
  const div = document.createElement("div");

  CARET_STYLE_PROPS.forEach((prop) => {
    div.style[prop] = style[prop];
  });

  div.style.position = "absolute";
  div.style.visibility = "hidden";
  div.style.whiteSpace = "pre-wrap";
  div.style.wordWrap = "break-word";
  div.style.overflowWrap = "break-word";
  div.style.top = "0";
  div.style.left = "-9999px";
  div.style.width = `${textarea.clientWidth}px`;

  div.textContent = value.slice(0, cursor);
  const span = document.createElement("span");
  span.textContent = value.slice(cursor) || ".";
  div.appendChild(span);

  document.body.appendChild(div);

  const spanRect = span.getBoundingClientRect();
  const divRect = div.getBoundingClientRect();
  const inputRect = textarea.getBoundingClientRect();

  const x = inputRect.left + (spanRect.left - divRect.left) - textarea.scrollLeft;
  const y = inputRect.top + (spanRect.top - divRect.top) - textarea.scrollTop;

  document.body.removeChild(div);

  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return { x, y };
}
