---
name: create-tool
description: Create new tools for the code-forge application. Tools are stored as TOOL.md files in the <cwd>/.forge/tools directory with YAML frontmatter (name, overview) and markdown body containing tool specifications. Use when users need to add new tools, modify existing tools, or understand the tool file structure.
---

# Create Tools

Create and manage dynamic tools for the code-forge application. Tools are executable actions that can be invoked to perform specific tasks.

## File Location

**CRITICAL**: All tool files must be created in the `<cwd>/.forge/tools` directory, where `<cwd>` is the current working directory of your code-forge project.

- **Directory**: `<cwd>/.forge/tools`
- **File format**: `{tool-name}/TOOL.md`
- **Example**: If your project is at `/home/user/my-project`, tools go in `/home/user/my-project/.forge/tools/`

This is the only location where forge will discover and load custom tools.

## Tool File Structure

Every tool file must have:

1. **YAML Frontmatter** (required):
   - `name`: Unique tool identifier
   - `overview`: Brief description of what the tool does

2. **Tool Body** (required):
   - Overview section
   - Scope (applicable and non-applicable scenarios)
   - Usage section (communication mode, parameters, return format)
   - Example usage

### Example Tool File

```markdown
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

### Complete Sample Tool

This sample demonstrates a complete tool structure:

```markdown
---
name: http-api-tool
overview: Tool for making HTTP API requests with authentication support
---

# HTTP API Tool

A tool that enables making HTTP requests to external APIs with built-in authentication handling.

## Overview

This tool provides a simple interface for calling REST APIs with automatic JWT token refresh and retry logic.

## Scope

### Applicable Scenarios

- Calling external REST APIs
- Integrating with third-party services
- Handling authenticated API requests

### Non-Applicable Scenarios

- Streaming real-time data (use stream mode)
- WebSocket connections (use socket mode)
- File system operations (use built-in file tools)

## Usage

### Communication Mode

- **Mode**: http
- **Endpoint**: The base URL for the API

### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| url | string | Yes | Full URL to request |
| method | string | Yes | HTTP method (GET, POST, PUT, DELETE) |
| headers | object | No | Custom headers as key-value pairs |
| body | object | No | Request body for POST/PUT |
| timeout | number | No | Request timeout in milliseconds (default: 30000) |

### Return Format

```json
{
  "success": boolean,
  "data": {
    "status": number,
    "headers": {},
    "body": {}
  },
  "error": string | null
}
```

## Example Usage

```markdown
{{#tool_call}}
http-api-tool --url "https://api.example.com/users" --method "GET" --headers '{"Authorization": "Bearer token123"}'
{{/tool_call}}
```
```

## Tool Communication Modes

### stdio Mode

Tools that execute local commands via standard I/O:

```markdown
### Communication Mode

- **Mode**: stdio
- **Command**: The shell command to execute
```

### http Mode

Tools that make HTTP requests:

```markdown
### Communication Mode

- **Mode**: http
- **Endpoint**: https://api.example.com/v1/
```

### stream Mode

Tools that handle streaming data:

```markdown
### Communication Mode

- **Mode**: stream
- **Endpoint**: wss://stream.example.com/events
```

### socket Mode

Tools that use WebSocket connections:

```markdown
### Communication Mode

- **Mode**: socket
- **Endpoint**: ws://localhost:8080/ws
```

## Creating a New Tool

### Step 1: Determine Tool Purpose

Identify what the tool should accomplish:

- What task will it perform?
- What parameters does it need?
- What should it return?
- Which communication mode is appropriate?

### Step 2: Choose Tool Name

Use lowercase with hyphens for multi-word names:

- Good: `http-api`, `file-transform`, `data-export`
- Bad: `HttpAPI`, `file_transform`, `DataExport`

### Step 3: Write the Tool File

Create the file in the `<cwd>/.forge/tools/{tool-name}/TOOL.md` directory.

**IMPORTANT**: The file MUST be in `<cwd>/.forge/tools` where `<cwd>` is your current working directory. Tools placed anywhere else will not be discovered by forge.

#### Frontmatter

```yaml
---
name: your-tool-name
overview: Clear, concise description of what this tool does
---
```

#### Tool Body

Use the standard sections:
- Overview
- Scope (Applicable/Non-Applicable Scenarios)
- Usage (Communication Mode, Parameters, Return Format)
- Example Usage

## Parameter Types

| Type | Description | Example |
|------|-------------|---------|
| string | Text value | `"hello"` |
| number | Numeric value | `123`, `45.67` |
| boolean | True/false | `true`, `false` |
| object | JSON object | `{"key": "value"}` |
| array | JSON array | `["a", "b", "c"]` |

## Quick Reference

### File Location

- **Directory**: `<cwd>/.forge/tools` (where `<cwd>` is current working directory)
- **Format**: `{tool-name}/TOOL.md`
- **CRITICAL**: Tools MUST be in this exact location to be discovered by forge

### Required Sections

- YAML frontmatter (name, overview)
- Overview section
- Scope section (Applicable/Non-Applicable)
- Usage section (Communication Mode, Parameters, Return Format)
- Example Usage

### Communication Modes

- `stdio` - Local command execution
- `http` - HTTP REST API calls
- `stream` - Streaming data (Server-Sent Events)
- `socket` - WebSocket connections

### Naming Rules

- Lowercase only
- Hyphens for multi-word names
- Keep it short but descriptive
- Use noun-based names

## Verification

After creating a tool:

1. **Verify the file location**: Ensure the file is in `<cwd>/.forge/tools/{tool-name}/TOOL.md` directory (CRITICAL - tools anywhere else will not be found)
2. Check YAML frontmatter is valid (use `---` delimiters)
3. Ensure all required sections are present
4. Verify the tool is recognized by forge

   ```bash
   # List all available tools
   forge list tool
   ```

5. Test the tool to ensure it works as expected
6. Verify parameters and return format are correctly documented

If your tool doesn't appear in the list, check:

- **File location**: File MUST be in `<cwd>/.forge/tools` directory (this is the most common issue)
- Directory name matches the `name` field in frontmatter
- YAML frontmatter is properly formatted with `---` delimiters
- Both `name` and `overview` fields are present
- All required sections are included in the body