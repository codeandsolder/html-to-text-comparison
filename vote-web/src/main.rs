//! Blind side-by-side voting on the immutable HTML extraction corpus.
//! Architecture follows rust-web-fullstack: Axum HTTP, typed DTOs,
//! SQLx persistence, explicit readiness, and a same-origin write boundary.

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path as RoutePath, State};
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

const RUN_ID: &str = "comparable-14-20261115";
const PREVIEW_LIMIT: usize = 160_000;

#[derive(Deserialize)]
struct Manifest {
    records: Vec<ManifestRow>,
}

#[derive(Deserialize)]
struct ManifestRow {
    capture_id: String,
    url: String,
    query_id: String,
    file: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RunReport {
    results: Vec<RunRow>,
}

#[derive(Deserialize)]
struct RunRow {
    capture_id: String,
    extractor: String,
    status: String,
    output_bytes: u64,
    output_sha256: Option<String>,
}

#[derive(Clone)]
struct Extraction {
    engine: String,
    status: String,
    text: Arc<str>,
}

#[derive(Clone)]
struct Page {
    capture_id: String,
    url: String,
    cohort: String,
    outputs: Vec<Extraction>,
}

#[derive(Clone)]
struct AppState {
    pages: Arc<Vec<Page>>,
    lookup: Arc<HashMap<String, usize>>,
    engines: Arc<Vec<String>>,
    pool: SqlitePool,
}

#[derive(Debug, Serialize)]
struct ExtractionView {
    engine: String,
    status: String,
    chars: usize,
    text: String,
    clipped: bool,
    full_url: String,
}

#[derive(Debug, Serialize)]
struct BallotView {
    id: String,
    capture_id: String,
    url: String,
    cohort: String,
    page_index: usize,
    page_count: usize,
    voted_total: i64,
    left: ExtractionView,
    right: ExtractionView,
}

#[derive(Deserialize)]
struct VoteRequest {
    ballot_id: String,
    choice: String,
}

#[derive(Deserialize)]
struct UndoRequest {
    ballot_id: String,
}

#[derive(Serialize)]
struct VoteResponse {
    ok: bool,
    total: i64,
}

#[derive(Serialize)]
struct Rankings {
    total_votes: usize,
    pages_covered: usize,
    results: Vec<RankRow>,
}

#[derive(Clone, Serialize)]
struct RankRow {
    engine: String,
    elo: f64,
    comparisons: usize,
    wins: usize,
    ties: usize,
    losses: usize,
}

#[derive(Debug)]
struct Vote {
    page_id: String,
    left_engine: String,
    right_engine: String,
    choice: String,
}

type ApiResult<T> = Result<T, (StatusCode, &'static str)>;

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn checksum(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn load_corpus(dataset: &Path) -> Result<(Vec<Page>, Vec<String>), Box<dyn std::error::Error>> {
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(dataset.join("manifest.json"))?)?;
    let run: RunReport = serde_json::from_slice(&std::fs::read(
        dataset.join("runs").join(RUN_ID).join("report.json"),
    )?)?;
    if manifest.records.len() != 100 {
        return Err("expected exactly 100 captured HTML pages".into());
    }
    let mut pages = Vec::with_capacity(manifest.records.len());
    let mut by_id = HashMap::new();
    for (position, entry) in manifest.records.into_iter().enumerate() {
        if !is_safe_name(&entry.capture_id) || !entry.file.starts_with("html/") {
            return Err("unsafe capture id or file path in manifest".into());
        }
        let expected_file = format!("html/{:03}-{}.html", position + 1, entry.capture_id);
        if entry.file != expected_file {
            return Err("unexpected corpus file layout".into());
        }
        let parsed_url = url::Url::parse(&entry.url)?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err("capture URL is not HTTP(S)".into());
        }
        let actual = std::fs::read(dataset.join(&entry.file))?;
        if checksum(&actual) != entry.sha256 {
            return Err(format!("source SHA mismatch: {}", entry.capture_id).into());
        }
        if by_id.insert(entry.capture_id.clone(), position).is_some() {
            return Err("duplicate capture in manifest".into());
        }
        pages.push(Page {
            capture_id: entry.capture_id,
            url: entry.url,
            cohort: entry.query_id,
            outputs: Vec::new(),
        });
    }
    let mut engines = BTreeMap::new();
    for item in run.results {
        if !is_safe_name(&item.extractor) || !is_safe_name(&item.capture_id) {
            return Err("unsafe extractor or capture identifier".into());
        }
        let Some(&idx) = by_id.get(&item.capture_id) else {
            return Err("result references an unknown capture".into());
        };
        if pages[idx]
            .outputs
            .iter()
            .any(|o| o.engine == item.extractor)
        {
            return Err("duplicate extractor/capture result".into());
        }
        let path = dataset
            .join("runs")
            .join(RUN_ID)
            .join("outputs")
            .join(&item.extractor)
            .join(format!("{}.txt", item.capture_id));
        let bytes = std::fs::read(path)?;
        if bytes.len() as u64 != item.output_bytes
            || item.output_sha256.as_deref() != Some(checksum(&bytes).as_str())
        {
            return Err(format!(
                "output hash mismatch: {} / {}",
                item.capture_id, item.extractor
            )
            .into());
        }
        engines.insert(item.extractor.clone(), ());
        pages[idx].outputs.push(Extraction {
            engine: item.extractor,
            status: item.status,
            text: String::from_utf8_lossy(&bytes).into_owned().into(),
        });
    }
    let engine_names: Vec<String> = engines.into_keys().collect();
    if engine_names.len() < 2 || pages.iter().any(|p| p.outputs.len() != engine_names.len()) {
        return Err("incomplete extractor-by-page matrix".into());
    }
    for page in &mut pages {
        page.outputs.sort_by(|a, b| a.engine.cmp(&b.engine));
    }
    Ok((pages, engine_names))
}

async fn init_db(path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ballots (
             id TEXT PRIMARY KEY, page_id TEXT NOT NULL,
             left_engine TEXT NOT NULL, right_engine TEXT NOT NULL,
             used INTEGER NOT NULL DEFAULT 0,
             created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         )",
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS votes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ballot_id TEXT NOT NULL UNIQUE REFERENCES ballots(id),
            page_id TEXT NOT NULL, left_engine TEXT NOT NULL,
            right_engine TEXT NOT NULL,
            choice TEXT NOT NULL CHECK(choice IN ('left','equal','right')),
            submitted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS votes_by_page ON votes(page_id)")
        .execute(&pool)
        .await?;
    Ok(pool)
}

async fn load_votes(pool: &SqlitePool) -> Result<Vec<Vote>, sqlx::Error> {
    let rows = sqlx::query("SELECT page_id,left_engine,right_engine,choice FROM votes ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| Vote {
            page_id: row.get("page_id"),
            left_engine: row.get("left_engine"),
            right_engine: row.get("right_engine"),
            choice: row.get("choice"),
        })
        .collect())
}

