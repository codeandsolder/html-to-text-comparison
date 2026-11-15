import test from 'node:test';
import assert from 'node:assert/strict';
import { acquireSnapshot } from '../cloakbrowser-acquire.mjs';

test('one navigation yields one immutable HTML snapshot and closes browser', async () => {
    const events = [];
    const launch = async (options) => {
        events.push(['launch', options]);
        return {
            newPage: async () => {
                events.push(['newPage']);
                return {
                    goto: async (...args) => events.push(['goto', ...args]),
                    waitForTimeout: async (ms) => events.push(['settle', ms]),
                    content: async () => {
                        events.push(['content']);
                        return '<html><body>snapshot</body></html>';
                    },
                };
            },
            close: async () => events.push(['close']),
        };
    };
    const html = await acquireSnapshot(launch, 'https://example.com/fixture', 25);
    assert.equal(html, '<html><body>snapshot</body></html>');
    assert.equal(events.filter(([event]) => event === 'goto').length, 1);
    assert.deepEqual(events.map(([event]) => event), ['launch', 'newPage', 'goto', 'settle', 'content', 'close']);
});

test('closes browser after navigation error', async () => {
    let closed = false;
    const launch = async () => ({
        newPage: async () => ({
            goto: async () => { throw new Error('navigation failed'); },
        }),
        close: async () => { closed = true; },
    });
    await assert.rejects(acquireSnapshot(launch, 'https://example.com/', 0), /navigation failed/);
    assert.ok(closed);
});
