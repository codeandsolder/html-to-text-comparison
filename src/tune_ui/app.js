'use strict';

// The browser is a view/controller only: every tunable field is sourced from
// Rust's serialized, typed extractor configuration, not a duplicated schema.
const $ = (id) => document.getElementById(id);
const clone = (value) => structuredClone(value);
const app = {
  engines: [], engineByName: new Map(), pages: [], engine: '',
  configByEngine: new Map(), saved: new Map(),
  pageIndices: [0, 1], display: ['tuned', 'tuned'], current: [null, null],
  baselines: new Map(), running: null, timer: null,
  generation: 0, normalized: true, initComplete: false,
};

const OPTIONS = {
  'htmd.heading_style': ['atx', 'setex'],
  'htmd.hr_style': ['asterisks', 'dashes', 'underscores'],
  'htmd.br_style': ['two_spaces', 'backslash'],
  'htmd.link_style': ['inlined', 'inlined_prefer_autolinks', 'referenced'],
  'htmd.link_reference_style': ['full', 'collapsed', 'shortcut'],
  'htmd.code_block_style': ['fenced', 'indented'],
  'htmd.code_block_fence': ['backticks', 'tildes'],
  'htmd.bullet_list_marker': ['*', '-'],
  'htmd.translation_mode': ['pure', 'faithful'],
  'mdka.mode': ['balanced', 'minimal'],
  'dom_smoothie.text_mode': ['markdown', 'raw', 'formatted'],
  'dom_smoothie.candidate_select_mode': ['readability', 'dom_smoothie'],
  'turndown.heading_style': ['setext', 'atx'],
  'turndown.bullet_list_marker': ['*', '-', '+'],
  'turndown.code_block_style': ['indented', 'fenced'],
  'turndown.fence': [String.fromCharCode(96).repeat(3), '~~~'],
  'turndown.em_delimiter': ['_', '*'],
  'turndown.strong_delimiter': ['**', '__'],
  'turndown.link_style': ['inlined', 'referenced'],
  'turndown.link_reference_style': ['full', 'collapsed', 'shortcut'],
  'markdownify.heading_style': ['UNDERLINED', 'ATX', 'ATX_CLOSED', 'SETEXT'],
  'markdownify.strong_em_symbol': ['ASTERISK', 'UNDERSCORE'],
  'markdownify.newline_style': ['SPACES', 'BACKSLASH'],
  'markdownify.strip_document': ['STRIP', 'LSTRIP', 'RSTRIP', ''],
  'markdownify.strip_pre': ['STRIP', 'STRIP_ONE', ''],
  'markdownify.bs4_parser': ['', 'html.parser', 'lxml', 'html5lib'],
  'markdownify.bullets': ['*+-', '*', '-', '+'],
  'webclaw.format': ['markdown', 'text', 'llm', 'html', 'json'],
  'resiliparse.formatting_mode': ['formatted', 'plain', 'minimal_html'],
  'node-html-markdown.code_block_style': ['fenced', 'indented'],
  'node-html-markdown.bullet_marker': ['*', '-', '+'],
  'node-html-markdown.code_fence': [String.fromCharCode(96).repeat(3), '~~~'],
  'node-html-markdown.em_delimiter': ['_', '*'],
  'node-html-markdown.strong_delimiter': ['**', '__'],
  'node-html-markdown.strike_delimiter': ['~~', '~'],
};

