//! AI model configuration and registry.

/// A model option for the AI settings picker.
pub struct ModelOption {
    pub id: &'static str,
    pub display_name: &'static str,
}

pub const AI_MODELS: &[ModelOption] = &[
    ModelOption {
        id: "google/gemini-3-flash-preview",
        display_name: "Gemini 3 Flash",
    },
    ModelOption {
        id: "google/gemini-2.5-pro-preview",
        display_name: "Gemini 2.5 Pro",
    },
    ModelOption {
        id: "anthropic/claude-sonnet-4",
        display_name: "Claude Sonnet 4",
    },
    ModelOption {
        id: "openai/gpt-4.1",
        display_name: "GPT-4.1",
    },
    ModelOption {
        id: "openai/o4-mini",
        display_name: "o4-mini",
    },
];

/// Resolve model ID to display name.
pub fn model_display_name(id: &str) -> &'static str {
    AI_MODELS
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.display_name)
        .unwrap_or("Unknown")
}

/// Resolve display name to model ID.
pub fn model_id_from_name(name: &str) -> &'static str {
    AI_MODELS
        .iter()
        .find(|m| m.display_name == name)
        .map(|m| m.id)
        .unwrap_or(AI_MODELS[0].id)
}
