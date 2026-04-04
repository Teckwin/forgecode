---
alwaysApply: true
globs: ["*.sh", "*.md", "*.yaml", "*.yml"]
description: Git workflow enforcement - main branch protection and version labels
priority: 100
---

# Git Workflow Rules (CRITICAL)

This rule enforces the strict git workflow for this project. These rules are MANDATORY and cannot be bypassed.

## 1. Main Branch Protection (FORBIDDEN)

The following actions are **STRICTLY FORBIDDEN** and will be blocked by git hooks:

- ❌ `git checkout main` for development
- ❌ `git commit -m "..."` directly on main
- ❌ `git push origin main` directly
- ❌ Working on main branch locally

## 2. Required Workflow

All changes MUST follow this exact sequence:

```
Local Dev (feature branch) -> CI Verify -> PR Verify (with version label) -> Release
```

## 3. Version Label Requirement (CRITICAL)

**Every PR MUST have a version label before merging:**

| Label | When to Use |
|-------|-------------|
| `version: major` | Breaking API changes |
| `version: minor` | New features (backward compatible) |
| `version: patch` | Bug fixes only |
| `version: skip` | No version change (docs, CI, etc.) |

**PRs without this label CANNOT be merged.**

## 4. Release Process

1. PR is merged with version label
2. Version is decided at merge time (not at release time)
3. After merge, create tag and GitHub Release
4. Tag format: `v1.2.0` (semver)

## 5. Prohibited Actions

- ❌ Creating releases without PR
- ❌ Skipping CI verification
- ❌ Merging PR without version label
- ❌ Tagging without all CI passing