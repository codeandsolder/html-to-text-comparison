use crate::extractor_config::ExtractorStates;
use crate::scores::run_cli_extractor;
use rand::seq::SliceRandom;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use similar::{Algorithm, DiffTag, TextDiff};
use std::collections::HashSet;
use std::io::Read;
use std::sync::OnceLock;
use url::Url;

const MAX_SAMPLE_COUNT: usize = 12;

#[derive(Debug, Clone, Serialize)]
pub struct BoilerplateDiffAnalysis {
    pub target_url: String,
    pub requested_links: usize,
    pub compared_links: usize,
    pub target_line_count: usize,
    pub eligible_target_lines: usize,
    pub selection_strategy: String,
    pub warnings: Vec<String>,
    pub samples: Vec<BoilerplateDiffSample>,
    pub lines: Vec<BoilerplateLineOverlap>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoilerplateDiffSample {
    pub url: String,
    pub tier: String,
    pub markdown_line_count: usize,
    pub matched_target_lines: usize,
    pub matched_target_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoilerplateLineOverlap {
    pub line_number: usize,
    pub text: String,
    pub overlap_count: usize,
    pub overlap_percent: f64,
    pub is_blank: bool,
}

#[derive(Debug, Clone)]
struct CandidateLink {
    url: Url,
    tier: CandidateTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateTier {
    SamePrefix,
    SameOrigin,
    AnyHttp,
}

impl CandidateTier {
    fn label(self) -> &'static str {
        match self {
            Self::SamePrefix => "same-prefix",
            Self::SameOrigin => "same-origin",
            Self::AnyHttp => "other-http",
        }
    }
}

pub fn analyze_boilerplate_diff(
    target_url: &str,
    target_html: &str,
    states: &ExtractorStates,
    requested_links: usize,
) -> Result<BoilerplateDiffAnalysis, String> {
    let parsed_target_url = Url::parse(target_url).map_err(|error| error.to_string())?;
    let requested_links = requested_links.clamp(1, MAX_SAMPLE_COUNT);
    let target_markdown = markdownify_html(target_html, states, &parsed_target_url)?;
    let target_lines = split_markdown_lines(&target_markdown);
    let normalized_target_lines = target_lines
        .iter()
        .map(|line| normalize_line(line))
        .collect::<Vec<_>>();
    let eligible_target_lines = normalized_target_lines
        .iter()
        .filter(|line| !line.is_empty())
        .count();

    let (candidate_pool, selection_strategy) =
        collect_candidate_links(&parsed_target_url, target_html, requested_links);
    if candidate_pool.is_empty() {
        return Err("No HTTP links were found on the target page.".to_string());
    }

    let mut warnings = Vec::new();
    if candidate_pool.len() < requested_links {
        warnings.push(format!(
            "Only {} candidate links were discovered for a {}-link sample.",
            candidate_pool.len(),
            requested_links
        ));
    }

    let mut overlap_counts = vec![0usize; target_lines.len()];
    let mut compared_samples = Vec::new();
    for candidate in candidate_pool {
        if compared_samples.len() >= requested_links {
            break;
        }

        let sample_html = match fetch_html(candidate.url.as_str()) {
            Ok(html) => html,
            Err(error) => {
                warnings.push(format!("Skipped {}: {}", candidate.url, error));
                continue;
            }
        };
        let sample_markdown = match markdownify_html(&sample_html, states, &candidate.url) {
            Ok(markdown) => markdown,
            Err(error) => {
                warnings.push(format!("Skipped {}: {}", candidate.url, error));
                continue;
            }
        };

        let sample_lines = split_markdown_lines(&sample_markdown);
        let normalized_sample_lines = sample_lines
            .iter()
            .map(|line| normalize_line(line))
            .collect::<Vec<_>>();
        let matched_target_lines = accumulate_equal_lines(
            &normalized_target_lines,
            &normalized_sample_lines,
            &mut overlap_counts,
        );
        compared_samples.push(BoilerplateDiffSample {
            url: candidate.url.to_string(),
            tier: candidate.tier.label().to_string(),
            markdown_line_count: sample_lines.len(),
            matched_target_lines,
            matched_target_percent: percent(matched_target_lines, eligible_target_lines),
        });
    }

    if compared_samples.is_empty() {
        return Err(format!(
            "All sampled links failed before diffing. {}",
            warnings.join(" ")
        ));
    }

    if compared_samples.len() < requested_links {
        warnings.push(format!(
            "Compared {} links instead of {} because the remaining candidates failed to fetch or convert with markdownify.",
            compared_samples.len(),
            requested_links
        ));
    }

    let compared_links = compared_samples.len();
    let lines = target_lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let is_blank = normalized_target_lines[index].is_empty();
            let overlap_count = if is_blank { 0 } else { overlap_counts[index] };
            BoilerplateLineOverlap {
                line_number: index + 1,
                text,
                overlap_count,
                overlap_percent: if is_blank {
                    0.0
                } else {
                    percent(overlap_count, compared_links)
                },
                is_blank,
            }
        })
        .collect();

