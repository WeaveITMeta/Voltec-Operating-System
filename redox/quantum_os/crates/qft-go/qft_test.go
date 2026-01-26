package qft

import (
	"math"
	"path/filepath"
	"testing"
)

func TestNew(t *testing.T) {
	file, err := New(4)
	if err != nil {
		t.Fatalf("New(4) failed: %v", err)
	}

	if file.NumQubits() != 4 {
		t.Errorf("NumQubits() = %d, want 4", file.NumQubits())
	}

	if file.Dimension() != 16 {
		t.Errorf("Dimension() = %d, want 16", file.Dimension())
	}
}

func TestInvalidQubits(t *testing.T) {
	_, err := New(0)
	if err == nil {
		t.Error("New(0) should fail")
	}

	_, err = New(31)
	if err == nil {
		t.Error("New(31) should fail")
	}
}

func TestAmplitudes(t *testing.T) {
	file, _ := New(2)

	// Initial state is |00⟩
	amp, _ := file.GetAmplitude(0)
	if real(amp) != 1.0 || imag(amp) != 0.0 {
		t.Errorf("Initial amplitude[0] = %v, want 1+0i", amp)
	}

	// Set to |11⟩
	file.SetAmplitude(0, 0)
	file.SetAmplitude(3, complex(1.0, 0))

	amp, _ = file.GetAmplitude(3)
	if real(amp) != 1.0 {
		t.Errorf("amplitude[3] = %v, want 1+0i", amp)
	}
}

func TestNormalization(t *testing.T) {
	file, _ := New(2)

	// Set unnormalized state
	file.SetAmplitude(0, complex(1.0, 0))
	file.SetAmplitude(1, complex(1.0, 0))

	if file.IsNormalized(1e-10) {
		t.Error("State should not be normalized")
	}

	file.Normalize()

	if !file.IsNormalized(1e-10) {
		t.Error("State should be normalized after Normalize()")
	}
}

func TestFidelity(t *testing.T) {
	file1, _ := New(2)
	file2, _ := New(2)

	// Same state should have fidelity 1
	fid, _ := file1.Fidelity(file2)
	if math.Abs(fid-1.0) > 1e-10 {
		t.Errorf("Fidelity of same state = %v, want 1.0", fid)
	}

	// Orthogonal states should have fidelity 0
	file2.SetAmplitude(0, 0)
	file2.SetAmplitude(1, complex(1.0, 0))

	fid, _ = file1.Fidelity(file2)
	if math.Abs(fid) > 1e-10 {
		t.Errorf("Fidelity of orthogonal states = %v, want 0.0", fid)
	}
}

func TestBellState(t *testing.T) {
	bell, err := BellState()
	if err != nil {
		t.Fatalf("BellState() failed: %v", err)
	}

	if bell.NumQubits() != 2 {
		t.Errorf("Bell state qubits = %d, want 2", bell.NumQubits())
	}

	if !bell.IsNormalized(1e-10) {
		t.Error("Bell state should be normalized")
	}

	// Check amplitudes
	amp0, _ := bell.GetAmplitude(0)
	amp3, _ := bell.GetAmplitude(3)
	sqrt2Inv := 1.0 / math.Sqrt(2.0)

	if math.Abs(real(amp0)-sqrt2Inv) > 1e-10 {
		t.Errorf("Bell amplitude[0] = %v, want %v", amp0, sqrt2Inv)
	}
	if math.Abs(real(amp3)-sqrt2Inv) > 1e-10 {
		t.Errorf("Bell amplitude[3] = %v, want %v", amp3, sqrt2Inv)
	}
}

func TestGHZState(t *testing.T) {
	ghz, err := GHZState(4)
	if err != nil {
		t.Fatalf("GHZState(4) failed: %v", err)
	}

	if ghz.NumQubits() != 4 {
		t.Errorf("GHZ state qubits = %d, want 4", ghz.NumQubits())
	}

	if !ghz.IsNormalized(1e-10) {
		t.Error("GHZ state should be normalized")
	}
}

