---
name: example-tool
overview: Example tool demonstrating the TOOL.md format specification
---

# Example Tool

This is an example tool definition file demonstrating the required format for Forge dynamic tools.

## Overview

A brief description of what this tool does and its purpose.

## Scope

### Applicable Scenarios

- Use case 1 where this tool is appropriate
- Use case 2 where this tool is appropriate

### Non-Applicable Scenarios

- Scenario 1 where this tool should not be used
- Scenario 2 where this tool should not be used

## Usage

### Communication Mode

- **Mode**: stdio | http | stream | socket
- **Command**: The command to execute (for stdio mode)
- **Endpoint**: The URL endpoint (for http/stream mode)

### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| param1 | string | Yes | Description of param1 |
| param2 | number | No | Description of param2 |

### Return Format

```json
{
  "success": boolean,
  "data": {},
  "error": string | null
}
```

## Example Usage

```markdown
{{#tool_call}}
example-tool --param1 "value1" --param2 123
{{/tool_call}}
```