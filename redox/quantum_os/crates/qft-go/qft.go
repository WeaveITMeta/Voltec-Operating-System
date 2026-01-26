// Package qft provides a production-grade Go SDK for Quantum File Type (.qft) format.
//
// Table of Contents:
//  1. Types and Interfaces
//  2. QftFile Implementation
//  3. State Operations
//  4. I/O Operations
//  5. Common States
//  6. Validation Functions
package qft

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"math/cmplx"
	"os"
)

// =============================================================================
// Section 1: Types and Interfaces
// =============================================================================

// Version is the library version
const Version = "1.0.0"

// Error types
var (
	ErrInvalidQubits     = errors.New("qft: invalid number of qubits (must be 1-30)")
	ErrIndexOutOfRange   = errors.New("qft: index out of range")
	ErrDimensionMismatch = errors.New("qft: dimension mismatch")
	ErrZeroNorm          = errors.New("qft: cannot normalize zero state")
	ErrInvalidFormat     = errors.New("qft: invalid file format")
	ErrInvalidMagic      = errors.New("qft: invalid magic number")
)

// Config holds configuration for QFT file operations
type Config struct {
	// BondDimension for MPS compression (1-1024)
	BondDimension int `json:"bondDimension"`
	// GolayEnabled enables Golay error correction
	GolayEnabled bool `json:"golayEnabled"`
	// TruncationThreshold for SVD
	TruncationThreshold float64 `json:"truncationThreshold"`
}

// DefaultConfig returns the default configuration
func DefaultConfig() Config {
	return Config{
		BondDimension:       64,
		GolayEnabled:        true,
		TruncationThreshold: 1e-10,
	}
}

// =============================================================================
// Section 2: QftFile Implementation
// =============================================================================

// QftFile represents a quantum state file in .qft format
type QftFile struct {
	numQubits  int
	amplitudes []complex128
	metadata   map[string]string
	config     Config
}

// New creates a new QFT file with the specified number of qubits.
// Initializes to the |0...0⟩ state.
func New(numQubits int) (*QftFile, error) {
	return NewWithConfig(numQubits, DefaultConfig())
}

// NewWithConfig creates a new QFT file with custom configuration
func NewWithConfig(numQubits int, config Config) (*QftFile, error) {
	if numQubits < 1 || numQubits > 30 {
		return nil, ErrInvalidQubits
	}

	dim := 1 << numQubits
	amplitudes := make([]complex128, dim)
	amplitudes[0] = complex(1.0, 0.0) // |0...0⟩

	return &QftFile{
		numQubits:  numQubits,
		amplitudes: amplitudes,
		metadata:   make(map[string]string),
		config:     config,
	}, nil
}

// FromAmplitudes creates a QFT file from a slice of complex amplitudes
func FromAmplitudes(amplitudes []complex128) (*QftFile, error) {
	return FromAmplitudesWithConfig(amplitudes, DefaultConfig())
}

// FromAmplitudesWithConfig creates a QFT file from amplitudes with custom config
func FromAmplitudesWithConfig(amplitudes []complex128, config Config) (*QftFile, error) {
	dim := len(amplitudes)
	if dim == 0 || (dim&(dim-1)) != 0 {
		return nil, fmt.Errorf("qft: amplitude count must be a power of 2, got %d", dim)
	}

	numQubits := 0
	for d := dim; d > 1; d >>= 1 {
		numQubits++
	}

	if numQubits > 30 {
		return nil, ErrInvalidQubits
	}

	ampCopy := make([]complex128, dim)
	copy(ampCopy, amplitudes)

	return &QftFile{
		numQubits:  numQubits,
		amplitudes: ampCopy,
		metadata:   make(map[string]string),
		config:     config,
	}, nil
}

