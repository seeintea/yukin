use crate::protocol::model_provider::{
    ApiFormat, ConnectionPreset, ModelPreset, ModelProviderPreset,
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
                },
                ModelPreset {
                    model_id: "deepseek-v4-pro".into(),
                    display_name: "V4 Pro".into(),
                },
            ],
        }],
    }]
}
