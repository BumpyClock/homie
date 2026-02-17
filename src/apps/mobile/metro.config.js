// ABOUTME: Metro bundler config for Expo in a pnpm workspace monorepo.
// ABOUTME: Configures watchFolders and nodeModulesPaths so Metro resolves workspace deps.

const { getDefaultConfig } = require("expo/metro-config");
const path = require("path");

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, "../../..");

const config = getDefaultConfig(projectRoot);

// Preserve Expo defaults, then add monorepo root (shared package source, etc.)
config.watchFolders = Array.from(
  new Set([...(config.watchFolders || []), monorepoRoot]),
);

// Tell Metro where to find hoisted node_modules
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
];
config.resolver.disableHierarchicalLookup = true;

// Force all workspace packages to share the same React runtime instance
config.resolver.extraNodeModules = {
  ...(config.resolver.extraNodeModules || {}),
  react: path.resolve(monorepoRoot, "node_modules/react"),
  "react/jsx-runtime": path.resolve(monorepoRoot, "node_modules/react/jsx-runtime.js"),
  "react/jsx-dev-runtime": path.resolve(monorepoRoot, "node_modules/react/jsx-dev-runtime.js"),
};

module.exports = config;
