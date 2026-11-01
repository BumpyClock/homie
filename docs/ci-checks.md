# Local CI checks

Install existing pnpm workspace dependencies with `pnpm install --frozen-lockfile`.
Build shared declarations with `pnpm shared:build` before checking consumers.
`pnpm web:typecheck` now explicitly checks both the application and Vite/Node
projects. The root solution config has no files; plain `tsc --noEmit` against it
was not a web source check. Explicit project checks retain `noEmit` and do not
write build-info files.

The existing Rust lint workflow checks out the recorded roci submodule recursively,
without persisting checkout credentials. It retains the pinned gitlink, rather
than fetching a moving upstream branch. Gateway, submodule identity and shared
Rust configuration now trigger the existing workflow. Its Linux job uses Boat;
owner-only public execution is configured separately, and no workflow was dispatched.

`python3 scripts/test-git-checks.py` runs offline disposable Git fixtures against
the executable pre-commit hook and existing CI path filters. The hook handles
staged deletions and renames, spaces and gitlinks, runs nonmutating workspace
format verification and preserves homie-core Clippy failure codes. No hook was
installed as part of this change.

Qualification limits from secret-free Linux checks follow.

- Web app and Node/Vite typechecks pass. Deliberate errors in each fail.
- Shared typecheck passes. Mobile typecheck still fails on missing Vitest/Node
  test-tool declarations in its existing package contract.
- Pinned roci retrieval succeeds and resolves the missing-path dependency failure.
- Workspace formatting passes. The sandbox Clippy baseline reports existing
  `collapsible_match` and `question_mark` diagnostics in homie-core.

A new mandatory full-graph pre-push gate remains deferred until those existing
baseline failures are resolved. No new schedule, required-check policy, native
acceptance or credentialed integration tests were enabled.
