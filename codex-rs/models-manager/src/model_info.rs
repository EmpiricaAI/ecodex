use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::openai_models::default_input_modalities;

use crate::config::ModelsManagerConfig;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use tracing::warn;

/// ecodex's singular base prompt. Replaces (does not concat with) the
/// upstream codex prompt.md.
///
/// Useful operational sections from the upstream prompt (apply_patch,
/// update_plan, shell guidelines, coding guidelines, validation
/// philosophy, formatting rules) have been absorbed into
/// prompt-empirica.md under the "Surface — the ecodex CLI" chapter,
/// integrated within the empirica frame rather than appended as a
/// separate document.
///
/// The upstream `prompt.md` is intentionally retained on disk for
/// upstream-merge tracking — when openai/codex evolves its prompt, the
/// diff against our retained copy shows what to fold into
/// prompt-empirica.md. It is NOT loaded by ecodex at runtime.
pub const BASE_INSTRUCTIONS: &str = include_str!("../prompt-empirica.md");
const DEFAULT_PERSONALITY_HEADER: &str = "You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals.";
const LOCAL_FRIENDLY_TEMPLATE: &str =
    "You optimize for team morale and being a supportive teammate as much as code quality.";
const LOCAL_PRAGMATIC_TEMPLATE: &str = "You are a deeply pragmatic, effective software engineer.";
const PERSONALITY_PLACEHOLDER: &str = "{{ personality }}";
const PERSONALITY_SECTION_HEADER: &str = "# Personality";

pub fn with_config_overrides(mut model: ModelInfo, config: &ModelsManagerConfig) -> ModelInfo {
    if let Some(context_window) = config.model_context_window {
        model.context_window = Some(
            model
                .max_context_window
                .map_or(context_window, |max_context_window| {
                    context_window.min(max_context_window)
                }),
        );
    }
    if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
        model.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    if let Some(token_limit) = config.tool_output_token_limit {
        model.truncation_policy = match model.truncation_policy.mode {
            TruncationMode::Bytes => {
                let byte_limit =
                    i64::try_from(approx_bytes_for_tokens(token_limit)).unwrap_or(i64::MAX);
                TruncationPolicyConfig::bytes(byte_limit)
            }
            TruncationMode::Tokens => {
                let limit = i64::try_from(token_limit).unwrap_or(i64::MAX);
                TruncationPolicyConfig::tokens(limit)
            }
        };
    }

    if let Some(base_instructions) = &config.base_instructions {
        model.base_instructions = base_instructions.clone();
        clear_instruction_messages(&mut model);
    } else {
        if config.personality_enabled && config.personality == Some(Personality::None) {
            model.base_instructions = strip_personality_section(model.base_instructions);
            if let Some(instructions_template) = model
                .model_messages
                .as_mut()
                .and_then(|messages| messages.instructions_template.as_mut())
            {
                *instructions_template =
                    strip_personality_section(std::mem::take(instructions_template));
            }
        }
        if !config.personality_enabled {
            clear_instruction_messages(&mut model);
        }
    }

    model
}

fn strip_personality_section(mut instructions: String) -> String {
    let mut section_start = None;
    let mut section_end = None;
    let mut offset = 0;

    for line_with_ending in instructions.split_inclusive('\n') {
        let line = match line_with_ending.strip_suffix('\n') {
            Some(line) => line.strip_suffix('\r').unwrap_or(line),
            None => line_with_ending,
        };
        if section_start.is_some() {
            if is_h1_heading(line) {
                section_end = Some(offset);
                break;
            }
        } else if line == PERSONALITY_SECTION_HEADER {
            section_start = Some(offset);
        }
        offset += line_with_ending.len();
    }

    if let Some(section_start) = section_start {
        let section_end = section_end.unwrap_or(instructions.len());
        instructions.replace_range(section_start..section_end, "");
    }

    instructions
}

fn is_h1_heading(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('#') else {
        return false;
    };
    rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')
}

fn clear_instruction_messages(model: &mut ModelInfo) {
    if let Some(model_messages) = &mut model.model_messages {
        model_messages.instructions_template = None;
        model_messages.instructions_variables = None;
        if model_messages.approvals.is_none()
            && model_messages.auto_review.is_none()
            && model_messages.permissions.is_none()
        {
            model.model_messages = None;
        }
    }
}

/// Build a minimal fallback model descriptor for missing/unknown slugs.
///
/// ecodex extension: before falling back, check whether the slug matches a
/// known open-weights model family (qwen, llama, mistral, gemma, deepseek,
/// gpt-oss, mirothinker, qwopus, kimi, glm, ollama-prefixed, etc.). When
/// matched, return a tuned descriptor with `used_fallback_model_metadata`
/// set to false — so the runtime doesn't emit the "Model metadata for X
/// not found" warning for models we explicitly recognize as a family even
/// if the exact tag isn't in models.json. Context windows are conservative
/// per-family defaults; tuning them per-tag would require maintaining a
/// table per Ollama upstream.
pub fn model_info_from_slug(slug: &str) -> ModelInfo {
    // Layer 1: exact-slug curated entry (bundled seed ∪ ~/.codex/models.user.json).
    // Most specific — wins over the family-prefix table below.
    if let Some(entry) = crate::curated_seed::lookup(slug) {
        return crate::curated_seed::enrich(build_fallback_model_info(slug), &entry);
    }
    // Layer 2: family-prefix recognition (legacy conservative fallback).
    let mut info = build_fallback_model_info(slug);
    if let Some(known) = recognize_open_weights_family(slug) {
        info.context_window = Some(known.context_window);
        info.max_context_window = Some(known.context_window);
        info.display_name = known.display_name;
        info.description = Some(known.family_description);
        info.used_fallback_model_metadata = false;
        info
    } else {
        warn!("Unknown model {slug} is used. This will use fallback model metadata.");
        info
    }
}

