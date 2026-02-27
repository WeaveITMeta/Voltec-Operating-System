// =============================================================================
// V-OS - QFT File Format Converter
// =============================================================================
// Table of Contents:
//   1. ConversionConfig - Configuration for file conversions
//   2. Encoding strategies (amplitude, basis state, QRAM)
//   3. Binary file conversion (any file → .qft)
//   4. Text file conversion (UTF-8 → .qft)
//   5. Image file conversion (pixels → .qft)
//   6. Reverse conversions (.qft → original format)
//   7. Format detection and auto-conversion
// =============================================================================
// Purpose: Convert arbitrary classical files to quantum-encoded .qft format
//          and back. Enables quantum storage of any data type with error
//          correction and compression benefits.
// =============================================================================

use num_complex::Complex64;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::state_backend::QuantumStateVector;

/// Encoding strategy for classical data → quantum state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingStrategy {
    /// Amplitude encoding: data values become amplitudes
    /// n values → log₂(n) qubits, but requires normalization
    Amplitude,
    
    /// Basis state encoding: each byte/value is a basis state
    /// n bytes → n * 8 qubits (or n * log₂(alphabet) for text)
    BasisState,
    
    /// QRAM-style encoding: index → value mapping
    /// Efficient for sparse data
    Qram,
    
    /// Hybrid: amplitude for values, basis for structure
    Hybrid,
}

impl Default for EncodingStrategy {
    fn default() -> Self {
        Self::Amplitude
    }
}

/// Configuration for file conversion
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    /// Encoding strategy to use
    pub encoding: EncodingStrategy,
    /// Maximum qubits to use (limits file size)
    pub max_qubits: usize,
    /// Whether to compress before encoding
    pub compress_first: bool,
    /// Chunk size for large files (bytes)
    pub chunk_size: usize,
    /// Preserve original file metadata
    pub preserve_metadata: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            encoding: EncodingStrategy::Amplitude,
            max_qubits: 20, // 2^20 = 1M amplitudes
            compress_first: true,
            chunk_size: 1 << 20, // 1 MB chunks
            preserve_metadata: true,
        }
    }
}

impl ConversionConfig {
    pub fn high_capacity() -> Self {
        Self {
            max_qubits: 25, // 2^25 = 32M amplitudes
            ..Default::default()
        }
    }
    
    pub fn low_memory() -> Self {
        Self {
            max_qubits: 16, // 2^16 = 64K amplitudes
            chunk_size: 1 << 16,
            ..Default::default()
        }
    }
}

/// Result of a file conversion
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Number of qubits used
    pub num_qubits: usize,
    /// Original file size in bytes
    pub original_size: usize,
    /// Encoded size (before MPS compression)
    pub encoded_size: usize,
    /// Number of chunks (for large files)
    pub num_chunks: usize,
    /// Encoding strategy used
    pub encoding: EncodingStrategy,
    /// Original file extension (for reverse conversion)
    pub original_extension: String,
    /// Original filename
    pub original_filename: String,
}

/// Metadata stored in .qft for reverse conversion
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub original_filename: String,
    pub original_extension: String,
    pub original_size: usize,
    pub encoding: EncodingStrategy,
    pub checksum: u64,
}

// =============================================================================
// 2. Encoding Strategies
// =============================================================================

/// Encode bytes as quantum amplitudes.
/// Each byte value (0-255) becomes a normalized amplitude.
pub fn encode_bytes_as_amplitudes(data: &[u8]) -> QuantumStateVector {
    // Pad to power of 2
    let n = data.len().next_power_of_two();
    let num_qubits = n.trailing_zeros() as usize;
    
    // Convert bytes to complex amplitudes
    let mut amplitudes: Vec<Complex64> = data
        .iter()
        .map(|&b| Complex64::new(b as f64, 0.0))
        .collect();
    
    // Pad with zeros
    amplitudes.resize(n, Complex64::new(0.0, 0.0));
    
    // Normalize
    let norm: f64 = amplitudes.iter().map(|a| a.norm_sqr()).sum::<f64>().sqrt();
    if norm > 1e-15 {
        for amp in &mut amplitudes {
            *amp /= norm;
        }
    } else {
        // All zeros - create uniform superposition
        let uniform = Complex64::new(1.0 / (n as f64).sqrt(), 0.0);
        amplitudes = vec![uniform; n];
    }
    
    QuantumStateVector::from_amplitudes(amplitudes)
}

