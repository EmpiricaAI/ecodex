//! `ecodex models` — L3 model registry (T3).
//!
//! `refresh`: probe each configured `[model_providers.*]` `/v1/models` endpoint,
//! keep only slugs recognized as coding/agentic models (curated seed ∪ family
//! table), synthesize lean curated entries, and write them to
//! `$CODEX_HOME/models.user.json`. The read/merge side lives in
//! `codex_models_manager::curated_seed`; this command only writes that file.
//!
//! `list`: show the resolved curated registry (bundled seed ∪ user overlay).

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_core::config::Config;
use codex_models_manager::curated_seed::{
    self, CuratedEntry, ReasoningMeta,
};
use codex_utils_cli::CliConfigOverrides;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(bin_name = "codex models")]
pub struct ModelsCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: ModelsSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum ModelsSubcommand {
    /// Probe configured providers' /v1/models and refresh ~/.codex/models.user.json.
    Refresh(RefreshArgs),
    /// Show the resolved curated model registry (bundled seed + user overlay).
    List,
}

#[derive(Debug, Parser, Default)]
pub struct RefreshArgs {
    /// Only probe this provider (by `[model_providers.<id>]` key). Repeatable.
    #[clap(long = "provider", value_name = "ID")]
    pub providers: Vec<String>,
    /// Print what would be written without touching models.user.json.
    #[clap(long)]
    pub dry_run: bool,
    /// Keep ALL discovered slugs, not just recognized coding/agentic models.
    #[clap(long)]
    pub no_filter: bool,
}

/// OpenAI-compatible `/v1/models` response: `{ "data": [ { "id": "..." }, ... ] }`.
#[derive(serde::Deserialize)]
struct ModelsListResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(serde::Deserialize)]
struct ModelEntry {
    id: String,
}

pub async fn run(overrides: Vec<(String, toml::Value)>, cli: ModelsCli) -> Result<()> {
    match cli.subcommand {
        ModelsSubcommand::Refresh(args) => run_refresh(overrides, args).await,
        ModelsSubcommand::List => run_list().await,
    }
}

async fn run_list() -> Result<()> {
    let entries = curated_seed::all_entries();
    if entries.is_empty() {
        println!("No curated models. (bundled seed failed to load?)");
        return Ok(());
    }
    println!("Resolved model registry ({} entries):\n", entries.len());
    for e in entries {
        let route = if e.routes.is_empty() {
            "-".to_string()
        } else {
            e.routes.join(",")
        };
        let ctx = e
            .context_window
            .map(|c| format!("{}k", c / 1000))
            .unwrap_or_else(|| "?".to_string());
        let tools = if e.supports_tools.unwrap_or(false) { "tools" } else { "" };
        let think = match &e.reasoning {
            Some(r) if r.supported => "think",
            _ => "",
        };
        let eu = match &e.jurisdiction {
            Some(j) if j.eu_data_residency => " [EU]",
            _ => "",
        };
        println!(
            "  {:<34} {:>6}  {:<5} {:<5} {}{}",
            e.slug, ctx, tools, think, route, eu
        );
    }
    Ok(())
}

