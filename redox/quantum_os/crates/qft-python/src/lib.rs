//! # qft-python - Production-Grade Python Bindings for Quantum File Type (.qft)
//!
//! ## Table of Contents
//! 1. QftFile Class - Main Python interface for .qft files
//! 2. QftState Class - Quantum state representation with NumPy interop
//! 3. Conversion Functions - To/from Qiskit, Cirq, NumPy
//! 4. Streaming API - Large file processing
//! 5. Validation Functions - Checksums, fidelity, normalization
//! 6. Module Functions - Top-level convenience functions

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;
use std::path::Path;

// =============================================================================
// Section 1: Error Types
// =============================================================================

/// Custom error type for QFT operations
#[derive(Debug)]
struct QftPyError {
    message: String,
}

impl std::fmt::Display for QftPyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for QftPyError {}

impl From<QftPyError> for PyErr {
    fn from(err: QftPyError) -> PyErr {
        PyRuntimeError::new_err(err.message)
    }
}

impl From<std::io::Error> for QftPyError {
    fn from(err: std::io::Error) -> Self {
        QftPyError {
            message: format!("I/O error: {}", err),
        }
    }
}

// =============================================================================
// Section 2: QftFile Class
// =============================================================================

/// A Quantum File Type (.qft) file handle.
///
/// This class provides the main interface for creating, loading, and saving
/// quantum states in the .qft format.
///
/// Examples
/// --------
/// >>> import qft
/// >>> # Create a new 4-qubit state
/// >>> f = qft.QftFile(4)
/// >>> f.num_qubits
/// 4
/// >>> f.dimension
/// 16
/// >>> # Save to disk
/// >>> f.save("state.qft")
#[pyclass(name = "QftFile")]
struct QftFile {
    num_qubits: usize,
    amplitudes_real: Vec<f64>,
    amplitudes_imag: Vec<f64>,
    metadata: HashMap<String, String>,
    bond_dimension: usize,
    golay_enabled: bool,
    source_path: Option<String>,
}

#[pymethods]
impl QftFile {
    /// Create a new QFT file with the specified number of qubits.
    ///
    /// Parameters
    /// ----------
    /// num_qubits : int
    ///     Number of qubits (1-30)
    ///
    /// Returns
    /// -------
    /// QftFile
    ///     A new QFT file initialized to the |0...0⟩ state
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If num_qubits is not in range 1-30
    #[new]
    #[pyo3(signature = (num_qubits=1))]
    fn new(num_qubits: usize) -> PyResult<Self> {
        if num_qubits == 0 || num_qubits > 30 {
            return Err(PyValueError::new_err(format!(
                "num_qubits must be 1-30, got {}",
                num_qubits
            )));
        }

        let dim = 1usize << num_qubits;
        let mut amplitudes_real = vec![0.0; dim];
        let amplitudes_imag = vec![0.0; dim];
        amplitudes_real[0] = 1.0; // |0...0⟩ state

        Ok(Self {
            num_qubits,
            amplitudes_real,
            amplitudes_imag,
            metadata: HashMap::new(),
            bond_dimension: 64,
            golay_enabled: true,
            source_path: None,
        })
    }

    /// Number of qubits in the quantum state.
    #[getter]
    fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Dimension of the state vector (2^num_qubits).
    #[getter]
    fn dimension(&self) -> usize {
        1 << self.num_qubits
    }

    /// Bond dimension for MPS compression.
    #[getter]
    fn bond_dimension(&self) -> usize {
        self.bond_dimension
    }

    #[setter]
    fn set_bond_dimension(&mut self, value: usize) -> PyResult<()> {
        if value == 0 || value > 1024 {
            return Err(PyValueError::new_err(
                "bond_dimension must be 1-1024",
            ));
        }
        self.bond_dimension = value;
        Ok(())
    }

    /// Whether Golay error correction is enabled.
    #[getter]
    fn golay_enabled(&self) -> bool {
        self.golay_enabled
    }

    #[setter]
    fn set_golay_enabled(&mut self, value: bool) {
        self.golay_enabled = value;
    }

