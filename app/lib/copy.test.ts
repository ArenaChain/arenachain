// Brand book: never use the word "stake" (or staking, staked...). Players pay an entry fee into a pool.
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';

const ROOT = new URL('..', import.meta.url).pathname;
const DIRS = ['app', 'components', 'lib'];
const BANNED = /\bstak(e|es|ed|er|ers|ing)\b/i;

function files(dir: string): string[] {
    return readdirSync(dir).flatMap((name) => {
        const path = join(dir, name);
        if (statSync(path).isDirectory()) return files(path);
        return /\.(tsx?|css|md)$/.test(name) && !name.endsWith('.test.ts') ? [path] : [];
    });
}

test('no "stake" in app copy', () => {
    const hits = [...DIRS.flatMap((d) => files(join(ROOT, d))), join(ROOT, 'README.md')].flatMap((path) =>
        readFileSync(path, 'utf8')
            .split('\n')
            .flatMap((line, i) => (BANNED.test(line) ? [`${path.slice(ROOT.length)}:${i + 1}: ${line.trim()}`] : [])),
    );
    assert.deepEqual(hits, []);
});
