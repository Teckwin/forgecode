use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use tokio::sync::Notify;

use crate::{ToolCallFull, ToolName, ToolResult};

#[derive(Debug, Clone, PartialEq)]
pub enum ChatResponseContent {
    // Should be only used to send tool input events.
    ToolInput(TitleFormat),
    // Should be only used to send tool outputs.
    ToolOutput(String),
    Markdown { text: String, partial: bool },
}

impl From<ChatResponseContent> for ChatResponse {
    fn from(content: ChatResponseContent) -> Self {
        ChatResponse::TaskMessage { content }
    }
}

impl From<TitleFormat> for ChatResponse {
    fn from(title: TitleFormat) -> Self {
        ChatResponse::TaskMessage { content: ChatResponseContent::ToolInput(title) }
    }
}

impl From<TitleFormat> for ChatResponseContent {
    fn from(title: TitleFormat) -> Self {
        ChatResponseContent::ToolInput(title)
    }
}

impl ChatResponseContent {
    pub fn contains(&self, needle: &str) -> bool {
        self.as_str().contains(needle)
    }

    pub fn as_str(&self) -> &str {
        match self {
            ChatResponseContent::ToolOutput(text) | ChatResponseContent::Markdown { text, .. } => {
                text
            }
            ChatResponseContent::ToolInput(_) => "",
        }
    }
}

/// Events that are emitted by the agent for external consumption. This includes
/// events for all internal state changes.
#[derive(Debug, Clone)]
pub enum ChatResponse {
    TaskMessage {
        content: ChatResponseContent,
    },
    TaskReasoning {
        content: String,
    },
    TaskComplete,
    ToolCallStart {
        tool_call: ToolCallFull,
        notifier: Arc<Notify>,
    },
    ToolCallEnd(ToolResult),
    RetryAttempt {
        cause: Cause,
        duration: Duration,
    },
    Interrupt {
        reason: InterruptionReason,
    },
}

