//! Error handling for [`rmcp`].
use rmcp::model::ErrorData as McpError;

/// Convert errors into [`McpError`].
pub(crate) trait ErrorExt {
    /// Treat errors as [internal error](McpError::internal_error).
    fn internal(self) -> McpError;
}

impl<E> ErrorExt for E
where
    E: AsRef<dyn std::error::Error>,
{
    fn internal(self) -> McpError {
        McpError::internal_error(format_error_chain(self.as_ref()), None)
    }
}

fn format_error_chain(e: &dyn std::error::Error) -> String {
    let mut maybe_e = Some(e);
    let mut s = String::new();
    while let Some(e) = maybe_e {
        let e_str = e.to_string();

        // only append error display if it wasn't already included by the parent
        if !s.ends_with(&e_str) {
            if !s.is_empty() {
                s.push_str(": ");
            }
            s.push_str(&e_str);
        }

        maybe_e = e.source();
    }
    s
}

/// Convert errors within [`Result`]s into [`McpError`]s.
pub(crate) trait ResultExt {
    type T;

    /// Treat errors as [internal error](McpError::internal_error).
    fn internal(self) -> Result<Self::T, McpError>;
}

impl<T, E> ResultExt for Result<T, E>
where
    E: AsRef<dyn std::error::Error>,
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format_error_chain() {
        assert_eq!(format_error_chain(&TextError::new("foo")), "foo");

        assert_eq!(
            format_error_chain(&TextError::new("foo").with_source(TextError::new("bar"))),
            "foo: bar",
        );

        let e = TextError::new("foo")
            .display_source()
            .with_source(TextError::new("bar"));
        assert_eq!(e.to_string(), "foo: bar");
        assert_eq!(format_error_chain(&e), "foo: bar");

        let e = TextError::new("foo").display_source().with_source(
            TextError::new("bar")
                .display_source()
                .with_source(TextError::new("baz").with_source(TextError::new("end"))),
        );
        assert_eq!(e.to_string(), "foo: bar: baz");
        assert_eq!(format_error_chain(&e), "foo: bar: baz: end");
    }

    #[derive(Debug)]
    struct TextError {
        msg: &'static str,
        source: Option<Box<dyn std::error::Error>>,
        display_source: bool,
    }

    impl TextError {
        fn new(msg: &'static str) -> Self {
            Self {
                msg,
                source: None,
                display_source: false,
            }
        }

        fn with_source<E>(mut self, e: E) -> Self
        where
            E: std::error::Error + 'static,
        {
            assert!(self.source.is_none());
            self.source = Some(Box::new(e));
            self
        }

        fn display_source(mut self) -> Self {
            self.display_source = true;
            self
        }
    }

    impl std::fmt::Display for TextError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.msg)?;
            if self.display_source {
                if let Some(source) = self.source.as_ref() {
                    write!(f, ": {source}")?;
                }
            }
            Ok(())
        }
    }

    impl std::error::Error for TextError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.source.as_deref()
        }
    }
}
