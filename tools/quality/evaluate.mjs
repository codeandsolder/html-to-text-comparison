import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';

const require = createRequire(new URL('../extractors/package.json', import.meta.url));
const { parseHTML } = require('linkedom');

export function tokens(value) {
  return (value.normalize('NFKC').toLocaleLowerCase('en-US').match(/[\p{L}\p{N}]+/gu) ?? []);
}

export function shingles(value, n = 4) {
  const terms = tokens(value);
  const out = new Set();
  for (let i = 0; i + n <= terms.length; i++) {
    out.add(terms.slice(i, i + n).join(' '));
  }
  return out;
}

function overlap(left, right) {
  if (!left.size) return null;
  let matched = 0;
  for (const item of left) if (right.has(item)) matched++;
  return matched / left.size;
}

function without(root, selector) {
  for (const element of root.querySelectorAll(selector)) element.remove();
}

function textOf(element) {
  return (element?.textContent ?? '').replace(/\s+/gu, ' ').trim();
}

const LEAF_SELECTORS = 'p,li,blockquote,pre,code,h1,h2,h3,h4,h5,h6,td,th';
const NOISE_SELECTORS = 'script,style,noscript,template,svg,canvas,iframe';
const NAV_SELECTORS = 'nav,aside,footer,header,[role=navigation],[role=complementary]';

function anchorsWithin(node) {
  const blocks = [...node.querySelectorAll(LEAF_SELECTORS)];
  const fragments = [];
  const seen = new Set();
  for (const block of blocks) {
    if (block.closest(NAV_SELECTORS)) continue;
    // Ignore a parent block if child elements supply the same text.
    if (block.querySelector(LEAF_SELECTORS) !== null) continue;
    const text = textOf(block);
    if (tokens(text).length < 6 || seen.has(text)) continue;
    seen.add(text);
    fragments.push(text);
  }
  return fragments;
}

export function referenceFromHtml(html) {
  const { document } = parseHTML(html);
  without(document, NOISE_SELECTORS);
  const rawBody = textOf(document.body ?? document.documentElement);
  const blocked = (html.length < 20000 &&
    (/geo\.captcha-delivery\.com|cf-chl-|challenge-platform|verify you are human/iu.test(html))) ||
    /<title[^>]*>\s*(?:just a moment|access denied|attention required)/iu.test(html);
  const candidates = [...document.querySelectorAll('main,article,[role=main]')];
  const areas = candidates.map(node => ({
    node, fragments: anchorsWithin(node),
    grams: shingles(anchorsWithin(node).join(' '))
  })).sort((a, b) => b.grams.size - a.grams.size);
  const main = areas[0];
  const mainGrams = main?.grams ?? new Set();
  const eligible = !blocked && mainGrams.size >= 20 && (main?.fragments.length ?? 0) >= 2;
  const navigation = new Set();
  for (const element of document.querySelectorAll(NAV_SELECTORS)) {
    if (main?.node?.contains(element)) continue;
    for (const gram of shingles(textOf(element))) if (!mainGrams.has(gram)) navigation.add(gram);
  }
  const structure = {};
  for (const [name, selector, threshold] of [
    ['headings', 'h1,h2,h3,h4,h5,h6', 2],
    ['code', 'pre,code', 4],
    ['table_cells', 'td,th', 2]
  ]) {
    const values = [];
    const found = new Set();
    for (const node of main?.node?.querySelectorAll(selector) ?? []) {
      if (node.closest(NAV_SELECTORS) || node.querySelector(selector)) continue;
      const content = textOf(node);
      if (tokens(content).length < threshold || found.has(content)) continue;
      found.add(content);
      values.push(content);
    }
    structure[name] = values;
  }
  const contentHints = {
    heading_elements: main?.node?.querySelectorAll('h1,h2,h3,h4,h5,h6').length ?? 0,
    code_elements: main?.node?.querySelectorAll('pre,code').length ?? 0,
    table_elements: main?.node?.querySelectorAll('table').length ?? 0,
    heading_reference_items: structure.headings.length,
    code_reference_items: structure.code.length,
    table_cell_reference_items: structure.table_cells.length
  };
  // Large heading-heavy hubs (videos, marketplaces, product grids) may
  // provide a semantic main that is not a single prose article. Keep their
  // metrics, but exclude them from the narrower prose-like sensitivity slice.
  const mainAnchorWords = tokens((main?.fragments ?? []).join(' ')).length;
  const headingDense = contentHints.heading_elements >= 20 &&
    contentHints.heading_elements * 20 > mainAnchorWords;
  return {
    eligible,
    proseCandidate: eligible && !headingDense,
    headingDense,
    blocked,
    mainGrams,
    navGrams: navigation,
    mainFragments: main?.fragments ?? [],
    body_words: tokens(rawBody).length,
    main_anchor_words: mainAnchorWords,
    semantic_container: main?.node?.tagName?.toLowerCase() ?? null,
    contentHints,
    structure
  };
}