impl ChatResponse {
    /// Returns `true` if the response contains no meaningful content.
    ///
    /// A response is considered empty if it's a `TaskMessage` or
    /// `TaskReasoning` with empty string content. All other variants are
    /// considered non-empty.
    pub fn is_empty(&self) -> bool {
        match self {
            ChatResponse::TaskMessage { content, .. } => match content {
                ChatResponseContent::ToolInput(_) => false,
                ChatResponseContent::ToolOutput(content) => content.is_empty(),
                ChatResponseContent::Markdown { text, .. } => text.is_empty(),
            },
            ChatResponse::TaskReasoning { content } => content.is_empty(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterruptionReason {
    MaxToolFailurePerTurnLimitReached {
        limit: u64,
        errors: HashMap<ToolName, usize>,
    },
    MaxRequestPerTurnLimitReached {
        limit: u64,
    },
}

#[derive(Clone)]
pub struct Cause(String);

impl Cause {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl From<&anyhow::Error> for Cause {
    fn from(value: &anyhow::Error) -> Self {
        Self(format!("{value:?}"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Category {
    Action,
    Info,
    Debug,
    Error,
    Completion,
    Warning,
}

#[derive(Clone, derive_setters::Setters, Debug, PartialEq)]
#[setters(into, strip_option)]
pub struct TitleFormat {
    pub title: String,
    pub sub_title: Option<String>,
    pub category: Category,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub trait TitleExt {
    fn title_fmt(&self) -> TitleFormat;
}

impl<T> TitleExt for T
where
    T: Into<TitleFormat> + Clone,
{
    fn title_fmt(&self) -> TitleFormat {
        self.clone().into()
    }
}

impl TitleFormat {
    /// Create a status for executing a tool
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Info,
            timestamp: Local::now().into(),
        }
    }

    /// Create a status for executing a tool
    pub fn action(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Action,
            timestamp: Local::now().into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Error,
            timestamp: Local::now().into(),
        }
    }

    pub fn debug(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Debug,
            timestamp: Local::now().into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            title: message.into(),
            sub_title: None,
            category: Category::Warning,
            timestamp: Local::now().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_title_format_with_timestamp() {
        let timestamp = DateTime::parse_from_rfc3339("2023-10-26T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let title = TitleFormat {
            title: "Test Action".to_string(),
            sub_title: Some("Subtitle".to_string()),
            category: Category::Action,
            timestamp,
        };

        assert_eq!(title.title, "Test Action");
        assert_eq!(title.sub_title, Some("Subtitle".to_string()));
        assert_eq!(title.category, Category::Action);
        assert_eq!(title.timestamp, timestamp);
    }

    /// Test: Duplicate messages cannot be distinguished
    ///
    /// This test demonstrates that the current ChatResponse implementation
    /// lacks unique message identifiers, making it impossible to detect
    /// duplicate messages. This is a root cause of the duplicate output bug.
    #[test]
    fn test_chat_response_lacks_unique_id_for_deduplication() {
        // Create two messages with identical content
        let content = ChatResponseContent::Markdown {
            text: "I should think step by step".to_string(),
            partial: false,
        };

        let msg1 = ChatResponse::TaskMessage { content: content.clone() };
        let msg2 = ChatResponse::TaskMessage { content };

        // Current behavior: Two messages with same content are equal
        // This makes it impossible to distinguish between original and duplicate
        // Using debug format to compare since PartialEq is not derived
        let msg1_debug = format!("{:?}", msg1);
        let msg2_debug = format!("{:?}", msg2);
        assert_eq!(
            msg1_debug, msg2_debug,
            "Messages with same content are equal"
        );

        // Problem: There's no way to track if msg2 is a duplicate of msg1
        // A proper fix would add a unique message_id field to ChatResponse
    }

    /// Test: TaskReasoning messages can be duplicated
    ///
    /// This test demonstrates that TaskReasoning messages (which include
    /// "I should think step by step") can be sent multiple times without
    /// any way to detect or prevent the duplication.
    #[test]
    fn test_task_reasoning_can_be_duplicated() {
        let reasoning_content = "I should think step by step".to_string();

        // Simulate the same reasoning being sent multiple times
        let messages: Vec<ChatResponse> = (0..3)
            .map(|_| ChatResponse::TaskReasoning { content: reasoning_content.clone() })
            .collect();

        // All messages have the same debug representation - no way to detect duplication
        let first_debug = format!("{:?}", messages[0]);
        for msg in &messages[1..] {
            let debug = format!("{:?}", msg);
            assert_eq!(
                first_debug, debug,
                "All reasoning messages have same content"
            );
        }

        // With a unique ID, we could track and deduplicate
        // let unique_ids: HashSet<_> = messages.iter().map(|m| m.id()).collect();
        // assert_eq!(unique_ids.len(), 1, "Should have only one unique message");
    }

    /// Test: Demonstrate the need for message deduplication in streams
    ///
    /// This test simulates a scenario where the same message might be
    /// processed multiple times in a stream, causing duplicate output.
    #[test]
    fn test_stream_needs_deduplication() {
        // Simulate messages that might be duplicated in a stream
        let original_content = ChatResponseContent::Markdown {
            text: "I should think step by step".to_string(),
            partial: false,
        };

        let messages = [
            ChatResponse::TaskMessage { content: original_content.clone() },
            ChatResponse::TaskMessage { content: original_content.clone() },
            ChatResponse::TaskComplete,
            ChatResponse::TaskMessage { content: original_content.clone() }, // Duplicate
        ];

        // Current: No way to filter duplicates based on unique IDs
        let task_messages: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m, ChatResponse::TaskMessage { .. }))
            .collect();

        // We get 3 task messages, but can't tell which are duplicates
        assert_eq!(
            task_messages.len(),
            3,
            "Cannot filter duplicates without unique IDs"
        );

        // Desired behavior with unique IDs:
        // let seen_ids = HashSet::new();
        // let unique: Vec<_> = messages
        //     .into_iter()
        //     .filter(|m| seen_ids.insert(m.id()))
        //     .collect();
    }
}
