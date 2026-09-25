import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { Pool } from 'pg';
import { createApp } from '../src/app.js';
import type { IdentityVerifier } from '../src/auth.js';

const wirePath = fileURLToPath(new URL('../../../contracts/rekky/v1/fixtures/wire.json', import.meta.url));
test('versioned wire fixtures have parseable account, item, Ask and error examples', async () => {
  const wire = JSON.parse(await readFile(wirePath, 'utf8')) as { version: number; examples: { name: string; status: number; body: Record<string, unknown> }[] };
  assert.equal(wire.version, 1);
  const examples = new Map(wire.examples.map((entry) => [entry.name, entry]));
  for (const name of ['session', 'signed_out', 'disclosure_required', 'account', 'saved_item', 'private_override_ack', 'items_page', 'ask_result', 'text_retained_audio_deleted', 'source_deleted_item_retained', 'idempotency_conflict', 'processing_withdrawal_ack']) assert.ok(examples.has(name), name);
  assert.equal((examples.get('saved_item')!.body.item as { visibility: string }).visibility, 'friends');
  assert.equal((examples.get('private_override_ack')!.body.item as { revision: number }).revision, 2);
  assert.equal((examples.get('text_retained_audio_deleted')!.body.capture as { source: { kind: string } }).source.kind, 'transcript');
  assert.equal((examples.get('ask_result')!.body.results as unknown[]).length, 1);
  assert.equal((examples.get('signed_out')!.body.error as { code: string }).code, 'unauthorized');
});

