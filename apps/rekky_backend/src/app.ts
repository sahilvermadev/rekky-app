import { createHash, randomUUID } from 'node:crypto';
import express, { type NextFunction, type Request, type Response } from 'express';
import { z } from 'zod';
import type { Database } from './db.js';
import { accountForToken, exchangeIdentity, hashToken, type IdentityVerifier } from './auth.js';

const exchangeInput = z.strictObject({ provider: z.enum(['google', 'apple']), id_token: z.string().min(1).max(12000) });
const itemInput = z.strictObject({ subject: z.string().trim().min(1).max(120), body: z.string().trim().min(1).max(20000), visibility: z.enum(['friends', 'private']).default('friends') });
const askInput = z.strictObject({ question: z.string().trim().min(1).max(500), cursor: z.string().max(500).optional() });
const uuid = z.uuid();
const keySchema = z.string().regex(/^[A-Za-z0-9_-]{8,128}$/);
const visibilityInput = z.strictObject({ visibility: z.enum(['friends', 'private']) });
function decodeCursor(value: unknown): { created_at: string; id: string } | null {
  if (value === undefined) return null;
  if (typeof value !== 'string' || value.length > 500) throw new Error('invalid_cursor');
  try {
    const parsed = JSON.parse(Buffer.from(value, 'base64url').toString());
    if (!z.strictObject({ created_at: z.iso.datetime(), id: uuid }).safeParse(parsed).success) throw new Error();
    return parsed;
  } catch { throw new Error('invalid_cursor'); }
}
function encodeCursor(row: { created_at: string; id: string }) { return Buffer.from(JSON.stringify(row)).toString('base64url'); }
declare global { namespace Express { interface Request { accountId?: string } } }
function error(res: Response, status: number, code: string, message: string) { return res.status(status).json({ error: { code, message } }); }

