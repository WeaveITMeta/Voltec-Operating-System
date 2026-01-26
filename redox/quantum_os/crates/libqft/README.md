# libqft - Quantum File Type C API

Production-grade C bindings for the Quantum File Type (.qft) format.

## Features

- **Thread-safe**: All functions are safe to call from multiple threads
- **Zero-copy where possible**: Efficient memory management
- **Comprehensive error handling**: Detailed error codes and messages
- **Streaming support**: Process files larger than memory
- **Cross-platform**: Windows, macOS, Linux, and Redox OS

## Installation

### From Source

```bash
cargo build --release
```

This produces:
- `target/release/libqft.so` (Linux)
- `target/release/libqft.dylib` (macOS)
- `target/release/qft.dll` (Windows)
- `target/release/libqft.a` (static library)

### Header File

After building, the C header is generated at `include/qft.h`.

## Quick Start

```c
#include "qft.h"
#include <stdio.h>

int main() {
    // Create a 4-qubit state
    QftFile* file = qft_new(4);
    if (!file) {
        printf("Error: %s\n", qft_error_string(qft_get_last_error()));
        return 1;
    }

    printf("Qubits: %d\n", qft_num_qubits(file));
    printf("Dimension: %zu\n", qft_dimension(file));

    // Set Bell state |00⟩ + |11⟩ (unnormalized)
    double real[16] = {0};
    double imag[16] = {0};
    real[0] = 1.0;   // |0000⟩
    real[15] = 1.0;  // |1111⟩
    
    qft_set_amplitudes(file, real, imag, 16);
    qft_normalize(file);

    // Save to file
    if (qft_save(file, "state.qft") != QFT_SUCCESS) {
        printf("Save error: %s\n", qft_error_string(qft_get_last_error()));
    }

    qft_close(file);
    return 0;
}
```

## API Reference

### Core Operations

| Function | Description |
|----------|-------------|
| `qft_new(n)` | Create n-qubit state initialized to \|0...0⟩ |
| `qft_open(path)` | Open existing .qft file |
| `qft_save(file, path)` | Save to disk |
| `qft_close(file)` | Free resources |

### State Access

| Function | Description |
|----------|-------------|
| `qft_num_qubits(file)` | Get qubit count |
| `qft_dimension(file)` | Get state vector dimension (2^n) |
| `qft_get_amplitudes_real(file, out, len)` | Get real parts |
| `qft_get_amplitudes_imag(file, out, len)` | Get imaginary parts |
| `qft_set_amplitudes(file, real, imag, len)` | Set all amplitudes |
| `qft_get_amplitude(file, idx, &imag)` | Get single amplitude |
| `qft_set_amplitude(file, idx, real, imag)` | Set single amplitude |

### Validation

| Function | Description |
|----------|-------------|
| `qft_is_normalized(file, tol)` | Check if ∑\|α\|² ≈ 1 |
| `qft_normalize(file)` | Normalize in place |
| `qft_fidelity(file1, file2)` | Calculate \|⟨ψ\|φ⟩\|² |

### Configuration

| Function | Description |
|----------|-------------|
| `qft_set_bond_dimension(file, χ)` | Set MPS bond dimension |
| `qft_set_golay_enabled(file, bool)` | Enable/disable error correction |

### Metadata

| Function | Description |
|----------|-------------|
| `qft_set_metadata(file, key, value)` | Set key-value pair |
| `qft_get_metadata(file, key)` | Get value (caller frees) |
| `qft_get_metadata_json(file)` | Get all as JSON (caller frees) |

### Conversion

| Function | Description |
|----------|-------------|
| `qft_from_amplitudes(real, imag, len)` | Create from arrays |
| `qft_to_numpy_interleaved(file, out, len)` | Export for NumPy |
| `qft_from_numpy_interleaved(data, len)` | Import from NumPy |

### Streaming

| Function | Description |
|----------|-------------|
| `qft_stream_new(total, chunk, encoding)` | Create stream converter |
| `qft_stream_process(stream, data, len, ...)` | Process chunk |
| `qft_stream_progress(stream)` | Get progress 0.0-1.0 |
| `qft_stream_close(stream)` | Close stream |

### Memory Management

| Function | Description |
|----------|-------------|
| `qft_free_string(s)` | Free string from library |
| `qft_free_array(arr, len, elem_size)` | Free array from library |

### Error Handling

| Function | Description |
|----------|-------------|
| `qft_get_last_error()` | Get error code |
| `qft_get_last_error_message()` | Get detailed message (caller frees) |
| `qft_error_string(code)` | Get error description |

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | `QFT_SUCCESS` | Operation succeeded |
| 1 | `QFT_NULL_POINTER` | Null pointer passed |
| 2 | `QFT_INVALID_HANDLE` | Invalid or closed handle |
| 3 | `QFT_FILE_NOT_FOUND` | File does not exist |
| 4 | `QFT_PERMISSION_DENIED` | Cannot access file |
| 5 | `QFT_INVALID_FORMAT` | Not a valid .qft file |
| 6 | `QFT_CHECKSUM_MISMATCH` | CRC-64 verification failed |
| 7 | `QFT_GOLAY_DECODE_FAILED` | Too many bit errors |
| 8 | `QFT_BUFFER_TOO_SMALL` | Output buffer insufficient |
| 9 | `QFT_INVALID_PARAMETER` | Bad parameter value |
| 10 | `QFT_OUT_OF_MEMORY` | Allocation failed |
| 11 | `QFT_IO_ERROR` | Read/write error |
| 12 | `QFT_ENCODING_ERROR` | Encoding/decoding failed |
| 13 | `QFT_DIMENSION_MISMATCH` | State sizes don't match |
| 14 | `QFT_NOT_SUPPORTED` | Feature not available |
| 99 | `QFT_INTERNAL_ERROR` | Bug in library |

## Thread Safety

- All functions are thread-safe
- Each `QftFile` handle should only be used from one thread at a time
- Error state is thread-local

## License

MIT OR Apache-2.0
