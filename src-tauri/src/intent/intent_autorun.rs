//! Intent auto-run policy and executor

use crate::config::privacy::AutonomyLevel;
use crate::core::utils::current_timestamp;

const DEFAULT_AUTO_INTENT_COOLDOWN_SECS: u64 = 15 * 60;

pub fn should_auto_create_intent(
    autonomy: AutonomyLevel,
    last_intent_at: u64,
    cooldown_override: Option<u64>,
) -> bool {
    if autonomy == AutonomyLevel::Observer || autonomy == AutonomyLevel::Suggester {
        return false;
    }

    let now = current_timestamp();
    let cooldown = cooldown_override
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_AUTO_INTENT_COOLDOWN_SECS);
    if last_intent_at > 0 && now.saturating_sub(last_intent_at) < cooldown {
        return false;
    }

    true
}
