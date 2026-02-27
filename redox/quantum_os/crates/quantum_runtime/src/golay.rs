// =============================================================================
// V-OS - Golay Error Correction
// =============================================================================
// Table of Contents:
//   1. Constants and Generator Polynomial
//   2. GolayEncoder - [23,12,7] Binary Golay Code Encoding
//   3. GolayDecoder - Syndrome-based Decoding with 3-error Correction
//   4. GolayStream - Stream-based Encoding/Decoding for Byte Arrays
//   5. GolayErrorStats - Error Correction Statistics
//   6. Quantum Golay [[23,1,7]] Stabilizer Definitions
// =============================================================================
// Purpose: Implements the [23,12,7] binary Golay code for fault-tolerant
//          quantum data storage. The Golay code is a perfect code capable
//          of correcting up to 3 bit errors per 23-bit codeword.
// =============================================================================
// Mathematical Foundation:
//   - Generator polynomial: g(x) = x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1
//   - Minimum distance: d = 7 (corrects floor((7-1)/2) = 3 errors)
//   - Connection to Leech lattice provides topological error resilience
// =============================================================================

use std::collections::HashMap;
use thiserror::Error;

// =============================================================================
// 1. Constants and Generator Polynomial
// =============================================================================

/// Magic constant: Generator polynomial for [23,12,7] Golay code
/// g(x) = x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1 = 0b100001100111 = 0xC75
/// Note: Represented in reversed bit order for efficient computation
pub const GOLAY_GENERATOR: u32 = 0xC75;

/// Parity check matrix rows (11 rows for syndrome computation)
/// Derived from the generator polynomial
pub const GOLAY_PARITY_CHECK: [u32; 11] = [
    0b10000000000_100001100111, // Row 0
    0b01000000000_110000110011, // Row 1  
    0b00100000000_111000011001, // Row 2
    0b00010000000_111100001100, // Row 3
    0b00001000000_011110000110, // Row 4
    0b00000100000_001111000011, // Row 5
    0b00000010000_100111100001, // Row 6
    0b00000001000_110011110000, // Row 7
    0b00000000100_011001111000, // Row 8
    0b00000000010_001100111100, // Row 9
    0b00000000001_000110011110, // Row 10
];

/// Parity matrix P (12x11) for systematic encoding
/// G = [I_12 | P] where I_12 is 12x12 identity
const GOLAY_P_MATRIX: [u16; 12] = [
    0b10001100111, // Row 0:  corresponds to bit 11
    0b11000110011, // Row 1:  corresponds to bit 10
    0b11100011001, // Row 2:  corresponds to bit 9
    0b11110001100, // Row 3:  corresponds to bit 8
    0b01111000110, // Row 4:  corresponds to bit 7
    0b00111100011, // Row 5:  corresponds to bit 6
    0b10011110001, // Row 6:  corresponds to bit 5
    0b11001111000, // Row 7:  corresponds to bit 4
    0b01100111100, // Row 8:  corresponds to bit 3
    0b00110011110, // Row 9:  corresponds to bit 2
    0b00011001111, // Row 10: corresponds to bit 1
    0b10001100111, // Row 11: corresponds to bit 0 (cyclic)
];

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Error, Clone, PartialEq)]
pub enum GolayError {
    #[error("Uncorrectable error: syndrome {syndrome:#06x} has no matching pattern")]
    UncorrectableError { syndrome: u16 },
    
    #[error("Invalid message: expected 12 bits, got {bits} bits set above position 11")]
    InvalidMessage { bits: u32 },
    
    #[error("Invalid codeword: expected 23 bits, got value {value:#010x}")]
    InvalidCodeword { value: u32 },
    
    #[error("Stream encoding error at byte offset {offset}: {message}")]
    StreamEncodingError { offset: usize, message: String },
    
    #[error("Stream decoding error at block {block}: {message}")]
    StreamDecodingError { block: usize, message: String },
    
    #[error("Data length {length} is not aligned to required boundary")]
    AlignmentError { length: usize },
}

// =============================================================================
// 2. GolayEncoder - [23,12,7] Binary Golay Code Encoding
// =============================================================================

/// Encoder for the [23,12,7] binary Golay code
/// 
/// Encodes 12-bit messages into 23-bit codewords with minimum distance 7.
/// Uses systematic encoding: codeword = [message (12 bits) | parity (11 bits)]
#[derive(Debug, Clone, Default)]
pub struct GolayEncoder {
    /// Running count of encoded blocks
    blocks_encoded: u64,
}

