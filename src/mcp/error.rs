//! Error handling for [`rmcp`].
use rmcp::model::ErrorData as McpError;

/// Convert errors into [`McpError`].
pub(crate) trait ErrorExt {
    /// Treat errors as [internal error](McpError::internal_error).
    fn internal(self) -> McpError;
}

impl<E> ErrorExt for E
where
    E: std::fmt::Display,
{
    fn internal(self) -> McpError {
        McpError::internal_error(self.to_string(), None)
    }
}

/// Convert errors within [`Result`]s into [`McpError`]s.
pub(crate) trait ResultExt {
    type T;

    /// Treat errors as [internal error](McpError::internal_error).
    fn internal(self) -> Result<Self::T, McpError>;
}

impl<T, E> ResultExt for Result<T, E>
where
    E: std::fmt::Display,
{
    type T = T;

    fn internal(self) -> Result<Self::T, McpError> {
        self.map_err(|e| e.internal())
    }
}

/// Convert [`Option`] into [`Result`] containing a [`McpError`].
pub(crate) trait OptionExt {
    type T;

    /// A value was expected, treat [`None`] as [not found](McpError::resource_not_found).
    fn not_found(self, what: String) -> Result<Self::T, McpError>;

    /// A value is required.
    fn required(self, what: String) -> Result<Self::T, McpError>;
}

impl<T> OptionExt for Option<T> {
    type T = T;

    fn not_found(self, what: String) -> Result<Self::T, McpError> {
        self.ok_or_else(|| McpError::resource_not_found(what, None))
    }

    fn required(self, what: String) -> Result<Self::T, McpError> {
        self.ok_or_else(|| McpError::invalid_params(format!("{what} is required"), None))
    }
}
