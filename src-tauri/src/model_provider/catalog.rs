use crate::protocol::model_provider::{
    ApiFormat, ConnectionPreset, ModelPreset, ModelProviderPreset, ReasoningEffort,
};

pub fn model_provider_presets() -> Vec<ModelProviderPreset> {
    vec![ModelProviderPreset {
        provider_name: "DeepSeek".into(),
        connections: vec![ConnectionPreset {
            api_format: ApiFormat::OpenAi,
            base_url: "https://api.deepseek.com/chat/completions".into(),
            models: vec![
                ModelPreset {
                    model_id: "deepseek-v4-flash".into(),
                    display_name: "V4 Flash".into(),
                    supports_thinking: true,
                    reasoning_efforts: vec![ReasoningEffort::High, ReasoningEffort::Max],
                },
                ModelPreset {
                    model_id: "deepseek-v4-pro".into(),
                    display_name: "V4 Pro".into(),
                    supports_thinking: true,
                    reasoning_efforts: vec![ReasoningEffort::High, ReasoningEffort::Max],
                },
            ],
        }],
    }]
}

#[cfg(test)]
mod tests {
    use crate::protocol::model_provider::ReasoningEffort;

    use super::model_provider_presets;

    #[test]
    fn describes_deepseek_model_reasoning_capabilities() {
        let presets = model_provider_presets();
        let models = &presets[0].connections[0].models;

        assert_eq!(models.len(), 2);
        assert!(models.iter().all(|model| model.supports_thinking));
        assert!(models.iter().all(|model| {
            model.reasoning_efforts == [ReasoningEffort::High, ReasoningEffort::Max]
        }));
    }
}
