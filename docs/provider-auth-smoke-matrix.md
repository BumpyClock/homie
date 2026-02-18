# Provider auth smoke matrix

Date: 2026-02-18
Beads: `remotely-o1v.5.3`, `remotely-o1v.4.3`, `remotely-o1v.1.4`

## Execution evidence

- Command:
  - `pnpm --filter @homie/shared test -- src/hooks/__tests__/useProviderAuth.test.ts`
- Result:
  - Passed (`15/15` tests)

## Matrix (web + mobile settings auth flows)

| Flow | Web | Mobile | Evidence |
|---|---|---|---|
| Pending | Pass | Pass | Shared `useProviderAuth` polling state tests; both clients use shared hook |
| Slow-down interval increase | Pass | Pass | Shared `useProviderAuth` RFC 8628 `slow_down` test |
| Authorized | Pass | Pass | Shared `useProviderAuth` authorized transition test; both clients refresh providers on authorize |
| Denied | Pass | Pass | Shared hook denied-state test + shared copy (`Access denied`) |
| Expired | Pass | Pass | Shared hook expired-state test + shared copy (`Code expired`) |
| Timeout | Pass | Pass | Shared hook max-iteration timeout test |
| Cancellation | Pass | Pass | Shared hook cancel/unmount/reconnect tests |

## Copy parity checks

- Shared source of truth: `src/apps/shared/src/provider-auth-copy.ts` (`AUTH_COPY`)
- Web chat banner uses `AUTH_COPY.bannerMessage` + `AUTH_COPY.bannerActionWeb`
- Mobile chat banner uses `AUTH_COPY.bannerMessage` + `AUTH_COPY.bannerActionMobile`
- Web and mobile provider rows use shared `Connected` / `Not connected` labels

## Follow-ups

- None required for `o1v` completion; runtime env-gated provider credentials are still required for live account login checks.
