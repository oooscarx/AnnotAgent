use std::sync::{Arc, Mutex};

use annotagent_core::RunStatus;
use thiserror::Error;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ControlError {
    #[error("run state lock poisoned")]
    Poisoned,
    #[error("illegal run status transition: {from:?} -> {to:?}")]
    IllegalTransition { from: RunStatus, to: RunStatus },
}

#[derive(Clone)]
pub struct RunControl {
    status: Arc<Mutex<RunStatus>>,
    cancellation: CancellationToken,
    resume_signal: Arc<Notify>,
}

impl RunControl {
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(RunStatus::Pending)),
            cancellation: CancellationToken::new(),
            resume_signal: Arc::new(Notify::new()),
        }
    }

    pub fn status(&self) -> Result<RunStatus, ControlError> {
        self.status
            .lock()
            .map(|status| *status)
            .map_err(|_| ControlError::Poisoned)
    }

    pub fn transition(&self, next: RunStatus) -> Result<RunStatus, ControlError> {
        let mut status = self.status.lock().map_err(|_| ControlError::Poisoned)?;
        let previous = *status;
        if !previous.can_transition_to(next) {
            return Err(ControlError::IllegalTransition {
                from: previous,
                to: next,
            });
        }
        *status = next;
        if next == RunStatus::Running {
            self.resume_signal.notify_waiters();
        }
        if next == RunStatus::Cancelled {
            self.cancellation.cancel();
            self.resume_signal.notify_waiters();
        }
        Ok(previous)
    }

    pub fn pause(&self) -> Result<RunStatus, ControlError> {
        self.transition(RunStatus::Paused)
    }

    pub fn resume(&self) -> Result<RunStatus, ControlError> {
        self.transition(RunStatus::Running)
    }

    pub fn cancel(&self) -> Result<RunStatus, ControlError> {
        self.transition(RunStatus::Cancelled)
    }

    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub async fn wait_until_runnable(&self) -> Result<(), ControlError> {
        loop {
            match self.status()? {
                RunStatus::Paused => {
                    tokio::select! {
                        () = self.resume_signal.notified() => {}
                        () = self.cancellation.cancelled() => return Ok(()),
                    }
                }
                _ => return Ok(()),
            }
        }
    }
}

impl Default for RunControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_and_cancel_are_distinct() {
        let control = RunControl::new();
        control.transition(RunStatus::Running).expect("start");
        control.pause().expect("pause");
        assert!(!control.cancellation_token().is_cancelled());
        control.resume().expect("resume");
        control.cancel().expect("cancel");
        assert!(control.cancellation_token().is_cancelled());
    }
}
