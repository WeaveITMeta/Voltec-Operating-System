/*
 * libqft - Quantum File Type C API
 * Version 1.0.0
 * 
 * Production-grade C bindings for the Quantum File Type (.qft) format.
 * Supports quantum state storage using Matrix Product State (MPS) tensor networks
 * with Golay error correction and CRC-64 checksums.
 *
 * Thread Safety: All functions are thread-safe. Each handle can only be used
 * from one thread at a time.
 *
 * Memory Management:
 *   - Strings returned by the library must be freed with qft_free_string()
 *   - Arrays must be freed with qft_free_array()
 *   - Handles must be closed with qft_close() or qft_stream_close()
 *
 * Error Handling:
 *   - Most functions return QftError or set qft_get_last_error()
 *   - Use qft_get_last_error_message() for detailed error information
 *
 * Copyright (c) 2026 MACROHARD Quantum OS Team
 * Licensed under MIT OR Apache-2.0
 */

#ifndef LIBQFT_H
#define LIBQFT_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ==========================================================================
 * Error Codes
 * ========================================================================== */

typedef enum QftError {
    QFT_SUCCESS = 0,
    QFT_NULL_POINTER = 1,
    QFT_INVALID_HANDLE = 2,
    QFT_FILE_NOT_FOUND = 3,
    QFT_PERMISSION_DENIED = 4,
    QFT_INVALID_FORMAT = 5,
    QFT_CHECKSUM_MISMATCH = 6,
    QFT_GOLAY_DECODE_FAILED = 7,
    QFT_BUFFER_TOO_SMALL = 8,
    QFT_INVALID_PARAMETER = 9,
    QFT_OUT_OF_MEMORY = 10,
    QFT_IO_ERROR = 11,
    QFT_ENCODING_ERROR = 12,
    QFT_DIMENSION_MISMATCH = 13,
    QFT_NOT_SUPPORTED = 14,
    QFT_INTERNAL_ERROR = 99,
} QftError;

/* ==========================================================================
 * Encoding Strategies
 * ========================================================================== */

typedef enum QftEncoding {
    QFT_ENCODING_AMPLITUDE = 0,
    QFT_ENCODING_BASIS_STATE = 1,
    QFT_ENCODING_QRAM = 2,
    QFT_ENCODING_ANGLE = 3,
    QFT_ENCODING_BLOCK = 4,
} QftEncoding;

/* ==========================================================================
 * Opaque Types
 * ========================================================================== */

typedef struct QftFile QftFile;
typedef struct QftState QftState;
typedef struct QftStream QftStream;

/* ==========================================================================
 * Version Information
 * ========================================================================== */

typedef struct QftVersion {
    unsigned int major;
    unsigned int minor;
    unsigned int patch;
} QftVersion;

/**
 * Get the library version as a struct.
 */
QftVersion qft_version(void);

/**
 * Get the library version as a string.
 * Caller must free with qft_free_string().
 */
char *qft_version_string(void);

/* ==========================================================================
 * Error Handling
 * ========================================================================== */

/**
 * Get the last error code from the current thread.
 */
QftError qft_get_last_error(void);

/**
 * Get the last error message from the current thread.
 * Caller must free with qft_free_string().
 */
char *qft_get_last_error_message(void);

/**
 * Get a human-readable description of an error code.
 * Returns a static string, do not free.
 */
const char *qft_error_string(QftError error);

/* ==========================================================================
 * Core Operations
 * ========================================================================== */

/**
 * Create a new QFT file with the specified number of qubits.
 * Initializes to the |0...0⟩ state.
 * 
 * @param num_qubits Number of qubits (1-30)
 * @return Handle to the file, or NULL on error
 */
QftFile *qft_new(unsigned int num_qubits);

/**
 * Open an existing QFT file from disk.
 * 
 * @param path Path to the .qft file
 * @return Handle to the file, or NULL on error
 */
QftFile *qft_open(const char *path);

/**
 * Save a QFT file to disk.
 * 
 * @param handle File handle
 * @param path Output path
 * @return QFT_SUCCESS on success
 */
QftError qft_save(QftFile *handle, const char *path);

/**
 * Close a QFT file and free resources.
 * 
 * @param handle File handle
 * @return QFT_SUCCESS on success
 */
QftError qft_close(QftFile *handle);

/* ==========================================================================
 * State Access
 * ========================================================================== */

/**
 * Get the number of qubits.
 * 
 * @param handle File handle
 * @return Number of qubits, or -1 on error
 */
int qft_num_qubits(const QftFile *handle);

/**
 * Get the state vector dimension (2^num_qubits).
 * 
 * @param handle File handle
 * @return Dimension, or 0 on error
 */
size_t qft_dimension(const QftFile *handle);

/**
 * Get the real parts of all amplitudes.
 * 
 * @param handle File handle
 * @param out Output buffer (must be at least qft_dimension() doubles)
 * @param len Length of output buffer
 * @return QFT_SUCCESS on success
 */
QftError qft_get_amplitudes_real(const QftFile *handle, double *out, size_t len);

/**
 * Get the imaginary parts of all amplitudes.
 * 
 * @param handle File handle
 * @param out Output buffer (must be at least qft_dimension() doubles)
 * @param len Length of output buffer
 * @return QFT_SUCCESS on success
 */
