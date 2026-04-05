---
name: release-workflow
description: |
  Complete open source release workflow following standard practices. This skill emphasizes
  triple verification gates: must pass local verification -> CI verification -> PR merge verification
  before releasing. Tag and release are only executed when user actively requests it.
  Prohibit bypassing PR restrictions or skipping CI verification.
---

# Release Workflow

This skill enforces a strict release workflow with version management and triple verification gates.

## CI Trigger Configuration

The project uses two workflow files:
- **ci.yml**: Runs on push (main branch, tags) and PR events
- **release.yml**: Runs only on release published events

### CI Jobs and Triggers

| Event | build | zsh_rprompt_perf | draft_release | build_release | Notes |
|-------|-------|------------------|---------------|---------------|-------|
| push main | ✓ | ✓ | ✓ | ✓ | Full CI with multi-platform build |
| push tags (v*) | ✓ | ✓ | - | - | Basic CI only |
| PR | ✓ | ✓ | - | - | Basic CI |
| PR + `ci: build all targets` | ✓ | ✓ | ✓* | ✓* | Full cross-platform build |
| release published | - | - | - | - | Handled by release.yml |

*PR builds require the `ci: build all targets` label to trigger draft_release and build_release

### Release Workflow

For official releases:
1. **push main** triggers ci.yml which runs build_release (multi-platform build)
2. When ready to release, create a GitHub Release which triggers release.yml
3. release.yml handles the actual multi-platform binary packaging and distribution

This ensures:
- Multi-platform builds are verified on push to main before release
- release.yml only handles packaging when user creates a GitHub Release

## Mandatory Rules

### 1. Version Decision at PR Merge Time

**CRITICAL**: When a PR is about to be merged, the version bump MUST be decided BEFORE merging.

- **Version bumps are decided at PR merge time, not at release time**
- A single version can contain multiple PRs (e.g., PR #1 + PR #2 -> v1.2.0)
- **Every PR must declare its version impact** using labels:
  - `version: major` - Breaking changes
  - `version: minor` - New features (backward compatible)
  - `version: patch` - Bug fixes only
  - `version: skip` - No version change (docs, CI, etc.)

### 2. PR Requirements for Release

Before a PR can be merged for release, it MUST:

1. **Have a version label** (`version: major|minor|patch|skip`)
2. **Cover all release build nodes** - If the release requires:
   - Building for multiple platforms
   - Running all tests
   - Creating release artifacts
   Then the PR MUST include all necessary changes for these build nodes

### 3. Triple Verification Gates

All releases MUST pass these three verification gates:

| Gate | Action | Verification |
|------|--------|--------------|
| **1. Local Verification** | Run locally before push | `cargo fmt && cargo clippy && cargo insta test --accept` |
| **2. CI Verification** | CI pipeline passes | All CI jobs pass (check + build + test) |
| **3. PR Merge Verification** | PR reviewed and merged | PR approved and merged to main |

**Tag and Release are ONLY executed after all three gates pass.**

## Release Types

### Regular Release (PR-based)

```
1. Developer creates feature branch -> implements -> local verify
2. Push branch -> CI verify
3. Create PR with version label -> PR verify
4. PR merged -> version decided at merge time
5. After merge, create tag and release (using this skill)
```

### Emergency Release (Hotfix)

For critical bugs requiring immediate release:
- Create hotfix branch from latest tag
- Implement fix with `version: patch` label
- Fast-track PR with explicit approval
- Merge and release immediately

## Version Label Usage

| Label | When to Use | Example |
|-------|-------------|---------|
| `version: major` | Breaking API changes | Removing a public method, changing function signature |
| `version: minor` | New features | Adding new command, new option |
| `version: patch` | Bug fixes | Fixing incorrect behavior |
| `version: skip` | No version change | Docs only, CI config, refactoring |

## Prohibited Actions

### These are FORBIDDEN:

- ❌ Creating releases without PR
- ❌ Skipping CI verification
- ❌ Merging PR without version label
- ❌ Tagging without all CI passing
- ❌ Bypassing PR merge for "quick fixes"
- ❌ Multiple releases without coordinating version

## Release Workflow in Code

When performing release operations:

1. **Verify PR has version label** - Check `version: major|minor|patch|skip`
2. **Verify CI passed** - All checks must pass
3. **Verify PR merged** - Must be merged to main
4. **Create tag with correct version** - Follow semver
5. **Create GitHub Release** - Include changelog

## Release Commands

### Create Version Tag
```bash
# Major release
git tag -a v2.0.0 -m "Release v2.0.0"

# Minor release
git tag -a v1.2.0 -m "Release v1.2.0"

# Patch release
git tag -a v1.1.1 -m "Release v1.1.1"
```

### Push Tag
```bash
git push origin v1.2.0
```

### Create GitHub Release
```bash
gh release create v1.2.0 --title "v1.2.0" --notes-file CHANGELOG.md
```

## Version Bump Decision

When merging multiple PRs:

```
PR #1: version: minor (new feature)
PR #2: version: patch (bug fix)
-> Combined version: minor (v1.2.0)

PR #1: version: minor (new feature)
PR #2: version: major (breaking change)
-> Combined version: major (v2.0.0)
```

Priority: major > minor > patch > skip