fn preview(state: &AppState, page: &Page, engine: &str) -> ApiResult<ExtractionView> {
    let extraction = page
        .outputs
        .iter()
        .find(|o| o.engine == engine)
        .ok_or((StatusCode::NOT_FOUND, "Unknown extractor"))?;
    let full = extraction.text.as_ref();
    let clipped = full.len() > PREVIEW_LIMIT;
    let length = if clipped {
        let mut pos = PREVIEW_LIMIT;
        while !full.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    } else {
        full.len()
    };
    let _ = state;
    Ok(ExtractionView {
        engine: engine.to_owned(),
        status: extraction.status.clone(),
        chars: full.chars().count(),
        text: full[..length].to_owned(),
        clipped,
        full_url: format!("/api/output/{}/{}", page.capture_id, engine),
    })
}

fn pick_next<'a>(state: &'a AppState, votes: &[Vote]) -> (&'a Page, &'a str, &'a str) {
    let mut page_count: HashMap<&str, usize> = HashMap::new();
    let mut engine_count: HashMap<&str, usize> = HashMap::new();
    let mut pair_count: HashMap<(&str, &str), usize> = HashMap::new();
    let mut page_pair_count: HashMap<(&str, &str, &str), usize> = HashMap::new();
    for vote in votes {
        *page_count.entry(&vote.page_id).or_default() += 1;
        *engine_count.entry(&vote.left_engine).or_default() += 1;
        *engine_count.entry(&vote.right_engine).or_default() += 1;
        let (a, b) = if vote.left_engine <= vote.right_engine {
            (vote.left_engine.as_str(), vote.right_engine.as_str())
        } else {
            (vote.right_engine.as_str(), vote.left_engine.as_str())
        };
        *pair_count.entry((a, b)).or_default() += 1;
        *page_pair_count.entry((&vote.page_id, a, b)).or_default() += 1;
    }
    let min_page = state
        .pages
        .iter()
        .map(|p| page_count.get(p.capture_id.as_str()).copied().unwrap_or(0))
        .min()
        .unwrap_or(0);
    let least_pages: Vec<&Page> = state
        .pages
        .iter()
        .filter(|p| page_count.get(p.capture_id.as_str()).copied().unwrap_or(0) == min_page)
        .collect();
    let page = least_pages[rand_index(least_pages.len())];

    let mut best = (usize::MAX, usize::MAX, usize::MAX);
    let mut candidates = Vec::new();
    for (i, left) in state.engines.iter().enumerate() {
        for right in state.engines.iter().skip(i + 1) {
            let a = left.as_str();
            let b = right.as_str();
            let criterion = (
                page_pair_count
                    .get(&(page.capture_id.as_str(), a, b))
                    .copied()
                    .unwrap_or(0),
                engine_count.get(a).copied().unwrap_or(0)
                    + engine_count.get(b).copied().unwrap_or(0),
                pair_count.get(&(a, b)).copied().unwrap_or(0),
            );
            if criterion < best {
                best = criterion;
                candidates.clear();
            }
            if criterion == best {
                candidates.push((a, b));
            }
        }
    }
    let (a, b) = candidates[rand_index(candidates.len())];
    if rand_index(2) == 0 {
        (page, a, b)
    } else {
        (page, b, a)
    }
}

