// =============================================================================
// MACROHARD Quantum OS - Quantum File Type (.qft) Demo
// =============================================================================
// Table of Contents:
//   1. Basic Storage and Retrieval
//   2. Golay Error Correction Demo
//   3. Bond Dimension Optimization
//   4. Benchmark Different State Types
// =============================================================================
// Purpose: Demonstrates the .qft file format for fault-tolerant quantum data
//          storage with MPS tensor network representation and Golay error
//          correction.
// =============================================================================
// Run with: cargo run --example qft_file_demo
// =============================================================================

use quantum_runtime::bond_optimization::{
    benchmark_bond_dimensions, find_optimal_bond_dim, standard_chi_values,
    BondDimensionOptimizer, EntanglementAnalyzer, OptimizationConfig,
};
use quantum_runtime::golay::{decode_golay, encode_golay, GolayDecoder, GolayEncoder};
use quantum_runtime::qft_format::{
    load_quantum_file, store_quantum_file, validate_qft_file, QftLoadConfig, QftMetadata,
    QftStorageConfig,
};
use quantum_runtime::state_backend::QuantumStateVector;
use num_complex::Complex64;
use std::path::Path;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     MACROHARD Quantum OS - Quantum File Type (.qft) Demo         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    // Demo 1: Basic Storage and Retrieval
    demo_basic_storage();

    // Demo 2: Golay Error Correction
    demo_golay_error_correction();

    // Demo 3: Bond Dimension Optimization
    demo_bond_optimization();

    // Demo 4: Benchmark Different State Types
    demo_state_benchmarks();

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("                        Demo Complete!                              ");
    println!("═══════════════════════════════════════════════════════════════════");
}

// =============================================================================
// Demo 1: Basic Storage and Retrieval
// =============================================================================

fn demo_basic_storage() {
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 1: Basic .qft Storage and Retrieval                        │");
    println!("└─────────────────────────────────────────────────────────────────┘");
    println!();

    // Create a simple quantum state (|00⟩ + |11⟩) / √2 - Bell state
    let mut amplitudes = vec![Complex64::new(0.0, 0.0); 4];
    amplitudes[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0); // |00⟩
    amplitudes[3] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0); // |11⟩
    let bell_state = QuantumStateVector::from_amplitudes(amplitudes);

    println!("  Created Bell state: (|00⟩ + |11⟩) / √2");
    println!("  Number of qubits: {}", bell_state.number_of_quantum_bits());
    println!();

    // Configure storage with metadata
    let metadata = QftMetadata {
        description: Some("Bell state (maximally entangled)".to_string()),
        source: Some("qft_file_demo".to_string()),
        tags: vec!["bell".to_string(), "entangled".to_string(), "demo".to_string()],
        ..Default::default()
    };

    let config = QftStorageConfig {
        enable_golay: true,
        metadata: Some(metadata),
        ..Default::default()
    };

    // Store to file
    let temp_path = std::env::temp_dir().join("bell_state.qft");
    println!("  Storing to: {}", temp_path.display());

    match store_quantum_file(&bell_state, &temp_path, config) {
        Ok(result) => {
            println!("  ✓ Storage successful!");
            println!("    - File size: {} bytes", result.file_size);
            println!("    - Bond dimension: {}", result.actual_bond_dim);
            println!("    - Compression ratio: {:.2}x", result.compression_ratio);
            println!("    - Golay blocks: {}", result.golay_blocks);
            println!("    - Estimated fidelity: {:.10}", result.estimated_fidelity);
        }
        Err(e) => {
            println!("  ✗ Storage failed: {}", e);
            return;
        }
    }
    println!();

    // Validate file
    println!("  Validating file...");
    match validate_qft_file(&temp_path) {
        Ok(validation) => {
            println!("  ✓ Validation: {}", if validation.is_valid { "PASSED" } else { "FAILED" });
            println!("    - Version: {}.{}", validation.version.0, validation.version.1);
            println!("    - Qubits: {}", validation.num_qubits);
            println!("    - Bond dim: {}", validation.bond_dim);
            println!("    - Checksum valid: {}", validation.checksum_valid);
            if validation.golay_errors_corrected > 0 {
                println!("    - Golay errors corrected: {}", validation.golay_errors_corrected);
            }
        }
        Err(e) => println!("  ✗ Validation failed: {}", e),
    }
    println!();

    // Load back
    println!("  Loading state from file...");
    let load_config = QftLoadConfig::default();
    match load_quantum_file(&temp_path, load_config) {
        Ok(result) => {
            println!("  ✓ Load successful!");
            println!("    - Loaded {} qubits", result.state.num_qubits());
            
            // Verify fidelity
            let loaded_dense = result.state.to_dense();
            let fidelity = bell_state.inner_product(&loaded_dense).norm_sqr();
            println!("    - Fidelity with original: {:.12}", fidelity);
            
            if let Some(meta) = result.metadata {
                if let Some(desc) = meta.description {
                    println!("    - Description: {}", desc);
                }
            }
        }
        Err(e) => println!("  ✗ Load failed: {}", e),
    }

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);
    println!();
}