const HINTS = {
  main_content: 'Extract primary article content rather than the entire page',
  only_main_content: 'Keep the article/main region when present',
  isolate_main: 'Prefer the semantic main content container',
  skip_tags: 'HTML tag names to omit, comma-separated',
  ignore: 'Element names to omit, comma-separated',
  ignore_links: 'Discard link destinations entirely',
  use_inline_links: 'Render bare URL labels as short autolinks',
  use_link_reference_definitions: 'Move link targets to numbered definitions below the text',
  no_link_wrapping: 'Prevent links from being line-wrapped',
  link_footnotes: 'Emit link destinations as separate footnotes',
  links: 'Retain link URLs in Resiliparse output',
  clean_urls: 'Normalize URL formatting and unnecessary parts',
  clean_fragments: 'Remove anchor fragments from URLs',
  clean_empty_links: 'Discard empty links',
  clean_redundant_links: 'Remove redundant links',
  preserve_spans: 'Keep span elements intact rather than flattening them',
  commonmark: 'Use CommonMark formatting rules',
  char_threshold: 'Minimum number of article characters used for candidate selection',
  nb_top_candidates: 'Number of article candidates to consider',
  n_top_candidates: 'Number of content candidates considered',
  max_elements_to_parse: 'Limit DOM nodes considered; blank means unlimited',
  max_elems_to_parse: 'Maximum DOM elements to consider; 0 means unlimited',
  link_density_modifier: 'Adjust the penalty for link-heavy article candidates',
  text_mode: 'Choose Markdown, formatted plain text, or raw text',
  formatting_mode: 'Plain text, formatted text, or minimal semantic HTML',
  alt_texts: 'Keep alternative descriptions for images',
  comments: 'Consider page comments as extractable content',
  post_meta: 'Keep article metadata such as author and date',
  noscript: 'Consider noscript fallback text',
  hidden_elements: 'Include elements with hidden class names',
  format: 'Choose the backend output representation',
  include_css: 'Limit extraction to CSS selectors',
  exclude_css: 'Exclude matching CSS selectors',
  include_selector: 'Only convert these selectors',
  exclude_selector: 'Omit these selectors',
  filter_include: 'CSS selectors included by Mdream, comma-separated',
  filter_exclude: 'CSS selectors excluded by Mdream, comma-separated',
  extraction_selectors: 'Semantic content selectors, comma-separated',
  frontmatter_meta_fields: 'Additional metadata fields for frontmatter',
  plugins: 'Go converter plugins, comma-separated; commonmark is always enabled',
  preserve_ids: 'Automatic uses the chosen Mdka mode default',
  drop_interactive_shell: 'Automatic uses the chosen Mdka mode default',
  emphasis_from_style: 'Automatic uses the chosen Mdka mode default',
  backslash_hard_breaks: 'Automatic uses the chosen Mdka mode default',
  origin: 'Override the base URL for resolving links',
};

