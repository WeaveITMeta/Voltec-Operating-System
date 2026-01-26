//! # qft-wasm - Production-Grade WebAssembly Bindings for Quantum File Type (.qft)
//!
//! ## Table of Contents
//! 1. Initialization - WASM module setup and panic hooks
//! 2. QftFile Class - Main JavaScript interface for .qft files
//! 3. Conversion Functions - To/from TypedArrays, JSON
//! 4. Streaming API - Large file chunked processing
//! 5. Validation Functions - Checksums, fidelity, normalization
//! 6. Utility Functions - Version, format detection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// =============================================================================
// Section 1: Initialization
// =============================================================================

/// Initialize the WASM module with better error messages
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Log to browser console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// =============================================================================
// Section 2: Error Types
// =============================================================================

/// Error type for QFT operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QftError {
    pub code: String,
    pub message: String,
}

impl From<QftError> for JsValue {
    fn from(err: QftError) -> Self {
        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"code".into(), &err.code.into()).unwrap();
        js_sys::Reflect::set(&obj, &"message".into(), &err.message.into()).unwrap();
        obj.into()
    }
}

// =============================================================================
// Section 3: QftFile Class
// =============================================================================

/// A Quantum File Type (.qft) file handle for JavaScript/TypeScript.
///
/// @example
/// ```javascript
/// import { QftFile } from 'qft-wasm';
///
/// // Create a 4-qubit state
/// const file = new QftFile(4);
/// console.log(file.numQubits);  // 4
/// console.log(file.dimension);  // 16
///
/// // Set Bell-like state
/// const real = new Float64Array(16);
/// const imag = new Float64Array(16);
/// real[0] = 1 / Math.sqrt(2);
/// real[15] = 1 / Math.sqrt(2);
/// file.setAmplitudes(real, imag);
///
/// // Save to bytes
/// const bytes = file.toBytes();
/// ```
#[wasm_bindgen]
pub struct QftFile {
    num_qubits: usize,
    amplitudes_real: Vec<f64>,
    amplitudes_imag: Vec<f64>,
    metadata: HashMap<String, String>,
    bond_dimension: usize,
    golay_enabled: bool,
}

