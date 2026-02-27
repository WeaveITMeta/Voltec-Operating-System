# Phase 3: Quantum Hardware Abstraction — Annotated Bibliography

> **Purpose**: Implementation-oriented references for Phase 3 of V-OS.  
> Each entry includes: (a) what it proves/defines, (b) why it matters to the roadmap, (c) Rust architecture implications.

---

## 3.1 Hardware Backend Interfaces

### QHAL — Quantum Hardware Abstraction Layer
- **Citation**: Multi-level quantum/classical interface specifications
- **What it is**: A multi-level abstraction model for interoperability between low-level control/hardware and higher-level software stacks. Treats the quantum/classical boundary as a designed interface, not an ad hoc SDK layer.
- **Why it matters**: Directly supports `HardwareDeviceInterface`, `HardwareCapabilities`, and topology mapping. Capabilities should be discoverable and negotiated at multiple levels (logical gates ↔ timing/pulses ↔ hardware constraints).
- **Rust implication**: Define a layered capability model:
  ```rust
  pub struct DeviceCapabilities {
      pub gate_level_profile: GateLevelProfile,
      pub timing_profile: Option<TimingProfile>,
      pub pulse_profile: Option<PulseProfile>,
  }
  ```
  Allow adapters to implement only the levels they support.

### QIR Specification (QIR Alliance)
- **Citation**: https://github.com/qir-alliance/qir-spec
- **What it is**: LLVM-based quantum intermediate representation with profiles (base vs adaptive) for hardware capability boundaries.
- **Why it matters**: Cleanest "portable contract" for hardware abstraction. Aligns with Azure Quantum and cross-platform efforts.
- **Rust implication**: `QuantumDeviceInterface` should accept either:
  - LogosQ IR (native)
  - QIR-compatible lowered form
  
  Use profiles to drive `HardwareCapabilities` negotiation.

### QIR Execution Engine (QIR-EE)
- **Citation**: https://github.com/qir-alliance/qir-runner
- **What it is**: Runtime that parses/interprets QIR and dispatches to multiple hardware platforms and simulators; includes extension points for custom runtime/hardware environments.
- **Why it matters**: Reference architecture for `ConnectionManager`, job dispatch, and mixed classical/quantum instruction handling.
- **Rust implication**: Model execution core as:
  ```
  job → lowered_ir → dispatch → measurement_stream
  ```
  With explicit extension points for provider adapters.

### OpenQASM 3
- **Citation**: arXiv:2104.14722 — "OpenQASM 3: A broader and deeper quantum assembly language"
- **What it is**: Machine-independent quantum assembly language extended to cover real-time classical control flow, timing, and near-hardware specificity (including calibration/characterization use cases).
- **Why it matters**: Validates "V-OS" runtime view: quantum programs aren't only static circuits; they include timing + measurement-driven control.
- **Rust implication**: IR needs explicit support for:
  - Measurement events
  - Conditional branching (restricted form)
  - Timing/scheduling metadata

---

## 3.2 Cloud Provider Integrations

### IBM Quantum (Qiskit Runtime)
- **Citation**: https://docs.quantum.ibm.com/api/runtime
- **What it is**: REST auth model + runtime job endpoints; backend classes and property models (calibration snapshots, gate/qubit properties).
- **Why it matters**: Ground truth for authentication, job lifecycle, and calibration retrieval/time series.
- **Rust implication**: Build provider-agnostic `CredentialProvider` + `JobClient`, then implement IBM as one adapter with typed parsing for backend properties:
  ```rust
  pub trait CloudProviderAdapter {
      fn authenticate(&self, credentials: &Credentials) -> Result<Session>;
      fn submit_job(&self, session: &Session, circuit: &CompiledCircuit) -> Result<JobId>;
      fn poll_status(&self, session: &Session, job_id: JobId) -> Result<JobStatus>;
      fn fetch_results(&self, session: &Session, job_id: JobId) -> Result<ExecutionResult>;
  }
  ```

