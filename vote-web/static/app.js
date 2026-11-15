'use strict';

const byId = (id) => document.getElementById(id);
const app = { pair: null, busy: false, lastVote: null, activeTab: 'vote' };

async function request(url, options) {
  const response = await fetch(url, { cache: 'no-store', ...options });
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(detail || 'Request failed: ' + response.status);
  }
  return response.json();
}

function feedback(message, isError = false) {
  const element = byId('feedback');
  element.textContent = message;
  element.classList.toggle('error', isError);
}

function setBusy(value) {
  app.busy = value;
  for (const name of ['vote-left', 'vote-right', 'vote-equal', 'skip']) {
    byId(name).disabled = value || !app.pair;
  }
  byId('undo').disabled = value || !app.lastVote;
}

function formatChars(chars) {
  if (chars < 1000) return chars + ' chars';
  return (chars / 1000).toFixed(1) + 'k chars';
}

function paintSide(which, extraction) {
  byId('content-' + which).textContent = extraction.text || '(empty extraction)';
  byId('name-' + which).textContent = extraction.engine;
  byId('name-' + which).hidden = !byId('reveal').checked;
  byId('status-' + which).textContent = extraction.status.replaceAll('_', ' ');
  byId('status-' + which).classList.toggle('failed', !extraction.status.startsWith('success'));
  byId('size-' + which).textContent = formatChars(extraction.chars);
  byId('full-' + which).href = extraction.full_url;
  byId('truncated-' + which).hidden = !extraction.clipped;
  byId('scroll-' + which).scrollTop = 0;
}

function showPair(pair) {
  app.pair = pair;
  byId('source-link').textContent = pair.url;
  byId('source-link').href = pair.url;
  byId('cohort').textContent = pair.cohort.replaceAll('-', ' ');
  byId('page-number').textContent = 'PAGE ' + String(pair.page_index).padStart(2, '0') + ' / ' + pair.page_count;
  byId('total-votes').textContent = pair.voted_total.toLocaleString();
  paintSide('left', pair.left);
  paintSide('right', pair.right);
  setBusy(false);
}

async function nextPair() {
  setBusy(true);
  try {
    const result = await request('/api/next');
    showPair(result);
    feedback('Ready. Choose the better extraction.');
  } catch (error) {
    setBusy(false);
    feedback(error.message, true);
  }
}

async function vote(choice) {
  if (app.busy || !app.pair || app.activeTab !== 'vote') return;
  setBusy(true);
  const current = app.pair;
  try {
    const result = await request('/api/vote', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ballot_id: current.id, choice })
    });
    app.lastVote = current;
    byId('total-votes').textContent = result.total.toLocaleString();
    await nextPair();
  } catch (error) {
    setBusy(false);
    feedback(error.message, true);
  }
}

async function undo() {
  if (app.busy || !app.lastVote) return;
  setBusy(true);
  try {
    const previous = app.lastVote;
    const result = await request('/api/undo', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ballot_id: previous.id })
    });
    app.lastVote = null;
    previous.voted_total = result.total;
    showPair(previous);
    feedback('Vote undone. You can choose again.');
  } catch (error) {
    setBusy(false);
    feedback(error.message, true);
  }
}

function cell(text, className) {
  const td = document.createElement('td');
  td.textContent = String(text);
  if (className) td.className = className;
  return td;
}

async function showRankings() {
  app.activeTab = 'ranking';
  byId('tab-ranking').classList.add('active');
  byId('tab-vote').classList.remove('active');
  byId('tab-vote').removeAttribute('aria-current');
  byId('tab-ranking').setAttribute('aria-current', 'page');
  byId('compare-view').hidden = true;
  byId('ranking-view').hidden = false;
  try {
    const data = await request('/api/rankings');
    byId('rank-vote-count').textContent = data.total_votes.toLocaleString() + ' votes';
    byId('rank-pages-count').textContent = data.pages_covered + ' of 100 pages compared';
    const table = byId('ranking-rows');
    table.replaceChildren();
    data.results.forEach((item, index) => {
      const tr = document.createElement('tr');
      tr.append(
        cell(String(index + 1).padStart(2, '0'), 'rank-number'),
        cell(item.engine, 'rank-engine'),
        cell(Math.round(item.elo), 'rank-elo'),
        cell(item.comparisons),
        cell(item.wins),
        cell(item.ties),
        cell(item.losses)
      );
      table.append(tr);
    });
  } catch (error) {
    byId('rank-vote-count').textContent = 'Could not load rankings: ' + error.message;
  }
}

function showVoting() {
  app.activeTab = 'vote';
  byId('tab-vote').classList.add('active');
  byId('tab-ranking').classList.remove('active');
  byId('tab-ranking').removeAttribute('aria-current');
  byId('tab-vote').setAttribute('aria-current', 'page');
  byId('ranking-view').hidden = true;
  byId('compare-view').hidden = false;
}

for (const choice of ['left', 'equal', 'right']) {
  byId('vote-' + choice).addEventListener('click', () => vote(choice));
}
byId('skip').addEventListener('click', nextPair);
byId('undo').addEventListener('click', undo);
byId('tab-vote').addEventListener('click', showVoting);
byId('back-to-vote').addEventListener('click', showVoting);
byId('tab-ranking').addEventListener('click', showRankings);
byId('reveal').addEventListener('change', () => {
  if (app.pair) {
    byId('name-left').hidden = !byId('reveal').checked;
    byId('name-right').hidden = !byId('reveal').checked;
  }
});

window.addEventListener('keydown', (event) => {
  if (event.altKey || event.ctrlKey || event.metaKey || event.repeat) return;
  if (['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement?.tagName)) return;
  if (event.key === 'z' || event.key === 'Z') {
    event.preventDefault();
    undo();
    return;
  }
  if (app.activeTab !== 'vote') return;
  const choices = { ArrowLeft: 'left', ArrowDown: 'equal', ArrowRight: 'right' };
  if (Object.hasOwn(choices, event.key)) {
    event.preventDefault();
    vote(choices[event.key]);
  } else if (event.key === 's' || event.key === 'S') {
    event.preventDefault();
    nextPair();
  }
});

nextPair();
