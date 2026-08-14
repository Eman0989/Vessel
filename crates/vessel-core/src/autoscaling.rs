use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoscalingPolicy {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub target_cpu_utilization_percent: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoscalingDirection {
    ScaleUp,
    ScaleDown,
    Stable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoscalingDecision {
    pub current_replicas: u32,
    pub desired_replicas: u32,
    pub observed_cpu_utilization_percent: u8,
    pub target_cpu_utilization_percent: u8,
    pub direction: AutoscalingDirection,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum AutoscalingDecisionError {
    #[error(
        "observed CPU utilization must be between 0 and 100 percent: observed={observed_percent}"
    )]
    InvalidObservedCpuUtilization { observed_percent: u8 },
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum AutoscalingPolicyError {
    #[error("autoscaling minimum replicas must be at least 1")]
    MinimumReplicasMustBePositive,

    #[error("autoscaling replica bounds are invalid: min={min_replicas}, max={max_replicas}")]
    InvalidReplicaBounds {
        min_replicas: u32,
        max_replicas: u32,
    },

    #[error(
        "autoscaling target CPU utilization must be between 1 and 100 percent: target={target_percent}"
    )]
    InvalidCpuTarget { target_percent: u8 },
}

impl AutoscalingPolicy {
    pub fn new(
        min_replicas: u32,
        max_replicas: u32,
        target_cpu_utilization_percent: u8,
    ) -> Result<Self, AutoscalingPolicyError> {
        if min_replicas == 0 {
            return Err(AutoscalingPolicyError::MinimumReplicasMustBePositive);
        }

        if max_replicas < min_replicas {
            return Err(AutoscalingPolicyError::InvalidReplicaBounds {
                min_replicas,
                max_replicas,
            });
        }

        if !(1..=100).contains(&target_cpu_utilization_percent) {
            return Err(AutoscalingPolicyError::InvalidCpuTarget {
                target_percent: target_cpu_utilization_percent,
            });
        }

        Ok(Self {
            min_replicas,
            max_replicas,
            target_cpu_utilization_percent,
        })
    }

    pub fn clamp_replicas(&self, replicas: u32) -> u32 {
        replicas.clamp(self.min_replicas, self.max_replicas)
    }

    pub fn contains_replicas(&self, replicas: u32) -> bool {
        (self.min_replicas..=self.max_replicas).contains(&replicas)
    }

