import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { createRemoteJWKSet, jwtVerify } from 'jose';
import type { Database } from './db.js';

export type Provider = 'google' | 'apple';
export type Identity = { provider: Provider; subject: string };
export type IdentityVerifier = (provider: Provider, idToken: string) => Promise<Identity>;
const jwks = {
  google: createRemoteJWKSet(new URL('https://www.googleapis.com/oauth2/v3/certs')),
  apple: createRemoteJWKSet(new URL('https://appleid.apple.com/auth/keys')),
};

export function oidcVerifier(audiences: { google: string[]; apple: string[] }): IdentityVerifier {
  return async (provider, idToken) => {
    const audience = audiences[provider];
    if (!audience.length) throw new Error(`${provider} client IDs are not configured`);
    const verified = await jwtVerify(idToken, jwks[provider], {
      algorithms: ['RS256'],
      issuer: provider === 'google' ? ['accounts.google.com', 'https://accounts.google.com'] : 'https://appleid.apple.com',
      audience,
    });
    if (typeof verified.payload.sub !== 'string' || !verified.payload.sub) throw new Error('Missing identity subject');
    return { provider, subject: verified.payload.sub };
  };
}

export function hashToken(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

export async function exchangeIdentity(db: Database, identity: Identity) {
  const client = await db.connect();
  try {
    await client.query('BEGIN');
    // Serialize first-use account creation for one provider subject.
    await client.query('SELECT pg_advisory_xact_lock(hashtextextended($1, 0))', [`${identity.provider}:${identity.subject}`]);
    let account = await client.query<{ account_id: string }>(
      'SELECT account_id FROM account_identities WHERE provider = $1 AND subject = $2',
      [identity.provider, identity.subject],
    );
    let accountId = account.rows[0]?.account_id;
    if (!accountId) {
      accountId = randomUUID();
      await client.query('INSERT INTO accounts(id) VALUES ($1)', [accountId]);
      await client.query('INSERT INTO account_identities(provider, subject, account_id) VALUES ($1, $2, $3)', [identity.provider, identity.subject, accountId]);
    }
    const token = randomBytes(32).toString('base64url');
    const expiresAt = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString();
    await client.query('INSERT INTO sessions(token_hash, account_id, expires_at) VALUES ($1, $2, $3)', [hashToken(token), accountId, expiresAt]);
    await client.query('COMMIT');
    return { token, accountId, expiresAt };
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  } finally {
    client.release();
  }
}

export async function accountForToken(db: Database, token: string): Promise<string | null> {
  const result = await db.query<{ account_id: string }>(
    'SELECT account_id FROM sessions WHERE token_hash = $1 AND expires_at > now()',
    [hashToken(token)],
  );
  return result.rows[0]?.account_id ?? null;
}
