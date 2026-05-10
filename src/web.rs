use crate::extractor_config::{ExtractorConfig, ExtractorState, ExtractorStates};
use crate::scores::ScoreStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    score_store: ScoreStore,
    extractor_states: Arc<RwLock<ExtractorStates>>,
    states_path: PathBuf,
}

pub async fn start_server(port: u16, data_dir: PathBuf) -> Result<(), String> {
    let states_path = data_dir.join("extractor_states.json");
    let scores_path = data_dir.join("scores");

    let score_store = ScoreStore::new(scores_path);
    let extractor_states = ExtractorStates::load(&states_path);

    let state = AppState {
        score_store,
        extractor_states: Arc::new(RwLock::new(extractor_states)),
        states_path,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index))
        .route("/scores", get(list_scores))
        .route("/scores/{id}", get(get_score))
        .route("/scores/{id}", delete(delete_score))
        .route("/scores/{id}/grade", patch(update_grade))
        .route("/scores/{id}/output/{name}", get(get_output))
        .route("/scores/{id}/preview-settings", post(preview_settings))
        .route("/scores/{id}/compare-settings", post(compare_settings))
        .route("/run", post(run_extraction))
        .route("/run-single", post(run_single_extraction))
        .route("/t/{name}/{enabled}", post(toggle_extractor))
        .route("/c/{name}", post(configure_extractor))
        .route("/states", get(get_all_states))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("Server running on http://{}", addr);
    axum::serve(listener, app).await.map_err(|e| e.to_string())
}

async fn index() -> impl IntoResponse {
    Html(HTML)
}

async fn list_scores(State(state): State<AppState>) -> impl IntoResponse {
    let scores = state.score_store.list().unwrap_or_default();
    Json(scores)
}

async fn get_score(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.score_store.load(&id) {
        Ok(score) => Json(score).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Score not found").into_response(),
    }
}

