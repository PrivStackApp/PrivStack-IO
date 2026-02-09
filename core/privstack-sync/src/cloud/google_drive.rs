//! Google Drive storage implementation.
//!
//! Uses Google Drive API v3 for file operations.

use super::storage::{ChangeSet, CloudFile, CloudStorage, CloudStorageConfig};
use crate::error::{SyncError, SyncResult};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Google Drive specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: String,
    /// Redirect URI for OAuth flow.
    pub redirect_uri: String,
    /// Base cloud storage config.
    #[serde(flatten)]
    pub base: CloudStorageConfig,
    /// Base URL for Google Drive API (e.g. `https://www.googleapis.com`).
    pub api_base_url: String,
    /// Base URL for Google OAuth2 (e.g. `https://oauth2.googleapis.com`).
    pub oauth_base_url: String,
    /// Base URL for Google Accounts auth page (e.g. `https://accounts.google.com`).
    pub auth_base_url: String,
}

impl Default for GoogleDriveConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "urn:ietf:wg:oauth:2.0:oob".to_string(),
            base: CloudStorageConfig::default(),
            api_base_url: "https://www.googleapis.com".to_string(),
            oauth_base_url: "https://oauth2.googleapis.com".to_string(),
            auth_base_url: "https://accounts.google.com".to_string(),
        }
    }
}

/// OAuth2 tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OAuthTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<SystemTime>,
}

