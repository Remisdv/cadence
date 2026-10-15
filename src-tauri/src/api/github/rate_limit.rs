//! Régimes de synchronisation selon `x-ratelimit-remaining` (5 000 requêtes/h par jeton).

use std::time::Duration;

use crate::domain::sync::RateLimitMode;

impl RateLimitMode {
    pub fn from_remaining(remaining: u32) -> Self {
        match remaining {
            r if r > 1_000 => RateLimitMode::Normal,
            r if r >= 200 => RateLimitMode::Econome,
            _ => RateLimitMode::Gel,
        }
    }

    /// Intervalle de sync incrémentale ; `None` = lecture du cache seule.
    pub fn sync_interval(self) -> Option<Duration> {
        match self {
            RateLimitMode::Normal => Some(Duration::from_secs(60)),
            RateLimitMode::Econome => Some(Duration::from_secs(5 * 60)),
            RateLimitMode::Gel => None,
        }
    }

    /// En gel, les agents sont suspendus (pas échoués) et reprennent à la réinitialisation.
    pub fn agents_allowed(self) -> bool {
        self != RateLimitMode::Gel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_the_plan() {
        assert_eq!(RateLimitMode::from_remaining(5_000), RateLimitMode::Normal);
        assert_eq!(RateLimitMode::from_remaining(1_001), RateLimitMode::Normal);
        assert_eq!(RateLimitMode::from_remaining(1_000), RateLimitMode::Econome);
        assert_eq!(RateLimitMode::from_remaining(200), RateLimitMode::Econome);
        assert_eq!(RateLimitMode::from_remaining(199), RateLimitMode::Gel);
    }

    #[test]
    fn freeze_stops_sync_and_agents() {
        assert_eq!(RateLimitMode::Gel.sync_interval(), None);
        assert!(!RateLimitMode::Gel.agents_allowed());
        assert_eq!(RateLimitMode::Econome.sync_interval(), Some(Duration::from_secs(300)));
    }
}
