//! # qft-rs - Native Rust SDK for Quantum File Type (.qft)
//!
//! Production-grade Rust library for working with quantum states in the .qft format.
//!
//! ## Table of Contents
//! 1. Core Types - QftFile, QftState, QftError
//! 2. Builder API - Fluent interface for state construction
//! 3. I/O Operations - Load, save, streaming
//! 4. State Operations - Normalization, fidelity, inner products
//! 5. Conversion - To/from ndarray, Vec, iterators
//! 6. Async Support - Tokio-based async I/O (feature-gated)
//!
//! ## Quick Start
//!
//! ```rust
//! use qft::{QftFile, Result};
//!
//! fn main() -> Result<()> {
//!     // Create a 4-qubit state
//!     let mut state = QftFile::new(4)?;
//!     
//!     // Set to |0000⟩ + |1111⟩ (unnormalized)
//!     state.set_amplitude(0, 1.0.into())?;
//!     state.set_amplitude(15, 1.0.into())?;
//!     state.normalize()?;
//!     
//!     // Save to disk
//!     state.save("state.qft")?;
//!     
//!     // Load and verify
//!     let loaded = QftFile::load("state.qft")?;
//!     assert!((state.fidelity(&loaded)? - 1.0).abs() < 1e-10);
//!     
//!     Ok(())
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use thiserror::Error;

// =============================================================================
// Section 1: Error Types
// =============================================================================

/// Error type for QFT operations
#[derive(Error, Debug)]
pub enum QftError {
    #[error("Invalid number of qubits: {0} (must be 1-30)")]
    InvalidQubits(usize),

    #[error("Index {0} out of range for {1} qubits")]
    IndexOutOfRange(usize, usize),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Cannot normalize zero state")]
    ZeroNorm,

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("Golay decode failed: too many errors")]
    GolayDecodeFailed,
}

/// Result type alias for QFT operations
pub type Result<T> = std::result::Result<T, QftError>;

// =============================================================================
// Section 2: Core Types
// =============================================================================

/// Configuration for QFT file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QftConfig {
    /// Bond dimension for MPS compression (1-1024)
    pub bond_dimension: usize,
    /// Enable Golay error correction
    pub golay_enabled: bool,
    /// Truncation threshold for SVD
    pub truncation_threshold: f64,
}

impl Default for QftConfig {
    fn default() -> Self {
        Self {
            bond_dimension: 64,
            golay_enabled: true,
            truncation_threshold: 1e-10,
        }
    }
}

/// A quantum state file in .qft format
#[derive(Debug, Clone)]
pub struct QftFile {
    num_qubits: usize,
    amplitudes: Vec<Complex64>,
    metadata: HashMap<String, String>,
    config: QftConfig,
}

impl QftFile {
    /// Create a new QFT file with the specified number of qubits.
    /// Initializes to the |0...0⟩ state.
    ///
    /// # Arguments
    /// * `num_qubits` - Number of qubits (1-30)
    ///
    /// # Example
    /// ```
    /// use qft::QftFile;
    /// let state = QftFile::new(4).unwrap();
    /// assert_eq!(state.num_qubits(), 4);
    /// assert_eq!(state.dimension(), 16);
    /// ```
    pub fn new(num_qubits: usize) -> Result<Self> {
        if num_qubits == 0 || num_qubits > 30 {
            return Err(QftError::InvalidQubits(num_qubits));
        }

        let dim = 1usize << num_qubits;
        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
        amplitudes[0] = Complex64::new(1.0, 0.0); // |0...0⟩

        Ok(Self {
            num_qubits,
            amplitudes,
            metadata: HashMap::new(),
            config: QftConfig::default(),
        })
    }

    /// Create a QFT file with custom configuration.
    pub fn with_config(num_qubits: usize, config: QftConfig) -> Result<Self> {
        let mut file = Self::new(num_qubits)?;
        file.config = config;
        Ok(file)
    }