// =============================================================================
// Demo 2: Golay Error Correction
// =============================================================================

fn demo_golay_error_correction() {
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 2: Golay [23,12,7] Error Correction                        │");
    println!("└─────────────────────────────────────────────────────────────────┘");
    println!();

    // Demonstrate single block encoding/decoding
    let mut encoder = GolayEncoder::new();
    let mut decoder = GolayDecoder::new();

    let message: u16 = 0b101010101010; // 12-bit message
    println!("  Original message: {:012b} (0x{:03X})", message, message);

    let codeword = encoder.encode(message).unwrap();
    println!("  Encoded codeword: {:023b} (23 bits)", codeword);
    println!();

    // Test error correction
    println!("  Testing error correction:");
    
    // No errors
    let decoded = decoder.decode(codeword).unwrap();
    println!("    - No errors:     {:012b} ✓", decoded);

    // 1 bit error
    let corrupted_1 = codeword ^ 0b1;
    let decoded_1 = decoder.decode(corrupted_1).unwrap();
    println!("    - 1 bit error:   {:012b} {}", decoded_1, if decoded_1 == message { "✓" } else { "✗" });

    // 2 bit errors
    let corrupted_2 = codeword ^ 0b11;
    let decoded_2 = decoder.decode(corrupted_2).unwrap();
    println!("    - 2 bit errors:  {:012b} {}", decoded_2, if decoded_2 == message { "✓" } else { "✗" });

    // 3 bit errors
    let corrupted_3 = codeword ^ 0b111;
    let decoded_3 = decoder.decode(corrupted_3).unwrap();
    println!("    - 3 bit errors:  {:012b} {}", decoded_3, if decoded_3 == message { "✓" } else { "✗" });

    println!();

    // Stream encoding demo
    println!("  Stream encoding demo:");
    let original_data = b"Hello, Quantum World!";
    println!("    Original: \"{}\" ({} bytes)", 
             String::from_utf8_lossy(original_data), original_data.len());

    let encoded = encode_golay(original_data).unwrap();
    println!("    Encoded: {} bytes (expansion: {:.2}x)", 
             encoded.len(), encoded.len() as f64 / original_data.len() as f64);

    // Introduce some errors
    let mut corrupted = encoded.clone();
    if corrupted.len() > 10 {
        corrupted[5] ^= 0x01;
        corrupted[10] ^= 0x02;
    }

    let (decoded_data, stats) = decode_golay(&corrupted).unwrap();
    println!("    Decoded: \"{}\"", String::from_utf8_lossy(&decoded_data[..original_data.len()]));
    println!("    Errors corrected: {}", stats.total_errors_corrected);
    println!("    Recovery rate: {:.2}%", stats.recovery_rate() * 100.0);

    println!();
}

// =============================================================================
// Demo 3: Bond Dimension Optimization
// =============================================================================