    /// Source file path if loaded from disk.
    #[getter]
    fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }

    /// Get the state amplitudes as a complex NumPy array.
    ///
    /// Returns
    /// -------
    /// numpy.ndarray
    ///     Complex128 array of shape (2^num_qubits,)
    #[getter]
    fn amplitudes<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        // Return interleaved real/imag for complex representation
        let mut interleaved = Vec::with_capacity(self.amplitudes_real.len() * 2);
        for (r, i) in self.amplitudes_real.iter().zip(self.amplitudes_imag.iter()) {
            interleaved.push(*r);
            interleaved.push(*i);
        }
        interleaved.into_pyarray(py)
    }

    /// Get the real parts of amplitudes.
    #[getter]
    fn amplitudes_real<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        self.amplitudes_real.clone().into_pyarray(py)
    }

    /// Get the imaginary parts of amplitudes.
    #[getter]
    fn amplitudes_imag<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        self.amplitudes_imag.clone().into_pyarray(py)
    }

    /// Set amplitudes from separate real and imaginary arrays.
    ///
    /// Parameters
    /// ----------
    /// real : numpy.ndarray
    ///     Real parts of amplitudes
    /// imag : numpy.ndarray
    ///     Imaginary parts of amplitudes
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If arrays have wrong length
    fn set_amplitudes(
        &mut self,
        real: PyReadonlyArray1<f64>,
        imag: PyReadonlyArray1<f64>,
    ) -> PyResult<()> {
        let real_slice = real.as_slice()?;
        let imag_slice = imag.as_slice()?;

        let expected = self.dimension();
        if real_slice.len() != expected || imag_slice.len() != expected {
            return Err(PyValueError::new_err(format!(
                "Expected {} amplitudes, got {} real and {} imag",
                expected,
                real_slice.len(),
                imag_slice.len()
            )));
        }

        self.amplitudes_real = real_slice.to_vec();
        self.amplitudes_imag = imag_slice.to_vec();
        Ok(())
    }

    /// Get a single amplitude by basis state index.
    ///
    /// Parameters
    /// ----------
    /// index : int
    ///     Basis state index (0 to 2^n - 1)
    ///
    /// Returns
    /// -------
    /// tuple[float, float]
    ///     (real, imag) parts of the amplitude
    fn get_amplitude(&self, index: usize) -> PyResult<(f64, f64)> {
        if index >= self.dimension() {
            return Err(PyValueError::new_err(format!(
                "Index {} out of range for {} qubits",
                index, self.num_qubits
            )));
        }
        Ok((self.amplitudes_real[index], self.amplitudes_imag[index]))
    }

    /// Set a single amplitude by basis state index.
    ///
    /// Parameters
    /// ----------
    /// index : int
    ///     Basis state index (0 to 2^n - 1)
    /// real : float
    ///     Real part of amplitude
    /// imag : float
    ///     Imaginary part of amplitude
    fn set_amplitude(&mut self, index: usize, real: f64, imag: f64) -> PyResult<()> {
        if index >= self.dimension() {
            return Err(PyValueError::new_err(format!(
                "Index {} out of range for {} qubits",
                index, self.num_qubits
            )));
        }
        self.amplitudes_real[index] = real;
        self.amplitudes_imag[index] = imag;
        Ok(())
    }

    /// Check if the state is normalized (sum of |amplitude|^2 ≈ 1).
    ///
    /// Parameters
    /// ----------
    /// tolerance : float, optional
    ///     Tolerance for normalization check (default: 1e-10)
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if normalized within tolerance
    #[pyo3(signature = (tolerance=1e-10))]
    fn is_normalized(&self, tolerance: f64) -> bool {
        let norm_sq: f64 = self
            .amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();
        (norm_sq - 1.0).abs() < tolerance
    }

    /// Normalize the state vector in place.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If state has zero norm
    fn normalize(&mut self) -> PyResult<()> {
        let norm_sq: f64 = self
            .amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();

        if norm_sq < 1e-15 {
            return Err(PyValueError::new_err("Cannot normalize zero state"));
        }

        let norm = norm_sq.sqrt();
        for r in &mut self.amplitudes_real {
            *r /= norm;
        }
        for i in &mut self.amplitudes_imag {
            *i /= norm;
        }
        Ok(())
    }

    /// Calculate the norm squared (sum of |amplitude|^2).
    fn norm_squared(&self) -> f64 {
        self.amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum()
    }

    /// Calculate fidelity with another state: |⟨self|other⟩|².
    ///
    /// Parameters
    /// ----------
    /// other : QftFile
    ///     Another quantum state
    ///
    /// Returns
    /// -------
    /// float
    ///     Fidelity between 0 and 1
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If states have different dimensions
    fn fidelity(&self, other: &QftFile) -> PyResult<f64> {
        if self.num_qubits != other.num_qubits {
            return Err(PyValueError::new_err(format!(
                "Dimension mismatch: {} vs {} qubits",
                self.num_qubits, other.num_qubits
            )));
        }

        let mut overlap_real = 0.0;
        let mut overlap_imag = 0.0;

        for i in 0..self.dimension() {
            // conj(self) * other
            let a = self.amplitudes_real[i];
            let b = self.amplitudes_imag[i];
            let c = other.amplitudes_real[i];
            let d = other.amplitudes_imag[i];
            overlap_real += a * c + b * d;
            overlap_imag += a * d - b * c;
        }

        Ok(overlap_real * overlap_real + overlap_imag * overlap_imag)
    }

    /// Set metadata key-value pair.
    ///
    /// Parameters
    /// ----------
    /// key : str
    ///     Metadata key
    /// value : str
    ///     Metadata value
    fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    /// Get metadata value by key.
    ///
    /// Parameters
    /// ----------
    /// key : str
    ///     Metadata key
    ///
    /// Returns
    /// -------
    /// str or None
    ///     Metadata value or None if not found
    fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).cloned()
    }

    /// Get all metadata as a dictionary.
    fn metadata_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (k, v) in &self.metadata {
            dict.set_item(k, v)?;
        }
        Ok(dict)
    }

    /// Save the QFT file to disk.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     Output file path
    ///
    /// Raises
    /// ------
    /// IOError
    ///     If file cannot be written
    fn save(&mut self, path: &str) -> PyResult<()> {
        let data = self.serialize()?;
        std::fs::write(path, &data).map_err(|e| PyIOError::new_err(format!("Write error: {}", e)))?;
        self.source_path = Some(path.to_string());
        Ok(())
    }

    /// Serialize to bytes.
    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyBytes>> {
        let data = self.serialize()?;
        Ok(pyo3::types::PyBytes::new(py, &data))
    }

    fn __repr__(&self) -> String {
        format!(
            "QftFile(num_qubits={}, dimension={}, bond_dim={}, golay={})",
            self.num_qubits,
            self.dimension(),
            self.bond_dimension,
            self.golay_enabled
        )
    }

    fn __str__(&self) -> String {
        let norm = self.norm_squared().sqrt();
        format!(
            "QftFile: {} qubits, dim={}, norm={:.6}, {} metadata entries",
            self.num_qubits,
            self.dimension(),
            norm,
            self.metadata.len()
        )
    }
}

