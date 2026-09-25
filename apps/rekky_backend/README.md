# Rekky backend

Fresh Express/TypeScript/Postgres API. This first slice supports verified Google/Apple identity exchange, new-product sessions, account disclosure, manual text items, owner-only source reads/deletion, Private overrides, item deletion, and own-only lexical Ask. Processing withdrawal has an acknowledged generation and leaves saved items/sources intact; voice jobs will need to enforce that fence when they arrive. The API does not yet process voice, contact discovery or friend access.

Use new-product accounts, database credentials, storage access, sessions, jobs and deployments. Do not depend on legacy routes or database tables. Keep provider credentials server-side.

From the repository root, start the isolated local database with `docker compose -f compose.dev.yml up -d --wait`. In this directory, run `npm ci`, set `DATABASE_URL` using `.env.example`, then run `npm run migrate` and `npm run dev`. Set `GOOGLE_CLIENT_IDS` and `APPLE_CLIENT_IDS` to the actual application audiences before testing sign-in. Unconfigured providers fail closed. Production should use a separate TLS database URL and set `HOST=0.0.0.0` behind an HTTPS ingress.

Checks: `npm run typecheck`, `npm run build`, and `npm test` with `DATABASE_URL` pointing to a migrated isolated test database. The integration test creates and removes its own accounts. Migrations are tracked by `schema_migrations`; do not run them against the legacy database.