QftError qft_get_amplitudes_imag(const QftFile *handle, double *out, size_t len);

/**
 * Set all amplitudes.
 * 
 * @param handle File handle
 * @param real Real parts (must be exactly qft_dimension() doubles)
 * @param imag Imaginary parts (must be exactly qft_dimension() doubles)
 * @param len Length of arrays
 * @return QFT_SUCCESS on success
 */
QftError qft_set_amplitudes(QftFile *handle,
                            const double *real,
                            const double *imag,
                            size_t len);

/**
 * Get a single amplitude by index.
 * 
 * @param handle File handle
 * @param index Basis state index (0 to 2^n - 1)
 * @param imag_out Output for imaginary part (can be NULL)
 * @return Real part, or NaN on error
 */
double qft_get_amplitude(const QftFile *handle, size_t index, double *imag_out);

/**
 * Set a single amplitude by index.
 * 
 * @param handle File handle
 * @param index Basis state index (0 to 2^n - 1)
 * @param real Real part
 * @param imag Imaginary part
 * @return QFT_SUCCESS on success
 */
QftError qft_set_amplitude(QftFile *handle, size_t index, double real, double imag);

/* ==========================================================================
 * Metadata
 * ========================================================================== */

/**
 * Set a metadata key-value pair.
 */
QftError qft_set_metadata(QftFile *handle, const char *key, const char *value);

/**
 * Get a metadata value by key.
 * Caller must free with qft_free_string().
 * Returns NULL if key not found.
 */
char *qft_get_metadata(const QftFile *handle, const char *key);

/**
 * Get all metadata as a JSON string.
 * Caller must free with qft_free_string().
 */
char *qft_get_metadata_json(const QftFile *handle);

/* ==========================================================================
 * Configuration
 * ========================================================================== */

/**
 * Set the bond dimension for MPS compression.
 * 
 * @param handle File handle
 * @param bond_dim Bond dimension (1-1024)
 * @return QFT_SUCCESS on success
 */
QftError qft_set_bond_dimension(QftFile *handle, unsigned int bond_dim);

/**
 * Get the current bond dimension.
 */
unsigned int qft_get_bond_dimension(const QftFile *handle);

/**
 * Enable or disable Golay error correction.
 */
QftError qft_set_golay_enabled(QftFile *handle, int enabled);

/**
 * Check if Golay error correction is enabled.
 * Returns 1 if enabled, 0 if disabled, -1 on error.
 */
int qft_get_golay_enabled(const QftFile *handle);

/* ==========================================================================
 * Validation
 * ========================================================================== */

/**
 * Check if the state is normalized.
 * 
 * @param handle File handle
 * @param tolerance Tolerance for check
 * @return 1 if normalized, 0 if not, -1 on error
 */
int qft_is_normalized(const QftFile *handle, double tolerance);

/**
 * Normalize the state vector in place.
 */
QftError qft_normalize(QftFile *handle);

/**
 * Calculate fidelity between two states: |⟨ψ|φ⟩|².
 * 
 * @param handle1 First state
 * @param handle2 Second state
 * @return Fidelity (0 to 1), or NaN on error
 */
double qft_fidelity(const QftFile *handle1, const QftFile *handle2);

/* ==========================================================================
 * Conversion
 * ========================================================================== */

/**
 * Create a QFT file from raw amplitude arrays.
 */
QftFile *qft_from_amplitudes(const double *real, const double *imag, size_t len);

/**
 * Export to NumPy-compatible interleaved format.
 * 
 * @param handle File handle
 * @param out Output buffer (must be 2 * qft_dimension() doubles)
 * @param len Length of output buffer
 * @return QFT_SUCCESS on success
 */
QftError qft_to_numpy_interleaved(const QftFile *handle, double *out, size_t len);

/**
 * Create from NumPy-compatible interleaved format.
 */
QftFile *qft_from_numpy_interleaved(const double *data, size_t len);

/* ==========================================================================
 * Streaming
 * ========================================================================== */

/**
 * Create a streaming converter for large files.
 * 
 * @param total_size Total size of input data in bytes
 * @param chunk_size Size of each chunk
 * @param encoding Encoding strategy
 * @return Stream handle, or NULL on error
 */
QftStream *qft_stream_new(size_t total_size, size_t chunk_size, QftEncoding encoding);

/**
 * Process a chunk of data.
 * 
 * @param stream Stream handle
 * @param data Input data
 * @param len Length of input data
 * @param out_real Output buffer for real parts
 * @param out_imag Output buffer for imaginary parts
 * @param out_len Length of output buffers
 * @return Number of amplitudes written, or -1 on error
 */
int qft_stream_process(QftStream *stream,
                       const uint8_t *data,
                       size_t len,
                       double *out_real,
                       double *out_imag,
                       size_t out_len);

/**
 * Get streaming progress (0.0 to 1.0).
 */
double qft_stream_progress(const QftStream *stream);

/**
 * Close a streaming converter.
 */
QftError qft_stream_close(QftStream *stream);

/* ==========================================================================
 * Memory Management
 * ========================================================================== */

/**
 * Free a string returned by the library.
 */
void qft_free_string(char *s);

/**
 * Free an array returned by the library.
 */
void qft_free_array(void *arr, size_t len, size_t elem_size);

#ifdef __cplusplus
}
#endif

#endif /* LIBQFT_H */