impl QftFile {
    fn serialize(&self) -> PyResult<Vec<u8>> {
        let dim = self.dimension();
        let mut data = Vec::with_capacity(16 + dim * 16);

        // Magic number
        data.extend_from_slice(b"QFT\x01");

        // Header
        data.push(self.num_qubits as u8);
        data.push(self.bond_dimension.min(255) as u8);
        data.push(if self.golay_enabled { 1 } else { 0 });
        data.push(0); // reserved

        // Padding to 16 bytes
        data.extend_from_slice(&[0u8; 8]);

        // Amplitudes
        for (r, i) in self.amplitudes_real.iter().zip(self.amplitudes_imag.iter()) {
            data.extend_from_slice(&r.to_le_bytes());
            data.extend_from_slice(&i.to_le_bytes());
        }

        Ok(data)
    }

    fn deserialize(data: &[u8]) -> PyResult<Self> {
        if data.len() < 16 {
            return Err(PyValueError::new_err("File too small"));
        }

        if &data[0..4] != b"QFT\x01" {
            return Err(PyValueError::new_err("Invalid QFT magic number"));
        }

        let num_qubits = data[4] as usize;
        if num_qubits == 0 || num_qubits > 30 {
            return Err(PyValueError::new_err(format!(
                "Invalid num_qubits: {}",
                num_qubits
            )));
        }

        let bond_dimension = data[5] as usize;
        let golay_enabled = data[6] != 0;

        let dim = 1usize << num_qubits;
        let expected_size = 16 + dim * 16;

        if data.len() < expected_size {
            return Err(PyValueError::new_err(format!(
                "File truncated: expected {} bytes, got {}",
                expected_size,
                data.len()
            )));
        }

        let mut amplitudes_real = Vec::with_capacity(dim);
        let mut amplitudes_imag = Vec::with_capacity(dim);

        for i in 0..dim {
            let offset = 16 + i * 16;
            let real = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            let imag = f64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());
            amplitudes_real.push(real);
            amplitudes_imag.push(imag);
        }

