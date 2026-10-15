//! Cycle de vie d'une exécution d'agent.
//!
//! ```text
//! Prepare → Running → Proposing → Review → Applying → Done
//!                 ↘ Suspended ↗        ↘ Discarded
//!   (Proposing → Applying directement si autonomie = auto ; Running → Failed sur erreur)
//! ```

use crate::domain::agent::{Autonomy, RunState};

impl RunState {
    pub fn can_transition_to(self, next: RunState) -> bool {
        use RunState::*;
        matches!(
            (self, next),
            (Prepare, Running)
                | (Running, Proposing)
                | (Running, Suspended)
                | (Suspended, Running)
                | (Running, Failed)
                | (Proposing, Review)
                | (Proposing, Applying)
                | (Review, Applying)
                | (Review, Discarded)
                | (Applying, Done)
        ) || (matches!(next, Failed | Discarded) && !self.is_terminal())
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, RunState::Done | RunState::Discarded | RunState::Failed)
    }

    /// État qui suit `Proposing` selon le niveau d'autonomie.
    pub fn after_proposing(autonomy: Autonomy) -> RunState {
        match autonomy {
            Autonomy::Auto => RunState::Applying,
            Autonomy::Propose | Autonomy::ApplyAfterReview => RunState::Review,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RunState::*;

    #[test]
    fn follows_the_documented_cycle() {
        let path = [Prepare, Running, Proposing, Review, Applying, Done];
        assert!(path.windows(2).all(|w| w[0].can_transition_to(w[1])));
    }

    #[test]
    fn rejects_shortcuts_and_exits_from_terminal_states() {
        assert!(!Running.can_transition_to(Done));
        assert!(!Review.can_transition_to(Running));
        assert!(!Done.can_transition_to(Failed));
        assert!(Applying.can_transition_to(Failed));
    }

    #[test]
    fn autonomy_decides_review() {
        assert_eq!(RunState::after_proposing(Autonomy::Auto), Applying);
        assert_eq!(RunState::after_proposing(Autonomy::ApplyAfterReview), Review);
    }
}