async function fetchJson(url, options = {}) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  if (!response.ok) throw new Error((await response.text()) || ('HTTP ' + response.status));
  return response.json();
}
function status(message, error = false) {
  $('status').textContent = message;
  $('status').classList.toggle('error', error);
}
function label(name) {
  return name.replaceAll('_', ' ').replaceAll('-', ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}
function shortNumber(n) {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(n);
}
function formatBytes(n) {
  return n < 1000 ? n + ' B' : (n / 1000).toFixed(1) + ' kB';
}
function settingDefaults() {
  return app.engineByName.get(app.engine).defaults;
}
function currentSettings() {
  return app.configByEngine.get(app.engine);
}
function modifiedFields() {
  const defaults = settingDefaults();
  const values = currentSettings();
  return Object.keys(defaults).filter((field) => JSON.stringify(defaults[field]) !== JSON.stringify(values[field]));
}

function showEngineList() {
  const query = $('engine-search').value.trim().toLowerCase();
  const list = $('engine-list');
  list.replaceChildren();
  let count = 0;
  for (const engine of app.engines) {
    if (!engine.name.toLowerCase().includes(query)) continue;
    count++;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'engine-item' + (app.engine === engine.name ? ' active' : '');
    button.setAttribute('role', 'listitem');
    const name = document.createElement('span');
    name.className = 'name';
    name.textContent = engine.name;
    const timing = document.createElement('span');
    timing.className = 'time';
    timing.textContent = Object.keys(engine.defaults).length
      ? shortNumber(engine.benchmark.worker_p50_ms) + ' ms'
      : 'fixed';
    button.title = engine.name + ' • ' + shortNumber(engine.benchmark.worker_p50_ms) + ' ms';
    button.append(name, timing);
    button.addEventListener('click', () => switchEngine(engine.name));
    list.append(button);
  }
  $('engine-counter').textContent = String(count);
}

function switchEngine(name) {
  if (!app.engineByName.has(name)) return;
  app.engine = name;
  app.baselines.clear();
  app.current = [null, null];
  app.display = ['tuned', 'tuned'];
  $('active-engine').textContent = name;
  const benchmark = app.engineByName.get(name).benchmark;
  $('engine-latency').textContent = shortNumber(benchmark.worker_p50_ms) + ' ms';
  $('engine-success').textContent = benchmark.success + ' / ' + benchmark.runs;
  showEngineList();
  showFields();
  showModified();
  for (let side = 0; side < 2; side++) showSource(side);
  schedulePreview(0);
}

function showModified() {
  const count = modifiedFields().length;
  $('modified-counter').textContent = String(count);
  $('config-tag').textContent = count ? 'MODIFIED · ' + count : 'DEFAULT';
  $('config-tag').classList.toggle('modified', count > 0);
  $('saved-status').textContent = app.saved.has(app.engine)
    ? 'A saved configuration exists for this extractor.'
    : 'Changes update both pages automatically. Save to keep them for later.';
  for (const element of $('fields').querySelectorAll('.setting-row[data-field]')) {
    element.classList.toggle('modified', modifiedFields().includes(element.dataset.field));
  }
}

function createInput(field, value, original, onChange) {
  const key = app.engine + '.' + field;
  const choices = OPTIONS[key];
  if (original === null && ['preserve_ids', 'drop_interactive_shell', 'emphasis_from_style', 'backslash_hard_breaks'].includes(field)) {
    const select = document.createElement('select');
    for (const [v, text] of [['auto', 'Automatic (mode default)'], ['true', 'On'], ['false', 'Off']]) {
      const option = document.createElement('option');
      option.value = v;
      option.textContent = text;
      select.append(option);
    }
    select.value = 'auto';
    select.addEventListener('change', () => onChange(select.value === 'auto' ? null : select.value === 'true'));
    return select;
  }
  if (choices && (typeof value === 'string' || value === null)) {
    const select = document.createElement('select');
    for (const v of choices) {
      const option = document.createElement('option');
      option.value = v;
      option.textContent = v || '(none)';
      select.append(option);
    }
    select.value = value || '';
    select.addEventListener('change', () => onChange(select.value));
    return select;
  }
  if (typeof value === 'boolean') {
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = value;
    input.addEventListener('change', () => onChange(input.checked));
    return input;
  }
  if (Array.isArray(value)) {
    const area = document.createElement('textarea');
    area.rows = Math.min(3, Math.max(2, value.length));
    area.value = value.join(', ');
    area.placeholder = 'Comma-separated values';
    area.addEventListener('input', () => {
      onChange(area.value.split(/[\n,]/).map((v) => v.trim()).filter(Boolean), true);
    });
    return area;
  }
  if (typeof value === 'number' || value === null) {
    const input = document.createElement('input');
    input.type = 'number';
    input.step = Number.isInteger(value) || value === null ? '1' : 'any';
    input.min = field.includes('modifier') || field.includes('adjust') ? '-100' : '0';
    input.value = value === null ? '' : String(value);
    input.placeholder = 'Automatic';
    input.addEventListener('input', () => {
      if (!input.value.trim()) {
        if (original === null) onChange(null, true);
        return;
      }
      const parsed = Number(input.value);
      if (Number.isFinite(parsed) && (input.step === 'any' || Number.isInteger(parsed))) {
        onChange(parsed, true);
      }
    });
    return input;
  }
  const input = document.createElement('input');
  input.type = 'text';
  input.value = value;
  input.spellcheck = false;
  input.addEventListener('input', () => onChange(input.value, true));
  return input;
}

function settingPriority(field) {
  if (/main|extract|filter|candidate|threshold|skip|exclude|include|element|link_density|json_ld|comments|post_meta|hidden/.test(field)) return 0;
  if (/link|url|image|classes|data|origin/.test(field)) return 1;
  if (/heading|fence|code|delimiter|bold|bullet|indent|emphasis|strong|wrap|space|format|newline|rule|hr|br|table/.test(field)) return 2;
  return 3;
}

function showFields() {
  const fields = $('fields');
  fields.replaceChildren();
  const settings = currentSettings();
  const defaults = settingDefaults();
  const keys = Object.keys(defaults).sort((a, b) => settingPriority(a) - settingPriority(b) || a.localeCompare(b));
  $('settings-description').textContent = keys.length
    ? keys.length + ' backend parameters · changes preview automatically'
    : 'This engine has no exposed formatting or extraction settings.';
  if (!keys.length) {
    const text = document.createElement('p');
    text.className = 'no-settings';
    text.textContent = 'No tunable parameters are supported by this backend. Its output is still available for both pages and in the default comparison.';
    fields.append(text);
    return;
  }
  let lastGroup = -1;
  for (const field of keys) {
    const group = settingPriority(field);
    if (group !== lastGroup) {
      const heading = document.createElement('div');
      heading.className = 'setting-group-label';
      heading.textContent = ['CONTENT & SELECTION', 'LINKS & METADATA', 'MARKDOWN & FORMATTING', 'OTHER OPTIONS'][group];
      fields.append(heading);
      lastGroup = group;
    }
    const value = settings[field];
    const row = document.createElement('div');
    row.className = 'setting-row';
    row.dataset.field = field;
    if (typeof value === 'boolean') row.classList.add('checkbox-row');
    const fieldLabel = document.createElement('label');
    fieldLabel.textContent = label(field);
    const control = createInput(field, value, defaults[field], (updated, isText) => {
      settings[field] = updated;
      showModified();
      schedulePreview(isText ? 400 : 170);
    });
    control.id = 'field-' + field;
    fieldLabel.htmlFor = control.id;
    row.append(fieldLabel, control);
    if (HINTS[field]) {
      const hint = document.createElement('div');
      hint.className = 'setting-description';
      hint.textContent = HINTS[field];
      row.append(hint);
    }
    fields.append(row);
  }
}

function showSource(side) {
  const prefix = side === 0 ? 'left' : 'right';
  const entry = app.pages[app.pageIndices[side]];
  $(prefix + '-source').value = entry.id;
  $(prefix + '-title').textContent = entry.title;
  $(prefix + '-rank').textContent = 'HN #' + entry.rank;
  $(prefix + '-size').textContent = formatBytes(entry.bytes) + ' HTML';
  $(prefix + '-link').href = entry.url;
  $(prefix + '-html').href = '/api/source/' + encodeURIComponent(entry.id);
  if (app.current[side]?.id !== entry.id) {
    $(prefix + '-output').textContent = 'Loading extraction…';
    $(prefix + '-runtime').textContent = '…';
    $(prefix + '-output-size').textContent = '—';
    $(prefix + '-status').textContent = 'LOADING';
  }
}

function selectPage(side, index) {
  let newIndex = (index + app.pages.length) % app.pages.length;
  if (newIndex === app.pageIndices[1 - side]) {
    newIndex = (newIndex + 1) % app.pages.length;
  }
  app.pageIndices[side] = newIndex;
  app.display[side] = 'tuned';
  showSource(side);
  showTabs(side);
  schedulePreview(0);
}

function showTabs(side) {
  const prefix = side === 0 ? 'left' : 'right';
  $(prefix + '-tuned').classList.toggle('active', app.display[side] === 'tuned');
  $(prefix + '-baseline').classList.toggle('active', app.display[side] === 'baseline');
}

function normalizedOutput(raw) {
  if (!app.normalized) return raw;
  return raw
    .replace(/^ {0,3}\[[a-zA-Z0-9_-]+\]:\s+https?:\/\/\S+.*$/gm, '')
    .replace(/!\[([^\]]*)\]\((?:<[^>]*>|[^)]*)\)/g, '[image: $1]')
    .replace(/\[([^\]]+)\]\((?:<[^>]*>|[^)]*)\)/g, '$1')
    .replace(/\[([^\]]+)\]\[[^\]]*\]/g, '$1');
}

