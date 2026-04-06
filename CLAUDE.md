# Forgecode Fork — Project Rules

## Release Workflow Architecture (DO NOT MODIFY without understanding)

This repository has GitHub **immutable releases** enforced. Published releases
cannot have assets uploaded after publication. This is a critical constraint.

### How releases work

```
ci.yml (push to main):
  build test → draft_release → build_release (uploads binaries to DRAFT)

Manual step:
  GitHub UI → Releases → Publish the draft

release.yml (release: published):
  Kept for upstream compatibility. Will fail on asset upload due to
  immutable releases — this is expected and matches upstream behavior.
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

### Key files

- `crates/forge_ci/src/workflows/release_publish.rs` — generates release.yml
- `crates/forge_ci/src/workflows/ci.rs` — generates ci.yml (has draft release flow)
- `crates/forge_ci/src/jobs/release_build_job.rs` — shared build matrix job
- `crates/forge_ci/src/jobs/release_draft.rs` — draft release creation job
