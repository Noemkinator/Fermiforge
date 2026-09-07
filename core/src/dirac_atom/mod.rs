//! Hydrogen-like atoms: analytic Dirac spectrum + radial Dirac shooting solver.
//!
//! Units: MeV, fm, natural units (hbar = c = 1 carried via HBARC).
//! Energies include the lepton rest mass; binding energy = m - E.

use serde::Serialize;

/// hbar*c in MeV*fm (CODATA 2022).
const HBARC: f64 = 197.326_980_4;
/// fine-structure constant (CODATA 2022).
const ALPHA: f64 = 7.297_352_5643e-3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Level {
    pub n: u32,
    pub kappa: i32,
    /// total energy (incl. rest mass) in MeV
    pub e_mev: f64,
    /// binding energy m - E in MeV
    pub binding_mev: f64,
}

/// Analytic Dirac-Coulomb spectrum (point nucleus), Bangert/Weber form:
/// E = m * [1 + (Z*a)^2 / (n - |k| + sqrt(k^2 - (Z*a)^2))^2]^(-1/2)
pub fn hydrogenic_energy(z: u32, mass_mev: f64, n: u32, kappa: i32) -> f64 {
    let za = z as f64 * ALPHA;
    let k = kappa as f64;
    let g = (k * k - za * za).sqrt();
    let denom = n as f64 - k.abs() + g;
    mass_mev / (1.0 + za * za / (denom * denom)).sqrt()
}

/// All (n, kappa) levels for n in 1..=n_max.
pub fn level_list(n_max: u32) -> Vec<(u32, i32)> {
    let mut out = Vec::new();
    for n in 1..=n_max {
        for k in 1..=(n as i32 - 1) {
            out.push((n, -k));
            out.push((n, k));
        }
        out.push((n, -(n as i32)));
    }
    out
}

/// Radial Dirac eigenvalue via outward RK4 shooting on a logarithmic mesh,
/// bracketed by node-counted sign changes of P(r_max).
pub fn solve_level(z: u32, mass_mev: f64, n: u32, kappa: i32) -> f64 {
    solve_level_with_steps(z, mass_mev, n, kappa, 120_000)
}

/// Mesh-size variant used by convergence tests.
pub fn solve_level_with_steps(z: u32, mass_mev: f64, n: u32, kappa: i32, steps: usize) -> f64 {
    let za = z as f64 * ALPHA;
    assert!(za < 1.0, "point nucleus requires Z*alpha < 1");
    let n_r = n as i32 - kappa.abs() - 1;

    // outer scale: Bohr radius of the lepton scaled by n^2/Z, generous margin
    let a = HBARC / (za * mass_mev);
    let r_max = a * (n * n) as f64 * 50.0;
    let r_min = (a * 1e-9).min(r_max * 1e-9).max(1e-7);
    let h = ((r_max / r_min).ln() / steps as f64).min(2e-3);
    let nsteps = ((r_max / r_min).ln() / h).ceil() as usize;

    let integrate = |e: f64| -> (f64, i32) {
        let g = ((kappa * kappa) as f64 - za * za).sqrt();
        let ratio = (g + kappa as f64) / za;
        let mut p = r_min.powf(g);
        let mut q = ratio * p;
        let mut nodes = 0i32;
        let mut prev_sign = p.signum();
        let mut r = r_min;
        for _ in 0..nsteps {
            let r2 = (r * h.exp()).min(r_max);
            let (p2, q2) = rk4_step(r, r2, p, q, kappa, e, mass_mev, za);
            let s = p2.signum();
            if s != 0.0 && s != prev_sign && p2.abs() > 1e-300 {
                nodes += 1;
                prev_sign = s;
            }
            p = p2;
            q = q2;
            r = r2;
            if !p.is_finite() || p.abs() > 1e250 {
                let s = if p >= 0.0 { 1.0 } else { -1.0 };
                return (s * f64::MAX / 2.0, nodes);
            }
        }
        (p, nodes)
    };

    // nonrelativistic anchor and window between neighboring principal levels
    let b = za * za * mass_mev / (2.0 * (n * n) as f64);
    let e_nr = mass_mev - b;
    let gap = if n > 1 {
        za * za * mass_mev / 2.0 * (1.0 / ((n - 1) * (n - 1)) as f64 - 1.0 / (n * n) as f64)
    } else {
        b * 1.5
    };
    let gap_below =
        za * za * mass_mev / 2.0 * (1.0 / (n * n) as f64 - 1.0 / ((n + 1) * (n + 1)) as f64);
    let lo = e_nr - 0.45 * gap;
    let hi = e_nr + 0.45 * gap_below;

    // scan for sign changes of P(r_max); the (n_r+1)-th is the target level
    let mut roots: Vec<(f64, f64)> = Vec::new();
    let mut e_prev = lo;
    let (mut f_prev, _) = integrate(e_prev);
    for i in 1..=96 {
        let e = lo + (hi - lo) * i as f64 / 96.0;
        let (f, _) = integrate(e);
        if f_prev.is_finite() && f.is_finite() && f_prev * f < 0.0 {
            roots.push((e_prev, e));
        }
        e_prev = e;
        f_prev = f;
    }
    let (mut a, mut bnd) = *roots
        .get(n_r as usize)
        .or_else(|| roots.last())
        .unwrap_or_else(|| panic!("no eigenvalue found for n={n} kappa={kappa}"));
    let _ = &mut a;
    // bisect on sign of P(r_max)
    let (mut fa, _) = integrate(a);
    for _ in 0..200 {
        let mid = 0.5 * (a + bnd);
        if mid == a || mid == bnd {
            break;
        }
        let (fm, _) = integrate(mid);
        if fa * fm <= 0.0 {
            bnd = mid;
        } else {
            a = mid;
            fa = fm;
        }
    }
    0.5 * (a + bnd)
}

