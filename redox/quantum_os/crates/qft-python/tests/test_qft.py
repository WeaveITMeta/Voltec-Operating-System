"""Tests for qft Python SDK."""

import numpy as np
import pytest


def test_import():
    """Test that qft module can be imported."""
    import qft
    assert hasattr(qft, 'QftFile')
    assert hasattr(qft, 'load')
    assert hasattr(qft, 'save')
    assert hasattr(qft, 'version')


def test_version():
    """Test version string."""
    import qft
    assert qft.version() == "1.0.0"


def test_create_file():
    """Test creating a new QftFile."""
    import qft
    
    f = qft.QftFile(4)
    assert f.num_qubits == 4
    assert f.dimension == 16
    assert f.bond_dimension == 64
    assert f.golay_enabled == True


def test_invalid_qubits():
    """Test that invalid qubit counts raise errors."""
    import qft
    
    with pytest.raises(ValueError):
        qft.QftFile(0)
    
    with pytest.raises(ValueError):
        qft.QftFile(31)


def test_amplitudes():
    """Test getting and setting amplitudes."""
    import qft
    
    f = qft.QftFile(2)
    
    # Initial state is |00⟩
    real = f.amplitudes_real
    imag = f.amplitudes_imag
    
    assert len(real) == 4
    assert len(imag) == 4
    assert real[0] == 1.0
    assert real[1] == 0.0
    
    # Set to |11⟩
    new_real = np.array([0.0, 0.0, 0.0, 1.0])
    new_imag = np.zeros(4)
    f.set_amplitudes(new_real, new_imag)
    
    r, i = f.get_amplitude(3)
    assert r == 1.0
    assert i == 0.0


def test_normalization():
    """Test normalization check and normalize."""
    import qft
    
    f = qft.QftFile(2)
    
    # Set unnormalized state
    real = np.array([1.0, 1.0, 0.0, 0.0])
    imag = np.zeros(4)
    f.set_amplitudes(real, imag)
    
    assert not f.is_normalized()
    
    f.normalize()
    
    assert f.is_normalized()
    assert abs(f.norm_squared() - 1.0) < 1e-10


def test_fidelity():
    """Test fidelity calculation."""
    import qft
    
    f1 = qft.QftFile(2)
    f2 = qft.QftFile(2)
    
    # Same state should have fidelity 1
    fid = f1.fidelity(f2)
    assert abs(fid - 1.0) < 1e-10
    
    # Orthogonal states should have fidelity 0
    real = np.array([0.0, 1.0, 0.0, 0.0])
    imag = np.zeros(4)
    f2.set_amplitudes(real, imag)
    
    fid = f1.fidelity(f2)
    assert abs(fid) < 1e-10


def test_metadata():
    """Test metadata operations."""
    import qft
    
    f = qft.QftFile(2)
    
    f.set_metadata("author", "test")
    f.set_metadata("experiment", "bell_state")
    
    assert f.get_metadata("author") == "test"
    assert f.get_metadata("experiment") == "bell_state"
    assert f.get_metadata("nonexistent") is None
    
    meta = f.metadata_dict()
    assert meta["author"] == "test"


def test_save_load(tmp_path):
    """Test saving and loading files."""
    import qft
    
    # Create Bell state
    f = qft.QftFile(2)
    real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
    imag = np.zeros(4)
    f.set_amplitudes(real, imag)
    f.set_metadata("type", "bell")
    
    # Save
    path = tmp_path / "test.qft"
    f.save(str(path))
    
    # Load
    loaded = qft.load(str(path))
    
    assert loaded.num_qubits == 2
    assert loaded.fidelity(f) > 0.9999


def test_from_arrays():
    """Test creating from arrays."""
    import qft
    
    real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
    imag = np.zeros(4)
    
    f = qft.from_arrays(real, imag)
    
    assert f.num_qubits == 2
    assert f.is_normalized()


def test_from_numpy():
    """Test creating from interleaved numpy array."""
    import qft
    
    # Interleaved real/imag
    data = np.array([1/np.sqrt(2), 0, 0, 0, 0, 0, 1/np.sqrt(2), 0])
    
    f = qft.from_numpy(data)
    
    assert f.num_qubits == 2


def test_verify(tmp_path):
    """Test file verification."""
    import qft
    
    f = qft.QftFile(2)
    path = tmp_path / "test.qft"
    f.save(str(path))
    
    assert qft.verify(str(path))
    
    # Invalid file
    invalid_path = tmp_path / "invalid.qft"
    invalid_path.write_bytes(b"not a qft file")
    
    assert not qft.verify(str(invalid_path))


def test_streaming_converter():
    """Test streaming converter."""
    import qft
    
    total_size = 1024
    converter = qft.StreamingConverter(total_size, chunk_size=256)
    
    assert converter.chunk_size == 256
    assert converter.progress == 0.0
    assert not converter.is_complete
    
    # Process chunks
    data = bytes(range(256))
    state = converter.process_chunk(data)
    
    assert state.num_qubits > 0
    assert converter.progress > 0


def test_bell_state():
    """Test creating a proper Bell state."""
    import qft
    
    f = qft.QftFile(2)
    
    # |Φ+⟩ = (|00⟩ + |11⟩) / √2
    real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
    imag = np.zeros(4)
    f.set_amplitudes(real, imag)
    
    assert f.is_normalized()
    
    # Check probabilities
    probs = real**2 + imag**2
    assert abs(probs[0] - 0.5) < 1e-10
    assert abs(probs[3] - 0.5) < 1e-10


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