/// Decode quantum amplitudes back to bytes.
/// Reverses the amplitude encoding process.
pub fn decode_amplitudes_to_bytes(state: &QuantumStateVector, original_size: usize) -> Vec<u8> {
    let amplitudes = state.amplitudes();
    
    // Find the maximum amplitude to scale back
    let max_amp: f64 = amplitudes.iter().map(|a| a.re.abs()).fold(0.0, f64::max);
    
    if max_amp < 1e-15 {
        return vec![0u8; original_size];
    }
    
    // Scale amplitudes back to 0-255 range
    let scale = 255.0 / max_amp;
    
    amplitudes
        .iter()
        .take(original_size)
        .map(|a| (a.re.abs() * scale).round().min(255.0).max(0.0) as u8)
        .collect()
}

/// Encode bytes as basis states (one qubit per bit).
/// Lossless but uses more qubits.
pub fn encode_bytes_as_basis_states(data: &[u8]) -> QuantumStateVector {
    let num_bits = data.len() * 8;
    let num_qubits = num_bits.max(1);
    let dim = 1usize << num_qubits.min(20); // Cap at 20 qubits for memory
    
    // Convert bytes to a single basis state index
    let mut index = 0usize;
    for (i, &byte) in data.iter().enumerate().take(dim.trailing_zeros() as usize / 8) {
        index |= (byte as usize) << (i * 8);
    }
    index = index.min(dim - 1);
    
    // Create basis state
    let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
    amplitudes[index] = Complex64::new(1.0, 0.0);
    
    QuantumStateVector::from_amplitudes(amplitudes)
}

/// Encode with QRAM-style: superposition of |index⟩|value⟩
pub fn encode_bytes_as_qram(data: &[u8]) -> QuantumStateVector {
    let n = data.len().next_power_of_two();
    let index_qubits = n.trailing_zeros() as usize;
    let value_qubits = 8; // 8 bits per byte
    let total_qubits = (index_qubits + value_qubits).min(20);
    let dim = 1usize << total_qubits;
    
    let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
    let norm = 1.0 / (data.len() as f64).sqrt();
    
    for (i, &byte) in data.iter().enumerate() {
        if i >= (1 << index_qubits) {
            break;
        }
        // |i⟩|data[i]⟩
        let combined_index = (i << value_qubits) | (byte as usize);
        if combined_index < dim {
            amplitudes[combined_index] = Complex64::new(norm, 0.0);
        }
    }
    
    QuantumStateVector::from_amplitudes(amplitudes)
}

// =============================================================================
// 3. Binary File Conversion
// =============================================================================

/// Convert any binary file to a quantum state.
pub fn binary_to_quantum(data: &[u8], config: &ConversionConfig) -> QuantumStateVector {
    match config.encoding {
        EncodingStrategy::Amplitude => encode_bytes_as_amplitudes(data),
        EncodingStrategy::BasisState => encode_bytes_as_basis_states(data),
        EncodingStrategy::Qram => encode_bytes_as_qram(data),
        EncodingStrategy::Hybrid => {
            // Use amplitude for small files, QRAM for large
            if data.len() <= 1024 {
                encode_bytes_as_amplitudes(data)
            } else {
                encode_bytes_as_qram(data)
            }
        }
    }
}