// FromRealImag creates a QFT file from separate real and imaginary slices
func FromRealImag(real, imag []float64) (*QftFile, error) {
	if len(real) != len(imag) {
		return nil, ErrDimensionMismatch
	}

	amplitudes := make([]complex128, len(real))
	for i := range real {
		amplitudes[i] = complex(real[i], imag[i])
	}

	return FromAmplitudes(amplitudes)
}

// =============================================================================
// Properties
// =============================================================================

// NumQubits returns the number of qubits
func (f *QftFile) NumQubits() int {
	return f.numQubits
}

// Dimension returns the state vector dimension (2^numQubits)
func (f *QftFile) Dimension() int {
	return 1 << f.numQubits
}

// Config returns the configuration
func (f *QftFile) Config() Config {
	return f.config
}

// SetConfig updates the configuration
func (f *QftFile) SetConfig(config Config) {
	f.config = config
}

// =============================================================================
// Amplitude Access
// =============================================================================

// Amplitudes returns all amplitudes
func (f *QftFile) Amplitudes() []complex128 {
	return f.amplitudes
}

// AmplitudesReal returns the real parts of all amplitudes
func (f *QftFile) AmplitudesReal() []float64 {
	result := make([]float64, len(f.amplitudes))
	for i, a := range f.amplitudes {
		result[i] = real(a)
	}
	return result
}

// AmplitudesImag returns the imaginary parts of all amplitudes
func (f *QftFile) AmplitudesImag() []float64 {
	result := make([]float64, len(f.amplitudes))
	for i, a := range f.amplitudes {
		result[i] = imag(a)
	}
	return result
}

// GetAmplitude returns a single amplitude by basis state index
func (f *QftFile) GetAmplitude(index int) (complex128, error) {
	if index < 0 || index >= f.Dimension() {
		return 0, ErrIndexOutOfRange
	}
	return f.amplitudes[index], nil
}

// SetAmplitude sets a single amplitude by basis state index
func (f *QftFile) SetAmplitude(index int, value complex128) error {
	if index < 0 || index >= f.Dimension() {
		return ErrIndexOutOfRange
	}
	f.amplitudes[index] = value
	return nil
}

// SetAmplitudes sets all amplitudes from a slice
func (f *QftFile) SetAmplitudes(amplitudes []complex128) error {
	if len(amplitudes) != f.Dimension() {
		return ErrDimensionMismatch
	}
	copy(f.amplitudes, amplitudes)
	return nil
}

// =============================================================================
// Metadata
// =============================================================================

// SetMetadata sets a metadata key-value pair
func (f *QftFile) SetMetadata(key, value string) {
	f.metadata[key] = value
}

// GetMetadata returns a metadata value by key
func (f *QftFile) GetMetadata(key string) (string, bool) {
	v, ok := f.metadata[key]
	return v, ok
}

// Metadata returns all metadata
func (f *QftFile) Metadata() map[string]string {
	result := make(map[string]string)
	for k, v := range f.metadata {
		result[k] = v
	}
	return result
}

// =============================================================================
// Section 3: State Operations
// =============================================================================

// NormSquared calculates the norm squared (sum of |amplitude|²)
func (f *QftFile) NormSquared() float64 {
	var sum float64
	for _, a := range f.amplitudes {
		sum += real(a)*real(a) + imag(a)*imag(a)
	}
	return sum
}

// Norm calculates the norm
func (f *QftFile) Norm() float64 {
	return math.Sqrt(f.NormSquared())
}

// IsNormalized checks if the state is normalized within tolerance
func (f *QftFile) IsNormalized(tolerance float64) bool {
	return math.Abs(f.NormSquared()-1.0) < tolerance
}

// Normalize normalizes the state vector in place
func (f *QftFile) Normalize() error {
	norm := f.Norm()
	if norm < 1e-15 {
		return ErrZeroNorm
	}
	for i := range f.amplitudes {
		f.amplitudes[i] /= complex(norm, 0)
	}
	return nil
}

