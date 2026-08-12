use std::collections::HashSet;
use std::sync::Arc;

use crate::SkillLoadOutcome;
use crate::SkillMetadata;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::TrackEventsContext;
use codex_exec_server::LOCAL_FS;
use codex_otel::SessionTelemetry;
use codex_otel::sanitize_metric_tag_value;
use codex_utils_output_truncation::approx_token_count;
pub use codex_skills::ToolMentionKind;
pub use codex_skills::ToolMentions;
pub use codex_skills::app_id_from_path;
pub use codex_skills::extract_tool_mentions;
pub use codex_skills::extract_tool_mentions_with_sigil;
pub use codex_skills::normalize_skill_path;
pub use codex_skills::plugin_config_name_from_path;
pub use codex_skills::tool_kind_for_path;
use codex_utils_path_uri::PathUri;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::MAX_SKILL_PROMPT_BYTES;

#[derive(Debug, Default)]
pub struct SkillInjections {
    pub items: Vec<SkillInjection>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInjection {
    pub name: String,
    pub path: String,
    pub contents: String,
}

/// Share of the model's context window that pinned-skill bodies (the
/// framework skills re-injected every session start / post-compact) are
/// allowed to consume. Mirrors `SKILL_METADATA_CONTEXT_WINDOW_PERCENT` in
/// `render.rs` but sized for full skill bodies rather than short
/// descriptions -- pinned bodies carry the actual framework instructions,
/// so they get a much larger share.
const PINNED_SKILL_CONTEXT_WINDOW_PERCENT: usize = 25;

/// Fallback budget (in characters) when the model's context window is
/// unknown, e.g. some local providers that don't report one.
const DEFAULT_PINNED_SKILL_CHAR_BUDGET: usize = 40_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinnedSkillBudget {
    Tokens(usize),
    Characters(usize),
}

impl PinnedSkillBudget {
    fn limit(self) -> usize {
        match self {
            Self::Tokens(limit) | Self::Characters(limit) => limit,
        }
    }

