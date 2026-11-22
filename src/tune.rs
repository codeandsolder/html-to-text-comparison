//! Two-source, single-engine live settings lab for the immutable HN HTML corpus.
//! Axum HTTP with typed data, bounded isolated-worker previews, and durable profiles.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, RwLock, Semaphore};

use crate::extractor_config::{ExtractorConfig, ExtractorState, ExtractorStates};
use crate::runtime_config::{ExecutionPolicy, TunerConfig};
use crate::scores;

const MAX_CACHE_ENTRIES: usize = 160;
const MAX_CACHE_BYTES: usize = 48 * 1024 * 1024;
const MAX_PREVIEW_BYTES: usize = 384 * 1024;
const MAX_STORY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Corpus {
    records: Vec<CorpusRecord>,
}

#[derive(Debug, Deserialize)]
struct CorpusRecord {
    capture_id: String,
    title: String,
    final_url: String,
    hn_rank: usize,
    bytes: usize,
    sha256: String,
    file: String,
}

#[derive(Debug, Deserialize)]
struct Shortlist {
    keep: Vec<String>,
    results: HashMap<String, Benchmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Benchmark {
    worker_p50_ms: Option<f64>,
    worker_p90_ms: Option<f64>,
    success: usize,
    runs: usize,
}

#[derive(Clone)]
struct Story {
    id: String,
    title: String,
    url: String,
    rank: usize,
    bytes: usize,
    html: Arc<str>,
}

#[derive(Debug, Clone, Serialize)]
struct StoryInfo {
    id: String,
    title: String,
    url: String,
    rank: usize,
    bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
struct EngineInfo {
    name: String,
    defaults: Value,
    benchmark: Benchmark,
}

#[derive(Clone)]
struct TunerState {
    stories: Arc<Vec<Story>>,
    indices: Arc<HashMap<String, usize>>,
    engines: Arc<HashMap<String, EngineInfo>>,
    names: Arc<Vec<String>>,
    baseline: Arc<HashMap<(String, String), Arc<str>>>,
    profiles: Arc<RwLock<HashMap<String, Value>>>,
    profile_path: Arc<PathBuf>,
    cache: Arc<Mutex<ResultCache>>,
    permits: Arc<Semaphore>,
    policy: Arc<ExecutionPolicy>,
}

#[derive(Default)]
struct ResultCache {
    entries: HashMap<String, PreviewView>,
    order: VecDeque<String>,
    bytes: usize,
}

impl ResultCache {
    fn get(&mut self, key: &str) -> Option<PreviewView> {
        let value = self.entries.get(key)?.clone();
        self.order.retain(|entry| entry != key);
        self.order.push_back(key.to_owned());
        Some(value)
    }

