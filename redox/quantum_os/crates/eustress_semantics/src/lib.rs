// =============================================================================
// V-OS - Eustress Semantics Layer
// =============================================================================
// Table of Contents:
//   1. MeaningPacket - Core semantic container
//   2. BasisFrame - Measurement basis representation
//   3. StatisticsWindow - Statistical analysis window
//   4. SemanticInferenceEngine - Meaning inference from measurements
// =============================================================================
// Purpose: Provides semantic meaning layer for quantum observability. Converts
//          raw measurement data into meaningful insights through operator,
//          basis, and statistics analysis. Enables "Eustress" - positive stress
//          from understanding quantum state evolution.
// =============================================================================

use num_complex::Complex64;
use quantum_runtime::measurement::{MeasurementStatistics, ObservableOperatorInterface};
use quantum_runtime::state_backend::QuantumStateVector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// 1. MeaningPacket - Core semantic container
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeaningPacket {
    pub id: Uuid,
    pub observable_operator: ObservableDescription,
    pub basis_frame: BasisFrame,
    pub statistics_window: StatisticsWindow,
    pub timestamp: u64,
    pub interpretation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservableDescription {
    pub name: String,
    pub operator_type: OperatorType,
    pub target_qubits: Vec<usize>,
    pub expectation_value: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OperatorType {
    PauliX,
    PauliY,
    PauliZ,
    Hadamard,
    Identity,
    Hamiltonian,
    Custom,
}

impl MeaningPacket {
    pub fn new(
        observable: ObservableDescription,
        basis: BasisFrame,
        statistics: StatisticsWindow,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            observable_operator: observable,
            basis_frame: basis,
            statistics_window: statistics,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            interpretation: None,
        }
    }

    pub fn with_interpretation(mut self, interpretation: impl Into<String>) -> Self {
        self.interpretation = Some(interpretation.into());
        self
    }

    pub fn is_highly_entangled(&self) -> bool {
        self.statistics_window.entropy > 0.9 * (self.statistics_window.number_of_qubits as f64)
    }

    pub fn is_near_eigenstate(&self) -> bool {
        self.observable_operator.expectation_value.abs() > 0.99
    }
}

// =============================================================================
// 2. BasisFrame - Measurement basis representation
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasisFrame {
    pub name: String,
    pub basis_type: BasisType,
    pub qubit_bases: Vec<SingleQubitBasis>,
    pub transformation_angles: Option<Vec<(f64, f64)>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BasisType {
    ComputationalBasis,
    PauliXBasis,
    PauliYBasis,
    PauliZBasis,
    BellBasis,
    CustomBasis,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SingleQubitBasis {
    pub qubit_index: usize,
    pub theta: f64,
    pub phi: f64,
}

impl BasisFrame {
    pub fn computational(number_of_qubits: usize) -> Self {
        let qubit_bases = (0..number_of_qubits)
            .map(|i| SingleQubitBasis {
                qubit_index: i,
                theta: 0.0,
                phi: 0.0,
            })
            .collect();

        Self {
            name: "computational_basis".to_string(),
            basis_type: BasisType::ComputationalBasis,
            qubit_bases,
            transformation_angles: None,
        }
    }

    pub fn pauli_x(number_of_qubits: usize) -> Self {
        let qubit_bases = (0..number_of_qubits)
            .map(|i| SingleQubitBasis {
                qubit_index: i,
                theta: std::f64::consts::FRAC_PI_2,
                phi: 0.0,
            })
            .collect();

        Self {
            name: "pauli_x_basis".to_string(),
            basis_type: BasisType::PauliXBasis,
            qubit_bases,
            transformation_angles: None,
        }
    }

    pub fn pauli_y(number_of_qubits: usize) -> Self {
        let qubit_bases = (0..number_of_qubits)
            .map(|i| SingleQubitBasis {
                qubit_index: i,
                theta: std::f64::consts::FRAC_PI_2,
                phi: std::f64::consts::FRAC_PI_2,
            })
            .collect();

        Self {
            name: "pauli_y_basis".to_string(),
            basis_type: BasisType::PauliYBasis,
            qubit_bases,
            transformation_angles: None,
        }
    }

    pub fn basis_transform_operation(&self) -> BasisTransformOperation {
        BasisTransformOperation {
            source_basis: BasisType::ComputationalBasis,
            target_basis: self.basis_type,
            rotation_sequence: self.qubit_bases.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasisTransformOperation {
    pub source_basis: BasisType,
    pub target_basis: BasisType,
    pub rotation_sequence: Vec<SingleQubitBasis>,
}

// =============================================================================
// 3. StatisticsWindow - Statistical analysis window
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsWindow {
    pub window_id: Uuid,
    pub number_of_qubits: usize,
    pub total_shots: usize,
    pub entropy: f64,
    pub entropy_delta: f64,
    pub purity: f64,
    pub probability_distribution_snapshot: HashMap<String, f64>,
    pub fidelity_estimate: Option<f64>,
}

impl StatisticsWindow {
    pub fn from_measurement_statistics(
        stats: &MeasurementStatistics,
        number_of_qubits: usize,
    ) -> Self {
        let purity = stats
            .probabilities
            .values()
            .map(|p| p * p)
            .sum::<f64>();

        Self {
            window_id: Uuid::new_v4(),
            number_of_qubits,
            total_shots: stats.total_shots,
            entropy: stats.entropy,
            entropy_delta: 0.0,
            purity,
            probability_distribution_snapshot: stats.probabilities.clone(),
            fidelity_estimate: None,
        }
    }

    pub fn compute_entropy_delta(&mut self, previous_entropy: f64) {
        self.entropy_delta = self.entropy - previous_entropy;
    }

    pub fn estimate_fidelity_to_target(&mut self, target_distribution: &HashMap<String, f64>) {
        let fidelity: f64 = self
            .probability_distribution_snapshot
            .iter()
            .map(|(k, &p)| {
                let q = target_distribution.get(k).copied().unwrap_or(0.0);
                (p.sqrt() * q.sqrt())
            })
            .sum::<f64>()
            .powi(2);

        self.fidelity_estimate = Some(fidelity);
    }

    pub fn is_pure_state(&self) -> bool {
        self.purity > 0.99
    }

    pub fn is_maximally_mixed(&self) -> bool {
        let max_mixed_purity = 1.0 / (1 << self.number_of_qubits) as f64;
        (self.purity - max_mixed_purity).abs() < 0.01
    }
}

// =============================================================================
// 4. SemanticInferenceEngine - Meaning inference from measurements
// =============================================================================

#[derive(Debug)]
pub struct SemanticInferenceEngine {
    history: Vec<MeaningPacket>,
    max_history_size: usize,
}

impl Default for SemanticInferenceEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticInferenceEngine {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_history_size: 1000,
        }
    }

    pub fn analyze_measurement(
        &mut self,
        stats: &MeasurementStatistics,
        observable_name: &str,
        number_of_qubits: usize,
    ) -> MeaningPacket {
        let observable = ObservableDescription {
            name: observable_name.to_string(),
            operator_type: OperatorType::Custom,
            target_qubits: (0..number_of_qubits).collect(),
            expectation_value: 0.0,
        };

        let basis = BasisFrame::computational(number_of_qubits);
        let mut statistics = StatisticsWindow::from_measurement_statistics(stats, number_of_qubits);

        if let Some(last) = self.history.last() {
            statistics.compute_entropy_delta(last.statistics_window.entropy);
        }

        let interpretation = self.generate_interpretation(&statistics);

        let packet = MeaningPacket::new(observable, basis, statistics)
            .with_interpretation(interpretation);

        self.record_packet(packet.clone());
        packet
    }

    pub fn analyze_state_with_observable(
        &mut self,
        state: &QuantumStateVector,
        observable: &dyn ObservableOperatorInterface,
    ) -> MeaningPacket {
        let expectation = observable.compute_expectation_value(state);
        let probs = state.probability_distribution();
        let number_of_qubits = state.number_of_quantum_bits();

        let mut prob_map: HashMap<String, f64> = HashMap::new();
        for (i, &prob) in probs.iter().enumerate() {
            let bitstring: String = (0..number_of_qubits)
                .rev()
                .map(|bit| if (i >> bit) & 1 == 0 { '0' } else { '1' })
                .collect();
            if prob > 1e-10 {
                prob_map.insert(bitstring, prob);
            }
        }

        let entropy: f64 = prob_map
            .values()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum();

        let observable_desc = ObservableDescription {
            name: observable.observable_name().to_string(),
            operator_type: OperatorType::Custom,
            target_qubits: (0..number_of_qubits).collect(),
            expectation_value: expectation,
        };

        let basis = BasisFrame::computational(number_of_qubits);

        let statistics = StatisticsWindow {
            window_id: Uuid::new_v4(),
            number_of_qubits,
            total_shots: 0,
            entropy,
            entropy_delta: 0.0,
            purity: prob_map.values().map(|p| p * p).sum(),
            probability_distribution_snapshot: prob_map,
            fidelity_estimate: None,
        };

        let interpretation = format!(
            "Observable '{}' expectation: {:.4}. State entropy: {:.4} bits.",
            observable.observable_name(),
            expectation,
            entropy
        );

        let packet = MeaningPacket::new(observable_desc, basis, statistics)
            .with_interpretation(interpretation);

        self.record_packet(packet.clone());
        packet
    }

    fn generate_interpretation(&self, stats: &StatisticsWindow) -> String {
        let mut interpretation = String::new();

        if stats.is_pure_state() {
            interpretation.push_str("State appears pure (high purity). ");
        } else if stats.is_maximally_mixed() {
            interpretation.push_str("State appears maximally mixed. ");
        }

        if stats.entropy < 0.1 {
            interpretation.push_str("Low entropy suggests near-deterministic outcome. ");
        } else if stats.entropy > 0.9 * (stats.number_of_qubits as f64) {
            interpretation.push_str("High entropy indicates significant superposition. ");
        }

        if stats.entropy_delta.abs() > 0.1 {
            if stats.entropy_delta > 0.0 {
                interpretation.push_str("Entropy increased - spreading in state space. ");
            } else {
                interpretation.push_str("Entropy decreased - state localization. ");
            }
        }

        if interpretation.is_empty() {
            interpretation = "Standard quantum state evolution observed.".to_string();
        }

        interpretation
    }

    fn record_packet(&mut self, packet: MeaningPacket) {
        self.history.push(packet);
        if self.history.len() > self.max_history_size {
            self.history.remove(0);
        }
    }

    pub fn history(&self) -> &[MeaningPacket] {
        &self.history
    }

    pub fn entropy_trend(&self, window_size: usize) -> Vec<f64> {
        self.history
            .iter()
            .rev()
            .take(window_size)
            .rev()
            .map(|p| p.statistics_window.entropy)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meaning_packet_creation() {
        let observable = ObservableDescription {
            name: "test_observable".to_string(),
            operator_type: OperatorType::PauliZ,
            target_qubits: vec![0],
            expectation_value: 0.5,
        };

        let basis = BasisFrame::computational(1);
        let mut prob_map = HashMap::new();
        prob_map.insert("0".to_string(), 0.75);
        prob_map.insert("1".to_string(), 0.25);

        let stats = StatisticsWindow {
            window_id: Uuid::new_v4(),
            number_of_qubits: 1,
            total_shots: 100,
            entropy: 0.811,
            entropy_delta: 0.0,
            purity: 0.625,
            probability_distribution_snapshot: prob_map,
            fidelity_estimate: None,
        };

        let packet = MeaningPacket::new(observable, basis, stats);
        assert!(!packet.is_near_eigenstate());
    }

    #[test]
    fn test_basis_frame_creation() {
        let basis = BasisFrame::computational(3);
        assert_eq!(basis.qubit_bases.len(), 3);
        assert_eq!(basis.basis_type, BasisType::ComputationalBasis);
    }
}