fn build_fallback_model_info(slug: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: None,
        default_reasoning_level: None,
        supported_reasoning_levels: Vec::new(),
        shell_type: ConfigShellToolType::Default,
        visibility: ModelVisibility::None,
        supported_in_api: true,
        priority: 99,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        availability_nux: None,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        model_messages: local_personality_messages_for_slug(slug),
        include_skills_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_parallel_tool_calls: false,
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: Some(272_000),
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: true, // this is the fallback model metadata
        supports_search_tool: false,
        use_responses_lite: false,
        auto_review_model_override: None,
        tool_mode: None,
        multi_agent_version: None,
    }
}

/// Tuning known to ecodex's target audience (open-weights operators).
/// Match is case-insensitive prefix on the slug, after stripping any
/// `<provider>/` namespace and trimming optional `:tag`. Conservative
/// context_window defaults — when the actual tag differs, the user can
/// override via `[model_providers.X.context_window]` in config.toml.
struct KnownOpenWeightsFamily {
    display_name: String,
    family_description: String,
    /// Tokens, conservative to avoid exceeding the model's actual capacity.
    context_window: i64,
}

/// True if `slug` matches a known coding/open-weights family (the prefix table)
/// OR an exact curated-seed entry. Used by `ecodex models refresh` to keep the
/// discovered registry epistemically relevant — a provider's `/v1/models` lists
/// hundreds of embeddings/audio/image models we do not want in the picker.
pub fn is_recognized_coding_model(slug: &str) -> bool {
    crate::curated_seed::lookup(slug).is_some() || recognize_open_weights_family(slug).is_some()
}

fn recognize_open_weights_family(slug: &str) -> Option<KnownOpenWeightsFamily> {
    let normalized = slug
        .rsplit_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(slug)
        .split(':')
        .next()
        .unwrap_or(slug)
        .to_ascii_lowercase();

    // Match order matters: more-specific prefixes must come before more-general
    // ones. qwen3-coder must be checked before qwen, otherwise the bare "qwen"
    // branch would shadow it and assign the small 32K default.
    let (display, family, ctx) = match () {
        _ if normalized.starts_with("qwen3-coder")
            || normalized.starts_with("qwen-coder")
            || normalized.starts_with("qwen2.5-coder")
            || normalized.starts_with("qwencoder") =>
        {
            (
                "Qwen Coder",
                "Qwen-Coder open-weights model (Alibaba) — purpose-built for coding agents, 256K native context",
                262_144,
            )
        }
        _ if normalized.starts_with("qwen3-next") => (
            "Qwen3-Next",
            "Qwen3-Next open-weights model (Alibaba) — extended-context variant, 256K native",
            262_144,
        ),
        _ if normalized.starts_with("qwen2.5") => (
            "Qwen2.5",
            "Qwen2.5-family open-weights model (Alibaba) — 128K native context",
            131_072,
        ),
        _ if normalized.starts_with("qwen") || normalized.starts_with("qwopus") => {
            // Generic Qwen / Qwopus fallback — Qwen3 base / community distills
            // typically max out at 32K without YaRN extension.
            (
                "Qwen / Qwopus",
                "Qwen-family open-weights model (Alibaba)",
                32_768,
            )
        }
        _ if normalized.starts_with("llama") => {
            ("Llama", "Llama-family open-weights model (Meta)", 128_000)
        }
        _ if normalized.starts_with("mistral") || normalized.starts_with("mixtral") => (
            "Mistral / Mixtral",
            "Mistral-family open-weights model",
            32_768,
        ),
        _ if normalized.starts_with("gemma") => {
            ("Gemma", "Gemma open-weights model (Google)", 8_192)
        }
        _ if normalized.starts_with("deepseek") => {
            ("DeepSeek", "DeepSeek open-weights model", 128_000)
        }
        _ if normalized.starts_with("gpt-oss") => ("GPT-OSS", "GPT-OSS open-weights model", 32_768),
        _ if normalized.starts_with("mirothinker") || normalized.starts_with("huihui") => (
            "MiroThinker",
            "MiroThinker reasoning open-weights model",
            32_768,
        ),
        _ if normalized.starts_with("kimi") => {
            ("Kimi", "Moonshot Kimi open-weights model", 200_000)
        }
        _ if normalized.starts_with("glm") => ("GLM", "Zhipu GLM open-weights model", 32_768),
        _ if normalized.starts_with("phi") => ("Phi", "Phi open-weights model (Microsoft)", 16_384),
        _ => return None,
    };

    Some(KnownOpenWeightsFamily {
        display_name: format!("{display} ({slug})"),
        family_description: family.to_string(),
        context_window: ctx,
    })
}

fn local_personality_messages_for_slug(slug: &str) -> Option<ModelMessages> {
    match slug {
        "gpt-5.2-codex" | "exp-codex-personality" => Some(ModelMessages {
            instructions_template: Some(format!(
                "{DEFAULT_PERSONALITY_HEADER}\n\n{PERSONALITY_PLACEHOLDER}\n\n{BASE_INSTRUCTIONS}"
            )),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: Some(String::new()),
                personality_friendly: Some(LOCAL_FRIENDLY_TEMPLATE.to_string()),
                personality_pragmatic: Some(LOCAL_PRAGMATIC_TEMPLATE.to_string()),
            }),
            approvals: None,
            auto_review: None,
            permissions: None,
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "model_info_tests.rs"]
mod tests;
