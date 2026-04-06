//! Code file chunking strategies.

/// Configuration for the code chunker.
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Maximum number of lines per chunk.
    pub chunk_size: usize,
    /// Number of overlapping lines between adjacent chunks.
    pub overlap: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self { chunk_size: 50, overlap: 20 }
    }
}

/// A single chunk of code with line range information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeChunk {
    /// The chunk content (lines joined with newlines).
    pub content: String,
    /// 1-based start line in the original file.
    pub start_line: u32,
    /// 1-based end line in the original file (inclusive).
    pub end_line: u32,
}

/// Splits source code into overlapping chunks.
pub struct Chunker {
    config: ChunkConfig,
}

impl Chunker {
    pub fn new(config: ChunkConfig) -> Self {
        Self { config }
    }

    /// Split file content into chunks.
    ///
    /// Uses a sliding window approach with overlap. Attempts to break at
    /// natural boundaries (empty lines) when possible.
    pub fn chunk(&self, content: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return vec![];
        }

        // For small files, return a single chunk
        if lines.len() <= self.config.chunk_size {
            return vec![CodeChunk {
                content: content.to_string(),
                start_line: 1,
                end_line: lines.len() as u32,
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < lines.len() {
            let mut end = (start + self.config.chunk_size).min(lines.len());

            // Try to find a natural break point (empty line) near the end
            if end < lines.len() {
                let search_start = if end > 5 { end - 5 } else { start };
                for i in (search_start..end).rev() {
                    if lines[i].trim().is_empty() {
                        end = i + 1;
                        break;
                    }
                }
            }

            let chunk_content = lines[start..end].join("\n");
            chunks.push(CodeChunk {
                content: chunk_content,
                start_line: (start + 1) as u32,
                end_line: end as u32,
            });

            // Advance by (chunk_end - overlap), ensuring we make progress
            let advance = if end - start > self.config.overlap {
                end - start - self.config.overlap
            } else {
                1
            };
            start += advance;

            // If remaining lines would be too small, merge with last
            if start < lines.len() && lines.len() - start < self.config.overlap {
                let last = chunks.last_mut().unwrap();
                let remaining = lines[last.end_line as usize..].join("\n");
                if !remaining.is_empty() {
                    last.content.push('\n');
                    last.content.push_str(&remaining);
                }
                last.end_line = lines.len() as u32;
                break;
            }
        }

        chunks
    }
}

impl Default for Chunker {
    fn default() -> Self {
        Self::new(ChunkConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_file_single_chunk() {
        let chunker = Chunker::default();
        let content = "line1\nline2\nline3";
        let chunks = chunker.chunk(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
    }

    #[test]
    fn test_empty_content() {
        let chunker = Chunker::default();
        assert!(chunker.chunk("").is_empty());
    }

    #[test]
    fn test_large_file_multiple_chunks() {
        let chunker = Chunker::new(ChunkConfig { chunk_size: 10, overlap: 3 });
        let lines: Vec<String> = (1..=30).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let chunks = chunker.chunk(&content);

        assert!(chunks.len() > 1);
        // First chunk starts at line 1
        assert_eq!(chunks[0].start_line, 1);
        // Last chunk ends at the last line
        assert_eq!(chunks.last().unwrap().end_line, 30);
    }

    #[test]
    fn test_overlapping_chunks() {
        let chunker = Chunker::new(ChunkConfig { chunk_size: 10, overlap: 3 });
        let lines: Vec<String> = (1..=25).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let chunks = chunker.chunk(&content);

        // Verify overlap: end of chunk N should overlap start of chunk N+1
        for i in 0..chunks.len() - 1 {
            assert!(
                chunks[i].end_line >= chunks[i + 1].start_line,
                "Chunk {} (end={}) should overlap with chunk {} (start={})",
                i,
                chunks[i].end_line,
                i + 1,
                chunks[i + 1].start_line,
            );
        }
    }
}