    fn cost(self, text: &str) -> usize {
        match self {
            Self::Tokens(_) => approx_token_count(text),
            Self::Characters(_) => text.chars().count(),
        }
    }
}

/// Default pinned-skill body budget for a session: a percentage of the
/// model's context window when known, otherwise a fixed character budget.
/// Small-context local models therefore get proportionally less pinned-skill
/// content injected rather than the full, un-budgeted set every session.
pub fn default_pinned_skill_budget(context_window: Option<i64>) -> PinnedSkillBudget {
    context_window
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
        .map(|window| {
            PinnedSkillBudget::Tokens(
                window
                    .saturating_mul(PINNED_SKILL_CONTEXT_WINDOW_PERCENT)
                    .saturating_div(100)
                    .max(1),
            )
        })
        .unwrap_or(PinnedSkillBudget::Characters(
            DEFAULT_PINNED_SKILL_CHAR_BUDGET,
        ))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinnedSkillBudgetReport {
    pub included_count: usize,
    pub omitted_count: usize,
    pub omitted_names: Vec<String>,
}

/// Trim already-loaded pinned-skill injections to fit `budget`, keeping
/// whole skill bodies only (never truncates a body mid-content) and
/// preserving their original order. Skills that don't fit are dropped
/// entirely, starting from the first one that would push the running total
/// over budget -- so higher-priority pinned skills (declared/loaded first)
/// are kept over later ones. The very first skill is always kept even if it
/// alone exceeds the budget, so a session never ends up with zero pinned
/// content injected just because one skill is large.
pub fn budget_skill_injections(
    items: Vec<SkillInjection>,
    budget: PinnedSkillBudget,
) -> (Vec<SkillInjection>, PinnedSkillBudgetReport) {
    let limit = budget.limit();
    let mut kept = Vec::with_capacity(items.len());
    let mut report = PinnedSkillBudgetReport::default();
    let mut used = 0usize;

    for item in items {
        let cost = budget.cost(&item.contents);
        if !kept.is_empty() && used.saturating_add(cost) > limit {
            report.omitted_count += 1;
            report.omitted_names.push(item.name);
            continue;
        }
        used = used.saturating_add(cost);
        report.included_count += 1;
        kept.push(item);
    }

    (kept, report)
}

/// Host skill prompts that have already been injected by an extension for this
/// turn.
///
/// Core uses this to keep the legacy skill-injection path from sending the same
/// host `SKILL.md` body again while the skills extension is being wired in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectedHostSkillPrompts {
    paths: HashSet<String>,
}

/// Marks a turn whose skills extension projects the host skill catalog through
/// WorldState.
///
/// Core uses this to keep its legacy thread-start catalog from duplicating the
/// extension-owned catalog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostSkillsCatalogInWorldState;

impl InjectedHostSkillPrompts {
    pub fn insert_path(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.paths.insert(normalize_host_skill_path(&path));
        self.paths.insert(path);
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains_path(&self, path: &str) -> bool {
        self.paths.contains(path) || self.paths.contains(&normalize_host_skill_path(path))
    }
}

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(mentioned_skill_count = mentioned_skills.len())
)]
pub async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    loaded_skills: Option<&SkillLoadOutcome>,
    otel: Option<&SessionTelemetry>,
    analytics_client: &AnalyticsEventsClient,
    tracking: TrackEventsContext,
) -> SkillInjections {
    if mentioned_skills.is_empty() {
        return SkillInjections::default();
    }

    let mut result = SkillInjections {
        items: Vec::with_capacity(mentioned_skills.len()),
        warnings: Vec::new(),
    };
    let mut invocations = Vec::new();

    for skill in mentioned_skills {
        let fs = loaded_skills
            .and_then(|outcome| outcome.file_system_for_skill(skill))
            .unwrap_or_else(|| Arc::clone(&LOCAL_FS));
        let path = PathUri::from_abs_path(&skill.path_to_skills_md);
        match fs.read_file_text(&path, /*sandbox*/ None).await {
            Ok(contents) => {
                let (contents, truncated) =
                    if loaded_skills.is_some_and(|outcome| outcome.is_agent_plugin_skill(skill)) {
                        bounded_skill_prompt_contents(&contents)
                    } else {
                        (contents, false)
                    };
                if truncated {
                    result.warnings.push(format!(
                        "Skill `{}` exceeded the main prompt context limit and was truncated.",
                        skill.name
                    ));
                }
                emit_skill_injected_metric(otel, skill, "ok");
                invocations.push(SkillInvocation {
                    skill_name: skill.name.clone(),
                    skill_scope: skill.scope,
                    skill_path: skill.path_to_skills_md.to_path_buf(),
                    plugin_id: skill.plugin_id.clone(),
                    remote_plugin_id: skill.remote_plugin_id.clone(),
                    invocation_type: InvocationType::Explicit,
                });
                result.items.push(SkillInjection {
                    name: skill.name.clone(),
                    path: skill.path_to_skills_md.to_string_lossy().into_owned(),
                    contents,
                });
            }
            Err(err) => {
                emit_skill_injected_metric(otel, skill, "error");
                let message = format!(
                    "Failed to load skill {name} at {path}: {err:#}",
                    name = skill.name,
                    path = skill.path_to_skills_md.display()
                );
                result.warnings.push(message);
            }
        }
    }

    analytics_client.track_skill_invocations(tracking, invocations);

    result
}

fn bounded_skill_prompt_contents(contents: &str) -> (String, bool) {
    let bounded = take_bytes_at_char_boundary(contents, MAX_SKILL_PROMPT_BYTES);
    (bounded.to_string(), bounded.len() < contents.len())
}

fn normalize_host_skill_path(path: &str) -> String {
    normalize_skill_path(path).replace('\\', "/")
}

fn emit_skill_injected_metric(
    otel: Option<&SessionTelemetry>,
    skill: &SkillMetadata,
    status: &str,
) {
    let Some(otel) = otel else {
        return;
    };
    let skill_name_tag = sanitize_metric_tag_value(skill.name.as_str());

    otel.counter(
        "codex.skill.injected",
        /*inc*/ 1,
        &[
            ("status", status),
            ("skill", skill_name_tag.as_str()),
            ("invoke_type", "explicit"),
        ],
    );
}

#[cfg(test)]
#[path = "prompt_injection_tests.rs"]
mod tests;