fn rand_index(len: usize) -> usize {
    // Selection is exploratory (not a cryptographic draw). All possible
    // indices are reachable, and the pair order is randomized separately.
    usize::try_from(rand::random::<u32>()).unwrap_or(0) % len
}

async fn make_ballot(state: &AppState) -> ApiResult<BallotView> {
    let votes = load_votes(&state.pool).await.map_err(internal)?;
    let (page, left, right) = pick_next(state, &votes);
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ballots(id,page_id,left_engine,right_engine) VALUES(?,?,?,?)")
        .bind(&id)
        .bind(&page.capture_id)
        .bind(left)
        .bind(right)
        .execute(&state.pool)
        .await
        .map_err(internal)?;
    ballot_view(
        state,
        id,
        page,
        left,
        right,
        i64::try_from(votes.len()).unwrap_or(i64::MAX),
    )
}

fn ballot_view(
    state: &AppState,
    id: String,
    page: &Page,
    left: &str,
    right: &str,
    total: i64,
) -> ApiResult<BallotView> {
    let page_index = state.lookup[page.capture_id.as_str()] + 1;
    Ok(BallotView {
        id,
        capture_id: page.capture_id.clone(),
        url: page.url.clone(),
        cohort: page.cohort.clone(),
        page_index,
        page_count: state.pages.len(),
        voted_total: total,
        left: preview(state, page, left)?,
        right: preview(state, page, right)?,
    })
}

fn internal<E: std::fmt::Display>(error: E) -> (StatusCode, &'static str) {
    error!(%error, "voter database operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database operation failed",
    )
}

async fn next(State(state): State<AppState>) -> ApiResult<Json<BallotView>> {
    Ok(Json(make_ballot(&state).await?))
}

fn same_origin(headers: &HeaderMap) -> ApiResult<()> {
    let Some(origin) = headers.get(ORIGIN) else {
        // Non-browser clients may omit Origin; transport still restricted
        // to loopback/Tailscale. Browsers always send it for JSON POST.
        return Ok(());
    };
    let origin = origin
        .to_str()
        .map_err(|_| (StatusCode::FORBIDDEN, "Invalid origin"))?;
    let url = url::Url::parse(origin).map_err(|_| (StatusCode::FORBIDDEN, "Invalid origin"))?;
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::FORBIDDEN, "Missing host"))?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str() != host.split(':').next()
        || (url.scheme() == "https" && url.port_or_known_default() != Some(443))
        || (url.scheme() == "http"
            && url.port_or_known_default() != Some(80)
            && !host.ends_with(&format!(":{}", url.port_or_known_default().unwrap_or(0))))
    {
        return Err((StatusCode::FORBIDDEN, "Cross-origin voting is not allowed"));
    }
    Ok(())
}

