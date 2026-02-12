// ABOUTME: Metro bundler config for Expo in a pnpm workspace monorepo.
// ABOUTME: Configures watchFolders and nodeModulesPaths so Metro resolves workspace deps.

const { getDefaultConfig } = require("expo/metro-config");
const path = require("path");

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, "../../..");

const config = getDefaultConfig(projectRoot);

// Watch all files in the monorepo (shared package source, etc.)
config.watchFolders = [monorepoRoot];

// Tell Metro where to find hoisted node_modules
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
];

module.exports = config;