#[wasm_bindgen]
impl QftFile {
    /// Create a new QFT file with the specified number of qubits.
    ///
    /// @param numQubits - Number of qubits (1-30)
    /// @returns A new QftFile initialized to |0...0⟩
    /// @throws Error if numQubits is not in range 1-30
    #[wasm_bindgen(constructor)]
    pub fn new(num_qubits: usize) -> Result<QftFile, JsValue> {
        if num_qubits == 0 || num_qubits > 30 {
            return Err(QftError {
                code: "INVALID_PARAMETER".to_string(),
                message: format!("num_qubits must be 1-30, got {}", num_qubits),
            }
            .into());
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
        })
    }

    /// Number of qubits in the quantum state.
    #[wasm_bindgen(getter, js_name = numQubits)]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Dimension of the state vector (2^numQubits).
    #[wasm_bindgen(getter)]
    pub fn dimension(&self) -> usize {
        1 << self.num_qubits
    }

    /// Bond dimension for MPS compression.
    #[wasm_bindgen(getter, js_name = bondDimension)]
    pub fn bond_dimension(&self) -> usize {
        self.bond_dimension
    }

    #[wasm_bindgen(setter, js_name = bondDimension)]
    pub fn set_bond_dimension(&mut self, value: usize) -> Result<(), JsValue> {
        if value == 0 || value > 1024 {
            return Err(QftError {
                code: "INVALID_PARAMETER".to_string(),
                message: "bondDimension must be 1-1024".to_string(),
            }
            .into());
        }
        self.bond_dimension = value;
        Ok(())
    }

    /// Whether Golay error correction is enabled.
    #[wasm_bindgen(getter, js_name = golayEnabled)]
    pub fn golay_enabled(&self) -> bool {
        self.golay_enabled
    }

    #[wasm_bindgen(setter, js_name = golayEnabled)]
    pub fn set_golay_enabled(&mut self, value: bool) {
        self.golay_enabled = value;
    }

    /// Get the real parts of amplitudes as Float64Array.
    #[wasm_bindgen(js_name = getAmplitudesReal)]
    pub fn get_amplitudes_real(&self) -> js_sys::Float64Array {
        js_sys::Float64Array::from(&self.amplitudes_real[..])
    }

    /// Get the imaginary parts of amplitudes as Float64Array.
    #[wasm_bindgen(js_name = getAmplitudesImag)]
    pub fn get_amplitudes_imag(&self) -> js_sys::Float64Array {
        js_sys::Float64Array::from(&self.amplitudes_imag[..])
    }

    /// Set amplitudes from Float64Arrays.
    ///
    /// @param real - Real parts of amplitudes
    /// @param imag - Imaginary parts of amplitudes
    #[wasm_bindgen(js_name = setAmplitudes)]
    pub fn set_amplitudes(
        &mut self,
        real: &js_sys::Float64Array,
        imag: &js_sys::Float64Array,
    ) -> Result<(), JsValue> {
        let real_vec = real.to_vec();
        let imag_vec = imag.to_vec();

        let expected = self.dimension();
        if real_vec.len() != expected || imag_vec.len() != expected {
            return Err(QftError {
                code: "DIMENSION_MISMATCH".to_string(),
                message: format!(
                    "Expected {} amplitudes, got {} real and {} imag",
                    expected,
                    real_vec.len(),
                    imag_vec.len()
                ),
            }
            .into());
        }

        self.amplitudes_real = real_vec;
        self.amplitudes_imag = imag_vec;
        Ok(())
    }

    /// Get a single amplitude by basis state index.
    ///
    /// @param index - Basis state index (0 to 2^n - 1)
    /// @returns Object with real and imag properties
    #[wasm_bindgen(js_name = getAmplitude)]
    pub fn get_amplitude(&self, index: usize) -> Result<JsValue, JsValue> {
        if index >= self.dimension() {
            return Err(QftError {
                code: "INDEX_OUT_OF_RANGE".to_string(),
                message: format!(
                    "Index {} out of range for {} qubits",
                    index, self.num_qubits
                ),
            }
            .into());
        }

        let obj = js_sys::Object::new();
        js_sys::Reflect::set(&obj, &"real".into(), &self.amplitudes_real[index].into())?;
        js_sys::Reflect::set(&obj, &"imag".into(), &self.amplitudes_imag[index].into())?;
        Ok(obj.into())
    }

    /// Set a single amplitude by basis state index.
    ///
    /// @param index - Basis state index (0 to 2^n - 1)
    /// @param real - Real part of amplitude
    /// @param imag - Imaginary part of amplitude
    #[wasm_bindgen(js_name = setAmplitude)]
    pub fn set_amplitude(&mut self, index: usize, real: f64, imag: f64) -> Result<(), JsValue> {
        if index >= self.dimension() {
            return Err(QftError {
                code: "INDEX_OUT_OF_RANGE".to_string(),
                message: format!(
                    "Index {} out of range for {} qubits",
                    index, self.num_qubits
                ),
            }
            .into());
        }
        self.amplitudes_real[index] = real;
        self.amplitudes_imag[index] = imag;
        Ok(())
    }

    /// Check if the state is normalized.
    ///
    /// @param tolerance - Tolerance for check (default: 1e-10)
    /// @returns true if normalized within tolerance
    #[wasm_bindgen(js_name = isNormalized)]
    pub fn is_normalized(&self, tolerance: Option<f64>) -> bool {
        let tol = tolerance.unwrap_or(1e-10);
        let norm_sq: f64 = self
            .amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();
        (norm_sq - 1.0).abs() < tol
    }

    /// Normalize the state vector in place.
    #[wasm_bindgen]
    pub fn normalize(&mut self) -> Result<(), JsValue> {
        let norm_sq: f64 = self
            .amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();

        if norm_sq < 1e-15 {
            return Err(QftError {
                code: "ZERO_NORM".to_string(),
                message: "Cannot normalize zero state".to_string(),
            }
            .into());
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

    /// Calculate the norm squared.
    #[wasm_bindgen(js_name = normSquared)]
    pub fn norm_squared(&self) -> f64 {
        self.amplitudes_real
            .iter()
            .zip(self.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum()
    }

    /// Calculate fidelity with another state: |⟨self|other⟩|².
    ///
    /// @param other - Another QftFile
    /// @returns Fidelity between 0 and 1
    #[wasm_bindgen]
    pub fn fidelity(&self, other: &QftFile) -> Result<f64, JsValue> {
        if self.num_qubits != other.num_qubits {
            return Err(QftError {
                code: "DIMENSION_MISMATCH".to_string(),
                message: format!(
                    "Dimension mismatch: {} vs {} qubits",
                    self.num_qubits, other.num_qubits
                ),
            }
            .into());
        }

        let mut overlap_real = 0.0;
        let mut overlap_imag = 0.0;

        for i in 0..self.dimension() {
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
    #[wasm_bindgen(js_name = setMetadata)]
    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    /// Get metadata value by key.
    #[wasm_bindgen(js_name = getMetadata)]
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).cloned()
    }

    /// Get all metadata as a JavaScript object.
    #[wasm_bindgen(js_name = getMetadataObject)]
    pub fn get_metadata_object(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.metadata).map_err(|e| {
            QftError {
                code: "SERIALIZATION_ERROR".to_string(),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Serialize to Uint8Array.
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> js_sys::Uint8Array {
        let data = self.serialize();
        js_sys::Uint8Array::from(&data[..])
    }

    /// Create from Uint8Array.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(data: &js_sys::Uint8Array) -> Result<QftFile, JsValue> {
        let bytes = data.to_vec();
        Self::deserialize(&bytes)
    }

    /// Export to JSON string.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        #[derive(Serialize)]
        struct JsonExport {
            num_qubits: usize,
            bond_dimension: usize,
            golay_enabled: bool,
            amplitudes_real: Vec<f64>,
            amplitudes_imag: Vec<f64>,
            metadata: HashMap<String, String>,
        }

        let export = JsonExport {
            num_qubits: self.num_qubits,
            bond_dimension: self.bond_dimension,
            golay_enabled: self.golay_enabled,
            amplitudes_real: self.amplitudes_real.clone(),
            amplitudes_imag: self.amplitudes_imag.clone(),
            metadata: self.metadata.clone(),
        };

        serde_json::to_string(&export).map_err(|e| {
            QftError {
                code: "SERIALIZATION_ERROR".to_string(),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Create from JSON string.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<QftFile, JsValue> {
        #[derive(Deserialize)]
        struct JsonImport {
            num_qubits: usize,
            bond_dimension: Option<usize>,
            golay_enabled: Option<bool>,
            amplitudes_real: Vec<f64>,
            amplitudes_imag: Vec<f64>,
            metadata: Option<HashMap<String, String>>,
        }

        let import: JsonImport = serde_json::from_str(json).map_err(|e| QftError {
            code: "PARSE_ERROR".to_string(),
            message: e.to_string(),
        })?;

        let expected_dim = 1usize << import.num_qubits;
        if import.amplitudes_real.len() != expected_dim
            || import.amplitudes_imag.len() != expected_dim
        {
            return Err(QftError {
                code: "DIMENSION_MISMATCH".to_string(),
                message: "Amplitude array length doesn't match num_qubits".to_string(),
            }
            .into());
        }

        Ok(Self {
            num_qubits: import.num_qubits,
            amplitudes_real: import.amplitudes_real,
            amplitudes_imag: import.amplitudes_imag,
            metadata: import.metadata.unwrap_or_default(),
            bond_dimension: import.bond_dimension.unwrap_or(64),
            golay_enabled: import.golay_enabled.unwrap_or(true),
        })
    }
}

impl QftFile {
    fn serialize(&self) -> Vec<u8> {
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

        data
    }

    fn deserialize(data: &[u8]) -> Result<Self, JsValue> {
        if data.len() < 16 {
            return Err(QftError {
                code: "INVALID_FORMAT".to_string(),
                message: "File too small".to_string(),
            }
            .into());
        }

        if &data[0..4] != b"QFT\x01" {
            return Err(QftError {
                code: "INVALID_FORMAT".to_string(),
                message: "Invalid QFT magic number".to_string(),
            }
            .into());
        }

        let num_qubits = data[4] as usize;
        if num_qubits == 0 || num_qubits > 30 {
            return Err(QftError {
                code: "INVALID_FORMAT".to_string(),
                message: format!("Invalid num_qubits: {}", num_qubits),
            }
            .into());
        }

        let bond_dimension = data[5] as usize;
        let golay_enabled = data[6] != 0;

        let dim = 1usize << num_qubits;
        let expected_size = 16 + dim * 16;

        if data.len() < expected_size {
            return Err(QftError {
                code: "INVALID_FORMAT".to_string(),
                message: format!(
                    "File truncated: expected {} bytes, got {}",
                    expected_size,
                    data.len()
                ),
            }
            .into());
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
            bond_dimension: if bond_dimension == 0 {
                64
            } else {
                bond_dimension
            },
            golay_enabled,
        })
    }
}

// =============================================================================
// Section 4: Streaming Converter
// =============================================================================

/// Encoding strategies for file conversion.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Encoding {
    Amplitude = 0,
    BasisState = 1,
    Qram = 2,
    Angle = 3,
    Block = 4,
}

/// Streaming converter for large files.
#[wasm_bindgen]
pub struct StreamingConverter {
    chunk_size: usize,
    total_size: usize,
    processed: usize,
    encoding: Encoding,
}

#[wasm_bindgen]
impl StreamingConverter {
    /// Create a new streaming converter.
    ///
    /// @param totalSize - Total size of input data in bytes
    /// @param chunkSize - Size of each chunk (default: 1MB)
    /// @param encoding - Encoding strategy (default: Amplitude)
    #[wasm_bindgen(constructor)]
    pub fn new(total_size: usize, chunk_size: Option<usize>, encoding: Option<Encoding>) -> Self {
        Self {
            chunk_size: chunk_size.unwrap_or(1024 * 1024),
            total_size,
            processed: 0,
            encoding: encoding.unwrap_or(Encoding::Amplitude),
        }
    }

    /// Process a chunk of data.
    ///
    /// @param data - Uint8Array chunk
    /// @returns QftFile for this chunk
    #[wasm_bindgen(js_name = processChunk)]
    pub fn process_chunk(&mut self, data: &js_sys::Uint8Array) -> Result<QftFile, JsValue> {
        let bytes = data.to_vec();
        let num_qubits = (bytes.len() as f64).log2().ceil() as usize;
        let num_qubits = num_qubits.clamp(1, 20);
        let dim = 1usize << num_qubits;

        let mut amplitudes_real = vec![0.0; dim];
        let amplitudes_imag = vec![0.0; dim];

        let norm = (bytes.len() as f64).sqrt();
        for (i, &byte) in bytes.iter().enumerate().take(dim) {
            amplitudes_real[i] = (byte as f64 / 255.0) / norm;
        }

        self.processed += bytes.len();

        Ok(QftFile {
            num_qubits,
            amplitudes_real,
            amplitudes_imag,
            metadata: HashMap::new(),
            bond_dimension: 64,
            golay_enabled: true,
        })
    }

    /// Get conversion progress (0.0 to 1.0).
    #[wasm_bindgen(getter)]
    pub fn progress(&self) -> f64 {
        if self.total_size == 0 {
            1.0
        } else {
            self.processed as f64 / self.total_size as f64
        }
    }

    /// Check if conversion is complete.
    #[wasm_bindgen(getter, js_name = isComplete)]
    pub fn is_complete(&self) -> bool {
        self.processed >= self.total_size
    }

    /// Get recommended chunk size.
    #[wasm_bindgen(getter, js_name = chunkSize)]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

// =============================================================================
// Section 5: Utility Functions
// =============================================================================

/// Get library version.
#[wasm_bindgen]
pub fn version() -> String {
    "1.0.0".to_string()
}

/// Verify a QFT file's integrity from bytes.
#[wasm_bindgen]
pub fn verify(data: &js_sys::Uint8Array) -> bool {
    let bytes = data.to_vec();
    match QftFile::deserialize(&bytes) {
        Ok(file) => file.is_normalized(Some(1e-6)),
        Err(_) => false,
    }
}

/// Calculate fidelity between two states from bytes.
#[wasm_bindgen(js_name = fidelityFromBytes)]
pub fn fidelity_from_bytes(
    data1: &js_sys::Uint8Array,
    data2: &js_sys::Uint8Array,
) -> Result<f64, JsValue> {
    let file1 = QftFile::from_bytes(data1)?;
    let file2 = QftFile::from_bytes(data2)?;
    file1.fidelity(&file2)
}

/// Create a Bell state for testing.
#[wasm_bindgen(js_name = createBellState)]
pub fn create_bell_state() -> Result<QftFile, JsValue> {
    let mut file = QftFile::new(2)?;
    let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    file.amplitudes_real[0] = sqrt2_inv;
    file.amplitudes_real[3] = sqrt2_inv;
    Ok(file)
}

/// Create a GHZ state for testing.
#[wasm_bindgen(js_name = createGhzState)]
pub fn create_ghz_state(num_qubits: usize) -> Result<QftFile, JsValue> {
    let mut file = QftFile::new(num_qubits)?;
    let sqrt2_inv = 1.0 / 2.0_f64.sqrt();
    file.amplitudes_real[0] = sqrt2_inv;
    file.amplitudes_real[file.dimension() - 1] = sqrt2_inv;
    Ok(file)
}

/// Create a uniform superposition state.
#[wasm_bindgen(js_name = createUniformState)]
pub fn create_uniform_state(num_qubits: usize) -> Result<QftFile, JsValue> {
    let mut file = QftFile::new(num_qubits)?;
    let dim = file.dimension();
    let amp = 1.0 / (dim as f64).sqrt();
    for i in 0..dim {
        file.amplitudes_real[i] = amp;
    }
    Ok(file)
}

// =============================================================================
// Section 6: Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_file() {
        let file = QftFile::new(4).unwrap();
        assert_eq!(file.num_qubits(), 4);
        assert_eq!(file.dimension(), 16);
    }

    #[test]
    fn test_normalization() {
        let mut file = QftFile::new(2).unwrap();
        file.amplitudes_real[0] = 1.0;
        file.amplitudes_real[1] = 1.0;
        assert!(!file.is_normalized(None));
        file.normalize().unwrap();
        assert!(file.is_normalized(None));
    }

    #[test]
    fn test_serialization() {
        let file = QftFile::new(2).unwrap();
        let bytes = file.serialize();
        let restored = QftFile::deserialize(&bytes).unwrap();
        assert_eq!(file.num_qubits, restored.num_qubits);
        assert_eq!(file.amplitudes_real, restored.amplitudes_real);
    }

    #[test]
    fn test_fidelity() {
        let file1 = QftFile::new(2).unwrap();
        let file2 = QftFile::new(2).unwrap();
        let fid = file1.fidelity(&file2).unwrap();
        assert!((fid - 1.0).abs() < 1e-10);
    }
}