    fn put(&mut self, key: String, value: PreviewView) {
        let size = value.output.len();
        if size > MAX_CACHE_BYTES {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes -= old.output.len();
            self.order.retain(|entry| entry != &key);
        }
        self.bytes += size;
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
        while self.entries.len() > MAX_CACHE_ENTRIES || self.bytes > MAX_CACHE_BYTES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes -= old.output.len();
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PreviewView {
    id: String,
    status: String,
    output: String,
    bytes: usize,
    conversion_ms: f64,
    worker_ms: f64,
    error: String,
    from_baseline: bool,
    from_cache: bool,
    clipped: bool,
}

#[derive(Debug, Serialize)]
struct PairResponse {
    engine: String,
    results: [PreviewView; 2],
}

#[derive(Debug, Deserialize)]
struct PairRequest {
    engine: String,
    pages: [String; 2],
    settings: Value,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize)]
struct SaveRequest {
    engine: String,
    settings: Value,
}

type ApiResult<T> = Result<T, (StatusCode, String)>;

fn invalid(message: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.into())
}

fn failure(message: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, message.to_string())
}

fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn config_group(state: &ExtractorState, name: &str) -> Value {
    let cfg = &state.config;
    match name {
        "html2text" => json!(cfg.html2text),
        "htmd" => json!(cfg.htmd),
        "mdka" => json!(cfg.mdka),
        "readable-readability" => json!(cfg.readable_readability),
        "dom_smoothie" => json!(cfg.dom_smoothie),
        "turndown" => json!(cfg.turndown),
        "mdream" => json!(cfg.mdream),
        "html2text-py" => json!(cfg.html2text_py),
        "markdownify" => json!(cfg.markdownify),
        "webclaw" => json!(cfg.webclaw),
        "html-to-markdown-go" => json!(cfg.html_to_markdown_go),
        "fast_html2md" => json!(cfg.fast_html2md),
        "html2md" => json!(cfg.html2md),
        "resiliparse" => json!(cfg.resiliparse),
        "mozilla-readability" => json!(cfg.mozilla_readability),
        "node-html-markdown" => json!(cfg.node_html_markdown),
        _ => json!({}),
    }
}

fn validate_settings(
    state: &TunerState,
    engine: &str,
    settings: &Value,
) -> ApiResult<ExtractorConfig> {
    let info = state
        .engines
        .get(engine)
        .ok_or_else(|| invalid("Engine not in the speed-qualified shortlist"))?;
    let expected = info
        .defaults
        .as_object()
        .ok_or_else(|| failure("Missing engine defaults"))?;
    let submitted = settings
        .as_object()
        .ok_or_else(|| invalid("Settings must be a JSON object"))?;
    if expected.len() != submitted.len() || expected.keys().any(|key| !submitted.contains_key(key))
    {
        return Err(invalid("Settings have missing or unrecognized fields"));
    }
    let default = ExtractorStates::default()
        .states
        .remove(engine)
        .ok_or_else(|| failure("Configured engine not in Rust extractor registry"))?;
    let mut selected = default;
    crate::web::apply_extractor_config(engine, settings, &mut selected).map_err(invalid)?;

    // Bounds keep a malformed form from generating enormous allocations.
    let length_limit = |value: &Value| value.to_string().len() <= 16_384;
    if !length_limit(settings) {
        return Err(invalid("Extractor settings exceed the 16 KiB limit"));
    }
    if selected.config.dom_smoothie.n_top_candidates > 1000
        || selected.config.dom_smoothie.char_threshold > 1_000_000
        || selected.config.mozilla_readability.nb_top_candidates > 1000
        || selected.config.mozilla_readability.char_threshold > 1_000_000
        || selected.config.resiliparse.skip_elements.len() > 64
        || selected.config.html2text.max_wrap_width > 32_000
        || selected.config.html2text.min_wrap_width > 32_000
        || selected.config.node_html_markdown.max_consecutive_newlines > 100
    {
        return Err(invalid(
            "Numeric or selector setting exceeds supported tuning limits",
        ));
    }
    Ok(selected.config)
}

fn load(config: &TunerConfig, policy: ExecutionPolicy) -> Result<TunerState, String> {
    let root = &config.corpus_dir;
    let state_dir = &config.state_dir;
    let policy = Arc::new(policy);
    let manifest: Corpus = serde_json::from_slice(
        &fs::read(root.join("manifest.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let shortlist: Shortlist = serde_json::from_slice(
        &fs::read(root.join("bench/shortlist.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if manifest.records.len() != 20 || shortlist.keep.len() != 20 {
        return Err("Expected 20 immutable HN sources and 20 qualified engines".into());
    }
    let mut stories = Vec::with_capacity(20);
    let mut indices = HashMap::new();
    for (i, record) in manifest.records.into_iter().enumerate() {
        let expected_file = format!("html/{:03}-{}.html", i + 1, record.capture_id);
        if !is_safe_id(&record.capture_id)
            || record.file != expected_file
            || record.bytes > MAX_STORY_BYTES.min(policy.max_html_bytes)
        {
            return Err("Unsafe or oversized corpus entry".into());
        }
        let data = fs::read(root.join(&record.file)).map_err(|e| e.to_string())?;
        if data.len() != record.bytes || format!("{:x}", Sha256::digest(&data)) != record.sha256 {
            return Err(format!("Source SHA256 mismatch: {}", record.capture_id));
        }
        let parsed = url::Url::parse(&record.final_url).map_err(|e| e.to_string())?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return Err("Corpus includes non-HTTP source URL".into());
        }
        if indices.insert(record.capture_id.clone(), i).is_some() {
            return Err("Duplicate captured story ID".into());
        }
        stories.push(Story {
            id: record.capture_id,
            title: record.title,
            url: record.final_url,
            rank: record.hn_rank,
            bytes: record.bytes,
            html: String::from_utf8(data).map_err(|e| e.to_string())?.into(),
        });
    }

    let registry = ExtractorStates::default();
    let mut engines = HashMap::new();
    let mut baseline = HashMap::new();
    let mut names = shortlist.keep;
    names.sort();
    for name in &names {
        if !is_safe_id(name) {
            return Err("Unsafe extractor ID in shortlist".into());
        }
        let info = registry
            .states
            .get(name)
            .ok_or_else(|| format!("Unknown extractor in shortlist: {name}"))?;
        let benchmark = shortlist
            .results
            .get(name)
            .ok_or_else(|| format!("No benchmark for {name}"))?
            .clone();
        engines.insert(
            name.clone(),
            EngineInfo {
                name: name.clone(),
                defaults: config_group(info, name),
                benchmark,
            },
        );
        let output_set = if matches!(name.as_str(), "webclaw" | "html-to-markdown-go") {
            "installed-backends-full-20261115"
        } else {
            "full-20261115"
        };
        for story in &stories {
            let path = root
                .join("bench")
                .join(output_set)
                .join("outputs")
                .join(name)
                .join(format!("{}.txt", story.id));
            let output = fs::read_to_string(&path)
                .map_err(|e| format!("Missing immutable baseline {}: {e}", path.display()))?;
            baseline.insert((story.id.clone(), name.clone()), Arc::from(output));
        }
    }
    fs::create_dir_all(state_dir).map_err(|e| e.to_string())?;
    let profile_path = state_dir.join("profiles.json");
    let stored: HashMap<String, Value> = if profile_path.is_file() {
        serde_json::from_slice(&fs::read(&profile_path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Cannot read saved profiles: {e}"))?
    } else {
        HashMap::new()
    };
    let mut valid_profiles = HashMap::new();
    // Ignore retired engines but reject invalid current profile data on startup.
    for (engine, settings) in stored {
        if !engines.contains_key(&engine) {
            continue;
        }
        let state = TunerState {
            stories: Arc::new(Vec::new()),
            indices: Arc::new(HashMap::new()),
            engines: Arc::new(engines.clone()),
            names: Arc::new(names.clone()),
            baseline: Arc::new(HashMap::new()),
            profiles: Arc::new(RwLock::new(HashMap::new())),
            profile_path: Arc::new(profile_path.clone()),
            cache: Arc::new(Mutex::new(ResultCache::default())),
            permits: Arc::new(Semaphore::new(config.max_concurrent_previews)),
            policy: Arc::clone(&policy),
        };
        validate_settings(&state, &engine, &settings)
            .map_err(|(_, error)| format!("Invalid saved profile {engine}: {error}"))?;
        valid_profiles.insert(engine, settings);
    }
    Ok(TunerState {
        stories: Arc::new(stories),
        indices: Arc::new(indices),
        engines: Arc::new(engines),
        names: Arc::new(names),
        baseline: Arc::new(baseline),
        profiles: Arc::new(RwLock::new(valid_profiles)),
        profile_path: Arc::new(profile_path),
        cache: Arc::new(Mutex::new(ResultCache::default())),
        permits: Arc::new(Semaphore::new(config.max_concurrent_previews)),
        policy,
    })
}

fn clip(text: String) -> (String, bool) {
    if text.len() <= MAX_PREVIEW_BYTES {
        return (text, false);
    }
    let mut end = MAX_PREVIEW_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

fn baseline_view(state: &TunerState, page: &Story, engine: &str) -> ApiResult<PreviewView> {
    let result = state
        .baseline
        .get(&(page.id.clone(), engine.to_owned()))
        .ok_or_else(|| failure("Default extraction output missing"))?;
    let info = state
        .engines
        .get(engine)
        .ok_or_else(|| invalid("Engine not available"))?;
    let (output, clipped) = clip(result.to_string());
    Ok(PreviewView {
        id: page.id.clone(),
        status: "success".into(),
        bytes: result.len(),
        output,
        clipped,
        error: String::new(),
        conversion_ms: 0.0,
        worker_ms: info.benchmark.worker_p50_ms.unwrap_or(0.0),
        from_baseline: true,
        from_cache: true,
    })
}

async fn preview_one(
    state: &TunerState,
    page: &Story,
    engine: &str,
    config: ExtractorConfig,
    settings: &Value,
    force: bool,
) -> ApiResult<PreviewView> {
    let info = state
        .engines
        .get(engine)
        .ok_or_else(|| invalid("Engine unavailable"))?;
    if settings == &info.defaults && !force {
        return baseline_view(state, page, engine);
    }
    let cache_key = format!("{}:{}:{}", page.id, engine, settings);
    if !force {
        if let Some(mut hit) = state.cache.lock().await.get(&cache_key) {
            hit.from_cache = true;
            return Ok(hit);
        }
    }
    // The owned permit remains held by the blocking job even if the browser
    // aborts the request, avoiding orphaned worker storms on rapid edits.
    let permit = state
        .permits
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| failure(e.to_string()))?;
    let input = page.html.clone();
    let page_url = page.url.clone();
    let id = page.id.clone();
    let extractor = engine.to_owned();
    let started = Instant::now();
    let policy = Arc::clone(&state.policy);
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        scores::preview_single_extractor_settings(&policy, &page_url, &input, &extractor, config)
    })
    .await
    .map_err(|e| failure(format!("Worker join failed: {e}")))?
    .map_err(failure)?;
    let worker_ms = result.wall_time_micros as f64 / 1000.0;
    let conversion_ms = result.time_micros as f64 / 1000.0;
    let size = result.output.len();
    let status = match result.status {
        crate::runner::ExtractionStatus::Success => "success",
        crate::runner::ExtractionStatus::NoContent => "no_content",
        crate::runner::ExtractionStatus::Failed => "failed",
        crate::runner::ExtractionStatus::TimedOut => "timed_out",
        crate::runner::ExtractionStatus::Panicked => "panicked",
    };
    let (output, clipped) = clip(result.output);
    let value = PreviewView {
        id,
        status: status.into(),
        output,
        bytes: size,
        worker_ms,
        conversion_ms,
        error: result.error,
        from_baseline: false,
        from_cache: false,
        clipped,
    };
    state.cache.lock().await.put(cache_key, value.clone());
    tracing::debug!(engine, page = %page.id, ms = started.elapsed().as_millis(),
        "settings preview completed");
    Ok(value)
}

fn same_origin(headers: &HeaderMap) -> ApiResult<()> {
    let Some(origin_header) = headers.get(ORIGIN) else {
        return Ok(()); // Explicit CLI callers can omit Origin.
    };
    let origin = origin_header
        .to_str()
        .map_err(|_| invalid("Invalid Origin header"))?;
    let host = headers
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| invalid("Host header required"))?;
    let parsed = url::Url::parse(origin).map_err(|_| invalid("Malformed Origin"))?;
    let hostname = parsed
        .host_str()
        .ok_or_else(|| invalid("Origin hostname missing"))?;
    let authority = parsed
        .port()
        .map_or_else(|| hostname.to_owned(), |port| format!("{hostname}:{port}"));
    if !matches!(parsed.scheme(), "http" | "https") || authority != host {
        return Err((
            StatusCode::FORBIDDEN,
            "Cross-origin writes are disallowed".into(),
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct Initial {
    pages: Vec<StoryInfo>,
    engines: Vec<EngineInfo>,
    profiles: HashMap<String, Value>,
}

async fn initial(State(state): State<TunerState>) -> Json<Initial> {
    let pages = state
        .stories
        .iter()
        .map(|s| StoryInfo {
            id: s.id.clone(),
            title: s.title.clone(),
            url: s.url.clone(),
            rank: s.rank,
            bytes: s.bytes,
        })
        .collect();
    let engines = state
        .names
        .iter()
        .filter_map(|name| state.engines.get(name).cloned())
        .collect();
    let profiles = state.profiles.read().await.clone();
    Json(Initial {
        pages,
        engines,
        profiles,
    })
}

async fn extract(
    State(state): State<TunerState>,
    headers: HeaderMap,
    Json(req): Json<PairRequest>,
) -> ApiResult<Json<PairResponse>> {
    same_origin(&headers)?;
    if req.pages[0] == req.pages[1] {
        return Err(invalid("Choose two different source pages"));
    }
    let config = validate_settings(&state, &req.engine, &req.settings)?;
    let i = *state
        .indices
        .get(&req.pages[0])
        .ok_or_else(|| invalid("Unknown left source page"))?;
    let j = *state
        .indices
        .get(&req.pages[1])
        .ok_or_else(|| invalid("Unknown right source page"))?;
    let (a, b) = tokio::join!(
        preview_one(
            &state,
            &state.stories[i],
            &req.engine,
            config.clone(),
            &req.settings,
            req.force
        ),
        preview_one(
            &state,
            &state.stories[j],
            &req.engine,
            config,
            &req.settings,
            req.force
        ),
    );
    Ok(Json(PairResponse {
        engine: req.engine,
        results: [a?, b?],
    }))
}

async fn get_baseline(
    State(state): State<TunerState>,
    Path((engine, page)): Path<(String, String)>,
) -> ApiResult<Json<PreviewView>> {
    let i = *state
        .indices
        .get(&page)
        .ok_or_else(|| invalid("Unknown source page"))?;
    Ok(Json(baseline_view(&state, &state.stories[i], &engine)?))
}

async fn get_source(
    State(state): State<TunerState>,
    Path(page): Path<String>,
) -> ApiResult<Response> {
    let i = *state
        .indices
        .get(&page)
        .ok_or_else(|| invalid("Unknown source page"))?;
    Ok((
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        state.stories[i].html.to_string(),
    )
        .into_response())
}

async fn save(
    State(state): State<TunerState>,
    headers: HeaderMap,
    Json(req): Json<SaveRequest>,
) -> ApiResult<Json<Value>> {
    same_origin(&headers)?;
    validate_settings(&state, &req.engine, &req.settings)?;
    let mut profiles = state.profiles.write().await;
    let mut next = profiles.clone();
    next.insert(req.engine.clone(), req.settings.clone());
    let path = state.profile_path.as_ref();
    let parent = path
        .parent()
        .ok_or_else(|| failure("Profile path has no directory"))?;
    let serialized = serde_json::to_vec_pretty(&next).map_err(failure)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(failure)?;
    use std::io::Write as _;
    temporary.write_all(&serialized).map_err(failure)?;
    temporary.persist(path).map_err(failure)?;
    *profiles = next;
    Ok(Json(json!({"saved": true, "engine": req.engine})))
}

fn asset(body: &'static str, mime: &'static str) -> Response {
    let mut response = ([(CONTENT_TYPE, mime)], body).into_response();
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
             img-src 'none'; base-uri 'none'; form-action 'none'; object-src 'none'; \
             frame-ancestors 'none'",
        ),
    );
    response.headers_mut().insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn homepage() -> Response {
    asset(
        include_str!("tune_ui/index.html"),
        "text/html; charset=utf-8",
    )
}

async fn stylesheet() -> Response {
    asset(include_str!("tune_ui/site.css"), "text/css; charset=utf-8")
}

async fn javascript() -> Response {
    asset(
        include_str!("tune_ui/app.js"),
        "text/javascript; charset=utf-8",
    )
}

fn build_router(state: TunerState) -> Router {
    Router::new()
        .route("/", get(homepage))
        .route("/site.css", get(stylesheet))
        .route("/app.js", get(javascript))
        .route("/health", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ready" }))
        .route("/api/state", get(initial))
        .route("/api/extract", post(extract))
        .route("/api/profile", post(save))
        .route("/api/baseline/{engine}/{page}", get(get_baseline))
        .route("/api/source/{page}", get(get_source))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

pub async fn serve(config: TunerConfig, policy: ExecutionPolicy) -> Result<(), String> {
    let state = load(&config, policy)?;
    tracing::info!(
        sources = state.stories.len(),
        engines = state.engines.len(),
        "Loaded immutable HN settings lab"
    );
    // Always bind loopback; Certifex is the only external HTTPS ingress.
    let address = config.listen;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(%address, "Live extraction settings server listening");
    axum::serve(listener, build_router(state))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_real_settings_are_exposed() {
        let defaults = ExtractorStates::default();
        assert_eq!(
            config_group(&defaults.states["boilerpipe"], "boilerpipe"),
            json!({})
        );
        assert_eq!(
            config_group(&defaults.states["readability"], "readability"),
            json!({})
        );
        assert!(
            config_group(&defaults.states["dom_smoothie"], "dom_smoothie")
                .get("disable_json_ld")
                .is_some()
        );
        assert!(config_group(&defaults.states["resiliparse"], "resiliparse")
            .get("links")
            .is_some());
        assert!(
            config_group(&defaults.states["node-html-markdown"], "node-html-markdown")
                .get("use_link_reference_definitions")
                .is_some()
        );
    }

    #[test]
    fn cache_evicts_without_unbounded_growth() {
        let mut cache = ResultCache::default();
        for i in 0..200 {
            cache.put(
                i.to_string(),
                PreviewView {
                    id: i.to_string(),
                    status: "success".into(),
                    output: String::from("preview"),
                    bytes: 7,
                    conversion_ms: 1.0,
                    worker_ms: 2.0,
                    error: String::new(),
                    from_baseline: false,
                    from_cache: false,
                    clipped: false,
                },
            );
        }
        assert_eq!(cache.entries.len(), MAX_CACHE_ENTRIES);
        assert!(cache.get("0").is_none());
        assert!(cache.get("199").is_some());
    }

    #[test]
    fn clipped_html_cannot_split_utf8() {
        let data = "é".repeat(MAX_PREVIEW_BYTES);
        let (text, clipped) = clip(data);
        assert!(clipped);
        assert!(text.len() <= MAX_PREVIEW_BYTES);
    }
}