### Amazon Braket
- **Citation**: https://docs.aws.amazon.com/braket/latest/APIReference/
- **What it is**: API endpoints like `CreateQuantumTask` and developer guide patterns for submission/monitoring via API and SDK.
- **Why it matters**: Defines "lowest-level" cloud contract: create task, poll status, fetch results, handle S3 outputs.
- **Rust implication**: Treat Braket as a clean REST adapter; keep device discovery separate from job submission.

### Azure Quantum
- **Citation**: https://learn.microsoft.com/en-us/azure/quantum/
- **What it is**: Official guidance for submitting QIR-formatted circuits and other formats to Azure Quantum, including provider targets and constraints.
- **Why it matters**: Reinforces QIR as practical interoperability backbone (especially for targets with profile constraints).
- **Rust implication**: If QIR export is supported, Azure becomes "just another adapter," not a special case.

### IonQ Direct API
- **Citation**: https://docs.ionq.com/api-reference/v0.3/
- **What it is**: Direct API submission docs + endpoint references and OpenAPI schema.
- **Why it matters**: Great for building stable Rust client with codegen against OpenAPI.
- **Rust implication**: Generate client bindings from OpenAPI, then wrap in unified `ConnectionManager` and `HardwareDeviceInterface`.

### Rigetti QCS
- **Citation**: https://docs.rigetti.com/qcs/
- **What it is**: Official QCS HTTP API docs + Quil/Quil-T docs; explicitly states programs are assembled using current calibrations, with override mechanisms.
- **Why it matters**: Directly supports calibration-aware compilation and native gate/pulse lowering strategy.
- **Rust implication**: Keep clean separation between `logical_program` and `assembled_program`, with calibration snapshots as input to assembly:
  ```rust
  pub fn assemble(
      logical: &LogicalProgram,
      calibration: &CalibrationSnapshot,
      device: &DeviceTopology,
  ) -> Result<AssembledProgram>
  ```

---

## 3.3 Noise Model Framework

### Qiskit Aer Noise Model Construction
- **Citation**: https://qiskit.github.io/qiskit-aer/stubs/qiskit_aer.noise.NoiseModel.html
- **What it is**: Concrete examples and APIs for building noise models from Kraus/SuperOp representations, composing readout errors, and assembling full-device noise models.
- **Why it matters**: Most implementation-ready reference for `NoiseModel` trait + composable channels (depolarizing, damping, dephasing, readout).
- **Rust implication**: Represent noise as composable channels:
  ```rust
  pub trait NoiseChannel: Send + Sync {
      fn kraus_operators(&self) -> Vec<KrausOperator>;
      fn compose(&self, other: &dyn NoiseChannel) -> Box<dyn NoiseChannel>;
      fn tensor(&self, other: &dyn NoiseChannel) -> Box<dyn NoiseChannel>;
  }
  ```

### Crosstalk / Correlated Noise
- **Citation**: arXiv:2005.10921 — PEC practical work emphasizing crosstalk during parallel gates
- **What it is**: Demonstrates difficulty of representing noise on full devices due to crosstalk during parallel gates, motivating explicit correlated-noise modeling.
- **Why it matters**: Justifies "crosstalk noise model for multi-qubit gates" as not optional for realism.
- **Rust implication**: Include correlated-noise hooks:
  ```rust
  fn channel_for_parallel_gate_set(&self, gates: &[GateInstance]) -> Box<dyn NoiseChannel>;
  fn conditional_channel(&self, context: &ExecutionContext) -> Box<dyn NoiseChannel>;
  ```

---

## 3.4 Calibration System