function retention(items, output, minimumTokens) {
  if (!items.length) return null;
  const tokensOutput = shingles(output, minimumTokens);
  const hits = items.filter(item => {
    const reference = shingles(item, minimumTokens);
    return reference.size && overlap(reference, tokensOutput) >= 0.6;
  }).length;
  return hits / items.length;
}

export function scoreOutput(reference, output, status = 'success') {
  if (status !== 'success' && status !== 'success_inferred') return {
    main_recall: reference.eligible ? 0 : null,
    nav_leakage: null, nav_density: null,
    heading_retention: null, code_retention: null, table_cell_retention: null
  };
  const outputGrams = shingles(output);
  let navHits = 0;
  for (const gram of reference.navGrams) if (outputGrams.has(gram)) navHits++;
  return {
    main_recall: reference.eligible ? overlap(reference.mainGrams, outputGrams) : null,
    nav_leakage: reference.navGrams.size ? navHits / reference.navGrams.size : null,
    nav_density: outputGrams.size ? navHits / outputGrams.size : null,
    output_distinct_4grams: outputGrams.size,
    heading_retention: retention(reference.structure.headings, output, 2),
    code_retention: retention(reference.structure.code, output, 3),
    table_cell_retention: retention(reference.structure.table_cells, output, 2)
  };
}

function median(values) {
  const a = [...values].filter(v => v != null).sort((x, y) => x - y);
  if (!a.length) return null;
  const m = Math.floor(a.length / 2);
  return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2;
}

function percentile(values, percentileIndex) {
  const a = [...values].filter(v => v != null).sort((x, y) => x - y);
  if (!a.length) return null;
  return a[Math.floor((a.length - 1) * percentileIndex)];
}

