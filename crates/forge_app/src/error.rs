use forge_domain::{InterruptionReason, ToolCallArgumentError, ToolName};

/// Domain-specific errors for Forge application
/// Each error type has clear semantic meaning for debugging and user feedback
#[derive(thiserror::Error, Debug)]
pub enum Error {
    // ========== Tool Errors ==========
    #[error("Invalid tool call arguments: {0}")]
    CallArgument(ToolCallArgumentError),

    #[error("Tool {0} not found")]
    NotFound(ToolName),

    #[error("Tool '{tool_name}' timed out after {timeout} minutes")]
    CallTimeout { tool_name: ToolName, timeout: u64 },

    #[error(
        "Tool '{name}' is not available. Please try again with one of these tools: [{supported_tools}]"
    )]
    NotAllowed {
        name: ToolName,
        supported_tools: String,
    },

    #[error(
        "Tool '{tool_name}' requires {required_modality} modality, but model only supports: {supported_modalities}"
    )]
    UnsupportedModality {
        tool_name: ToolName,
        required_modality: String,
        supported_modalities: String,
    },

    #[error("Empty tool response")]
    EmptyToolResponse,

    // ========== Agent Errors ==========
    #[error("Agent execution was interrupted: {0:?}")]
    AgentToolInterrupted(InterruptionReason),

    #[error("Authentication still in progress")]
    AuthInProgress,

    #[error("Agent '{0}' not found")]
    AgentNotFound(forge_domain::AgentId),

    #[error("Circular agent call detected: {agent_id} -> {chain}")]
    CircularAgentCall {
        agent_id: forge_domain::AgentId,
        chain: String,
    },

    #[error("Permission denied for operation: {operation}")]
    PermissionDenied { operation: String },

    // ========== Configuration Errors ==========
    #[error("No active provider configured")]
    NoActiveProvider,

    #[error("No active model configured")]
    NoActiveModel,

    // ========== File System Errors ==========
    #[error("Failed to read file '{path}': {source}")]
    FileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file '{path}': {source}")]
    FileWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to remove file '{path}': {source}")]
    FileRemove {
        path: String,
        #[source]
        source: std::io::Error,
    },

    // ========== Git Errors ==========
    #[error("Git operation failed: {operation} - {message}")]
    GitOperation { operation: String, message: String },

    #[error("No changes to commit")]
    GitNoChanges,

    // ========== Network Errors ==========
    #[error("HTTP request failed: {url} - {status}")]
    HttpRequest { url: String, status: u16 },

    #[error("Network fetch failed: {url}")]
    NetworkFetch { url: String },

    // ========== Authentication Errors ==========
    #[error("Authentication failed: {provider}")]
    AuthFailed { provider: String },

    #[error("Invalid API key for provider: {provider}")]
    InvalidApiKey { provider: String },

    // ========== Conversation Errors ==========
    #[error("Conversation not found: {0}")]
    ConversationNotFound(forge_domain::ConversationId),

    #[error("Failed to generate conversation title")]
    TitleGenerationFailed,

    // ========== Workspace Errors ==========
    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(forge_domain::WorkspaceId),

    #[error("Failed to initialize workspace: {path}")]
    WorkspaceInit { path: String },

    // ========== Template Errors ==========
    #[error("Template rendering failed: {0}")]
    TemplateRender(String),
}
