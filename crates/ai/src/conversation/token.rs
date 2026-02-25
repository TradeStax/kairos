//! Token usage tracking and estimation.

use serde::{Deserialize, Serialize};

/// Accumulated token usage for a conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Add usage from another round.
    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }

    /// Create from API usage response.
    pub fn from_api(
        usage: &crate::client::response::Usage,
    ) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }
    }

    /// Format for display (e.g. "12,450 tokens").
    pub fn format_display(&self) -> String {
        format_with_commas(self.total_tokens)
            + " tokens"
    }
}

/// Rough token estimation: ~4 chars per token for English text.
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32 + 3) / 4
}

/// Format an integer with comma separators.
fn format_with_commas(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        // 4 chars per token
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(
            estimate_tokens("Hello, world!"),
            // 13 chars -> (13 + 3) / 4 = 4
            4
        );
    }

    #[test]
    fn test_format_with_commas() {
        assert_eq!(format_with_commas(0), "0");
        assert_eq!(format_with_commas(999), "999");
        assert_eq!(format_with_commas(1000), "1,000");
        assert_eq!(format_with_commas(12450), "12,450");
        assert_eq!(format_with_commas(1234567), "1,234,567");
    }

    #[test]
    fn test_token_usage_add() {
        let mut a = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        let b = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 80,
            total_tokens: 280,
        };
        a.add(&b);
        assert_eq!(a.prompt_tokens, 300);
        assert_eq!(a.completion_tokens, 130);
        assert_eq!(a.total_tokens, 430);
    }
}