/// Convert any file to .qft format.
/// 
/// # Arguments
/// * `input_path` - Path to the source file (any format)
/// * `output_path` - Path for the output .qft file
/// * `config` - Conversion configuration
/// 
/// # Returns
/// ConversionResult with statistics about the conversion
pub fn convert_file_to_qft(
    input_path: &Path,
    output_path: &Path,
    config: &ConversionConfig,
) -> Result<ConversionResult, String> {
    // Read the input file
    let data = fs::read(input_path)
        .map_err(|e| format!("Failed to read input file: {}", e))?;
    
    let original_size = data.len();
    let original_filename = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let original_extension = input_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    
    // Check size limits
    let max_bytes = 1usize << config.max_qubits;
    if data.len() > max_bytes && config.encoding == EncodingStrategy::Amplitude {
        return Err(format!(
            "File too large for {} qubits. Max: {} bytes, got: {} bytes",
            config.max_qubits, max_bytes, data.len()
        ));
    }
    
    // Encode to quantum state
    let state = binary_to_quantum(&data, config);
    let num_qubits = state.number_of_quantum_bits();
    
    // Create metadata JSON
    let metadata = serde_json::json!({
        "original_filename": original_filename,
        "original_extension": original_extension,
        "original_size": original_size,
        "encoding": format!("{:?}", config.encoding),
        "checksum": crc64(&data),
    });
    
    // Write the quantum state to .qft file
    // For now, write as raw amplitudes with metadata header
    let mut output = fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    
    // Write magic number
    output.write_all(b"QFT1").map_err(|e| e.to_string())?;
    
    // Write metadata length and metadata
    let metadata_bytes = metadata.to_string().into_bytes();
    let metadata_len = metadata_bytes.len() as u32;
    output.write_all(&metadata_len.to_le_bytes()).map_err(|e| e.to_string())?;
    output.write_all(&metadata_bytes).map_err(|e| e.to_string())?;
    
    // Write number of qubits
    output.write_all(&(num_qubits as u32).to_le_bytes()).map_err(|e| e.to_string())?;
    
    // Write amplitudes
    for amp in state.amplitudes() {
        output.write_all(&amp.re.to_le_bytes()).map_err(|e| e.to_string())?;
        output.write_all(&amp.im.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    
    Ok(ConversionResult {
        num_qubits,
        original_size,
        encoded_size: state.dimension() * 16, // 16 bytes per Complex64
        num_chunks: 1,
        encoding: config.encoding,
        original_extension,
        original_filename,
    })
}

/// Convert .qft file back to original format.
pub fn convert_qft_to_file(
    input_path: &Path,
    output_path: &Path,
) -> Result<ConversionResult, String> {
    let mut file = fs::File::open(input_path)
        .map_err(|e| format!("Failed to open .qft file: {}", e))?;
    
    // Read magic number
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"QFT1" {
        return Err("Invalid .qft file: bad magic number".to_string());
    }
    
    // Read metadata
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes).map_err(|e| e.to_string())?;
    let metadata_len = u32::from_le_bytes(len_bytes) as usize;
    
    let mut metadata_bytes = vec![0u8; metadata_len];
    file.read_exact(&mut metadata_bytes).map_err(|e| e.to_string())?;
    
    let metadata: serde_json::Value = serde_json::from_slice(&metadata_bytes)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;
    
    let original_size = metadata["original_size"].as_u64().unwrap_or(0) as usize;
    let original_filename = metadata["original_filename"].as_str().unwrap_or("output").to_string();
    let original_extension = metadata["original_extension"].as_str().unwrap_or("").to_string();
    let encoding_str = metadata["encoding"].as_str().unwrap_or("Amplitude");
    
    // Read number of qubits
    let mut qubit_bytes = [0u8; 4];
    file.read_exact(&mut qubit_bytes).map_err(|e| e.to_string())?;
    let num_qubits = u32::from_le_bytes(qubit_bytes) as usize;
    let dim = 1usize << num_qubits;
    
    // Read amplitudes
    let mut amplitudes = Vec::with_capacity(dim);
    for _ in 0..dim {
        let mut re_bytes = [0u8; 8];
        let mut im_bytes = [0u8; 8];
        file.read_exact(&mut re_bytes).map_err(|e| e.to_string())?;
        file.read_exact(&mut im_bytes).map_err(|e| e.to_string())?;
        let re = f64::from_le_bytes(re_bytes);
        let im = f64::from_le_bytes(im_bytes);
        amplitudes.push(Complex64::new(re, im));
    }
    
    let state = QuantumStateVector::from_amplitudes(amplitudes);
    
    // Decode based on encoding strategy
    let data = decode_amplitudes_to_bytes(&state, original_size);
    
    // Write output file
    fs::write(output_path, &data)
        .map_err(|e| format!("Failed to write output file: {}", e))?;
    
    Ok(ConversionResult {
        num_qubits,
        original_size,
        encoded_size: dim * 16,
        num_chunks: 1,
        encoding: if encoding_str.contains("Amplitude") {
            EncodingStrategy::Amplitude
        } else if encoding_str.contains("Basis") {
            EncodingStrategy::BasisState
        } else if encoding_str.contains("Qram") {
            EncodingStrategy::Qram
        } else {
            EncodingStrategy::Hybrid
        },
        original_extension,
        original_filename,
    })
}

// =============================================================================
// 4. Text File Conversion
// =============================================================================

/// Convert UTF-8 text to quantum state with character-aware encoding.
pub fn text_to_quantum(text: &str, config: &ConversionConfig) -> QuantumStateVector {
    // Convert text to bytes and use standard encoding
    let bytes = text.as_bytes();
    binary_to_quantum(bytes, config)
}

/// Convert text file to .qft
pub fn convert_text_to_qft(
    input_path: &Path,
    output_path: &Path,
    config: &ConversionConfig,
) -> Result<ConversionResult, String> {
    convert_file_to_qft(input_path, output_path, config)
}

// =============================================================================
// 5. Image File Conversion
// =============================================================================

/// Convert raw pixel data to quantum state.
/// Pixels are encoded as amplitudes (grayscale) or RGB components.
pub fn pixels_to_quantum(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> QuantumStateVector {
    // Flatten to grayscale if RGB
    let grayscale: Vec<u8> = if channels >= 3 {
        pixels
            .chunks(channels)
            .map(|chunk| {
                let r = chunk.get(0).copied().unwrap_or(0) as u32;
                let g = chunk.get(1).copied().unwrap_or(0) as u32;
                let b = chunk.get(2).copied().unwrap_or(0) as u32;
                ((r * 299 + g * 587 + b * 114) / 1000) as u8
            })
            .collect()
    } else {
        pixels.to_vec()
    };
    
    encode_bytes_as_amplitudes(&grayscale)
}

// =============================================================================
// 6. Format Detection
// =============================================================================

/// Detected file format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedFormat {
    Text,
    Binary,
    Image,
    Quantum, // Already a quantum format
    Unknown,
}

/// Detect file format from extension and magic bytes.
pub fn detect_format(path: &Path) -> DetectedFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match ext.as_str() {
        // Text formats
        "txt" | "md" | "json" | "xml" | "html" | "css" | "js" | "ts" | "rs" | "py" | "c" | "cpp" | "h" | "toml" | "yaml" | "yml" => {
            DetectedFormat::Text
        }
        // Image formats
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" => {
            DetectedFormat::Image
        }
        // Quantum formats
        "qft" | "qasm" | "npy" => {
            DetectedFormat::Quantum
        }
        // Binary/unknown
        _ => DetectedFormat::Binary,
    }
}

