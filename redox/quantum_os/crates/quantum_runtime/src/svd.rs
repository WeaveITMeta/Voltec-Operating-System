// =============================================================================
// MACROHARD Quantum OS - Singular Value Decomposition Module
// =============================================================================
// Table of Contents:
//   1. SvdResult - Result structure for SVD operations
//   2. Power iteration SVD (fallback, always available)
//   3. Production SVD via ndarray-linalg (optional, feature-gated)
//   4. Truncated SVD for MPS bond dimension control
// =============================================================================
// Purpose: Provides SVD implementations for MPS tensor decomposition.
//          Power iteration for portability, ndarray-linalg for production.
// =============================================================================

use ndarray::{Array1, Array2};
use num_complex::Complex64;

/// Result of a singular value decomposition A = U * S * V^H
#[derive(Debug, Clone)]
pub struct SvdResult {
    /// Left singular vectors (m x k matrix)
    pub u: Array2<Complex64>,
    /// Singular values (k vector, sorted descending)
    pub s: Array1<f64>,
    /// Right singular vectors (k x n matrix, already conjugate-transposed)
    pub vh: Array2<Complex64>,
}

impl SvdResult {
    /// Number of singular values retained
    pub fn rank(&self) -> usize {
        self.s.len()
    }

    /// Reconstruct the original matrix (or truncated approximation)
    pub fn reconstruct(&self) -> Array2<Complex64> {
        let mut result = Array2::zeros((self.u.nrows(), self.vh.ncols()));
        for k in 0..self.rank() {
            let s_k = Complex64::new(self.s[k], 0.0);
            for i in 0..self.u.nrows() {
                for j in 0..self.vh.ncols() {
                    result[[i, j]] += self.u[[i, k]] * s_k * self.vh[[k, j]];
                }
            }
        }
        result
    }

    /// Frobenius norm of the truncation error
    pub fn truncation_error(&self, original: &Array2<Complex64>) -> f64 {
        let reconstructed = self.reconstruct();
        let mut error = 0.0;
        for i in 0..original.nrows() {
            for j in 0..original.ncols() {
                let diff = original[[i, j]] - reconstructed[[i, j]];
                error += diff.norm_sqr();
            }
        }
        error.sqrt()
    }
}

// =============================================================================
// 2. Power Iteration SVD (Fallback Implementation)
// =============================================================================

/// Compute truncated SVD using power iteration method.
/// This is a portable fallback that doesn't require external BLAS libraries.
/// 
/// # Arguments
/// * `matrix` - Input matrix to decompose
/// * `max_rank` - Maximum number of singular values to compute
/// * `tolerance` - Convergence tolerance for power iteration
/// * `max_iterations` - Maximum iterations per singular value
/// 
/// # Returns
/// SvdResult containing U, S, V^H truncated to at most max_rank components
pub fn svd_power_iteration(
    matrix: &Array2<Complex64>,
    max_rank: usize,
    tolerance: f64,
    max_iterations: usize,
) -> SvdResult {
    let (m, n) = (matrix.nrows(), matrix.ncols());
    let rank = max_rank.min(m).min(n);
    
    let mut u_vecs: Vec<Array1<Complex64>> = Vec::with_capacity(rank);
    let mut s_vals: Vec<f64> = Vec::with_capacity(rank);
    let mut v_vecs: Vec<Array1<Complex64>> = Vec::with_capacity(rank);
    
    // Work with a deflated copy of the matrix
    let mut residual = matrix.clone();
    
    for _ in 0..rank {
        // Power iteration for largest singular value of residual
        let (u, s, v) = power_iteration_single(&residual, tolerance, max_iterations);
        
        if s < tolerance {
            break; // Remaining singular values are negligible
        }
        
        // Deflate: residual = residual - s * u * v^H
        for i in 0..m {
            for j in 0..n {
                residual[[i, j]] -= Complex64::new(s, 0.0) * u[i] * v[j].conj();
            }
        }
        
        u_vecs.push(u);
        s_vals.push(s);
        v_vecs.push(v);
    }
    
    let k = s_vals.len();
    if k == 0 {
        return SvdResult {
            u: Array2::zeros((m, 1)),
            s: Array1::zeros(1),
            vh: Array2::zeros((1, n)),
        };
    }
    
    // Assemble result matrices
    let mut u = Array2::zeros((m, k));
    let mut vh = Array2::zeros((k, n));
    
    for (idx, (u_vec, v_vec)) in u_vecs.iter().zip(v_vecs.iter()).enumerate() {
        for i in 0..m {
            u[[i, idx]] = u_vec[i];
        }
        for j in 0..n {
            vh[[idx, j]] = v_vec[j].conj();
        }
    }
    
    SvdResult {
        u,
        s: Array1::from_vec(s_vals),
        vh,
    }
}

