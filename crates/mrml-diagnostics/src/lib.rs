//! Span-labeled diagnostics for `mrml`.
//!
//! A thin, dependency-light diagnostics type analogous to `oxc_diagnostics`. A
//! [`Diagnostic`] carries a [`Severity`], a message, zero or more span-anchored
//! [`LabeledSpan`] labels, and optional help text, built fluently.

use std::fmt;

use mrml_span::Span;

/// The severity of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A hard error that prevents successful compilation.
    Error,
    /// A warning that does not prevent compilation.
    Warning,
    /// An advisory note or suggestion.
    Advice,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Advice => "advice",
        };
        f.write_str(text)
    }
}

/// A message anchored to a source [`Span`].
#[derive(Debug, Clone)]
pub struct LabeledSpan {
    /// The source span this label points at.
    pub span: Span,
    /// The message describing the labeled region.
    pub message: String,
}

/// A single diagnostic: a severity, a message, span labels, and optional help.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The severity of this diagnostic.
    pub severity: Severity,
    /// The primary message.
    pub message: String,
    /// Span-anchored labels providing additional context.
    pub labels: Vec<LabeledSpan>,
    /// Optional help text suggesting a fix.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Create an [`Severity::Error`] diagnostic with the given message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            labels: Vec::new(),
            help: None,
        }
    }

    /// Create a [`Severity::Warning`] diagnostic with the given message.
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            labels: Vec::new(),
            help: None,
        }
    }

    /// Create a [`Severity::Advice`] diagnostic with the given message.
    pub fn advice(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Advice,
            message: message.into(),
            labels: Vec::new(),
            help: None,
        }
    }

    /// Attach a span-anchored label to this diagnostic.
    #[must_use]
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(LabeledSpan {
            span,
            message: message.into(),
        });
        self
    }

    /// Attach help text to this diagnostic.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.severity, self.message)?;
        for label in &self.labels {
            write!(
                f,
                "\n  at {}..{}: {}",
                label.span.start, label.span.end, label.message
            )?;
        }
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {}

/// A `Result` whose error is a [`Diagnostic`].
pub type Result<T> = std::result::Result<T, Diagnostic>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_error_with_label_and_help() {
        let diag = Diagnostic::error("unexpected token")
            .with_label(Span::new(4, 9), "here")
            .with_help("expected `>`");

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "unexpected token");
        assert_eq!(diag.labels.len(), 1);
        assert_eq!(diag.labels[0].span, Span::new(4, 9));
        assert_eq!(diag.labels[0].message, "here");
        assert_eq!(diag.help.as_deref(), Some("expected `>`"));
    }

    #[test]
    fn display_contains_message() {
        let diag = Diagnostic::error("unexpected token")
            .with_label(Span::new(4, 9), "here")
            .with_help("expected `>`");
        let rendered = diag.to_string();
        assert!(rendered.contains("unexpected token"));
        assert!(rendered.contains("error"));
        assert!(rendered.contains("here"));
        assert!(rendered.contains("expected `>`"));
    }
}
