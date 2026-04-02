use std::collections::HashSet;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::Result;
use futures::Stream;
use uuid::Uuid;

use forge_domain::ChatResponse;
use forge_stream::MpscStream;

/// A stream wrapper that filters out duplicate ChatResponse messages.
///
/// This prevents duplicate outputs like "I should think step by step..." that can
/// occur in long-running sessions where the same reasoning content might be sent
/// multiple times by the LLM.
pub struct ChatResponseDeduplicator {
    stream: MpscStream<Result<ChatResponse>>,
    seen_task_message_ids: HashSet<Uuid>,
    seen_reasoning_ids: HashSet<Uuid>,
}

impl ChatResponseDeduplicator {
    /// Creates a new ChatResponse deduplicating stream.
    pub fn new(stream: MpscStream<Result<ChatResponse>>) -> Self {
        Self { stream, seen_task_message_ids: HashSet::new(), seen_reasoning_ids: HashSet::new() }
    }

    /// Consumes the deduplicator and returns the inner stream.
    pub fn into_inner(self) -> MpscStream<Result<ChatResponse>> {
        self.stream
    }

    /// Returns the number of unique task messages seen so far.
    pub fn unique_task_message_count(&self) -> usize {
        self.seen_task_message_ids.len()
    }

    /// Returns the number of unique reasoning messages seen so far.
    pub fn unique_reasoning_count(&self) -> usize {
        self.seen_reasoning_ids.len()
    }
}

impl Stream for ChatResponseDeduplicator {
    type Item = Result<ChatResponse>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.stream).poll_next(cx) {
                Poll::Ready(Some(Ok(message))) => {
                    // Check if this message is a duplicate based on its type and message_id
                    let is_duplicate = match &message {
                        ChatResponse::TaskMessage { message_id, .. } => {
                            // insert returns false if the ID was already present
                            !self.seen_task_message_ids.insert(*message_id)
                        }
                        ChatResponse::TaskReasoning { message_id, .. } => {
                            !self.seen_reasoning_ids.insert(*message_id)
                        }
                        // TaskComplete and other variants don't have IDs, always pass through
                        _ => false,
                    };

                    if is_duplicate {
                        tracing::debug!("Filtering out duplicate ChatResponse: {:?}", message);
                        continue;
                    }

                    return Poll::Ready(Some(Ok(message)));
                }
                // Pass through errors and None
                other => return other,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use futures::StreamExt;
    use uuid::Uuid;

    use super::*;
    use forge_domain::ChatResponseContent;

    fn create_task_message(id: Uuid, text: &str) -> ChatResponse {
        ChatResponse::TaskMessage {
            message_id: id,
            content: ChatResponseContent::Markdown { text: text.to_string(), partial: false },
        }
    }

    fn create_task_reasoning(id: Uuid, text: &str) -> ChatResponse {
        ChatResponse::TaskReasoning { message_id: id, content: text.to_string() }
    }

    #[tokio::test]
    async fn test_deduplicator_passes_through_unique_messages() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let stream = MpscStream::spawn(move |tx| async move {
            tx.send(Ok(create_task_message(id1, "First message"))).await.unwrap();
            tx.send(Ok(create_task_message(id2, "Second message"))).await.unwrap();
        });

        let mut dedup = ChatResponseDeduplicator::new(stream);

        let msg1 = dedup.next().await.unwrap().unwrap();
        let msg2 = dedup.next().await.unwrap().unwrap();

        if let ChatResponse::TaskMessage { message_id, .. } = msg1 {
            assert_eq!(message_id, id1);
        } else {
            panic!("Expected TaskMessage");
        }

        if let ChatResponse::TaskMessage { message_id, .. } = msg2 {
            assert_eq!(message_id, id2);
        } else {
            panic!("Expected TaskMessage");
        }

        assert_eq!(dedup.unique_task_message_count(), 2);
    }

    #[tokio::test]
    async fn test_deduplicator_filters_duplicate_messages() {
        let id1 = Uuid::new_v4();

        // Create two messages with the same ID
        let stream = MpscStream::spawn(move |tx| async move {
            tx.send(Ok(create_task_message(id1, "First occurrence"))).await.unwrap();
            tx.send(Ok(create_task_message(id1, "Duplicate"))).await.unwrap();
            tx.send(Ok(ChatResponse::TaskComplete)).await.unwrap();
        });

        let mut dedup = ChatResponseDeduplicator::new(stream);

        // Should get first message
        let msg1 = dedup.next().await.unwrap().unwrap();
        if let ChatResponse::TaskMessage { content, .. } = msg1 {
            assert!(matches!(content, ChatResponseContent::Markdown { text, .. } if text == "First occurrence"));
        }

        // Should skip duplicate and get TaskComplete
        let msg2 = dedup.next().await.unwrap().unwrap();
        assert!(matches!(msg2, ChatResponse::TaskComplete));

        // No more messages
        let msg3 = dedup.next().await;
        assert!(msg3.is_none());

        assert_eq!(dedup.unique_task_message_count(), 1);
    }

    #[tokio::test]
    async fn test_deduplicator_filters_duplicate_reasoning() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Create messages with duplicate reasoning content but different IDs
        let stream = MpscStream::spawn(move |tx| async move {
            tx.send(Ok(create_task_reasoning(id1, "I should think step by step"))).await.unwrap();
            // Different ID - this is NOT a duplicate
            tx.send(Ok(create_task_reasoning(id2, "I should think step by step"))).await.unwrap();
        });

        let mut dedup = ChatResponseDeduplicator::new(stream);

        // Both should pass through since they have different IDs
        let msg1 = dedup.next().await.unwrap().unwrap();
        let msg2 = dedup.next().await.unwrap().unwrap();

        assert!(matches!(msg1, ChatResponse::TaskReasoning { .. }));
        assert!(matches!(msg2, ChatResponse::TaskReasoning { .. }));

        assert_eq!(dedup.unique_reasoning_count(), 2);
    }

    #[tokio::test]
    async fn test_deduplicator_task_complete_always_passes() {
        let stream = MpscStream::spawn(|tx| async move {
            tx.send(Ok(ChatResponse::TaskComplete)).await.unwrap();
            tx.send(Ok(ChatResponse::TaskComplete)).await.unwrap();
            tx.send(Ok(ChatResponse::TaskComplete)).await.unwrap();
        });

        let mut dedup = ChatResponseDeduplicator::new(stream);

        // TaskComplete has no message_id, so it should always pass through
        let count = (&mut dedup).collect::<Vec<_>>().await.len();
        assert_eq!(count, 3);
    }
}