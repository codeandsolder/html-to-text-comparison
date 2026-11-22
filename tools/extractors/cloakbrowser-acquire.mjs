import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { importLocalPackage } from './local-package.mjs';

/**
 * One and only one browser navigation for a page acquisition.
 * All downstream parsers consume the resulting HTML snapshot.
 *
 * Inject the launcher for deterministic tests without downloading Chromium or
 * requiring a license; production uses the real CloakBrowser Playwright API.
 */
export async function acquireSnapshot(launchBrowser, targetUrl, settleMs = 1000) {
    const browser = await launchBrowser({ headless: true });
    try {
        const page = await browser.newPage();
        await page.goto(targetUrl, { waitUntil: 'domcontentloaded', timeout: 45000 });
        if (settleMs > 0) await page.waitForTimeout(settleMs);
        return await page.content();
    } finally {
        await browser.close();
    }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    const targetUrl = process.argv[2];
    if (!targetUrl || !/^https?:\/\//i.test(targetUrl)) {
        console.error('Usage: node cloakbrowser-acquire.mjs <http(s)-url>');
        process.exitCode = 2;
    } else {
        try {
            // Reserve stdout strictly for the HTML snapshot.
            console.log = (...args) => console.error(...args);
            const { launch } = await importLocalPackage('cloakbrowser');
            const html = await acquireSnapshot(launch, targetUrl);
            process.stdout.write(html);
        } catch (error) {
            console.error('CloakBrowser acquisition failed:', error);
            process.exitCode = 1;
        }
    }
}