export function evaluate({ dataset, runs }) {
  const manifest = JSON.parse(readFileSync(join(dataset, 'manifest.json')));
  const byCapture = new Map();
  for (const entry of manifest.records) {
    const body = readFileSync(join(dataset, entry.file));
    const hash = createHash('sha256').update(body).digest('hex');
    if (hash !== entry.sha256) throw new Error('corrupt sample ' + entry.capture_id);
    byCapture.set(entry.capture_id, {
      ...entry, reference: referenceFromHtml(body.toString('utf8'))
    });
  }
  const records = [];
  for (const run of runs) {
    const report = JSON.parse(readFileSync(join(dataset, 'runs', run, 'report.json')));
    for (const item of report.results) {
      const entry = byCapture.get(item.capture_id);
      if (!entry) throw new Error('unknown capture ' + item.capture_id);
      const file = join(dataset, 'runs', run, 'outputs', item.extractor, item.capture_id + '.txt');
      const output = item.output_sha256 ? readFileSync(file) : Buffer.from('');
      const hash = output.length ? createHash('sha256').update(output).digest('hex') : null;
      if (hash !== item.output_sha256 || output.length !== item.output_bytes) {
        throw new Error('corrupt extractor output ' + item.capture_id + ' ' + item.extractor);
      }
      records.push({
        run, capture_id: item.capture_id, cohort: item.cohort,
        extractor: item.extractor, status: item.status,
        elapsed_seconds: item.elapsed_seconds, html_bytes: item.html_bytes,
        output_bytes: item.output_bytes,
        eligible: entry.reference.eligible,
        prose_candidate: entry.reference.proseCandidate,
        heading_dense: entry.reference.headingDense,
        blocked: entry.reference.blocked,
        main_anchor_words: entry.reference.main_anchor_words,
        ...entry.reference.contentHints,
        ...scoreOutput(entry.reference, output.toString('utf8'), item.status)
      });
    }
  }
  const engineKeys = [...new Set(records.map(x => x.extractor))].sort();
  const engineSummaries = {};
  for (const engine of engineKeys) {
    const engineRows = records.filter(x => x.extractor === engine);
    const eligible = engineRows.filter(x => x.eligible);
    const scored = eligible.filter(x => x.main_recall !== null);
    const prose = scored.filter(x => x.prose_candidate);
    engineSummaries[engine] = {
      samples: engineRows.length,
      semantic_reference_pages: eligible.length,
      scored_pages: scored.length,
      successful: engineRows.filter(x => x.status === 'success' || x.status === 'success_inferred').length,
      main_recall_median: median(scored.map(x => x.main_recall)),
      prose_candidate_pages: prose.length,
      prose_main_recall_median: median(prose.map(x => x.main_recall)),
      prose_main_recall_p10: percentile(prose.map(x => x.main_recall), 0.1),
      main_recall_p10: percentile(scored.map(x => x.main_recall), 0.1),
      main_recall_mean: scored.length ? scored.reduce((sum, row) => sum + row.main_recall, 0) / scored.length : null,
      nav_density_median: median(scored.map(x => x.nav_density)),
      nav_leakage_median: median(scored.map(x => x.nav_leakage)),
      inferred_status_rows: engineRows.filter(x => x.status.endsWith("_inferred")).length,
      heading_retention_median: median(scored.map(x => x.heading_retention)),
      table_cell_retention_median: median(scored.map(x => x.table_cell_retention)),
      code_retention_median: median(scored.map(x => x.code_retention)),
      eligible_successes: eligible.filter(x => x.status === 'success' || x.status === 'success_inferred').length,
      elapsed_seconds_median: median(engineRows.map(x => x.elapsed_seconds)),
      low_recall_cases: [...scored].sort((a, b) => a.main_recall - b.main_recall).slice(0, 10)
        .map(x => ({ capture_id: x.capture_id, cohort: x.cohort, main_recall: x.main_recall }))
    };
  }
  const pageSummary = [...byCapture.values()].map(entry => ({
    capture_id: entry.capture_id, cohort: entry.query_id, url: entry.url, bytes: entry.bytes,
    semantic_container: entry.reference.semantic_container,
    eligible: entry.reference.eligible,
    prose_candidate: entry.reference.proseCandidate,
    heading_dense: entry.reference.headingDense,
    blocked: entry.reference.blocked,
    main_anchor_words: entry.reference.main_anchor_words,
    main_4grams: entry.reference.mainGrams.size,
    nav_4grams: entry.reference.navGrams.size,
    ...entry.reference.contentHints
  }));
  return {
    method: 'DOM semantic main/article anchored 4-token shingles versus extracted output; navigation out-of-main leakage; no ground-truth precision claim',
    total_pages: pageSummary.length,
    eligible_pages: pageSummary.filter(x => x.eligible).length,
    prose_candidate_pages: pageSummary.filter(x => x.prose_candidate).length,
    heading_dense_pages: pageSummary.filter(x => x.eligible && x.heading_dense).length,
    suspected_blocked_pages: pageSummary.filter(x => x.blocked).length,
    engines: engineSummaries,
    pages: pageSummary,
    results: records
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const argumentsList = process.argv.slice(2);
  const datasetArg = argumentsList.indexOf('--dataset');
  const outArg = argumentsList.indexOf('--out');
  const runArgs = [];
  for (let i = 0; i < argumentsList.length; i++) {
    if (argumentsList[i] === '--run') runArgs.push(argumentsList[i + 1]);
  }
  if (datasetArg < 0 || outArg < 0 || !runArgs.length) {
    throw new Error('Usage: node tools/quality/evaluate.mjs --dataset DIR --run RUN_ID [--run RUN_ID] --out PATH');
  }
  const result = evaluate({
    dataset: resolve(argumentsList[datasetArg + 1]), runs: runArgs
  });
  writeFileSync(resolve(argumentsList[outArg + 1]), JSON.stringify(result, null, 2) + '\n');
  process.stdout.write(JSON.stringify({
    total_pages: result.total_pages, eligible_pages: result.eligible_pages,
    prose_candidate_pages: result.prose_candidate_pages,
    heading_dense_pages: result.heading_dense_pages,
    suspected_blocked_pages: result.suspected_blocked_pages, engines: result.engines
  }, null, 2) + '\n');
}