impl GolayEncoder {
    /// Create a new Golay encoder
    pub fn new() -> Self {
        Self { blocks_encoded: 0 }
    }
    
    /// Encode a 12-bit message into a 23-bit Golay codeword
    /// 
    /// # Arguments
    /// * `message` - 12-bit message (bits 0-11, upper bits must be 0)
    /// 
    /// # Returns
    /// * 23-bit codeword with message in upper 12 bits and parity in lower 11 bits
    /// 
    /// # Example
    /// ```
    /// let encoder = GolayEncoder::new();
    /// let codeword = encoder.encode(0b101010101010)?; // 12-bit message
    /// assert!(codeword < (1 << 23)); // Result is 23 bits
    /// ```
    pub fn encode(&mut self, message: u16) -> Result<u32, GolayError> {
        // Validate message is 12 bits
        if message > 0xFFF {
            return Err(GolayError::InvalidMessage { 
                bits: (message >> 12) as u32 
            });
        }
        
        // Systematic encoding: codeword = message || parity
        // Parity bits are computed as message * P^T (mod 2)
        let mut parity: u16 = 0;
        
        for i in 0..12 {
            if (message >> (11 - i)) & 1 == 1 {
                parity ^= GOLAY_P_MATRIX[i];
            }
        }
        
        // Combine: [message (12 bits) | parity (11 bits)]
        let codeword = ((message as u32) << 11) | (parity as u32);
        
        self.blocks_encoded += 1;
        Ok(codeword)
    }
    
    /// Encode using polynomial division (alternative method)
    /// 
    /// This method computes parity using the generator polynomial directly.
    pub fn encode_polynomial(&mut self, message: u16) -> Result<u32, GolayError> {
        if message > 0xFFF {
            return Err(GolayError::InvalidMessage { 
                bits: (message >> 12) as u32 
            });
        }
        
        // Shift message to make room for parity bits
        let mut remainder = (message as u32) << 11;
        
        // Polynomial division by generator
        for i in (11..23).rev() {
            if (remainder >> i) & 1 == 1 {
                remainder ^= GOLAY_GENERATOR << (i - 11);
            }
        }
        
        // Codeword = shifted message XOR remainder
        let codeword = ((message as u32) << 11) | (remainder & 0x7FF);
        
        self.blocks_encoded += 1;
        Ok(codeword)
    }
    
    /// Get the number of blocks encoded
    pub fn blocks_encoded(&self) -> u64 {
        self.blocks_encoded
    }
    
    /// Reset the encoder statistics
    pub fn reset(&mut self) {
        self.blocks_encoded = 0;
    }
}

// =============================================================================
// 3. GolayDecoder - Syndrome-based Decoding with 3-error Correction
// =============================================================================

/// Decoder for the [23,12,7] binary Golay code
/// 
/// Uses syndrome lookup table for efficient decoding.
/// Capable of correcting up to 3 bit errors per codeword.
#[derive(Debug, Clone)]
pub struct GolayDecoder {
    /// Syndrome to error pattern lookup table
    syndrome_table: HashMap<u16, u32>,
    /// Statistics tracking
    stats: GolayErrorStats,
}

impl GolayDecoder {
    /// Create a new Golay decoder with precomputed syndrome table
    pub fn new() -> Self {
        Self {
            syndrome_table: Self::build_syndrome_table(),
            stats: GolayErrorStats::default(),
        }
    }
    