/// Single power iteration to find the largest singular value/vectors
fn power_iteration_single(
    matrix: &Array2<Complex64>,
    tolerance: f64,
    max_iterations: usize,
) -> (Array1<Complex64>, f64, Array1<Complex64>) {
    let (m, n) = (matrix.nrows(), matrix.ncols());
    
    // Initialize with random vector
    let mut v: Array1<Complex64> = Array1::from_iter(
        (0..n).map(|i| Complex64::new(((i * 7 + 13) % 100) as f64 / 100.0, 0.0))
    );
    normalize_vector(&mut v);
    
    let mut sigma = 0.0;
    
    for _ in 0..max_iterations {
        // u = A * v
        let mut u = Array1::zeros(m);
        for i in 0..m {
            for j in 0..n {
                u[i] += matrix[[i, j]] * v[j];
            }
        }
        
        let new_sigma = normalize_vector(&mut u);
        
        // v = A^H * u
        let mut new_v = Array1::zeros(n);
        for j in 0..n {
            for i in 0..m {
                new_v[j] += matrix[[i, j]].conj() * u[i];
            }
        }
        normalize_vector(&mut new_v);
        
        // Check convergence
        if (new_sigma - sigma).abs() < tolerance * sigma.max(1.0) {
            return (u, new_sigma, new_v);
        }
        
        sigma = new_sigma;
        v = new_v;
    }
    
    // Final computation
    let mut u = Array1::zeros(m);
    for i in 0..m {
        for j in 0..n {
            u[i] += matrix[[i, j]] * v[j];
        }
    }
    let final_sigma = normalize_vector(&mut u);
    
    (u, final_sigma, v)
}

/// Normalize a complex vector in-place, return the original norm
fn normalize_vector(v: &mut Array1<Complex64>) -> f64 {
    let norm: f64 = v.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
    if norm > 1e-15 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    norm
}

// =============================================================================
// 3. Production SVD via ndarray-linalg (Feature-Gated)
// =============================================================================

#[cfg(feature = "production_svd")]
pub mod production {
    use super::*;
    use ndarray_linalg::SVD;
    
    /// Compute full SVD using LAPACK via ndarray-linalg.
    /// This is the production-quality implementation with O(mn·min(m,n)) complexity.
    /// 
    /// # Arguments
    /// * `matrix` - Input matrix to decompose
    /// 
    /// # Returns
    /// Full SvdResult containing all singular values and vectors
    pub fn svd_full(matrix: &Array2<Complex64>) -> Result<SvdResult, String> {
        let (u_opt, s, vh_opt) = matrix.svd(true, true)
            .map_err(|e| format!("SVD failed: {:?}", e))?;
        
        let u = u_opt.ok_or("SVD did not return U matrix")?;
        let vh = vh_opt.ok_or("SVD did not return V^H matrix")?;
        
        Ok(SvdResult { u, s, vh })
    }
    
    /// Compute truncated SVD using LAPACK, keeping only top-k singular values.
    /// 
    /// # Arguments
    /// * `matrix` - Input matrix to decompose
    /// * `max_rank` - Maximum number of singular values to retain
    /// * `threshold` - Minimum singular value to retain (relative to largest)
    /// 
    /// # Returns
    /// Truncated SvdResult
    pub fn svd_truncated(
        matrix: &Array2<Complex64>,
        max_rank: usize,
        threshold: f64,
    ) -> Result<SvdResult, String> {
        let full = svd_full(matrix)?;
        
        // Determine truncation point
        let s_max = full.s[0];
        let mut k = 0;
        for (i, &s_i) in full.s.iter().enumerate() {
            if i >= max_rank || s_i < threshold * s_max {
                break;
            }
            k = i + 1;
        }
        k = k.max(1); // Keep at least one singular value
        
        // Truncate matrices
        let u = full.u.slice(ndarray::s![.., ..k]).to_owned();
        let s = full.s.slice(ndarray::s![..k]).to_owned();
        let vh = full.vh.slice(ndarray::s![..k, ..]).to_owned();
        
        Ok(SvdResult { u, s, vh })
    }
}

// =============================================================================
// 4. Truncated SVD for MPS (Unified Interface)
// =============================================================================