fn rk4_step(r: f64, r_new: f64, p: f64, q: f64, kappa: i32, e: f64, m: f64, za: f64) -> (f64, f64) {
    // derivative w.r.t. x = ln r of (P, Q)
    let deriv = |rr: f64, pp: f64, qq: f64| -> (f64, f64) {
        let v = -za * HBARC / rr; // V(r) in MeV
        let k1 = (e - v + m) / HBARC;
        let k2 = (e - v - m) / HBARC;
        let dp = rr * (-(kappa as f64) / rr * pp + k1 * qq);
        let dq = rr * ((kappa as f64) / rr * qq - k2 * pp);
        (dp, dq)
    };
    let hh = (r_new / r).ln();
    let (k1p, k1q) = deriv(r, p, q);
    let rm = r * (h05(hh));
    let (k2p, k2q) = deriv(rm, p + 0.5 * hh * k1p, q + 0.5 * hh * k1q);
    let (k3p, k3q) = deriv(rm, p + 0.5 * hh * k2p, q + 0.5 * hh * k2q);
    let re = r * hh.exp();
    let (k4p, k4q) = deriv(re, p + hh * k3p, q + hh * k3q);
    (
        p + hh / 6.0 * (k1p + 2.0 * k2p + 2.0 * k3p + k4p),
        q + hh / 6.0 * (k1q + 2.0 * k2q + 2.0 * k3q + k4q),
    )
}

#[inline]
fn h05(hh: f64) -> f64 {
    (hh * 0.5).exp()
}

/// Solve all levels up to n_max.
pub fn solve_levels(z: u32, mass_mev: f64, n_max: u32) -> Vec<Level> {
    level_list(n_max)
        .into_iter()
        .map(|(n, kappa)| {
            let e = solve_level(z, mass_mev, n, kappa);
            Level {
                n,
                kappa,
                e_mev: e,
                binding_mev: mass_mev - e,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const M_E: f64 = 0.510_998_95; // electron, MeV (PDG 2022)
    const M_MU: f64 = 105.658_375_5; // muon, MeV (PDG 2022)

    fn rel_err(num: f64, exact: f64) -> f64 {
        ((num - exact) / exact).abs()
    }

    #[test]
    fn hydrogen_levels_match_analytic_dirac() {
        for (n, kappa) in [(1, -1), (2, -2), (2, 1), (2, -1), (3, 1), (3, -3), (3, 2)] {
            let num = solve_level(1, M_E, n, kappa);
            let exact = hydrogenic_energy(1, M_E, n, kappa);
            assert!(
                rel_err(num, exact) < 1e-7,
                "n={n} k={kappa}: num={num} exact={exact} rel={}",
                rel_err(num, exact)
            );
        }
    }

    #[test]
    fn muonic_hydrogen_matches_analytic() {
        for (n, kappa) in [(1, -1), (2, 1), (2, -1)] {
            let num = solve_level(1, M_MU, n, kappa);
            let exact = hydrogenic_energy(1, M_MU, n, kappa);
            assert!(
                rel_err(num, exact) < 1e-7,
                "muon n={n} k={kappa}: rel={}",
                rel_err(num, exact)
            );
        }
    }

    #[test]
    fn high_z_point_nucleus_matches_analytic() {
        // Z=82: Z*alpha=0.598, still below 1; point-nucleus reference is exact
        for (n, kappa) in [(1, -1), (2, 1)] {
            let num = solve_level(82, M_E, n, kappa);
            let exact = hydrogenic_energy(82, M_E, n, kappa);
            assert!(
                rel_err(num, exact) < 1e-5,
                "U n={n} k={kappa}: rel={}",
                rel_err(num, exact)
            );
        }
    }

    #[test]
    fn fine_structure_ordering() {
        // Dirac-Coulomb: E depends on (n, |kappa|); 2s1/2 (k=-1) degenerate with
        // 2p1/2 (k=+1); 2p3/2 (k=-2) lies above (less bound).
        let e2s = solve_level(1, M_E, 2, -1);
        let e2p12 = solve_level(1, M_E, 2, 1);
        let e2p32 = solve_level(1, M_E, 2, -2);
        assert!(
            (e2s - e2p12).abs() < 1e-9 * M_E,
            "2s1/2 and 2p1/2 must be degenerate: {e2s} vs {e2p12}"
        );
        assert!(e2p12 < e2p32, "2p1/2 should bind more than 2p3/2");
    }

    #[test]
    fn determinism_bitwise() {
        let a = solve_levels(1, M_E, 2);
        let b = solve_levels(1, M_E, 2);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.e_mev.to_bits(), y.e_mev.to_bits());
        }
    }

    #[test]
    fn mesh_convergence() {
        let exact = hydrogenic_energy(1, M_E, 1, -1);
        let coarse = solve_level_with_steps(1, M_E, 1, -1, 20_000);
        let fine = solve_level_with_steps(1, M_E, 1, -1, 160_000);
        let ec = ((coarse - exact) / exact).abs();
        let ef = ((fine - exact) / exact).abs();
        assert!(ef < ec / 16.0 || ef < 1e-12, "ec={ec} ef={ef}");
    }
}
