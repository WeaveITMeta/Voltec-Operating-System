# qft-go - Go SDK for Quantum File Type

Production-grade Go library for working with quantum states in the .qft format.

## Installation

```bash
go get github.com/WeaveITMeta/Quantum-Operating-System/qft-go
```

## Quick Start

```go
package main

import (
    "fmt"
    "log"
    
    "github.com/WeaveITMeta/Quantum-Operating-System/qft-go"
)

func main() {
    // Create a 4-qubit state
    state, err := qft.New(4)
    if err != nil {
        log.Fatal(err)
    }
    
    fmt.Printf("Qubits: %d\n", state.NumQubits())
    fmt.Printf("Dimension: %d\n", state.Dimension())
    
    // Save to disk
    if err := state.Save("state.qft"); err != nil {
        log.Fatal(err)
    }
    
    // Load and verify
    loaded, err := qft.Load("state.qft")
    if err != nil {
        log.Fatal(err)
    }
    
    fid, _ := state.Fidelity(loaded)
    fmt.Printf("Fidelity: %.6f\n", fid)
}
```

## Features

### Common States

```go
bell, _ := qft.BellState()           // |00⟩ + |11⟩
ghz, _ := qft.GHZState(4)            // |0000⟩ + |1111⟩
uniform, _ := qft.UniformState(3)    // Equal superposition
basis, _ := qft.BasisState(4, 5)     // |0101⟩
```

### State Operations

```go
state, _ := qft.New(4)

// Normalization
state.Normalize()
fmt.Println(state.IsNormalized(1e-10))  // true

// Fidelity
other, _ := qft.New(4)
fid, _ := state.Fidelity(other)

// Inner product
overlap, _ := state.InnerProduct(other)

// Trace distance
dist, _ := state.TraceDistance(other)
```

### Amplitude Access

```go
state, _ := qft.New(4)

// Get/set single amplitude
amp, _ := state.GetAmplitude(0)
state.SetAmplitude(0, complex(0.5, 0.5))

// Get all amplitudes
amps := state.Amplitudes()
real := state.AmplitudesReal()
imag := state.AmplitudesImag()
```

### Configuration

```go
config := qft.Config{
    BondDimension:       128,
    GolayEnabled:        true,
    TruncationThreshold: 1e-12,
}

state, _ := qft.NewWithConfig(4, config)
```

### Metadata

```go
state, _ := qft.New(4)

state.SetMetadata("author", "Alice")
state.SetMetadata("date", "2026-01-25")

author, ok := state.GetMetadata("author")
if ok {
    fmt.Println("Author:", author)
}

meta := state.Metadata()
```

### Serialization

```go
// Binary format
state.Save("state.qft")
loaded, _ := qft.Load("state.qft")

// Bytes
bytes, _ := state.ToBytes()
fromBytes, _ := qft.FromBytes(bytes)

// JSON
jsonStr, _ := state.ToJSON()
fromJSON, _ := qft.FromJSON(jsonStr)
```

### I/O with Readers/Writers

```go
import "bytes"

var buf bytes.Buffer
state.WriteTo(&buf)

loaded, _ := qft.ReadFrom(&buf)
```

## API Reference

### QftFile

| Method | Description |
|--------|-------------|
| `New(n)` | Create n-qubit state |
| `NewWithConfig(n, cfg)` | Create with config |
| `FromAmplitudes([]complex128)` | Create from amplitudes |
| `FromRealImag([]float64, []float64)` | Create from real/imag |
| `Load(path)` | Load from file |
| `FromBytes([]byte)` | Deserialize |
| `FromJSON(string)` | Parse JSON |
| `NumQubits()` | Get qubit count |
| `Dimension()` | Get dimension (2^n) |
| `Amplitudes()` | Get all amplitudes |
| `AmplitudesReal()` | Get real parts |
| `AmplitudesImag()` | Get imaginary parts |
| `GetAmplitude(i)` | Get single amplitude |
| `SetAmplitude(i, v)` | Set single amplitude |
| `SetAmplitudes([]complex128)` | Set all amplitudes |
| `Norm()` | Calculate norm |
| `NormSquared()` | Calculate norm² |
| `IsNormalized(tol)` | Check normalization |
| `Normalize()` | Normalize in place |
| `InnerProduct(other)` | Calculate ⟨this\|other⟩ |
| `Fidelity(other)` | Calculate \|⟨this\|other⟩\|² |
| `TraceDistance(other)` | Calculate trace distance |
| `Clone()` | Deep copy |
| `Save(path)` | Save to file |
| `WriteTo(io.Writer)` | Write to writer |
| `ToBytes()` | Serialize to bytes |
| `ToJSON()` | Export to JSON |
| `SetMetadata(k, v)` | Set metadata |
| `GetMetadata(k)` | Get metadata |
| `Metadata()` | Get all metadata |
| `Config()` | Get configuration |
| `SetConfig(cfg)` | Update configuration |
| `String()` | String representation |

### Common States

| Function | Description |
|----------|-------------|
| `BellState()` | (|00⟩ + |11⟩) / √2 |
| `GHZState(n)` | (|0...0⟩ + |1...1⟩) / √2 |
| `UniformState(n)` | Equal superposition |
| `BasisState(n, i)` | Computational basis |

### Error Types

| Error | Description |
|-------|-------------|
| `ErrInvalidQubits` | Qubit count not in 1-30 |
| `ErrIndexOutOfRange` | Index too large |
| `ErrDimensionMismatch` | Array sizes don't match |
| `ErrZeroNorm` | Cannot normalize zero |
| `ErrInvalidFormat` | Not a valid .qft file |
| `ErrInvalidMagic` | Wrong magic number |

## Thread Safety

`QftFile` is **not** thread-safe. Use external synchronization if accessing from multiple goroutines.

## License

MIT OR Apache-2.0
