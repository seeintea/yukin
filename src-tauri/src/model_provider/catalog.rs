use crate::protocol::model_provider::{
    ApiFormat, ConnectionPreset, ModelPreset, ModelProviderPreset, ReasoningEffort,
};

pub fn model_provider_presets() -> Vec<ModelProviderPreset> {
    vec![ModelProviderPreset {
        provider_key: "deepseek".into(),
        display_name: "DeepSeek".into(),
        connections: vec![ConnectionPreset {
            api_format: ApiFormat::OpenAi,
            base_url: "https://api.deepseek.com/chat/completions".into(),
            models: vec![
                ModelPreset {
                    model_id: "deepseek-v4-flash".into(),
                    display_name: "DeepSeek V4 Flash".into(),
                    supports_thinking: true,
                    reasoning_efforts: vec![
                        ReasoningEffort::Low,
                        ReasoningEffort::High,
                        ReasoningEffort::Max,
                    ],
                },
                ModelPreset {
                    model_id: "deepseek-v4-pro".into(),
                    display_name: "DeepSeek V4 Pro".into(),
                    supports_thinking: true,
                    reasoning_efforts: vec![
                        ReasoningEffort::Low,
                        ReasoningEffort::High,
                        ReasoningEffort::Max,
                    ],
                },
            ],
        }],
    }]
}

pub fn find_model_provider_preset(provider_key: &str) -> Option<ModelProviderPreset> {
    model_provider_presets()
        .into_iter()
        .find(|preset| preset.provider_key == provider_key)
}

#[cfg(test)]
mod tests {
    use crate::protocol::model_provider::ReasoningEffort;

    use super::{find_model_provider_preset, model_provider_presets};

    #[test]
    fn finds_provider_by_stable_key() {
        let preset = find_model_provider_preset("deepseek").expect("DeepSeek preset");

        assert_eq!(preset.display_name, "DeepSeek");
        assert!(find_model_provider_preset("DeepSeek").is_none());
    }

    #[test]
    fn describes_deepseek_model_reasoning_capabilities() {
        let presets = model_provider_presets();
        let models = &presets[0].connections[0].models;

        assert_eq!(models.len(), 2);
        assert!(models.iter().all(|model| model.supports_thinking));
        assert!(models.iter().all(|model| {
            model.reasoning_efforts
                == [
                    ReasoningEffort::Low,
                    ReasoningEffort::High,
                    ReasoningEffort::Max,
                ]
        }));
    }
}