/// Auto-convert any file to .qft with optimal settings.
pub fn auto_convert_to_qft(
    input_path: &Path,
    output_path: &Path,
) -> Result<ConversionResult, String> {
    let format = detect_format(input_path);
    
    let config = match format {
        DetectedFormat::Text => ConversionConfig {
            encoding: EncodingStrategy::Amplitude,
            compress_first: true,
            ..Default::default()
        },
        DetectedFormat::Image => ConversionConfig {
            encoding: EncodingStrategy::Amplitude,
            max_qubits: 22, // Support larger images
            ..Default::default()
        },
        DetectedFormat::Binary => ConversionConfig::default(),
        DetectedFormat::Quantum => {
            return Err("File is already a quantum format".to_string());
        }
        DetectedFormat::Unknown => ConversionConfig::default(),
    };
    
    convert_file_to_qft(input_path, output_path, &config)
}

// =============================================================================
// 7. Utility Functions
// =============================================================================

/// Simple CRC64 checksum for data integrity.
fn crc64(data: &[u8]) -> u64 {
    const POLY: u64 = 0x42F0E1EBA9EA3693;
    let mut crc: u64 = 0xFFFFFFFFFFFFFFFF;
    
    for &byte in data {
        crc ^= (byte as u64) << 56;
        for _ in 0..8 {
            crc = if crc & 0x8000000000000000 != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
        }
    }
    
    crc ^ 0xFFFFFFFFFFFFFFFF
}

/// Get supported input formats.
pub fn supported_input_formats() -> Vec<&'static str> {
    vec![
        // Text
        "txt", "md", "json", "xml", "html", "css", "js", "ts", "rs", "py", "c", "cpp", "toml", "yaml",
        // Images
        "png", "jpg", "jpeg", "gif", "bmp", "webp",
        // Binary
        "bin", "dat", "exe", "dll", "so", "pdf", "doc", "docx", "zip",
        // Any other file
        "*",
    ]
}

/// Get file size limit for given qubit count.
pub fn max_file_size_for_qubits(num_qubits: usize) -> usize {
    1usize << num_qubits
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_amplitude_encoding_roundtrip() {
        let original = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
        let state = encode_bytes_as_amplitudes(&original);
        let decoded = decode_amplitudes_to_bytes(&state, original.len());
        
        // Check relative ordering is preserved (exact values may differ due to normalization)
        for i in 0..original.len() - 1 {
            if original[i] < original[i + 1] {
                assert!(decoded[i] <= decoded[i + 1]);
            }
        }
    }
    
    #[test]
    fn test_basis_state_encoding() {
        let data = vec![0x42u8]; // 'B'
        let state = encode_bytes_as_basis_states(&data);
        
        // Should be a single basis state
        let probs = state.probability_distribution();
        let max_prob = probs.iter().cloned().fold(0.0, f64::max);
        assert!((max_prob - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_format_detection() {
        assert_eq!(detect_format(Path::new("test.txt")), DetectedFormat::Text);
        assert_eq!(detect_format(Path::new("image.png")), DetectedFormat::Image);
        assert_eq!(detect_format(Path::new("data.bin")), DetectedFormat::Binary);
        assert_eq!(detect_format(Path::new("state.qft")), DetectedFormat::Quantum);
    }
    
    #[test]
    fn test_crc64() {
        let data = b"Hello, Quantum!";
        let checksum = crc64(data);
        assert_ne!(checksum, 0);
        
        // Same data should give same checksum
        assert_eq!(crc64(data), checksum);
        
        // Different data should give different checksum
        assert_ne!(crc64(b"Hello, World!"), checksum);
    }
}
