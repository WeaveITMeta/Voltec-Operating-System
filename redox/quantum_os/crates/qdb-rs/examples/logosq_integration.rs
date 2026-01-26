//! LogosQ Integration Examples
//!
//! Demonstrates the LogosQ ↔ qft-rs ↔ qdb-rs data flow for VQE workflows.
//! Run with: cargo run --example logosq_integration --features logosq

use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;

/// Example: Storing VQE results from a LogosQ simulation
/// 
/// This demonstrates the complete data flow:
/// 1. LogosQ runs VQE optimization
/// 2. Final MPS state is converted to QftFile
/// 3. QftFile is stored in QDB with metadata
/// 4. State can be retrieved and analyzed later
fn main() -> Result<()> {
    println!("=== LogosQ Integration Examples ===\n");

    let db = QDB::new(MemoryBackend::new());

    // =========================================================================
    // 1. Simulated VQE Result Storage
    // =========================================================================
    println!("1. VQE Result Storage (Simulated LogosQ Output)");

    // In a real workflow, this would come from LogosQ:
    // ```rust
    // use logosq::vqe::{VqeSolver, Hamiltonian};
    // use logosq::ansatz::UCCSD;
    // 
    // let hamiltonian = Hamiltonian::from_molecule("H2", 0.74);
    // let ansatz = UCCSD::new(4, 2);
    // let solver = VqeSolver::new(hamiltonian, ansatz);
    // let result = solver.run(100)?;
    // let mps_state = result.final_state();
    // ```

    // Simulated: Create a state representing VQE ground state
    let mut vqe_state = QftBuilder::new(4)
        .bond_dimension(64)
        .golay(true)
        .truncation_threshold(1e-12)
        .metadata("algorithm", "VQE")
        .metadata("ansatz", "UCCSD")
        .metadata("molecule", "H2")
        .metadata("bond_length_angstrom", "0.74")
        .metadata("basis_set", "STO-3G")
        .metadata("energy_hartree", "-1.1372838")
        .metadata("iterations", "47")
        .metadata("converged", "true")
        .metadata("timestamp", "2026-01-26T14:30:00Z")
        .build()?;

    // Set to approximate H2 ground state (simplified)
    // Real state would come from MPS conversion
    let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    vqe_state.set_amplitude(0, Complex64::new(sqrt2_inv, 0.0))?;
    vqe_state.set_amplitude(15, Complex64::new(sqrt2_inv, 0.0))?;

    db.put_quantum("vqe:h2:sto3g:0.74", vqe_state)?;
    println!("   Stored VQE ground state for H2");

    // =========================================================================
    // 2. Potential Energy Surface Scan Storage
    // =========================================================================
    println!("\n2. Potential Energy Surface Scan");

    let bond_lengths = vec![0.5, 0.6, 0.7, 0.74, 0.8, 0.9, 1.0, 1.5, 2.0];
    let energies = vec![-0.85, -1.02, -1.12, -1.137, -1.13, -1.08, -1.02, -0.75, -0.55];

    for (r, e) in bond_lengths.iter().zip(energies.iter()) {
        let state = QftBuilder::new(4)
            .bond_dimension(64)
            .golay(true)
            .metadata("molecule", "H2")
            .metadata("bond_length", &r.to_string())
            .metadata("energy", &e.to_string())
            .build()?;

        let key = format!("pes:h2:{:.2}", r);
        db.put_quantum(&key, state)?;
    }

    println!("   Stored {} PES points", bond_lengths.len());

    // =========================================================================
    // 3. Retrieve and Analyze
    // =========================================================================
    println!("\n3. Retrieve and Analyze Stored States");

    if let Some(entry) = db.get_entry("vqe:h2:sto3g:0.74")? {
        println!("   Key: vqe:h2:sto3g:0.74");
        println!("   Entry type: {:?}", entry.entry_type);
        println!("   Metadata:");
        for (k, v) in &entry.metadata {
            println!("      {}: {}", k, v);
        }

        if let Some(qft) = entry.value.as_quantum() {
            println!("   State properties:");
            println!("      Qubits: {}", qft.num_qubits());
            println!("      Dimension: {}", qft.dimension());
            println!("      Norm: {:.10}", qft.norm());
            println!("      Bond dimension: {}", qft.config().bond_dimension);
        }
    }

    // =========================================================================
    // 4. MPS Bridge Pattern (Conceptual)
    // =========================================================================
    println!("\n4. MPS Bridge Pattern (LogosQ → QFT → QDB)");
    println!("   
   // Conceptual flow for LogosQ integration:
   //
   // use logosq::mps::MpsState;
   // use qft::QftFile;
   //
   // // From LogosQ VQE result
   // let mps: MpsState = vqe_result.final_state();
   //
   // // Convert MPS tensors to QftFile
   // // Option A: Direct amplitude extraction
   // let amplitudes = mps.to_state_vector();
   // let qft = QftFile::from_amplitudes(amplitudes)?;
   //
   // // Option B: MPS tensor serialization (preserves structure)
   // let qft = QftFile::from_mps(&mps, QftConfig {{
   //     bond_dimension: mps.bond_dimension(),
   //     golay_enabled: true,
   //     truncation_threshold: 1e-12,
   // }})?;
   //
   // // Store in QDB
   // db.put_quantum(\"vqe:result\", qft)?;
   //
   // // Later retrieval
   // let loaded = db.get_quantum(\"vqe:result\")?.unwrap();
   // let mps_restored = loaded.to_mps()?;
   ");

    // =========================================================================
    // 5. Batch VQE Experiment Storage
    // =========================================================================
    println!("5. Batch VQE Experiment Storage");

    let experiments = vec![
        ("LiH", 4, -7.882),
        ("BeH2", 6, -15.835),
        ("H2O", 8, -75.012),
    ];

    for (molecule, qubits, energy) in experiments {
        let state = QftBuilder::new(qubits)
            .bond_dimension(128)
            .golay(true)
            .metadata("molecule", molecule)
            .metadata("energy", &energy.to_string())
            .metadata("method", "VQE-UCCSD")
            .build()?;

        let key = format!("vqe:{}", molecule.to_lowercase());
        db.put_quantum(&key, state)?;
        println!("   Stored {}: {} qubits, E = {:.3} Ha", molecule, qubits, energy);
    }

    println!("\n   Total quantum states in database: {}", 
        db.keys()?.iter().filter(|k| k.to_string().starts_with("vqe:") || k.to_string().starts_with("pes:")).count());

    println!("\n=== LogosQ Integration Complete ===");
    Ok(())
}
