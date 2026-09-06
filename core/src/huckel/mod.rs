//! Hückel / Extended Hückel LCAO engine (PLAN.md § 6.1).
//!
//! The generalized symmetric eigenproblem `H C = e S C` is reduced through a
//! Cholesky factorization of the overlap matrix `S = L L^T` to the standard
//! problem `A y = e y` with `A = L^-1 H L^-T`, solved by cyclic Jacobi
//! rotations (deterministic sweep order — PLAN.md § 13.4/§ 8.5).
//!
//! No physical constant lives here: `alpha`, `beta` and every later
//! Extended-Hückel parameter enter as provenance-tracked [`Value`]s.

use crate::value::Value;
use thiserror::Error;

/// Errors of the Hückel engine.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum HuckelError {
    #[error("dimension mismatch: h {h} x {h}, s {s} x {s}")]
    DimensionMismatch { h: usize, s: usize },
    #[error("overlap matrix is not positive definite (basis linearly dependent)")]
    NotPositiveDefinite,
    #[error("invalid electron count {electrons} for {orbitals} orbitals")]
    InvalidElectronCount { electrons: usize, orbitals: usize },
}

/// Dense row-major matrix.
pub type Matrix = Vec<Vec<f64>>;

/// Cholesky factorization `S = L L^T` (lower triangular L).
fn cholesky(s: &Matrix) -> Result<Matrix, HuckelError> {
    let n = s.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum: f64 = s[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                if sum <= 0.0 {
                    return Err(HuckelError::NotPositiveDefinite);
                }
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    Ok(l)
}

/// Solve `L x = b` for lower-triangular L, writing the result over `b`.
fn forward_substitute_in_place(l: &Matrix, b: &mut [f64]) {
    let n = l.len();
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i][k] * b[k];
        }
        b[i] = sum / l[i][i];
    }
}

/// HOMO/LUMO indices for `electrons` in `orbitals` spatial orbitals.
fn occupation_edges(electrons: usize, orbitals: usize) -> (Option<usize>, Option<usize>) {
    let homo = if electrons == 0 {
        None
    } else {
        Some(electrons.div_ceil(2) - 1)
    };
    let lumo = if electrons >= 2 * orbitals {
        None
    } else {
        Some(electrons.div_ceil(2))
    };
    (homo, lumo)
}

/// Cyclic Jacobi eigensolver for symmetric matrices.
/// Returns eigenvalues ascending with matching eigenvectors (columns).
fn jacobi_eigen(a: &Matrix) -> (Vec<f64>, Matrix) {
    let n = a.len();
    let mut a = a.clone();
    let mut v = vec![vec![0.0; n]; n];
    for i in 0..n {
        v[i][i] = 1.0;
    }
    for _sweep in 0..100 {
        let mut off = 0.0f64;
        for i in 0..n {
            for j in i + 1..n {
                off += a[i][j] * a[i][j];
            }
        }
        if off <= 1e-24 * (1.0 + trace_norm(&a)) {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p][q];
                if apq.abs() <= 1e-300 {
                    continue;
                }
                let app = a[p][p];
                let aqq = a[q][q];
                let theta = -0.5 * (2.0 * apq).atan2(app - aqq);
                let (sin, cos) = theta.sin_cos();
                rotate(&mut a, &mut v, p, q, sin, cos);
            }
        }
    }
    let mut eigen: Vec<(f64, usize)> = (0..n).map(|i| (a[i][i], i)).collect();
    eigen.sort_by(|x, y| x.0.total_cmp(&y.0));
    let values: Vec<f64> = eigen.iter().map(|(v, _)| *v).collect();
    let mut vectors = vec![vec![0.0; n]; n];
    for (new_col, (_, old_col)) in eigen.iter().enumerate() {
        for row in 0..n {
            vectors[row][new_col] = v[row][*old_col];
        }
    }
    canonicalize_signs(&mut vectors);
    (values, vectors)
}