### IBM Backend Properties
- **Citation**: https://docs.quantum.ibm.com/api/qiskit/qiskit.providers.BackendV2
- **What it is**: Backend properties and interfaces expose calibrated values over time (qubits, gates, last update timestamp).
- **Why it matters**: Canonical model for `CalibrationFetcher`, time series tracking, and qubit quality scoring.
- **Rust implication**: Store calibration as immutable snapshots:
  ```rust
  pub struct CalibrationSnapshot {
      pub device_id: DeviceId,
      pub timestamp: DateTime<Utc>,
      pub qubit_properties: Vec<QubitCalibration>,
      pub gate_properties: Vec<GateCalibration>,
      pub readout_errors: Vec<ReadoutError>,
  }
  
  impl CalibrationSnapshot {
      pub fn qubit_quality_score(&self, qubit: usize) -> f64;
      pub fn select_best_qubits(&self, count: usize) -> Vec<usize>;
  }
  ```

### Rigetti Calibration-Aware Assembly
- **Citation**: https://docs.rigetti.com/qcs/guides/quil-t
- **What it is**: Rigetti's docs explicitly describe compilation/assembly using current calibrations and support calibration overrides.
- **Why it matters**: Supports "calibration-aware circuit compilation" and JSON format parsing strategy (provider-specific schemas).
- **Rust implication**: Calibration must be first-class input to compilation passes (routing/scheduling/echo pulses).

---

## 3.5 Error Mitigation

### Temme–Bravyi–Gambetta (2017) — Foundational ZNE + PEC
- **Citation**: arXiv:1612.02058 — "Error mitigation for short-depth quantum circuits"
- **What it is**: Introduces two key families:
  1. Extrapolation to zero-noise limit (Richardson-like) — **ZNE**
  2. Quasi-probability resampling — **PEC lineage**
- **Why it matters**: Citation anchor for ZNE and PEC existence/validity.
- **Rust implication**: Structure mitigation layer as:
  ```rust
  pub trait ErrorMitigator {
      fn transform_circuit(&self, circuit: &Circuit, noise_scale: f64) -> Circuit;
      fn estimate(&self, results: &[ExecutionResult]) -> MitigatedResult;
  }
  ```

### PEC at Scale — Sparse Pauli–Lindblad Models
- **Citation**: Nature Physics 2023 / arXiv:2201.09866
- **What it is**: Demonstrates PEC with tractable noise modeling on real noisy processors; emphasizes learning/representing noise and handling crosstalk complexity.
- **Why it matters**: Justifies "import noise models from calibration data" and "crosstalk noise" as prerequisites for meaningful PEC.
- **Rust implication**: Define noise-model import/export as stable format; treat "noise characterization fidelity" as mitigation precondition.

### Mitiq Toolkit — Engineering Patterns
- **Citation**: https://mitiq.readthedocs.io/
- **What it is**: Backend-agnostic toolkit implementing ZNE, PEC, and readout mitigation with pluggable executors and circuit factories.
- **Why it matters**: "How to structure it in software" reference: executor abstraction, circuit factories, estimator plugins.
- **Rust implication**: Copy architecture, not Python:
  ```rust
  pub trait Executor {
      fn execute(&self, circuit: &Circuit, shots: usize) -> ExecutionResult;
  }
  
  pub trait CircuitTransform {
      fn transform(&self, circuit: &Circuit) -> Vec<Circuit>;
  }
  
  pub trait Estimator {
      fn estimate(&self, results: &[ExecutionResult]) -> f64;
  }
  ```

### Readout Error Mitigation
- **Citation**: arXiv:2006.14044 — Confusion matrix inversion and scalable variants
- **What it is**: Confusion-matrix / transition-matrix inversion is standard REM primitive; modern variants address scalability and mid-circuit cases.
- **Why it matters**: Anchors "confusion matrix inversion" and future-proofing for dynamic circuits.
- **Rust implication**: Provide both implementations:
  ```rust
  pub struct GlobalConfusionMatrix { matrix: Array2<f64> }
  pub struct FactorizedLocalMatrices { matrices: Vec<Array2<f64>> }
  
  impl ReadoutMitigator for GlobalConfusionMatrix {
      fn mitigate(&self, counts: &Counts, regularization: f64) -> Counts;
  }
  ```

