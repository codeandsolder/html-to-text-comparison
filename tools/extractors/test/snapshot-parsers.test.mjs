import test from 'node:test';
import assert from 'node:assert/strict';
import { parseSnapshot } from '../snapshot-parsers.mjs';

const articleBody = 'The primary content of the article is present and readable. '.repeat(25);
const fixture = `<html><head><title>Fixture</title></head><body><nav>Navigation should not be copied</nav><article><h1>Fixture article</h1><p>${articleBody}</p></article></body></html>`;

test('Defuddle extracts only local snapshot in non-fetching mode', async () => {
    const text = await parseSnapshot('defuddle', fixture, 'https://example.com/article');
    assert.match(text, /primary content of the article/);
    assert.doesNotMatch(text, /Navigation should not be copied/);
});

test('node-html-markdown converts the supplied full document', async () => {
    const text = await parseSnapshot('node-html-markdown', '<h1>Header</h1><p>A <strong>strong</strong> line</p>', 'https://example.com');
    assert.match(text, /# Header/);
    assert.match(text, /\*\*strong\*\*/);
});

test('node-html-markdown preserves defaults when only selected options change', async () => {
    const text = await parseSnapshot('node-html-markdown', '<h1>Header</h1><p>A <strong>strong</strong> line</p>', 'https://example.com', {
        strong_delimiter: '__',
        use_inline_links: false,
        max_consecutive_newlines: 0,
    });
    assert.match(text, /# Header/);
    assert.match(text, /__strong__/);
    assert.doesNotMatch(text, /undefined/);
});

test('webforai reads the same snapshot and preserves technical tables and code', async () => {
    const { readFile } = await import('node:fs/promises');
    const raw = await readFile(new URL('../../../fixtures/technical-reference.html', import.meta.url), 'utf8');
    const text = await parseSnapshot('webforai', raw, 'https://example.com/docs/registers');
    assert.match(text, /\\| CTRL_A \\| 0x18 \\| 7:0 \\|/);
    assert.match(text, /0x00ff_0000/);
    assert.ok(text.includes(String.fromCharCode(96).repeat(3)), 'fenced code block missing');
    assert.doesNotMatch(text, /IRRELEVANT_DOCS_NAVIGATION/);
});

test('webforai ignores navigation but retains all article paragraphs', async () => {
    const { readFile } = await import('node:fs/promises');
    const raw = await readFile(new URL('../../../fixtures/article-with-navigation.html', import.meta.url), 'utf8');
    const text = await parseSnapshot('webforai', raw, 'https://example.com/article');
    for (const marker of ['CONTENT_SENTINEL_ALPHA', 'CONTENT_SENTINEL_BETA', 'CONTENT_SENTINEL_GAMMA']) {
        assert.ok(text.includes(marker), 'missing ' + marker);
    }
    assert.doesNotMatch(text, /UNRELATED_NAVIGATION_HELP_CENTER/);
    assert.doesNotMatch(text, /UNRELATED_SIDEBAR_SPONSORED_LINKS/);
});

test('unknown parser fails explicitly', async () => {
    await assert.rejects(parseSnapshot('missing', fixture, 'https://example.com'), /Unknown snapshot parser/);
});