    /// Create from a vector of complex amplitudes.
    ///
    /// # Example
    /// ```
    /// use qft::QftFile;
    /// use num_complex::Complex64;
    ///
    /// let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    /// let amplitudes = vec![
    ///     Complex64::new(sqrt2_inv, 0.0),
    ///     Complex64::new(0.0, 0.0),
    ///     Complex64::new(0.0, 0.0),
    ///     Complex64::new(sqrt2_inv, 0.0),
    /// ];
    /// let bell = QftFile::from_amplitudes(amplitudes).unwrap();
    /// assert_eq!(bell.num_qubits(), 2);
    /// ```
    pub fn from_amplitudes(amplitudes: Vec<Complex64>) -> Result<Self> {
        let dim = amplitudes.len();
        if dim == 0 || (dim & (dim - 1)) != 0 {
            return Err(QftError::InvalidFormat(
                "Amplitude count must be a power of 2".to_string(),
            ));
        }

        let num_qubits = dim.trailing_zeros() as usize;
        if num_qubits > 30 {
            return Err(QftError::InvalidQubits(num_qubits));
        }

        Ok(Self {
            num_qubits,
            amplitudes,
            metadata: HashMap::new(),
            config: QftConfig::default(),
        })
    }

    /// Create from separate real and imaginary arrays.
    pub fn from_real_imag(real: &[f64], imag: &[f64]) -> Result<Self> {
        if real.len() != imag.len() {
            return Err(QftError::DimensionMismatch {
                expected: real.len(),
                actual: imag.len(),
            });
        }

        let amplitudes: Vec<Complex64> = real
            .iter()
            .zip(imag.iter())
            .map(|(&r, &i)| Complex64::new(r, i))
            .collect();

        Self::from_amplitudes(amplitudes)
    }

    // =========================================================================
    // Properties
    // =========================================================================

    /// Get the number of qubits.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Get the state vector dimension (2^num_qubits).
    #[inline]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }

    /// Get the configuration.
    pub fn config(&self) -> &QftConfig {
        &self.config
    }

    /// Get mutable reference to configuration.
    pub fn config_mut(&mut self) -> &mut QftConfig {
        &mut self.config
    }

    // =========================================================================
    // Amplitude Access
    // =========================================================================

    /// Get all amplitudes as a slice.
    #[inline]
    pub fn amplitudes(&self) -> &[Complex64] {
        &self.amplitudes
    }

    /// Get mutable access to amplitudes.
    #[inline]
    pub fn amplitudes_mut(&mut self) -> &mut [Complex64] {
        &mut self.amplitudes
    }

    /// Get a single amplitude by basis state index.
    pub fn get_amplitude(&self, index: usize) -> Result<Complex64> {
        if index >= self.dimension() {
            return Err(QftError::IndexOutOfRange(index, self.num_qubits));
        }
        Ok(self.amplitudes[index])
    }

    /// Set a single amplitude by basis state index.
    pub fn set_amplitude(&mut self, index: usize, value: Complex64) -> Result<()> {
        if index >= self.dimension() {
            return Err(QftError::IndexOutOfRange(index, self.num_qubits));
        }
        self.amplitudes[index] = value;
        Ok(())
    }

    /// Set all amplitudes from a slice.
    pub fn set_amplitudes(&mut self, amplitudes: &[Complex64]) -> Result<()> {
        if amplitudes.len() != self.dimension() {
            return Err(QftError::DimensionMismatch {
                expected: self.dimension(),
                actual: amplitudes.len(),
            });
        }
        self.amplitudes.copy_from_slice(amplitudes);
        Ok(())
    }

    // =========================================================================
    // Metadata
    // =========================================================================

