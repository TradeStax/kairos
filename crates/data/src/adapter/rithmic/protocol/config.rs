//! Rithmic connection configuration and environment selection.
//!
//! [`RithmicConnectionConfig`] holds account credentials and server URLs.
//! Configs can be built from environment variables ([`from_env`](RithmicConnectionConfig::from_env))
//! or programmatically via [`RithmicConnectionConfigBuilder`].

use std::{env, fmt, str::FromStr};

/// Trading environment selector.
///
/// Determines which Rithmic environment to connect to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RithmicEnv {
    #[default]
    Demo,
    Live,
    Test,
}

impl fmt::Display for RithmicEnv {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RithmicEnv::Demo => write!(f, "demo"),
            RithmicEnv::Live => write!(f, "live"),
            RithmicEnv::Test => write!(f, "test"),
        }
    }
}

impl FromStr for RithmicEnv {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "demo" | "development" => Ok(RithmicEnv::Demo),
            "live" | "production" => Ok(RithmicEnv::Live),
            "test" => Ok(RithmicEnv::Test),
            _ => Err(ConfigError::InvalidEnvironment(s.to_string())),
        }
    }
}

/// Configuration error types.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// A required environment variable is missing
    MissingEnvVar(String),
    /// An invalid environment string was provided
    InvalidEnvironment(String),
    /// A configuration value is invalid
    InvalidValue { var: String, reason: String },
    /// A required field is missing when building
    MissingField(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::MissingEnvVar(var) => {
                write!(f, "Missing environment variable: {}", var)
            }
            ConfigError::InvalidEnvironment(env) => {
                write!(f, "Invalid environment: {}", env)
            }
            ConfigError::InvalidValue { var, reason } => {
                write!(f, "Invalid value for {}: {}", var, reason)
            }
            ConfigError::MissingField(field) => {
                write!(f, "Missing required field: {}", field)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Configuration for Rithmic connections.
///
/// Contains both account information (IDs) and connection details
/// (URLs, credentials, environment).
#[derive(Clone, Debug)]
pub struct RithmicConnectionConfig {
    /// Rithmic account identifier
    pub account_id: String,
    /// Futures Commission Merchant identifier
    pub fcm_id: String,
    /// Introducing Broker identifier
    pub ib_id: String,
    /// Primary WebSocket URL
    pub url: String,
    /// Alternate/beta WebSocket URL for failover
    pub beta_url: String,
    /// Login username
    pub user: String,
    /// Login password
    pub password: String,
    /// Rithmic system name (e.g. "Rithmic Paper Trading")
    pub system_name: String,
    /// Target environment
    pub env: RithmicEnv,
}

impl RithmicConnectionConfig {
    /// Creates a configuration by loading values from environment
    /// variables prefixed with `RITHMIC_{DEMO,LIVE,TEST}_`.
    ///
    /// Required vars per environment: `ACCOUNT_ID`, `FCM_ID`,
    /// `IB_ID`, `USER`, `PW`, `URL`, `ALT_URL`.
    pub fn from_env(env: RithmicEnv) -> Result<Self, ConfigError> {
        let (account_id, fcm_id, ib_id, url, beta_url, user, password, system_name) = match &env {
            RithmicEnv::Demo => (
                env::var("RITHMIC_DEMO_ACCOUNT_ID").map_err(|_| {
                    ConfigError::MissingEnvVar("RITHMIC_DEMO_ACCOUNT_ID".to_string())
                })?,
                env::var("RITHMIC_DEMO_FCM_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_FCM_ID".to_string()))?,
                env::var("RITHMIC_DEMO_IB_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_IB_ID".to_string()))?,
                env::var("RITHMIC_DEMO_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_URL".to_string()))?,
                env::var("RITHMIC_DEMO_ALT_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_ALT_URL".to_string()))?,
                env::var("RITHMIC_DEMO_USER")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_USER".to_string()))?,
                env::var("RITHMIC_DEMO_PW")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_DEMO_PW".to_string()))?,
                "Rithmic Paper Trading".to_string(),
            ),
            RithmicEnv::Live => (
                env::var("RITHMIC_LIVE_ACCOUNT_ID").map_err(|_| {
                    ConfigError::MissingEnvVar("RITHMIC_LIVE_ACCOUNT_ID".to_string())
                })?,
                env::var("RITHMIC_LIVE_FCM_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_FCM_ID".to_string()))?,
                env::var("RITHMIC_LIVE_IB_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_IB_ID".to_string()))?,
                env::var("RITHMIC_LIVE_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_URL".to_string()))?,
                env::var("RITHMIC_LIVE_ALT_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_ALT_URL".to_string()))?,
                env::var("RITHMIC_LIVE_USER")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_USER".to_string()))?,
                env::var("RITHMIC_LIVE_PW")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_LIVE_PW".to_string()))?,
                "Rithmic 01".to_string(),
            ),
            RithmicEnv::Test => (
                env::var("RITHMIC_TEST_ACCOUNT_ID").map_err(|_| {
                    ConfigError::MissingEnvVar("RITHMIC_TEST_ACCOUNT_ID".to_string())
                })?,
                env::var("RITHMIC_TEST_FCM_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_FCM_ID".to_string()))?,
                env::var("RITHMIC_TEST_IB_ID")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_IB_ID".to_string()))?,
                env::var("RITHMIC_TEST_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_URL".to_string()))?,
                env::var("RITHMIC_TEST_ALT_URL")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_ALT_URL".to_string()))?,
                env::var("RITHMIC_TEST_USER")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_USER".to_string()))?,
                env::var("RITHMIC_TEST_PW")
                    .map_err(|_| ConfigError::MissingEnvVar("RITHMIC_TEST_PW".to_string()))?,
                "Rithmic Test".to_string(),
            ),
        };

        Ok(Self {
            account_id,
            fcm_id,
            ib_id,
            url,
            beta_url,
            user,
            password,
            system_name,
            env,
        })
    }

    /// Creates a builder for programmatic configuration
    #[must_use]
    pub fn builder(env: RithmicEnv) -> RithmicConnectionConfigBuilder {
        RithmicConnectionConfigBuilder::new(env)
    }
}

