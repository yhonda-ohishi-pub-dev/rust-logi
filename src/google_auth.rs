use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Google JWKS endpoint
const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

/// Allowed issuers for Google ID tokens
const ALLOWED_ISSUERS: &[&str] = &["accounts.google.com", "https://accounts.google.com"];

/// Cache TTL in seconds (1 hour)
const JWKS_CACHE_TTL_SECS: u64 = 3600;

/// Claims extracted from a verified Google ID token
#[derive(Debug, Clone)]
pub struct GoogleClaims {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

/// Internal JWT claims structure for Google ID token
#[derive(Debug, Serialize, Deserialize)]
struct GoogleIdTokenClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
    aud: String,
    iss: String,
    exp: u64,
    iat: u64,
}

/// JWKS key from Google
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
    kty: String,
    alg: Option<String>,
}

/// JWKS response from Google
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

/// Cached JWKS keys
struct JwksCache {
    keys: HashMap<String, JwkKey>,
    fetched_at: std::time::Instant,
}

/// Google ID token verifier with JWKS caching
#[derive(Clone)]
pub struct GoogleTokenVerifier {
    client: Client,
    client_id: String,
    cache: Arc<RwLock<Option<JwksCache>>>,
}

impl GoogleTokenVerifier {
    pub fn new(client_id: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Verify a Google ID token and return the claims
    pub async fn verify(&self, id_token: &str) -> Result<GoogleClaims, String> {
        // Decode header to get kid
        let header = decode_header(id_token).map_err(|e| format!("Invalid token header: {}", e))?;
        let kid = header.kid.ok_or("Token missing kid header")?;

        // Get the matching key from JWKS
        let decoding_key = self.get_decoding_key(&kid).await?;

        // Validate the token
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.client_id]);
        validation.set_issuer(ALLOWED_ISSUERS);

        let token_data = decode::<GoogleIdTokenClaims>(id_token, &decoding_key, &validation)
            .map_err(|e| format!("Token validation failed: {}", e))?;

        let claims = token_data.claims;

        // Ensure email is present and verified
        let email = claims.email.ok_or("Token missing email claim")?;
        if claims.email_verified != Some(true) {
            return Err("Email not verified".to_string());
        }

        Ok(GoogleClaims {
            sub: claims.sub,
            email,
            name: claims.name,
            picture: claims.picture,
        })
    }

    async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, String> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                if cached.fetched_at.elapsed().as_secs() < JWKS_CACHE_TTL_SECS {
                    if let Some(key) = cached.keys.get(kid) {
                        return Self::jwk_to_decoding_key(key);
                    }
                }
            }
        }

        // Fetch fresh JWKS
        let response = self
            .client
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch JWKS: {}", e))?;

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JWKS: {}", e))?;

        let mut keys = HashMap::new();
        for key in jwks.keys {
            keys.insert(key.kid.clone(), key);
        }

        let decoding_key = keys
            .get(kid)
            .ok_or_else(|| format!("Key with kid '{}' not found in JWKS", kid))
            .and_then(Self::jwk_to_decoding_key)?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(JwksCache {
                keys,
                fetched_at: std::time::Instant::now(),
            });
        }

        Ok(decoding_key)
    }

    fn jwk_to_decoding_key(key: &JwkKey) -> Result<DecodingKey, String> {
        if key.kty != "RSA" {
            return Err(format!("Unsupported key type: {}", key.kty));
        }
        DecodingKey::from_rsa_components(&key.n, &key.e)
            .map_err(|e| format!("Failed to create decoding key: {}", e))
    }
}
