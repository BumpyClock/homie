import { useState, useEffect } from 'react';
import { uuid } from '@/lib/uuid';

export interface Target {
  id: string;
  name: string;
  url: string;
  type: 'local' | 'custom';
}

const STORAGE_KEY_TARGETS = 'homie-targets';
const STORAGE_KEY_ACTIVE_TARGET = 'homie-active-target';
const STORAGE_KEY_HIDE_LOCAL = 'homie-hide-local';

function normalizeGatewayUrl(raw: string) {
  const trimmed = raw.trim();
  let value = trimmed;
  if (value.startsWith("http://")) value = `ws://${value.slice(7)}`;
  if (value.startsWith("https://")) value = `wss://${value.slice(8)}`;
  if (!value.startsWith("ws://") && !value.startsWith("wss://")) return value;
  try {
    const parsed = new URL(value);
    if (!parsed.pathname || parsed.pathname === "/") {
      parsed.pathname = "/ws";
    } else if (!parsed.pathname.endsWith("/ws")) {
      parsed.pathname = `${parsed.pathname.replace(/\/$/, "")}/ws`;
    }
    return parsed.toString();
  } catch {
    return value;
  }
}

function getLocalGatewayUrl() {
  const envUrl = import.meta.env.VITE_GATEWAY_URL as string | undefined;
  if (envUrl) return normalizeGatewayUrl(envUrl);
  if (import.meta.env.DEV) {
    return normalizeGatewayUrl("ws://127.0.0.1:9800/ws"); 
  }
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const host = window.location.host;
  return normalizeGatewayUrl(`${protocol}//${host}/ws`);
}

const DEFAULT_TARGET: Target = {
  id: 'default-local',
  name: 'Local Gateway',
  url: getLocalGatewayUrl(),
  type: 'local'
};

export function useTargets() {
  const initialHideLocal = localStorage.getItem(STORAGE_KEY_HIDE_LOCAL) === "1";
  const initialTargets = (() => {
    const saved = localStorage.getItem(STORAGE_KEY_TARGETS);
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        const normalized = parsed.map((t: Target) =>
          t.type === "local"
            ? { ...t, url: getLocalGatewayUrl() }
            : { ...t, url: normalizeGatewayUrl(t.url) }
        );
        if (initialHideLocal) {
          return normalized.filter((t: Target) => t.type !== "local");
        }
        if (!normalized.some((t: Target) => t.type === "local")) {
          return [DEFAULT_TARGET, ...normalized];
        }
        return normalized;
      } catch (e) {
        console.error("Failed to parse saved targets", e);
      }
    }
    return initialHideLocal ? [] : [DEFAULT_TARGET];
  })();

  const initialActiveTargetId = (() => {
    const stored = localStorage.getItem(STORAGE_KEY_ACTIVE_TARGET);
    if (stored && initialTargets.some((t: Target) => t.id === stored)) return stored;
    return initialTargets[0]?.id ?? "";
  })();

  const [hideLocal, setHideLocal] = useState(initialHideLocal);
  const [targets, setTargets] = useState<Target[]>(initialTargets);
  const [activeTargetId, setActiveTargetId] = useState<string>(initialActiveTargetId);

  // Persist targets
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY_TARGETS, JSON.stringify(targets));
  }, [targets]);

  // Persist active selection
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY_ACTIVE_TARGET, activeTargetId);
  }, [activeTargetId]);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY_HIDE_LOCAL, hideLocal ? "1" : "0");
  }, [hideLocal]);

  const addTarget = (name: string, url: string) => {
    const newTarget: Target = {
      id: uuid(),
      name,
      url: normalizeGatewayUrl(url),
      type: 'custom'
    };
    setTargets(prev => [...prev, newTarget]);
    setActiveTargetId(newTarget.id); // Auto-select new target
  };

  const updateTarget = (id: string, updates: { name?: string; url?: string }) => {
    setTargets(prev => prev.map((t) => {
      if (t.id !== id) return t;
      if (t.type === 'local') {
        return { ...t, name: updates.name ?? t.name, url: getLocalGatewayUrl() };
      }
      return {
        ...t,
        name: updates.name ?? t.name,
        url: updates.url ? normalizeGatewayUrl(updates.url) : t.url,
      };
    }));
  };

  const removeTarget = (id: string) => {
    const target = targets.find((t) => t.id === id);
    const nextTargets = targets.filter((t) => t.id !== id);
    setTargets(nextTargets);
    if (activeTargetId === id) {
      setActiveTargetId(nextTargets[0]?.id ?? "");
    }
    if (target?.type === "local") {
      setHideLocal(true);
    }
  };

  const restoreLocal = () => {
    setHideLocal(false);
    if (!targets.some((t) => t.type === "local")) {
      setTargets([DEFAULT_TARGET, ...targets]);
    }
    if (!activeTargetId) {
      setActiveTargetId(DEFAULT_TARGET.id);
    }
  };

  const activeTarget = targets.find(t => t.id === activeTargetId) || targets[0] || null;

  return {
    targets,
    activeTarget,
    activeTargetId,
    setActiveTargetId,
    addTarget,
    updateTarget,
    removeTarget,
    hideLocal,
    restoreLocal
  };
}
