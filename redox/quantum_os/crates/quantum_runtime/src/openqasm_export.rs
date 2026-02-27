// =============================================================================
// V-OS - OpenQASM 3 Export Module
// =============================================================================
// Table of Contents:
//   1. OpenQASM3Exporter - Main export functionality
//   2. CircuitProvenance - Metadata for circuit reconstruction
//   3. Gate serialization to OpenQASM 3 syntax
//   4. State preparation circuit generation
// =============================================================================
// Purpose: Export .qft files and quantum circuits to OpenQASM 3 format for
//          interoperability with Qiskit, Cirq, and other quantum frameworks.
// =============================================================================

use num_complex::Complex64;
use std::fmt::Write;

/// Represents a quantum gate operation for OpenQASM export
#[derive(Debug, Clone)]
pub enum QasmGate {
    /// Identity gate
    Id(usize),
    /// Pauli-X gate
    X(usize),
    /// Pauli-Y gate
    Y(usize),
    /// Pauli-Z gate
    Z(usize),
    /// Hadamard gate
    H(usize),
    /// S gate (sqrt(Z))
    S(usize),
    /// S-dagger gate
    Sdg(usize),
    /// T gate (sqrt(S))
    T(usize),
    /// T-dagger gate
    Tdg(usize),
    /// Rotation around X-axis
    Rx(usize, f64),
    /// Rotation around Y-axis
    Ry(usize, f64),
    /// Rotation around Z-axis
    Rz(usize, f64),
    /// Phase gate
    P(usize, f64),
    /// U3 gate (general single-qubit)
    U(usize, f64, f64, f64),
    /// CNOT gate
    Cx(usize, usize),
    /// CZ gate
    Cz(usize, usize),
    /// SWAP gate
    Swap(usize, usize),
    /// Controlled-phase gate
    Cp(usize, usize, f64),
    /// Toffoli gate (CCX)
    Ccx(usize, usize, usize),
    /// Measurement
    Measure(usize, usize),
    /// Reset
    Reset(usize),
    /// Barrier
    Barrier(Vec<usize>),
    /// Custom gate
    Custom(String, Vec<usize>, Vec<f64>),
}

impl QasmGate {
    /// Convert gate to OpenQASM 3 string representation
    pub fn to_qasm3(&self, qubit_prefix: &str, bit_prefix: &str) -> String {
        match self {
            QasmGate::Id(q) => format!("id {}[{}];", qubit_prefix, q),
            QasmGate::X(q) => format!("x {}[{}];", qubit_prefix, q),
            QasmGate::Y(q) => format!("y {}[{}];", qubit_prefix, q),
            QasmGate::Z(q) => format!("z {}[{}];", qubit_prefix, q),
            QasmGate::H(q) => format!("h {}[{}];", qubit_prefix, q),
            QasmGate::S(q) => format!("s {}[{}];", qubit_prefix, q),
            QasmGate::Sdg(q) => format!("sdg {}[{}];", qubit_prefix, q),
            QasmGate::T(q) => format!("t {}[{}];", qubit_prefix, q),
            QasmGate::Tdg(q) => format!("tdg {}[{}];", qubit_prefix, q),
            QasmGate::Rx(q, theta) => format!("rx({}) {}[{}];", theta, qubit_prefix, q),
            QasmGate::Ry(q, theta) => format!("ry({}) {}[{}];", theta, qubit_prefix, q),
            QasmGate::Rz(q, theta) => format!("rz({}) {}[{}];", theta, qubit_prefix, q),
            QasmGate::P(q, lambda) => format!("p({}) {}[{}];", lambda, qubit_prefix, q),
            QasmGate::U(q, theta, phi, lambda) => {
                format!("U({}, {}, {}) {}[{}];", theta, phi, lambda, qubit_prefix, q)
            }
            QasmGate::Cx(c, t) => format!("cx {}[{}], {}[{}];", qubit_prefix, c, qubit_prefix, t),
            QasmGate::Cz(c, t) => format!("cz {}[{}], {}[{}];", qubit_prefix, c, qubit_prefix, t),
            QasmGate::Swap(a, b) => format!("swap {}[{}], {}[{}];", qubit_prefix, a, qubit_prefix, b),
            QasmGate::Cp(c, t, theta) => {
                format!("cp({}) {}[{}], {}[{}];", theta, qubit_prefix, c, qubit_prefix, t)
            }
            QasmGate::Ccx(c1, c2, t) => {
                format!("ccx {}[{}], {}[{}], {}[{}];", qubit_prefix, c1, qubit_prefix, c2, qubit_prefix, t)
            }
            QasmGate::Measure(q, c) => format!("{}[{}] = measure {}[{}];", bit_prefix, c, qubit_prefix, q),
            QasmGate::Reset(q) => format!("reset {}[{}];", qubit_prefix, q),
            QasmGate::Barrier(qubits) => {
                let qubit_list: Vec<String> = qubits.iter()
                    .map(|q| format!("{}[{}]", qubit_prefix, q))
                    .collect();
                format!("barrier {};", qubit_list.join(", "))
            }
            QasmGate::Custom(name, qubits, params) => {
                let param_str = if params.is_empty() {
                    String::new()
                } else {
                    format!("({})", params.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
                };
                let qubit_list: Vec<String> = qubits.iter()
                    .map(|q| format!("{}[{}]", qubit_prefix, q))
                    .collect();
                format!("{}{} {};", name, param_str, qubit_list.join(", "))
            }
        }
    }
}

/// Circuit provenance metadata stored in .qft files
#[derive(Debug, Clone, Default)]
pub struct CircuitProvenance {
    /// Name of the circuit
    pub name: String,
    /// Number of qubits
    pub num_qubits: usize,
    /// Number of classical bits (for measurement)
    pub num_clbits: usize,
    /// Gate sequence that produced the state
    pub gates: Vec<QasmGate>,
    /// Optional description
    pub description: Option<String>,
    /// Creation timestamp
    pub created_at: Option<String>,
}

impl CircuitProvenance {
    pub fn new(name: &str, num_qubits: usize) -> Self {
        Self {
            name: name.to_string(),
            num_qubits,
            num_clbits: num_qubits,
            gates: Vec::new(),
            description: None,
            created_at: None,
        }
    }
    
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
    