func TestSerialization(t *testing.T) {
	original, _ := BellState()

	// Serialize
	data, err := original.ToBytes()
	if err != nil {
		t.Fatalf("ToBytes() failed: %v", err)
	}

	// Deserialize
	restored, err := FromBytes(data)
	if err != nil {
		t.Fatalf("FromBytes() failed: %v", err)
	}

	// Check fidelity
	fid, _ := original.Fidelity(restored)
	if math.Abs(fid-1.0) > 1e-10 {
		t.Errorf("Fidelity after serialization = %v, want 1.0", fid)
	}
}

func TestFileIO(t *testing.T) {
	tmpDir := t.TempDir()
	path := filepath.Join(tmpDir, "test.qft")

	original, _ := BellState()
	original.SetMetadata("test", "value")

	// Save
	err := original.Save(path)
	if err != nil {
		t.Fatalf("Save() failed: %v", err)
	}

	// Load
	loaded, err := Load(path)
	if err != nil {
		t.Fatalf("Load() failed: %v", err)
	}

	// Check fidelity
	fid, _ := original.Fidelity(loaded)
	if math.Abs(fid-1.0) > 1e-10 {
		t.Errorf("Fidelity after file IO = %v, want 1.0", fid)
	}
}

func TestJSON(t *testing.T) {
	original, _ := BellState()
	original.SetMetadata("author", "test")

	// Export
	jsonStr, err := original.ToJSON()
	if err != nil {
		t.Fatalf("ToJSON() failed: %v", err)
	}

	// Import
	restored, err := FromJSON(jsonStr)
	if err != nil {
		t.Fatalf("FromJSON() failed: %v", err)
	}

	// Check fidelity
	fid, _ := original.Fidelity(restored)
	if math.Abs(fid-1.0) > 1e-10 {
		t.Errorf("Fidelity after JSON = %v, want 1.0", fid)
	}
}

func TestMetadata(t *testing.T) {
	file, _ := New(2)

	file.SetMetadata("key1", "value1")
	file.SetMetadata("key2", "value2")

	v1, ok := file.GetMetadata("key1")
	if !ok || v1 != "value1" {
		t.Errorf("GetMetadata(key1) = %v, %v, want value1, true", v1, ok)
	}

	_, ok = file.GetMetadata("nonexistent")
	if ok {
		t.Error("GetMetadata(nonexistent) should return false")
	}

	meta := file.Metadata()
	if len(meta) != 2 {
		t.Errorf("Metadata() length = %d, want 2", len(meta))
	}
}

func TestClone(t *testing.T) {
	original, _ := BellState()
	original.SetMetadata("test", "value")

	clone := original.Clone()

	// Modify original
	original.SetAmplitude(0, 0)

	// Clone should be unchanged
	amp, _ := clone.GetAmplitude(0)
	sqrt2Inv := 1.0 / math.Sqrt(2.0)
	if math.Abs(real(amp)-sqrt2Inv) > 1e-10 {
		t.Error("Clone should be independent of original")
	}
}

func TestVerify(t *testing.T) {
	bell, _ := BellState()
	data, _ := bell.ToBytes()

	if !Verify(data) {
		t.Error("Verify should return true for valid normalized state")
	}

	// Invalid data
	if Verify([]byte{1, 2, 3}) {
		t.Error("Verify should return false for invalid data")
	}
}

func BenchmarkNew(b *testing.B) {
	for i := 0; i < b.N; i++ {
		New(10)
	}
}

func BenchmarkFidelity(b *testing.B) {
	state1, _ := New(10)
	state2, _ := New(10)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		state1.Fidelity(state2)
	}
}

func BenchmarkNormalize(b *testing.B) {
	state, _ := New(10)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		state.Normalize()
	}
}

func BenchmarkSerialization(b *testing.B) {
	state, _ := New(10)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		data, _ := state.ToBytes()
		FromBytes(data)
	}
}
