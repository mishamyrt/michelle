//! One compaction before the assumed 30-minute prompt cache expires.

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvError, RecvTimeoutError};

use super::CommandMessage;

const CACHE_LIFETIME: Duration = Duration::from_secs(30 * 60);
const COMPACTION_LEAD_TIME: Duration = Duration::from_secs(60);
const CONTEXT_TOKEN_THRESHOLD: u64 = 200_000;

#[derive(Default)]
pub(super) struct IdleCompaction {
    deadline: Option<Instant>,
    compacting: bool,
}

impl IdleCompaction {
    pub(super) fn recv(
        &self,
        commands: &Receiver<CommandMessage>,
    ) -> Result<CommandMessage, RecvError> {
        match self.deadline {
            Some(deadline) => match commands.recv_deadline(deadline) {
                Ok(command) => Ok(command),
                Err(RecvTimeoutError::Timeout) => Ok(CommandMessage::Prompt("/compact".to_owned())),
                Err(RecvTimeoutError::Disconnected) => Err(RecvError),
            },
            None => commands.recv(),
        }
    }

    pub(super) fn prompt(&mut self, compacting: bool) {
        self.deadline = None;
        self.compacting = compacting;
    }

    pub(super) fn turn_started(&mut self) {
        self.deadline = None;
    }

    pub(super) fn turn_finished(
        &mut self,
        success: bool,
        context_tokens: Option<u64>,
        now: Instant,
    ) {
        // Compaction must not keep an abandoned session warm indefinitely.
        self.deadline = (success
            && !self.compacting
            && context_tokens.is_some_and(|tokens| tokens > CONTEXT_TOKEN_THRESHOLD))
        .then_some(now + CACHE_LIFETIME - COMPACTION_LEAD_TIME);
        self.compacting = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn waits_29_minutes_after_a_successful_turn_and_cancels_on_new_work() {
        let now = Instant::now();
        let mut idle = IdleCompaction::default();
        assert!(idle.deadline.is_none());
        idle.turn_finished(true, Some(200_001), now);
        assert_eq!(idle.deadline, Some(now + Duration::from_secs(29 * 60)));

        idle.prompt(false);
        assert!(idle.deadline.is_none());
        idle.turn_finished(true, Some(200_001), now + Duration::from_secs(60));
        assert_eq!(idle.deadline, Some(now + CACHE_LIFETIME));

        idle.turn_started();
        assert!(idle.deadline.is_none());
        idle.turn_finished(false, Some(200_001), now);
        assert!(idle.deadline.is_none());
    }

    #[test]
    fn compacts_once_and_service_commands_do_not_reset_the_deadline() {
        let mut idle = IdleCompaction::default();
        idle.turn_finished(true, Some(200_001), Instant::now() - CACHE_LIFETIME);
        let deadline = idle.deadline;
        let (commands, received) = unbounded();
        commands
            .send(CommandMessage::RefreshBackgroundWork)
            .unwrap();
        assert!(matches!(
            idle.recv(&received),
            Ok(CommandMessage::RefreshBackgroundWork)
        ));
        assert_eq!(idle.deadline, deadline);
        assert!(
            matches!(idle.recv(&received), Ok(CommandMessage::Prompt(text)) if text == "/compact")
        );

        idle.prompt(true);
        idle.turn_started();
        idle.turn_finished(true, Some(200_001), Instant::now());
        assert!(idle.deadline.is_none());
        drop(commands);
        assert!(idle.recv(&received).is_err());
    }

    #[test]
    fn a_queued_prompt_wins_over_an_expired_timer() {
        let mut idle = IdleCompaction::default();
        idle.turn_finished(true, Some(200_001), Instant::now() - CACHE_LIFETIME);
        let (commands, received) = unbounded();
        commands
            .send(CommandMessage::Prompt("Continue".to_owned()))
            .unwrap();
        assert!(
            matches!(idle.recv(&received), Ok(CommandMessage::Prompt(text)) if text == "Continue")
        );
        idle.prompt(false);
        assert!(idle.deadline.is_none());
    }

    #[test]
    fn requires_context_above_200_000_tokens() {
        let now = Instant::now();
        let mut idle = IdleCompaction::default();
        for tokens in [None, Some(0), Some(199_999), Some(200_000), Some(200_001)] {
            idle.turn_finished(true, tokens, now);
            assert_eq!(idle.deadline.is_some(), tokens == Some(200_001));
        }
        idle.turn_finished(true, Some(100_000), now);
        assert!(idle.deadline.is_none());
    }
}