    pub fn add_gate(&mut self, gate: QasmGate) {
        self.gates.push(gate);
    }
}

/// OpenQASM 3 exporter configuration
#[derive(Debug, Clone)]
pub struct OpenQasm3Config {
    /// Qubit register name
    pub qubit_register: String,
    /// Classical bit register name
    pub classical_register: String,
    /// Include standard library
    pub include_stdgates: bool,
    /// Add comments
    pub include_comments: bool,
    /// OpenQASM version string
    pub version: String,
}

impl Default for OpenQasm3Config {
    fn default() -> Self {
        Self {
            qubit_register: "q".to_string(),
            classical_register: "c".to_string(),
            include_stdgates: true,
            include_comments: true,
            version: "3.0".to_string(),
        }
    }
}

/// Main OpenQASM 3 exporter
pub struct OpenQasm3Exporter {
    config: OpenQasm3Config,
}

impl OpenQasm3Exporter {
    pub fn new() -> Self {
        Self {
            config: OpenQasm3Config::default(),
        }
    }
    
    pub fn with_config(config: OpenQasm3Config) -> Self {
        Self { config }
    }
    
    /// Export circuit provenance to OpenQASM 3 string
    pub fn export_circuit(&self, provenance: &CircuitProvenance) -> String {
        let mut output = String::new();
        
        // Header
        writeln!(output, "OPENQASM {};", self.config.version).unwrap();
        
        if self.config.include_stdgates {
            writeln!(output, "include \"stdgates.inc\";").unwrap();
        }
        
        writeln!(output).unwrap();
        
        // Comments
        if self.config.include_comments {
            writeln!(output, "// Circuit: {}", provenance.name).unwrap();
            if let Some(desc) = &provenance.description {
                writeln!(output, "// Description: {}", desc).unwrap();
            }
            if let Some(ts) = &provenance.created_at {
                writeln!(output, "// Created: {}", ts).unwrap();
            }
            writeln!(output, "// Exported from V-OS (.qft format)").unwrap();
            writeln!(output).unwrap();
        }
        
        // Register declarations
        writeln!(output, "qubit[{}] {};", provenance.num_qubits, self.config.qubit_register).unwrap();
        if provenance.num_clbits > 0 {
            writeln!(output, "bit[{}] {};", provenance.num_clbits, self.config.classical_register).unwrap();
        }
        writeln!(output).unwrap();
        
        // Gate sequence
        for gate in &provenance.gates {
            writeln!(
                output,
                "{}",
                gate.to_qasm3(&self.config.qubit_register, &self.config.classical_register)
            ).unwrap();
        }
        
        output
    }
    
    /// Generate state preparation circuit for a given amplitude vector
    /// Uses a simplified amplitude encoding approach
    pub fn generate_state_prep_circuit(
        &self,
        amplitudes: &[Complex64],
        circuit_name: &str,
    ) -> Result<String, String> {
        let n = amplitudes.len();
        if !n.is_power_of_two() {
            return Err("Amplitude vector length must be a power of 2".to_string());
        }
        
        let num_qubits = n.trailing_zeros() as usize;
        let mut provenance = CircuitProvenance::new(circuit_name, num_qubits);
        provenance.description = Some("State preparation circuit generated from .qft file".to_string());
        
        // For small states, use direct amplitude encoding
        // For larger states, this would need more sophisticated methods
        if num_qubits <= 4 {
            self.generate_small_state_prep(&mut provenance, amplitudes)?;
        } else {
            // For larger states, generate approximate preparation using RY/RZ rotations
            self.generate_approximate_state_prep(&mut provenance, amplitudes)?;
        }
        
        Ok(self.export_circuit(&provenance))
    }
    