// InnerProduct calculates ⟨f|other⟩
func (f *QftFile) InnerProduct(other *QftFile) (complex128, error) {
	if f.numQubits != other.numQubits {
		return 0, ErrDimensionMismatch
	}

	var result complex128
	for i := range f.amplitudes {
		result += cmplx.Conj(f.amplitudes[i]) * other.amplitudes[i]
	}
	return result, nil
}

// Fidelity calculates |⟨f|other⟩|²
func (f *QftFile) Fidelity(other *QftFile) (float64, error) {
	overlap, err := f.InnerProduct(other)
	if err != nil {
		return 0, err
	}
	return real(overlap)*real(overlap) + imag(overlap)*imag(overlap), nil
}

// TraceDistance calculates the trace distance to another state
func (f *QftFile) TraceDistance(other *QftFile) (float64, error) {
	fid, err := f.Fidelity(other)
	if err != nil {
		return 0, err
	}
	return math.Sqrt(1.0 - fid), nil
}

// Clone creates a deep copy
func (f *QftFile) Clone() *QftFile {
	ampCopy := make([]complex128, len(f.amplitudes))
	copy(ampCopy, f.amplitudes)

	metaCopy := make(map[string]string)
	for k, v := range f.metadata {
		metaCopy[k] = v
	}

	return &QftFile{
		numQubits:  f.numQubits,
		amplitudes: ampCopy,
		metadata:   metaCopy,
		config:     f.config,
	}
}

// String returns a string representation
func (f *QftFile) String() string {
	return fmt.Sprintf("QftFile(%d qubits, dim=%d, norm=%.6f)",
		f.numQubits, f.Dimension(), f.Norm())
}

// =============================================================================
// Section 4: I/O Operations
// =============================================================================

// Magic number for QFT files
var qftMagic = []byte{'Q', 'F', 'T', 0x01}

// Load reads a QFT file from disk
func Load(path string) (*QftFile, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	return ReadFrom(file)
}

// Save writes the QFT file to disk
func (f *QftFile) Save(path string) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	return f.WriteTo(file)
}

// ReadFrom reads from any io.Reader
func ReadFrom(r io.Reader) (*QftFile, error) {
	// Read header
	header := make([]byte, 16)
	if _, err := io.ReadFull(r, header); err != nil {
		return nil, err
	}

	// Verify magic
	if !bytes.Equal(header[0:4], qftMagic) {
		return nil, ErrInvalidMagic
	}

	numQubits := int(header[4])
	if numQubits < 1 || numQubits > 30 {
		return nil, ErrInvalidQubits
	}

	bondDim := int(header[5])
	if bondDim == 0 {
		bondDim = 64
	}
	golayEnabled := header[6] != 0

	dim := 1 << numQubits
	amplitudes := make([]complex128, dim)

	// Read amplitudes
	for i := 0; i < dim; i++ {
		var re, im float64
		if err := binary.Read(r, binary.LittleEndian, &re); err != nil {
			return nil, err
		}
		if err := binary.Read(r, binary.LittleEndian, &im); err != nil {
			return nil, err
		}
		amplitudes[i] = complex(re, im)
	}

	return &QftFile{
		numQubits:  numQubits,
		amplitudes: amplitudes,
		metadata:   make(map[string]string),
		config: Config{
			BondDimension:       bondDim,
			GolayEnabled:        golayEnabled,
			TruncationThreshold: 1e-10,
		},
	}, nil
}

// WriteTo writes to any io.Writer
func (f *QftFile) WriteTo(w io.Writer) error {
	// Write header
	header := make([]byte, 16)
	copy(header[0:4], qftMagic)
	header[4] = byte(f.numQubits)
	header[5] = byte(min(f.config.BondDimension, 255))
	if f.config.GolayEnabled {
		header[6] = 1
	}

	if _, err := w.Write(header); err != nil {
		return err
	}

	// Write amplitudes
	for _, a := range f.amplitudes {
		if err := binary.Write(w, binary.LittleEndian, real(a)); err != nil {
			return err
		}
		if err := binary.Write(w, binary.LittleEndian, imag(a)); err != nil {
			return err
		}
	}

	return nil
}