    Ok(BoilerplateDiffAnalysis {
        target_url: target_url.to_string(),
        requested_links,
        compared_links,
        target_line_count: normalized_target_lines.len(),
        eligible_target_lines,
        selection_strategy,
        warnings,
        samples: compared_samples,
        lines,
    })
}

fn collect_candidate_links(
    target_url: &Url,
    target_html: &str,
    requested_links: usize,
) -> (Vec<CandidateLink>, String) {
    let prefix = parent_path_prefix(target_url);
    let mut same_prefix = Vec::new();
    let mut same_origin = Vec::new();
    let mut other_http = Vec::new();
    let mut seen = HashSet::new();

    for url in discover_links(target_url, target_html) {
        if !seen.insert(url.to_string()) {
            continue;
        }
        let candidate = CandidateLink {
            tier: classify_candidate_tier(target_url, &url, &prefix),
            url,
        };
        match candidate.tier {
            CandidateTier::SamePrefix => same_prefix.push(candidate),
            CandidateTier::SameOrigin => same_origin.push(candidate),
            CandidateTier::AnyHttp => other_http.push(candidate),
        }
    }

    let same_prefix_count = same_prefix.len();
    let same_origin_count = same_origin.len();
    let other_http_count = other_http.len();

    let mut rng = rand::thread_rng();
    same_prefix.shuffle(&mut rng);
    same_origin.shuffle(&mut rng);
    other_http.shuffle(&mut rng);

    let mut ordered = Vec::with_capacity(same_prefix.len() + same_origin.len() + other_http.len());
    ordered.extend(same_prefix);
    ordered.extend(same_origin);
    ordered.extend(other_http);

    let strategy = if same_prefix_count >= requested_links {
        format!(
            "Found {} links under the `{}` URL prefix and sampled only from that group.",
            same_prefix_count, prefix
        )
    } else if same_prefix_count + same_origin_count >= requested_links {
        format!(
            "Found {} same-prefix links under `{}` and filled the remaining slots from {} other same-origin links.",
            same_prefix_count, prefix, same_origin_count
        )
    } else {
        format!(
            "Found {} same-prefix links under `{}`, {} other same-origin links, and {} additional HTTP links. The sample spills outside the preferred prefix because the page did not expose enough close matches.",
            same_prefix_count, prefix, same_origin_count, other_http_count
        )
    };

    (ordered, strategy)
}

fn discover_links(target_url: &Url, html: &str) -> Vec<Url> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("valid link selector");
    document
        .select(&selector)
        .filter_map(|node| node.value().attr("href"))
        .filter_map(|href| target_url.join(href).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .filter_map(|mut url| {
            url.set_fragment(None);
            if url == *target_url {
                return None;
            }
            Some(url)
        })
        .collect()
}

fn classify_candidate_tier(target_url: &Url, candidate_url: &Url, prefix: &str) -> CandidateTier {
    if same_origin(target_url, candidate_url) && candidate_url.path().starts_with(prefix) {
        CandidateTier::SamePrefix
    } else if same_origin(target_url, candidate_url) {
        CandidateTier::SameOrigin
    } else {
        CandidateTier::AnyHttp
    }
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.domain() == right.domain()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn parent_path_prefix(url: &Url) -> String {
    let trimmed = url.path().trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    match trimmed.rsplit_once('/') {
        Some((parent, _)) if !parent.is_empty() => format!("{parent}/"),
        _ => "/".to_string(),
    }
}

fn fetch_html(url: &str) -> Result<String, String> {
    let response = ureq::get(url).call().map_err(|error| error.to_string())?;
    let mut html = String::new();
    response
        .into_reader()
        .read_to_string(&mut html)
        .map_err(|error| error.to_string())?;
    Ok(html)
}

fn markdownify_html(
    html: &str,
    states: &ExtractorStates,
    parsed_url: &Url,
) -> Result<String, String> {
    let markdown = run_cli_extractor("markdownify", html, states, parsed_url);
    if markdown.starts_with("[ERROR]") {
        return Err(markdown.trim().to_string());
    }
    if markdown.trim().is_empty() {
        return Err("markdownify returned empty output".to_string());
    }
    Ok(markdown)
}

fn split_markdown_lines(text: &str) -> Vec<String> {
    let normalized = text.replace('\r', "");
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut lines = normalized
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn normalize_line(line: &str) -> String {
    static BULLET_RE: OnceLock<Regex> = OnceLock::new();
    static ORDERED_RE: OnceLock<Regex> = OnceLock::new();
    static LINK_RE: OnceLock<Regex> = OnceLock::new();
    static PUNCT_RE: OnceLock<Regex> = OnceLock::new();
    static SPACE_RE: OnceLock<Regex> = OnceLock::new();

    let mut normalized = line.trim().to_lowercase();
    normalized = BULLET_RE
        .get_or_init(|| Regex::new(r"^[-*+]\s+").expect("valid bullet regex"))
        .replace(&normalized, "")
        .into_owned();
    normalized = ORDERED_RE
        .get_or_init(|| Regex::new(r"^\d+\.\s+").expect("valid ordered list regex"))
        .replace(&normalized, "")
        .into_owned();
    normalized = LINK_RE
        .get_or_init(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("valid markdown link regex"))
        .replace_all(&normalized, "$1 $2")
        .into_owned();
    normalized = PUNCT_RE
        .get_or_init(|| Regex::new(r"[^\p{L}\p{N}:/._ -]+").expect("valid cleanup regex"))
        .replace_all(&normalized, " ")
        .into_owned();
    SPACE_RE
        .get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
        .replace_all(&normalized, " ")
        .trim()
        .to_string()
}

fn diff_tokens(lines: &[String], prefix: &str) -> Vec<String> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if line.is_empty() {
                format!("{prefix}__blank_{index}")
            } else {
                line.clone()
            }
        })
        .collect()
}

