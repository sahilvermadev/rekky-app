import assert from 'node:assert/strict';
import { randomBytes, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { Pool } from 'pg';
import { createApp } from '../src/app.js';
import { hashToken } from '../src/auth.js';

type Case = { key: string; language: string; class: string; question: string; expected: string[]; forbidden: string[] };
type Corpus = { version: number; provenance: string; items: { key: string; subject: string; body: string }[]; questions: Case[] };
const corpusPath = fileURLToPath(new URL('../../../docs/evaluation/ask_text_seed_v0.json', import.meta.url));

test('synthetic own-Ask seed reports per-language lexical baseline', { skip: !process.env.DATABASE_URL }, async () => {
  const corpus = JSON.parse(await readFile(corpusPath, 'utf8')) as Corpus;
  assert.equal(corpus.version, 0);
  assert.match(corpus.provenance, /Synthetic/);
  assert.equal(new Set(corpus.items.map((item) => item.key)).size, corpus.items.length);
  const db = new Pool({ connectionString: process.env.DATABASE_URL });
  const accountId = randomUUID(), token = randomBytes(32).toString('base64url');
  const server = createServer(createApp(db, async () => { throw new Error('Not used in evaluation'); }));
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address && typeof address !== 'string');
  const base = `http://127.0.0.1:${address.port}`;
  const ids = new Map<string, string>();
  try {
    await db.query('INSERT INTO accounts(id,disclosure_accepted_at) VALUES ($1,now())', [accountId]);
    await db.query('INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,now()+interval \'1 hour\')', [hashToken(token), accountId]);
    for (const item of corpus.items) {
      const response = await fetch(`${base}/v1/items`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json', 'idempotency-key': randomUUID() }, body: JSON.stringify({ subject: item.subject, body: item.body, visibility: 'private' }) });
      assert.equal(response.status, 201);
      ids.set(item.key, ((await response.json()) as { item: { id: string } }).item.id);
    }
    const perLanguage = new Map<string, { hit: number; total: number; forbidden: number }>();
    const misses: string[] = [], hardFailures: string[] = [];
    for (const query of corpus.questions) {
      const response = await fetch(`${base}/v1/ask`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: JSON.stringify({ question: query.question }) });
      assert.equal(response.status, 200, query.key);
      const result = (await response.json()) as { results: { item_id: string; body: string }[] };
      if (query.key === 'no_answer') assert.equal(result.results.length, 0);
      if (query.key === 'warning') assert.ok(result.results.some((row) => row.item_id === ids.get('kuuraku') && row.body.includes('small tables')));
      const shown = new Set(result.results.slice(0, 5).map((row) => row.item_id));
      const metric = perLanguage.get(query.language) ?? { hit: 0, total: 0, forbidden: 0 };
      if (query.expected.length) {
        metric.total++;
        if (query.expected.some((key) => shown.has(ids.get(key)!))) metric.hit++;
        else misses.push(query.key);
      }
      if (query.forbidden.some((key) => shown.has(ids.get(key)!))) { metric.forbidden++; hardFailures.push(query.key); }
      perLanguage.set(query.language, metric);
    }
    // This corpus is a starting measurement, not a launch-quality threshold.
    process.stdout.write(`Synthetic lexical baseline: ${JSON.stringify({ by_language: Object.fromEntries(perLanguage), missed_recall: misses, forbidden_results: hardFailures })}\n`);
    assert.equal(corpus.questions.length, 10);
  } finally {
    await db.query('DELETE FROM accounts WHERE id=$1', [accountId]);
    await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    await db.end();
  }
});
