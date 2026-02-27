// =============================================================================
// V-OS - UX Visualization Layer
// =============================================================================
// Table of Contents:
//   1. VisualizationDataPacket - Data for rendering
//   2. BlochSphereCoordinates - Single qubit visualization
//   3. StateHistogramData - Measurement histogram
//   4. VisualizationRenderer - Abstract renderer interface
// =============================================================================
// Purpose: Provides visualization data structures and rendering interfaces
//          for quantum state visualization. Feeds 3D/latent charts from
//          measurement streams for real-time quantum observability.
// =============================================================================

use eustress_semantics::{MeaningPacket, StatisticsWindow};
use num_complex::Complex64;
use quantum_runtime::measurement::MeasurementStatistics;
use quantum_runtime::state_backend::QuantumStateVector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// 1. VisualizationDataPacket - Data for rendering
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationDataPacket {
    pub id: Uuid,
    pub timestamp: u64,
    pub visualization_type: VisualizationType,
    pub bloch_spheres: Vec<BlochSphereCoordinates>,
    pub histogram: Option<StateHistogramData>,
    pub entanglement_graph: Option<EntanglementGraphData>,
    pub time_series: Option<TimeSeriesData>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VisualizationType {
    BlochSphere,
    StateHistogram,
    EntanglementGraph,
    TimeSeries,
    Combined,
}

impl VisualizationDataPacket {
    pub fn from_state(state: &QuantumStateVector) -> Self {
        let n = state.number_of_quantum_bits();
        let bloch_spheres: Vec<BlochSphereCoordinates> = (0..n)
            .map(|q| BlochSphereCoordinates::from_reduced_density_matrix(state, q))
            .collect();

        let histogram = StateHistogramData::from_state(state);

        Self {
            id: Uuid::new_v4(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            visualization_type: VisualizationType::Combined,
            bloch_spheres,
            histogram: Some(histogram),
            entanglement_graph: None,
            time_series: None,
        }
    }

    pub fn from_measurement_statistics(stats: &MeasurementStatistics, n_qubits: usize) -> Self {
        let histogram = StateHistogramData::from_statistics(stats);

        Self {
            id: Uuid::new_v4(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            visualization_type: VisualizationType::StateHistogram,
            bloch_spheres: Vec::new(),
            histogram: Some(histogram),
            entanglement_graph: None,
            time_series: None,
        }
    }

    pub fn from_meaning_packet(packet: &MeaningPacket) -> Self {
        let histogram = StateHistogramData {
            id: Uuid::new_v4(),
            bitstring_probabilities: packet
                .statistics_window
                .probability_distribution_snapshot
                .clone(),
            sorted_bitstrings: Vec::new(),
            max_probability: packet
                .statistics_window
                .probability_distribution_snapshot
                .values()
                .cloned()
                .fold(0.0, f64::max),
        };

        Self {
            id: Uuid::new_v4(),
            timestamp: packet.timestamp,
            visualization_type: VisualizationType::StateHistogram,
            bloch_spheres: Vec::new(),
            histogram: Some(histogram),
            entanglement_graph: None,
            time_series: None,
        }
    }
}

// =============================================================================
// 2. BlochSphereCoordinates - Single qubit visualization
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlochSphereCoordinates {
    pub qubit_index: usize,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub theta: f64,
    pub phi: f64,
    pub purity: f64,
}

impl BlochSphereCoordinates {
    pub fn from_reduced_density_matrix(state: &QuantumStateVector, qubit: usize) -> Self {
        let n = state.number_of_quantum_bits();
        let dim = state.dimension();
        let qubit_bit = n - 1 - qubit;
        let qubit_mask = 1usize << qubit_bit;

        let mut rho_00 = Complex64::new(0.0, 0.0);
        let mut rho_01 = Complex64::new(0.0, 0.0);
        let mut rho_10 = Complex64::new(0.0, 0.0);
        let mut rho_11 = Complex64::new(0.0, 0.0);

        for i in 0..dim {
            let amp_i = state.amplitude(i);
            for j in 0..dim {
                let amp_j = state.amplitude(j);
                let other_bits_i = i & !qubit_mask;
                let other_bits_j = j & !qubit_mask;

                if other_bits_i == other_bits_j {
                    let bit_i = (i & qubit_mask) != 0;
                    let bit_j = (j & qubit_mask) != 0;
                    let contrib = amp_i * amp_j.conj();

                    match (bit_i, bit_j) {
                        (false, false) => rho_00 += contrib,
                        (false, true) => rho_01 += contrib,
                        (true, false) => rho_10 += contrib,
                        (true, true) => rho_11 += contrib,
                    }
                }
            }
        }

        let x = 2.0 * rho_01.re;
        let y = 2.0 * rho_01.im;
        let z = rho_00.re - rho_11.re;

        let r = (x * x + y * y + z * z).sqrt();
        let theta = if r > 1e-10 { z.acos() } else { 0.0 };
        let phi = y.atan2(x);

        let purity = (rho_00.norm_sqr() + rho_11.norm_sqr() + 2.0 * rho_01.norm_sqr()).sqrt();

        Self {
            qubit_index: qubit,
            x,
            y,
            z,
            theta,
            phi,
            purity: purity.min(1.0),
        }
    }

    pub fn from_angles(qubit_index: usize, theta: f64, phi: f64) -> Self {
        let x = theta.sin() * phi.cos();
        let y = theta.sin() * phi.sin();
        let z = theta.cos();

        Self {
            qubit_index,
            x,
            y,
            z,
            theta,
            phi,
            purity: 1.0,
        }
    }

    pub fn north_pole(qubit_index: usize) -> Self {
        Self {
            qubit_index,
            x: 0.0,
            y: 0.0,
            z: 1.0,
            theta: 0.0,
            phi: 0.0,
            purity: 1.0,
        }
    }

    pub fn south_pole(qubit_index: usize) -> Self {
        Self {
            qubit_index,
            x: 0.0,
            y: 0.0,
            z: -1.0,
            theta: std::f64::consts::PI,
            phi: 0.0,
            purity: 1.0,
        }
    }

    pub fn equator(qubit_index: usize, phi: f64) -> Self {
        Self::from_angles(qubit_index, std::f64::consts::FRAC_PI_2, phi)
    }

    pub fn is_pure(&self) -> bool {
        self.purity > 0.99
    }

    pub fn bloch_vector_length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

// =============================================================================
// 3. StateHistogramData - Measurement histogram
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateHistogramData {
    pub id: Uuid,
    pub bitstring_probabilities: HashMap<String, f64>,
    pub sorted_bitstrings: Vec<(String, f64)>,
    pub max_probability: f64,
}

impl StateHistogramData {
    pub fn from_state(state: &QuantumStateVector) -> Self {
        let probs = state.probability_distribution();
        let n = state.number_of_quantum_bits();

        let mut bitstring_probs: HashMap<String, f64> = HashMap::new();
        for (i, &prob) in probs.iter().enumerate() {
            if prob > 1e-10 {
                let bitstring: String = (0..n)
                    .rev()
                    .map(|bit| if (i >> bit) & 1 == 0 { '0' } else { '1' })
                    .collect();
                bitstring_probs.insert(bitstring, prob);
            }
        }

        let mut sorted: Vec<(String, f64)> = bitstring_probs
            .iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let max_prob = sorted.first().map(|(_, p)| *p).unwrap_or(0.0);

        Self {
            id: Uuid::new_v4(),
            bitstring_probabilities: bitstring_probs,
            sorted_bitstrings: sorted,
            max_probability: max_prob,
        }
    }

    pub fn from_statistics(stats: &MeasurementStatistics) -> Self {
        let mut sorted: Vec<(String, f64)> = stats
            .probabilities
            .iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let max_prob = sorted.first().map(|(_, p)| *p).unwrap_or(0.0);

        Self {
            id: Uuid::new_v4(),
            bitstring_probabilities: stats.probabilities.clone(),
            sorted_bitstrings: sorted,
            max_probability: max_prob,
        }
    }

    pub fn top_n(&self, n: usize) -> Vec<(&String, f64)> {
        self.sorted_bitstrings
            .iter()
            .take(n)
            .map(|(s, p)| (s, *p))
            .collect()
    }
}

// =============================================================================
// Entanglement and Time Series Data
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntanglementGraphData {
    pub nodes: Vec<QubitNode>,
    pub edges: Vec<EntanglementEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QubitNode {
    pub qubit_index: usize,
    pub label: String,
    pub state_purity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntanglementEdge {
    pub qubit_a: usize,
    pub qubit_b: usize,
    pub entanglement_strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub metric_name: String,
    pub timestamps: Vec<u64>,
    pub values: Vec<f64>,
}

impl TimeSeriesData {
    pub fn new(metric_name: impl Into<String>) -> Self {
        Self {
            metric_name: metric_name.into(),
            timestamps: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn add_point(&mut self, timestamp: u64, value: f64) {
        self.timestamps.push(timestamp);
        self.values.push(value);
    }

    pub fn latest_value(&self) -> Option<f64> {
        self.values.last().copied()
    }
}

// =============================================================================
// 4. VisualizationRenderer - Abstract renderer interface
// =============================================================================

pub trait VisualizationRenderer: Send + Sync {
    fn render_bloch_sphere(&self, coords: &BlochSphereCoordinates) -> RenderOutput;
    fn render_histogram(&self, histogram: &StateHistogramData) -> RenderOutput;
    fn render_packet(&self, packet: &VisualizationDataPacket) -> RenderOutput;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOutput {
    pub format: RenderFormat,
    pub data: Vec<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RenderFormat {
    Svg,
    Png,
    Json,
    Ascii,
}

pub struct AsciiRenderer;

impl VisualizationRenderer for AsciiRenderer {
    fn render_bloch_sphere(&self, coords: &BlochSphereCoordinates) -> RenderOutput {
        let output = format!(
            "Qubit {}: θ={:.2}rad φ={:.2}rad\n  x={:.3} y={:.3} z={:.3}\n  Purity: {:.2}%",
            coords.qubit_index,
            coords.theta,
            coords.phi,
            coords.x,
            coords.y,
            coords.z,
            coords.purity * 100.0
        );

        RenderOutput {
            format: RenderFormat::Ascii,
            data: output.into_bytes(),
            width: None,
            height: None,
        }
    }

    fn render_histogram(&self, histogram: &StateHistogramData) -> RenderOutput {
        let mut lines = Vec::new();
        lines.push("State Histogram:".to_string());

        for (bitstring, prob) in histogram.sorted_bitstrings.iter().take(10) {
            let bar_width = (prob / histogram.max_probability * 40.0) as usize;
            let bar: String = "█".repeat(bar_width);
            lines.push(format!("  |{}⟩ {:5.2}% {}", bitstring, prob * 100.0, bar));
        }

        let output = lines.join("\n");

        RenderOutput {
            format: RenderFormat::Ascii,
            data: output.into_bytes(),
            width: None,
            height: None,
        }
    }

    fn render_packet(&self, packet: &VisualizationDataPacket) -> RenderOutput {
        let mut parts = Vec::new();

        for bloch in &packet.bloch_spheres {
            let rendered = self.render_bloch_sphere(bloch);
            parts.push(String::from_utf8_lossy(&rendered.data).to_string());
        }

        if let Some(ref hist) = packet.histogram {
            let rendered = self.render_histogram(hist);
            parts.push(String::from_utf8_lossy(&rendered.data).to_string());
        }

        let output = parts.join("\n\n");

        RenderOutput {
            format: RenderFormat::Ascii,
            data: output.into_bytes(),
            width: None,
            height: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloch_sphere_pure_state() {
        let state = QuantumStateVector::zero_state(1);
        let bloch = BlochSphereCoordinates::from_reduced_density_matrix(&state, 0);
        assert!((bloch.z - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_histogram_from_state() {
        let state = QuantumStateVector::zero_state(2);
        let histogram = StateHistogramData::from_state(&state);
        assert!(histogram.bitstring_probabilities.contains_key("00"));
    }

    #[test]
    fn test_ascii_renderer() {
        let bloch = BlochSphereCoordinates::north_pole(0);
        let renderer = AsciiRenderer;
        let output = renderer.render_bloch_sphere(&bloch);
        assert_eq!(output.format, RenderFormat::Ascii);
    }
}
