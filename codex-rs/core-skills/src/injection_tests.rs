use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;

fn make_skill(name: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} skill"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: test_path_buf(path).abs(),
        scope: codex_protocol::protocol::SkillScope::User,
        pinned: false,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

fn set<'a>(items: &'a [&'a str]) -> HashSet<&'a str> {
    items.iter().copied().collect()
}

fn assert_mentions(text: &str, expected_names: &[&str], expected_paths: &[&str]) {
    let mentions = extract_tool_mentions(text);
    assert_eq!(mentions.names, set(expected_names));
    assert_eq!(mentions.paths, set(expected_paths));
}

fn linked_skill_mention(name: &str, unix_path: &str) -> String {
    format!("[${name}]({})", test_path_buf(unix_path).display())
}

fn collect_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    collect_explicit_skill_mentions(inputs, skills, disabled_paths, connector_slug_counts)
}

#[test]
fn text_mentions_skill_requires_exact_boundary() {
    assert_eq!(
        true,
        text_mentions_skill("use $notion-research-doc please", "notion-research-doc")
    );
    assert_eq!(
        true,
        text_mentions_skill("($notion-research-doc)", "notion-research-doc")
    );
    assert_eq!(
        true,
        text_mentions_skill("$notion-research-doc.", "notion-research-doc")
    );
    assert_eq!(
        false,
        text_mentions_skill("$notion-research-docs", "notion-research-doc")
    );
    assert_eq!(
        false,
        text_mentions_skill("$notion-research-doc_extra", "notion-research-doc")
    );
}

#[test]
fn text_mentions_skill_handles_end_boundary_and_near_misses() {
    assert_eq!(true, text_mentions_skill("$alpha-skill", "alpha-skill"));
    assert_eq!(false, text_mentions_skill("$alpha-skillx", "alpha-skill"));
    assert_eq!(
        true,
        text_mentions_skill("$alpha-skillx and later $alpha-skill ", "alpha-skill")
    );
}

#[test]
fn text_mentions_skill_handles_many_dollars_without_looping() {
    let prefix = "$".repeat(256);
    let text = format!("{prefix} not-a-mention");
    assert_eq!(false, text_mentions_skill(&text, "alpha-skill"));
}

#[test]
fn extract_tool_mentions_handles_plain_and_linked_mentions() {
    assert_mentions(
        "use $alpha and [$beta](/tmp/beta)",
        &["alpha", "beta"],
        &["/tmp/beta"],
    );
}