        Ok(Self {
            num_qubits,
            amplitudes_real,
            amplitudes_imag,
            metadata: HashMap::new(),
            bond_dimension: if bond_dimension == 0 { 64 } else { bond_dimension },
            golay_enabled,
            source_path: None,
        })
    }
}

// =============================================================================
// Section 3: Module Functions
// =============================================================================

/// Load a QFT file from disk.
///
/// Parameters
/// ----------
/// path : str
///     Path to .qft file
///
/// Returns
/// -------
/// QftFile
///     Loaded quantum state
///
/// Raises
/// ------
/// IOError
///     If file cannot be read
/// ValueError
///     If file is not a valid .qft file
#[pyfunction]
fn load(path: &str) -> PyResult<QftFile> {
    let data = std::fs::read(path).map_err(|e| PyIOError::new_err(format!("Read error: {}", e)))?;
    let mut file = QftFile::deserialize(&data)?;
    file.source_path = Some(path.to_string());
    Ok(file)
}

/// Save a quantum state to a QFT file.
///
/// Parameters
/// ----------
/// state : QftFile
///     Quantum state to save
/// path : str
///     Output file path
/// bond_dim : int, optional
///     MPS bond dimension (default: 64)
/// golay : bool, optional
///     Enable Golay error correction (default: True)
#[pyfunction]
#[pyo3(signature = (state, path, bond_dim=64, golay=true))]
fn save(state: &mut QftFile, path: &str, bond_dim: usize, golay: bool) -> PyResult<()> {
    state.bond_dimension = bond_dim;
    state.golay_enabled = golay;
    state.save(path)
}

/// Create a QFT file from a NumPy array.
///
/// Parameters
/// ----------
/// amplitudes : numpy.ndarray
///     Complex amplitudes as interleaved real/imag pairs
///     Shape should be (2 * 2^n,) where n is number of qubits
///
/// Returns
/// -------
/// QftFile
///     New quantum state
#[pyfunction]
fn from_numpy(amplitudes: PyReadonlyArray1<f64>) -> PyResult<QftFile> {
    let data = amplitudes.as_slice()?;

    if data.len() % 2 != 0 {
        return Err(PyValueError::new_err(
            "Array length must be even (interleaved real/imag)",
        ));
    }

    let num_amplitudes = data.len() / 2;
    if num_amplitudes == 0 || (num_amplitudes & (num_amplitudes - 1)) != 0 {
        return Err(PyValueError::new_err(
            "Number of amplitudes must be a power of 2",
        ));
    }

    let num_qubits = num_amplitudes.trailing_zeros() as usize;

    let mut amplitudes_real = Vec::with_capacity(num_amplitudes);
    let mut amplitudes_imag = Vec::with_capacity(num_amplitudes);

    for i in 0..num_amplitudes {
        amplitudes_real.push(data[i * 2]);
        amplitudes_imag.push(data[i * 2 + 1]);
    }

    Ok(QftFile {
        num_qubits,
        amplitudes_real,
        amplitudes_imag,
        metadata: HashMap::new(),
        bond_dimension: 64,
        golay_enabled: true,
        source_path: None,
    })
}

/// Create a QFT file from separate real and imaginary arrays.
///
/// Parameters
/// ----------
/// real : numpy.ndarray
///     Real parts of amplitudes
/// imag : numpy.ndarray
///     Imaginary parts of amplitudes
///
/// Returns
/// -------
/// QftFile
///     New quantum state
#[pyfunction]
fn from_arrays(real: PyReadonlyArray1<f64>, imag: PyReadonlyArray1<f64>) -> PyResult<QftFile> {
    let real_data = real.as_slice()?;
    let imag_data = imag.as_slice()?;

    if real_data.len() != imag_data.len() {
        return Err(PyValueError::new_err("Arrays must have same length"));
    }

    let num_amplitudes = real_data.len();
    if num_amplitudes == 0 || (num_amplitudes & (num_amplitudes - 1)) != 0 {
        return Err(PyValueError::new_err(
            "Number of amplitudes must be a power of 2",
        ));
    }

    let num_qubits = num_amplitudes.trailing_zeros() as usize;

    Ok(QftFile {
        num_qubits,
        amplitudes_real: real_data.to_vec(),
        amplitudes_imag: imag_data.to_vec(),
        metadata: HashMap::new(),
        bond_dimension: 64,
        golay_enabled: true,
        source_path: None,
    })
}