/// Builder for constructing a [`RithmicConnectionConfig`] with custom values.
#[derive(Default)]
pub struct RithmicConnectionConfigBuilder {
    env: Option<RithmicEnv>,
    account_id: Option<String>,
    fcm_id: Option<String>,
    ib_id: Option<String>,
    url: Option<String>,
    beta_url: Option<String>,
    user: Option<String>,
    password: Option<String>,
    system_name: Option<String>,
}

impl RithmicConnectionConfigBuilder {
    /// Create a new builder for the specified environment.
    pub fn new(env: RithmicEnv) -> Self {
        // Set system name default based on environment
        let system_name = match &env {
            RithmicEnv::Demo => "Rithmic Paper Trading".to_string(),
            RithmicEnv::Live => "Rithmic 01".to_string(),
            RithmicEnv::Test => "Rithmic Test".to_string(),
        };

        Self {
            env: Some(env),
            system_name: Some(system_name),
            ..Default::default()
        }
    }

    /// Set the account ID.
    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Set the FCM ID.
    pub fn fcm_id(mut self, fcm_id: impl Into<String>) -> Self {
        self.fcm_id = Some(fcm_id.into());
        self
    }

    /// Set the IB ID.
    pub fn ib_id(mut self, ib_id: impl Into<String>) -> Self {
        self.ib_id = Some(ib_id.into());
        self
    }

    /// Set the WebSocket URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set the beta WebSocket URL.
    pub fn beta_url(mut self, beta_url: impl Into<String>) -> Self {
        self.beta_url = Some(beta_url.into());
        self
    }

