//! Circuit Breaker Pattern Implementation
//!
//! Prevents wasted CPU/network resources when S3 is unavailable by rejecting
//! calls after a configurable number of consecutive failures.
//!
//! ## State Machine
//! - **Closed** → Normal operation, all calls pass through, failures counted
//! - **Open** → All calls rejected immediately with `CircuitOpenError`, waits for timeout
//! - **HalfOpen** → Single test call allowed after timeout expires
//!
//! ## Transitions
//! - Closed → Open: after `failure_threshold` (5) consecutive failures
//! - Open → HalfOpen: after `timeout` (60s) expires
//! - HalfOpen → Closed: after 1 successful call
//! - HalfOpen → Open: after any failure

use std::time::{Duration, Instant};

use crate::shared::errors::InfrastructureError;

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — all calls pass through, failures counted.
    Closed,
    /// Failing — reject all calls immediately.
    Open,
    /// Testing — allow single call to check if service recovered.
    HalfOpen,
}

/// Circuit breaker that wraps fallible operations.
///
/// Prevents retry storms by opening the circuit after consecutive failures
/// and only allowing a test call after a cooldown period.
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    #[allow(dead_code)]
    success_count: u32,
    open_until: Option<Instant>,
    timeout: Duration,
}

impl CircuitBreaker {
    /// Create a new circuit breaker in Closed state.
    ///
    /// # Defaults
    /// - `failure_threshold`: 5 consecutive failures to open circuit
    /// - `timeout`: 60 seconds before transitioning Open → HalfOpen
    pub fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold: 5,
            success_count: 0,
            open_until: None,
            timeout: Duration::from_secs(60),
        }
    }

    /// Execute a fallible operation through the circuit breaker.
    ///
    /// # Behavior by State
    /// - **Closed**: Execute `f`, call `on_success` or `on_failure` based on result
    /// - **Open**: If timeout expired, transition to HalfOpen and execute `f`;
    ///   otherwise return `CircuitOpenError` immediately
    /// - **HalfOpen**: Execute `f`, transition to Closed on success or Open on failure
    ///
    /// # Errors
    /// Returns `InfrastructureError::CircuitOpenError` when the circuit is open
    /// and the timeout has not yet expired.
    pub fn call<F, T>(&mut self, f: F) -> Result<T, InfrastructureError>
    where
        F: FnOnce() -> Result<T, InfrastructureError>,
    {
        if !self.should_attempt() {
            return Err(InfrastructureError::CircuitOpenError);
        }

        // Transition Open → HalfOpen before attempting the call
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
        }

        let result = f();

        match &result {
            Ok(_) => self.on_success(),
            Err(_) => self.on_failure(),
        }

        result
    }

    /// Record a successful operation.
    ///
    /// Resets failure count to 0. Transitions:
    /// - HalfOpen → Closed (service recovered)
    /// - Closed → Closed (reset failure streak)
    pub fn on_success(&mut self) {
        self.failure_count = 0;
        if self.state == CircuitState::HalfOpen {
            self.state = CircuitState::Closed;
            self.open_until = None;
        }
    }

    /// Record a failed operation.
    ///
    /// Increments failure count. Transitions:
    /// - Closed → Open: if `failure_count >= failure_threshold`
    /// - HalfOpen → Open: any failure
    pub fn on_failure(&mut self) {
        self.failure_count += 1;

        match self.state {
            CircuitState::HalfOpen => {
                // Any failure in HalfOpen immediately re-opens the circuit
                self.state = CircuitState::Open;
                self.open_until = Some(Instant::now() + self.timeout);
            }
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    self.open_until = Some(Instant::now() + self.timeout);
                }
            }
            CircuitState::Open => {
                // Should not reach here via call(), but handle defensively
                self.open_until = Some(Instant::now() + self.timeout);
            }
        }
    }

    /// Check if a call should be attempted.
    ///
    /// Returns `true` when:
    /// - State is Closed (always attempt)
    /// - State is HalfOpen (single test call)
    /// - State is Open AND timeout has expired (transition to HalfOpen)
    ///
    /// Returns `false` when:
    /// - State is Open AND timeout has NOT expired
    fn should_attempt(&self) -> bool {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                if let Some(until) = self.open_until {
                    Instant::now() >= until
                } else {
                    true
                }
            }
        }
    }

    /// Return the current circuit state.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Return the current failure count.
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Set the failure threshold (for testing).
    #[cfg(test)]
    fn with_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set the timeout duration (for testing).
    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_successful_calls_reset_failure_count() {
        let mut cb = CircuitBreaker::new();

        // Simulate 3 failures
        for _ in 0..3 {
            let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
        }
        assert_eq!(cb.failure_count(), 3);

        // Success resets
        let _ = cb.call(|| Ok::<_, InfrastructureError>(()));
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_opens_after_threshold_failures() {
        let mut cb = CircuitBreaker::new().with_threshold(3);

        for _ in 0..3 {
            let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
        }

        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_returns_circuit_open_error_when_open() {
        let mut cb = CircuitBreaker::new().with_threshold(2);

        // Trigger open
        for _ in 0..2 {
            let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
        }

        assert_eq!(cb.state(), CircuitState::Open);

        // Next call should fail immediately
        let result = cb.call(|| Ok::<_, InfrastructureError>(()));
        assert!(matches!(result, Err(InfrastructureError::CircuitOpenError)));
    }

    #[test]
    fn test_circuit_transitions_open_to_halfopen_after_timeout() {
        let mut cb = CircuitBreaker::new()
            .with_threshold(2)
            .with_timeout(Duration::from_millis(50));

        // Open the circuit
        for _ in 0..2 {
            let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(60));

        // should_attempt should now return true (Open → HalfOpen transition)
        assert!(cb.should_attempt());

        // Make a successful call — should transition to Closed
        let result = cb.call(|| Ok::<_, InfrastructureError>(()));
        assert!(result.is_ok());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_transitions_halfopen_to_open_on_failure() {
        let mut cb = CircuitBreaker::new()
            .with_threshold(2)
            .with_timeout(Duration::from_millis(50));

        // Open the circuit
        for _ in 0..2 {
            let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
        }
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(60));

        // Failure in HalfOpen should re-open
        let result = cb.call(|| {
            Err::<(), InfrastructureError>(InfrastructureError::NetworkError("fail again".into()))
        });
        assert!(result.is_err());
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_five_consecutive_failures_open_circuit_default() {
        let mut cb = CircuitBreaker::new(); // default threshold = 5

        for i in 0..5 {
            let result =
                cb.call(|| Err::<(), _>(InfrastructureError::NetworkError(format!("fail {}", i))));
            if i < 4 {
                assert!(result.is_err());
                assert_eq!(cb.state(), CircuitState::Closed);
            } else {
                // 5th failure should open the circuit
                assert!(result.is_err());
                assert_eq!(cb.state(), CircuitState::Open);
            }
        }
    }
}
