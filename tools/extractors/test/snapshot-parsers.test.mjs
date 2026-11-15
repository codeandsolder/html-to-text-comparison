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

test('unknown parser fails explicitly', async () => {
    await assert.rejects(parseSnapshot('missing', fixture, 'https://example.com'), /Unknown snapshot parser/);
});