    /// Build the syndrome lookup table for all correctable error patterns
    /// 
    /// This precomputes syndromes for:
    /// - 1 weight-0 pattern (no error): syndrome = 0
    /// - 23 weight-1 patterns (single bit errors)
    /// - 253 weight-2 patterns (double bit errors)
    /// - 1771 weight-3 patterns (triple bit errors)
    /// Total: 2048 patterns = 2^11 (perfect code property)
    fn build_syndrome_table() -> HashMap<u16, u32> {
        let mut table = HashMap::with_capacity(2048);
        
        // Weight-0: no error
        table.insert(0, 0);
        
        // Weight-1 errors: 23 patterns
        for i in 0..23u32 {
            let error = 1u32 << i;
            let syndrome = Self::compute_syndrome_static(error);
            table.insert(syndrome, error);
        }
        
        // Weight-2 errors: C(23,2) = 253 patterns
        for i in 0..23u32 {
            for j in (i + 1)..23u32 {
                let error = (1u32 << i) | (1u32 << j);
                let syndrome = Self::compute_syndrome_static(error);
                table.insert(syndrome, error);
            }
        }
        
        // Weight-3 errors: C(23,3) = 1771 patterns
        for i in 0..23u32 {
            for j in (i + 1)..23u32 {
                for k in (j + 1)..23u32 {
                    let error = (1u32 << i) | (1u32 << j) | (1u32 << k);
                    let syndrome = Self::compute_syndrome_static(error);
                    table.insert(syndrome, error);
                }
            }
        }
        
        table
    }
    
    /// Compute syndrome for a received codeword (static version for table building)
    fn compute_syndrome_static(codeword: u32) -> u16 {
        let mut syndrome: u16 = 0;
        
        // Syndrome = H * r^T where H is parity check matrix
        // For systematic code, syndrome depends on parity check equations
        let mut temp = codeword;
        
        // Use generator polynomial for syndrome computation
        for i in (11..23).rev() {
            if (temp >> i) & 1 == 1 {
                temp ^= GOLAY_GENERATOR << (i - 11);
            }
        }
        
        syndrome = (temp & 0x7FF) as u16;
        syndrome
    }
    
    /// Compute syndrome for a received codeword
    pub fn compute_syndrome(&self, codeword: u32) -> u16 {
        Self::compute_syndrome_static(codeword)
    }
    
    /// Decode a 23-bit Golay codeword, correcting up to 3 errors
    /// 
    /// # Arguments
    /// * `codeword` - 23-bit received codeword (possibly with errors)
    /// 
    /// # Returns
    /// * Original 12-bit message if decoding succeeds
    /// * Error if more than 3 bit errors occurred
    pub fn decode(&mut self, codeword: u32) -> Result<u16, GolayError> {
        // Validate codeword is 23 bits
        if codeword >= (1 << 23) {
            return Err(GolayError::InvalidCodeword { value: codeword });
        }
        
        self.stats.blocks_processed += 1;
        
        // Compute syndrome
        let syndrome = self.compute_syndrome(codeword);
        
        // No errors if syndrome is zero
        if syndrome == 0 {
            return Ok((codeword >> 11) as u16);
        }
        
        // Lookup error pattern
        if let Some(&error_pattern) = self.syndrome_table.get(&syndrome) {
            let corrected = codeword ^ error_pattern;
            let error_weight = error_pattern.count_ones();
            
            // Update statistics
            match error_weight {
                1 => self.stats.single_bit_errors += 1,
                2 => self.stats.double_bit_errors += 1,
                3 => self.stats.triple_bit_errors += 1,
                _ => {}
            }
            self.stats.total_errors_corrected += error_weight as u64;
            
            Ok((corrected >> 11) as u16)
        } else {
            self.stats.uncorrectable_errors += 1;
            Err(GolayError::UncorrectableError { syndrome })
        }
    }
    
    /// Decode without modifying statistics (for validation)
    pub fn decode_check(&self, codeword: u32) -> Result<u16, GolayError> {
        if codeword >= (1 << 23) {
            return Err(GolayError::InvalidCodeword { value: codeword });
        }
        
        let syndrome = self.compute_syndrome(codeword);
        
        if syndrome == 0 {
            return Ok((codeword >> 11) as u16);
        }
        
        if let Some(&error_pattern) = self.syndrome_table.get(&syndrome) {
            let corrected = codeword ^ error_pattern;
            Ok((corrected >> 11) as u16)
        } else {
            Err(GolayError::UncorrectableError { syndrome })
        }
    }
    
    /// Get current error statistics
    pub fn stats(&self) -> &GolayErrorStats {
        &self.stats
    }
    
    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = GolayErrorStats::default();
    }
}

impl Default for GolayDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 4. GolayStream - Stream-based Encoding/Decoding for Byte Arrays
// =============================================================================

/// Stream encoder/decoder for applying Golay codes to byte arrays
/// 
/// Handles the conversion between arbitrary byte streams and Golay codewords:
/// - Encoding: bytes → 12-bit blocks → 23-bit codewords → packed bytes
/// - Decoding: packed bytes → 23-bit codewords → 12-bit blocks → bytes
#[derive(Debug, Clone)]
pub struct GolayStream {
    encoder: GolayEncoder,
    decoder: GolayDecoder,
}

