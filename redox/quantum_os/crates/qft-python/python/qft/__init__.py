"""
qft - Quantum File Type Python SDK

Production-grade Python bindings for the Quantum File Type (.qft) format.

Example:
    >>> import qft
    >>> import numpy as np
    >>> 
    >>> # Create a Bell state
    >>> f = qft.QftFile(2)
    >>> real = np.array([1/np.sqrt(2), 0, 0, 1/np.sqrt(2)])
    >>> imag = np.zeros(4)
    >>> f.set_amplitudes(real, imag)
    >>> f.save("bell.qft")
"""

from .qft import (
    QftFile,
    StreamingConverter,
    Encoding,
    load,
    save,
    from_numpy,
    from_arrays,
    verify,
    version,
    from_qiskit,
    to_qiskit,
)

__all__ = [
    "QftFile",
    "StreamingConverter", 
    "Encoding",
    "load",
    "save",
    "from_numpy",
    "from_arrays",
    "verify",
    "version",
    "from_qiskit",
    "to_qiskit",
]

__version__ = version()