fn accumulate_equal_lines(
    target_lines: &[String],
    sample_lines: &[String],
    overlap_counts: &mut [usize],
) -> usize {
    let target_tokens = diff_tokens(target_lines, "target");
    let sample_tokens = diff_tokens(sample_lines, "sample");
    let target_refs = target_tokens.iter().map(String::as_str).collect::<Vec<_>>();
    let sample_refs = sample_tokens.iter().map(String::as_str).collect::<Vec<_>>();
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .diff_slices(&target_refs, &sample_refs);

    let mut matched_target_lines = 0usize;
    for op in diff.ops() {
        if op.tag() != DiffTag::Equal {
            continue;
        }
        for index in op.old_range().start..op.old_range().end {
            if target_lines[index].is_empty() {
                continue;
            }
            overlap_counts[index] += 1;
            matched_target_lines += 1;
        }
    }
    matched_target_lines
}

fn percent(value: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        value as f64 * 100.0 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_path_prefix_prefers_parent_directory() {
        let nested = Url::parse("https://example.com/docs/api/page.html").unwrap();
        let top_level = Url::parse("https://example.com/article").unwrap();
        let root = Url::parse("https://example.com/").unwrap();

        assert_eq!(parent_path_prefix(&nested), "/docs/api/");
        assert_eq!(parent_path_prefix(&top_level), "/");
        assert_eq!(parent_path_prefix(&root), "/");
    }

    #[test]
    fn classify_candidate_tier_prefers_same_prefix_before_same_origin() {
        let target = Url::parse("https://example.com/docs/api/page.html").unwrap();
        let prefix = parent_path_prefix(&target);

        let same_prefix = Url::parse("https://example.com/docs/api/other.html").unwrap();
        let same_origin = Url::parse("https://example.com/blog/post").unwrap();
        let external = Url::parse("https://other.example.com/docs/api/other.html").unwrap();

        assert_eq!(
            classify_candidate_tier(&target, &same_prefix, &prefix),
            CandidateTier::SamePrefix
        );
        assert_eq!(
            classify_candidate_tier(&target, &same_origin, &prefix),
            CandidateTier::SameOrigin
        );
        assert_eq!(
            classify_candidate_tier(&target, &external, &prefix),
            CandidateTier::AnyHttp
        );
    }

    #[test]
    fn accumulate_equal_lines_counts_overlap_per_target_line() {
        let target = vec![
            normalize_line("Header"),
            normalize_line("Shared body"),
            normalize_line("Footer"),
        ];
        let sample_one = vec![
            normalize_line("Header"),
            normalize_line("Other body"),
            normalize_line("Footer"),
        ];
        let sample_two = vec![
            normalize_line("Intro"),
            normalize_line("Shared body"),
            normalize_line("Footer"),
        ];
        let mut counts = vec![0usize; target.len()];

        let matched_one = accumulate_equal_lines(&target, &sample_one, &mut counts);
        let matched_two = accumulate_equal_lines(&target, &sample_two, &mut counts);

        assert_eq!(matched_one, 2);
        assert_eq!(matched_two, 2);
        assert_eq!(counts, vec![1, 1, 2]);
    }
}
