// =============================================================================
// V-OS - Bell State Demo
// =============================================================================
// Table of Contents:
//   1. Circuit construction with full snake_case naming
//   2. Execution and measurement stream
//   3. Semantic analysis with eustress layer
//   4. Visualization output
// =============================================================================
// Purpose: Demonstrates complete workflow from circuit creation to visualization
//          using the V-OS architecture with LogosQ integration.
// =============================================================================

use quantum_runtime::prelude::*;
use quantum_runtime::circuit_program::QuantumCircuitStructure;
use quantum_runtime::execution::QuantumExecutionEngine;
use quantum_runtime::measurement::{PauliZObservable, MeasurementStatistics};
use quantum_runtime::state_backend::QuantumStateVector;

use eustress_semantics::{SemanticInferenceEngine, BasisFrame, MeaningPacket};
use ux_visualization::{VisualizationDataPacket, AsciiRenderer, VisualizationRenderer};
use quantum_device_abstraction::{DeviceRegistry, SimulatorDevice, QuantumDeviceInterface};
use kernel_services::handle::QuantumDeviceHandle;
use kernel_services::task::{HybridScheduler, QuantumJobContext, ProcessContext};
use kernel_services::message::CircuitProgramMessage;

use std::sync::Arc;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║       V-OS - Bell State Demonstration            ║");
    println!("║                  LogosQ Integration Example                       ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // =========================================================================
    // 1. Create Bell State Circuit using full snake_case naming
    // =========================================================================
    println!("📐 Step 1: Constructing quantum_circuit_structure (Bell State)");
    println!("   - Apply hadamard_gate to qubit 0");
    println!("   - Apply controlled_not_gate with control=0, target=1");
    println!();

    let mut quantum_circuit = QuantumCircuitStructure::new(2);
    quantum_circuit.apply_hadamard_gate(0);
    quantum_circuit.apply_controlled_not_gate(0, 1);

    println!("   Circuit ID: {}", quantum_circuit.id());
    println!("   Number of quantum bits: {}", quantum_circuit.number_of_quantum_bits());
    println!("   Gate count: {}", quantum_circuit.gate_count());
    println!();

    // =========================================================================
    // 2. Execute circuit and get measurement stream
    // =========================================================================
    println!("⚡ Step 2: Executing circuit with quantum_execution_engine");
    println!();

    let execution_engine = QuantumExecutionEngine::new();
    let measurement_stream = execution_engine.execute_and_measure(&quantum_circuit, 1000);

    println!("   Total shots: {}", measurement_stream.total_shots());
    println!("   Completed shots: {}", measurement_stream.completed_shots());
    println!("   Stream complete: {}", measurement_stream.is_complete());
    println!();

    // =========================================================================
    // 3. Analyze measurement statistics
    // =========================================================================
    println!("📊 Step 3: Computing measurement_statistics");
    println!();

    let statistics = measurement_stream.compute_statistics();

    println!("   Entropy: {:.4} bits", statistics.entropy);
    println!("   Probability distribution:");
    for (bitstring, prob) in &statistics.probabilities {
        let bar_len = (prob * 50.0) as usize;
        let bar: String = "█".repeat(bar_len);
        println!("     |{}⟩: {:.2}% {}", bitstring, prob * 100.0, bar);
    }
    println!();

    // =========================================================================
    // 4. Semantic inference with eustress layer
    // =========================================================================
    println!("🧠 Step 4: Semantic analysis with eustress_semantics layer");
    println!();

    let mut semantic_engine = SemanticInferenceEngine::new();
    let meaning_packet = semantic_engine.analyze_measurement(
        &statistics,
        "bell_state_measurement",
        2,
    );

    println!("   Meaning Packet ID: {}", meaning_packet.id);
    println!("   Observable: {}", meaning_packet.observable_operator.name);
    println!("   Basis Frame: {}", meaning_packet.basis_frame.name);
    println!("   Statistics Window:");
    println!("     - Entropy: {:.4}", meaning_packet.statistics_window.entropy);
    println!("     - Purity: {:.4}", meaning_packet.statistics_window.purity);
    if let Some(ref interpretation) = meaning_packet.interpretation {
        println!("   Interpretation: {}", interpretation);
    }
    println!();

    // =========================================================================
    // 5. Compute expectation values
    // =========================================================================
    println!("📈 Step 5: Computing expectation values");
    println!();

    let observable_z0 = PauliZObservable::new(0);
    let observable_z1 = PauliZObservable::new(1);

    let expectation_z0 = execution_engine.compute_expectation_value(&quantum_circuit, &observable_z0);
    let expectation_z1 = execution_engine.compute_expectation_value(&quantum_circuit, &observable_z1);

    println!("   ⟨Z₀⟩ = {:.4}", expectation_z0);
    println!("   ⟨Z₁⟩ = {:.4}", expectation_z1);
    println!("   Note: For Bell state |Φ⁺⟩, both should be ≈ 0");
    println!();

    // =========================================================================
    // 6. Visualize with ux_visualization layer
    // =========================================================================
    println!("🎨 Step 6: Generating visualization with ux_visualization layer");
    println!();

    let mut state = QuantumStateVector::zero_state(2);
    for gate in quantum_circuit.gate_application_instances() {
        gate.quantum_gate_interface.apply_to_full_state_vector(&mut state);
    }

    let viz_packet = VisualizationDataPacket::from_state(&state);
    let renderer = AsciiRenderer;
    let render_output = renderer.render_packet(&viz_packet);

    println!("{}", String::from_utf8_lossy(&render_output.data));
    println!();

    // =========================================================================
    // 7. Device abstraction demonstration
    // =========================================================================
    println!("🔧 Step 7: Using quantum_device_abstraction layer");
    println!();

    let registry = DeviceRegistry::new();
    registry.create_default_simulators();

    let available_devices = registry.list_available_devices();
    println!("   Available quantum devices:");
    for (id, name, qubits) in &available_devices {
        println!("     - {} ({} qubits) [{}]", name, qubits, id);
    }
    println!();

    if let Some(device) = registry.find_suitable_device(2) {
        println!("   Selected device: {}", device.device_name());
        
        let job_id = device.submit_circuit(&quantum_circuit, 100).unwrap();
        println!("   Submitted job: {}", job_id);
        
        if let Ok(Some(results)) = device.get_results(job_id) {
            println!("   Results received: {} measurements", results.completed_shots());
        }
    }
    println!();

    // =========================================================================
    // 8. Kernel services demonstration
    // =========================================================================
    println!("⚙️  Step 8: Using kernel_services layer");
    println!();

    let device_handle = QuantumDeviceHandle::new_simulator("demo_device", 4, false);
    println!("   Created QuantumDeviceHandle:");
    println!("     - Device: {}", device_handle.device_name());
    println!("     - Qubits: {}", device_handle.number_of_quantum_bits());
    println!("     - Backend: {:?}", device_handle.backend_type());
    println!();

    let scheduler = HybridScheduler::new(2);
    let process = ProcessContext::new("quantum_demo_process");
    scheduler.enqueue_process(process.clone());
    println!("   Process enqueued: {} (ID: {})", process.name, process.id);
    println!();

    // =========================================================================
    // Summary
    // =========================================================================
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                       Demo Complete                               ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  ✓ quantum_circuit_structure created with snake_case naming      ║");
    println!("║  ✓ Executed on quantum_execution_engine                          ║");
    println!("║  ✓ measurement_stream collected 1000 shots                       ║");
    println!("║  ✓ eustress_semantics generated meaning_packet                   ║");
    println!("║  ✓ ux_visualization rendered Bloch spheres and histogram         ║");
    println!("║  ✓ quantum_device_abstraction managed simulators                 ║");
    println!("║  ✓ kernel_services handled scheduling and capabilities           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