impl GolayStream {
    /// Create a new Golay stream processor
    pub fn new() -> Self {
        Self {
            encoder: GolayEncoder::new(),
            decoder: GolayDecoder::new(),
        }
    }
    
    /// Encode a byte array using Golay codes
    /// 
    /// # Process
    /// 1. Pad input to multiple of 3 bytes (24 bits = 2 × 12-bit messages)
    /// 2. Extract 12-bit messages from padded input
    /// 3. Encode each message to 23-bit codeword
    /// 4. Pack codewords into output byte array
    /// 
    /// # Overhead
    /// - Input: 12 bits per message
    /// - Output: 23 bits per codeword
    /// - Expansion ratio: 23/12 ≈ 1.917
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<u8>, GolayError> {
        if data.is_empty() {
            return Ok(Vec::new());
        }
        
        // Pad to multiple of 3 bytes (24 bits = 2 messages of 12 bits each)
        let padded_len = ((data.len() + 2) / 3) * 3;
        let mut padded = data.to_vec();
        padded.resize(padded_len, 0);
        
        // Extract 12-bit messages
        let mut messages = Vec::with_capacity(padded_len * 8 / 12);
        let mut bit_buffer: u32 = 0;
        let mut bits_in_buffer = 0;
        
        for &byte in &padded {
            bit_buffer = (bit_buffer << 8) | (byte as u32);
            bits_in_buffer += 8;
            
            while bits_in_buffer >= 12 {
                bits_in_buffer -= 12;
                let message = ((bit_buffer >> bits_in_buffer) & 0xFFF) as u16;
                messages.push(message);
            }
        }
        
        // Encode each message
        let mut codewords = Vec::with_capacity(messages.len());
        for (i, &message) in messages.iter().enumerate() {
            match self.encoder.encode(message) {
                Ok(codeword) => codewords.push(codeword),
                Err(e) => return Err(GolayError::StreamEncodingError {
                    offset: i * 12 / 8,
                    message: e.to_string(),
                }),
            }
        }
        
        // Pack codewords into bytes
        let output = self.pack_codewords(&codewords);
        
        Ok(output)
    }
    
    /// Decode a Golay-encoded byte array
    /// 
    /// # Process
    /// 1. Unpack 23-bit codewords from input bytes
    /// 2. Decode each codeword (with error correction)
    /// 3. Reconstruct original bytes from 12-bit messages
    pub fn decode(&mut self, encoded: &[u8]) -> Result<(Vec<u8>, GolayErrorStats), GolayError> {
        if encoded.is_empty() {
            return Ok((Vec::new(), self.decoder.stats().clone()));
        }
        
        // Unpack codewords
        let codewords = self.unpack_codewords(encoded);
        
        // Decode each codeword
        let mut messages = Vec::with_capacity(codewords.len());
        for (i, &codeword) in codewords.iter().enumerate() {
            match self.decoder.decode(codeword) {
                Ok(message) => messages.push(message),
                Err(e) => return Err(GolayError::StreamDecodingError {
                    block: i,
                    message: e.to_string(),
                }),
            }
        }
        
        // Reconstruct bytes from 12-bit messages
        let mut output = Vec::new();
        let mut bit_buffer: u32 = 0;
        let mut bits_in_buffer = 0;
        
        for message in messages {
            bit_buffer = (bit_buffer << 12) | (message as u32);
            bits_in_buffer += 12;
            
            while bits_in_buffer >= 8 {
                bits_in_buffer -= 8;
                let byte = ((bit_buffer >> bits_in_buffer) & 0xFF) as u8;
                output.push(byte);
            }
        }
        
        let stats = self.decoder.stats().clone();
        Ok((output, stats))
    }
    
