//! ecodex L3 curated model seed (layer 1 of model resolution).
//!
//! The bundled `models.curated.json` holds LEAN per-slug entries — only the
//! curated/epistemic fields (context, tools, reasoning, route, jurisdiction,
//! calibration_tier). Codex-runtime fields (base_instructions, shell_type,
//! truncation_policy, ...) are NOT in the seed; they are inherited from
//! [`crate::model_info::build_fallback_model_info`] at enrichment time.
//!
//! Resolution precedence (see `model_info::model_info_from_slug`):
//!   1. exact-slug curated entry (seed  ∪  ~/.codex/models.user.json)  ← this module
//!   2. `recognize_open_weights_family` prefix table (legacy fallback)
//!   3. `build_fallback_model_info` generic default
//!
//! `models.user.json` (written by `ecodex models refresh`) uses the SAME lean
//! schema and overlays the bundled seed (user entries win on slug collision).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use codex_protocol::openai_models::ModelInfo;
use serde::Deserialize;
use serde::Serialize;

/// Lean curated entry — the only fields a human (or `models refresh`) authors.
/// Everything else on `ModelInfo` is inherited from the runtime template.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CuratedEntry {
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningMeta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<Jurisdiction>,
    /// Always `unmeasured` on seed — populated from grounded usage, never asserted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<String>,
    /// Provenance: where this entry came from (e.g. "discovered: <provider>").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReasoningMeta {
    #[serde(default)]
    pub supported: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Jurisdiction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default)]
    pub eu_data_residency: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct CuratedSeedFile {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default)]
    seed: Vec<CuratedEntry>,
}

fn default_schema_version() -> u32 {
    1
}

/// Bundled seed, parsed once. A malformed bundled file is a build/release bug,
/// so we fail loud in debug and fall back to empty in release.
static BUNDLED_SEED: LazyLock<Vec<CuratedEntry>> = LazyLock::new(|| {
    match serde_json::from_str::<CuratedSeedFile>(include_str!("../models.curated.json")) {
        Ok(f) => f.seed,
        Err(e) => {
            debug_assert!(false, "bundled models.curated.json is malformed: {e}");
            tracing::error!("bundled models.curated.json failed to parse: {e}");
            Vec::new()
        }
    }
});

/// Merged curated table: bundled seed overlaid by the user's `models.user.json`
/// (user entries win on slug collision). Keyed by exact slug.
static CURATED_TABLE: LazyLock<HashMap<String, CuratedEntry>> = LazyLock::new(|| {
    let mut table: HashMap<String, CuratedEntry> = HashMap::new();
    for e in BUNDLED_SEED.iter().cloned() {
        table.insert(e.slug.clone(), e);
    }
    for e in load_user_models() {
        table.insert(e.slug.clone(), e); // user overlay wins
    }
    table
});

/// `~/.codex/models.user.json` — written by `ecodex models refresh`. Absent or
/// malformed is non-fatal: discovery is optional, the bundled seed still works.
fn load_user_models() -> Vec<CuratedEntry> {
    let Some(path) = user_models_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<CuratedSeedFile>(&text) {
        Ok(f) => f.seed,
        Err(e) => {
            tracing::warn!("{} failed to parse, ignoring: {e}", path.display());
            Vec::new()
        }
    }
}