#[test]
fn extract_tool_mentions_skips_common_env_vars() {
    assert_mentions("use $PATH and $alpha", &["alpha"], &[]);
    assert_mentions("use [$HOME](/tmp/skill)", &[], &[]);
    assert_mentions("use $XDG_CONFIG_HOME and $beta", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_requires_link_syntax() {
    assert_mentions("[beta](/tmp/beta)", &[], &[]);
    assert_mentions("[$beta] /tmp/beta", &["beta"], &[]);
    assert_mentions("[$beta]()", &["beta"], &[]);
}

#[test]
fn extract_tool_mentions_trims_linked_paths_and_allows_spacing() {
    assert_mentions("use [$beta]   ( /tmp/beta )", &["beta"], &["/tmp/beta"]);
}

#[test]
fn extract_tool_mentions_stops_at_non_name_chars() {
    assert_mentions(
        "use $alpha.skill and $beta_extra",
        &["alpha", "beta_extra"],
        &[],
    );
}

#[test]
fn extract_tool_mentions_keeps_plugin_skill_namespaces() {
    assert_mentions(
        "use $slack:search and $alpha",
        &["alpha", "slack:search"],
        &[],
    );
}

#[test]
fn collect_explicit_skill_mentions_text_respects_skill_order() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![beta.clone(), alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: "first $alpha-skill then $beta-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    // Text scanning should not change the previous selection ordering semantics.
    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_prioritizes_structured_inputs() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let beta = make_skill("beta-skill", "/tmp/beta");
    let skills = vec![alpha.clone(), beta.clone()];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "beta-skill".to_string(),
            path: test_path_buf("/tmp/beta"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_invalid_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/missing"),
        },
    ];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_disabled_structured_and_blocks_plain_fallback() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![
        UserInput::Text {
            text: "please run $alpha-skill".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Skill {
            name: "alpha-skill".to_string(),
            path: test_path_buf("/tmp/alpha"),
        },
    ];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_dedupes_by_path() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let mention = linked_skill_mention("alpha-skill", "/tmp/alpha");
    let inputs = vec![UserInput::Text {
        text: format!("use {mention} and {mention}"),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_ambiguous_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: "use $demo-skill and again $demo-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_linked_path_over_name() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!(
            "use $demo-skill and {}",
            linked_skill_mention("demo-skill", "/tmp/beta")
        ),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_plain_name_when_connector_matches() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: "use $alpha-skill".to_string(),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_allows_explicit_path_with_connector_conflict() {
    let alpha = make_skill("alpha-skill", "/tmp/alpha");
    let skills = vec![alpha.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("alpha-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![alpha]);
}

#[test]
fn collect_explicit_skill_mentions_skips_when_linked_path_disabled() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/alpha")),
        text_elements: Vec::new(),
    }];
    let disabled = HashSet::from([test_path_buf("/tmp/alpha").abs()]);
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_prefers_resource_path() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta.clone()];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/beta")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, vec![beta]);
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_with_no_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let beta = make_skill("demo-skill", "/tmp/beta");
    let skills = vec![alpha, beta];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

#[test]
fn collect_explicit_skill_mentions_skips_missing_path_without_fallback() {
    let alpha = make_skill("demo-skill", "/tmp/alpha");
    let skills = vec![alpha];
    let inputs = vec![UserInput::Text {
        text: format!("use {}", linked_skill_mention("demo-skill", "/tmp/missing")),
        text_elements: Vec::new(),
    }];
    let connector_counts = HashMap::new();

    let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

    assert_eq!(selected, Vec::new());
}

fn injection(name: &str, contents: &str) -> SkillInjection {
    SkillInjection {
        name: name.to_string(),
        path: format!("/tmp/{name}/SKILL.md"),
        contents: contents.to_string(),
    }
}

#[test]
fn default_pinned_skill_budget_scales_with_context_window() {
    let budget = default_pinned_skill_budget(Some(16_384));
    assert_eq!(budget, PinnedSkillBudget::Tokens(4_096));
}

#[test]
fn default_pinned_skill_budget_falls_back_to_char_budget_when_unknown() {
    assert_eq!(
        default_pinned_skill_budget(None),
        PinnedSkillBudget::Characters(DEFAULT_PINNED_SKILL_CHAR_BUDGET)
    );
    assert_eq!(
        default_pinned_skill_budget(Some(0)),
        PinnedSkillBudget::Characters(DEFAULT_PINNED_SKILL_CHAR_BUDGET)
    );
    assert_eq!(
        default_pinned_skill_budget(Some(-1)),
        PinnedSkillBudget::Characters(DEFAULT_PINNED_SKILL_CHAR_BUDGET)
    );
}

#[test]
fn budget_skill_injections_keeps_everything_under_budget() {
    let items = vec![injection("a", "short"), injection("b", "also short")];
    let (kept, report) =
        budget_skill_injections(items.clone(), PinnedSkillBudget::Characters(1_000));

    assert_eq!(kept, items);
    assert_eq!(report.included_count, 2);
    assert_eq!(report.omitted_count, 0);
    assert!(report.omitted_names.is_empty());
}

#[test]
fn budget_skill_injections_drops_lower_priority_items_over_budget() {
    let items = vec![
        injection("first", "12345"),
        injection("second", "12345"),
        injection("third", "12345"),
    ];
    // Budget fits exactly the first two 5-char bodies, not the third.
    let (kept, report) = budget_skill_injections(items, PinnedSkillBudget::Characters(10));

    assert_eq!(
        kept.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(report.included_count, 2);
    assert_eq!(report.omitted_count, 1);
    assert_eq!(report.omitted_names, vec!["third".to_string()]);
}

#[test]
fn budget_skill_injections_always_keeps_the_first_item_even_if_oversized() {
    let items = vec![injection("huge", "0123456789"), injection("small", "x")];
    let (kept, report) = budget_skill_injections(items, PinnedSkillBudget::Characters(1));

    assert_eq!(
        kept.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(),
        vec!["huge"]
    );
    assert_eq!(report.included_count, 1);
    assert_eq!(report.omitted_count, 1);
    assert_eq!(report.omitted_names, vec!["small".to_string()]);
}

#[test]
fn budget_skill_injections_preserves_declared_order() {
    let items = vec![
        injection("epistemic-transaction", "aaaaa"),
        injection("secondary", "bbbbb"),
    ];
    let (kept, _report) = budget_skill_injections(items, PinnedSkillBudget::Characters(100));

    assert_eq!(
        kept.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(),
        vec!["epistemic-transaction", "secondary"]
    );
}
