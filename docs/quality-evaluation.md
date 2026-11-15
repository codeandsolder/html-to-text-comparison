# Offline HTML extraction quality evaluation

## Corpus contract

Keep captured HTML outside public Git. Source inputs are immutable bytes with
a content SHA-256, captured original URL, capture ID, query cohort, and exact
capture provenance. Each extractor must see the same input bytes and the real
source page URL as its link base, without a second HTTP navigation or fetch.

The first private corpus is the reproducible 100-page js-redox sample documented
in the project's Notion page. It uses original main-frame browser DOM snapshots,
not origin response bytes. It includes blocked pages, authentication screens,
listing/forum pages, and articles. Those populations must not be mixed into a
single claim of article-extraction quality.

## Separate questions to measure

- Execution: per-extractor success, typed failure, no-content, timeout and
  explicit diagnostic. Failed runs must not disappear from aggregate recall.
- Relevant content: textual recall of independently annotated main-content
  passages. A parser with a short result is not automatically better.
- Boilerplate: navigation/header/footer/aside leakage, reported independently
  rather than treated as missing main content.
- Structure: headings, tables and code text retention; eventual annotation must
  distinguish preserved markdown/plaintext syntax as separate output contracts.
- Performance: repeated warm and cold elapsed time, peak resident memory and
  comparable configurations. A single debug-build smoke is not a benchmark.

## Automated DOM *proxy* (not human truth)

Run the Node script at tools/quality/evaluate.mjs. It requires the repository's
existing pinned linkedom package installed via npm ci in tools/extractors.

The script reads a private sample manifest plus prior offline extraction runs.
It verifies each source and output SHA-256. A reference is *eligible* only when
a captured page has sufficient text in a semantic main/article/role=main block.
The largest independently structured candidate supplies paragraph/list/heading/
code/table text. Distinct 4-word contiguous shingles from that DOM-derived
content are matched against each extractor's output. A failure on an eligible
reference counts as zero recall. Boilerplate phrases come from DOM navigation,
header, aside and footer elements outside the chosen main block, with overlaps
against main content removed. Heading/code/table retention reports text presence,
not Markdown formatting preservation. Suspected anti-bot/challenge pages are
explicitly excluded from article recall but remain in execution statistics.

This is **not ground truth**. The proxy is biased by semantic HTML, implicit DOM
ordering, word normalization, duplicate text and source-page role labeling.
A secondary prose-candidate slice excludes semantic main areas with at least
20 headings and more than five headings per hundred anchor words. That
heading-density rule is only a shape-based sensitivity analysis, not a page-type
classifier. It avoids treating video recommendations and dense product grids as
the reference for one prose article, but can also exclude genuine long documents.
Do not combine main recall and navigation density into a single ranking. Compare
pairwise cases and manually annotate a diverse sample (article, listing, login,
forum, anti-bot and technical documentation) before calling a quality leaderboard
definitive. Pages without reliable semantic-main elements are left unscored.

Example workflow: first collect outputs with the private sample's run_smoke.py
using its --source-repo/--binary/--scratch/--run-id arguments and the real
captured URL via --base-url; then run the evaluator with --dataset SAMPLE_DIR,
one or more --run RUN_ID options, and --out REPORT_PATH. Put report files under
the sample's runs/ directory. Never write derived artifacts into the original
js-redox corpus or a repository source tree.

## Known limitation

The CLI with --base-url accepts the exact original HTTP(S) URL only when input
is a local HTML file, retaining the captured link base without fetching that URL.
This is essential for comparing offline Markdown engines that resolve relative
links. It does not execute browser JavaScript, so captured DOM and origin network
HTML can differ considerably.