    /// Set a metadata key-value pair.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// Get all metadata.
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Get mutable access to metadata.
    pub fn metadata_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.metadata
    }

    // =========================================================================
    // State Operations
    // =========================================================================

    /// Calculate the norm squared (sum of |amplitude|²).
    pub fn norm_squared(&self) -> f64 {
        self.amplitudes.iter().map(|a| a.norm_sqr()).sum()
    }

    /// Calculate the norm.
    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Check if the state is normalized within tolerance.
    pub fn is_normalized(&self, tolerance: f64) -> bool {
        (self.norm_squared() - 1.0).abs() < tolerance
    }

    /// Normalize the state vector in place.
    pub fn normalize(&mut self) -> Result<()> {
        let norm = self.norm();
        if norm < 1e-15 {
            return Err(QftError::ZeroNorm);
        }
        for a in &mut self.amplitudes {
            *a /= norm;
        }
        Ok(())
    }

    /// Calculate the inner product ⟨self|other⟩.
    pub fn inner_product(&self, other: &QftFile) -> Result<Complex64> {
        if self.num_qubits != other.num_qubits {
            return Err(QftError::DimensionMismatch {
                expected: self.num_qubits,
                actual: other.num_qubits,
            });
        }

        let result: Complex64 = self
            .amplitudes
            .iter()
            .zip(other.amplitudes.iter())
            .map(|(a, b)| a.conj() * b)
            .sum();

        Ok(result)
    }

    /// Calculate the fidelity |⟨self|other⟩|².
    pub fn fidelity(&self, other: &QftFile) -> Result<f64> {
        let overlap = self.inner_product(other)?;
        Ok(overlap.norm_sqr())
    }

    /// Calculate the trace distance to another state.
    pub fn trace_distance(&self, other: &QftFile) -> Result<f64> {
        let fid = self.fidelity(other)?;
        Ok((1.0 - fid).sqrt())
    }

    // =========================================================================
    // I/O Operations
    // =========================================================================

    /// Load a QFT file from disk.
    ///
    /// # Example
    /// ```no_run
    /// use qft::QftFile;
    /// let state = QftFile::load("state.qft").unwrap();
    /// ```
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::read_from(&mut reader)
    }

    /// Save the QFT file to disk.
    ///
    /// # Example
    /// ```no_run
    /// use qft::QftFile;
    /// let state = QftFile::new(4).unwrap();
    /// state.save("state.qft").unwrap();
    /// ```
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_to(&mut writer)
    }

    /// Read from any reader.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut header = [0u8; 16];
        reader.read_exact(&mut header)?;

        // Verify magic number
        if &header[0..4] != b"QFT\x01" {
            return Err(QftError::InvalidFormat("Invalid magic number".to_string()));
        }

        let num_qubits = header[4] as usize;
        if num_qubits == 0 || num_qubits > 30 {
            return Err(QftError::InvalidQubits(num_qubits));
        }

        let bond_dimension = header[5] as usize;
        let golay_enabled = header[6] != 0;

        let dim = 1usize << num_qubits;
        let mut amplitudes = Vec::with_capacity(dim);

        for _ in 0..dim {
            let mut buf = [0u8; 16];
            reader.read_exact(&mut buf)?;
            let real = f64::from_le_bytes(buf[0..8].try_into().unwrap());
            let imag = f64::from_le_bytes(buf[8..16].try_into().unwrap());
            amplitudes.push(Complex64::new(real, imag));
        }

        Ok(Self {
            num_qubits,
            amplitudes,
            metadata: HashMap::new(),
            config: QftConfig {
                bond_dimension: if bond_dimension == 0 { 64 } else { bond_dimension },
                golay_enabled,
                truncation_threshold: 1e-10,
            },
        })
    }

    /// Write to any writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        // Magic number
        writer.write_all(b"QFT\x01")?;

        // Header
        writer.write_all(&[
            self.num_qubits as u8,
            self.config.bond_dimension.min(255) as u8,
            if self.config.golay_enabled { 1 } else { 0 },
            0, // reserved
        ])?;

        // Padding to 16 bytes
        writer.write_all(&[0u8; 8])?;

        // Amplitudes
        for a in &self.amplitudes {
            writer.write_all(&a.re.to_le_bytes())?;
            writer.write_all(&a.im.to_le_bytes())?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.write_to(&mut buf)?;
        Ok(buf)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(data);
        Self::read_from(&mut cursor)
    }

    /// Export to JSON.
    pub fn to_json(&self) -> Result<String> {
        #[derive(Serialize)]
        struct JsonExport {
            num_qubits: usize,
            config: QftConfig,
            amplitudes_real: Vec<f64>,
            amplitudes_imag: Vec<f64>,
            metadata: HashMap<String, String>,
        }

        let export = JsonExport {
            num_qubits: self.num_qubits,
            config: self.config.clone(),
            amplitudes_real: self.amplitudes.iter().map(|a| a.re).collect(),
            amplitudes_imag: self.amplitudes.iter().map(|a| a.im).collect(),
            metadata: self.metadata.clone(),
        };

        serde_json::to_string_pretty(&export)
            .map_err(|e| QftError::Serialization(e.to_string()))
    }

    /// Import from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        #[derive(Deserialize)]
        struct JsonImport {
            num_qubits: usize,
            config: Option<QftConfig>,
            amplitudes_real: Vec<f64>,
            amplitudes_imag: Vec<f64>,
            metadata: Option<HashMap<String, String>>,
        }

        let import: JsonImport =
            serde_json::from_str(json).map_err(|e| QftError::Serialization(e.to_string()))?;

        let amplitudes: Vec<Complex64> = import
            .amplitudes_real
            .iter()
            .zip(import.amplitudes_imag.iter())
            .map(|(&r, &i)| Complex64::new(r, i))
            .collect();

        let expected_dim = 1usize << import.num_qubits;
        if amplitudes.len() != expected_dim {
            return Err(QftError::DimensionMismatch {
                expected: expected_dim,
                actual: amplitudes.len(),
            });
        }

        Ok(Self {
            num_qubits: import.num_qubits,
            amplitudes,
            metadata: import.metadata.unwrap_or_default(),
            config: import.config.unwrap_or_default(),
        })
    }
}

