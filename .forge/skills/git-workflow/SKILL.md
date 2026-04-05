---
name: git-workflow
description: |
  Git workflow enforcement for this project. Use when: (1) Any git operations are needed,
  (2) Creating branches for development, (3) Merging or releasing code, (4) User asks about
  git workflow or branch management. Enforces: NO development on main branch, NO switching
  to main branch locally, strict iteration workflow: local dev -> CI verify -> PR verify -> release.
  **CRITICAL**: All PRs MUST have version label (version: major|minor|patch|skip) before merging.
---

# Git Workflow Enforcement

This skill enforces strict git workflow rules to ensure code quality and traceability.

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

## Mandatory Rules

### 1. NEVER Develop on main Branch (CRITICAL)

- **禁止在 main 分支开发**: All development must happen on feature branches
- **禁止切换到 main 分支**: Never checkout main for development work
- **禁止在 main 上提交**: No commits directly to main
- **GIT HOOK ENFORCED**: Pre-commit and pre-push hooks prevent main branch operations

### 2. Strict Development Workflow

All changes MUST follow this exact sequence:

```
Local Dev -> CI Verify -> PR Verify -> Release
```

| Stage | Action | Verification |
|-------|--------|--------------|
| **1. Local Dev** | Create feature branch, implement, test locally | `cargo fmt && cargo clippy && cargo test` |
| **2. CI Verify** | Push branch, trigger CI (add `ci: build all targets` label if needed) | All CI jobs pass |
| **3. PR Verify** | Create PR with version label, review, address feedback | PR approved & merged |
| **4. Release** | Tag version, create GitHub Release | Release published |

### 3. Version Label Requirement (CRITICAL)

**Every PR MUST have a version label before merging:**

| Label | When to Use | Version Bump |
|-------|-------------|--------------|
| `version: major` | Breaking API changes | +1.0.0 |
| `version: minor` | New features (backward compatible) | x+1.0 |
| `version: patch` | Bug fixes only | x.x+1 |
| `version: skip` | No version change | no bump |

**Without this label, PR cannot be merged.**

### 4. Branch Naming Convention

Use prefixes:
- `feat/` - New features
- `fix/` - Bug fixes
- `refactor/` - Code refactoring
- `chore/` - Maintenance tasks

Examples: `feat/new-tool`, `fix/bug-description`, `refactor/core-module`

### 5. Commit Message Format

```
<type>(<scope>): <description>

[optional body]

Co-Authored-By: ForgeCode <noreply@forgecode.dev>
```

Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `ci`

### 6. CI Label Usage

| Label | When to Use | Effect |
|-------|-------------|--------|
| `ci: build all targets` | Full cross-platform validation needed | Runs 9 platform builds (Linux musl/gnu, macOS, Windows, Android) |
| (no label) | Quick local validation | Basic CI (build + zsh_rprompt_perf only) |

**Note**: The `ci: build all targets` label triggers draft_release and build_release jobs for PRs, enabling full cross-platform build verification before merging.

## Prohibited Actions

### These are FORBIDDEN:

- ❌ `git checkout main` for any development work
- ❌ `git commit -m "..."` directly on main
- ❌ `git push origin main` directly
- ❌ Working on main branch locally
- ❌ Skipping CI verification before PR
- ❌ Merging PR without CI passing
- ❌ Merging PR without version label (`version: major|minor|patch|skip`)

## Workflow Enforcement in Code

When generating code or performing git operations:

1. **Always create/use feature branches** - Never commit to main
2. **Verify before commit** - Run `cargo fmt && cargo clippy && cargo test`
3. **Use correct workflow** - Feature branch -> CI -> PR -> Release
4. **Add Co-Authored-By** - Include `Co-Authored-By: ForgeCode <noreply@forgecode.dev>` in commit messages

## Git Commands Reference

### Create Feature Branch
```bash
git checkout -b feat/my-feature
```

### Sync with Upstream
```bash
git fetch origin
git rebase origin/main
```

### Verify Before Commit
```bash
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace
```

### Push Branch
```bash
git push -u origin feat/my-feature
```

### Create PR
```bash
gh pr create --base main --head feat/my-feature --title "Description" --body "Details"
```