    /// Pack 23-bit codewords into a byte array
    fn pack_codewords(&self, codewords: &[u32]) -> Vec<u8> {
        // Each codeword is 23 bits
        // LCM(23, 8) = 184 bits = 8 codewords = 23 bytes
        // For simplicity, we'll pack bit-by-bit
        
        let total_bits = codewords.len() * 23;
        let output_len = (total_bits + 7) / 8;
        let mut output = vec![0u8; output_len];
        
        let mut bit_pos = 0;
        for &codeword in codewords {
            for i in (0..23).rev() {
                let bit = (codeword >> i) & 1;
                let byte_idx = bit_pos / 8;
                let bit_idx = 7 - (bit_pos % 8);
                output[byte_idx] |= (bit as u8) << bit_idx;
                bit_pos += 1;
            }
        }
        
        output
    }
    
    /// Unpack 23-bit codewords from a byte array
    fn unpack_codewords(&self, data: &[u8]) -> Vec<u32> {
        let total_bits = data.len() * 8;
        let num_codewords = total_bits / 23;
        let mut codewords = Vec::with_capacity(num_codewords);
        
        let mut bit_pos = 0;
        for _ in 0..num_codewords {
            let mut codeword: u32 = 0;
            for _ in 0..23 {
                let byte_idx = bit_pos / 8;
                let bit_idx = 7 - (bit_pos % 8);
                let bit = (data[byte_idx] >> bit_idx) & 1;
                codeword = (codeword << 1) | (bit as u32);
                bit_pos += 1;
            }
            codewords.push(codeword);
        }
        
        codewords
    }
    
    /// Get encoder statistics
    pub fn encoder_stats(&self) -> u64 {
        self.encoder.blocks_encoded()
    }
    
    /// Get decoder statistics
    pub fn decoder_stats(&self) -> &GolayErrorStats {
        self.decoder.stats()
    }
    
    /// Reset all statistics
    pub fn reset_stats(&mut self) {
        self.encoder.reset();
        self.decoder.reset_stats();
    }
}

impl Default for GolayStream {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 5. GolayErrorStats - Error Correction Statistics
// =============================================================================

/// Statistics for Golay error correction operations
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GolayErrorStats {
    /// Total number of blocks processed
    pub blocks_processed: u64,
    /// Number of single-bit errors corrected
    pub single_bit_errors: u64,
    /// Number of double-bit errors corrected
    pub double_bit_errors: u64,
    /// Number of triple-bit errors corrected
    pub triple_bit_errors: u64,
    /// Total number of bit errors corrected
    pub total_errors_corrected: u64,
    /// Number of uncorrectable errors (>3 bit errors)
    pub uncorrectable_errors: u64,
}

impl GolayErrorStats {
    /// Calculate the bit error rate (corrected errors / total bits)
    pub fn bit_error_rate(&self) -> f64 {
        if self.blocks_processed == 0 {
            return 0.0;
        }
        let total_bits = self.blocks_processed * 23;
        self.total_errors_corrected as f64 / total_bits as f64
    }
    
    /// Calculate the block error rate (blocks with errors / total blocks)
    pub fn block_error_rate(&self) -> f64 {
        if self.blocks_processed == 0 {
            return 0.0;
        }
        let blocks_with_errors = self.single_bit_errors 
            + self.double_bit_errors 
            + self.triple_bit_errors 
            + self.uncorrectable_errors;
        blocks_with_errors as f64 / self.blocks_processed as f64
    }
    
    /// Calculate recovery rate (corrected / (corrected + uncorrectable))
    pub fn recovery_rate(&self) -> f64 {
        let correctable = self.single_bit_errors 
            + self.double_bit_errors 
            + self.triple_bit_errors;
        let total_errors = correctable + self.uncorrectable_errors;
        if total_errors == 0 {
            return 1.0;
        }
        correctable as f64 / total_errors as f64
    }
    
