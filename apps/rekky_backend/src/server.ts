import 'dotenv/config';
import { createApp } from './app.js';
import { createPool } from './db.js';
import { oidcVerifier } from './auth.js';

const databaseUrl = process.env.DATABASE_URL;
if (!databaseUrl) throw new Error('DATABASE_URL is required');
const port = Number(process.env.PORT ?? 3088);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be a valid port');
const host = process.env.HOST ?? '127.0.0.1';
const audiences = {
  google: (process.env.GOOGLE_CLIENT_IDS ?? '').split(',').map((value) => value.trim()).filter(Boolean),
  apple: (process.env.APPLE_CLIENT_IDS ?? '').split(',').map((value) => value.trim()).filter(Boolean),
};
const db = createPool(databaseUrl);
const server = createApp(db, oidcVerifier(audiences)).listen(port, host, () => process.stdout.write(`Rekky backend listening on ${host}:${port}\n`));
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => server.close(() => void db.end()));