// ToBytes serializes to a byte slice
func (f *QftFile) ToBytes() ([]byte, error) {
	var buf bytes.Buffer
	if err := f.WriteTo(&buf); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// FromBytes deserializes from a byte slice
func FromBytes(data []byte) (*QftFile, error) {
	return ReadFrom(bytes.NewReader(data))
}

// jsonExport is the JSON serialization format
type jsonExport struct {
	NumQubits      int               `json:"numQubits"`
	Config         Config            `json:"config"`
	AmplitudesReal []float64         `json:"amplitudesReal"`
	AmplitudesImag []float64         `json:"amplitudesImag"`
	Metadata       map[string]string `json:"metadata"`
}

// ToJSON exports to JSON
func (f *QftFile) ToJSON() (string, error) {
	export := jsonExport{
		NumQubits:      f.numQubits,
		Config:         f.config,
		AmplitudesReal: f.AmplitudesReal(),
		AmplitudesImag: f.AmplitudesImag(),
		Metadata:       f.Metadata(),
	}

	data, err := json.MarshalIndent(export, "", "  ")
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// FromJSON imports from JSON
func FromJSON(jsonStr string) (*QftFile, error) {
	var export jsonExport
	if err := json.Unmarshal([]byte(jsonStr), &export); err != nil {
		return nil, err
	}

	file, err := FromRealImag(export.AmplitudesReal, export.AmplitudesImag)
	if err != nil {
		return nil, err
	}

	file.config = export.Config
	file.metadata = export.Metadata
	return file, nil
}

// =============================================================================
// Section 5: Common States
// =============================================================================

// BellState creates a Bell state (|00⟩ + |11⟩) / √2
func BellState() (*QftFile, error) {
	sqrt2Inv := 1.0 / math.Sqrt(2.0)
	return FromAmplitudes([]complex128{
		complex(sqrt2Inv, 0),
		0,
		0,
		complex(sqrt2Inv, 0),
	})
}

// GHZState creates a GHZ state (|0...0⟩ + |1...1⟩) / √2
func GHZState(numQubits int) (*QftFile, error) {
	file, err := New(numQubits)
	if err != nil {
		return nil, err
	}

	sqrt2Inv := 1.0 / math.Sqrt(2.0)
	file.amplitudes[0] = complex(sqrt2Inv, 0)
	file.amplitudes[file.Dimension()-1] = complex(sqrt2Inv, 0)
	return file, nil
}

// UniformState creates a uniform superposition state
func UniformState(numQubits int) (*QftFile, error) {
	file, err := New(numQubits)
	if err != nil {
		return nil, err
	}

	amp := 1.0 / math.Sqrt(float64(file.Dimension()))
	for i := range file.amplitudes {
		file.amplitudes[i] = complex(amp, 0)
	}
	return file, nil
}

// BasisState creates a computational basis state |i⟩
func BasisState(numQubits, index int) (*QftFile, error) {
	file, err := New(numQubits)
	if err != nil {
		return nil, err
	}

	if index < 0 || index >= file.Dimension() {
		return nil, ErrIndexOutOfRange
	}

	file.amplitudes[0] = 0
	file.amplitudes[index] = complex(1.0, 0)
	return file, nil
}

// =============================================================================
// Section 6: Validation Functions
// =============================================================================

// Verify checks if a byte slice is a valid QFT file
func Verify(data []byte) bool {
	file, err := FromBytes(data)
	if err != nil {
		return false
	}
	return file.IsNormalized(1e-6)
}

// FidelityFromBytes calculates fidelity between two byte arrays
func FidelityFromBytes(data1, data2 []byte) (float64, error) {
	file1, err := FromBytes(data1)
	if err != nil {
		return 0, err
	}
	file2, err := FromBytes(data2)
	if err != nil {
		return 0, err
	}
	return file1.Fidelity(file2)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
