# Forgecode Fork — Project Rules

## Release Workflow Architecture (DO NOT MODIFY without understanding)

This repository has GitHub **immutable releases** enforced. Published releases
cannot have assets uploaded after publication. This is a critical constraint.

### How releases work — correct sequence (verified: v0.1.3, v0.1.4)

```
┌─────────────────────────────────────────────────────────────────┐
│ ci.yml (triggers: push to main OR v* tag push)                  │
│                                                                 │
│   1. build (test + coverage)                                    │
│   2. draft_release (release-drafter creates/updates DRAFT)      │
│      ↓ needs: build                                             │
│      ↓ condition: push to main only                             │
│   3. build_release (9 platform binaries → upload to DRAFT)      │
│      ↓ needs: draft_release                                     │
│      ↓ condition: push to main only                             │
│                                                                 │
│   Result: Draft release with all 9 binaries attached            │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Manual step: GitHub UI → Releases → Publish the draft           │
│                                                                 │
│   This transitions the draft to a published release.            │
│   All binaries are already present — no upload needed.          │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ release.yml "Multi Channel Release" (trigger: release published)│
│                                                                 │
│   Tries to rebuild + re-upload binaries to the published release│
│   ⚠ EXPECTED FAILURE: "Upload to Release" step fails because   │
│   immutable releases reject asset uploads after publication.    │
│   The fastest platform to finish its build reaches the upload   │
│   step first and fails, triggering fail-fast cascade that       │
│   cancels all remaining matrix jobs (e.g. v0.1.4:               │
│   x86_64-apple-darwin failed → 8 others cancelled).             │
│                                                                 │
│   THIS IS FINE — the binaries were already uploaded during the  │
│   draft phase by ci.yml. This workflow exists only for upstream │
│   compatibility with antinomyhq/forgecode.                      │
└─────────────────────────────────────────────────────────────────┘
```

### Rules for AI agents

1. **NEVER** change `release.yml` trigger from `published` to `created` or other types
2. **NEVER** add experimental upload methods (gh cli, softprops/action-gh-release, etc.)
   to work around immutable releases — the draft flow in ci.yml already handles it
3. **NEVER** remove `xresloader/upload-to-github-release` from release_build_job.rs —
   it's needed for upstream compatibility even though it fails on published releases
4. The correct fix for "Cannot upload assets to an immutable release" is to use
   the draft release flow in ci.yml, NOT to modify release.yml
5. Keep release.yml as close to upstream (antinomyhq/forgecode) as possible,
   only removing npm_release and homebrew_release jobs
6. The "Multi Channel Release" workflow **always shows as failed** in GitHub Actions
   after publishing a draft — this is normal and expected. Do NOT attempt to fix it.
7. If ci.yml's `build_release` job fails, the draft will have missing binaries.
   Fix the build issue and re-push to main — release-drafter will update the
   existing draft, and `build_release` will upload the missing binaries.

### Key files

- `crates/forge_ci/src/workflows/release_publish.rs` — generates release.yml
- `crates/forge_ci/src/workflows/ci.rs` — generates ci.yml (has draft release flow)
- `crates/forge_ci/src/jobs/release_build_job.rs` — shared build matrix job
- `crates/forge_ci/src/jobs/release_draft.rs` — draft release creation job

## Open Feature Issues

### Issue #42: Align .forge/ directory convention with Claude Code
- Add `tools/` and `agents/` directory support
- Consolidate `policies.yaml` into `settings.json`
- Extend config adapter migration coverage (currently 45%)

### Issue #45: Complete auto-memory integration and conversational definitions
- Wire `ForgeMemoryService` into agent pipeline (load + system prompt + tools)
- Add `create-agent`, `create-rule`, `create-tool` skills
- Hot-reload for definitions created mid-session
- Depends on #42 (directories must exist before runtime can use them)
