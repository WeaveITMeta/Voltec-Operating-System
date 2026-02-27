# qft-scala - Scala SDK for Quantum File Type

Production-grade Scala library for working with quantum states in the .qft format.

## Installation

### sbt

```scala
libraryDependencies += "dev.v-os" %% "qft-scala" % "1.0.0"
```

### Maven

```xml
<dependency>
    <groupId>dev.v-os</groupId>
    <artifactId>qft-scala_3</artifactId>
    <version>1.0.0</version>
</dependency>
```

## Quick Start

```scala
import dev.v-os.qft._

// Create a 4-qubit state
val state = QftFile(4)
println(s"Qubits: ${state.numQubits}")
println(s"Dimension: ${state.dimension}")

// Set Bell-like state |0000⟩ + |1111⟩
val sqrt2Inv = 1.0 / Math.sqrt(2.0)
state(0) = Complex(sqrt2Inv)
state(15) = Complex(sqrt2Inv)

// Save to disk
state.save("state.qft")

// Load and verify
val loaded = QftFile.load("state.qft").get
println(s"Fidelity: ${state.fidelity(loaded)}")
```

## Features

### Complex Numbers

```scala
import dev.v-os.qft.Complex

val a = Complex(1, 2)                    // 1 + 2i
val b = Complex.fromPolar(1, Math.PI/4)  // e^(iπ/4)

println(a + b)        // Addition
println(a * b)        // Multiplication
println(a.conj)       // Conjugate
println(a.norm)       // Magnitude
println(a.normSqr)    // |a|²

// Constants
Complex.Zero  // 0
Complex.One   // 1
Complex.I     // i
```

### Common States

```scala
val bell = QftFile.bellState           // |00⟩ + |11⟩
val ghz = QftFile.ghzState(4)          // |0000⟩ + |1111⟩
val uniform = QftFile.uniformState(3)  // Equal superposition
val basis = QftFile.basisState(4, 5)   // |0101⟩
```

### State Operations

```scala
val state = QftFile(4)

// Normalization
state.normalize()
println(state.isNormalized())  // true

// Fidelity
val other = QftFile(4)
val fid = state.fidelity(other)

// Inner product
val overlap = state.innerProduct(other)

// Trace distance
val dist = state.traceDistance(other)
```

### Amplitude Access

```scala
val state = QftFile(4)

// Index syntax
val amp = state(0)           // Get
state(0) = Complex(0.5, 0.5) // Set

// Bulk access
val amps = state.amplitudes
val real = state.amplitudesReal
val imag = state.amplitudesImag
```

### Configuration

```scala
val config = QftConfig(
  bondDimension = 128,
  golayEnabled = true,
  truncationThreshold = 1e-12
)

val state = QftFile(4, config)
```

### Metadata

```scala
val state = QftFile(4)

state.setMetadata("author", "Alice")
state.setMetadata("date", "2026-01-25")

val author = state.getMetadata("author")  // Option[String]
val all = state.metadata                   // Map[String, String]
```

### Serialization

```scala
// Binary format
state.save("state.qft")
val loaded = QftFile.load("state.qft")  // Try[QftFile]

// Bytes
val bytes = state.toBytes
val fromBytes = QftFile.fromBytes(bytes)  // Try[QftFile]

// JSON
val json = state.toJson
```

### Error Handling

```scala
import scala.util.{Try, Success, Failure}

QftFile.load("state.qft") match {
  case Success(file) => println(s"Loaded: $file")
  case Failure(e) => println(s"Error: ${e.getMessage}")
}

// Or with for-comprehension
for {
  file1 <- QftFile.load("state1.qft")
  file2 <- QftFile.load("state2.qft")
} yield file1.fidelity(file2)
```

## API Reference

### QftFile

| Method | Description |
|--------|-------------|
| `QftFile(n)` | Create n-qubit state |
| `QftFile(n, config)` | Create with config |
| `fromAmplitudes(seq)` | Create from amplitudes |
| `fromRealImag(r, i)` | Create from arrays |
| `load(path)` | Load from file (Try) |
| `fromBytes(data)` | Deserialize (Try) |
| `numQubits` | Get qubit count |
| `dimension` | Get dimension (2^n) |
| `amplitudes` | Get all amplitudes |
| `amplitudesReal` | Get real parts |
| `amplitudesImag` | Get imaginary parts |
| `apply(i)` | Get amplitude |
| `update(i, v)` | Set amplitude |
| `setAmplitudes(seq)` | Set all amplitudes |
| `norm` | Calculate norm |
| `normSquared` | Calculate norm² |
| `isNormalized(tol)` | Check normalization |
| `normalize()` | Normalize in place |
| `innerProduct(other)` | Calculate ⟨this\|other⟩ |
| `fidelity(other)` | Calculate \|⟨this\|other⟩\|² |
| `traceDistance(other)` | Calculate trace distance |
| `copy` | Deep copy |
| `save(path)` | Save to file |
| `toBytes` | Serialize to bytes |
| `toJson` | Export to JSON |
| `setMetadata(k, v)` | Set metadata |
| `getMetadata(k)` | Get metadata (Option) |
| `metadata` | Get all metadata |
| `config` | Get configuration |

### Complex

| Method | Description |
|--------|-------------|
| `Complex(re, im)` | Create complex |
| `Complex.fromPolar(r, θ)` | From polar |
| `re`, `im` | Components |
| `conj` | Conjugate |
| `norm` | Magnitude |
| `normSqr` | Magnitude² |
| `+`, `-`, `*`, `/` | Arithmetic |

### Static Factory Methods

| Method | Description |
|--------|-------------|
| `bellState` | (|00⟩ + |11⟩) / √2 |
| `ghzState(n)` | (|0...0⟩ + |1...1⟩) / √2 |
| `uniformState(n)` | Equal superposition |
| `basisState(n, i)` | Computational basis |
| `verify(bytes)` | Check validity |
| `fidelityFromBytes(d1, d2)` | Fidelity (Try) |

## Requirements

- Scala 3.3+
- Java 17+
- Gson 2.10+ (for JSON)

## Thread Safety

`QftFile` is **not** thread-safe. Use external synchronization or create copies for concurrent access.

## License

MIT OR Apache-2.0