/// Configuration for truncated SVD in MPS operations
#[derive(Debug, Clone)]
pub struct TruncatedSvdConfig {
    /// Maximum bond dimension (rank) to retain
    pub max_bond_dim: usize,
    /// Minimum singular value to retain (relative to largest)
    pub truncation_threshold: f64,
    /// Maximum iterations for power iteration (fallback only)
    pub max_iterations: usize,
}

impl Default for TruncatedSvdConfig {
    fn default() -> Self {
        Self {
            max_bond_dim: 64,
            truncation_threshold: 1e-10,
            max_iterations: 100,
        }
    }
}

impl TruncatedSvdConfig {
    pub fn new(max_bond_dim: usize, truncation_threshold: f64) -> Self {
        Self {
            max_bond_dim,
            truncation_threshold,
            max_iterations: 100,
        }
    }
    
    pub fn high_accuracy() -> Self {
        Self {
            max_bond_dim: 256,
            truncation_threshold: 1e-14,
            max_iterations: 200,
        }
    }
}

/// Compute truncated SVD using the best available implementation.
/// Uses ndarray-linalg if `production_svd` feature is enabled, otherwise falls back to power iteration.
/// 
/// # Arguments
/// * `matrix` - Input matrix to decompose
/// * `config` - Truncation configuration
/// 
/// # Returns
/// Truncated SvdResult
pub fn truncated_svd(matrix: &Array2<Complex64>, config: &TruncatedSvdConfig) -> SvdResult {
    #[cfg(feature = "production_svd")]
    {
        match production::svd_truncated(matrix, config.max_bond_dim, config.truncation_threshold) {
            Ok(result) => return result,
            Err(e) => {
                tracing::warn!("Production SVD failed, falling back to power iteration: {}", e);
            }
        }
    }
    
    // Fallback to power iteration
    svd_power_iteration(
        matrix,
        config.max_bond_dim,
        config.truncation_threshold,
        config.max_iterations,
    )
}

/// Compute truncated SVD and return U * sqrt(S) and sqrt(S) * V^H for MPS splitting.
/// This is the typical form needed when splitting an MPS tensor.
pub fn svd_for_mps_split(
    matrix: &Array2<Complex64>,
    config: &TruncatedSvdConfig,
) -> (Array2<Complex64>, Array2<Complex64>, f64) {
    let svd = truncated_svd(matrix, config);
    
    // Compute truncation error (sum of discarded singular values squared)
    let total_norm_sq: f64 = svd.s.iter().map(|s| s * s).sum();
    
    // U_left = U * sqrt(S)
    let mut u_left = svd.u.clone();
    for k in 0..svd.rank() {
        let sqrt_s = svd.s[k].sqrt();
        for i in 0..u_left.nrows() {
            u_left[[i, k]] *= sqrt_s;
        }
    }
    
    // V_right = sqrt(S) * V^H
    let mut v_right = svd.vh.clone();
    for k in 0..svd.rank() {
        let sqrt_s = svd.s[k].sqrt();
        for j in 0..v_right.ncols() {
            v_right[[k, j]] *= sqrt_s;
        }
    }
    
    (u_left, v_right, total_norm_sq.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_svd_identity() {
        let eye: Array2<Complex64> = Array2::from_diag(&Array1::from_vec(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]));
        
        let config = TruncatedSvdConfig::default();
        let result = truncated_svd(&eye, &config);
        
        assert_eq!(result.rank(), 2);
        assert!((result.s[0] - 1.0).abs() < 1e-10);
        assert!((result.s[1] - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_svd_rank_1() {
        // Rank-1 matrix: outer product of [1, 0] and [1, 1]
        let mut matrix = Array2::zeros((2, 2));
        matrix[[0, 0]] = Complex64::new(1.0, 0.0);
        matrix[[0, 1]] = Complex64::new(1.0, 0.0);
        
        let config = TruncatedSvdConfig::new(2, 1e-10);
        let result = truncated_svd(&matrix, &config);
        
        // Should have only 1 significant singular value
        assert!(result.s[0] > 1e-10);
        if result.rank() > 1 {
            assert!(result.s[1] < 1e-10);
        }
    }
    
    #[test]
    fn test_svd_reconstruction() {
        let mut matrix = Array2::zeros((3, 2));
        matrix[[0, 0]] = Complex64::new(1.0, 0.0);
        matrix[[1, 1]] = Complex64::new(2.0, 0.0);
        matrix[[2, 0]] = Complex64::new(0.5, 0.0);
        
        let config = TruncatedSvdConfig::default();
        let result = truncated_svd(&matrix, &config);
        let reconstructed = result.reconstruct();
        
        let error = result.truncation_error(&matrix);
        assert!(error < 1e-10, "Reconstruction error too large: {}", error);
    }
}
