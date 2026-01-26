# qft - Quantum File Type Python SDK

Production-grade Python bindings for the Quantum File Type (.qft) format.

## Installation

### From PyPI (recommended)

```bash
pip install qft
```

### From Source

```bash
# Requires Rust toolchain and maturin
pip install maturin
cd crates/qft-python
maturin develop --release
```

## Quick Start

```python
import qft
import numpy as np

# Create a 4-qubit state
f = qft.QftFile(4)
print(f"Qubits: {f.num_qubits}")
print(f"Dimension: {f.dimension}")

# Set to Bell-like state |0000⟩ + |1111⟩
real = np.zeros(16)
imag = np.zeros(16)
real[0] = 1/np.sqrt(2)
real[15] = 1/np.sqrt(2)
f.set_amplitudes(real, imag)

# Save to disk
f.save("state.qft")

# Load and verify
loaded = qft.load("state.qft")
print(f"Fidelity: {f.fidelity(loaded)}")  # 1.0
```

## API Reference

### QftFile Class

```python
class QftFile:
    def __init__(self, num_qubits: int = 1) -> None:
        """Create a new QFT file initialized to |0...0⟩."""
    
    # Properties
    num_qubits: int          # Number of qubits
    dimension: int           # State vector dimension (2^n)
    bond_dimension: int      # MPS bond dimension (1-1024)
    golay_enabled: bool      # Golay error correction
    source_path: str | None  # Source file path if loaded
    amplitudes_real: np.ndarray  # Real parts
    amplitudes_imag: np.ndarray  # Imaginary parts
    
    # Methods
    def set_amplitudes(self, real: np.ndarray, imag: np.ndarray) -> None:
        """Set all amplitudes from arrays."""
    
    def get_amplitude(self, index: int) -> tuple[float, float]:
        """Get single amplitude (real, imag)."""
    
    def set_amplitude(self, index: int, real: float, imag: float) -> None:
        """Set single amplitude."""
    
    def is_normalized(self, tolerance: float = 1e-10) -> bool:
        """Check if sum of |amplitude|² ≈ 1."""
    
    def normalize(self) -> None:
        """Normalize state in place."""
    
    def norm_squared(self) -> float:
        """Calculate sum of |amplitude|²."""
    
    def fidelity(self, other: QftFile) -> float:
        """Calculate |⟨self|other⟩|²."""
    
    def set_metadata(self, key: str, value: str) -> None:
        """Set metadata key-value pair."""
    
    def get_metadata(self, key: str) -> str | None:
        """Get metadata value."""
    
    def metadata_dict(self) -> dict[str, str]:
        """Get all metadata."""
    
    def save(self, path: str) -> None:
        """Save to disk."""
    
    def to_bytes(self) -> bytes:
        """Serialize to bytes."""
```

### Module Functions

```python
def load(path: str) -> QftFile:
    """Load a .qft file from disk."""

def save(state: QftFile, path: str, bond_dim: int = 64, golay: bool = True) -> None:
    """Save a quantum state to disk."""

def from_numpy(amplitudes: np.ndarray) -> QftFile:
    """Create from interleaved real/imag array."""

def from_arrays(real: np.ndarray, imag: np.ndarray) -> QftFile:
    """Create from separate real/imag arrays."""

def verify(path: str) -> bool:
    """Verify file integrity."""

def version() -> str:
    """Get library version."""
```

### Qiskit Integration

```python
from qiskit.quantum_info import Statevector
import qft

# Qiskit → QFT
sv = Statevector.from_label('00') + Statevector.from_label('11')
sv = sv / sv.norm()
f = qft.from_qiskit(sv)
f.save("bell.qft")

# QFT → Qiskit
loaded = qft.load("bell.qft")
sv_back = qft.to_qiskit(loaded)
print(sv_back)
```

### Streaming for Large Files

```python
import qft

# Convert a large file in chunks
total_size = 1_000_000_000  # 1GB
converter = qft.StreamingConverter(total_size, chunk_size=1024*1024)

with open("large_file.bin", "rb") as f:
    while not converter.is_complete:
        chunk = f.read(converter.chunk_size)
        if not chunk:
            break
        state = converter.process_chunk(chunk)
        state.save(f"chunk_{converter.progress:.2f}.qft")
        print(f"Progress: {converter.progress * 100:.1f}%")
```

### Encoding Strategies

```python
import qft

# Available encodings
qft.Encoding.Amplitude   # Byte values as amplitudes
qft.Encoding.BasisState  # Bytes as basis state indices
qft.Encoding.Qram        # QRAM-style superposition
qft.Encoding.Angle       # Angle encoding for continuous data
qft.Encoding.Block       # Block encoding for matrices
```

## Examples

### Create Bell State

```python
import qft
import numpy as np

f = qft.QftFile(2)
real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
imag = np.zeros(4)
f.set_amplitudes(real, imag)
f.save("bell.qft")
```

### Calculate Fidelity

```python
import qft

state1 = qft.load("state1.qft")
state2 = qft.load("state2.qft")
fidelity = state1.fidelity(state2)
print(f"Fidelity: {fidelity:.6f}")
```

### Add Metadata

```python
import qft

f = qft.QftFile(4)
f.set_metadata("author", "Alice")
f.set_metadata("experiment", "VQE ground state")
f.set_metadata("date", "2026-01-25")
f.save("annotated.qft")

# Read metadata
loaded = qft.load("annotated.qft")
print(loaded.metadata_dict())
```

### Batch Processing

```python
import qft
from pathlib import Path

# Convert all .npy files to .qft
for npy_file in Path("states/").glob("*.npy"):
    data = np.load(npy_file)
    real = data.real.flatten()
    imag = data.imag.flatten()
    
    f = qft.from_arrays(real, imag)
    f.set_metadata("source", str(npy_file))
    f.save(npy_file.with_suffix(".qft"))
```

## Performance

| Operation | 10 qubits | 20 qubits | 25 qubits |
|-----------|-----------|-----------|-----------|
| Create | 0.1 ms | 1 ms | 32 ms |
| Save | 0.5 ms | 50 ms | 1.6 s |
| Load | 0.3 ms | 30 ms | 1.0 s |
| Fidelity | 0.1 ms | 10 ms | 320 ms |

## Thread Safety

- All functions are thread-safe
- Each `QftFile` instance should only be used from one thread at a time
- Use separate instances for parallel processing

## License

MIT OR Apache-2.0
