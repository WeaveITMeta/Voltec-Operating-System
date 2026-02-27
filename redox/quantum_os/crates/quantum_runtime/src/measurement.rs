// =============================================================================
// V-OS - Measurement Module
// =============================================================================
// Table of Contents:
//   1. MeasurementEvent - Single measurement result
//   2. MeasurementStream - Stream of measurement events
//   3. MeasurementStatistics - Aggregated statistics
//   4. ObservableOperatorInterface - Trait for observables
// =============================================================================
// Purpose: Handles quantum measurement operations, result streaming, and
//          statistical analysis of measurement outcomes.
// =============================================================================

use crate::state_backend::QuantumStateVector;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// 1. MeasurementEvent - Single measurement result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementEvent {
    pub job_id: Uuid,
    pub shot_index: usize,
    pub measurement_bitstring: Vec<u8>,
    pub timestamp_nanoseconds: u64,
    pub classical_register_values: Option<Vec<u64>>,
}

impl MeasurementEvent {
    pub fn new(job_id: Uuid, shot_index: usize, bitstring: Vec<u8>) -> Self {
        Self {
            job_id,
            shot_index,
            measurement_bitstring: bitstring,
            timestamp_nanoseconds: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            classical_register_values: None,
        }
    }

    pub fn bitstring_as_string(&self) -> String {
        self.measurement_bitstring
            .iter()
            .map(|b| if *b == 0 { '0' } else { '1' })
            .collect()
    }

    pub fn bitstring_as_integer(&self) -> u64 {
        self.measurement_bitstring
            .iter()
            .fold(0u64, |acc, &bit| (acc << 1) | (bit as u64))
    }
}

// =============================================================================
// 2. MeasurementStream - Stream of measurement events
// =============================================================================

#[derive(Debug)]
pub struct MeasurementStream {
    job_id: Uuid,
    events: Vec<MeasurementEvent>,
    is_complete: bool,
    total_shots: usize,
}

impl MeasurementStream {
    pub fn new(job_id: Uuid, total_shots: usize) -> Self {
        Self {
            job_id,
            events: Vec::with_capacity(total_shots),
            is_complete: false,
            total_shots,
        }
    }

    pub fn add_event(&mut self, event: MeasurementEvent) {
        self.events.push(event);
        if self.events.len() >= self.total_shots {
            self.is_complete = true;
        }
    }

    pub fn events(&self) -> &[MeasurementEvent] {
        &self.events
    }

    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    pub fn completed_shots(&self) -> usize {
        self.events.len()
    }

    pub fn total_shots(&self) -> usize {
        self.total_shots
    }

    pub fn progress_fraction(&self) -> f64 {
        self.events.len() as f64 / self.total_shots as f64
    }

    pub fn compute_statistics(&self) -> MeasurementStatistics {
        MeasurementStatistics::from_events(&self.events)
    }
}

// =============================================================================
// 3. MeasurementStatistics - Aggregated statistics
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementStatistics {
    pub total_shots: usize,
    pub bitstring_counts: HashMap<String, usize>,
    pub probabilities: HashMap<String, f64>,
    pub entropy: f64,
}

impl MeasurementStatistics {
    pub fn from_events(events: &[MeasurementEvent]) -> Self {
        let total_shots = events.len();
        let mut bitstring_counts: HashMap<String, usize> = HashMap::new();

        for event in events {
            let key = event.bitstring_as_string();
            *bitstring_counts.entry(key).or_insert(0) += 1;
        }

        let probabilities: HashMap<String, f64> = bitstring_counts
            .iter()
            .map(|(k, &v)| (k.clone(), v as f64 / total_shots as f64))
            .collect();

        let entropy = Self::compute_entropy(&probabilities);

        Self {
            total_shots,
            bitstring_counts,
            probabilities,
            entropy,
        }
    }

    fn compute_entropy(probabilities: &HashMap<String, f64>) -> f64 {
        probabilities
            .values()
            .filter(|&&p| p > 0.0)
            .map(|&p| -p * p.log2())
            .sum()
    }