async fn run_refresh(overrides: Vec<(String, toml::Value)>, args: RefreshArgs) -> Result<()> {
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .context("failed to build HTTP client")?;

    // Don't re-write bundled-seed slugs into the user file; start from any
    // existing user entries so a re-run is additive across providers.
    let bundled = curated_seed::bundled_slugs();
    let mut acc: BTreeMap<String, CuratedEntry> = curated_seed::existing_user_entries()
        .into_iter()
        .map(|e| (e.slug.clone(), e))
        .collect();

    let mut probed = 0usize;
    let mut discovered = 0usize;
    let mut kept = 0usize;

    for (id, provider) in &config.model_providers {
        if !args.providers.is_empty() && !args.providers.contains(id) {
            continue;
        }
        let Some(base_url) = provider.base_url.clone() else {
            continue; // non-HTTP provider (e.g. bedrock); skip
        };
        probed += 1;

        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let mut req = client.get(&url);
        match provider.api_key() {
            Ok(Some(key)) => req = req.bearer_auth(key),
            Ok(None) => {}
            Err(e) => {
                eprintln!("  {id}: skipping — no API key ({e})");
                continue;
            }
        }
        if let Some(headers) = &provider.http_headers {
            for (k, v) in headers {
                // upstream wraps header values in RedactedString; deref to &str
                // for the HeaderValue conversion.
                req = req.header(k, v.as_str());
            }
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {id}: probe failed ({e})");
                continue;
            }
        };
        if !resp.status().is_success() {
            eprintln!("  {id}: {} from {url}", resp.status());
            continue;
        }
        let parsed: ModelsListResponse = match resp.json().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  {id}: response not /v1/models-shaped ({e})");
                continue;
            }
        };

        // Collect this provider's kept slugs, collapse version-sprawl to the
        // latest-per-line, then synthesize. Collapse is per-provider so a
        // slug's provider route stays correct.
        let mut provider_slugs: Vec<String> = Vec::new();
        for m in parsed.data {
            discovered += 1;
            if bundled.contains(&m.id) || acc.contains_key(&m.id) {
                continue; // already covered by seed or a prior provider
            }
            if !args.no_filter && !curated_seed::discovery_keeps(&m.id) {
                continue; // not a curated-family coding model — keep registry lean
            }
            provider_slugs.push(m.id);
        }
        let collapsed = if args.no_filter {
            provider_slugs
        } else {
            curated_seed::collapse_to_latest_per_line(&provider_slugs)
        };
        for slug in collapsed {
            if acc.contains_key(&slug) {
                continue;
            }
            kept += 1;
            acc.insert(slug.clone(), synthesize_entry(&slug, id));
        }
        println!("  {id}: probed {url}");
    }

    let entries: Vec<CuratedEntry> = acc.into_values().collect();
    println!(
        "\nProbed {probed} provider(s), saw {discovered} model(s), kept {kept} new coding model(s); {} total in user overlay.",
        entries.len()
    );

    if args.dry_run {
        println!("(dry-run — models.user.json not written)");
        for e in &entries {
            println!("  + {}", e.slug);
        }
        return Ok(());
    }

    let path = curated_seed::write_user_models(&entries)
        .context("failed to write models.user.json")?;
    println!("Wrote {}", path.display());
    println!("(restart ecodex to pick up the refreshed registry)");
    Ok(())
}

/// Build a lean entry for a newly discovered slug. Capability metadata is
/// inferred conservatively from family recognition; `calibration_tier` is
/// always `unmeasured`. `last_verified` is left None (caller has no clock here);
/// provenance is recorded in `evidence`.
fn synthesize_entry(slug: &str, provider_id: &str) -> CuratedEntry {
    // Tools/reasoning default conservative-true for recognized coding families;
    // the per-family/per-slug truth is refined by the curated seed when present.
    CuratedEntry {
        slug: slug.to_string(),
        display_name: None,
        description: None,
        context_window: None, // family table / fallback supplies a default at resolve time
        supports_tools: Some(true),
        reasoning: Some(ReasoningMeta { supported: true }),
        routes: vec![route_for_provider(provider_id)],
        jurisdiction: None,
        calibration_tier: Some("unmeasured".to_string()),
        last_verified: None,
        evidence: Some(format!("discovered: {provider_id}")),
    }
}

/// Heuristic route label from the provider id.
fn route_for_provider(provider_id: &str) -> String {
    let p = provider_id.to_ascii_lowercase();
    if p.contains("openrouter") {
        "openrouter".to_string()
    } else if p.contains("ollama")
        || p.contains("llama")
        || p.contains("local")
        || p.contains("lmstudio")
        || p.contains("vllm")
        || p == "oss"
    {
        "local".to_string()
    } else {
        "direct".to_string()
    }
}