test('signed-in text memory, owner isolation, revision and non-resurrection', { skip: !process.env.DATABASE_URL }, async () => {
  const db = new Pool({ connectionString: process.env.DATABASE_URL });
  const marker = randomUUID();
  const verify: IdentityVerifier = async (provider, idToken) => {
    if (idToken !== 'valid-a' && idToken !== 'valid-b') throw new Error('invalid token');
    return { provider, subject: `${marker}:${idToken}` };
  };
  const server = createServer(createApp(db, verify));
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address && typeof address !== 'string');
  const base = `http://127.0.0.1:${address.port}`;
  const accountIds: string[] = [];
  async function call(path: string, options: { method?: string; token?: string; body?: unknown; headers?: Record<string, string> } = {}) {
    const response = await fetch(base + path, { method: options.method ?? 'GET', headers: { ...(options.token ? { authorization: `Bearer ${options.token}` } : {}), ...(options.body ? { 'content-type': 'application/json' } : {}), ...options.headers }, body: options.body ? JSON.stringify(options.body) : undefined });
    return { status: response.status, body: response.status === 204 ? null : await response.json() as any };
  }
  try {
    assert.equal((await call('/v1/items')).status, 401);
    assert.equal((await call('/v1/session/exchange', { method: 'POST', body: { provider: 'google', id_token: 'bad' } })).status, 401);
    const signInA = await call('/v1/session/exchange', { method: 'POST', body: { provider: 'google', id_token: 'valid-a' } });
    const signInB = await call('/v1/session/exchange', { method: 'POST', body: { provider: 'apple', id_token: 'valid-b' } });
    assert.equal(signInA.status, 201);
    accountIds.push(signInA.body.account.id, signInB.body.account.id);
    const a = signInA.body.session.token as string, b = signInB.body.session.token as string;
    const foreignCapture = randomUUID();
    await db.query('INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,$3,$4,$5)', [foreignCapture, signInA.body.account.id, 'typed', 'completed', 'friends']);
    await assert.rejects(db.query('INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,$4,$5)', [randomUUID(), foreignCapture, signInB.body.account.id, 'typed', 'private text']));
    assert.equal((await call('/v1/items', { token: a })).status, 403);
    assert.equal((await call('/v1/me/visibility-disclosure', { method: 'POST', token: a, body: { accept: true } })).status, 200);
    assert.equal((await call('/v1/me/visibility-disclosure', { method: 'POST', token: b, body: { accept: true } })).status, 200);
    const key = randomUUID();
    const input = { subject: 'Raju', body: 'Raju fixed our kitchen tap; ask Priya for his number.' };
    const saved = await call('/v1/items', { method: 'POST', token: a, body: input, headers: { 'idempotency-key': key } });
    assert.equal(saved.status, 201);
    assert.equal(saved.body.item.visibility, 'friends');
    assert.equal((await call('/v1/items', { token: b })).body.items.length, 0);
    assert.equal((await call(`/v1/captures/${saved.body.item.capture_id}`, { token: b })).status, 404);
    const source = await call(`/v1/captures/${saved.body.item.capture_id}`, { token: a });
    assert.equal(source.body.capture.source.text, input.body);
    await db.query('INSERT INTO processing_permissions(account_id,enabled,generation,purpose,provider_ids,disclosure_version) VALUES ($1,true,1,$2,$3,1)', [signInA.body.account.id, 'test-only processing permission', ['test-only']]);
    const withdrawal = await call('/v1/me/processing-withdrawal', { method: 'POST', token: a, body: {} });
    assert.deepEqual(withdrawal.body.processing, { enabled: false, generation: 2, acknowledged: true });
    assert.equal((await call(`/v1/captures/${saved.body.item.capture_id}`, { token: a })).body.capture.source.text, input.body);
    assert.equal((await call('/v1/items', { token: a })).body.items.length, 1);
    const replay = await call('/v1/items', { method: 'POST', token: a, body: input, headers: { 'idempotency-key': key } });
    assert.equal(replay.body.item.id, saved.body.item.id);
    assert.equal((await call('/v1/items', { method: 'POST', token: a, body: { ...input, subject: 'Other' }, headers: { 'idempotency-key': key } })).status, 409);
    const changed = await call(`/v1/items/${saved.body.item.id}`, { method: 'PATCH', token: a, body: { visibility: 'private' }, headers: { 'if-match': '1' } });
    assert.equal(changed.body.item.visibility, 'private');
    assert.equal(changed.body.item.revision, 2);
    const afterPrivacyChangeReplay = await call('/v1/items', { method: 'POST', token: a, body: input, headers: { 'idempotency-key': key } });
    assert.equal(afterPrivacyChangeReplay.body.item.visibility, 'private');
    assert.equal(afterPrivacyChangeReplay.body.item.revision, 2);
    assert.equal((await call(`/v1/items/${saved.body.item.id}`, { method: 'PATCH', token: a, body: { visibility: 'friends' }, headers: { 'if-match': '1' } })).status, 409);
    const ask = await call('/v1/ask', { method: 'POST', token: a, body: { question: 'Who fixed the kitchen tap?' } });
    assert.equal(ask.body.results[0].item_id, saved.body.item.id);
    assert.equal((await call('/v1/ask', { method: 'POST', token: b, body: { question: 'kitchen tap' } })).body.results.length, 0);
    const another = await call('/v1/items', { method: 'POST', token: a, body: { subject: 'Meera', body: 'Meera teaches swimming, but I have not tried her class.', visibility: 'private' }, headers: { 'idempotency-key': randomUUID() } });
    assert.equal((await call(`/v1/captures/${another.body.item.capture_id}/source`, { method: 'DELETE', token: b, headers: { 'if-match': '1' } })).status, 404);
    assert.equal((await call(`/v1/captures/${another.body.item.capture_id}/source`, { method: 'DELETE', token: a, headers: { 'if-match': '2' } })).status, 409);
    assert.equal((await call(`/v1/captures/${another.body.item.capture_id}/source`, { method: 'DELETE', token: a, headers: { 'if-match': '1' } })).status, 204);
    assert.equal((await call(`/v1/captures/${another.body.item.capture_id}`, { token: a })).body.capture.source, null);
    assert.equal((await call('/v1/ask', { method: 'POST', token: a, body: { question: 'swimming' } })).body.results[0].item_id, another.body.item.id);
    assert.equal((await call(`/v1/items/${saved.body.item.id}`, { method: 'DELETE', token: a, headers: { 'if-match': '1' } })).status, 409);
    assert.equal((await call(`/v1/items/${saved.body.item.id}`, { method: 'DELETE', token: a, headers: { 'if-match': '2' } })).status, 204);
    assert.equal((await call(`/v1/captures/${saved.body.item.capture_id}`, { token: a })).status, 404);
    assert.equal((await db.query('SELECT 1 FROM source_texts WHERE capture_id=$1', [saved.body.item.capture_id])).rowCount, 0);
    assert.equal((await call('/v1/items', { method: 'POST', token: a, body: input, headers: { 'idempotency-key': key } })).status, 410);
    assert.equal((await call('/v1/ask', { method: 'POST', token: a, body: { question: 'kitchen tap' } })).body.results.length, 0);
    assert.equal((await call('/v1/session', { method: 'DELETE', token: a })).status, 204);
    assert.equal((await call('/v1/me', { token: a })).status, 401);
  } finally {
    for (const id of accountIds) await db.query('DELETE FROM accounts WHERE id=$1', [id]);
    await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
    await db.end();
  }
});