async function baselineFor(side) {
  const page = app.pages[app.pageIndices[side]].id;
  const key = app.engine + ':' + page;
  if (app.baselines.has(key)) return app.baselines.get(key);
  const value = await fetchJson('/api/baseline/' + encodeURIComponent(app.engine) + '/' + encodeURIComponent(page));
  app.baselines.set(key, value);
  return value;
}

async function showSide(side) {
  const prefix = side === 0 ? 'left' : 'right';
  let result = app.current[side];
  if (app.display[side] === 'baseline') {
    const generation = app.generation;
    try {
      result = await baselineFor(side);
    } catch (error) {
      status('Could not open original defaults: ' + error.message, true);
      return;
    }
    if (generation !== app.generation) return;
  }
  if (!result || result.id !== app.pages[app.pageIndices[side]].id) return;
  $(prefix + '-output').textContent = normalizedOutput(result.output || '(empty result)');
  $(prefix + '-runtime').textContent = result.from_baseline
    ? 'DEFAULT'
    : shortNumber(result.worker_ms) + ' ms' + (result.from_cache ? ' · cached' : '');
  $(prefix + '-status').textContent = result.status.toUpperCase() + (result.clipped ? ' · PREVIEW CLIPPED' : '');
  $(prefix + '-status').classList.toggle('failed', result.status !== 'success');
  $(prefix + '-output-size').textContent = formatBytes(result.bytes);
}

