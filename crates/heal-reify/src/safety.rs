use heal_core::{ActionStrategy, SafetyError};

pub struct SafetyGate;

impl SafetyGate {
    pub fn verify(
        strategy: &ActionStrategy,
        risk_score: f64,
        confidence: f64,
        is_agent: bool,
    ) -> Result<(), SafetyError> {
        if *strategy == ActionStrategy::Abort {
            return Err(SafetyError::Aborted(strategy.clone()));
        }

        let min_confidence = if is_agent { 0.92 } else { 0.70 };
        let max_risk = if is_agent { 0.10 } else { 0.40 };

        if confidence < min_confidence {
            return Err(SafetyError::LowConfidence(confidence, min_confidence));
        }

        if risk_score > max_risk {
            return Err(SafetyError::DestructiveAction(risk_score, max_risk));
        }

        Ok(())
    }

    pub fn requires_explicit_yes(risk_score: f64) -> bool {
        risk_score > 0.30
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_gate_agent_strict_bounds() {
        let action = ActionStrategy::GitSetUpstream;
        assert!(SafetyGate::verify(&action, 0.05, 0.95, true).is_ok());
        assert!(SafetyGate::verify(&action, 0.12, 0.95, true).is_err());
        assert!(SafetyGate::verify(&action, 0.05, 0.90, true).is_err());
    }

    #[test]
    fn test_safety_gate_human_bounds() {
        let action = ActionStrategy::PrependSudo;
        assert!(SafetyGate::verify(&action, 0.35, 0.85, false).is_ok());
        assert!(SafetyGate::verify(&action, 0.45, 0.85, false).is_err());
        assert!(SafetyGate::verify(&action, 0.20, 0.65, false).is_err());
    }

    #[test]
    fn test_explicit_yes_prompt_trigger() {
        assert!(SafetyGate::requires_explicit_yes(0.35));
        assert!(!SafetyGate::requires_explicit_yes(0.25));
    }
}