    /// Merge statistics from another instance
    pub fn merge(&mut self, other: &GolayErrorStats) {
        self.blocks_processed += other.blocks_processed;
        self.single_bit_errors += other.single_bit_errors;
        self.double_bit_errors += other.double_bit_errors;
        self.triple_bit_errors += other.triple_bit_errors;
        self.total_errors_corrected += other.total_errors_corrected;
        self.uncorrectable_errors += other.uncorrectable_errors;
    }
}

// =============================================================================
// 6. Quantum Golay [[23,1,7]] Stabilizer Definitions
// =============================================================================

/// Quantum Golay code [[23,1,7]] stabilizer generators
/// 
/// The quantum Golay code encodes 1 logical qubit into 23 physical qubits
/// with distance 7, capable of correcting up to 3 arbitrary single-qubit errors.
/// 
/// This is a CSS code derived from the classical [23,12,7] Golay code.
#[derive(Debug, Clone)]
pub struct QuantumGolayCode {
    /// X-type stabilizer generators (11 generators)
    pub x_stabilizers: Vec<u32>,
    /// Z-type stabilizer generators (11 generators)
    pub z_stabilizers: Vec<u32>,
    /// Logical X operator
    pub logical_x: u32,
    /// Logical Z operator
    pub logical_z: u32,
}

impl QuantumGolayCode {
    /// Create the quantum Golay code stabilizer structure
    pub fn new() -> Self {
        // X stabilizers derived from rows of parity check matrix H
        // Each row defines which qubits have X operators
        let x_stabilizers: Vec<u32> = vec![
            0b10000000000_100001100111,
            0b01000000000_110000110011,
            0b00100000000_111000011001,
            0b00010000000_111100001100,
            0b00001000000_011110000110,
            0b00000100000_001111000011,
            0b00000010000_100111100001,
            0b00000001000_110011110000,
            0b00000000100_011001111000,
            0b00000000010_001100111100,
            0b00000000001_000110011110,
        ];
        
        // Z stabilizers have the same structure (CSS code property)
        let z_stabilizers = x_stabilizers.clone();
        
        // Logical operators
        // Logical X: X on all qubits (weight 23)
        let logical_x = (1u32 << 23) - 1;
        // Logical Z: Z on all qubits (weight 23)
        let logical_z = (1u32 << 23) - 1;
        
        Self {
            x_stabilizers,
            z_stabilizers,
            logical_x,
            logical_z,
        }
    }
    
    /// Get the number of physical qubits
    pub fn num_physical_qubits(&self) -> usize {
        23
    }
    
    /// Get the number of logical qubits
    pub fn num_logical_qubits(&self) -> usize {
        1
    }
    
    /// Get the code distance
    pub fn distance(&self) -> usize {
        7
    }
    
    /// Get the number of correctable errors
    pub fn correctable_errors(&self) -> usize {
        3
    }
    