fn trace_norm(a: &Matrix) -> f64 {
    a.iter().enumerate().map(|(i, row)| row[i].abs()).sum()
}

fn rotate(a: &mut Matrix, v: &mut Matrix, p: usize, q: usize, sin: f64, cos: f64) {
    let n = a.len();
    let app = a[p][p];
    let aqq = a[q][q];
    let apq = a[p][q];
    a[p][p] = cos * cos * app - 2.0 * sin * cos * apq + sin * sin * aqq;
    a[q][q] = sin * sin * app + 2.0 * sin * cos * apq + cos * cos * aqq;
    let apq_new = sin * cos * (app - aqq) + (cos * cos - sin * sin) * apq;
    a[p][q] = apq_new;
    a[q][p] = apq_new;
    for k in 0..n {
        if k != p && k != q {
            let akp = a[k][p];
            let akq = a[k][q];
            a[k][p] = cos * akp - sin * akq;
            a[p][k] = a[k][p];
            a[k][q] = sin * akp + cos * akq;
            a[q][k] = a[k][q];
        }
    }
    for k in 0..n {
        let vkp = v[k][p];
        let vkq = v[k][q];
        v[k][p] = cos * vkp - sin * vkq;
        v[k][q] = sin * vkp + cos * vkq;
    }
}

/// Deterministic sign convention: the largest-|coefficient| entry of each
/// MO is positive.
fn canonicalize_signs(v: &mut Matrix) {
    let n = v.len();
    for col in 0..n {
        let mut best = 0usize;
        for row in 1..n {
            if v[row][col].abs() > v[best][col].abs() {
                best = row;
            }
        }
        if v[best][col] < 0.0 {
            for row in 0..n {
                v[row][col] = -v[row][col];
            }
        }
    }
}

/// Solution of a Hückel problem.
#[derive(Debug, Clone)]
pub struct HuckelSolution {
    /// Orbital energies in ascending order, provenance-tracked.
    pub energies: Vec<Value>,
    /// MO coefficients: `coefficients[orbital][atom]`, S-orthonormal.
    pub coefficients: Vec<Vec<f64>>,
    /// Number of electrons placed.
    pub electrons: usize,
    /// Index of the highest occupied orbital (HOMO), if any electron.
    pub homo: Option<usize>,
    /// Index of the lowest unoccupied orbital (LUMO), if not full.
    pub lumo: Option<usize>,
}

impl HuckelSolution {
    /// HOMO-LUMO gap in the energy units of the inputs.
    #[must_use]
    pub fn gap(&self) -> Option<f64> {
        let (h, l) = (self.homo?, self.lumo?);
        Some(self.energies[l].value - self.energies[h].value)
    }

    /// Total energy of the placed electrons.
    #[must_use]
    pub fn total_energy(&self) -> f64 {
        let mut total = 0.0;
        for (i, e) in self.energies.iter().enumerate() {
            let occ = if i < self.electrons / 2 {
                2.0
            } else if i == self.electrons / 2 && self.electrons % 2 == 1 {
                1.0
            } else {
                0.0
            };
            total += occ * e.value;
        }
        total
    }
}