    pub fn most_probable_bitstring(&self) -> Option<(&String, f64)> {
        self.probabilities
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, &v)| (k, v))
    }

    pub fn probability_of(&self, bitstring: &str) -> f64 {
        *self.probabilities.get(bitstring).unwrap_or(&0.0)
    }

    pub fn count_of(&self, bitstring: &str) -> usize {
        *self.bitstring_counts.get(bitstring).unwrap_or(&0)
    }
}

// =============================================================================
// 4. ObservableOperatorInterface - Trait for quantum observables
// =============================================================================

pub trait ObservableOperatorInterface: Send + Sync + std::fmt::Debug {
    fn compute_expectation_value(&self, state: &QuantumStateVector) -> f64;
    fn observable_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct PauliZObservable {
    target_qubit: usize,
}

impl PauliZObservable {
    pub fn new(target_qubit: usize) -> Self {
        Self { target_qubit }
    }
}

impl ObservableOperatorInterface for PauliZObservable {
    fn compute_expectation_value(&self, state: &QuantumStateVector) -> f64 {
        state.expectation_value_pauli_z(self.target_qubit)
    }

    fn observable_name(&self) -> &str {
        "pauli_z_observable"
    }
}

#[derive(Debug, Clone)]
pub struct HamiltonianObservable {
    terms: Vec<(f64, Vec<(usize, PauliOperator)>)>,
    name: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PauliOperator {
    Identity,
    PauliX,
    PauliY,
    PauliZ,
}

impl HamiltonianObservable {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            terms: Vec::new(),
            name: name.into(),
        }
    }

    pub fn add_term(&mut self, coefficient: f64, operators: Vec<(usize, PauliOperator)>) {
        self.terms.push((coefficient, operators));
    }

    pub fn ising_zz(number_of_qubits: usize, coupling: f64) -> Self {
        let mut hamiltonian = Self::new("ising_zz_hamiltonian");
        for i in 0..number_of_qubits - 1 {
            hamiltonian.add_term(
                coupling,
                vec![(i, PauliOperator::PauliZ), (i + 1, PauliOperator::PauliZ)],
            );
        }
        hamiltonian
    }
}

impl ObservableOperatorInterface for HamiltonianObservable {
    fn compute_expectation_value(&self, state: &QuantumStateVector) -> f64 {
        let mut total = 0.0;
        for (coeff, operators) in &self.terms {
            let mut term_expectation = *coeff;
            for (qubit, op) in operators {
                match op {
                    PauliOperator::Identity => {}
                    PauliOperator::PauliZ => {
                        term_expectation *= state.expectation_value_pauli_z(*qubit);
                    }
                    PauliOperator::PauliX | PauliOperator::PauliY => {
                        term_expectation *= 0.0;
                    }
                }
            }
            total += term_expectation;
        }
        total
    }

    fn observable_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measurement_event_creation() {
        let job_id = Uuid::new_v4();
        let event = MeasurementEvent::new(job_id, 0, vec![0, 1, 1]);
        assert_eq!(event.bitstring_as_string(), "011");
        assert_eq!(event.bitstring_as_integer(), 3);
    }

    #[test]
    fn test_measurement_statistics() {
        let job_id = Uuid::new_v4();
        let events = vec![
            MeasurementEvent::new(job_id, 0, vec![0, 0]),
            MeasurementEvent::new(job_id, 1, vec![0, 0]),
            MeasurementEvent::new(job_id, 2, vec![1, 1]),
            MeasurementEvent::new(job_id, 3, vec![1, 1]),
        ];

        let stats = MeasurementStatistics::from_events(&events);
        assert_eq!(stats.total_shots, 4);
        assert!((stats.probability_of("00") - 0.5).abs() < 1e-10);
        assert!((stats.probability_of("11") - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_z_expectation() {
        let state = QuantumStateVector::zero_state(1);
        let observable = PauliZObservable::new(0);
        let expectation = observable.compute_expectation_value(&state);
        assert!((expectation - 1.0).abs() < 1e-10);
    }
}