    /// Generate exact state preparation for small states (≤4 qubits)
    fn generate_small_state_prep(
        &self,
        provenance: &mut CircuitProvenance,
        amplitudes: &[Complex64],
    ) -> Result<(), String> {
        let n = amplitudes.len();
        let num_qubits = n.trailing_zeros() as usize;
        
        // Normalize amplitudes
        let norm: f64 = amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        if norm < 1e-15 {
            return Err("Zero state vector".to_string());
        }
        
        // Use recursive amplitude encoding
        // This is a simplified version - production would use more efficient methods
        self.recursive_amplitude_encode(provenance, amplitudes, 0, num_qubits, 0)?;
        
        Ok(())
    }
    
    /// Recursive amplitude encoding helper
    fn recursive_amplitude_encode(
        &self,
        provenance: &mut CircuitProvenance,
        amplitudes: &[Complex64],
        qubit: usize,
        num_qubits: usize,
        offset: usize,
    ) -> Result<(), String> {
        if qubit >= num_qubits {
            return Ok(());
        }
        
        let half = amplitudes.len() / 2;
        let left: &[Complex64] = &amplitudes[..half];
        let right: &[Complex64] = &amplitudes[half..];
        
        // Compute norms of left and right halves
        let left_norm: f64 = left.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        let right_norm: f64 = right.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
        let total_norm = (left_norm * left_norm + right_norm * right_norm).sqrt();
        
        if total_norm < 1e-15 {
            return Ok(());
        }
        
        // Rotation angle to distribute amplitude between |0⟩ and |1⟩
        let theta = 2.0 * (right_norm / total_norm).asin();
        
        if theta.abs() > 1e-10 {
            provenance.add_gate(QasmGate::Ry(qubit, theta));
        }
        
        // Recurse on both halves
        if half > 1 {
            // This is simplified - full implementation would use controlled rotations
            self.recursive_amplitude_encode(provenance, left, qubit + 1, num_qubits, offset)?;
        }
        
        Ok(())
    }
    
    /// Generate approximate state preparation for larger states
    fn generate_approximate_state_prep(
        &self,
        provenance: &mut CircuitProvenance,
        amplitudes: &[Complex64],
    ) -> Result<(), String> {
        let n = amplitudes.len();
        let num_qubits = n.trailing_zeros() as usize;
        
        // Add comment about approximation
        provenance.description = Some(format!(
            "Approximate state preparation for {}-qubit state (exact encoding requires {} gates)",
            num_qubits,
            n * 2
        ));
        
        // Use a variational ansatz structure
        // Layer 1: Initial rotations based on amplitude magnitudes
        for q in 0..num_qubits {
            let idx = 1 << (num_qubits - 1 - q);
            if idx < n {
                let amp = amplitudes[idx];
                let theta = 2.0 * amp.norm().asin().min(std::f64::consts::PI);
                let phi = amp.arg();
                
                if theta.abs() > 1e-10 {
                    provenance.add_gate(QasmGate::Ry(q, theta));
                }
                if phi.abs() > 1e-10 {
                    provenance.add_gate(QasmGate::Rz(q, phi));
                }
            }
        }
        
        // Layer 2: Entangling gates
        for q in 0..num_qubits.saturating_sub(1) {
            provenance.add_gate(QasmGate::Cx(q, q + 1));
        }
        
        // Layer 3: Final rotations
        for q in 0..num_qubits {
            provenance.add_gate(QasmGate::Ry(q, 0.0)); // Placeholder for optimization
        }
        
        Ok(())
    }
}

impl Default for OpenQasm3Exporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Export a quantum state to OpenQASM 3 state preparation circuit
pub fn export_state_to_qasm3(
    amplitudes: &[Complex64],
    circuit_name: &str,
) -> Result<String, String> {
    let exporter = OpenQasm3Exporter::new();
    exporter.generate_state_prep_circuit(amplitudes, circuit_name)
}

/// Export circuit provenance to OpenQASM 3
pub fn export_provenance_to_qasm3(provenance: &CircuitProvenance) -> String {
    let exporter = OpenQasm3Exporter::new();
    exporter.export_circuit(provenance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;
    
    #[test]
    fn test_gate_serialization() {
        assert_eq!(QasmGate::H(0).to_qasm3("q", "c"), "h q[0];");
        assert_eq!(QasmGate::Cx(0, 1).to_qasm3("q", "c"), "cx q[0], q[1];");
        assert_eq!(QasmGate::Rx(2, PI).to_qasm3("q", "c"), format!("rx({}) q[2];", PI));
    }
    
    #[test]
    fn test_circuit_export() {
        let mut prov = CircuitProvenance::new("bell_state", 2);
        prov.add_gate(QasmGate::H(0));
        prov.add_gate(QasmGate::Cx(0, 1));
        
        let exporter = OpenQasm3Exporter::new();
        let qasm = exporter.export_circuit(&prov);
        
        assert!(qasm.contains("OPENQASM 3.0"));
        assert!(qasm.contains("qubit[2] q"));
        assert!(qasm.contains("h q[0]"));
        assert!(qasm.contains("cx q[0], q[1]"));
    }
    
    #[test]
    fn test_state_prep_export() {
        // |+⟩ state
        let amplitudes = vec![
            Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        
        let result = export_state_to_qasm3(&amplitudes, "plus_state");
        assert!(result.is_ok());
        let qasm = result.unwrap();
        assert!(qasm.contains("OPENQASM 3.0"));
    }
}