async fn vote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VoteRequest>,
) -> ApiResult<Json<VoteResponse>> {
    same_origin(&headers)?;
    if !matches!(request.choice.as_str(), "left" | "equal" | "right") {
        return Err((StatusCode::BAD_REQUEST, "Expected left, equal or right"));
    }
    if Uuid::parse_str(&request.ballot_id).is_err() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ballot identifier"));
    }
    let mut transaction = state.pool.begin().await.map_err(internal)?;
    let row = sqlx::query("SELECT page_id,left_engine,right_engine,used FROM ballots WHERE id=?")
        .bind(&request.ballot_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(internal)?
        .ok_or((StatusCode::NOT_FOUND, "Ballot not found"))?;
    let used: i64 = row.get("used");
    if used != 0 {
        return Err((StatusCode::CONFLICT, "Ballot was already used"));
    }
    let page: String = row.get("page_id");
    let left: String = row.get("left_engine");
    let right: String = row.get("right_engine");
    if left == right
        || !state.lookup.contains_key(&page)
        || !state.engines.contains(&left)
        || !state.engines.contains(&right)
    {
        return Err((
            StatusCode::CONFLICT,
            "Ballot does not match the current corpus",
        ));
    }
    let updated = sqlx::query("UPDATE ballots SET used=1 WHERE id=? AND used=0")
        .bind(&request.ballot_id)
        .execute(&mut *transaction)
        .await
        .map_err(internal)?;
    if updated.rows_affected() != 1 {
        return Err((StatusCode::CONFLICT, "Ballot already used"));
    }
    sqlx::query(
        "INSERT INTO votes(ballot_id,page_id,left_engine,right_engine,choice) VALUES(?,?,?,?,?)",
    )
    .bind(&request.ballot_id)
    .bind(&page)
    .bind(&left)
    .bind(&right)
    .bind(&request.choice)
    .execute(&mut *transaction)
    .await
    .map_err(internal)?;
    transaction.commit().await.map_err(internal)?;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM votes")
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    Ok(Json(VoteResponse { ok: true, total }))
}

async fn undo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UndoRequest>,
) -> ApiResult<Json<VoteResponse>> {
    same_origin(&headers)?;
    if Uuid::parse_str(&request.ballot_id).is_err() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ballot identifier"));
    }
    let mut tx = state.pool.begin().await.map_err(internal)?;
    let result = sqlx::query("DELETE FROM votes WHERE ballot_id=?")
        .bind(&request.ballot_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Vote not found"));
    }
    sqlx::query("UPDATE ballots SET used=0 WHERE id=?")
        .bind(&request.ballot_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM votes")
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    Ok(Json(VoteResponse { ok: true, total }))
}

async fn rankings(State(state): State<AppState>) -> ApiResult<Json<Rankings>> {
    let votes = load_votes(&state.pool).await.map_err(internal)?;
    let mut standings: BTreeMap<String, RankRow> = state
        .engines
        .iter()
        .map(|engine| {
            (
                engine.clone(),
                RankRow {
                    engine: engine.clone(),
                    elo: 1200.0,
                    comparisons: 0,
                    wins: 0,
                    ties: 0,
                    losses: 0,
                },
            )
        })
        .collect();
    let mut page_set = std::collections::HashSet::new();
    for v in &votes {
        let Some(left) = standings.get(&v.left_engine).cloned() else {
            continue;
        };
        let Some(right) = standings.get(&v.right_engine).cloned() else {
            continue;
        };
        if v.left_engine == v.right_engine {
            continue;
        }
        page_set.insert(v.page_id.as_str());
        let left_score = match v.choice.as_str() {
            "left" => 1.0,
            "right" => 0.0,
            "equal" => 0.5,
            _ => continue,
        };
        let expected_left = 1.0 / (1.0 + 10.0_f64.powf((right.elo - left.elo) / 400.0));
        let change = 24.0 * (left_score - expected_left);
        let a = standings.get_mut(&v.left_engine).expect("known extractor");
        a.elo += change;
        a.comparisons += 1;
        if left_score == 1.0 {
            a.wins += 1;
        } else if left_score == 0.0 {
            a.losses += 1;
        } else {
            a.ties += 1;
        }
        let b = standings.get_mut(&v.right_engine).expect("known extractor");
        b.elo -= change;
        b.comparisons += 1;
        if left_score == 0.0 {
            b.wins += 1;
        } else if left_score == 1.0 {
            b.losses += 1;
        } else {
            b.ties += 1;
        }
    }
    let mut results: Vec<_> = standings.into_values().collect();
    results.sort_by(|a, b| {
        b.elo
            .total_cmp(&a.elo)
            .then_with(|| a.engine.cmp(&b.engine))
    });
    Ok(Json(Rankings {
        total_votes: votes.len(),
        pages_covered: page_set.len(),
        results,
    }))
}