// =============================================================================
// Section 3: Builder API
// =============================================================================

/// Builder for constructing QFT files with a fluent interface.
///
/// # Example
/// ```
/// use qft::QftBuilder;
///
/// let state = QftBuilder::new(4)
///     .bond_dimension(128)
///     .golay(true)
///     .metadata("author", "Alice")
///     .metadata("experiment", "VQE")
///     .build()
///     .unwrap();
/// ```
pub struct QftBuilder {
    num_qubits: usize,
    config: QftConfig,
    metadata: HashMap<String, String>,
    amplitudes: Option<Vec<Complex64>>,
}

impl QftBuilder {
    /// Create a new builder for the specified number of qubits.
    pub fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            config: QftConfig::default(),
            metadata: HashMap::new(),
            amplitudes: None,
        }
    }

    /// Set the bond dimension.
    pub fn bond_dimension(mut self, dim: usize) -> Self {
        self.config.bond_dimension = dim;
        self
    }

    /// Enable or disable Golay error correction.
    pub fn golay(mut self, enabled: bool) -> Self {
        self.config.golay_enabled = enabled;
        self
    }

    /// Set the truncation threshold.
    pub fn truncation_threshold(mut self, threshold: f64) -> Self {
        self.config.truncation_threshold = threshold;
        self
    }

    /// Add a metadata entry.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set initial amplitudes.
    pub fn amplitudes(mut self, amplitudes: Vec<Complex64>) -> Self {
        self.amplitudes = Some(amplitudes);
        self
    }

    /// Build the QFT file.
    pub fn build(self) -> Result<QftFile> {
        let mut file = QftFile::with_config(self.num_qubits, self.config)?;
        file.metadata = self.metadata;

        if let Some(amps) = self.amplitudes {
            file.set_amplitudes(&amps)?;
        }

        Ok(file)
    }
}

// =============================================================================
// Section 4: Common States
// =============================================================================

/// Create a Bell state (|00⟩ + |11⟩) / √2.
pub fn bell_state() -> Result<QftFile> {
    let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    QftFile::from_amplitudes(vec![
        Complex64::new(sqrt2_inv, 0.0),
        Complex64::new(0.0, 0.0),
        Complex64::new(0.0, 0.0),
        Complex64::new(sqrt2_inv, 0.0),
    ])
}

