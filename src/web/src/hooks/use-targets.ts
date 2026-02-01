import { useState, useEffect } from 'react';

export interface Target {
  id: string;
  name: string;
  url: string;
  type: 'local' | 'custom';
}

const STORAGE_KEY_TARGETS = 'homie-targets';
const STORAGE_KEY_ACTIVE_TARGET = 'homie-active-target';

function getLocalGatewayUrl() {
  if (import.meta.env.DEV) {
    return "ws://127.0.0.1:3000"; 
  }
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const host = window.location.host;
  return `${protocol}//${host}`;
}

const DEFAULT_TARGET: Target = {
  id: 'default-local',
  name: 'Local Gateway',
  url: getLocalGatewayUrl(),
  type: 'local'
};

export function useTargets() {
  // Initialize targets from storage or default
  const [targets, setTargets] = useState<Target[]>(() => {
    const saved = localStorage.getItem(STORAGE_KEY_TARGETS);
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        // Ensure default target always has the correct calculated URL
        const withUpdatedDefault = parsed.map((t: Target) => 
          t.type === 'local' ? { ...t, url: getLocalGatewayUrl() } : t
        );
        // If local target is missing (legacy storage?), add it
        if (!withUpdatedDefault.some((t: Target) => t.type === 'local')) {
           return [DEFAULT_TARGET, ...withUpdatedDefault];
        }
        return withUpdatedDefault;
      } catch (e) {
        console.error("Failed to parse saved targets", e);
      }
    }
    return [DEFAULT_TARGET];
  });

  // Initialize active target ID
  const [activeTargetId, setActiveTargetId] = useState<string>(() => {
    return localStorage.getItem(STORAGE_KEY_ACTIVE_TARGET) || DEFAULT_TARGET.id;
  });

  // Persist targets
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY_TARGETS, JSON.stringify(targets));
  }, [targets]);

  // Persist active selection
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY_ACTIVE_TARGET, activeTargetId);
  }, [activeTargetId]);

  const addTarget = (name: string, url: string) => {
    const newTarget: Target = {
      id: crypto.randomUUID(),
      name,
      url,
      type: 'custom'
    };
    setTargets(prev => [...prev, newTarget]);
    setActiveTargetId(newTarget.id); // Auto-select new target
  };

  const removeTarget = (id: string) => {
    setTargets(prev => prev.filter(t => t.id !== id));
    if (activeTargetId === id) {
      setActiveTargetId(DEFAULT_TARGET.id);
    }
  };

  const activeTarget = targets.find(t => t.id === activeTargetId) || DEFAULT_TARGET;

  return {
    targets,
    activeTarget,
    activeTargetId,
    setActiveTargetId,
    addTarget,
    removeTarget
  };
}
