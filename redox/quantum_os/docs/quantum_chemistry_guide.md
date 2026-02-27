# Quantum Chemistry Implementation Guide

**Version**: 1.0.0  
**Authors**: V-OS Team  
**Date**: 2026-01-25  

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Molecular Hamiltonians](#2-molecular-hamiltonians)
3. [Variational Quantum Eigensolver (VQE)](#3-variational-quantum-eigensolver-vqe)
4. [Molecular Ansätze](#4-molecular-ansätze)
5. [Practical Molecules](#5-practical-molecules)
6. [Implementation with QFT](#6-implementation-with-qft)
7. [References](#7-references)

---

## 1. Introduction

Quantum chemistry is a leading near-term application of quantum computing. The electronic structure problem is naturally suited for quantum simulation.

### 1.1 Key Concepts

- **Second Quantization**: Electrons as creation/annihilation operators
- **Jordan-Wigner Transform**: Map fermionic to qubit operators
- **Variational Principle**: E₀ ≤ ⟨ψ|H|ψ⟩

### 1.2 QFT's Role

The `.qft` format enables efficient storage of molecular ground states, checkpointing during VQE, and reproducible experiments.

---

## 2. Molecular Hamiltonians

### 2.1 Electronic Structure

```
H = Σᵢⱼ hᵢⱼ aᵢ†aⱼ + ½ Σᵢⱼₖₗ hᵢⱼₖₗ aᵢ†aⱼ†aₖaₗ
```

### 2.2 Jordan-Wigner Transformation

```rust
fn jordan_wigner_creation(j: usize, n_qubits: usize) -> Vec<PauliString> {
    // aⱼ† = (Xⱼ - iYⱼ)/2 ⊗ Z₀Z₁...Zⱼ₋₁
    let mut z_string = vec![PauliOp::I; n_qubits];
    for k in 0..j { z_string[k] = PauliOp::Z; }
    
    let mut x_term = z_string.clone();
    x_term[j] = PauliOp::X;
    
    let mut y_term = z_string.clone();
    y_term[j] = PauliOp::Y;
    
    vec![
        PauliString::new(x_term, Complex64::new(0.5, 0.0)),
        PauliString::new(y_term, Complex64::new(0.0, -0.5)),
    ]
}
```

---

## 3. Variational Quantum Eigensolver (VQE)

### 3.1 Algorithm

1. Prepare trial state: |ψ(θ)⟩ = U(θ)|HF⟩
2. Measure energy: E(θ) = ⟨ψ(θ)|H|ψ(θ)⟩
3. Classical optimization: θ* = argmin E(θ)
4. Iterate until convergence

### 3.2 Implementation

```rust
struct VQE {
    hamiltonian: QubitHamiltonian,
    ansatz: Box<dyn Ansatz>,
    n_qubits: usize,
}

impl VQE {
    fn compute_energy(&self, params: &[f64]) -> f64 {
        let mut qft = QftFile::new(self.n_qubits, QftConfig::default());
        self.ansatz.apply(&mut qft, params);
        self.hamiltonian.expectation_value(&qft)
    }
    
    fn run(&mut self, initial_params: &[f64], max_iter: usize) -> VQEResult {
        let mut params = initial_params.to_vec();
        let mut best_energy = f64::INFINITY;
        
        for _ in 0..max_iter {
            let energy = self.compute_energy(&params);
            if energy < best_energy { best_energy = energy; }
            
            let gradient = self.compute_gradient(&params);
            for (p, g) in params.iter_mut().zip(gradient.iter()) {
                *p -= 0.1 * g;
            }
        }
        
        VQEResult { energy: best_energy, params }
    }
}
```

---

## 4. Molecular Ansätze

### 4.1 UCCSD (Unitary Coupled Cluster)

```rust
struct UCCSD {
    singles: Vec<(usize, usize)>,    // (occupied, virtual)
    doubles: Vec<(usize, usize, usize, usize)>,
}

impl Ansatz for UCCSD {
    fn apply(&self, qft: &mut QftFile, params: &[f64]) {
        let mut idx = 0;
        for &(i, a) in &self.singles {
            apply_single_excitation(qft, i, a, params[idx]);
            idx += 1;
        }
        for &(i, j, a, b) in &self.doubles {
            apply_double_excitation(qft, i, j, a, b, params[idx]);
            idx += 1;
        }
    }
}
```

### 4.2 Hardware-Efficient Ansatz

```rust
fn hardware_efficient(qft: &mut QftFile, params: &[f64], layers: usize) {
    let n = qft.num_qubits();
    let mut idx = 0;
    
    for _ in 0..layers {
        for q in 0..n {
            qft.apply_ry(q, params[idx]);
            qft.apply_rz(q, params[idx + 1]);
            idx += 2;
        }
        for q in 0..(n - 1) {
            qft.apply_cnot(q, q + 1);
        }
    }
}
```

---

## 5. Practical Molecules

### 5.1 H₂ (Hydrogen)

```rust
fn h2_example() {
    let bond_lengths = vec![0.5, 0.74, 1.0, 1.5, 2.0];
    
    for r in bond_lengths {
        let mol = hydrogen_molecule(r);
        let hamiltonian = mol.to_qubit_hamiltonian();
        
        let mut vqe = VQE::new(hamiltonian, Box::new(UCCSD::new(2, 2)));
        let result = vqe.run(&[0.0], 100);
        
        println!("R = {:.2} Å: E = {:.6} Ha", r, result.energy);
        
        // Save ground state
        if let Err(e) = result.state.save(&format!("h2_r{:.2}.qft", r)) {
            eprintln!("Failed to save state for R={:.2}: {:?}", r, e);
        }
    }
}
```

### 5.2 LiH, H₂O, N₂

Similar approach with larger active spaces (4-12 qubits).

---

## 6. Implementation with QFT

### 6.1 Saving Ground States

```rust
fn save_ground_state(result: &VQEResult, molecule: &str, path: &Path) {
    let mut qft = result.state.clone();
    
    qft.set_metadata("molecule", molecule);
    qft.set_metadata("energy_hartree", &result.energy.to_string());
    if let Ok(params_json) = serde_json::to_string(&result.params) {
        qft.set_metadata("params", &params_json);
    }
    
    if let Err(e) = qft.save(path) {
        eprintln!("Failed to save ground state: {:?}", e);
    }
}
```

### 6.2 Potential Energy Surface

```rust
fn scan_pes(molecule_fn: impl Fn(f64) -> Hamiltonian, r_values: &[f64]) {
    for &r in r_values {
        let h = molecule_fn(r);
        let result = vqe.run(&h);
        save_ground_state(&result, "scan", &format!("pes_{:.3}.qft", r));
    }
}
```

---

## 7. References

1. Peruzzo, A., et al. (2014). "A variational eigenvalue solver on a photonic quantum processor." *Nature Communications*, 5, 4213.

2. Cao, Y., et al. (2019). "Quantum Chemistry in the Age of Quantum Computing." *Chemical Reviews*, 119(19), 10856-10915.

3. McArdle, S., et al. (2020). "Quantum computational chemistry." *Reviews of Modern Physics*, 92(1), 015003.

4. Tilly, J., et al. (2022). "The Variational Quantum Eigensolver: A review." *Physics Reports*, 986, 1-128.

---

*Document created for V-OS*