async fn delete_score(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let score = match state.score_store.load(&id) {
        Ok(score) => score,
        Err(_) => return (StatusCode::NOT_FOUND, "Score not found").into_response(),
    };

    match state.score_store.delete(&id) {
        Ok(_) => {
            if let Some(output_dir) = score
                .extractor_results
                .first()
                .and_then(|result| std::path::Path::new(&result.output_file).parent())
            {
                let _ = std::fs::remove_dir_all(output_dir);
            }
            "Deleted".into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Score not found").into_response(),
    }
}

async fn run_extraction(
    State(state): State<AppState>,
    Json(payload): Json<RunRequest>,
) -> impl IntoResponse {
    let html = match fetch_html(&payload.url).await {
        Ok(h) => h,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let states = state.extractor_states.read().await.clone();
    let score = crate::scores::run_extraction(&payload.url, &html, &states, &state.score_store);
    Json(score).into_response()
}

async fn run_single_extraction(
    State(state): State<AppState>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let html = match fetch_html(&payload.url).await {
        Ok(h) => h,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let mut states = state.extractor_states.read().await.clone();
    for extractor_state in states.states.values_mut() {
        extractor_state.enabled = false;
    }

    let entry = states
        .states
        .entry(payload.extractor.clone())
        .or_insert_with(ExtractorState::default);
    entry.enabled = true;
    if let Some(config) = &payload.config {
        apply_extractor_config(&payload.extractor, config, entry);
    }

    let score = crate::scores::run_extraction(&payload.url, &html, &states, &state.score_store);
    Json(score).into_response()
}

async fn compare_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let base_score = match state.score_store.load(&id) {
        Ok(score) => score,
        Err(_) => return (StatusCode::NOT_FOUND, "Score not found").into_response(),
    };

    let html = match load_score_source_html(&base_score).await {
        Ok(content) => content,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let baseline_config = base_score
        .settings_snapshot
        .states
        .get(&payload.extractor)
        .map(|state| state.config.clone())
        .unwrap_or_default();

    let candidate_config = if let Some(config) = &payload.config {
        let mut candidate_state = crate::extractor_config::ExtractorState {
            enabled: true,
            config: baseline_config.clone(),
        };
        apply_extractor_config(&payload.extractor, config, &mut candidate_state);
        candidate_state.config
    } else {
        baseline_config.clone()
    };

    let score = crate::scores::compare_single_extractor_settings(
        &base_score.url,
        &html,
        &payload.extractor,
        baseline_config,
        candidate_config,
        &state.score_store,
    );
    Json(score).into_response()
}

async fn preview_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SingleRunRequest>,
) -> impl IntoResponse {
    let base_score = match state.score_store.load(&id) {
        Ok(score) => score,
        Err(_) => return (StatusCode::NOT_FOUND, "Score not found").into_response(),
    };

    let html = match load_score_source_html(&base_score).await {
        Ok(content) => content,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let baseline_config = base_score
        .settings_snapshot
        .states
        .get(&payload.extractor)
        .map(|state| state.config.clone())
        .unwrap_or_default();

    let candidate_config = if let Some(config) = &payload.config {
        let mut candidate_state = crate::extractor_config::ExtractorState {
            enabled: true,
            config: baseline_config.clone(),
        };
        apply_extractor_config(&payload.extractor, config, &mut candidate_state);
        candidate_state.config
    } else {
        baseline_config
    };

    let preview = crate::scores::preview_single_extractor_settings(
        &base_score.url,
        &html,
        &payload.extractor,
        candidate_config,
    );
    Json(preview).into_response()
}

async fn toggle_extractor(
    State(state): State<AppState>,
    Path((name, enabled)): Path<(String, bool)>,
) -> impl IntoResponse {
    let mut states = state.extractor_states.write().await;
    if let Some(s) = states.states.get_mut(&name) {
        s.enabled = enabled;
    } else {
        let mut cfg = ExtractorConfig::default();
        match name.as_str() {
            "html2text" => {
                cfg.html2text = crate::extractor_config::Html2TextConfig {
                    max_wrap_width: 1000,
                    ..Default::default()
                }
            }
            "htmd" => {
                cfg.htmd = crate::extractor_config::HtmdConfig {
                    skip_tags: vec!["nav".to_string(), "script".to_string()],
                    ..Default::default()
                };
                cfg.skip_tags = crate::extractor_config::DEFAULT_SKIP_TAGS
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
            }
            "mdka" => {
                cfg.mdka = crate::extractor_config::MdkaConfig {
                    mode: "balanced".to_string(),
                    ..Default::default()
                }
            }
            "readable-readability" => {
                cfg.readable_readability = crate::extractor_config::ReadableReadabilityConfig {
                    strip_unlikelys: true,
                    weight_classes: true,
                    ..Default::default()
                }
            }
            "dom_smoothie" => {
                cfg.dom_smoothie = crate::extractor_config::DomSmoothieConfig {
                    ..Default::default()
                }
            }
            "august" => cfg.augus_max_width = usize::MAX,
            "html2md-rs" => {
                cfg.html2md_rs = crate::extractor_config::Html2MdRsConfig {
                    ignore_tags: vec!["nav".to_string(), "script".to_string()],
                };
                cfg.skip_tags = crate::extractor_config::DEFAULT_SKIP_TAGS
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
            }
            "turndown" => {
                cfg.turndown = crate::extractor_config::TurndownConfig::default();
            }
            "percollate" => {
                cfg.percollate = crate::extractor_config::PercollateConfig::default();
            }
            "mdream" => {
                cfg.mdream = crate::extractor_config::MdreamConfig::default();
            }
            "trafilatura" => {
                cfg.trafilatura = crate::extractor_config::TrafilaturaConfig::default();
            }
            "html2text-py" => {
                cfg.html2text_py = crate::extractor_config::Html2TextPythonConfig::default();
            }
            "markdownify" => {
                cfg.markdownify = crate::extractor_config::MarkdownifyConfig::default();
            }
            "lightpanda" => {
                cfg.lightpanda = crate::extractor_config::LightpandaConfig::default();
            }
            "webclaw" => {
                cfg.webclaw = crate::extractor_config::WebclawConfig::default();
            }
            "e2m" => {
                cfg.e2m = crate::extractor_config::E2mConfig::default();
            }
            "html-to-markdown-go" => {
                cfg.html_to_markdown_go =
                    crate::extractor_config::HtmlToMarkdownGoConfig::default();
            }
            _ => {}
        }
        states.states.insert(
            name.clone(),
            ExtractorState {
                enabled,
                config: cfg,
            },
        );
    }
    let _ = states.save(&state.states_path);
    (StatusCode::OK, "Toggled").into_response()
}

async fn configure_extractor(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut states = state.extractor_states.write().await;
    if let Some(s) = states.states.get_mut(&name) {
        apply_extractor_config(&name, &config, s);
    }
    let _ = states.save(&state.states_path);
    (StatusCode::OK, "Configured").into_response()
}

fn extractor_config_payload<'a>(config: &'a serde_json::Value, key: &str) -> &'a serde_json::Value {
    config.get(key).unwrap_or(config)
}

fn apply_extractor_config(name: &str, config: &serde_json::Value, state: &mut ExtractorState) {
    match name {
        "html2text" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "html2text").clone())
            {
                state.config.html2text = cfg;
            }
        }
        "htmd" => {
            if let Ok(cfg) = serde_json::from_value::<crate::extractor_config::HtmdConfig>(
                extractor_config_payload(config, "htmd").clone(),
            ) {
                state.config.skip_tags = cfg.skip_tags.clone();
                state.config.htmd = cfg;
            }
        }
        "html2md-rs" => {
            if let Ok(cfg) = serde_json::from_value::<crate::extractor_config::Html2MdRsConfig>(
                extractor_config_payload(config, "html2md_rs").clone(),
            ) {
                state.config.skip_tags = cfg.ignore_tags.clone();
                state.config.html2md_rs = cfg;
            }
        }
        "mdka" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "mdka").clone())
            {
                state.config.mdka = cfg;
            }
        }
        "readable-readability" => {
            if let Ok(cfg) = serde_json::from_value(
                extractor_config_payload(config, "readable_readability").clone(),
            ) {
                state.config.readable_readability = cfg;
            }
        }
        "dom_smoothie" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "dom_smoothie").clone())
            {
                state.config.dom_smoothie = cfg;
            }
        }
        "august" => {
            if let Some(w) = config
                .get("max_width")
                .or_else(|| config.get("augus_max_width"))
                .and_then(|v| v.as_u64())
            {
                state.config.augus_max_width = w as usize;
            }
        }
        "turndown" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "turndown").clone())
            {
                state.config.turndown = cfg;
            }
        }
        "percollate" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "percollate").clone())
            {
                state.config.percollate = cfg;
            }
        }
        "mdream" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "mdream").clone())
            {
                state.config.mdream = cfg;
            }
        }
        "trafilatura" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "trafilatura").clone())
            {
                state.config.trafilatura = cfg;
            }
        }
        "html2text-py" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "html2text_py").clone())
            {
                state.config.html2text_py = cfg;
            }
        }
        "markdownify" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "markdownify").clone())
            {
                state.config.markdownify = cfg;
            }
        }
        "lightpanda" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "lightpanda").clone())
            {
                state.config.lightpanda = cfg;
            }
        }
        "webclaw" => {
            if let Ok(cfg) =
                serde_json::from_value(extractor_config_payload(config, "webclaw").clone())
            {
                state.config.webclaw = cfg;
            }
        }
        "e2m" => {
            if let Ok(cfg) = serde_json::from_value(extractor_config_payload(config, "e2m").clone())
            {
                state.config.e2m = cfg;
            }
        }
        "html-to-markdown-go" => {
            if let Ok(cfg) = serde_json::from_value(
                extractor_config_payload(config, "html_to_markdown_go").clone(),
            ) {
                state.config.html_to_markdown_go = cfg;
            }
        }
        _ => {}
    }
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

    match state
        .score_store
        .update_grade(&id, &payload.extractor, payload.grade)
    {
        Ok(score) => Json(score).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Score not found").into_response(),
    }
}

