use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SLACK_API_BASE: &str = "https://slack.com/api";

#[derive(Debug)]
pub enum SlackError {
    Http(String),
    Api(String),
    Json(String),
}

impl std::fmt::Display for SlackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlackError::Http(msg) => write!(f, "Slack HTTP error: {}", msg),
            SlackError::Api(msg) => write!(f, "Slack API error: {}", msg),
            SlackError::Json(msg) => write!(f, "Slack JSON error: {}", msg),
        }
    }
}

impl std::error::Error for SlackError {}

/// Low-level Slack API client wrapping reqwest with a token.
pub struct SlackClient {
    http: Client,
    token: String,
}

impl SlackClient {
    pub fn new(token: &str) -> Self {
        Self {
            http: Client::new(),
            token: token.to_string(),
        }
    }

    /// Generic helper for GET requests to the Slack API.
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.token)
            .query(params)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let body = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&body).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&body).map_err(|e| SlackError::Json(e.to_string()))
    }

    /// Generic helper for POST requests to the Slack API (JSON body).
    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: &impl Serialize,
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let text = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))
    }

    /// Generic helper for POST requests with form-encoded body.
    async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SlackError> {
        let url = format!("{}/{}", SLACK_API_BASE, method);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .form(params)
            .send()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let text = resp
            .text()
            .await
            .map_err(|e| SlackError::Http(e.to_string()))?;

        let envelope: SlackEnvelope =
            serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))?;

        if !envelope.ok {
            return Err(SlackError::Api(envelope.error.unwrap_or_default()));
        }

        serde_json::from_str(&text).map_err(|e| SlackError::Json(e.to_string()))
    }

    // ---- auth.test ----

    /// Validate the token and return the authenticated user/team info.
    pub async fn auth_test(&self) -> Result<AuthTestResponse, SlackError> {
        self.post_form("auth.test", &[]).await
    }

    // ---- users.list ----

    /// Fetch all users in the workspace. Returns a map of user_id -> username
    /// for non-deleted users.
    pub async fn get_users(&self) -> Result<HashMap<String, String>, SlackError> {
        let resp: UsersListResponse = self.get("users.list", &[]).await?;
        let mut cache = HashMap::new();
        for user in resp.members {
            if !user.deleted {
                cache.insert(user.id, user.name);
            }
        }
        Ok(cache)
    }

    // ---- users.info ----

    /// Get info for a single user by ID.
    pub async fn get_user_info(&self, user_id: &str) -> Result<UserInfo, SlackError> {
        let resp: UserInfoResponse = self.get("users.info", &[("user", user_id)]).await?;
        Ok(resp.user)
    }

    // ---- bots.info ----

    /// Get info for a bot by bot ID.
    pub async fn get_bot_info(&self, bot_id: &str) -> Result<BotInfo, SlackError> {
        let resp: BotInfoResponse = self.get("bots.info", &[("bot", bot_id)]).await?;
        Ok(resp.bot)
    }

    // ---- users.getPresence ----

    /// Get the presence status of a user ("active" or "away").
    pub async fn get_user_presence(&self, user_id: &str) -> Result<String, SlackError> {
        let resp: PresenceResponse = self.get("users.getPresence", &[("user", user_id)]).await?;
        Ok(resp.presence)
    }

    // ---- users.setPresence ----

    /// Set the current user's presence (typically "auto").
    pub async fn set_user_presence(&self, presence: &str) -> Result<(), SlackError> {
        let _: SlackEnvelope = self
            .post_json(
                "users.setPresence",
                &serde_json::json!({ "presence": presence }),
            )
            .await?;
        Ok(())
    }
}

// ---- Response types ----

/// Minimal envelope for checking ok/error on any Slack API response.
#[derive(Debug, Deserialize)]
struct SlackEnvelope {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

/// Response from auth.test.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthTestResponse {
    #[allow(dead_code)]
    ok: bool,
    pub url: String,
    pub team: String,
    pub user: String,
    pub team_id: String,
    pub user_id: String,
}