/// Google Drive API response structures.
#[derive(Debug, Deserialize)]
struct DriveFileList {
    files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(rename = "mimeType")]
    #[allow(dead_code)]
    mime_type: String,
    size: Option<String>,
    #[serde(rename = "modifiedTime")]
    modified_time: Option<String>,
    #[serde(rename = "md5Checksum")]
    md5_checksum: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveChanges {
    changes: Vec<DriveChange>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "newStartPageToken")]
    new_start_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveChange {
    removed: Option<bool>,
    #[serde(rename = "fileId")]
    file_id: String,
    file: Option<DriveFile>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

/// Google Drive storage implementation.
pub struct GoogleDriveStorage {
    config: GoogleDriveConfig,
    client: Client,
    tokens: Arc<RwLock<Option<OAuthTokens>>>,
    sync_folder_id: Arc<RwLock<Option<String>>>,
}

impl GoogleDriveStorage {
    /// Creates a new Google Drive storage instance.
    pub fn new(config: GoogleDriveConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to create HTTP client");

        Self {
            config,
            client,
            tokens: Arc::new(RwLock::new(None)),
            sync_folder_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Sets existing tokens (e.g., loaded from storage).
    pub async fn set_tokens(&self, access_token: String, refresh_token: Option<String>) {
        let tokens = OAuthTokens {
            access_token,
            refresh_token,
            expires_at: None,
        };
        *self.tokens.write().await = Some(tokens);
    }

    /// Gets the OAuth2 authorization URL.
    fn get_auth_url(&self) -> String {
        let scope = "https://www.googleapis.com/auth/drive.file";
        format!(
            "{}/o/oauth2/v2/auth?\
            client_id={}&\
            redirect_uri={}&\
            response_type=code&\
            scope={}&\
            access_type=offline&\
            prompt=consent",
            self.config.auth_base_url,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(scope)
        )
    }

    /// Gets the current access token, refreshing if needed.
    async fn get_access_token(&self) -> SyncResult<String> {
        let (access_token, expired) = {
            let guard = self.tokens.read().await;
            let tokens = guard
                .as_ref()
                .ok_or_else(|| SyncError::Auth("not authenticated".to_string()))?;

            let expired = tokens
                .expires_at
                .map_or(false, |exp| SystemTime::now() > exp);

            (tokens.access_token.clone(), expired)
        }; // read lock dropped here

        if expired {
            return self.refresh_token().await;
        }

        Ok(access_token)
    }

    /// Refreshes the access token.
    async fn refresh_token(&self) -> SyncResult<String> {
        let tokens = self.tokens.read().await;
        let refresh_token = tokens
            .as_ref()
            .and_then(|t| t.refresh_token.as_ref())
            .ok_or_else(|| SyncError::Auth("no refresh token available".to_string()))?
            .clone();
        drop(tokens);

        debug!("Refreshing Google Drive access token");

        let response = self
            .client
            .post(format!("{}/token", self.config.oauth_base_url))
            .form(&[
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("refresh_token", &refresh_token),
                ("grant_type", &"refresh_token".to_string()),
            ])
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("token refresh failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(SyncError::Auth(format!("token refresh failed: {error}")));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| SyncError::Auth(format!("failed to parse token response: {e}")))?;

        let expires_at = token_response
            .expires_in
            .map(|secs| SystemTime::now() + Duration::from_secs(secs.saturating_sub(60))); // 60s buffer

        let new_tokens = OAuthTokens {
            access_token: token_response.access_token.clone(),
            refresh_token: token_response.refresh_token.or(Some(refresh_token)),
            expires_at,
        };

        *self.tokens.write().await = Some(new_tokens);

        Ok(token_response.access_token)
    }

    /// Finds or creates the sync folder.
    async fn get_or_create_sync_folder(&self) -> SyncResult<String> {
        // Check cache first
        if let Some(folder_id) = self.sync_folder_id.read().await.as_ref() {
            return Ok(folder_id.clone());
        }

        let access_token = self.get_access_token().await?;
        let folder_parts: Vec<&str> = self.config.base.sync_folder.split('/').collect();

        let mut parent_id = "root".to_string();

        for folder_name in folder_parts {
            if folder_name.is_empty() {
                continue;
            }

            // Search for existing folder
            let query = format!(
                "name = '{}' and mimeType = 'application/vnd.google-apps.folder' and '{}' in parents and trashed = false",
                folder_name, parent_id
            );

            let response = self
                .client
                .get(format!("{}/drive/v3/files", self.config.api_base_url))
                .bearer_auth(&access_token)
                .query(&[("q", query.as_str()), ("fields", "files(id,name)")])
                .send()
                .await
                .map_err(|e| SyncError::Network(format!("folder search failed: {e}")))?;

            let file_list: DriveFileList = response
                .json()
                .await
                .map_err(|e| SyncError::Network(format!("failed to parse folder list: {e}")))?;

            if let Some(folder) = file_list.files.first() {
                parent_id = folder.id.clone();
            } else {
                // Create the folder
                let metadata = serde_json::json!({
                    "name": folder_name,
                    "mimeType": "application/vnd.google-apps.folder",
                    "parents": [parent_id]
                });

                let response = self
                    .client
                    .post(format!("{}/drive/v3/files", self.config.api_base_url))
                    .bearer_auth(&access_token)
                    .json(&metadata)
                    .send()
                    .await
                    .map_err(|e| SyncError::Network(format!("folder creation failed: {e}")))?;

                if !response.status().is_success() {
                    let error = response.text().await.unwrap_or_default();
                    return Err(SyncError::Network(format!(
                        "failed to create folder: {error}"
                    )));
                }

                let created: DriveFile = response.json().await.map_err(|e| {
                    SyncError::Network(format!("failed to parse created folder: {e}"))
                })?;

                info!("Created sync folder: {}", folder_name);
                parent_id = created.id;
            }
        }

        *self.sync_folder_id.write().await = Some(parent_id.clone());
        Ok(parent_id)
    }

    fn drive_file_to_cloud_file(&self, file: DriveFile) -> CloudFile {
        let size = file.size.and_then(|s| s.parse().ok()).unwrap_or(0);
        let modified_at = file
            .modified_time
            .and_then(|t| {
                chrono::DateTime::parse_from_rfc3339(&t)
                    .ok()
                    .map(|dt| UNIX_EPOCH + Duration::from_secs(dt.timestamp() as u64))
            })
            .unwrap_or(SystemTime::now());

        CloudFile {
            id: file.id,
            name: file.name.clone(),
            path: format!("{}/{}", self.config.base.sync_folder, file.name),
            size,
            modified_at,
            content_hash: file.md5_checksum,
        }
    }
}

#[async_trait]
impl CloudStorage for GoogleDriveStorage {
    fn provider_name(&self) -> &'static str {
        "Google Drive"
    }

    fn is_authenticated(&self) -> bool {
        futures::executor::block_on(async { self.tokens.read().await.is_some() })
    }

    async fn authenticate(&mut self) -> SyncResult<Option<String>> {
        if self.is_authenticated() {
            return Ok(None);
        }

        // Return the OAuth URL for user to authenticate
        Ok(Some(self.get_auth_url()))
    }

    async fn complete_auth(&mut self, auth_code: &str) -> SyncResult<()> {
        debug!("Exchanging auth code for tokens");

        let response = self
            .client
            .post(format!("{}/token", self.config.oauth_base_url))
            .form(&[
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("code", &auth_code.to_string()),
                ("redirect_uri", &self.config.redirect_uri),
                ("grant_type", &"authorization_code".to_string()),
            ])
            .send()
            .await
            .map_err(|e| SyncError::Auth(format!("token exchange failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(SyncError::Auth(format!("token exchange failed: {error}")));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| SyncError::Auth(format!("failed to parse token response: {e}")))?;

        let expires_at = token_response
            .expires_in
            .map(|secs| SystemTime::now() + Duration::from_secs(secs.saturating_sub(60)));

        let tokens = OAuthTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
        };

        *self.tokens.write().await = Some(tokens);
        info!("Google Drive authentication successful");

        Ok(())
    }

    async fn list_files(&self) -> SyncResult<Vec<CloudFile>> {
        let access_token = self.get_access_token().await?;
        let folder_id = self.get_or_create_sync_folder().await?;

        let query = format!(
            "'{}' in parents and trashed = false and mimeType != 'application/vnd.google-apps.folder'",
            folder_id
        );

        let mut all_files = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut request = self
                .client
                .get(format!("{}/drive/v3/files", self.config.api_base_url))
                .bearer_auth(&access_token)
                .query(&[
                    ("q", query.as_str()),
                    (
                        "fields",
                        "nextPageToken,files(id,name,size,modifiedTime,md5Checksum,mimeType)",
                    ),
                    ("pageSize", "100"),
                ]);

            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let response = request
                .send()
                .await
                .map_err(|e| SyncError::Network(format!("file list failed: {e}")))?;

            if !response.status().is_success() {
                let error = response.text().await.unwrap_or_default();
                return Err(SyncError::Network(format!("file list failed: {error}")));
            }

            let file_list: DriveFileList = response
                .json()
                .await
                .map_err(|e| SyncError::Network(format!("failed to parse file list: {e}")))?;

            for file in file_list.files {
                all_files.push(self.drive_file_to_cloud_file(file));
            }

            page_token = file_list.next_page_token;
            if page_token.is_none() {
                break;
            }
        }

        Ok(all_files)
    }

    async fn get_changes(&self, cursor: Option<&str>) -> SyncResult<ChangeSet> {
        let access_token = self.get_access_token().await?;

        // If no cursor, get the initial start token
        let start_token = if let Some(c) = cursor {
            c.to_string()
        } else {
            let response = self
                .client
                .get(format!("{}/drive/v3/changes/startPageToken", self.config.api_base_url))
                .bearer_auth(&access_token)
                .send()
                .await
                .map_err(|e| SyncError::Network(format!("get start token failed: {e}")))?;

            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| SyncError::Network(format!("parse start token failed: {e}")))?;

            result["startPageToken"]
                .as_str()
                .ok_or_else(|| SyncError::Network("no start token in response".to_string()))?
                .to_string()
        };

        let mut changed = Vec::new();
        let mut deleted = Vec::new();
        let mut page_token = Some(start_token);
        let mut new_cursor = None;

        while let Some(token) = page_token {
            let response = self
                .client
                .get(format!("{}/drive/v3/changes", self.config.api_base_url))
                .bearer_auth(&access_token)
                .query(&[
                    ("pageToken", token.as_str()),
                    ("fields", "nextPageToken,newStartPageToken,changes(removed,fileId,file(id,name,size,modifiedTime,md5Checksum,mimeType))"),
                    ("pageSize", "100"),
                ])
                .send()
                .await
                .map_err(|e| SyncError::Network(format!("get changes failed: {e}")))?;

            if !response.status().is_success() {
                let error = response.text().await.unwrap_or_default();
                return Err(SyncError::Network(format!("get changes failed: {error}")));
            }

            let changes: DriveChanges = response
                .json()
                .await
                .map_err(|e| SyncError::Network(format!("parse changes failed: {e}")))?;

            for change in changes.changes {
                if change.removed.unwrap_or(false) {
                    deleted.push(change.file_id);
                } else if let Some(file) = change.file {
                    changed.push(self.drive_file_to_cloud_file(file));
                }
            }

            page_token = changes.next_page_token;
            if let Some(cursor) = changes.new_start_page_token {
                new_cursor = Some(cursor);
            }
        }

        Ok(ChangeSet {
            changed,
            deleted,
            next_cursor: new_cursor,
        })
    }

    async fn upload(&self, name: &str, content: &[u8]) -> SyncResult<CloudFile> {
        let access_token = self.get_access_token().await?;
        let folder_id = self.get_or_create_sync_folder().await?;

        debug!("Uploading file: {} ({} bytes)", name, content.len());

        // Use multipart upload for files
        let metadata = serde_json::json!({
            "name": name,
            "parents": [folder_id]
        });

        // Build a proper multipart body that handles binary data correctly.
        // String::from_utf8_lossy would corrupt binary content.
        let boundary = "privstack_boundary_2024";
        let mut body = Vec::new();
        body.extend_from_slice(format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/octet-stream\r\n\r\n"
        ).as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

        let response = self
            .client
            .post(format!("{}/upload/drive/v3/files?uploadType=multipart", self.config.api_base_url))
            .bearer_auth(&access_token)
            .header("Content-Type", format!("multipart/related; boundary={boundary}"))
            .body(body)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("upload failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(SyncError::Network(format!("upload failed: {error}")));
        }

        let file: DriveFile = response
            .json()
            .await
            .map_err(|e| SyncError::Network(format!("parse upload response failed: {e}")))?;

        info!("Uploaded file: {} (id: {})", name, file.id);
        Ok(self.drive_file_to_cloud_file(file))
    }

    async fn download(&self, file_id: &str) -> SyncResult<Vec<u8>> {
        let access_token = self.get_access_token().await?;

        debug!("Downloading file: {}", file_id);

        let response = self
            .client
            .get(format!(
                "{}/drive/v3/files/{}?alt=media",
                self.config.api_base_url, file_id
            ))
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("download failed: {e}")))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(SyncError::Network(format!("download failed: {error}")));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| SyncError::Network(format!("read download body failed: {e}")))?;

        Ok(bytes.to_vec())
    }

    async fn delete(&self, file_id: &str) -> SyncResult<()> {
        let access_token = self.get_access_token().await?;

        debug!("Deleting file: {}", file_id);

        let response = self
            .client
            .delete(format!(
                "{}/drive/v3/files/{}",
                self.config.api_base_url, file_id
            ))
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| SyncError::Network(format!("delete failed: {e}")))?;

        if !response.status().is_success() && response.status().as_u16() != 404 {
            let error = response.text().await.unwrap_or_default();
            return Err(SyncError::Network(format!("delete failed: {error}")));
        }

        info!("Deleted file: {}", file_id);
        Ok(())
    }

    async fn ensure_sync_folder(&self) -> SyncResult<()> {
        self.get_or_create_sync_folder().await?;
        Ok(())
    }
}