async fn get_output(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Response {
    let score = match state.score_store.load(&id) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "Score not found").into_response(),
    };

    let output_path = match score
        .extractor_results
        .iter()
        .find(|result| result.name == name)
    {
        Some(result) => PathBuf::from(&result.output_file),
        None => return (StatusCode::NOT_FOUND, "Output not found").into_response(),
    };

    match std::fs::read_to_string(&output_path) {
        Ok(content) => content.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Output not found").into_response(),
    }
}

async fn fetch_html(url: &str) -> Result<String, String> {
    let response = ureq::get(url).call().map_err(|e| e.to_string())?;
    let mut s = String::new();
    response
        .into_reader()
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    Ok(s)
}

async fn load_score_source_html(score: &crate::scores::Score) -> Result<String, String> {
    if !score.source_html_file.is_empty() {
        match std::fs::read_to_string(&score.source_html_file) {
            Ok(content) => Ok(content),
            Err(_) => fetch_html(&score.url).await,
        }
    } else {
        fetch_html(&score.url).await
    }
}

#[derive(serde::Deserialize)]
pub struct RunRequest {
    pub url: String,
}

#[derive(serde::Deserialize)]
pub struct SingleRunRequest {
    #[serde(default)]
    pub url: String,
    pub extractor: String,
    pub config: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct GradeRequest {
    pub extractor: String,
    pub grade: u8,
}

static HTML: &str = include_str!("web_ui.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_extractor_config_accepts_nested_payloads() {
        let mut state = ExtractorState::default();

        apply_extractor_config(
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
        );

        assert_eq!(state.config.html2text.max_wrap_width, 42);
        assert!(state.config.html2text.raw_mode);
        assert!(state.config.html2text.no_link_wrapping);
    }

    #[test]
    fn apply_extractor_config_accepts_direct_payloads() {
        let mut state = ExtractorState::default();

        apply_extractor_config(
            "htmd",
            &serde_json::json!({
                "skip_tags": ["nav", "aside"],
                "heading_style": "setex"
            }),
            &mut state,
        );

        assert_eq!(state.config.htmd.heading_style, "setex");
        assert_eq!(state.config.htmd.skip_tags, vec!["nav", "aside"]);
        assert_eq!(state.config.skip_tags, vec!["nav", "aside"]);
    }
}