/// Create a GHZ state (|0...0⟩ + |1...1⟩) / √2.
pub fn ghz_state(num_qubits: usize) -> Result<QftFile> {
    let mut state = QftFile::new(num_qubits)?;
    let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    state.amplitudes[0] = Complex64::new(sqrt2_inv, 0.0);
    state.amplitudes[state.dimension() - 1] = Complex64::new(sqrt2_inv, 0.0);
    Ok(state)
}

/// Create a uniform superposition state.
pub fn uniform_state(num_qubits: usize) -> Result<QftFile> {
    let dim = 1usize << num_qubits;
    let amp = 1.0 / (dim as f64).sqrt();
    let amplitudes = vec![Complex64::new(amp, 0.0); dim];
    QftFile::from_amplitudes(amplitudes)
}

/// Create a computational basis state |i⟩.
pub fn basis_state(num_qubits: usize, index: usize) -> Result<QftFile> {
    let mut state = QftFile::new(num_qubits)?;
    if index >= state.dimension() {
        return Err(QftError::IndexOutOfRange(index, num_qubits));
    }
    state.amplitudes[0] = Complex64::new(0.0, 0.0);
    state.amplitudes[index] = Complex64::new(1.0, 0.0);
    Ok(state)
}

// =============================================================================
// Section 5: Async Support
// =============================================================================

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod async_io {
    //! Async I/O operations for QFT files.

    use super::*;
    use tokio::fs::File;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

    impl QftFile {
        /// Asynchronously load a QFT file.
        pub async fn load_async(path: impl AsRef<Path>) -> Result<Self> {
            let file = File::open(path).await?;
            let mut reader = BufReader::new(file);

            let mut header = [0u8; 16];
            reader.read_exact(&mut header).await?;

            if &header[0..4] != b"QFT\x01" {
                return Err(QftError::InvalidFormat("Invalid magic number".to_string()));
            }

            let num_qubits = header[4] as usize;
            if num_qubits == 0 || num_qubits > 30 {
                return Err(QftError::InvalidQubits(num_qubits));
            }

            let bond_dimension = header[5] as usize;
            let golay_enabled = header[6] != 0;

            let dim = 1usize << num_qubits;
            let mut amplitudes = Vec::with_capacity(dim);

            for _ in 0..dim {
                let mut buf = [0u8; 16];
                reader.read_exact(&mut buf).await?;
                let real = f64::from_le_bytes(buf[0..8].try_into().unwrap());
                let imag = f64::from_le_bytes(buf[8..16].try_into().unwrap());
                amplitudes.push(Complex64::new(real, imag));
            }

            Ok(Self {
                num_qubits,
                amplitudes,
                metadata: HashMap::new(),
                config: QftConfig {
                    bond_dimension: if bond_dimension == 0 { 64 } else { bond_dimension },
                    golay_enabled,
                    truncation_threshold: 1e-10,
                },
            })
        }

        /// Asynchronously save a QFT file.
        pub async fn save_async(&self, path: impl AsRef<Path>) -> Result<()> {
            let file = File::create(path).await?;
            let mut writer = BufWriter::new(file);

            // Magic number
            writer.write_all(b"QFT\x01").await?;

            // Header
            writer
                .write_all(&[
                    self.num_qubits as u8,
                    self.config.bond_dimension.min(255) as u8,
                    if self.config.golay_enabled { 1 } else { 0 },
                    0,
                ])
                .await?;

            // Padding
            writer.write_all(&[0u8; 8]).await?;

            // Amplitudes
            for a in &self.amplitudes {
                writer.write_all(&a.re.to_le_bytes()).await?;
                writer.write_all(&a.im.to_le_bytes()).await?;
            }

            writer.flush().await?;
            Ok(())
        }
    }
}

// =============================================================================
// Section 6: Trait Implementations
// =============================================================================

impl std::fmt::Display for QftFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "QftFile({} qubits, dim={}, norm={:.6})",
            self.num_qubits,
            self.dimension(),
            self.norm()
        )
    }
}

