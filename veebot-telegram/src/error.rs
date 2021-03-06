use std::fmt;
use std::time::Duration;

use crate::util::{tracing_err, DynError};
use backtrace::Backtrace;
use regex::Regex;
use teloxide::types::ChatId;
use thiserror::Error;
use tracing::trace;

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

/// Macro to reduce the boilerplate of creating crate-level errors.
/// It directly accepts the body of [`ErrorKind`] variant without type name qualification.
/// It also automatically calls [`Into`] conversion for each passed field.
macro_rules! err_val {
    (@val $variant_ident:ident $field_val:expr) => ($field_val);
    (@val $variant_ident:ident) => ($variant_ident);
    ($variant_path:path $({
        $( $field_ident:ident $(: $field_val:expr)? ),*
        $(,)?
    })?) => {{
        use $variant_path as Variant;

        $crate::error::Error::from(
            Variant $({$(
                $field_ident: ::std::convert::Into::into(
                    $crate::error::err_val!(@val $field_ident $($field_val)?)
                )
            ),*})?
        )
    }};
}

/// Shortcut for defining `map_err` closures that automatically forwards `source`
/// error to the variant.
macro_rules! err_ctx {
    ($variant_path:path $({ $($variant_fields:tt)* })?) => {
        |source| $crate::error::err_val!($variant_path { source, $($($variant_fields)*)? })
    };
}

pub(crate) use err_ctx;
pub(crate) use err_val;

/// Describes any possible error that may happen in the application lifetime.
#[derive(Debug)]
pub struct Error {
    /// Small identifier used for debugging purposes.
    /// It is mentioned in the chat when the error happens.
    /// This way we as developers can copy it and lookup the logs using this id.
    pub(crate) id: String,
    pub(crate) backtrace: Option<Backtrace>,
    pub(crate) kind: ErrorKind,
}

#[derive(Error, Debug)]
pub(crate) enum ErrorKind {
    #[error(transparent)]
    User {
        #[from]
        source: UserError,
    },

    #[error(transparent)]
    Http {
        #[from]
        source: HttpError,
    },

    #[error(transparent)]
    FtAi {
        #[from]
        source: FtAiError,
    },

    #[error(transparent)]
    Tg {
        #[from]
        source: teloxide::RequestError,
    },

    #[error(transparent)]
    Db { source: DbError },
}

impl<T: Into<DbError>> From<T> for ErrorKind {
    fn from(err: T) -> Self {
        Self::Db { source: err.into() }
    }
}

#[derive(Debug, Error)]
pub(crate) enum FtAiError {
    #[error("15.ai returned zero WAV files in the response")]
    MissingWavFile,

    #[error(
        "Failed to create a WAV reader, that is probably a bug, it must be infallible: {message}"
    )]
    CreateWavReader { message: &'static str },

    #[error("Failed to read WAV header returned by 15.ai: {message}")]
    ReadWavHeader { message: &'static str },

    #[error("Failed to read WAV samples returned by 15.ai: {message}")]
    ReadWavSamples { message: &'static str },

    #[error("Failed to encode the resampled WAV to OGG")]
    EncodeWavToOpus { source: ogg_opus::Error },

    #[error("???? ???????????????????? ????????. ?????????????????? ?????? ?????????????????? ???? ?????????? 15.ai, ?????? ???????????????????????? ?????????????????? ????????????")]
    Service { source: Box<Error> },
}

/// Errors caused by interaction with the user.
/// These are most likely caused by humanz sending wrong input.
#[derive(Debug, Error)]
pub(crate) enum UserError {
    #[error("The specified image tags contain a comma (which is prohibited): {input}")]
    CommaInImageTag { input: String },

    // #[error("Invalid regular expression: {input:?}")]
    // InvalidRegex { input: String, source: regex::Error },
    #[error("Requested pattern already exists in the database: {pattern}")]
    BannedPatternAlreadyExists { pattern: Regex },

    #[error("Requested pattern was not found in the database: {pattern}")]
    BannedPatternNotFound { pattern: Regex },

    #[error("Requested chat already exists in the database (chat_id: {chat_id})")]
    ChatAlreadyExists { chat_id: ChatId },

    #[error("Requested chat was not found in the database (chat_id: {chat_id})")]
    ChatNotFound { chat_id: ChatId },

    #[error("?????????? ?????? 15.ai ???? ???????????? ?????????????????? ???????? ?????? ARPAbet ??????????????")]
    FtaiTextContainsNumber,

    #[error(
        "?????????? ?????? 15.ai ???????????? ???????? ???? ?????????? {} ????????????????. ?????????? ???????????????? ????????????: {actual_len}",
        crate::ftai::MAX_TEXT_LENGTH
    )]
    FtaiTextTooLong { actual_len: usize },

    #[error("?????????????? ?????? 15.ai ???????????? ?????????? ???????????????? ?????????????????? ?? ?????????? ?????????? ??????????????: <????????????????>,<??????????>")]
    FtaiInvalidFormat,
}

/// Errors at the layer of the HTTP API
#[derive(Debug, Error)]
pub(crate) enum HttpError {
    #[error("Failed to send an http request")]
    SendRequest { source: reqwest::Error },

    #[error("Failed to read http response")]
    ReadResponse { source: reqwest::Error },

    #[error("HTTP request has failed (http status code: {status}):\n{body}")]
    BadResponseStatusCode {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("Received an unexpected response JSON object")]
    UnexpectedResponseJsonShape { source: serde_json::Error },
}

/// Most likely unrecoverable errors from database communication layer
#[derive(Debug, Error)]
pub(crate) enum DbError {
    #[error("Failed to connect to the database")]
    Connect { source: sqlx::Error },

    #[error("Failed to migrate the database")]
    Migrate { source: sqlx::Error },

    #[error("Database query failed")]
    Query {
        #[from]
        source: sqlx::Error,
    },

    #[error("Duration can't be converted to database representation: {duration:?}")]
    InvalidDuration {
        duration: Duration,
        source: Box<DynError>,
    },
}

impl ErrorKind {
    pub(crate) fn is_user_error(&self) -> bool {
        matches!(self, Self::User { .. })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error (id: {}): {}", self.id, self.kind)?;

        if let Some(backtrace) = &self.backtrace {
            write!(f, "\n{:?}", backtrace)?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.kind.source()
    }
}

impl<T: Into<ErrorKind>> From<T> for Error {
    #[track_caller]
    fn from(kind: T) -> Self {
        let kind: ErrorKind = kind.into();
        // No need for a backtrace if the error is an expected one
        // TODO: add ability to send multiple message to overcome message limit
        // or truncate the backtrace
        let backtrace = if !kind.is_user_error() {
            // We don't use `bool::then` adapter to reduce the backtrace
            None
            // Some(Backtrace::new())
        } else {
            None
        };

        let err = Self {
            kind,
            id: nanoid::nanoid!(6),
            backtrace,
        };

        trace!(err = tracing_err(&err), "Created an error");

        err
    }
}