fn demo_bond_optimization() {
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 3: Bond Dimension Optimization                             │");
    println!("└─────────────────────────────────────────────────────────────────┘");
    println!();

    // Create different types of states
    let n = 4; // 4 qubits for demo

    // Product state |0000⟩
    let product_state = QuantumStateVector::zero_state(n);

    // GHZ state (|0000⟩ + |1111⟩) / √2
    let mut ghz_amps = vec![Complex64::new(0.0, 0.0); 1 << n];
    ghz_amps[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    ghz_amps[(1 << n) - 1] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    let ghz_state = QuantumStateVector::from_amplitudes(ghz_amps);

    // Analyze entanglement
    let analyzer = EntanglementAnalyzer::new();

    println!("  Entanglement Analysis:");
    println!("    Product state |0000⟩:");
    let product_entropy = analyzer.estimate_max_entanglement_entropy(&product_state);
    let product_class = analyzer.classify_entanglement(&product_state);
    println!("      - Max entropy: {:.4} bits", product_entropy);
    println!("      - Classification: {:?}", product_class);

    println!("    GHZ state (|0000⟩ + |1111⟩)/√2:");
    let ghz_entropy = analyzer.estimate_max_entanglement_entropy(&ghz_state);
    let ghz_class = analyzer.classify_entanglement(&ghz_state);
    println!("      - Max entropy: {:.4} bits", ghz_entropy);
    println!("      - Classification: {:?}", ghz_class);
    println!();

    // Optimize bond dimension
    println!("  Bond Dimension Optimization:");
    
    let config = OptimizationConfig {
        target_fidelity: 0.9999,
        ..Default::default()
    };

    let mut optimizer = BondDimensionOptimizer::new(config.clone());

    println!("    Product state:");
    match optimizer.optimize(&product_state) {
        Ok(result) => {
            println!("      - Optimal χ: {}", result.optimal_bond_dim);
            println!("      - Fidelity: {:.10}", result.achieved_fidelity);
            println!("      - Compression: {:.2}x", result.compression_ratio);
        }
        Err(e) => println!("      - Error: {}", e),
    }

    let mut optimizer = BondDimensionOptimizer::new(config);
    println!("    GHZ state:");
    match optimizer.optimize(&ghz_state) {
        Ok(result) => {
            println!("      - Optimal χ: {}", result.optimal_bond_dim);
            println!("      - Fidelity: {:.10}", result.achieved_fidelity);
            println!("      - Compression: {:.2}x", result.compression_ratio);
        }
        Err(e) => println!("      - Error: {}", e),
    }

    println!();
}

// =============================================================================
// Demo 4: Benchmark Different State Types
// =============================================================================

fn demo_state_benchmarks() {
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 4: Bond Dimension Benchmarks                               │");
    println!("└─────────────────────────────────────────────────────────────────┘");
    println!();

    let n = 4; // 4 qubits

    // Product state
    let product = QuantumStateVector::zero_state(n);

    // GHZ state
    let mut ghz_amps = vec![Complex64::new(0.0, 0.0); 1 << n];
    ghz_amps[0] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    ghz_amps[(1 << n) - 1] = Complex64::new(1.0 / 2.0_f64.sqrt(), 0.0);
    let ghz = QuantumStateVector::from_amplitudes(ghz_amps);

    let chi_values = vec![1, 2, 4, 8];

    println!("  Benchmark: Fidelity vs Bond Dimension (n={} qubits)", n);
    println!();
    println!("  {:^12} | {:^12} | {:^12} | {:^12} | {:^12}", 
             "χ", "Product", "GHZ", "Storage", "Compression");
    println!("  {:-^12}-+-{:-^12}-+-{:-^12}-+-{:-^12}-+-{:-^12}", 
             "", "", "", "", "");

    for &chi in &chi_values {
        let product_result = benchmark_bond_dimensions(&product, "product", &[chi]);
        let ghz_result = benchmark_bond_dimensions(&ghz, "ghz", &[chi]);

        if let (Ok(p), Ok(g)) = (product_result, ghz_result) {
            if let (Some(pp), Some(gp)) = (p.points.first(), g.points.first()) {
                println!("  {:^12} | {:^12.6} | {:^12.6} | {:^10} B | {:^12.2}x",
                         chi, pp.fidelity, gp.fidelity, pp.storage_bytes, pp.compression_ratio);
            }
        }
    }

    println!();
    println!("  Legend:");
    println!("    - χ: Bond dimension");
    println!("    - Fidelity: |⟨ψ_original|ψ_reconstructed⟩|²");
    println!("    - Storage: Estimated file size in bytes");
    println!("    - Compression: Dense size / MPS size");
    println!();
}