    pub fn decide(
        &self,
        current_replicas: u32,
        observed_cpu_utilization_percent: u8,
    ) -> Result<AutoscalingDecision, AutoscalingDecisionError> {
        if observed_cpu_utilization_percent > 100 {
            return Err(AutoscalingDecisionError::InvalidObservedCpuUtilization {
                observed_percent: observed_cpu_utilization_percent,
            });
        }

        let numerator = u64::from(current_replicas) * u64::from(observed_cpu_utilization_percent);

        let denominator = u64::from(self.target_cpu_utilization_percent);

        // Integer ceiling division implements:
        //
        // ceil(
        //   current replicas * observed utilization
        //   / target utilization
        // )
        //
        // Calculating in u64 avoids overflow for u32 replica
        // counts before the result is bounded by the policy.
        let raw_desired = numerator.div_ceil(denominator);

        let desired_replicas =
            raw_desired.clamp(u64::from(self.min_replicas), u64::from(self.max_replicas)) as u32;

        let direction = match desired_replicas.cmp(&current_replicas) {
            std::cmp::Ordering::Greater => AutoscalingDirection::ScaleUp,
            std::cmp::Ordering::Less => AutoscalingDirection::ScaleDown,
            std::cmp::Ordering::Equal => AutoscalingDirection::Stable,
        };

        Ok(AutoscalingDecision {
            current_replicas,
            desired_replicas,
            observed_cpu_utilization_percent,
            target_cpu_utilization_percent: self.target_cpu_utilization_percent,
            direction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_policy_is_created() {
        let policy = AutoscalingPolicy::new(2, 10, 70).unwrap();

        assert_eq!(policy.min_replicas, 2);
        assert_eq!(policy.max_replicas, 10);
        assert_eq!(policy.target_cpu_utilization_percent, 70,);
    }

    #[test]
    fn minimum_replicas_must_be_positive() {
        assert_eq!(
            AutoscalingPolicy::new(0, 10, 70),
            Err(AutoscalingPolicyError::MinimumReplicasMustBePositive),
        );
    }

    #[test]
    fn maximum_must_not_be_below_minimum() {
        assert_eq!(
            AutoscalingPolicy::new(5, 4, 70),
            Err(AutoscalingPolicyError::InvalidReplicaBounds {
                min_replicas: 5,
                max_replicas: 4,
            }),
        );
    }

    #[test]
    fn cpu_target_cannot_be_zero() {
        assert_eq!(
            AutoscalingPolicy::new(1, 10, 0),
            Err(AutoscalingPolicyError::InvalidCpuTarget { target_percent: 0 }),
        );
    }

    #[test]
    fn cpu_target_cannot_exceed_one_hundred() {
        assert_eq!(
            AutoscalingPolicy::new(1, 10, 101),
            Err(AutoscalingPolicyError::InvalidCpuTarget {
                target_percent: 101,
            }),
        );
    }

    #[test]
    fn replica_count_is_clamped_to_policy_bounds() {
        let policy = AutoscalingPolicy::new(2, 10, 70).unwrap();

        assert_eq!(policy.clamp_replicas(1), 2);
        assert_eq!(policy.clamp_replicas(6), 6);
        assert_eq!(policy.clamp_replicas(15), 10);
    }

    #[test]
    fn utilization_at_target_keeps_replica_count_stable() {
        let policy = AutoscalingPolicy::new(1, 10, 70).unwrap();

        let decision = policy.decide(4, 70).unwrap();

        assert_eq!(decision.current_replicas, 4);
        assert_eq!(decision.desired_replicas, 4);
        assert_eq!(decision.direction, AutoscalingDirection::Stable,);
    }

    #[test]
    fn utilization_above_target_scales_up_with_ceiling() {
        let policy = AutoscalingPolicy::new(1, 10, 70).unwrap();

        let decision = policy.decide(4, 80).unwrap();

        assert_eq!(decision.desired_replicas, 5);
        assert_eq!(decision.direction, AutoscalingDirection::ScaleUp,);
    }

    #[test]
    fn utilization_below_target_scales_down() {
        let policy = AutoscalingPolicy::new(1, 10, 70).unwrap();

        let decision = policy.decide(4, 35).unwrap();

        assert_eq!(decision.desired_replicas, 2);
        assert_eq!(decision.direction, AutoscalingDirection::ScaleDown,);
    }

    #[test]
    fn decision_respects_minimum_replica_bound() {
        let policy = AutoscalingPolicy::new(2, 10, 70).unwrap();

        let decision = policy.decide(4, 0).unwrap();

        assert_eq!(decision.desired_replicas, 2);
    }

    #[test]
    fn decision_respects_maximum_replica_bound() {
        let policy = AutoscalingPolicy::new(1, 6, 50).unwrap();

        let decision = policy.decide(5, 100).unwrap();

        assert_eq!(decision.desired_replicas, 6);
    }

    #[test]
    fn zero_replicas_recover_to_policy_minimum() {
        let policy = AutoscalingPolicy::new(2, 10, 70).unwrap();

        let decision = policy.decide(0, 0).unwrap();

        assert_eq!(decision.desired_replicas, 2);
        assert_eq!(decision.direction, AutoscalingDirection::ScaleUp,);
    }

    #[test]
    fn observed_cpu_above_one_hundred_is_rejected() {
        let policy = AutoscalingPolicy::new(1, 10, 70).unwrap();

        assert_eq!(
            policy.decide(4, 101),
            Err(AutoscalingDecisionError::InvalidObservedCpuUtilization {
                observed_percent: 101,
            }),
        );
    }

    #[test]
    fn policy_reports_replica_membership() {
        let policy = AutoscalingPolicy::new(2, 10, 70).unwrap();

        assert!(!policy.contains_replicas(1));
        assert!(policy.contains_replicas(2));
        assert!(policy.contains_replicas(7));
        assert!(policy.contains_replicas(10));
        assert!(!policy.contains_replicas(11));
    }
}
