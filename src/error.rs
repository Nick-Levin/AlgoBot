use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("API error: {message} (retryable: {retryable})")]
    ApiError { message: String, retryable: bool },

    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimitExceeded { retry_after: std::time::Duration },

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("State desynchronization: {details}")]
    StateDesync { details: String },

    #[error("Insufficient margin: required {required:.2}, available {available:.2}")]
    InsufficientMargin { required: f64, available: f64 },

    #[error("Order rejected: {reason}")]
    OrderRejected { reason: String },

    #[error("Position size below minimum: {size:.6} < {minimum:.6}")]
    PositionSizeTooSmall { size: f64, minimum: f64 },

    #[error("Maximum grid levels ({0}) exceeded")]
    MaxGridLevelsExceeded(u8),

    #[error("Strategy timeout: held for {held_hours} hours, max {max_hours} hours")]
    StrategyTimeout { held_hours: u64, max_hours: u32 },

    #[error("Emergency stop loss triggered: loss {loss_pct:.2}%")]
    EmergencyStopLoss { loss_pct: f64 },

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl BotError {
    /// Returns true if the error is transient and the operation can be retried
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BotError::ApiError { retryable: true, .. }
                | BotError::RateLimitExceeded { .. }
                | BotError::NetworkError(_)
                | BotError::WebSocketError(_)
        )
    }

    /// Returns true if this error should trigger an emergency shutdown
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            BotError::StateDesync { .. }
                | BotError::EmergencyStopLoss { .. }
                | BotError::AuthenticationError(_)
        )
    }

    /// Returns the severity level of the error
    pub fn severity(&self) -> tracing::Level {
        match self {
            BotError::EmergencyStopLoss { .. } => tracing::Level::ERROR,
            BotError::StateDesync { .. } => tracing::Level::ERROR,
            BotError::InsufficientMargin { .. } => tracing::Level::WARN,
            BotError::MaxGridLevelsExceeded(_) => tracing::Level::WARN,
            BotError::StrategyTimeout { .. } => tracing::Level::INFO,
            BotError::ApiError { retryable: false, .. } => tracing::Level::ERROR,
            BotError::ApiError { retryable: true, .. } => tracing::Level::WARN,
            BotError::OrderRejected { .. } => tracing::Level::WARN,
            BotError::PositionSizeTooSmall { .. } => tracing::Level::ERROR,
            _ => tracing::Level::DEBUG,
        }
    }
}

/// Result type alias for the bot
pub type BotResult<T> = Result<T, BotError>;

/// Convert anyhow::Error to BotError
impl From<anyhow::Error> for BotError {
    fn from(err: anyhow::Error) -> Self {
        BotError::Unknown(err.to_string())
    }
}

/// Helper to create retryable API errors
pub fn retryable_api_error(message: impl Into<String>) -> BotError {
    BotError::ApiError {
        message: message.into(),
        retryable: true,
    }
}

/// Helper to create non-retryable API errors
pub fn fatal_api_error(message: impl Into<String>) -> BotError {
    BotError::ApiError {
        message: message.into(),
        retryable: false,
    }
}