/// Response from users.list.
#[derive(Debug, Deserialize)]
struct UsersListResponse {
    #[allow(dead_code)]
    ok: bool,
    members: Vec<UserMember>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserMember {
    id: String,
    name: String,
    #[serde(default)]
    deleted: bool,
}

/// Response from users.info.
#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[allow(dead_code)]
    ok: bool,
    user: UserInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub deleted: bool,
}

/// Response from bots.info.
#[derive(Debug, Deserialize)]
struct BotInfoResponse {
    #[allow(dead_code)]
    ok: bool,
    bot: BotInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotInfo {
    pub id: String,
    pub name: String,
}

/// Response from users.getPresence.
#[derive(Debug, Deserialize)]
struct PresenceResponse {
    #[allow(dead_code)]
    ok: bool,
    presence: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_client_new() {
        let client = SlackClient::new("xoxp-test-token");
        assert_eq!(client.token, "xoxp-test-token");
    }

    #[test]
    fn test_auth_test_response_deserialization() {
        let json = r#"{
            "ok": true,
            "url": "https://myteam.slack.com/",
            "team": "My Team",
            "user": "alice",
            "team_id": "T123",
            "user_id": "U456"
        }"#;

        let resp: AuthTestResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.user_id, "U456");
        assert_eq!(resp.team_id, "T123");
        assert_eq!(resp.user, "alice");
        assert_eq!(resp.team, "My Team");
    }

    #[test]
    fn test_users_list_response_deserialization() {
        let json = r#"{
            "ok": true,
            "members": [
                {"id": "U1", "name": "alice", "deleted": false},
                {"id": "U2", "name": "bob", "deleted": true},
                {"id": "U3", "name": "charlie", "deleted": false}
            ]
        }"#;

        let resp: UsersListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.members.len(), 3);
        assert!(!resp.members[0].deleted);
        assert!(resp.members[1].deleted);
    }

    #[test]
    fn test_users_list_filtering() {
        // Simulate the cache-building logic
        let members = vec![
            UserMember {
                id: "U1".into(),
                name: "alice".into(),
                deleted: false,
            },
            UserMember {
                id: "U2".into(),
                name: "bob".into(),
                deleted: true,
            },
            UserMember {
                id: "U3".into(),
                name: "charlie".into(),
                deleted: false,
            },
        ];

        let mut cache = HashMap::new();
        for user in members {
            if !user.deleted {
                cache.insert(user.id, user.name);
            }
        }

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get("U1").unwrap(), "alice");
        assert_eq!(cache.get("U3").unwrap(), "charlie");
        assert!(!cache.contains_key("U2"));
    }

    #[test]
    fn test_user_info_response_deserialization() {
        let json = r#"{
            "ok": true,
            "user": {
                "id": "U123",
                "name": "alice",
                "deleted": false
            }
        }"#;

        let resp: UserInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.user.id, "U123");
        assert_eq!(resp.user.name, "alice");
    }

    #[test]
    fn test_bot_info_response_deserialization() {
        let json = r#"{
            "ok": true,
            "bot": {
                "id": "B123",
                "name": "mybot"
            }
        }"#;

        let resp: BotInfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.bot.id, "B123");
        assert_eq!(resp.bot.name, "mybot");
    }

    #[test]
    fn test_presence_response_deserialization() {
        let json = r#"{"ok": true, "presence": "active"}"#;
        let resp: PresenceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.presence, "active");
    }

    #[test]
    fn test_slack_envelope_ok() {
        let json = r#"{"ok": true}"#;
        let env: SlackEnvelope = serde_json::from_str(json).unwrap();
        assert!(env.ok);
        assert!(env.error.is_none());
    }

    #[test]
    fn test_slack_envelope_error() {
        let json = r#"{"ok": false, "error": "invalid_auth"}"#;
        let env: SlackEnvelope = serde_json::from_str(json).unwrap();
        assert!(!env.ok);
        assert_eq!(env.error.unwrap(), "invalid_auth");
    }

    #[test]
    fn test_slack_error_display() {
        let err = SlackError::Api("not_authed".to_string());
        assert_eq!(format!("{}", err), "Slack API error: not_authed");

        let err = SlackError::Http("connection refused".to_string());
        assert_eq!(
            format!("{}", err),
            "Slack HTTP error: connection refused"
        );
    }
}
