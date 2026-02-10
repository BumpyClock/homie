import type { ChatSettings } from "@homie/shared";

const OVERRIDE_KEY_PREFIX = "homie-chat-overrides:";
const SETTINGS_KEY_PREFIX = "homie-chat-settings:";

function overridesKey(namespace: string) {
  return `${OVERRIDE_KEY_PREFIX}${namespace || "default"}`;
}

function settingsKey(namespace: string) {
  return `${SETTINGS_KEY_PREFIX}${namespace || "default"}`;
}

export function loadOverrides(namespace: string): Record<string, string> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(overridesKey(namespace));
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return parsed as Record<string, string>;
    }
  } catch {
    return {};
  }
  return {};
}

export function saveOverrides(namespace: string, overrides: Record<string, string>) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(overridesKey(namespace), JSON.stringify(overrides));
  } catch {
    return;
  }
}

export function loadSettings(namespace: string): Record<string, ChatSettings> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(settingsKey(namespace));
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return parsed as Record<string, ChatSettings>;
    }
  } catch {
    return {};
  }
  return {};
}

export function saveSettings(namespace: string, settings: Record<string, ChatSettings>) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(settingsKey(namespace), JSON.stringify(settings));
  } catch {
    return;
  }
}
