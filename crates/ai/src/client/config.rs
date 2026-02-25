/// Client configuration for the OpenRouter API.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// OpenRouter API key.
    pub api_key: String,
    /// Base URL (default `https://openrouter.ai/api/v1`).
    pub base_url: String,
    /// Model identifier (OpenRouter format, e.g.
    /// `google/gemini-3-flash-preview`).
    pub model: String,
    /// Maximum tokens in the completion response.
    pub max_tokens: u32,
    /// Sampling temperature (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
    /// HTTP request timeout in seconds.
    pub timeout_secs: u64,
}

impl ClientConfig {
    /// Create a new config with sensible defaults.
    pub fn new(api_key: String) -> Self {
        let default_model = AiModel::default_model();
        Self {
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            model: default_model.id,
            max_tokens: 8192,
            temperature: 0.3,
            timeout_secs: 120,
        }
    }
}

/// Describes an AI model available through OpenRouter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiModel {
    /// OpenRouter model identifier
    /// (e.g. `google/gemini-3-flash-preview`).
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
}

impl AiModel {
    /// All models currently offered in the Kairos model picker.
    pub fn available_models() -> Vec<AiModel> {
        vec![
            AiModel {
                id: "google/gemini-3-flash-preview".into(),
                display_name: "Gemini 3 Flash".into(),
            },
            AiModel {
                id: "google/gemini-2.5-pro-preview".into(),
                display_name: "Gemini 2.5 Pro".into(),
            },
            AiModel {
                id: "anthropic/claude-sonnet-4".into(),
                display_name: "Claude Sonnet 4".into(),
            },
            AiModel {
                id: "openai/gpt-4.1".into(),
                display_name: "GPT-4.1".into(),
            },
            AiModel {
                id: "openai/o4-mini".into(),
                display_name: "o4-mini".into(),
            },
        ]
    }

    /// The default model for new conversations.
    pub fn default_model() -> AiModel {
        AiModel {
            id: "google/gemini-3-flash-preview".into(),
            display_name: "Gemini 3 Flash".into(),
        }
    }
}
