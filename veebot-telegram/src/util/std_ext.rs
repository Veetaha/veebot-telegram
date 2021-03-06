use crate::util;
use easy_ext::ext;
use tracing::error;

#[ext(OptionExt)]
pub(crate) impl<T> Option<T> {
    #[track_caller]
    fn unwrapx(self) -> T {
        match self {
            Some(x) => x,
            None => {
                error!("The application is crashing...");
                panic!("unwrap called on None");
            }
        }
    }
}

#[ext(ResultExt)]
pub(crate) impl<T, E> Result<T, E>
where
    E: std::error::Error + 'static,
{
    #[track_caller]
    fn unwrapx(self) -> T {
        match self {
            Ok(x) => x,
            Err(err) => {
                error!(
                    err = util::tracing_err(&err),
                    "The application is crashing..."
                );
                panic!("unwrap called on None");
            }
        }
    }
}