    /// Check if a given error pattern is correctable
    pub fn is_correctable(&self, error_weight: usize) -> bool {
        error_weight <= 3
    }
}

impl Default for QuantumGolayCode {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Encode a byte slice using Golay codes
pub fn encode_golay(data: &[u8]) -> Result<Vec<u8>, GolayError> {
    let mut stream = GolayStream::new();
    stream.encode(data)
}

/// Decode a Golay-encoded byte slice
pub fn decode_golay(encoded: &[u8]) -> Result<(Vec<u8>, GolayErrorStats), GolayError> {
    let mut stream = GolayStream::new();
    stream.decode(encoded)
}

/// Calculate the encoded size for a given input size
pub fn encoded_size(input_size: usize) -> usize {
    if input_size == 0 {
        return 0;
    }
    // Pad to multiple of 3 bytes
    let padded = ((input_size + 2) / 3) * 3;
    // Number of 12-bit messages
    let num_messages = padded * 8 / 12;
    // Each message becomes 23 bits
    let total_bits = num_messages * 23;
    // Convert to bytes
    (total_bits + 7) / 8
}

/// Calculate the decoded size for a given encoded size
pub fn decoded_size(encoded_size: usize) -> usize {
    if encoded_size == 0 {
        return 0;
    }
    // Number of 23-bit codewords
    let num_codewords = encoded_size * 8 / 23;
    // Each codeword yields 12 bits
    let total_bits = num_codewords * 12;
    // Convert to bytes
    total_bits / 8
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_single_block() {
        let mut encoder = GolayEncoder::new();
        let mut decoder = GolayDecoder::new();
        
        // Test all possible 12-bit messages
        for message in 0..=0xFFF_u16 {
            let codeword = encoder.encode(message).unwrap();
            let decoded = decoder.decode(codeword).unwrap();
            assert_eq!(message, decoded, "Failed for message {:#06x}", message);
        }
    }
    
    #[test]
    fn test_single_bit_error_correction() {
        let mut encoder = GolayEncoder::new();
        let mut decoder = GolayDecoder::new();
        
        let message: u16 = 0b101010101010;
        let codeword = encoder.encode(message).unwrap();
        
        // Introduce single bit errors at each position
        for bit in 0..23 {
            let corrupted = codeword ^ (1 << bit);
            let decoded = decoder.decode(corrupted).unwrap();
            assert_eq!(message, decoded, "Failed to correct error at bit {}", bit);
        }
    }
    
    #[test]
    fn test_double_bit_error_correction() {
        let mut encoder = GolayEncoder::new();
        let mut decoder = GolayDecoder::new();
        
        let message: u16 = 0b110011001100;
        let codeword = encoder.encode(message).unwrap();
        
        // Test a sample of double bit errors
        for i in 0..23 {
            for j in (i + 1)..23 {
                let corrupted = codeword ^ (1 << i) ^ (1 << j);
                let decoded = decoder.decode(corrupted).unwrap();
                assert_eq!(message, decoded, "Failed to correct errors at bits {} and {}", i, j);
            }
        }
    }
    
    #[test]
    fn test_triple_bit_error_correction() {
        let mut encoder = GolayEncoder::new();
        let mut decoder = GolayDecoder::new();
        
        let message: u16 = 0b111100001111;
        let codeword = encoder.encode(message).unwrap();
        
        // Test a sample of triple bit errors
        let test_cases = [
            (0, 1, 2),
            (5, 10, 15),
            (0, 11, 22),
            (7, 14, 21),
        ];
        
        for (i, j, k) in test_cases {
            let corrupted = codeword ^ (1 << i) ^ (1 << j) ^ (1 << k);
            let decoded = decoder.decode(corrupted).unwrap();
            assert_eq!(message, decoded, "Failed to correct errors at bits {}, {}, {}", i, j, k);
        }
    }
    
    #[test]
    fn test_stream_encode_decode() {
        let mut stream = GolayStream::new();
        
        let original = b"Hello, Quantum World! This is a test of Golay encoding.";
        let encoded = stream.encode(original).unwrap();
        let (decoded, stats) = stream.decode(&encoded).unwrap();
        
        // Decoded should start with original (may have padding)
        assert!(decoded.starts_with(original));
        assert_eq!(stats.uncorrectable_errors, 0);
    }
    
    #[test]
    fn test_stream_with_errors() {
        let mut stream = GolayStream::new();
        
        let original = b"Test data for error correction";
        let mut encoded = stream.encode(original).unwrap();
        
        // Introduce some bit errors (within correctable range)
        if encoded.len() > 10 {
            encoded[5] ^= 0x01; // Flip 1 bit
            encoded[10] ^= 0x02; // Flip 1 bit
        }
        
        let (decoded, stats) = stream.decode(&encoded).unwrap();
        
        // Should still recover original data
        assert!(decoded.starts_with(original));
        assert!(stats.total_errors_corrected > 0);
    }
    
    #[test]
    fn test_encoded_size_calculation() {
        assert_eq!(encoded_size(0), 0);
        assert_eq!(encoded_size(3), 6); // 3 bytes = 24 bits = 2 messages = 46 bits = 6 bytes
        assert_eq!(encoded_size(6), 12); // 6 bytes = 48 bits = 4 messages = 92 bits = 12 bytes
    }
    
    #[test]
    fn test_error_stats() {
        let mut decoder = GolayDecoder::new();
        let mut encoder = GolayEncoder::new();
        
        // Process some blocks with errors
        let message: u16 = 0xABC;
        let codeword = encoder.encode(message).unwrap();
        
        // No error
        decoder.decode(codeword).unwrap();
        
        // Single error
        decoder.decode(codeword ^ 0x01).unwrap();
        
        // Double error
        decoder.decode(codeword ^ 0x03).unwrap();
        
        // Triple error
        decoder.decode(codeword ^ 0x07).unwrap();
        
        let stats = decoder.stats();
        assert_eq!(stats.blocks_processed, 4);
        assert_eq!(stats.single_bit_errors, 1);
        assert_eq!(stats.double_bit_errors, 1);
        assert_eq!(stats.triple_bit_errors, 1);
    }
    
    #[test]
    fn test_quantum_golay_code() {
        let code = QuantumGolayCode::new();
        
        assert_eq!(code.num_physical_qubits(), 23);
        assert_eq!(code.num_logical_qubits(), 1);
        assert_eq!(code.distance(), 7);
        assert_eq!(code.correctable_errors(), 3);
        assert_eq!(code.x_stabilizers.len(), 11);
        assert_eq!(code.z_stabilizers.len(), 11);
    }
}
