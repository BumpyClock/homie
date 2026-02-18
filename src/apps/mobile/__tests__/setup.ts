/**
 * Test setup for the mobile app.
 *
 * - Mocks RN modules that are unavailable in jsdom.
 * - Provides an env gate: tests importing `LIVE_TEST_ENABLED` and
 *   `GATEWAY_URL` can skip when no live gateway is configured.
 */

/** True when GATEWAY_URL env var is set — guards live integration tests. */
export const LIVE_TEST_ENABLED = Boolean(process.env.GATEWAY_URL);

/** Gateway URL from env; empty string when not set. */
export const GATEWAY_URL = process.env.GATEWAY_URL ?? '';
