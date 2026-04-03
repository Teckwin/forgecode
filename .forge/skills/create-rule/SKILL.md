---
name: create-rule
description: Create new rules for the code-forge application. Rules are stored as .md files in the <cwd>/.forge/rules directory with YAML frontmatter (alwaysApply, globs, description) and markdown body containing rule content. Use when users need to add new rules, modify existing rules, or understand the rule file structure.
---

# Create Rules

Create and manage dynamic rules for the code-forge application. Rules are instructions that get injected into the conversation context based on certain conditions.

## File Location

**CRITICAL**: All rule files must be created in the `<cwd>/.forge/rules` directory, where `<cwd>` is the current working directory of your code-forge project.

- **Directory**: `<cwd>/.forge/rules`
- **File format**: `{rule-name}.md`
- **Example**: If your project is at `/home/user/my-project`, rules go in `/home/user/my-project/.forge/rules/`

This is the only location where forge will discover and load custom rules.

## Rule File Structure

Every rule file must have:

1. **YAML Frontmatter** (optional but recommended):
   - `alwaysApply`: If true, rule always applies to all conversations
   - `globs`: File patterns to match (e.g., `["*.rs", "**/src/**/*.ts"]`)
   - `description`: Brief description of what the rule does
   - `priority`: Rule priority (higher = more important, default: 0)
   - `enabled`: Whether the rule is active (default: true)

2. **Rule Body** (required):
   - Rule content that gets injected into conversation context
   - Can include instructions, guidelines, or context-specific information

### Example Rule File

```markdown
---
alwaysApply: true
globs: ["*.rs"]
description: Rust coding standards for this project
priority: 10
---

# Rust Coding Standards

When writing Rust code for this project:

1. Use `anyhow::Result` for error handling in services
2. Create domain errors using `thiserror`
3. Never implement `From` for converting domain errors
4. Use `pretty_assertions` for better test error messages
```

### Complete Sample Rule

This sample demonstrates all frontmatter options:

```markdown
---
alwaysApply: false
globs: ["*.ts", "*.tsx"]
description: TypeScript best practices for React components
priority: 5
enabled: true
---

# React TypeScript Guidelines

When creating React components:

1. Use functional components with hooks
2. Define proper TypeScript types for props
3. Use `React.FC<Props>` for component typing
4. Keep components small and focused
```

## Frontmatter Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| alwaysApply | boolean | No | false | If true, rule always applies |
| globs | array | No | [] | File patterns to match |
| description | string | No | none | Brief description |
| priority | number | No | 0 | Higher = more important |
| enabled | boolean | No | true | Whether rule is active |

### Glob Patterns

Common glob patterns:

- `*.rs` - All Rust files in current directory
- `**/*.ts` - All TypeScript files recursively
- `src/**/*` - All files in src directory
- `*.{js,ts}` - All JS and TS files
- `!test*.js` - Exclude test files

## Creating a New Rule

### Step 1: Determine Rule Purpose

Identify what the rule should accomplish:

- What context should be added?
- When should it apply?
- Is it project-specific or universal?

### Step 2: Choose Rule Name

Use descriptive names with hyphens for multi-word names:

- Good: `rust-standards`, `react-guidelines`, `api-documentation`
- Bad: `rust`, `React`, `APIDocs`

### Step 3: Write the Rule File

Create the file in the `<cwd>/.forge/rules/{rule-name}.md` directory.

**IMPORTANT**: The file MUST be in `<cwd>/.forge/rules` where `<cwd>` is your current working directory. Rules placed anywhere else will not be discovered by forge.

#### Frontmatter

```yaml
---
alwaysApply: false
globs: ["*.rs"]
description: Your rule description
priority: 5
---
```

#### Rule Body

Write the rule content in markdown format. This will be injected into the conversation context when the rule applies.

## Rule Application Logic

### alwaysApply: true

Rules with `alwaysApply: true` are injected into every conversation regardless of context.

```markdown
---
alwaysApply: true
---

# Global Rule

This rule applies to all conversations.
```

### File-based Rules

Rules with `globs` are only injected when the conversation involves files matching those patterns.

```markdown
---
globs: ["*.rs", "**/src/**/*.rs"]
description: Rust-specific guidelines
---

# Rust Guidelines

These guidelines apply when working with Rust files.
```

### Combined

You can combine both for more complex logic:

```markdown
---
alwaysApply: false
globs: ["*.ts", "*.tsx"]
description: React TypeScript rules
priority: 10
---

# React TypeScript Rules

1. Use functional components
2. Define props types
```

## Quick Reference

### File Location

- **Directory**: `<cwd>/.forge/rules` (where `<cwd>` is current working directory)
- **Format**: `{rule-name}.md`
- **CRITICAL**: Rules MUST be in this exact location to be discovered by forge

### Frontmatter Fields

- `alwaysApply` - Apply to all conversations
- `globs` - File patterns to match
- `description` - Brief description
- `priority` - Importance (higher = more important)
- `enabled` - Active status

### Naming Rules

- Lowercase preferred
- Hyphens for multi-word names
- Descriptive and specific

## Integration with Claude Code

This rule system is inspired by Claude Code's rules design. If you have rules in `~/.claude/rules/`, they can be imported to `.forge/rules/` using the rules converter in forge_config_adapter.

### Importing Claude Rules

```bash
# Rules from ~/.claude/rules/ can be converted to .forge/rules/
# The converter is automatically invoked during Forge initialization
```

## Verification

After creating a rule:

1. **Verify the file location**: Ensure the file is in `<cwd>/.forge/rules` directory (CRITICAL - rules anywhere else will not be found)
2. Check YAML frontmatter is valid (use `---` delimiters)
3. Ensure rule content is meaningful and well-formatted
4. Verify the rule is recognized by forge

   ```bash
   # List all available rules
   forge list rule
   ```

5. Test that the rule applies correctly in relevant contexts

If your rule doesn't appear or doesn't apply correctly, check:

- **File location**: File MUST be in `<cwd>/.forge/rules` directory (this is the most common issue)
- Filename matches the expected pattern
- YAML frontmatter is properly formatted with `---` delimiters
- If using globs, ensure patterns are valid
- If using alwaysApply, ensure it's set to true for global rules