function schedulePreview(delay = 280, force = false) {
  if (!app.initComplete) return;
  if (app.timer) clearTimeout(app.timer);
  const generation = ++app.generation;
  if (app.running) app.running.abort();
  app.timer = setTimeout(() => { void runPreview(generation, force); }, delay);
  status('Updating both extractions…');
}

async function runPreview(generation, force) {
  const controller = new AbortController();
  app.running = controller;
  const pages = app.pageIndices.map((index) => app.pages[index].id);
  const engine = app.engine;
  const settings = clone(currentSettings());
  try {
    const start = performance.now();
    const answer = await fetchJson('/api/extract', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ engine, pages, settings, force }),
      signal: controller.signal,
    });
    if (generation !== app.generation) return;
    app.current = answer.results;
    for (let side = 0; side < 2; side++) await showSide(side);
    if (generation !== app.generation) return;
    const failures = answer.results.filter((result) => result.status !== 'success').length;
    status(failures
      ? failures + ' extraction(s) reported a failure; see the panel status.'
      : 'Updated both pages in ' + Math.round(performance.now() - start) + ' ms · source snapshots unchanged.',
      failures > 0);
    $('network-status').textContent = 'CONNECTED';
  } catch (error) {
    if (generation !== app.generation || error.name === 'AbortError') return;
    status(error.message, true);
    $('network-status').textContent = 'PREVIEW ERROR';
  } finally {
    if (generation === app.generation) app.running = null;
  }
}

async function showBaseline(side) {
  app.display[side] = 'baseline';
  showTabs(side);
  await showSide(side);
}
function showTuned(side) {
  app.display[side] = 'tuned';
  showTabs(side);
  void showSide(side);
}

async function saveProfile() {
  const button = $('save-profile');
  button.disabled = true;
  try {
    const engine = app.engine;
    const settings = clone(currentSettings());
    await fetchJson('/api/profile', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ engine, settings }),
    });
    app.saved.set(engine, settings);
    $('saved-status').textContent = 'Saved to the server. This profile will load next time.';
  } catch (error) {
    $('saved-status').textContent = 'Save failed: ' + error.message;
  } finally {
    button.disabled = false;
  }
}