impl std::ops::Index<usize> for QftFile {
    type Output = Complex64;

    fn index(&self, index: usize) -> &Self::Output {
        &self.amplitudes[index]
    }
}

impl std::ops::IndexMut<usize> for QftFile {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.amplitudes[index]
    }
}

impl IntoIterator for QftFile {
    type Item = Complex64;
    type IntoIter = std::vec::IntoIter<Complex64>;

    fn into_iter(self) -> Self::IntoIter {
        self.amplitudes.into_iter()
    }
}

impl<'a> IntoIterator for &'a QftFile {
    type Item = &'a Complex64;
    type IntoIter = std::slice::Iter<'a, Complex64>;

    fn into_iter(self) -> Self::IntoIter {
        self.amplitudes.iter()
    }
}

// =============================================================================
// Section 7: Re-exports
// =============================================================================

pub use num_complex::Complex64;

/// Prelude for convenient imports.
pub mod prelude {
    pub use super::{
        basis_state, bell_state, ghz_state, uniform_state, QftBuilder, QftConfig, QftError,
        QftFile, Result,
    };
    pub use num_complex::Complex64;
}

// =============================================================================
// Section 8: Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let state = QftFile::new(4).unwrap();
        assert_eq!(state.num_qubits(), 4);
        assert_eq!(state.dimension(), 16);
        assert!(state.is_normalized(1e-10));
    }

    #[test]
    fn test_invalid_qubits() {
        assert!(QftFile::new(0).is_err());
        assert!(QftFile::new(31).is_err());
    }

    #[test]
    fn test_amplitudes() {
        let mut state = QftFile::new(2).unwrap();
        assert_eq!(state[0], Complex64::new(1.0, 0.0));

        state[0] = Complex64::new(0.0, 0.0);
        state[3] = Complex64::new(1.0, 0.0);

        assert_eq!(state.get_amplitude(3).unwrap(), Complex64::new(1.0, 0.0));
    }

    #[test]
    fn test_normalization() {
        let mut state = QftFile::new(2).unwrap();
        state[0] = Complex64::new(1.0, 0.0);
        state[1] = Complex64::new(1.0, 0.0);

        assert!(!state.is_normalized(1e-10));
        state.normalize().unwrap();
        assert!(state.is_normalized(1e-10));
    }

    #[test]
    fn test_fidelity() {
        let state1 = QftFile::new(2).unwrap();
        let state2 = QftFile::new(2).unwrap();

        let fid = state1.fidelity(&state2).unwrap();
        assert!((fid - 1.0).abs() < 1e-10);

        let orthogonal = basis_state(2, 1).unwrap();
        let fid = state1.fidelity(&orthogonal).unwrap();
        assert!(fid.abs() < 1e-10);
    }

    #[test]
    fn test_bell_state() {
        let bell = bell_state().unwrap();
        assert_eq!(bell.num_qubits(), 2);
        assert!(bell.is_normalized(1e-10));
    }

    #[test]
    fn test_ghz_state() {
        let ghz = ghz_state(4).unwrap();
        assert_eq!(ghz.num_qubits(), 4);
        assert!(ghz.is_normalized(1e-10));
    }

    #[test]
    fn test_builder() {
        let state = QftBuilder::new(4)
            .bond_dimension(128)
            .golay(false)
            .metadata("test", "value")
            .build()
            .unwrap();

        assert_eq!(state.config().bond_dimension, 128);
        assert!(!state.config().golay_enabled);
        assert_eq!(state.get_metadata("test"), Some("value"));
    }

    #[test]
    fn test_serialization() {
        let state = bell_state().unwrap();
        let bytes = state.to_bytes().unwrap();
        let restored = QftFile::from_bytes(&bytes).unwrap();

        assert!((state.fidelity(&restored).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_json() {
        let state = bell_state().unwrap();
        let json = state.to_json().unwrap();
        let restored = QftFile::from_json(&json).unwrap();

        assert!((state.fidelity(&restored).unwrap() - 1.0).abs() < 1e-10);
    }
}