### Dynamical Decoupling (DD) & TREX
- **Citation**: 
  - DD: arXiv:1807.08768 — "Dynamical decoupling and noise spectroscopy"
  - TREX: IBM Qiskit Runtime docs + arXiv:2012.09738
- **What it is**: DD reduces decoherence during idle times; TREX is practical mitigation strategy using twirled readout.
- **Why it matters**: Supports "pulse insertion" and "TREX" items with implementation guidance and research context.
- **Rust implication**:
  - DD: Implement as compiler pass inserting pulses based on schedule gaps
  - TREX: Implement as execution strategy (twirling randomizations + estimator)

---

## 3.6 Circuit Compilation

### Qiskit Transpiler + Staged Pass Managers
- **Citation**: https://docs.quantum.ibm.com/api/qiskit/transpiler
- **What it is**: Clear decomposition into pass manager stages (layout, routing, optimization), plugin model, and reproducibility constraints.
- **Why it matters**: Direct template for transpiler framework and pass manager.
- **Rust implication**:
  ```rust
  pub trait CompilerPass: Send + Sync {
      fn name(&self) -> &str;
      fn run(&self, dag: &mut CircuitDag, context: &CompilationContext) -> Result<()>;
  }
  
  pub struct StagedPassManager {
      pub init_passes: Vec<Box<dyn CompilerPass>>,
      pub layout_passes: Vec<Box<dyn CompilerPass>>,
      pub routing_passes: Vec<Box<dyn CompilerPass>>,
      pub optimization_passes: Vec<Box<dyn CompilerPass>>,
      pub scheduling_passes: Vec<Box<dyn CompilerPass>>,
  }
  ```

### "On the Qubit Routing Problem" (TQC 2019)
- **Citation**: arXiv:1902.08091
- **What it is**: Practical, architecture-agnostic routing methodology with empirical comparisons; widely cited for limited-connectivity devices.
- **Why it matters**: Core citation for SWAP-based routing and mapping quality metrics (depth, 2Q count).
- **Rust implication**: Provide routing as strategy trait:
  ```rust
  pub trait RoutingStrategy {
      fn route(
          &self,
          circuit: &Circuit,
          coupling_graph: &CouplingGraph,
      ) -> Result<RoutedCircuit>;
      
      fn cost(&self, routed: &RoutedCircuit) -> RoutingCost;
  }
  
  pub struct RoutingCost {
      pub depth: usize,
      pub two_qubit_gate_count: usize,
      pub swap_count: usize,
  }
  ```

### tket Mapping/Routing Guide
- **Citation**: https://tket.quantinuum.com/user-guide/
- **What it is**: Provider guide showing how routing is represented as architecture object and how routing constraints shape compilation.
- **Why it matters**: Bridges paper → implementation; helps define `DeviceTopology` structure.
- **Rust implication**:
  ```rust
  pub struct DeviceTopology {
      pub nodes: Vec<PhysicalQubit>,
      pub edges: Vec<(PhysicalQubit, PhysicalQubit)>,
      pub directionality: Directionality,
      pub native_two_qubit_gates: Vec<NativeGate>,
  }
  
  pub enum Directionality {
      Undirected,
      Directed,
  }
  ```

---

## Summary: Defensible Design Citations

| Subsection | Key Citations |
|------------|---------------|
| **3.1 IR + Abstraction** | QHAL, QIR Spec, QIR-EE, OpenQASM 3 |
| **3.2 Provider Adapters** | IBM/AWS/Azure/IonQ/Rigetti official contracts |
| **3.3 Noise + Calibration** | Aer noise model patterns, calibration property models |
| **3.4 Calibration System** | IBM BackendV2, Rigetti Quil-T |
| **3.5 Mitigation** | Temme–Bravyi–Gambetta, PEC Nature Physics, Mitiq, TREX/DD |
| **3.6 Compilation** | Staged pass managers, routing literature, tket |

---

*Document generated for V-OS Phase 3 implementation planning.*
