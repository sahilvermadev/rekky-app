use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Apple,
}

impl Provider {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "google" => Some(Self::Google),
            "apple" => Some(Self::Apple),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Apple => "apple",
        }
    }
    fn jwks_url(self) -> &'static str {
        match self {
            Self::Google => "https://www.googleapis.com/oauth2/v3/certs",
            Self::Apple => "https://appleid.apple.com/auth/keys",
        }
    }
    fn issuers(self) -> &'static [&'static str] {
        match self {
            Self::Google => &["accounts.google.com", "https://accounts.google.com"],
            Self::Apple => &["https://appleid.apple.com"],
        }
    }
}

#[derive(Debug)]
pub enum VerifyError {
    Unconfigured,
    Rejected,
}

#[async_trait]
pub trait IdentityVerifier: Send + Sync {
    async fn verify(&self, provider: Provider, token: &str) -> Result<String, VerifyError>;
}

type CachedKeys = Arc<RwLock<HashMap<&'static str, (Instant, JwkSet)>>>;

pub struct OidcVerifier {
    audiences: HashMap<&'static str, Vec<String>>,
    client: reqwest::Client,
    keys: CachedKeys,
}

impl OidcVerifier {
    pub fn from_env() -> Self {
        fn values(name: &str) -> Vec<String> {
            std::env::var(name)
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect()
        }
        let mut audiences = HashMap::new();
        audiences.insert("google", values("GOOGLE_CLIENT_IDS"));
        audiences.insert("apple", values("APPLE_CLIENT_IDS"));
        Self {
            audiences,
            client: reqwest::Client::builder()
                .timeout(StdDuration::from_secs(8))
                .build()
                .expect("HTTP client"),
            keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn keys(&self, provider: Provider, force_refresh: bool) -> Result<JwkSet, VerifyError> {
        let name = provider.as_str();
        if !force_refresh
            && let Some((at, set)) = self.keys.read().await.get(name)
            && at.elapsed() < StdDuration::from_secs(3600)
        {
            return Ok(set.clone());
        }
        let set = self
            .client
            .get(provider.jwks_url())
            .send()
            .await
            .map_err(|_| VerifyError::Rejected)?
            .error_for_status()
            .map_err(|_| VerifyError::Rejected)?
            .json::<JwkSet>()
            .await
            .map_err(|_| VerifyError::Rejected)?;
        self.keys
            .write()
            .await
            .insert(name, (Instant::now(), set.clone()));
        Ok(set)
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

#[async_trait]
impl IdentityVerifier for OidcVerifier {
    async fn verify(&self, provider: Provider, token: &str) -> Result<String, VerifyError> {
        let audiences = self
            .audiences
            .get(provider.as_str())
            .ok_or(VerifyError::Unconfigured)?;
        if audiences.is_empty() {
            return Err(VerifyError::Unconfigured);
        }
        let header = decode_header(token).map_err(|_| VerifyError::Rejected)?;
        if header.alg != Algorithm::RS256 {
            return Err(VerifyError::Rejected);
        }
        let kid = header.kid.ok_or(VerifyError::Rejected)?;
        let mut keys = self.keys(provider, false).await?;
        if !keys
            .keys
            .iter()
            .any(|key| key.common.key_id.as_deref() == Some(kid.as_str()))
        {
            keys = self.keys(provider, true).await?;
        }
        let jwk = keys
            .keys
            .iter()
            .find(|key| key.common.key_id.as_deref() == Some(kid.as_str()))
            .ok_or(VerifyError::Rejected)?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| VerifyError::Rejected)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.leeway = 0;
        validation.validate_nbf = true;
        validation
            .required_spec_claims
            .extend(["iss".into(), "aud".into(), "sub".into()]);
        validation.set_audience(audiences);
        validation.set_issuer(provider.issuers());
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|_| VerifyError::Rejected)?
            .claims;
        if claims.sub.is_empty() {
            return Err(VerifyError::Rejected);
        }
        Ok(claims.sub)
    }
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn account_for_token(pool: &PgPool, token: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let row =
        sqlx::query("SELECT account_id FROM sessions WHERE token_hash=$1 AND expires_at>now()")
            .bind(hash_token(token))
            .fetch_optional(pool)
            .await?;
    row.map(|r| r.try_get("account_id")).transpose()
}

pub async fn exchange_identity(
    pool: &PgPool,
    provider: Provider,
    subject: &str,
) -> Result<(Uuid, String, DateTime<Utc>), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("{}:{subject}", provider.as_str()))
        .execute(&mut *transaction)
        .await?;
    let found =
        sqlx::query("SELECT account_id FROM account_identities WHERE provider=$1 AND subject=$2")
            .bind(provider.as_str())
            .bind(subject)
            .fetch_optional(&mut *transaction)
            .await?;
    let account_id: Uuid = if let Some(row) = found {
        row.try_get("account_id")?
    } else {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO accounts(id) VALUES ($1)")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO account_identities(provider,subject,account_id) VALUES ($1,$2,$3)",
        )
        .bind(provider.as_str())
        .bind(subject)
        .bind(id)
        .execute(&mut *transaction)
        .await?;
        id
    };
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let expires_at = Utc::now() + Duration::days(30);
    sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,$3)")
        .bind(hash_token(&token))
        .bind(account_id)
        .bind(expires_at)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok((account_id, token, expires_at))
}
