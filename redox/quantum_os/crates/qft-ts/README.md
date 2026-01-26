# @qft/core - TypeScript SDK for Quantum File Type

Production-grade TypeScript library for working with quantum states in the .qft format.

## Installation

```bash
npm install @qft/core
# or
pnpm add @qft/core
# or
bun add @qft/core
```

## Quick Start

```typescript
import { QftFile, bellState, Complex } from '@qft/core';

// Create a 4-qubit state
const state = new QftFile(4);
console.log(`Qubits: ${state.numQubits}`);
console.log(`Dimension: ${state.dimension}`);

// Set to GHZ-like state |0000⟩ + |1111⟩
const sqrt2Inv = 1 / Math.sqrt(2);
state.setAmplitude(0, new Complex(sqrt2Inv));
state.setAmplitude(15, new Complex(sqrt2Inv));

// Serialize
const bytes = state.toBytes();
console.log(`Serialized: ${bytes.length} bytes`);

// Deserialize
const loaded = QftFile.fromBytes(bytes);
console.log(`Fidelity: ${state.fidelity(loaded)}`);
```

## Features

### Complex Numbers

```typescript
import { Complex, ZERO, ONE, I } from '@qft/core';

const a = new Complex(1, 2);      // 1 + 2i
const b = Complex.fromPolar(1, Math.PI / 4);  // e^(iπ/4)

console.log(a.add(b));            // Addition
console.log(a.mul(b));            // Multiplication
console.log(a.conj());            // Conjugate
console.log(a.norm());            // Magnitude
console.log(a.normSqr());         // |a|²
```

### Builder API

```typescript
import { QftBuilder, Complex } from '@qft/core';

const state = new QftBuilder(4)
  .bondDimension(128)
  .golay(true)
  .metadata('author', 'Alice')
  .metadata('experiment', 'VQE')
  .amplitudes([new Complex(1), ...])
  .build();
```

### Common States

```typescript
import { bellState, ghzState, uniformState, basisState } from '@qft/core';

const bell = bellState();              // |00⟩ + |11⟩
const ghz = ghzState(4);               // |0000⟩ + |1111⟩
const uniform = uniformState(3);       // Equal superposition
const basis = basisState(4, 5);        // |0101⟩
```

### State Operations

```typescript
const state = new QftFile(4);

// Normalization
state.normalize();
console.log(state.isNormalized());     // true

// Fidelity
const other = new QftFile(4);
const fid = state.fidelity(other);     // |⟨ψ|φ⟩|²

// Inner product
const overlap = state.innerProduct(other);  // ⟨ψ|φ⟩

// Trace distance
const dist = state.traceDistance(other);
```

### Serialization

```typescript
// Binary format
const bytes = state.toBytes();
const fromBytes = QftFile.fromBytes(bytes);

// JSON format
const json = state.toJSON();
const fromJson = QftFile.fromJSON(json);
```

### File I/O (Node.js)

```typescript
import { load, save } from '@qft/core';

// Save
await save(state, 'state.qft');

// Load
const loaded = await load('state.qft');
```

### Metadata

```typescript
const state = new QftFile(4);

state.setMetadata('author', 'Alice');
state.setMetadata('date', '2026-01-25');

console.log(state.getMetadata('author'));  // 'Alice'
console.log(state.metadata);               // { author: 'Alice', date: '2026-01-25' }
```

## API Reference

### QftFile

| Property/Method | Description |
|----------------|-------------|
| `new QftFile(n, config?)` | Create n-qubit state |
| `QftFile.fromAmplitudes(arr)` | Create from complex array |
| `QftFile.fromRealImag(r, i)` | Create from real/imag arrays |
| `QftFile.fromBytes(data)` | Deserialize from bytes |
| `QftFile.fromJSON(json)` | Deserialize from JSON |
| `numQubits` | Number of qubits |
| `dimension` | State dimension (2^n) |
| `amplitudes` | Get all amplitudes |
| `amplitudesReal` | Get real parts |
| `amplitudesImag` | Get imaginary parts |
| `getAmplitude(i)` | Get single amplitude |
| `setAmplitude(i, v)` | Set single amplitude |
| `setAmplitudes(arr)` | Set all amplitudes |
| `norm()` | Calculate norm |
| `normSquared()` | Calculate norm² |
| `isNormalized(tol?)` | Check normalization |
| `normalize()` | Normalize in place |
| `innerProduct(other)` | Calculate ⟨this\|other⟩ |
| `fidelity(other)` | Calculate \|⟨this\|other⟩\|² |
| `traceDistance(other)` | Calculate trace distance |
| `clone()` | Deep copy |
| `toBytes()` | Serialize to bytes |
| `toJSON()` | Serialize to JSON |
| `setMetadata(k, v)` | Set metadata |
| `getMetadata(k)` | Get metadata |
| `metadata` | Get all metadata |
| `config` | Get configuration |
| `setConfig(cfg)` | Update configuration |

### Complex

| Property/Method | Description |
|----------------|-------------|
| `new Complex(re, im?)` | Create complex number |
| `Complex.fromPolar(r, θ)` | Create from polar |
| `re` | Real part |
| `im` | Imaginary part |
| `conj()` | Conjugate |
| `norm()` | Magnitude |
| `normSqr()` | Magnitude squared |
| `add(other)` | Addition |
| `sub(other)` | Subtraction |
| `mul(other)` | Multiplication |
| `div(other)` | Division |
| `equals(other, tol?)` | Equality check |

### QftBuilder

| Method | Description |
|--------|-------------|
| `new QftBuilder(n)` | Create builder |
| `bondDimension(d)` | Set bond dimension |
| `golay(bool)` | Enable/disable Golay |
| `truncationThreshold(t)` | Set SVD threshold |
| `metadata(k, v)` | Add metadata |
| `amplitudes(arr)` | Set amplitudes |
| `build()` | Build QftFile |

### Error Handling

```typescript
import { QftError, QftErrorCode } from '@qft/core';

try {
  const state = new QftFile(50);  // Too many qubits
} catch (e) {
  if (e instanceof QftError) {
    console.log(e.code);     // QftErrorCode.InvalidQubits
    console.log(e.message);  // 'num_qubits must be 1-30, got 50'
  }
}
```

## Browser Usage

```html
<script type="module">
  import { QftFile, bellState } from 'https://esm.sh/@qft/core';
  
  const bell = bellState();
  console.log(bell.toString());
</script>
```

## TypeScript Support

Full TypeScript support with strict types:

```typescript
import type { QftConfig } from '@qft/core';

const config: QftConfig = {
  bondDimension: 128,
  golayEnabled: true,
  truncationThreshold: 1e-12,
};
```

## License

MIT OR Apache-2.0