function replaceSettings(value) {
  app.configByEngine.set(app.engine, clone(value));
  showFields();
  showModified();
  schedulePreview(0);
}

async function copyConfig() {
  await navigator.clipboard.writeText(JSON.stringify({
    extractor: app.engine, settings: currentSettings(),
  }, null, 2));
  $('saved-status').textContent = 'Configuration JSON copied.';
}

for (const side of [0, 1]) {
  const prefix = side === 0 ? 'left' : 'right';
  $(prefix + '-source').addEventListener('change', (event) => {
    selectPage(side, app.pages.findIndex((page) => page.id === event.target.value));
  });
  $(prefix + '-prev').addEventListener('click', () => selectPage(side, app.pageIndices[side] - 1));
  $(prefix + '-next').addEventListener('click', () => selectPage(side, app.pageIndices[side] + 1));
  $(prefix + '-baseline').addEventListener('click', () => { void showBaseline(side); });
  $(prefix + '-tuned').addEventListener('click', () => showTuned(side));
  $(prefix + '-copy').addEventListener('click', async () => {
    await navigator.clipboard.writeText($(prefix + '-output').textContent);
    status('Copied visible ' + (side === 0 ? 'left' : 'right') + ' text.');
  });
}
$('engine-search').addEventListener('input', showEngineList);
$('refresh').addEventListener('click', () => schedulePreview(0, true));
$('clean-links').addEventListener('change', () => {
  app.normalized = $('clean-links').checked;
  void showSide(0);
  void showSide(1);
});
$('reset-defaults').addEventListener('click', () => replaceSettings(settingDefaults()));
$('restore-saved').addEventListener('click', () => {
  if (app.saved.has(app.engine)) replaceSettings(app.saved.get(app.engine));
  else $('saved-status').textContent = 'No saved profile for this extractor.';
});
$('save-profile').addEventListener('click', saveProfile);
$('copy-config').addEventListener('click', () => {
  void copyConfig().catch((error) => status(error.message, true));
});
document.addEventListener('keydown', (event) => {
  if (event.ctrlKey || event.altKey || event.metaKey) return;
  if (['INPUT', 'SELECT', 'TEXTAREA'].includes(document.activeElement?.tagName)) return;
  if (event.key === 'a' || event.key === 'A') selectPage(0, app.pageIndices[0] - 1);
  if (event.key === 'd' || event.key === 'D') selectPage(0, app.pageIndices[0] + 1);
  if (event.key === 'j' || event.key === 'J') selectPage(1, app.pageIndices[1] - 1);
  if (event.key === 'l' || event.key === 'L') selectPage(1, app.pageIndices[1] + 1);
});

async function initialize() {
  try {
    const data = await fetchJson('/api/state');
    app.pages = data.pages;
    app.engines = data.engines.sort((a, b) =>
      a.benchmark.worker_p50_ms - b.benchmark.worker_p50_ms || a.name.localeCompare(b.name));
    for (const engine of app.engines) {
      app.engineByName.set(engine.name, engine);
      const saved = data.profiles[engine.name];
      app.configByEngine.set(engine.name, clone(saved || engine.defaults));
      if (saved) app.saved.set(engine.name, clone(saved));
    }
    for (const prefix of ['left', 'right']) {
      const select = $(prefix + '-source');
      for (const page of app.pages) {
        const option = document.createElement('option');
        option.value = page.id;
        option.textContent = '#' + page.rank + ' · ' + page.title;
        select.append(option);
      }
    }
    app.initComplete = true;
    $('network-status').textContent = 'CONNECTED';
    $('footer-meta').textContent = app.pages.length + ' SOURCES / ' + app.engines.length + ' QUALIFIED ENGINES';
    switchEngine(app.engineByName.has('dom_smoothie') ? 'dom_smoothie' : app.engines[0].name);
  } catch (error) {
    $('network-status').textContent = 'UNAVAILABLE';
    status('Initialization failed: ' + error.message, true);
  }
}
void initialize();
