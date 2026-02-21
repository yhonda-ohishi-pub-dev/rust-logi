//! SSO provider registry — provider-specific endpoints and profile fetching
//!
//! Each provider implements the standard OAuth2 authorization code flow
//! with different endpoints, scopes, and profile response formats.

use serde::Deserialize;

/// Supported SSO providers
pub enum Provider {
    Lineworks,
    // Discord,  // future
    // Slack,    // future
}

impl Provider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lineworks" => Some(Self::Lineworks),
            // "discord" => Some(Self::Discord),
            // "slack" => Some(Self::Slack),
            _ => None,
        }
    }

    pub fn authorize_url(&self) -> &str {
        match self {
            Self::Lineworks => "https://auth.worksmobile.com/oauth2/v2.0/authorize",
        }
    }

    pub fn token_url(&self) -> &str {
        match self {
            Self::Lineworks => "https://auth.worksmobile.com/oauth2/v2.0/token",
        }
    }

    pub fn userinfo_url(&self) -> &str {
        match self {
            Self::Lineworks => "https://www.worksapis.com/v1.0/users/me",
        }
    }

    pub fn default_scopes(&self) -> &str {
        match self {
            Self::Lineworks => "user.profile.read",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Lineworks => "lineworks",
        }
    }
}

/// Unified user profile returned by all providers
pub struct SsoUserProfile {
    pub provider_user_id: String,
    pub email: Option<String>,
    pub display_name: String,
}

/// Exchange authorization code for access token (standard OAuth2)
pub async fn exchange_code(
    http_client: &reqwest::Client,
    provider: &Provider,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];

    let response = http_client
        .post(provider.token_url())
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed: status={}, body={}", status, body));
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let token: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    Ok(token.access_token)
}

/// Fetch user profile from provider's userinfo endpoint
pub async fn fetch_user_profile(
    http_client: &reqwest::Client,
    provider: &Provider,
    access_token: &str,
) -> Result<SsoUserProfile, String> {
    let response = http_client
        .get(provider.userinfo_url())
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Profile request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Profile fetch failed: status={}, body={}", status, body));
    }

    match provider {
        Provider::Lineworks => parse_lineworks_profile(response).await,
    }
}

// --- Provider-specific profile parsers ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineworksProfile {
    user_id: String,
    #[serde(default)]
    user_name: Option<LineworksUserName>,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineworksUserName {
    #[serde(default)]
    last_name: Option<String>,
    #[serde(default)]
    first_name: Option<String>,
}

async fn parse_lineworks_profile(response: reqwest::Response) -> Result<SsoUserProfile, String> {
    let profile: LineworksProfile = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LINE WORKS profile: {}", e))?;

    let display_name = if let Some(ref name) = profile.user_name {
        let parts: Vec<&str> = [name.last_name.as_deref(), name.first_name.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        if !parts.is_empty() {
            parts.join(" ")
        } else {
            profile.email.clone().unwrap_or_else(|| profile.user_id.clone())
        }
    } else {
        profile.email.clone().unwrap_or_else(|| profile.user_id.clone())
    };

    Ok(SsoUserProfile {
        provider_user_id: profile.user_id,
        email: profile.email,
        display_name,
    })
}

/// Build the full authorize URL for a provider
pub fn build_authorize_url(
    provider: &Provider,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        provider.authorize_url(),
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(provider.default_scopes()),
        urlencoding::encode(state),
    )
}
