use crate::boilerplate;
use crate::extractor_config::{ExtractorState, ExtractorStates};
use crate::scores::ScoreStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

#[derive(Clone)]
struct AppState {
    score_store: ScoreStore,
    extractor_states: Arc<RwLock<ExtractorStates>>,
    states_path: PathBuf,
    jobs: Arc<Semaphore>,
}

pub async fn start_server(port: u16, data_dir: PathBuf) -> Result<(), String> {
    let states_path = data_dir.join("extractor_states.json");
    let scores_path = data_dir.join("scores");

    let max_saved_runs = std::env::var("HTML_TO_TEXT_MAX_SAVED_RUNS")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()
        .map_err(|error| format!("invalid HTML_TO_TEXT_MAX_SAVED_RUNS: {error}"))?;
    let score_store = ScoreStore::with_retention(scores_path, max_saved_runs)?;
    // On restart, reap metadata records above the limit before accepting traffic.
    score_store.prune()?;
    let extractor_states = ExtractorStates::load(&states_path);

    let state = AppState {
        score_store,
        extractor_states: Arc::new(RwLock::new(extractor_states)),
        states_path,
        jobs: Arc::new(Semaphore::new(max_concurrent_jobs()?)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/scores", get(list_scores))
        .route("/scores/{id}", get(get_score))
        .route("/scores/{id}", delete(delete_score))
        .route("/scores/{id}/grade", patch(update_grade))
        .route("/scores/{id}/output/{name}", get(get_output))
        .route("/scores/{id}/preview-settings", post(preview_settings))
        .route("/scores/{id}/compare-settings", post(compare_settings))
        .route("/scores/{id}/boilerplate-diff", post(run_boilerplate_diff))
        .route("/run", post(run_extraction))
        .route("/run-single", post(run_single_extraction))
        .route("/t/{name}/{enabled}", post(toggle_extractor))
        .route("/c/{name}", post(configure_extractor))
        .route("/states", get(get_all_states))
        .with_state(state);

    let bind = std::env::var("HTML_TO_TEXT_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = parse_listen_addr(&bind, port)?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("Server running on http://{}", addr);
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

fn parse_listen_addr(ip: &str, port: u16) -> Result<SocketAddr, String> {
    let ip = ip
        .parse::<IpAddr>()
        .map_err(|error| format!("invalid HTML_TO_TEXT_BIND address: {error}"))?;
    Ok(SocketAddr::new(ip, port))
}

/// Extraction, external tools and boilerplate sampling are blocking
/// workloads; never run them on Tokio's async I/O worker threads.
async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|error| format!("blocking job failed: {error}"))?
}

fn max_concurrent_jobs() -> Result<usize, String> {
    const DEFAULT: usize = 2;
    const MAXIMUM: usize = 64;
    let configured =
        std::env::var("HTML_TO_TEXT_MAX_CONCURRENT_RUNS").map_or(Ok(DEFAULT), |value| {
            value
                .parse::<usize>()
                .map_err(|error| format!("invalid HTML_TO_TEXT_MAX_CONCURRENT_RUNS: {error}"))
        })?;
    if !(1..=MAXIMUM).contains(&configured) {
        return Err(format!(
            "HTML_TO_TEXT_MAX_CONCURRENT_RUNS must be 1..={MAXIMUM}"
        ));
    }
    Ok(configured)
}

/// Fail fast rather than queueing hundreds of long-running browser/LLM jobs.
/// A cloned permit stays inside the blocking task even if the HTTP client
/// disconnects, so capacity cannot be released while the job still runs.
fn reserve_job(state: &AppState) -> Result<Arc<OwnedSemaphorePermit>, StatusCode> {
    state
        .jobs
        .clone()
        .try_acquire_owned()
        .map(Arc::new)
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)
}

async fn index() -> impl IntoResponse {
    Html(HTML)
}

async fn list_scores(State(state): State<AppState>) -> impl IntoResponse {
    let scores = state.score_store.list().unwrap_or_default();
    Json(scores)
}

async fn get_score(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    state.score_store.load(&id).map_or_else(
        |_| (StatusCode::NOT_FOUND, "Score not found").into_response(),
        |score| Json(score).into_response(),
    )
}

async fn delete_score(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.score_store.delete(&id) {
        Ok(()) => "Deleted".into_response(),
        Err(error) => (StatusCode::NOT_FOUND, error).into_response(),
    }
}

async fn run_extraction(
    State(state): State<AppState>,
    Json(payload): Json<RunRequest>,
) -> impl IntoResponse {
    let admission = match reserve_job(&state) {
        Ok(admission) => admission,
        Err(code) => return (code, "all extraction slots are busy").into_response(),
    };
    let html = match fetch_html(&payload.url, payload.acquisition, Arc::clone(&admission)).await {
        Ok(h) => h,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let states = state.extractor_states.read().await.clone();
    let repository = state.score_store.clone();
    let score = run_blocking(move || {
        let _admission = admission;
        crate::scores::run_extraction(
            &payload.url,
            &html,
            payload.acquisition,
            &states,
            &repository,
        )
    })
    .await;
    match score {
        Ok(score) => Json(score).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

async fn run_single_extraction(
    State(state): State<AppState>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let mut states = state.extractor_states.read().await.clone();
    if !states.states.contains_key(&payload.extractor) {
        return (
            StatusCode::BAD_REQUEST,
            format!("unknown extractor: {}", payload.extractor),
        )
            .into_response();
    }
    let admission = match reserve_job(&state) {
        Ok(admission) => admission,
        Err(code) => return (code, "all extraction slots are busy").into_response(),
    };
    let html = match fetch_html(&payload.url, payload.acquisition, Arc::clone(&admission)).await {
        Ok(h) => h,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    for extractor_state in states.states.values_mut() {
        extractor_state.enabled = false;
    }

    if let Some(entry) = states.states.get_mut(&payload.extractor) {
        entry.enabled = true;
        if let Some(config) = &payload.config {
            if let Err(error) = apply_extractor_config(&payload.extractor, config, entry) {
                return (StatusCode::BAD_REQUEST, error).into_response();
            }
        }
    }
    let repository = state.score_store.clone();
    let score = run_blocking(move || {
        let _admission = admission;
        crate::scores::run_extraction(
            &payload.url,
            &html,
            payload.acquisition,
            &states,
            &repository,
        )
    })
    .await;
    match score {
        Ok(score) => Json(score).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

async fn compare_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let Ok(base_score) = state.score_store.load(&id) else {
        return (StatusCode::NOT_FOUND, "Score not found").into_response();
    };

    let admission = match reserve_job(&state) {
        Ok(admission) => admission,
        Err(code) => return (code, "all extraction slots are busy").into_response(),
    };
    let html = match load_score_source_html(&base_score, &state.score_store) {
        Ok(content) => content,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let baseline_config = base_score
        .settings_snapshot
        .states
        .get(&payload.extractor)
        .map_or_default(|state| state.config.clone());

    let candidate_config = if let Some(config) = &payload.config {
        let mut candidate_state = crate::extractor_config::ExtractorState {
            enabled: true,
            config: baseline_config.clone(),
        };
        if let Err(error) = apply_extractor_config(&payload.extractor, config, &mut candidate_state)
        {
            return (StatusCode::BAD_REQUEST, error).into_response();
        }
        candidate_state.config
    } else {
        baseline_config.clone()
    };

    let repository = state.score_store.clone();
    let score = run_blocking(move || {
        let _admission = admission;
        crate::scores::compare_single_extractor_settings(
            &base_score.url,
            &html,
            &payload.extractor,
            baseline_config,
            candidate_config,
            &repository,
        )
    })
    .await;
    match score {
        Ok(score) => Json(score).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

async fn preview_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let Ok(base_score) = state.score_store.load(&id) else {
        return (StatusCode::NOT_FOUND, "Score not found").into_response();
    };

    let admission = match reserve_job(&state) {
        Ok(admission) => admission,
        Err(code) => return (code, "all extraction slots are busy").into_response(),
    };
    let html = match load_score_source_html(&base_score, &state.score_store) {
        Ok(content) => content,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let baseline_config = base_score
        .settings_snapshot
        .states
        .get(&payload.extractor)
        .map_or_default(|state| state.config.clone());

    let candidate_config = if let Some(config) = &payload.config {
        let mut candidate_state = crate::extractor_config::ExtractorState {
            enabled: true,
            config: baseline_config,
        };
        if let Err(error) = apply_extractor_config(&payload.extractor, config, &mut candidate_state)
        {
            return (StatusCode::BAD_REQUEST, error).into_response();
        }
        candidate_state.config
    } else {
        baseline_config
    };

    let preview = run_blocking(move || {
        let _admission = admission;
        crate::scores::preview_single_extractor_settings(
            &base_score.url,
            &html,
            &payload.extractor,
            candidate_config,
        )
    })
    .await;
    match preview {
        Ok(preview) => Json(preview).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

async fn run_boilerplate_diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<BoilerplateDiffRequest>,
) -> impl IntoResponse {
    let Ok(base_score) = state.score_store.load(&id) else {
        return (StatusCode::NOT_FOUND, "Score not found").into_response();
    };

    let admission = match reserve_job(&state) {
        Ok(admission) => admission,
        Err(code) => return (code, "all extraction slots are busy").into_response(),
    };
    let html = match load_score_source_html(&base_score, &state.score_store) {
        Ok(content) => content,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let states = state.extractor_states.read().await.clone();
    let result = run_blocking(move || {
        let _admission = admission;
        boilerplate::analyze_boilerplate_diff(
            &base_score.url,
            &html,
            &states,
            payload.sample_count.unwrap_or(5),
        )
    })
    .await;
    match result {
        Ok(result) => Json(result).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}

async fn toggle_extractor(
    State(state): State<AppState>,
    Path((name, enabled)): Path<(String, bool)>,
) -> impl IntoResponse {
    let mut states = state.extractor_states.write().await;
    let mut updated = states.clone();
    let Some(extractor) = updated.states.get_mut(&name) else {
        return (StatusCode::NOT_FOUND, format!("unknown extractor: {name}")).into_response();
    };
    extractor.enabled = enabled;
    let persisted = updated.save(&state.states_path);
    if persisted.is_ok() {
        *states = updated;
    }
    drop(states);
    match persisted {
        Ok(()) => (StatusCode::OK, "Toggled").into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

async fn configure_extractor(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut states = state.extractor_states.write().await;
    let mut updated = states.clone();
    let Some(extractor) = updated.states.get_mut(&name) else {
        return (StatusCode::NOT_FOUND, format!("unknown extractor: {name}")).into_response();
    };
    if let Err(error) = apply_extractor_config(&name, &config, extractor) {
        return (StatusCode::BAD_REQUEST, error).into_response();
    }
    let persisted = updated.save(&state.states_path);
    if persisted.is_ok() {
        *states = updated;
    }
    drop(states);
    match persisted {
        Ok(()) => (StatusCode::OK, "Configured").into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

fn extractor_config_payload<'a>(config: &'a serde_json::Value, key: &str) -> &'a serde_json::Value {
    config.get(key).unwrap_or(config)
}

pub(crate) fn apply_extractor_config(
    name: &str,
    config: &serde_json::Value,
    state: &mut ExtractorState,
) -> Result<(), String> {
    if let Some(value) = config.get("timeout_ms") {
        let timeout = value
            .as_u64()
            .ok_or("timeout_ms must be a positive integer")?;
        if !(1..=900_000).contains(&timeout) {
            return Err("timeout_ms must be between 1 and 900000".to_string());
        }
        state.config.timeout_ms = Some(timeout);
    }
    macro_rules! set_typed {
        ($field:ident, $key:literal) => {{
            state.config.$field =
                serde_json::from_value(extractor_config_payload(config, $key).clone())
                    .map_err(|error| format!("{name} config invalid: {error}"))?;
        }};
    }
    match name {
        "fast_html2md" => set_typed!(fast_html2md, "fast_html2md"),
        "html2md" => set_typed!(html2md, "html2md"),
        "resiliparse" => {
            set_typed!(resiliparse, "resiliparse");
            if !matches!(
                state.config.resiliparse.formatting_mode.as_str(),
                "plain" | "formatted" | "minimal_html"
            ) {
                return Err(
                    "resiliparse formatting_mode must be plain, formatted or minimal_html".into(),
                );
            }
        }
        "mozilla-readability" => set_typed!(mozilla_readability, "mozilla_readability"),
        "node-html-markdown" => set_typed!(node_html_markdown, "node_html_markdown"),
        "html2text" => set_typed!(html2text, "html2text"),
        "htmd" => {
            set_typed!(htmd, "htmd");
            state
                .config
                .skip_tags
                .clone_from(&state.config.htmd.skip_tags);
        }
        "html2md-rs" => {
            set_typed!(html2md_rs, "html2md_rs");
            state
                .config
                .skip_tags
                .clone_from(&state.config.html2md_rs.ignore_tags);
        }
        "mdka" => {
            set_typed!(mdka, "mdka");
            state.config.mdka.clone().into_conversion_options()?;
        }
        "readable-readability" => set_typed!(readable_readability, "readable_readability"),
        "dom_smoothie" => set_typed!(dom_smoothie, "dom_smoothie"),
        "august" => {
            if let Some(value) = config
                .get("max_width")
                .or_else(|| config.get("augus_max_width"))
            {
                let width = value.as_u64().ok_or("max_width must be an integer")?;
                state.config.augus_max_width = usize::try_from(width)
                    .map_err(|error| format!("invalid august max_width: {error}"))?;
            }
        }
        "turndown" => set_typed!(turndown, "turndown"),
        "percollate" => set_typed!(percollate, "percollate"),
        "mdream" => set_typed!(mdream, "mdream"),
        "trafilatura" => set_typed!(trafilatura, "trafilatura"),
        "html2text-py" => set_typed!(html2text_py, "html2text_py"),
        "markdownify" => set_typed!(markdownify, "markdownify"),
        "webclaw" => set_typed!(webclaw, "webclaw"),
        "e2m" => set_typed!(e2m, "e2m"),
        "html-to-markdown-go" => set_typed!(html_to_markdown_go, "html_to_markdown_go"),
        // Extractors with no custom configuration still accept a deadline.
        _ => {}
    }
    Ok(())
}

async fn get_all_states(State(state): State<AppState>) -> Response {
    let states = state.extractor_states.read().await.clone();
    Json(states).into_response()
}

async fn update_grade(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<GradeRequest>,
) -> Response {
    if payload.grade > 9 {
        return (StatusCode::BAD_REQUEST, "Grade must be between 0 and 9").into_response();
    }

    state
        .score_store
        .update_grade(&id, &payload.extractor, payload.grade)
        .map_or_else(
            |_| (StatusCode::NOT_FOUND, "Score not found").into_response(),
            |score| Json(score).into_response(),
        )
}

async fn get_output(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Response {
    let Ok(score) = state.score_store.load(&id) else {
        return (StatusCode::NOT_FOUND, "Score not found").into_response();
    };

    if !score
        .extractor_results
        .iter()
        .any(|result| result.name == name)
    {
        return (StatusCode::NOT_FOUND, "Output not found").into_response();
    }
    let Ok(output_path) = state.score_store.output_path(&id, &name) else {
        return (StatusCode::BAD_REQUEST, "Invalid output name").into_response();
    };

    std::fs::read_to_string(&output_path).map_or_else(
        |_| (StatusCode::NOT_FOUND, "Output not found").into_response(),
        IntoResponse::into_response,
    )
}

async fn fetch_html(
    url: &str,
    acquisition: crate::acquisition::Acquisition,
    admission: Arc<OwnedSemaphorePermit>,
) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        acquisition.acquire(&parsed)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn load_score_source_html(
    score: &crate::scores::Score,
    repository: &ScoreStore,
) -> Result<String, String> {
    let path = repository.source_path(&score.id)?;
    std::fs::read_to_string(path)
        .map_err(|error| format!("saved HTML for run {} is unavailable: {error}", score.id))
}

#[derive(serde::Deserialize)]
pub struct RunRequest {
    pub url: String,
    #[serde(default)]
    pub acquisition: crate::acquisition::Acquisition,
}

#[derive(serde::Deserialize)]
pub struct SingleRunRequest {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub acquisition: crate::acquisition::Acquisition,
    pub extractor: String,
    pub config: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct GradeRequest {
    pub extractor: String,
    pub grade: u8,
}

#[derive(serde::Deserialize)]
pub struct BoilerplateDiffRequest {
    pub sample_count: Option<usize>,
}

static HTML: &str = include_str!("web_ui.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_bind_address_is_explicit_and_supports_ipv6() {
        assert_eq!(
            parse_listen_addr("127.0.0.1", 3000).map(|addr| addr.to_string()),
            Ok("127.0.0.1:3000".to_string())
        );
        assert_eq!(
            parse_listen_addr("::1", 8080).map(|addr| addr.to_string()),
            Ok("[::1]:8080".to_string())
        );
        assert!(parse_listen_addr("not an IP", 3000).is_err());
    }

    #[test]
    fn apply_extractor_config_accepts_nested_payloads() {
        let mut state = ExtractorState::default();

        assert!(apply_extractor_config(
            "html2text",
            &serde_json::json!({
                "skip_tags": [],
                "html2text": {
                    "max_wrap_width": 42,
                    "raw_mode": true,
                    "no_link_wrapping": true
                }
            }),
            &mut state,
        )
        .is_ok());

        assert_eq!(state.config.html2text.max_wrap_width, 42);
        assert!(state.config.html2text.raw_mode);
        assert!(state.config.html2text.no_link_wrapping);
    }

    #[test]
    fn invalid_extractor_configuration_is_rejected_without_mutation() {
        let mut state = ExtractorState::default();
        let before = serde_json::to_value(&state.config).ok();
        assert!(apply_extractor_config(
            "html2text",
            &serde_json::json!({"max_wrap_width": "not a number"}),
            &mut state,
        )
        .is_err());
        assert_eq!(serde_json::to_value(&state.config).ok(), before);
        assert!(apply_extractor_config(
            "html2text",
            &serde_json::json!({"timeout_ms": 900_001}),
            &mut state,
        )
        .is_err());
        assert!(
            apply_extractor_config("mdka", &serde_json::json!({"mode": "strict"}), &mut state,)
                .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocking_extraction_does_not_stall_tokio_reactor() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let complete = std::sync::Arc::new(AtomicBool::new(false));
        let completed = std::sync::Arc::clone(&complete);
        let extraction = async move {
            run_blocking(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                completed.store(true, Ordering::SeqCst);
                Ok::<(), String>(())
            })
            .await
        };
        let observed_early = async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            !complete.load(Ordering::SeqCst)
        };
        let (result, not_blocked) = tokio::join!(extraction, observed_early);
        assert!(result.is_ok());
        assert!(
            not_blocked,
            "long extraction blocked the current-thread reactor"
        );
    }

    #[tokio::test]
    async fn admission_is_bounded_and_cancellation_keeps_active_slot(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let state = AppState {
            score_store: ScoreStore::with_retention(temp.path().join("scores"), 8)?,
            extractor_states: Arc::new(RwLock::new(ExtractorStates::default())),
            states_path: temp.path().join("extractor_states.json"),
            jobs: Arc::new(Semaphore::new(1)),
        };
        let admission = reserve_job(&state).map_err(|_| "failed to reserve first slot")?;
        assert_eq!(
            reserve_job(&state).err(),
            Some(StatusCode::TOO_MANY_REQUESTS)
        );
        let active_slot = Arc::clone(&admission);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let running = tokio::spawn(run_blocking(move || {
            let _admission = active_slot;
            let _ = started_tx.send(());
            std::thread::sleep(std::time::Duration::from_millis(90));
            Ok::<(), String>(())
        }));
        started_rx.await?;
        drop(admission);
        assert_eq!(
            reserve_job(&state).err(),
            Some(StatusCode::TOO_MANY_REQUESTS),
            "disconnecting the request freed an active blocking job's slot"
        );
        running.await??;
        assert!(reserve_job(&state).is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn saturated_server_rejects_new_run_before_network_io(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let state = AppState {
            score_store: ScoreStore::with_retention(temp.path().join("scores"), 8)?,
            extractor_states: Arc::new(RwLock::new(ExtractorStates::default())),
            states_path: temp.path().join("extractor_states.json"),
            jobs: Arc::new(Semaphore::new(1)),
        };
        let _active = reserve_job(&state).map_err(|_| "could not acquire active slot")?;
        let response = run_extraction(
            State(state),
            Json(RunRequest {
                url: "https://example.invalid/should-not-fetch".to_string(),
                acquisition: crate::acquisition::Acquisition::Http,
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        Ok(())
    }

    #[tokio::test]
    async fn unknown_extractor_toggle_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let state = AppState {
            score_store: ScoreStore::with_retention(temp.path().join("scores"), 8)?,
            extractor_states: Arc::new(RwLock::new(ExtractorStates::default())),
            states_path: temp.path().join("extractor_states.json"),
            jobs: Arc::new(Semaphore::new(1)),
        };
        let response = toggle_extractor(
            State(state.clone()),
            Path(("unregistered".to_string(), true)),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(!state
            .extractor_states
            .read()
            .await
            .states
            .contains_key("unregistered"));
        Ok(())
    }

    #[test]
    fn apply_extractor_config_accepts_direct_payloads() {
        let mut state = ExtractorState::default();

        assert!(apply_extractor_config(
            "htmd",
            &serde_json::json!({
                "skip_tags": ["nav", "aside"],
                "heading_style": "setex"
            }),
            &mut state,
        )
        .is_ok());

        assert_eq!(state.config.htmd.heading_style, "setex");
        assert_eq!(state.config.htmd.skip_tags, vec!["nav", "aside"]);
        assert_eq!(state.config.skip_tags, vec!["nav", "aside"]);
    }
}