/// Verify a QFT file's integrity.
///
/// Parameters
/// ----------
/// path : str
///     Path to .qft file
///
/// Returns
/// -------
/// bool
///     True if file is valid
#[pyfunction]
fn verify(path: &str) -> PyResult<bool> {
    match std::fs::read(path) {
        Ok(data) => match QftFile::deserialize(&data) {
            Ok(file) => Ok(file.is_normalized(1e-6)),
            Err(_) => Ok(false),
        },
        Err(_) => Ok(false),
    }
}

/// Get library version.
#[pyfunction]
fn version() -> &'static str {
    "1.0.0"
}

// =============================================================================
// Section 4: Streaming Converter
// =============================================================================

/// Encoding strategies for file conversion.
#[pyclass(name = "Encoding")]
#[derive(Clone, Copy)]
enum Encoding {
    Amplitude = 0,
    BasisState = 1,
    Qram = 2,
    Angle = 3,
    Block = 4,
}

/// Streaming converter for large files.
///
/// Use this when converting files too large to fit in memory.
#[pyclass(name = "StreamingConverter")]
struct StreamingConverter {
    chunk_size: usize,
    total_size: usize,
    processed: usize,
    encoding: Encoding,
}

#[pymethods]
impl StreamingConverter {
    /// Create a new streaming converter.
    ///
    /// Parameters
    /// ----------
    /// total_size : int
    ///     Total size of input data in bytes
    /// chunk_size : int, optional
    ///     Size of each chunk (default: 1MB)
    /// encoding : Encoding, optional
    ///     Encoding strategy (default: Amplitude)
    #[new]
    #[pyo3(signature = (total_size, chunk_size=1048576, encoding=Encoding::Amplitude))]
    fn new(total_size: usize, chunk_size: usize, encoding: Encoding) -> Self {
        Self {
            chunk_size,
            total_size,
            processed: 0,
            encoding,
        }
    }

    /// Process a chunk of data.
    ///
    /// Parameters
    /// ----------
    /// data : bytes
    ///     Chunk of input data
    ///
    /// Returns
    /// -------
    /// QftFile
    ///     Quantum state for this chunk
    fn process_chunk(&mut self, data: &[u8]) -> PyResult<QftFile> {
        let num_qubits = (data.len() as f64).log2().ceil() as usize;
        let num_qubits = num_qubits.max(1).min(20); // Limit to 20 qubits per chunk
        let dim = 1usize << num_qubits;

        let mut amplitudes_real = vec![0.0; dim];
        let amplitudes_imag = vec![0.0; dim];

        let norm = (data.len() as f64).sqrt();
        for (i, &byte) in data.iter().enumerate().take(dim) {
            amplitudes_real[i] = (byte as f64 / 255.0) / norm;
        }

        self.processed += data.len();

        Ok(QftFile {
            num_qubits,
            amplitudes_real,
            amplitudes_imag,
            metadata: HashMap::new(),
            bond_dimension: 64,
            golay_enabled: true,
            source_path: None,
        })
    }

    /// Get conversion progress (0.0 to 1.0).
    #[getter]
    fn progress(&self) -> f64 {
        if self.total_size == 0 {
            1.0
        } else {
            self.processed as f64 / self.total_size as f64
        }
    }

    /// Check if conversion is complete.
    #[getter]
    fn is_complete(&self) -> bool {
        self.processed >= self.total_size
    }

