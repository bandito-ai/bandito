/// Bayesian linear algebra for shared Linear Thompson Sampling.
///
/// Mathematical core:
///     - Bayesian linear regression: maintains precision matrix A and accumulator b
///     - Posterior: theta = A^{-1} b, covariance = A^{-1}
///     - Thompson Sampling: sample theta_tilde ~ N(theta, beta^2 * A^{-1})
///     - Arm scoring: score_a = x_a^T * theta_tilde
///
/// All functions operate on raw f64 slices (flattened row-major for matrices).
use crate::rng::Xoshiro256StarStar;

/// Draw one sample from the Thompson Sampling posterior.
///
/// theta_tilde = theta + beta * L * epsilon
/// where epsilon ~ N(0, I) and L is the Cholesky factor of the covariance.
pub fn sample_thompson(
    theta: &[f64],
    chol: &[f64],
    d: usize,
    beta: f64,
    rng: &mut Xoshiro256StarStar,
) -> Vec<f64> {
    let mut epsilon = vec![0.0; d];
    rng.fill_normal(&mut epsilon);

    // theta_tilde = theta + beta * chol @ epsilon
    let mut result = vec![0.0; d];
    for i in 0..d {
        let mut dot = 0.0;
        for j in 0..d {
            dot += chol[i * d + j] * epsilon[j];
        }
        result[i] = theta[i] + beta * dot;
    }
    result
}

/// Matrix-vector multiply: mat (rows x cols) @ vec (cols) -> result (rows).
/// mat is flattened row-major.
pub fn matvec(mat: &[f64], vec: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let mut result = std::vec![0.0; rows];
    for i in 0..rows {
        let mut dot = 0.0;
        for j in 0..cols {
            dot += mat[i * cols + j] * vec[j];
        }
        result[i] = dot;
    }
    result
}

/// Full Bayesian update: incorporate a new observation.
///
/// A_new = A + x x^T
/// b_new = b + x * reward
///
/// `a` and `b` are modified in place. `a` is flattened row-major d x d.
pub fn bayesian_update_full(a: &mut [f64], b: &mut [f64], x: &[f64], reward: f64, d: usize) {
    // A += outer(x, x)
    for i in 0..d {
        for j in 0..d {
            a[i * d + j] += x[i] * x[j];
        }
        // b += x * reward
        b[i] += x[i] * reward;
    }
}

/// Delta update to reward accumulator (A unchanged).
///
/// b_new = b + x * (new_reward - old_reward)
pub fn bayesian_update_delta(
    b: &mut [f64],
    x: &[f64],
    new_reward: f64,
    old_reward: f64,
    d: usize,
) {
    let delta = new_reward - old_reward;
    for i in 0..d {
        b[i] += x[i] * delta;
    }
}

/// Compute posterior mean and Cholesky factor from precision matrix.
///
/// theta = A^{-1} b
/// chol = cholesky(A^{-1} + jitter * I)
///
/// Returns (theta, chol) where chol is flattened row-major d x d.
pub fn compute_posterior(a: &[f64], b: &[f64], d: usize, jitter: f64) -> (Vec<f64>, Vec<f64>) {
    // Compute A^{-1} via Cholesky of A, then solve
    let a_chol = cholesky(a, d).expect("A must be positive definite for posterior computation");

    // theta = A^{-1} b via forward/back substitution on A's Cholesky
    let theta = cholesky_solve(&a_chol, b, d);

    // A_inv = A^{-1} via solving A * X = I column by column
    let mut a_inv = vec![0.0; d * d];
    for col in 0..d {
        let mut e = vec![0.0; d];
        e[col] = 1.0;
        let x = cholesky_solve(&a_chol, &e, d);
        for row in 0..d {
            a_inv[row * d + col] = x[row];
        }
    }

    // chol = cholesky(A_inv + jitter * I)
    let chol = safe_cholesky(&a_inv, d, jitter);

    (theta, chol)
}

