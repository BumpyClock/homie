#!/usr/bin/env node

import { spawnSync } from "node:child_process";

function trimTrailingDot(value) {
  return value.endsWith(".") ? value.slice(0, -1) : value;
}

function detectTailscaleDnsName() {
  const status = spawnSync("tailscale", ["status", "--json"], {
    encoding: "utf8",
  });

  if (status.status !== 0 || !status.stdout) {
    return null;
  }

  try {
    const parsed = JSON.parse(status.stdout);
    const dnsName = parsed?.Self?.DNSName;
    if (typeof dnsName !== "string" || dnsName.trim().length === 0) {
      return null;
    }
    return trimTrailingDot(dnsName.trim());
  } catch {
    return null;
  }
}

function resolveExpoHostname() {
  const explicitHost = process.env.EXPO_DEV_HOSTNAME?.trim();
  if (explicitHost) {
    return explicitHost;
  }
  return detectTailscaleDnsName();
}

const hostname = resolveExpoHostname();
const nextEnv = { ...process.env };

if (hostname) {
  nextEnv.REACT_NATIVE_PACKAGER_HOSTNAME = hostname;
}

if (hostname && !process.env.EXPO_PACKAGER_PROXY_URL) {
  const portIndex = process.argv.indexOf("--port");
  const port =
    portIndex >= 0 && process.argv[portIndex + 1]
      ? process.argv[portIndex + 1]
      : "8081";
  nextEnv.EXPO_PACKAGER_PROXY_URL = `http://${hostname}:${port}`;
}

if (hostname) {
  console.log(`[mobile] Expo host: ${hostname}`);
} else {
  console.log("[mobile] Expo host: default LAN IP (Tailscale hostname unavailable)");
}

const result = spawnSync("pnpm", ["exec", "expo", "start", ...process.argv.slice(2)], {
  stdio: "inherit",
  env: nextEnv,
});

if (typeof result.status === "number") {
  process.exit(result.status);
}

process.exit(1);
