use log::warn;
use std::time::Duration;

/// Configuration for [`retry_with_exponential_backoff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExponentialBackoff {
    /// Number of retries after the first failed attempt.
    pub max_retries: u32,
    /// Wait duration before the first retry.
    pub initial_wait: Duration,
    /// Maximum wait duration between retries.
    pub max_wait: Duration,
}

impl ExponentialBackoff {
    #[must_use]
    pub const fn new(max_retries: u32, initial_wait: Duration, max_wait: Duration) -> Self {
        Self {
            max_retries,
            initial_wait,
            max_wait,
        }
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_wait: Duration::from_secs(1),
            max_wait: Duration::from_secs(30),
        }
    }
}

/// Signals from an operation closure to [`retry_with_exponential_backoff`].
///
/// Return [`RetryError::Retryable`] to allow the retry loop to sleep and try
/// again. Return [`RetryError::Fatal`] to stop immediately and return the
/// error without any further attempts.
pub enum RetryError<E> {
    /// A transient failure — the operation will be retried after a backoff.
    Retryable(E),
    /// A permanent failure — the operation will not be retried.
    Fatal(E),
}

impl<E: std::fmt::Display> std::fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryError::Retryable(e) | RetryError::Fatal(e) => e.fmt(f),
        }
    }
}

impl<E> RetryError<E> {
    /// Unwrap the inner error regardless of variant.
    pub fn into_inner(self) -> E {
        match self {
            RetryError::Retryable(e) | RetryError::Fatal(e) => e,
        }
    }
}

/// Retry an operation with exponential backoff.
///
/// The operation is attempted once immediately. If it fails, the function logs
/// a warning, waits, and retries up to `max_retries` times.
///
/// # Logging
///
/// Each failed retryable attempt logs:
/// - operation name
/// - current attempt and total attempts
/// - error string
/// - wait duration before the next attempt
///
/// When retries are exhausted, a final warning is logged and the last error is
/// returned.
///
/// # Examples
///
/// ```
/// use finance_as_code_utils_resilience::{ExponentialBackoff, retry_with_exponential_backoff};
/// use std::time::Duration;
///
/// let mut attempts = 0;
/// let result = retry_with_exponential_backoff(
///     "download",
///     ExponentialBackoff::new(3, Duration::from_millis(10), Duration::from_millis(40)),
///     || {
///         attempts += 1;
///         if attempts < 3 {
///             Err("temporary error")
///         } else {
///             Ok("done")
///         }
///     },
/// );
///
/// assert_eq!(result, Ok("done"));
/// ```
pub fn retry_with_exponential_backoff<T, E, F>(
    operation_name: &str,
    backoff: ExponentialBackoff,
    operation: F,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Result<T, E>,
{
    retry_with_exponential_backoff_with_sleep(
        operation_name,
        backoff,
        operation,
        std::thread::sleep,
    )
}

fn retry_with_exponential_backoff_with_sleep<T, E, F, S>(
    operation_name: &str,
    backoff: ExponentialBackoff,
    mut operation: F,
    mut sleep: S,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Result<T, E>,
    S: FnMut(Duration),
{
    let total_attempts = backoff.max_retries + 1;
    let mut wait = backoff.initial_wait.min(backoff.max_wait);

    for attempt in 1..=total_attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                if attempt == total_attempts {
                    warn!(
                        "{operation_name} failed (attempt {attempt}/{total_attempts}): {error}; no retries left"
                    );
                    return Err(error);
                }

                warn!(
                    "{operation_name} failed (attempt {attempt}/{total_attempts}): {error}; next retry in {wait:?}"
                );

                sleep(wait);
                wait = wait.saturating_mul(2).min(backoff.max_wait);
            }
        }
    }

    unreachable!("retry loop must always return")
}

