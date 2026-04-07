use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const EPSILON: f64 = 1.0e-9;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorSnapshot {
    pub id: String,
    pub stake: f64,
    pub weights: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MinerSnapshot {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Yc3Input {
    pub validators: Vec<ValidatorSnapshot>,
    pub miners: Vec<MinerSnapshot>,
    pub previous_bonds: Option<Vec<Vec<f64>>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Yc3Parameters {
    pub kappa: f64,
    pub bonds_penalty: f64,
    pub ema_alpha: f64,
    pub validator_emission_ratio: f64,
}

impl Default for Yc3Parameters {
    fn default() -> Self {
        Self {
            kappa: 0.5,
            bonds_penalty: 0.9,
            ema_alpha: 0.1,
            validator_emission_ratio: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MinerReport {
    pub id: String,
    pub prerank: f64,
    pub rank: f64,
    pub consensus_weight: f64,
    pub trust: f64,
    pub incentive: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorReport {
    pub id: String,
    pub normalized_stake: f64,
    pub validator_trust: f64,
    pub reward_share: f64,
    pub ema_bonds: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Yc3Report {
    pub parameters: Yc3Parameters,
    pub miners: Vec<MinerReport>,
    pub validators: Vec<ValidatorReport>,
    pub clipped_weights: Vec<Vec<f64>>,
    pub penalized_weights: Vec<Vec<f64>>,
    pub bonds_delta: Vec<Vec<f64>>,
    pub ema_bonds: Vec<Vec<f64>>,
}

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("invalid consensus input: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone)]
pub struct Yc3Engine {
    params: Yc3Parameters,
}

impl Yc3Engine {
    pub fn new(params: Yc3Parameters) -> Self {
        Self { params }
    }

    pub fn evaluate(&self, input: &Yc3Input) -> Result<Yc3Report, ConsensusError> {
        self.validate_input(input)?;

        let stakes = normalize(
            &input
                .validators
                .iter()
                .map(|validator| validator.stake.max(0.0))
                .collect::<Vec<_>>(),
        );
        let raw_weights = input
            .validators
            .iter()
            .map(|validator| validator.weights.clone())
            .collect::<Vec<_>>();
        let normalized_weights = normalize_rows(&raw_weights);

        let consensus_weights = (0..input.miners.len())
            .map(|miner_index| {
                let column = normalized_weights
                    .iter()
                    .map(|row| row[miner_index])
                    .collect::<Vec<_>>();
                supported_weight(&stakes, &column, self.params.kappa)
            })
            .collect::<Vec<_>>();

        let clipped_weights = normalized_weights
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(miner_index, weight)| weight.min(consensus_weights[miner_index]))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let penalized_weights = normalized_weights
            .iter()
            .zip(clipped_weights.iter())
            .map(|(raw_row, clipped_row)| {
                raw_row
                    .iter()
                    .zip(clipped_row.iter())
                    .map(|(raw, clipped)| {
                        ((1.0 - self.params.bonds_penalty) * raw)
                            + (self.params.bonds_penalty * clipped)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let preranks = column_weighted_sum(&normalized_weights, &stakes);
        let ranks = column_weighted_sum(&clipped_weights, &stakes);
        let trusts = ranks
            .iter()
            .zip(preranks.iter())
            .map(|(rank, prerank)| safe_ratio(*rank, *prerank))
            .collect::<Vec<_>>();
        let incentives = normalize(&ranks);
        let validator_trust = clipped_weights
            .iter()
            .map(|row| row.iter().sum::<f64>())
            .collect::<Vec<_>>();
        let bonds_delta = build_bonds_delta(&penalized_weights, &stakes);
        let ema_bonds = self.build_ema_bonds(input, &bonds_delta)?;
        let validator_rewards_raw = ema_bonds
            .iter()
            .map(|row| dot(row, &incentives))
            .collect::<Vec<_>>();
        let validator_rewards = normalize(&validator_rewards_raw);

        let miners = input
            .miners
            .iter()
            .enumerate()
            .map(|(miner_index, miner)| MinerReport {
                id: miner.id.clone(),
                prerank: preranks[miner_index],
                rank: ranks[miner_index],
                consensus_weight: consensus_weights[miner_index],
                trust: trusts[miner_index],
                incentive: incentives[miner_index],
            })
            .collect::<Vec<_>>();

        let validators = input
            .validators
            .iter()
            .enumerate()
            .map(|(validator_index, validator)| ValidatorReport {
                id: validator.id.clone(),
                normalized_stake: stakes[validator_index],
                validator_trust: validator_trust[validator_index],
                reward_share: validator_rewards[validator_index],
                ema_bonds: ema_bonds[validator_index].clone(),
            })
            .collect::<Vec<_>>();

        Ok(Yc3Report {
            parameters: self.params,
            miners,
            validators,
            clipped_weights,
            penalized_weights,
            bonds_delta,
            ema_bonds,
        })
    }

    fn validate_input(&self, input: &Yc3Input) -> Result<(), ConsensusError> {
        if input.validators.is_empty() {
            return Err(ConsensusError::InvalidInput(
                "at least one validator is required".to_string(),
            ));
        }

        if input.miners.is_empty() {
            return Err(ConsensusError::InvalidInput(
                "at least one miner is required".to_string(),
            ));
        }

        for validator in &input.validators {
            if validator.stake < 0.0 {
                return Err(ConsensusError::InvalidInput(format!(
                    "validator {} has negative stake",
                    validator.id
                )));
            }

            if validator.weights.len() != input.miners.len() {
                return Err(ConsensusError::InvalidInput(format!(
                    "validator {} weight count does not match miner count",
                    validator.id
                )));
            }

            if validator.weights.iter().any(|weight| *weight < 0.0) {
                return Err(ConsensusError::InvalidInput(format!(
                    "validator {} has negative weights",
                    validator.id
                )));
            }
        }

        if let Some(previous_bonds) = &input.previous_bonds {
            if previous_bonds.len() != input.validators.len() {
                return Err(ConsensusError::InvalidInput(
                    "previous_bonds row count must match validators".to_string(),
                ));
            }

            for row in previous_bonds {
                if row.len() != input.miners.len() {
                    return Err(ConsensusError::InvalidInput(
                        "previous_bonds column count must match miners".to_string(),
                    ));
                }
            }
        }

        if !(0.5..=1.0).contains(&self.params.kappa) {
            return Err(ConsensusError::InvalidInput(
                "kappa must be between 0.5 and 1.0".to_string(),
            ));
        }

        if !(0.0..=1.0).contains(&self.params.bonds_penalty)
            || !(0.0..=1.0).contains(&self.params.ema_alpha)
            || !(0.0..=1.0).contains(&self.params.validator_emission_ratio)
        {
            return Err(ConsensusError::InvalidInput(
                "bonds_penalty, ema_alpha, and validator_emission_ratio must be between 0.0 and 1.0"
                    .to_string(),
            ));
        }

        Ok(())
    }

    fn build_ema_bonds(
        &self,
        input: &Yc3Input,
        bonds_delta: &[Vec<f64>],
    ) -> Result<Vec<Vec<f64>>, ConsensusError> {
        let previous = input.previous_bonds.clone().unwrap_or_else(|| {
            vec![vec![0.0; input.miners.len()]; input.validators.len()]
        });

        if previous.len() != bonds_delta.len() {
            return Err(ConsensusError::InvalidInput(
                "previous_bonds row count must match bonds_delta".to_string(),
            ));
        }

        Ok(previous
            .iter()
            .zip(bonds_delta.iter())
            .map(|(prev_row, delta_row)| {
                prev_row
                    .iter()
                    .zip(delta_row.iter())
                    .map(|(prev, delta)| {
                        (self.params.ema_alpha * delta)
                            + ((1.0 - self.params.ema_alpha) * prev)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>())
    }
}

fn normalize(values: &[f64]) -> Vec<f64> {
    let total = values.iter().sum::<f64>();
    if total <= EPSILON {
        return vec![0.0; values.len()];
    }

    values.iter().map(|value| value / total).collect::<Vec<_>>()
}

fn normalize_rows(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    matrix.iter().map(|row| normalize(row)).collect::<Vec<_>>()
}

fn supported_weight(stakes: &[f64], column: &[f64], kappa: f64) -> f64 {
    let mut candidates = column.to_vec();
    candidates.sort_by(|left, right| right.partial_cmp(left).unwrap_or(Ordering::Equal));
    candidates.dedup_by(|left, right| (*left - *right).abs() < EPSILON);

    for candidate in candidates {
        let supported = stakes
            .iter()
            .zip(column.iter())
            .filter(|(_, weight)| **weight + EPSILON >= candidate)
            .map(|(stake, _)| *stake)
            .sum::<f64>();

        if supported + EPSILON >= kappa {
            return candidate;
        }
    }

    0.0
}

fn column_weighted_sum(matrix: &[Vec<f64>], stakes: &[f64]) -> Vec<f64> {
    let Some(width) = matrix.first().map(Vec::len) else {
        return Vec::new();
    };

    let mut output = vec![0.0; width];
    for (row, stake) in matrix.iter().zip(stakes.iter()) {
        for (index, value) in row.iter().enumerate() {
            output[index] += value * stake;
        }
    }

    output
}

fn build_bonds_delta(penalized_weights: &[Vec<f64>], stakes: &[f64]) -> Vec<Vec<f64>> {
    let Some(width) = penalized_weights.first().map(Vec::len) else {
        return Vec::new();
    };

    let mut bonds = vec![vec![0.0; width]; penalized_weights.len()];
    for miner_index in 0..width {
        let denominator = penalized_weights
            .iter()
            .zip(stakes.iter())
            .map(|(row, stake)| row[miner_index] * stake)
            .sum::<f64>();

        if denominator <= EPSILON {
            continue;
        }

        for (validator_index, (row, stake)) in penalized_weights.iter().zip(stakes.iter()).enumerate() {
            bonds[validator_index][miner_index] = (row[miner_index] * stake) / denominator;
        }
    }

    bonds
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum::<f64>()
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator <= EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1.0e-6,
            "expected {left} to be close to {right}"
        );
    }

    #[test]
    fn clips_outlier_weight_to_majority_supported_value() {
        let engine = Yc3Engine::new(Yc3Parameters::default());
        let input = Yc3Input {
            validators: vec![
                ValidatorSnapshot {
                    id: "majority".to_string(),
                    stake: 70.0,
                    weights: vec![0.4, 0.6],
                },
                ValidatorSnapshot {
                    id: "outlier".to_string(),
                    stake: 30.0,
                    weights: vec![1.0, 0.0],
                },
            ],
            miners: vec![
                MinerSnapshot {
                    id: "miner-a".to_string(),
                },
                MinerSnapshot {
                    id: "miner-b".to_string(),
                },
            ],
            previous_bonds: None,
        };

        let report = engine.evaluate(&input).expect("report should build");

        assert_close(report.miners[0].consensus_weight, 0.4);
        assert_close(report.clipped_weights[1][0], 0.4);
        assert!(report.miners[0].trust < 1.0);
    }

    #[test]
    fn incentives_and_validator_rewards_form_probability_vectors() {
        let engine = Yc3Engine::new(Yc3Parameters::default());
        let input = Yc3Input {
            validators: vec![
                ValidatorSnapshot {
                    id: "v1".to_string(),
                    stake: 110.0,
                    weights: vec![0.5, 0.3, 0.2],
                },
                ValidatorSnapshot {
                    id: "v2".to_string(),
                    stake: 90.0,
                    weights: vec![0.4, 0.4, 0.2],
                },
                ValidatorSnapshot {
                    id: "v3".to_string(),
                    stake: 60.0,
                    weights: vec![0.2, 0.3, 0.5],
                },
            ],
            miners: vec![
                MinerSnapshot {
                    id: "m1".to_string(),
                },
                MinerSnapshot {
                    id: "m2".to_string(),
                },
                MinerSnapshot {
                    id: "m3".to_string(),
                },
            ],
            previous_bonds: None,
        };

        let report = engine.evaluate(&input).expect("report should build");
        let incentive_sum = report.miners.iter().map(|miner| miner.incentive).sum::<f64>();
        let reward_sum = report
            .validators
            .iter()
            .map(|validator| validator.reward_share)
            .sum::<f64>();

        assert_close(incentive_sum, 1.0);
        assert_close(reward_sum, 1.0);
    }

    #[test]
    fn ema_bonds_blend_previous_state_and_current_observation() {
        let engine = Yc3Engine::new(Yc3Parameters {
            ema_alpha: 0.25,
            ..Yc3Parameters::default()
        });
        let input = Yc3Input {
            validators: vec![
                ValidatorSnapshot {
                    id: "v1".to_string(),
                    stake: 60.0,
                    weights: vec![1.0],
                },
                ValidatorSnapshot {
                    id: "v2".to_string(),
                    stake: 40.0,
                    weights: vec![1.0],
                },
            ],
            miners: vec![MinerSnapshot {
                id: "m1".to_string(),
            }],
            previous_bonds: Some(vec![vec![0.2], vec![0.8]]),
        };

        let report = engine.evaluate(&input).expect("report should build");

        assert_close(report.bonds_delta[0][0], 0.6);
        assert_close(report.bonds_delta[1][0], 0.4);
        assert_close(report.ema_bonds[0][0], 0.3);
        assert_close(report.ema_bonds[1][0], 0.7);
    }
}