/// `$CODEX_HOME/models.user.json`, honoring the `CODEX_HOME` override.
pub fn user_models_path() -> Option<PathBuf> {
    let home = match std::env::var_os("CODEX_HOME") {
        Some(h) if !h.is_empty() => PathBuf::from(h),
        _ => dirs_home()?.join(".codex"),
    };
    Some(home.join("models.user.json"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Look up an exact-slug curated entry (layer 1). `None` → fall through to the
/// family-prefix table.
pub fn lookup(slug: &str) -> Option<CuratedEntry> {
    CURATED_TABLE.get(slug).cloned()
}

/// All curated entries currently in effect (bundled seed ∪ user overlay),
/// sorted by slug. For `ecodex models list`.
pub fn all_entries() -> Vec<CuratedEntry> {
    let mut v: Vec<CuratedEntry> = CURATED_TABLE.values().cloned().collect();
    v.sort_by(|a, b| a.slug.cmp(&b.slug));
    v
}

/// The bundled seed entries only (not the user overlay). For `models refresh`
/// so discovery never re-writes a bundled slug into the user file.
pub fn bundled_slugs() -> std::collections::HashSet<String> {
    BUNDLED_SEED.iter().map(|e| e.slug.clone()).collect()
}

/// Write `entries` to `$CODEX_HOME/models.user.json` (the file `load_user_models`
/// reads at next process start). Creates the parent dir if needed. Entries are
/// the lean schema — discovery output, not full `ModelInfo`.
pub fn write_user_models(entries: &[CuratedEntry]) -> std::io::Result<PathBuf> {
    let path = user_models_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve CODEX_HOME for models.user.json",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = CuratedSeedFile {
        schema_version: default_schema_version(),
        seed: entries.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Read the existing user overlay entries (for merge-on-refresh). Empty if absent.
pub fn existing_user_entries() -> Vec<CuratedEntry> {
    load_user_models()
}

/// Families we endorse for the curated registry. `ecodex models refresh` keeps a
/// discovered slug ONLY if its family is in this set — this is the "curated
/// families only" discovery policy (drops gemma/llama/phi/cohere general-chat
/// families that aren't coding-tier for our purposes). Edit here to widen.
const CURATED_FAMILIES: &[&str] = &[
    "kimi",     // Moonshot
    "qwen",     // Alibaba (incl qwen-coder, qwen*-max)
    "deepseek", // incl R1 / V3.x
    "gpt-oss",  // OpenAI open-weights
    "glm",      // Z.ai / Zhipu
    "minimax",  // MiniMax
    "mistral",  // Mistral AI
    "mixtral",  // Mistral MoE
    "devstral", // Mistral agentic-coding
    "codestral",// Mistral coding
];

/// Suffixes/markers that indicate a NON-coding variant even within a curated
/// family (OCR, vision, embeddings, audio, rerank, raw base models).
const NONCODING_MARKERS: &[&str] = &[
    "ocr", "vision", "-vl", "embed", "embedding", "rerank", "audio", "tts",
    "whisper", "image", "-base", "guard", "moderation",
];

/// Variant markers that indicate an UNSTABLE or DERIVATIVE release we don't want
/// cluttering the picker even within an endorsed family: experimental/preview
/// builds, distillations (a distill is a different, smaller model — keep the
/// real one), and snapshot/dated point-releases (we want the rolling alias, not
/// every dated pin). Stable flagship tiers (-pro/-max/-large/-flash/-mini) are
/// deliberately NOT here. Applied by `discovery_keeps` (never to exact seed
/// entries — the seed always wins).
const UNSTABLE_VARIANT_MARKERS: &[&str] = &[
    "-exp", "exp-", "experimental", "-preview", "preview-", "-rc", "-beta",
    "-alpha", "distill", "-terminus", "-thinking-", "-nightly", "-latest-",
];

/// Normalize a slug to its bare family-matchable form: strip `<provider>/`
/// namespace and any `:tag`, lowercase.
fn normalize_slug(slug: &str) -> String {
    slug.rsplit_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(slug)
        .split(':')
        .next()
        .unwrap_or(slug)
        .to_ascii_lowercase()
}

/// Discovery filter (curated-families-only policy): keep a discovered slug iff
/// (a) it is an exact curated-seed entry, OR (b) its family is in
/// `CURATED_FAMILIES` and it carries no non-coding marker. Used by
/// `ecodex models refresh`.
pub fn discovery_keeps(slug: &str) -> bool {
    if lookup(slug).is_some() {
        return true;
    }
    let norm = normalize_slug(slug);
    let full = slug.to_ascii_lowercase();
    if NONCODING_MARKERS.iter().any(|m| norm.contains(m) || full.contains(m)) {
        return false;
    }
    if UNSTABLE_VARIANT_MARKERS.iter().any(|m| full.contains(m)) {
        return false; // unstable/derivative — keep the picker to stable flagships
    }
    // Drop dated snapshot pins (e.g. `-2501`, `-0528`, `-v3-0324`): a trailing
    // 4-digit token that looks like MMYY/MMDD, in favor of the rolling alias.
    if has_dated_snapshot_suffix(&full) {
        return false;
    }
    CURATED_FAMILIES.iter().any(|fam| norm.starts_with(fam))
}

/// True if the slug ends in a bare 4-digit snapshot token (a dated pin like
/// `-2501`, `-0528`, `-2512`). Conservative: only a *trailing* all-digit 4-char
/// token, so real version numbers (`v3.2`, `m2.5`) and sizes (`70b`, `30b`) are
/// untouched.
fn has_dated_snapshot_suffix(full: &str) -> bool {
    full.rsplit(['-', ':', '/'])
        .next()
        .map(|tok| tok.len() == 4 && tok.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// Collapse a set of discovered slugs to the latest version per model *line*,
/// so the picker shows e.g. one `minimax-m3` instead of m1/m2/m2.1/m2.5/m3.
///
/// A "line key" is the slug with its trailing version token stripped (provider
/// namespace kept so `qwen/x` and `deepseek/x` never collide). Within a line we
/// keep the slug with the highest parsed version tuple; ties / unparseable
/// versions fall back to keeping the lexicographically-greatest slug — and any
/// slug we can't confidently place in a line is kept as its own line. The bias
/// is toward KEEPING: collapse only fires when we're confident two slugs are
/// the same line.
pub fn collapse_to_latest_per_line(slugs: &[String]) -> Vec<String> {
    use std::collections::BTreeMap;
    // line_key -> (best_version_tuple, best_slug)
    let mut lines: BTreeMap<String, (Vec<u64>, String)> = BTreeMap::new();
    for slug in slugs {
        let (key, ver) = line_key_and_version(slug);
        match lines.get_mut(&key) {
            Some((best_ver, best_slug)) => {
                let newer = ver > *best_ver || (ver == *best_ver && slug > best_slug);
                if newer {
                    *best_ver = ver;
                    *best_slug = slug.clone();
                }
            }
            None => {
                lines.insert(key, (ver, slug.clone()));
            }
        }
    }
    let mut out: Vec<String> = lines.into_values().map(|(_, s)| s).collect();
    out.sort();
    out
}

/// Split a slug into (line_key, version_tuple). The version is the LAST
/// version-looking token (e.g. `m2.5`, `v3.2`, `3.1`, `2512`); the line key is
/// everything else with that token replaced by a placeholder so siblings group.
/// Slugs with no version token get the whole slug as key and an empty version.
fn line_key_and_version(slug: &str) -> (String, Vec<u64>) {
    let lower = slug.to_ascii_lowercase();
    // Tokenize on '/', '-', ':' but remember positions by rebuilding a key.
    let parts: Vec<&str> = lower.split(['-', ':', '/']).collect();
    // Find the LAST token that parses as a version (contains a digit and only
    // digits/dots, optionally a leading single letter like v/m/r).
    let mut ver_idx: Option<usize> = None;
    for (i, tok) in parts.iter().enumerate() {
        if parse_version(tok).is_some() {
            ver_idx = Some(i);
        }
    }
    match ver_idx {
        Some(i) => {
            let ver = parse_version(parts[i]).unwrap_or_default();
            // The version token's leading letter (r1 vs v3, the DeepSeek
            // reasoning-vs-chat distinction) is LINE IDENTITY, not version —
            // keep it in the key so r1 and v3 stay separate lines.
            let alpha_prefix: String = parts[i]
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            // line key = all parts except the numeric version token, with the
            // alpha-prefix re-appended so siblings group but distinct lines split.
            let mut key: Vec<String> = parts
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, t)| (*t).to_string())
                .collect();
            if !alpha_prefix.is_empty() {
                key.push(format!("#{alpha_prefix}")); // '#' marks the version-line tag
            }
            (key.join("-"), ver)
        }
        None => (lower, Vec::new()),
    }
}

/// Parse a version-looking token into a comparable tuple. Accepts an optional
/// single leading letter (v3.2, m2.5, r1) then dotted digits. Returns None if
/// the token isn't version-shaped (so plain words like "coder", "max", "instruct"
/// don't get treated as versions).
fn parse_version(tok: &str) -> Option<Vec<u64>> {
    let body = match tok.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => &tok[1..],
        _ => tok,
    };
    if body.is_empty() || !body.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let nums: Option<Vec<u64>> = body.split('.').map(|p| p.parse::<u64>().ok()).collect();
    nums.filter(|v| !v.is_empty())
}

/// Enrich a runtime-default `ModelInfo` template with curated overlay fields.
/// Default-inherit, explicit-wins: only fields the curated entry actually sets
/// override the template. `base_instructions`, `shell_type`, `truncation_policy`,
/// etc. always come from the template.
pub fn enrich(mut template: ModelInfo, e: &CuratedEntry) -> ModelInfo {
    template.slug = e.slug.clone();
    if let Some(name) = &e.display_name {
        template.display_name = name.clone();
    }
    if e.description.is_some() {
        template.description = e.description.clone();
    }
    if let Some(ctx) = e.context_window {
        template.context_window = Some(ctx);
        template.max_context_window = Some(ctx);
    }
    if let Some(tools) = e.supports_tools {
        template.supports_parallel_tool_calls = tools;
    }
    // A curated entry is, by definition, a recognized model — suppress the
    // "fallback model metadata" warning the way the family table does.
    template.used_fallback_model_metadata = false;
    template
}

#[cfg(test)]
#[path = "curated_seed_tests.rs"]
mod tests;