async fn output(
    State(state): State<AppState>,
    RoutePath((capture, engine)): RoutePath<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    let Some(&idx) = state.lookup.get(&capture) else {
        return Err((StatusCode::NOT_FOUND, "Capture not found"));
    };
    let Some(extraction) = state.pages[idx].outputs.iter().find(|o| o.engine == engine) else {
        return Err((StatusCode::NOT_FOUND, "Extractor not found"));
    };
    Ok((
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        extraction.text.to_string(),
    ))
}

async fn ready(State(state): State<AppState>) -> ApiResult<&'static str> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(internal)?;
    Ok("ready")
}

fn asset(body: &'static str, mime: &'static str) -> Response {
    let mut response = ([(CONTENT_TYPE, mime)], body).into_response();
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
             img-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'",
        ),
    );
    response.headers_mut().insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn home() -> Response {
    let body = Html(include_str!("../static/index.html"));
    let mut response = body.into_response();
    response.headers_mut().insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
             img-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'",
        ),
    );
    response.headers_mut().insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn styles() -> Response {
    asset(
        include_str!("../static/site.css"),
        "text/css; charset=utf-8",
    )
}
async fn script() -> Response {
    asset(
        include_str!("../static/app.js"),
        "text/javascript; charset=utf-8",
    )
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/site.css", get(styles))
        .route("/app.js", get(script))
        .route("/health", get(|| async { "ok" }))
        .route("/readyz", get(ready))
        .route("/api/next", get(next))
        .route("/api/vote", post(vote))
        .route("/api/undo", post(undo))
        .route("/api/rankings", get(rankings))
        .route("/api/output/{capture}/{engine}", get(output))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "html_extraction_voter=info,tower_http=info".into()),
        )
        .init();
    let dataset = PathBuf::from(
        env::var("VOTER_DATASET")
            .map_err(|_| "VOTER_DATASET must point to a private 100-page corpus")?,
    );
    let database = PathBuf::from(
        env::var("VOTER_DATABASE")
            .unwrap_or_else(|_| dataset.join("voting/votes.sqlite").display().to_string()),
    );
    let bind: SocketAddr = env::var("VOTER_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18578".to_owned())
        .parse()?;
    if !bind.ip().is_loopback() {
        return Err("VOTER_BIND must be loopback; use Certifex for HTTPS ingress".into());
    }
    let (pages, engines) = load_corpus(&dataset)?;
    let lookup: HashMap<String, usize> = pages
        .iter()
        .enumerate()
        .map(|(i, page)| (page.capture_id.clone(), i))
        .collect();
    let pool = init_db(&database).await?;
    info!(count = pages.len(), extractors = engines.len(), database = %database.display(),
        "verified extraction corpus and initialized durable vote storage");
    let state = AppState {
        pages: Arc::new(pages),
        lookup: Arc::new(lookup),
        engines: Arc::new(engines),
        pool,
    };
    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!(%bind, "HTML extraction voter listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation_prevents_path_traversal() {
        assert!(is_safe_name("5915-scam-recovery-fast-e3d21bb1f9"));
        assert!(!is_safe_name("../secrets"));
        assert!(!is_safe_name("file/name"));
        assert!(!is_safe_name(""));
    }

    #[tokio::test]
    async fn picker_covers_multiple_corpus_pages_and_engines() {
        let names = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let pages = vec!["p1", "p2", "p3"]
            .into_iter()
            .map(|capture_id| Page {
                capture_id: capture_id.to_owned(),
                url: "https://example.com".to_string(),
                cohort: "test".to_string(),
                outputs: names
                    .iter()
                    .map(|engine| Extraction {
                        engine: engine.to_string(),
                        status: "success".to_string(),
                        text: "content".into(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let state = AppState {
            pages: Arc::new(pages),
            lookup: Arc::new(HashMap::from([
                ("p1".to_string(), 0),
                ("p2".to_string(), 1),
                ("p3".to_string(), 2),
            ])),
            engines: Arc::new(names),
            pool: SqlitePoolOptions::new().connect_lazy_with(SqliteConnectOptions::new()),
        };
        let mut seen = Vec::new();
        let mut votes = Vec::new();
        for _ in 0..3 {
            let (page, left, right) = pick_next(&state, &votes);
            assert_ne!(left, right);
            seen.push(page.capture_id.clone());
            votes.push(Vote {
                page_id: page.capture_id.clone(),
                left_engine: left.to_owned(),
                right_engine: right.to_owned(),
                choice: "equal".to_owned(),
            });
        }
        seen.sort();
        assert_eq!(seen, ["p1", "p2", "p3"]);
    }
}
