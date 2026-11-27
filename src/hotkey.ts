const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS"]);

/** Builds a hotkey string such as "Ctrl+Shift+Insert" from a key press, or null while only modifiers are held. */
export function hotkeyFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");
  const key = e.code.replace(/^Key(?=[A-Z]$)/, "").replace(/^Digit(?=\d$)/, "");
  parts.push(key);
  return parts.join("+");
}

/** True when the press has no modifier and is not a function key, which a global hotkey needs. */
export function needsModifier(e: KeyboardEvent): boolean {
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
  return plain && !/^F\d+$/.test(e.code);
}