/// Cholesky decomposition of a symmetric positive-definite matrix.
///
/// Returns lower-triangular L such that L * L^T = M.
/// M is flattened row-major d x d.
fn cholesky(m: &[f64], d: usize) -> Result<Vec<f64>, String> {
    let mut l = vec![0.0; d * d];

    for i in 0..d {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i * d + k] * l[j * d + k];
            }

            if i == j {
                let val = m[i * d + i] - sum;
                if val <= 0.0 {
                    return Err(format!(
                        "Matrix not positive definite at index {}: diagonal value {}",
                        i, val
                    ));
                }
                l[i * d + j] = val.sqrt();
            } else {
                l[i * d + j] = (m[i * d + j] - sum) / l[j * d + j];
            }
        }
    }

    Ok(l)
}

/// Cholesky decomposition with static diagonal jitter.
///
/// Computes cholesky(M + jitter * I).
pub fn safe_cholesky(m: &[f64], d: usize, jitter: f64) -> Vec<f64> {
    let mut jittered = m.to_vec();
    for i in 0..d {
        jittered[i * d + i] += jitter;
    }
    cholesky(&jittered, d).expect("Cholesky failed even with jitter — matrix is fundamentally broken")
}

/// Solve L * L^T * x = b where L is lower-triangular (from Cholesky).
///
/// Forward substitution: L * y = b
/// Back substitution: L^T * x = y
fn cholesky_solve(l: &[f64], b: &[f64], d: usize) -> Vec<f64> {
    // Forward substitution: L * y = b
    let mut y = vec![0.0; d];
    for i in 0..d {
        let mut sum = 0.0;
        for j in 0..i {
            sum += l[i * d + j] * y[j];
        }
        y[i] = (b[i] - sum) / l[i * d + i];
    }

    // Back substitution: L^T * x = y
    let mut x = vec![0.0; d];
    for i in (0..d).rev() {
        let mut sum = 0.0;
        for j in (i + 1)..d {
            sum += l[j * d + i] * x[j]; // L^T[i][j] = L[j][i]
        }
        x[i] = (y[i] - sum) / l[i * d + i];
    }

    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::CHOLESKY_JITTER;

    #[test]
    fn test_sample_thompson_shape() {
        let d = 4;
        let theta = vec![0.1, 0.2, 0.3, 0.4];
        // Identity matrix as Cholesky factor
        let mut chol = vec![0.0; d * d];
        for i in 0..d {
            chol[i * d + i] = 1.0;
        }
        let mut rng = Xoshiro256StarStar::new(42);
        let result = sample_thompson(&theta, &chol, d, 1.0, &mut rng);
        assert_eq!(result.len(), d);
    }

    #[test]
    fn test_sample_thompson_deterministic() {
        let d = 4;
        let theta = vec![0.1, 0.2, 0.3, 0.4];
        let mut chol = vec![0.0; d * d];
        for i in 0..d {
            chol[i * d + i] = 1.0;
        }

        let mut rng1 = Xoshiro256StarStar::new(42);
        let mut rng2 = Xoshiro256StarStar::new(42);
        let r1 = sample_thompson(&theta, &chol, d, 1.0, &mut rng1);
        let r2 = sample_thompson(&theta, &chol, d, 1.0, &mut rng2);
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_sample_thompson_beta_scaling() {
        let d = 4;
        let theta = vec![0.0; d];
        let mut chol = vec![0.0; d * d];
        for i in 0..d {
            chol[i * d + i] = 1.0;
        }

        // With zero theta, the result magnitude scales with beta
        let mut rng = Xoshiro256StarStar::new(42);
        let r_low = sample_thompson(&theta, &chol, d, 0.1, &mut rng);
        let mut rng = Xoshiro256StarStar::new(42);
        let r_high = sample_thompson(&theta, &chol, d, 10.0, &mut rng);

        let mag_low: f64 = r_low.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mag_high: f64 = r_high.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!(mag_high > mag_low);
    }

    #[test]
    fn test_matvec() {
        // 2x3 matrix [[1,2,3],[4,5,6]] @ [1,1,1] = [6, 15]
        let mat = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let v = vec![1.0, 1.0, 1.0];
        let result = matvec(&mat, &v, 2, 3);
        assert_eq!(result, vec![6.0, 15.0]);
    }

    #[test]
    fn test_matvec_identity() {
        let d = 3;
        let mut mat = vec![0.0; d * d];
        for i in 0..d {
            mat[i * d + i] = 1.0;
        }
        let v = vec![3.0, 5.0, 7.0];
        let result = matvec(&mat, &v, d, d);
        assert_eq!(result, v);
    }

    #[test]
    fn test_bayesian_update_full() {
        let d = 3;
        let mut a = vec![0.0; d * d];
        // Start with identity
        for i in 0..d {
            a[i * d + i] = 1.0;
        }
        let mut b = vec![0.0; d];
        let x = vec![1.0, 0.0, 1.0];
        let reward = 0.8;

        bayesian_update_full(&mut a, &mut b, &x, reward, d);

        // A should be I + outer(x, x) = [[2,0,1],[0,1,0],[1,0,2]]
        assert_eq!(a[0 * d + 0], 2.0);
        assert_eq!(a[0 * d + 2], 1.0);
        assert_eq!(a[1 * d + 1], 1.0);
        assert_eq!(a[2 * d + 0], 1.0);
        assert_eq!(a[2 * d + 2], 2.0);

        // b should be [0.8, 0.0, 0.8]
        assert!((b[0] - 0.8).abs() < 1e-10);
        assert!((b[1] - 0.0).abs() < 1e-10);
        assert!((b[2] - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_bayesian_update_delta() {
        let d = 3;
        let mut b = vec![0.8, 0.0, 0.8];
        let x = vec![1.0, 0.0, 1.0];

        bayesian_update_delta(&mut b, &x, 0.5, 0.8, d);

        // b += x * (0.5 - 0.8) = x * (-0.3)
        assert!((b[0] - 0.5).abs() < 1e-10);
        assert!((b[1] - 0.0).abs() < 1e-10);
        assert!((b[2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cholesky_identity() {
        let d = 3;
        let mut m = vec![0.0; d * d];
        for i in 0..d {
            m[i * d + i] = 1.0;
        }
        let l = cholesky(&m, d).unwrap();
        // Cholesky of identity is identity
        for i in 0..d {
            for j in 0..d {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((l[i * d + j] - expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_safe_cholesky() {
        let d = 3;
        let mut m = vec![0.0; d * d];
        for i in 0..d {
            m[i * d + i] = 1.0;
        }
        let l = safe_cholesky(&m, d, CHOLESKY_JITTER);
        // Should be close to identity (jitter is tiny)
        for i in 0..d {
            assert!((l[i * d + i] - (1.0_f64 + CHOLESKY_JITTER).sqrt()).abs() < 1e-5);
        }
    }

    #[test]
    fn test_compute_posterior() {
        let d = 2;
        // A = [[2, 0], [0, 2]]
        let a = vec![2.0, 0.0, 0.0, 2.0];
        // b = [1, 2]
        let b = vec![1.0, 2.0];

        let (theta, chol) = compute_posterior(&a, &b, d, CHOLESKY_JITTER);

        // theta = A^{-1} b = [[0.5, 0], [0, 0.5]] @ [1, 2] = [0.5, 1.0]
        assert!((theta[0] - 0.5).abs() < 1e-10);
        assert!((theta[1] - 1.0).abs() < 1e-10);

        // A^{-1} = [[0.5, 0], [0, 0.5]]
        // chol should be cholesky of that + jitter*I
        assert!(chol.len() == d * d);
        // Diagonal should be sqrt(0.5 + jitter) ≈ 0.7071
        assert!((chol[0] - (0.5_f64 + CHOLESKY_JITTER).sqrt()).abs() < 1e-5);
    }

    #[test]
    fn test_cholesky_solve_roundtrip() {
        let d = 3;
        // Symmetric positive definite matrix
        let m = vec![4.0, 2.0, 1.0, 2.0, 5.0, 3.0, 1.0, 3.0, 6.0];
        let l = cholesky(&m, d).unwrap();
        let b = vec![1.0, 2.0, 3.0];
        let x = cholesky_solve(&l, &b, d);

        // Verify M @ x ≈ b
        let result = matvec(&m, &x, d, d);
        for i in 0..d {
            assert!(
                (result[i] - b[i]).abs() < 1e-10,
                "Mismatch at {}: {} vs {}",
                i,
                result[i],
                b[i]
            );
        }
    }
}
