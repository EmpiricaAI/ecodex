use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use pretty_assertions::assert_eq;

use super::ConfigToml;

#[test]
fn runtime_provider_accepts_aws_profile_and_region_overrides() {
    let config = toml::from_str::<ConfigToml>(
        r#"
[model_providers.amazon-bedrock-runtime.aws]
profile = "runtime-profile"
region = "us-west-2"
"#,
    )
    .expect("Bedrock Runtime AWS overrides should deserialize");

    assert_eq!(
        config
            .model_providers
            .get(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID)
            .and_then(|provider| provider.aws.clone()),
        Some(ModelProviderAwsAuthInfo {
            profile: Some("runtime-profile".to_string()),
            region: Some("us-west-2".to_string()),
            credential_export: None,
            auth_refresh: None,
        })
    );
}

#[test]
fn custom_provider_still_rejects_aws_auth() {
    let error = toml::from_str::<ConfigToml>(
        r#"
[model_providers.custom]
name = "Custom"

[model_providers.custom.aws]
region = "us-west-2"
"#,
    )
    .expect_err("custom providers must not accept AWS auth");

    assert!(error.to_string().contains(
        "provider aws is only supported for `amazon-bedrock` or `amazon-bedrock-runtime`"
    ));
}

// Upstream documents overriding the built-in openai provider's catalog URL; the
// validator allows an `openai` entry that equals the default provider apart from
// `model_catalog_url`. ecodex's `supports_openai_builtin_tools` deserializes to
// true when omitted but derives false in `Default`, which made such an entry look
// like a full override and rejected the config at load.
#[test]
fn openai_entry_that_only_sets_model_catalog_url_is_accepted() {
    let config = toml::from_str::<ConfigToml>(
        r#"
[model_providers.openai]
model_catalog_url = "https://chatgpt.com/backend-api/codex/models"
"#,
    )
    .expect("an openai catalog override should deserialize");

    super::validate_reserved_model_provider_ids(&config.model_providers)
        .expect("overriding only the catalog URL of the built-in openai provider is allowed");
}

#[test]
fn openai_entry_that_changes_anything_else_is_still_rejected() {
    // The reserved-ID check runs while the config deserializes.
    let error = toml::from_str::<ConfigToml>(
        r#"
[model_providers.openai]
model_catalog_url = "https://chatgpt.com/backend-api/codex/models"
name = "Not OpenAI"
"#,
    )
    .expect_err("a real override of the built-in openai provider is rejected");

    assert!(
        error
            .to_string()
            .contains("reserved built-in provider IDs: `openai`")
    );
}

#[test]
fn openai_entry_cannot_switch_off_builtin_tools() {
    toml::from_str::<ConfigToml>(
        r#"
[model_providers.openai]
supports_openai_builtin_tools = false
"#,
    )
    .expect_err("the catalog-URL exception must not let the built-in tool flag be overridden");
}