/// Simple Hückel model (pi system): `H = alpha I + beta A` over the bond
/// adjacency, `S = I`. `alpha`/`beta` carry provenance (PLAN.md § 13.3);
/// each eigenvalue is `alpha + k * beta` with `k` the dimensionless
/// adjacency eigenvalue, derived through [`Value::derive`] so uncertainty
/// propagates from the parameters.
pub fn simple_huckel(
    n_atoms: usize,
    bonds: &[(usize, usize)],
    alpha: &Value,
    beta: &Value,
    electrons: usize,
) -> Result<HuckelSolution, HuckelError> {
    if electrons > 2 * n_atoms {
        return Err(HuckelError::InvalidElectronCount {
            electrons,
            orbitals: n_atoms,
        });
    }
    let mut adjacency = vec![vec![0.0; n_atoms]; n_atoms];
    for &(i, j) in bonds {
        adjacency[i][j] = 1.0;
        adjacency[j][i] = 1.0;
    }
    let (k_values, vectors) = jacobi_eigen(&adjacency);
    // order orbitals by energy alpha + k*beta (sign of beta decides the order)
    let mut orbitals: Vec<(f64, f64, usize)> = k_values
        .iter()
        .enumerate()
        .map(|(idx, &k)| (alpha.value + k * beta.value, k, idx))
        .collect();
    orbitals.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    let (homo, lumo) = occupation_edges(electrons, n_atoms);
    let energies = orbitals
        .iter()
        .map(|&(_, k, _)| {
            Value::derive(
                &|x| x[0] + k * x[1],
                &[alpha, beta],
                "simple-huckel/linear-in-alpha-beta",
            )
            .expect("linear derivation of an eigenvalue cannot fail for finite inputs")
        })
        .collect();
    let coefficients = orbitals
        .iter()
        .map(|&(_, _, idx)| column(&vectors, idx))
        .collect();
    Ok(HuckelSolution {
        energies,
        coefficients,
        electrons,
        homo,
        lumo,
    })
}

/// Solve a generalized problem `H C = e S C` directly (Extended Hückel path).
pub fn solve_generalized(
    h: &Matrix,
    s: &Matrix,
    electrons: usize,
    provenance: &dyn Fn(f64) -> Value,
) -> Result<HuckelSolution, HuckelError> {
    let n = h.len();
    if s.len() != n || s.iter().any(|row| row.len() != n) {
        return Err(HuckelError::DimensionMismatch { h: n, s: s.len() });
    }
    if electrons > 2 * n {
        return Err(HuckelError::InvalidElectronCount {
            electrons,
            orbitals: n,
        });
    }
    let l = cholesky(s)?;
    // A = L^-1 H L^-T in O(n^3): B = L^-1 H, then A = B L^-T.
    let b: Matrix = (0..n)
        .map(|j| {
            let mut col = column(h, j);
            forward_substitute_in_place(&l, &mut col);
            col
        })
        .collect();
    // rows of L^-1 (i.e. columns of L^-T) via forward substitution on unit vectors
    let l_inv_rows: Vec<Vec<f64>> = (0..n)
        .map(|j| {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            forward_substitute_in_place(&l, &mut e);
            e
        })
        .collect();
    let a: Matrix = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| (0..n).map(|p| b[i][p] * l_inv_rows[j][p]).sum())
                .collect()
        })
        .collect();
    let (values, y) = jacobi_eigen(&a);
    // C = L^-T Y
    let mut coefficients = Vec::with_capacity(n);
    for col in 0..n {
        let c = back_substitute(&l, &column(&y, col));
        coefficients.push(c);
    }
    canonicalize_signs_rows(&mut coefficients);
    let (homo, lumo) = occupation_edges(electrons, n);
    Ok(HuckelSolution {
        energies: values.iter().map(|&e| provenance(e)).collect(),
        coefficients,
        electrons,
        homo,
        lumo,
    })
}

fn column(m: &Matrix, col: usize) -> Vec<f64> {
    m.iter().map(|row| row[col]).collect()
}

/// Solve L^T x = b (back substitution).
fn back_substitute(l: &Matrix, b: &[f64]) -> Vec<f64> {
    let n = l.len();
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = b[i];
        for k in i + 1..n {
            sum -= l[k][i] * x[k];
        }
        x[i] = sum / l[i][i];
    }
    x
}

/// Sign convention for MO rows stored per orbital.
fn canonicalize_signs_rows(mos: &mut [Vec<f64>]) {
    for mo in mos.iter_mut() {
        let mut best = 0usize;
        for (row, &c) in mo.iter().enumerate() {
            if c.abs() > mo[best].abs() {
                best = row;
            }
        }
        if mo[best] < 0.0 {
            for c in mo.iter_mut() {
                *c = -*c;
            }
        }
    }
}