export function createApp(db: Database, verify: IdentityVerifier) {
  const app = express();
  app.disable('x-powered-by');
  app.use(express.json({ limit: '100kb' }));
  app.get('/health', (_req, res) => res.json({ status: 'ok' }));
  app.post('/v1/session/exchange', async (req, res) => {
    const input = exchangeInput.safeParse(req.body);
    if (!input.success) return error(res, 400, 'invalid_request', 'Invalid sign-in request');
    let identity;
    try {
      identity = await verify(input.data.provider, input.data.id_token);
    } catch (cause) {
      if (cause instanceof Error && cause.message.includes('not configured')) return error(res, 503, 'identity_unavailable', 'Identity provider is not configured');
      return error(res, 401, 'invalid_identity', 'Identity token was not accepted');
    }
    if (identity.provider !== input.data.provider || !identity.subject) return error(res, 401, 'invalid_identity', 'Identity token was not accepted');
    const result = await exchangeIdentity(db, identity);
    return res.status(201).json({ session: { token: result.token, expires_at: result.expiresAt }, account: { id: result.accountId } });
  });
  app.use('/v1', async (req, res, next) => {
    const match = /^Bearer ([A-Za-z0-9_-]{32,128})$/.exec(req.header('authorization') ?? '');
    if (!match?.[1]) return error(res, 401, 'unauthorized', 'Sign in is required');
    const accountId = await accountForToken(db, match[1]);
    if (!accountId) return error(res, 401, 'unauthorized', 'Session is invalid or expired');
    req.accountId = accountId;
    next();
  });
  app.get('/v1/me', async (req, res) => {
    const id = req.accountId!;
    const result = await db.query<{ disclosure_accepted_at: Date | null; processing_enabled: boolean; processing_generation: string }>('SELECT a.disclosure_accepted_at,COALESCE(p.enabled,false) processing_enabled,COALESCE(p.generation,0) processing_generation FROM accounts a LEFT JOIN processing_permissions p ON p.account_id=a.id WHERE a.id=$1', [id]);
    return res.json({ account: { id, visibility_disclosure_accepted: Boolean(result.rows[0]?.disclosure_accepted_at), processing_enabled: result.rows[0]?.processing_enabled ?? false, processing_generation: Number(result.rows[0]?.processing_generation ?? 0) } });
  });
  app.post('/v1/me/visibility-disclosure', async (req, res) => {
    if (!z.strictObject({ accept: z.literal(true) }).safeParse(req.body).success) return error(res, 400, 'invalid_request', 'Accept the visibility disclosure to continue');
    const id = req.accountId!;
    await db.query('UPDATE accounts SET disclosure_accepted_at=COALESCE(disclosure_accepted_at,now()) WHERE id=$1', [id]);
    return res.json({ account: { id, visibility_disclosure_accepted: true } });
  });
  app.delete('/v1/session', async (req, res) => {
    await db.query('DELETE FROM sessions WHERE token_hash=$1', [hashToken(req.header('authorization')!.slice(7))]);
    return res.status(204).end();
  });
  app.post('/v1/me/processing-withdrawal', async (req, res) => {
    if (!z.strictObject({}).safeParse(req.body ?? {}).success) return error(res, 400, 'invalid_request', 'Invalid withdrawal request');
    const result = await db.query<{ generation: string }>('INSERT INTO processing_permissions(account_id,enabled,generation) VALUES ($1,false,1) ON CONFLICT (account_id) DO UPDATE SET enabled=false,generation=processing_permissions.generation+1,updated_at=now() RETURNING generation', [req.accountId!]);
    return res.json({ processing: { enabled: false, generation: Number(result.rows[0]!.generation), acknowledged: true } });
  });
  app.use('/v1', async (req, res, next) => {
    const check = await db.query('SELECT 1 FROM accounts WHERE id=$1 AND disclosure_accepted_at IS NOT NULL', [req.accountId!]);
    if (!check.rowCount) return error(res, 403, 'visibility_disclosure_required', 'Accept the visibility disclosure before using Rekky');
    next();
  });
  // This explicitly saves one authored item. AI extraction of free-form text follows in F-01.
  app.post('/v1/items', async (req, res) => {
    const input = itemInput.safeParse(req.body);
    const key = keySchema.safeParse(req.header('idempotency-key'));
    if (!input.success || !key.success) return error(res, 400, 'invalid_request', 'Valid item and Idempotency-Key are required');
    const owner = req.accountId!;
    const payloadHash = createHash('sha256').update(JSON.stringify(input.data)).digest('hex');
    const client = await db.connect();
    try {
      await client.query('BEGIN');
      await client.query('SELECT pg_advisory_xact_lock(hashtextextended($1,0))', [`${owner}:manual_item:${key.data}`]);
      const prior = await client.query<{ payload_hash: string; response: { item: { id: string } } }>('SELECT payload_hash,response FROM idempotency_records WHERE account_id=$1 AND operation=$2 AND request_key=$3', [owner, 'manual_item', key.data]);
      if (prior.rows[0]) {
        if (prior.rows[0].payload_hash !== payloadHash) { await client.query('COMMIT'); return error(res, 409, 'idempotency_conflict', 'This key was used for another request'); }
        const live = await client.query<{ id: string; capture_id: string; subject: string; body: string; visibility: string; revision: number; created_at: Date }>('SELECT id,capture_id,subject,body,visibility,revision,created_at FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL', [prior.rows[0].response.item.id, owner]);
        await client.query('COMMIT');
        if (!live.rows[0]) return error(res, 410, 'item_deleted', 'The previously saved item was deleted');
        return res.json({ item: { ...live.rows[0], created_at: live.rows[0].created_at.toISOString() } });
      }
      const captureId = randomUUID(), sourceId = randomUUID(), itemId = randomUUID();
      await client.query('INSERT INTO captures(id,owner_id,kind,status,desired_visibility) VALUES ($1,$2,$3,$4,$5)', [captureId, owner, 'typed', 'completed', input.data.visibility]);
      await client.query('INSERT INTO source_texts(id,capture_id,owner_id,kind,content) VALUES ($1,$2,$3,$4,$5)', [sourceId, captureId, owner, 'typed', input.data.body]);
      const created = await client.query<{ created_at: Date }>('INSERT INTO knowledge_items(id,capture_id,owner_id,subject,body,visibility) VALUES ($1,$2,$3,$4,$5,$6) RETURNING created_at', [itemId, captureId, owner, input.data.subject, input.data.body, input.data.visibility]);
      const response = { item: { id: itemId, capture_id: captureId, subject: input.data.subject, body: input.data.body, visibility: input.data.visibility, revision: 1, created_at: created.rows[0]!.created_at.toISOString() } };
      await client.query('INSERT INTO idempotency_records(account_id,operation,request_key,payload_hash,response) VALUES ($1,$2,$3,$4,$5)', [owner, 'manual_item', key.data, payloadHash, JSON.stringify(response)]);
      await client.query('COMMIT');
      return res.status(201).json(response);
    } catch (cause) { await client.query('ROLLBACK'); throw cause; }
    finally { client.release(); }
  });
  app.get('/v1/items', async (req, res) => {
    let cursor;
    try { cursor = decodeCursor(req.query.cursor); } catch { return error(res, 400, 'invalid_cursor', 'Invalid page cursor'); }
    const rows = await db.query<{ id: string; capture_id: string; subject: string; body: string; visibility: string; revision: number; created_at: Date }>('SELECT id,capture_id,subject,body,visibility,revision,created_at FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL AND ($2::timestamptz IS NULL OR (created_at,id)<($2::timestamptz,$3::uuid)) ORDER BY created_at DESC,id DESC LIMIT 21', [req.accountId!, cursor?.created_at ?? null, cursor?.id ?? null]);
    const page = rows.rows.slice(0, 20).map((row) => ({ ...row, created_at: row.created_at.toISOString() }));
    return res.json({ items: page, next_cursor: rows.rows.length > 20 ? encodeCursor(page[page.length - 1]!) : null });
  });
  app.get('/v1/captures/:id', async (req, res) => {
    const id = uuid.safeParse(req.params.id);
    if (!id.success) return error(res, 400, 'invalid_request', 'Invalid capture ID');
    const rows = await db.query<{ id: string; status: string; revision: number; source_kind: string | null; source_content: string | null; source_revision: number | null }>('SELECT c.id,c.status,c.revision,s.kind source_kind,s.content source_content,s.revision source_revision FROM captures c LEFT JOIN source_texts s ON s.capture_id=c.id AND s.owner_id=c.owner_id WHERE c.id=$1 AND c.owner_id=$2 AND EXISTS (SELECT 1 FROM knowledge_items i WHERE i.capture_id=c.id AND i.owner_id=c.owner_id AND i.deleted_at IS NULL)', [id.data, req.accountId!]);
    const row = rows.rows[0];
    if (!row) return error(res, 404, 'not_found', 'Capture not found');
    return res.json({ capture: { id: row.id, status: row.status, revision: row.revision, source: row.source_content === null ? null : { kind: row.source_kind, text: row.source_content, revision: row.source_revision } } });
  });
  app.delete('/v1/captures/:id/source', async (req, res) => {
    const id = uuid.safeParse(req.params.id), revision = z.coerce.number().int().positive().safeParse(req.header('if-match'));
    if (!id.success || !revision.success) return error(res, 400, 'invalid_request', 'Capture ID and source If-Match revision are required');
    const result = await db.query('DELETE FROM source_texts WHERE capture_id=$1 AND owner_id=$2 AND revision=$3 RETURNING id', [id.data, req.accountId!, revision.data]);
    if (!result.rowCount) {
      const exists = await db.query('SELECT 1 FROM source_texts WHERE capture_id=$1 AND owner_id=$2', [id.data, req.accountId!]);
      return error(res, exists.rowCount ? 409 : 404, exists.rowCount ? 'revision_conflict' : 'not_found', exists.rowCount ? 'Source changed; refresh before deleting' : 'Source not found');
    }
    return res.status(204).end();
  });
  app.patch('/v1/items/:id', async (req, res) => {
    const id = uuid.safeParse(req.params.id), revision = z.coerce.number().int().positive().safeParse(req.header('if-match'));
    const input = visibilityInput.safeParse(req.body);
    if (!id.success || !revision.success || !input.success) return error(res, 400, 'invalid_request', 'Item ID, visibility and If-Match revision are required');
    const updated = await db.query<{ id: string; capture_id: string; subject: string; body: string; visibility: string; revision: number; created_at: Date }>('UPDATE knowledge_items SET visibility=$1,revision=revision+1 WHERE id=$2 AND owner_id=$3 AND revision=$4 AND deleted_at IS NULL RETURNING id,capture_id,subject,body,visibility,revision,created_at', [input.data.visibility, id.data, req.accountId!, revision.data]);
    if (!updated.rows[0]) {
      const exists = await db.query('SELECT 1 FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL', [id.data, req.accountId!]);
      return error(res, exists.rowCount ? 409 : 404, exists.rowCount ? 'revision_conflict' : 'not_found', exists.rowCount ? 'Item changed; refresh before editing' : 'Item not found');
    }
    return res.json({ item: { ...updated.rows[0], created_at: updated.rows[0].created_at.toISOString() } });
  });
  app.delete('/v1/items/:id', async (req, res) => {
    const id = uuid.safeParse(req.params.id), revision = z.coerce.number().int().positive().safeParse(req.header('if-match'));
    if (!id.success || !revision.success) return error(res, 400, 'invalid_request', 'Item ID and If-Match revision are required');
    const client = await db.connect();
    try {
      await client.query('BEGIN');
      const rows = await client.query<{ capture_id: string; revision: number }>('SELECT capture_id,revision FROM knowledge_items WHERE id=$1 AND owner_id=$2 AND deleted_at IS NULL FOR UPDATE', [id.data, req.accountId!]);
      const item = rows.rows[0];
      if (!item) { await client.query('ROLLBACK'); return error(res, 404, 'not_found', 'Item not found'); }
      if (item.revision !== revision.data) { await client.query('ROLLBACK'); return error(res, 409, 'revision_conflict', 'Item changed; refresh before deleting'); }
      await client.query('UPDATE knowledge_items SET deleted_at=now(),revision=revision+1 WHERE id=$1', [id.data]);
      // One manual item per capture; purge its owner-only source too.
      await client.query('DELETE FROM source_texts WHERE capture_id=$1 AND owner_id=$2', [item.capture_id, req.accountId!]);
      await client.query('COMMIT');
      return res.status(204).end();
    } catch (cause) { await client.query('ROLLBACK'); throw cause; }
    finally { client.release(); }
  });
  app.post('/v1/ask', async (req, res) => {
    const input = askInput.safeParse(req.body);
    if (!input.success) return error(res, 400, 'invalid_request', 'Invalid question');
    const questionWords = new Set(['who', 'what', 'where', 'which', 'did', 'does', 'the', 'our', 'can', 'is', 'in', 'for', 'me', 'to', 'a', 'an']);
    const terms = (input.data.question.toLowerCase().match(/[\p{L}\p{M}\p{N}]+/gu) ?? []).filter((term) => term.length >= 2 && !questionWords.has(term)).slice(0, 8);
    if (!terms.length) return res.json({ scope: 'own', results: [], answer: null, next_cursor: null });
    const queryHash = createHash('sha256').update(input.data.question).digest('hex');
    let offset = 0;
    if (input.data.cursor) {
      try {
        const decoded = JSON.parse(Buffer.from(input.data.cursor, 'base64url').toString());
        const parsed = z.strictObject({ offset: z.number().int().min(0).max(100000), question_hash: z.string() }).parse(decoded);
        if (parsed.question_hash !== queryHash) throw new Error();
        offset = parsed.offset;
      } catch { return error(res, 400, 'invalid_cursor', 'Invalid question cursor'); }
    }
    // A broad lexical baseline. Better intent/capability matching is evaluated in F-01/F-04.
    const searchTerms = terms.map((term) => `${term}:*`).join(' | ');
    const rows = await db.query<{ id: string; subject: string; body: string; visibility: string; revision: number }>(`SELECT id,subject,body,visibility,revision FROM knowledge_items WHERE owner_id=$1 AND deleted_at IS NULL AND to_tsvector('simple',subject||' '||body) @@ to_tsquery('simple',$2) ORDER BY ts_rank_cd(to_tsvector('simple',subject||' '||body),to_tsquery('simple',$2)) DESC,created_at DESC,id DESC LIMIT 21 OFFSET $3`, [req.accountId!, searchTerms, offset]);
    return res.json({ scope: 'own', results: rows.rows.slice(0, 20).map((row) => ({ item_id: row.id, subject: row.subject, body: row.body, visibility: row.visibility, revision: row.revision, evidence_item_id: row.id })), answer: null, next_cursor: rows.rows.length > 20 ? Buffer.from(JSON.stringify({ offset: offset + 20, question_hash: queryHash })).toString('base64url') : null });
  });
  app.use((cause: unknown, _req: Request, res: Response, _next: NextFunction) => {
    if (cause instanceof SyntaxError && 'body' in cause) return error(res, 400, 'invalid_json', 'Invalid JSON body');
    return error(res, 500, 'server_error', 'Request could not be completed');
  });
  return app;
}