    /// Set the username.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Set the system name.
    pub fn system_name(mut self, system_name: impl Into<String>) -> Self {
        self.system_name = Some(system_name.into());
        self
    }

    /// Builds the configuration, returning an error if any required
    /// fields are missing
    pub fn build(self) -> Result<RithmicConnectionConfig, ConfigError> {
        Ok(RithmicConnectionConfig {
            env: self
                .env
                .ok_or_else(|| ConfigError::MissingField("env".to_string()))?,
            account_id: self
                .account_id
                .ok_or_else(|| ConfigError::MissingField("account_id".to_string()))?,
            fcm_id: self
                .fcm_id
                .ok_or_else(|| ConfigError::MissingField("fcm_id".to_string()))?,
            ib_id: self
                .ib_id
                .ok_or_else(|| ConfigError::MissingField("ib_id".to_string()))?,
            url: self
                .url
                .ok_or_else(|| ConfigError::MissingField("url".to_string()))?,
            beta_url: self
                .beta_url
                .ok_or_else(|| ConfigError::MissingField("beta_url".to_string()))?,
            user: self
                .user
                .ok_or_else(|| ConfigError::MissingField("user".to_string()))?,
            password: self
                .password
                .ok_or_else(|| ConfigError::MissingField("password".to_string()))?,
            system_name: self
                .system_name
                .ok_or_else(|| ConfigError::MissingField("system_name".to_string()))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rithmic_env_display() {
        assert_eq!(RithmicEnv::Demo.to_string(), "demo");
        assert_eq!(RithmicEnv::Live.to_string(), "live");
        assert_eq!(RithmicEnv::Test.to_string(), "test");
    }

    #[test]
    fn test_rithmic_env_from_str() {
        assert_eq!("demo".parse::<RithmicEnv>().unwrap(), RithmicEnv::Demo);
        assert_eq!(
            "development".parse::<RithmicEnv>().unwrap(),
            RithmicEnv::Demo
        );
        assert_eq!("live".parse::<RithmicEnv>().unwrap(), RithmicEnv::Live);
        assert_eq!(
            "production".parse::<RithmicEnv>().unwrap(),
            RithmicEnv::Live
        );
        assert_eq!("test".parse::<RithmicEnv>().unwrap(), RithmicEnv::Test);

        // Test invalid input
        let result = "invalid".parse::<RithmicEnv>();
        assert!(result.is_err());
        if let Err(ConfigError::InvalidEnvironment(env)) = result {
            assert_eq!(env, "invalid");
        } else {
            panic!("Expected InvalidEnvironment error");
        }
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::MissingEnvVar("TEST_VAR".to_string());
        assert_eq!(err.to_string(), "Missing environment variable: TEST_VAR");

        let err = ConfigError::InvalidEnvironment("bad_env".to_string());
        assert_eq!(err.to_string(), "Invalid environment: bad_env");

        let err = ConfigError::InvalidValue {
            var: "TEST".to_string(),
            reason: "too short".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid value for TEST: too short");

        let err = ConfigError::MissingField("account_id".to_string());
        assert_eq!(err.to_string(), "Missing required field: account_id");
    }

    #[test]
    fn test_builder_complete() {
        let config = RithmicConnectionConfig::builder(RithmicEnv::Demo)
            .account_id("my_account")
            .fcm_id("my_fcm")
            .ib_id("my_ib")
            .user("my_user")
            .password("my_password")
            .url("wss://test.example.com:443")
            .beta_url("wss://test-alt.example.com:443")
            .build()
            .unwrap();

        assert_eq!(config.account_id, "my_account");
        assert_eq!(config.fcm_id, "my_fcm");
        assert_eq!(config.ib_id, "my_ib");
        assert_eq!(config.user, "my_user");
        assert_eq!(config.password, "my_password");
        assert_eq!(config.env, RithmicEnv::Demo);
        assert_eq!(config.url, "wss://test.example.com:443");
        assert_eq!(config.beta_url, "wss://test-alt.example.com:443");
        // Builder should set system_name default
        assert_eq!(config.system_name, "Rithmic Paper Trading");
    }

    #[test]
    fn test_builder_custom_urls() {
        let config = RithmicConnectionConfig::builder(RithmicEnv::Demo)
            .account_id("my_account")
            .fcm_id("my_fcm")
            .ib_id("my_ib")
            .user("my_user")
            .password("my_password")
            .url("wss://custom.example.com:443")
            .beta_url("wss://custom-beta.example.com:443")
            .system_name("Custom System")
            .build()
            .unwrap();

        assert_eq!(config.url, "wss://custom.example.com:443");
        assert_eq!(config.beta_url, "wss://custom-beta.example.com:443");
        assert_eq!(config.system_name, "Custom System");
    }

    #[test]
    fn test_builder_missing_account_id() {
        let result = RithmicConnectionConfig::builder(RithmicEnv::Demo)
            .fcm_id("my_fcm")
            .ib_id("my_ib")
            .user("my_user")
            .password("my_password")
            .build();

        assert!(result.is_err());
        if let Err(ConfigError::MissingField(field)) = result {
            assert_eq!(field, "account_id");
        } else {
            panic!("Expected MissingField error");
        }
    }

    #[test]
    fn test_builder_missing_user() {
        let result = RithmicConnectionConfig::builder(RithmicEnv::Demo)
            .account_id("my_account")
            .fcm_id("my_fcm")
            .ib_id("my_ib")
            .password("my_password")
            .url("wss://test.example.com:443")
            .beta_url("wss://test-alt.example.com:443")
            .build();

        assert!(result.is_err());
        if let Err(ConfigError::MissingField(field)) = result {
            assert_eq!(field, "user");
        } else {
            panic!("Expected MissingField error");
        }
    }

    #[test]
    fn test_builder_demo_defaults() {
        let builder = RithmicConnectionConfigBuilder::new(RithmicEnv::Demo);
        let config = builder
            .account_id("test")
            .fcm_id("test")
            .ib_id("test")
            .user("test")
            .password("test")
            .url("wss://test.example.com:443")
            .beta_url("wss://test-alt.example.com:443")
            .build()
            .unwrap();

        // Builder should set system_name default
        assert_eq!(config.system_name, "Rithmic Paper Trading");
    }

    #[test]
    fn test_builder_live_defaults() {
        let builder = RithmicConnectionConfigBuilder::new(RithmicEnv::Live);
        let config = builder
            .account_id("test")
            .fcm_id("test")
            .ib_id("test")
            .user("test")
            .password("test")
            .url("wss://test.example.com:443")
            .beta_url("wss://test-alt.example.com:443")
            .build()
            .unwrap();

        // Builder should set system_name default
        assert_eq!(config.system_name, "Rithmic 01");
    }

    #[test]
    fn test_builder_test_defaults() {
        let builder = RithmicConnectionConfigBuilder::new(RithmicEnv::Test);
        let config = builder
            .account_id("test")
            .fcm_id("test")
            .ib_id("test")
            .user("test")
            .password("test")
            .url("wss://test.example.com:443")
            .beta_url("wss://test-alt.example.com:443")
            .build()
            .unwrap();

        // Builder should set system_name default
        assert_eq!(config.system_name, "Rithmic Test");
    }

    #[test]
    fn test_builder_into_string_conversions() {
        // Test that Into<String> works for builder methods
        let config = RithmicConnectionConfig::builder(RithmicEnv::Demo)
            .account_id(String::from("my_account"))
            .fcm_id(String::from("my_fcm"))
            .ib_id(String::from("my_ib"))
            .user(String::from("my_user"))
            .password(String::from("my_password"))
            .url(String::from("wss://test.example.com:443"))
            .beta_url(String::from("wss://test-alt.example.com:443"))
            .build()
            .unwrap();

        assert_eq!(config.account_id, "my_account");
    }
}