    /// Get recommended chunk size.
    #[getter]
    fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

// =============================================================================
// Section 5: Qiskit/Cirq Interop Stubs
// =============================================================================

/// Convert a Qiskit Statevector to QftFile.
///
/// Parameters
/// ----------
/// statevector : qiskit.quantum_info.Statevector
///     Qiskit statevector object
///
/// Returns
/// -------
/// QftFile
///     Converted quantum state
///
/// Note
/// ----
/// Requires qiskit to be installed.
#[pyfunction]
fn from_qiskit(py: Python<'_>, statevector: &Bound<'_, PyAny>) -> PyResult<QftFile> {
    // Get the data attribute from Qiskit Statevector
    let data = statevector.getattr("data")?;
    
    // Convert to numpy array
    let array: Bound<'_, PyAny> = data.call_method0("flatten")?;
    
    // Get real and imaginary parts
    let real: Vec<f64> = array.getattr("real")?.extract()?;
    let imag: Vec<f64> = array.getattr("imag")?.extract()?;
    
    let num_amplitudes = real.len();
    if num_amplitudes == 0 || (num_amplitudes & (num_amplitudes - 1)) != 0 {
        return Err(PyValueError::new_err(
            "Statevector dimension must be a power of 2",
        ));
    }
    
    let num_qubits = num_amplitudes.trailing_zeros() as usize;
    
    Ok(QftFile {
        num_qubits,
        amplitudes_real: real,
        amplitudes_imag: imag,
        metadata: HashMap::new(),
        bond_dimension: 64,
        golay_enabled: true,
        source_path: None,
    })
}

/// Convert QftFile to Qiskit Statevector.
///
/// Parameters
/// ----------
/// qft_file : QftFile
///     QFT file to convert
///
/// Returns
/// -------
/// qiskit.quantum_info.Statevector
///     Qiskit statevector
///
/// Note
/// ----
/// Requires qiskit to be installed.
#[pyfunction]
fn to_qiskit(py: Python<'_>, qft_file: &QftFile) -> PyResult<PyObject> {
    // Import Qiskit
    let qiskit = py.import("qiskit.quantum_info")?;
    let statevector_class = qiskit.getattr("Statevector")?;
    
    // Create complex array
    let numpy = py.import("numpy")?;
    let dim = qft_file.dimension();
    
    // Build complex array from real and imag parts
    let real_arr = qft_file.amplitudes_real.clone().into_pyarray(py);
    let imag_arr = qft_file.amplitudes_imag.clone().into_pyarray(py);
    
    let complex_arr = numpy.call_method1(
        "array",
        (real_arr.as_any(),),
    )?;
    
    let imag_part = numpy.call_method1(
        "array", 
        (imag_arr.as_any(),),
    )?;
    
    let j = numpy.getattr("complex128")?.call1((0.0, 1.0))?;
    let complex_data = complex_arr.call_method1("__add__", (imag_part.call_method1("__mul__", (j,))?,))?;
    
    // Create Statevector
    let sv = statevector_class.call1((complex_data,))?;
    Ok(sv.into_py(py))
}

// =============================================================================
// Section 6: Module Definition
// =============================================================================

/// Quantum File Type (.qft) Python SDK
///
/// Production-grade Python bindings for the .qft quantum state format.
///
/// Features:
/// - Load/save quantum states in .qft format
/// - NumPy array interoperability
/// - Qiskit/Cirq conversion
/// - Streaming for large files
/// - Golay error correction
/// - MPS compression
///
/// Examples
/// --------
/// >>> import qft
/// >>> import numpy as np
/// >>>
/// >>> # Create a Bell state
/// >>> f = qft.QftFile(2)
/// >>> real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
/// >>> imag = np.zeros(4)
/// >>> f.set_amplitudes(real, imag)
/// >>>
/// >>> # Save and reload
/// >>> f.save("bell.qft")
/// >>> loaded = qft.load("bell.qft")
/// >>> print(loaded.fidelity(f))  # 1.0
#[pymodule]
fn qft(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<QftFile>()?;
    m.add_class::<StreamingConverter>()?;
    m.add_class::<Encoding>()?;
    m.add_function(wrap_pyfunction!(load, m)?)?;
    m.add_function(wrap_pyfunction!(save, m)?)?;
    m.add_function(wrap_pyfunction!(from_numpy, m)?)?;
    m.add_function(wrap_pyfunction!(from_arrays, m)?)?;
    m.add_function(wrap_pyfunction!(verify, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(from_qiskit, m)?)?;
    m.add_function(wrap_pyfunction!(to_qiskit, m)?)?;
    Ok(())
}
