//! Runtime resolution of Rithmic WebSocket server URLs.
//!
//! Server URLs are not compiled into the binary. They are resolved at startup
//! via (in priority order):
//!
//! 1. Environment variables (`KAIROS_RITHMIC_SERVER_<KEY>`)
//! 2. Remote fetch from a Cloudflare Worker (`GET /v1/servers`)
//! 3. Local cache file (`{data_dir}/servers.json`)

use std::collections::HashMap;
use std::path::PathBuf;

use super::config::RithmicServer;

/// Default URL for the server configuration API.
const DEFAULT_SERVER_API_URL: &str = "https://kairos.org";

/// Resolves [`RithmicServer`] variants to WebSocket URLs at runtime.
#[derive(Debug, Clone)]
pub struct ServerResolver {
    urls: HashMap<String, String>,
}

impl ServerResolver {
    /// Build a resolver by trying env vars, then remote fetch, then local
    /// cache. Returns an error only if all three sources fail.
    pub async fn initialize(data_dir: PathBuf) -> Result<Self, crate::Error> {
        let mut urls = HashMap::new();

        // 1. Collect any per-server env vars
        for server in RithmicServer::ALL {
            let env_key = format!(
                "KAIROS_RITHMIC_SERVER_{}",
                server.key().to_ascii_uppercase()
            );
            if let Ok(val) = std::env::var(&env_key)
                && !val.is_empty()
            {
                urls.insert(server.key().to_string(), val);
            }
        }

        // If every server is covered by env vars, skip network/cache
        if urls.len() == RithmicServer::ALL.len() {
            log::info!("All {} server URLs provided via env vars", urls.len());
            return Ok(Self { urls });
        }

        // 2. Try remote fetch (10s timeout)
        let cache_path = data_dir.join("servers.json");
        match Self::fetch_remote().await {
            Ok(remote) => {
                // Merge remote into map (env vars take precedence)
                for (k, v) in &remote {
                    urls.entry(k.clone()).or_insert_with(|| v.clone());
                }
                // Persist to cache for offline use
                if let Err(e) = Self::write_cache(&cache_path, &remote).await {
                    log::warn!("Failed to write server cache: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Remote server fetch failed (will try cache): {}", e);
                // 3. Fall back to local cache
                match Self::read_cache(&cache_path).await {
                    Ok(cached) => {
                        for (k, v) in cached {
                            urls.entry(k).or_insert(v);
                        }
                    }
                    Err(e2) => {
                        if urls.is_empty() {
                            return Err(crate::Error::Config(format!(
                                "No server URLs available: \
                                 remote fetch failed ({e}), \
                                 cache read failed ({e2})"
                            )));
                        }
                        log::warn!("Cache read also failed: {}", e2);
                    }
                }
            }
        }

        log::info!(
            "ServerResolver initialized with {} server URL(s)",
            urls.len()
        );
        Ok(Self { urls })
    }

    /// Look up the WebSocket URL for a given server.
    pub fn resolve(&self, server: RithmicServer) -> Result<String, crate::Error> {
        self.urls.get(server.key()).cloned().ok_or_else(|| {
            crate::Error::Config(format!(
                "No URL configured for Rithmic server '{}'. \
                     Set KAIROS_RITHMIC_SERVER_{} or ensure the \
                     server config API is reachable.",
                server.display_name(),
                server.key().to_ascii_uppercase(),
            ))
        })
    }

    // ── private helpers ────────────────────────────────────────────────

    /// Validate that a server URL matches expected patterns.
    /// Only `wss://` URLs pointing to known Rithmic domains are accepted.
    fn is_trusted_server_url(url: &str) -> bool {
        if !url.starts_with("wss://") {
            return false;
        }

        // Extract the host portion after "wss://"
        let host_part = &url["wss://".len()..];
        let host = host_part.split(':').next().unwrap_or(host_part);
        let host = host.split('/').next().unwrap_or(host);

        // Accept known Rithmic infrastructure domains
        host.ends_with(".rithmic.com")
            || host.ends_with(".rithmic.net")
            || host == "rithmic.com"
            || host == "rithmic.net"
    }

    async fn fetch_remote() -> Result<HashMap<String, String>, String> {
        let base_url = std::env::var("KAIROS_SERVER_API_URL")
            .unwrap_or_else(|_| DEFAULT_SERVER_API_URL.to_string());

        let url = format!("{}/v1/servers", base_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("GET {url} failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("GET {url} returned {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON decode error: {e}"))?;

        let servers = body
            .get("servers")
            .and_then(|v| v.as_object())
            .ok_or("Response missing 'servers' object")?;

        let mut map = HashMap::new();
        for (k, v) in servers {
            if let Some(url_str) = v.as_str() {
                if Self::is_trusted_server_url(url_str) {
                    map.insert(k.clone(), url_str.to_string());
                } else {
                    log::warn!("Rejected untrusted server URL for '{}': {}", k, url_str);
                }
            }
        }

        if map.is_empty() {
            return Err("Remote 'servers' object was empty".to_string());
        }

        Ok(map)
    }

    async fn read_cache(path: &std::path::Path) -> Result<HashMap<String, String>, String> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("read {}: {e}", path.display()))?;

        let map: HashMap<String, String> =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))?;

        Ok(map)
    }

    async fn write_cache(
        path: &std::path::Path,
        servers: &HashMap<String, String>,
    ) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }

        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(servers).map_err(|e| format!("serialize: {e}"))?;

        tokio::fs::write(&tmp, json.as_bytes())
            .await
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;

        tokio::fs::rename(&tmp, path)
            .await
            .map_err(|e| format!("rename to {}: {e}", path.display()))?;

        Ok(())
    }
}