/// Like [`retry_with_exponential_backoff`] but the closure returns
/// `Result<T, `[`RetryError<E>`]`>`, allowing it to signal whether a failure
/// is transient (retryable) or permanent (fatal / non-retryable).
///
/// [`RetryError::Retryable`] errors trigger a backoff and a retry.
/// [`RetryError::Fatal`] errors are returned immediately without retrying.
///
/// # Examples
///
/// ```
/// use finance_as_code_utils_resilience::{
///     ExponentialBackoff, RetryError, retry_with_exponential_backoff_selective,
/// };
/// use std::time::Duration;
///
/// let mut attempts = 0;
/// let result: Result<&str, &str> = retry_with_exponential_backoff_selective(
///     "upload",
///     ExponentialBackoff::new(3, Duration::from_millis(10), Duration::from_millis(40)),
///     || {
///         attempts += 1;
///         if attempts == 1 {
///             Err(RetryError::Retryable("transient"))
///         } else if attempts == 2 {
///             Err(RetryError::Fatal("permanent"))
///         } else {
///             Ok("done")
///         }
///     },
/// );
///
/// assert_eq!(result, Err("permanent"));
/// assert_eq!(attempts, 2);
/// ```
pub fn retry_with_exponential_backoff_selective<T, E, F>(
    operation_name: &str,
    backoff: ExponentialBackoff,
    operation: F,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Result<T, RetryError<E>>,
{
    retry_with_exponential_backoff_selective_with_sleep(
        operation_name,
        backoff,
        operation,
        std::thread::sleep,
    )
}

fn retry_with_exponential_backoff_selective_with_sleep<T, E, F, S>(
    operation_name: &str,
    backoff: ExponentialBackoff,
    mut operation: F,
    mut sleep: S,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Result<T, RetryError<E>>,
    S: FnMut(Duration),
{
    let total_attempts = backoff.max_retries + 1;
    let mut wait = backoff.initial_wait.min(backoff.max_wait);

    for attempt in 1..=total_attempts {
        match operation() {
            Ok(value) => return Ok(value),
            Err(RetryError::Fatal(error)) => {
                warn!(
                    "{operation_name} failed (attempt {attempt}/{total_attempts}): {error}; not retrying (fatal)"
                );
                return Err(error);
            }
            Err(RetryError::Retryable(error)) => {
                if attempt == total_attempts {
                    warn!(
                        "{operation_name} failed (attempt {attempt}/{total_attempts}): {error}; no retries left"
                    );
                    return Err(error);
                }

                warn!(
                    "{operation_name} failed (attempt {attempt}/{total_attempts}): {error}; next retry in {wait:?}"
                );

                sleep(wait);
                wait = wait.saturating_mul(2).min(backoff.max_wait);
            }
        }
    }

    unreachable!("retry loop must always return")
}

#[cfg(test)]
mod tests {
    use super::{
        ExponentialBackoff, RetryError, retry_with_exponential_backoff_selective_with_sleep,
        retry_with_exponential_backoff_with_sleep,
    };
    use googletest::prelude::*;
    use std::cell::RefCell;
    use std::time::Duration;

    #[test]
    fn retries_then_returns_success() {
        let waits = RefCell::new(Vec::<Duration>::new());
        let mut attempts = 0;

        let result = retry_with_exponential_backoff_with_sleep(
            "test-op",
            ExponentialBackoff::new(5, Duration::from_millis(10), Duration::from_millis(100)),
            || {
                attempts += 1;
                if attempts < 4 {
                    Err("temporary")
                } else {
                    Ok("ok")
                }
            },
            |duration| waits.borrow_mut().push(duration),
        );

        assert_that!(result.unwrap(), eq("ok"));
        assert_that!(attempts, eq(4));
        let waits = waits.into_inner();
        assert_that!(
            waits.as_slice(),
            eq(&[
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(40),
            ])
        );
    }

    #[test]
    fn backoff_wait_is_capped_by_max_wait() {
        let waits = RefCell::new(Vec::<Duration>::new());
        let mut attempts = 0;

        let result = retry_with_exponential_backoff_with_sleep(
            "test-op",
            ExponentialBackoff::new(3, Duration::from_millis(50), Duration::from_millis(60)),
            || {
                attempts += 1;
                if attempts < 4 {
                    Err("temporary")
                } else {
                    Ok(7_u32)
                }
            },
            |duration| waits.borrow_mut().push(duration),
        );

        assert_that!(result.unwrap(), eq(7));
        let waits = waits.into_inner();
        assert_that!(
            waits.as_slice(),
            eq(&[
                Duration::from_millis(50),
                Duration::from_millis(60),
                Duration::from_millis(60),
            ])
        );
    }

    #[test]
    fn returns_last_error_after_retries_are_exhausted() {
        let waits = RefCell::new(Vec::<Duration>::new());
        let mut attempts = 0;

        let result = retry_with_exponential_backoff_with_sleep(
            "test-op",
            ExponentialBackoff::new(2, Duration::from_millis(5), Duration::from_millis(20)),
            || -> std::result::Result<(), &str> {
                attempts += 1;
                Err("still failing")
            },
            |duration| waits.borrow_mut().push(duration),
        );

        assert_that!(result.unwrap_err(), eq("still failing"));
        assert_that!(attempts, eq(3));
        let waits = waits.into_inner();
        assert_that!(
            waits.as_slice(),
            eq(&[Duration::from_millis(5), Duration::from_millis(10)])
        );
    }

    #[test]
    fn successful_first_attempt_does_not_sleep() {
        let waits = RefCell::new(Vec::<Duration>::new());

        let result = retry_with_exponential_backoff_with_sleep(
            "test-op",
            ExponentialBackoff::new(3, Duration::from_millis(5), Duration::from_millis(20)),
            || Ok::<_, &str>(123_i32),
            |duration| waits.borrow_mut().push(duration),
        );

        assert_that!(result.unwrap(), eq(123));
        assert_that!(waits.into_inner(), is_empty());
    }

    #[test]
    fn selective_fatal_error_stops_immediately_without_retrying() {
        let waits = RefCell::new(Vec::<Duration>::new());
        let mut attempts = 0;

        let result: std::result::Result<&str, &str> =
            retry_with_exponential_backoff_selective_with_sleep(
                "test-op",
                ExponentialBackoff::new(3, Duration::from_millis(5), Duration::from_millis(20)),
                || {
                    attempts += 1;
                    Err(RetryError::Fatal("permanent error"))
                },
                |duration| waits.borrow_mut().push(duration),
            );

        assert_that!(result.unwrap_err(), eq("permanent error"));
        assert_that!(attempts, eq(1));
        assert_that!(waits.into_inner(), is_empty());
    }

    #[test]
    fn selective_retryable_error_retries_then_succeeds() {
        let waits = RefCell::new(Vec::<Duration>::new());
        let mut attempts = 0;

        let result = retry_with_exponential_backoff_selective_with_sleep(
            "test-op",
            ExponentialBackoff::new(3, Duration::from_millis(5), Duration::from_millis(20)),
            || -> std::result::Result<&str, RetryError<&str>> {
                attempts += 1;
                if attempts < 3 {
                    Err(RetryError::Retryable("transient"))
                } else {
                    Ok("done")
                }
            },
            |duration| waits.borrow_mut().push(duration),
        );

        assert_that!(result.unwrap(), eq("done"));
        assert_that!(attempts, eq(3));
        assert_that!(waits.into_inner().len(), eq(2));
    }
}
