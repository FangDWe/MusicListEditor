use serde::Serialize;

/// 统一错误类型：序列化后即为前端可读的 `{ code, message }`（需求文档 §6 错误码）
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn io(context: &str, error: std::io::Error) -> Self {
        Self::new("E5000", format!("{context}：{error}"))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new("E5000", error.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

