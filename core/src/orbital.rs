//! Hydrogen-like orbital densities for visualization (PLAN.md M3/M4 views).
//!
//! Non-relativistic Schrödinger solution of the Coulomb problem — the
//! standard normalized wavefunctions psi_nl m = R_nl(r) Y_lm(theta, phi):
//!  * radial part with generalized Laguerre polynomials and
//!    angular part |Y_lm|^2 via associated Legendre functions
//!    (Condon–Shortley phase).
//!
//! References: Bethe & Salpeter, Quantum Mechanics of One- and Two-Electron
//! Atoms (1957) §3.1; Bethe & Jackisch, Intermediate Quantum Mechanics (1968)
//! §3; Arfken, Weber & Harris, Mathematical Methods for Physicists (2013)
//! §19.2 for the polynomial recurrences. The relativistic (Dirac) corrections
//! to the DENSITY shape are O((Z alpha)^2) and deliberately not included —
//! this module is for visualization; level energies stay fully relativistic
//! in `dirac_atom`.
//!
//! All lengths are fm and densities fm^-3; the Bohr radius of the system is
//! a = hbar*c / (Z alpha m) with the caller-supplied (possibly reduced)
//! lepton mass, consistent with `dirac_atom`.

use crate::dirac_atom::{ALPHA, HBARC};

/// Bohr radius a = hbar / (Z alpha m) in fm.
pub fn bohr_radius_fm(z: u32, mass_mev: f64) -> f64 {
    HBARC / (z as f64 * ALPHA * mass_mev)
}

/// n! for the small n used here (exact in f64 up to 170).
fn factorial(k: u32) -> f64 {
    let mut f = 1.0f64;
    for i in 2..=k {
        f *= i as f64;
    }
    f
}

/// Generalized Laguerre polynomial L_k^{alpha}(x) by the stable
/// three-term recurrence (Arfken et al. 2013, eq. 19.70):
/// (k+1) L_{k+1} = (2k+1+alpha-x) L_k - (k+alpha) L_{k-1}.
fn gen_laguerre(k: u32, alpha: u32, x: f64) -> f64 {
    if k == 0 {
        return 1.0;
    }
    let mut l_prev = 1.0f64;
    let mut l_cur = 1.0 + alpha as f64 - x;
    for i in 1..k {
        let next = ((2.0 * i as f64 + 1.0 + alpha as f64 - x) * l_cur
            - (i as f64 + alpha as f64) * l_prev)
            / (i as f64 + 1.0);
        l_prev = l_cur;
        l_cur = next;
    }
    l_cur
}

/// Associated Legendre function P_l^m(x) on [-1, 1] including the
/// Condon–Shortley phase, by upward recurrence in l for fixed m
/// (Arfken et al. 2013, §19.4):
/// P_m^m = (-1)^m (2m-1)!! (1-x^2)^{m/2}, P_{m+1}^m = x (2m+1) P_m^m,
/// (l-m) P_l^m = x (2l-1) P_{l-1}^m - (l+m-1) P_{l-2}^m.
fn assoc_legendre(l: u32, m: u32, x: f64) -> f64 {
    if m > l {
        return 0.0;
    }
    let somx2 = ((1.0 - x) * (1.0 + x)).max(0.0).sqrt();
    let mut p_mm = 1.0f64;
    for i in 1..=m {
        // (2i-1)!! * (-somx2)^i built incrementally
        p_mm *= -((2 * i - 1) as f64) * somx2;
    }
    if l == m {
        return p_mm;
    }
    let p_m1 = x * (2 * m + 1) as f64 * p_mm;
    if l == m + 1 {
        return p_m1;
    }
    let mut p_prev = p_mm;
    let mut p_cur = p_m1;
    for ll in m + 2..=l {
        let next =
            (x * (2 * ll - 1) as f64 * p_cur - (ll + m - 1) as f64 * p_prev) / (ll - m) as f64;
        p_prev = p_cur;
        p_cur = next;
    }
    p_cur
}

/// Radial wavefunction R_nl(r), normalized as int R^2 r^2 dr = 1
/// (Bethe & Salpeter 1957 §3.1, eq. 3.12; r and a in fm).
pub fn radial(n: u32, l: u32, a_fm: f64, r_fm: f64) -> f64 {
    if n == 0 || l >= n {
        return 0.0;
    }
    let rho = 2.0 * r_fm / (n as f64 * a_fm);
    let norm = ((2.0 / (n as f64 * a_fm)).powi(3) * factorial(n - l - 1)
        / (2.0 * n as f64 * factorial(n + l)))
    .sqrt();
    norm * (-rho / 2.0).exp() * rho.powi(l as i32) * gen_laguerre(n - l - 1, 2 * l + 1, rho)
}

/// |Y_lm(theta)|^2 (phi-independent after squaring; the density of a
/// hydrogenic state is axially symmetric about z for every m).
pub fn angular_density(l: u32, m: u32, cos_theta: f64) -> f64 {
    let m = m.min(l);
    let p = assoc_legendre(l, m, cos_theta.clamp(-1.0, 1.0));
    (2 * l + 1) as f64 / (4.0 * std::f64::consts::PI) * factorial(l - m) / factorial(l + m) * p * p
}

/// Probability density |psi_nlm(x,y,z)|^2 in fm^-3, z = quantization axis.
pub fn density_at(n: u32, l: u32, m: u32, a_fm: f64, x: f64, y: f64, z: f64) -> f64 {
    let r = (x * x + y * y + z * z).sqrt();
    let cos_theta = if r > 0.0 { z / r } else { 1.0 };
    let r2 = radial(n, l, a_fm, r);
    r2 * r2 * angular_density(l, m, cos_theta)
}

/// Radial probability distribution 4 pi r^2 R_nl(r)^2 (probability per fm,
/// independent of m and of the angular pattern).
pub fn radial_distribution(n: u32, l: u32, a_fm: f64, r_fm: f64) -> f64 {
    let r = radial(n, l, a_fm, r_fm);
    4.0 * std::f64::consts::PI * r_fm * r_fm * r * r
}

/// Half-width of the plotting window (fm) that comfortably contains the
/// orbital: the hydrogenic radial extent scales as ~2 n^2 a plus tail.
pub fn extent_fm(n: u32, a_fm: f64) -> f64 {
    a_fm * (2.0 * n as f64 * n as f64 + 8.0)
}

/// Density on a square slice through the origin. `plane` "xz" (y = 0) or
/// "xy" (z = 0); returns row-major size x size, x along columns, z (or y)
/// along rows, centered at the nucleus.
pub fn slice_grid(n: u32, l: u32, m: u32, a_fm: f64, plane: &str, size: usize) -> (Vec<f64>, f64) {
    let half = extent_fm(n, a_fm);
    let step = 2.0 * half / size as f64;
    let mut out = vec![0.0f64; size * size];
    for (row, iy) in (0..size).enumerate() {
        let v = -half + (iy as f64 + 0.5) * step;
        for (col, ix) in (0..size).enumerate() {
            let u = -half + (ix as f64 + 0.5) * step;
            let (x, y, z) = if plane == "xy" {
                (u, v, 0.0)
            } else {
                (u, 0.0, v)
            };
            out[row * size + col] = density_at(n, l, m, a_fm, x, y, z);
        }
    }
    (out, half)
}

/// Position of the outer maximum of 4 pi r^2 R^2 (fm): coarse scan then
/// golden-section refinement (Bethe & Salpeter analytic anchors used in
/// tests: 2p -> 4a, 3d -> 9a).
pub fn peak_radius(n: u32, l: u32, a_fm: f64) -> f64 {
    let hi = extent_fm(n, a_fm);
    let steps = 4000;
    let dr = hi / steps as f64;
    let mut best = 0.0f64;
    let mut best_r = 0.0f64;
    for i in 1..=steps {
        let r = i as f64 * dr;
        let p = radial_distribution(n, l, a_fm, r);
        if p > best {
            best = p;
            best_r = r;
        }
    }
    // golden-section around the coarse maximum
    let gr = (5.0f64).sqrt() / 2.0 - 0.5;
    let mut lo = (best_r - dr).max(0.0);
    let mut up = best_r + dr;
    let mut c = up - gr * (up - lo);
    let mut d = lo + gr * (up - lo);
    for _ in 0..80 {
        if radial_distribution(n, l, a_fm, c) > radial_distribution(n, l, a_fm, d) {
            up = d;
        } else {
            lo = c;
        }
        c = up - gr * (up - lo);
        d = lo + gr * (up - lo);
    }
    0.5 * (lo + up)
}

#[cfg(test)]
mod tests {
    use super::*;

    // electron mass (MeV, CODATA 2022) and hydrogenic Bohr radius (fm)
    const M_E: f64 = 0.510_998_950_69;

    fn a_h() -> f64 {
        bohr_radius_fm(1, M_E)
    }

    #[test]
    fn bohr_radius_matches_known_value() {
        // a0 = 52918 fm (CODATA 2022, via alpha and hbarc above)
        assert!((a_h() - 52_918.0).abs() / 52_918.0 < 1e-4, "a={}", a_h());
    }

    #[test]
    fn one_s_density_at_nucleus_is_exact() {
        // psi_1s(0) = (Z/a)^(3/2)/sqrt(pi)  =>  |psi(0)|^2 = (Z/a)^3/pi
        let a = a_h();
        let rho = density_at(1, 0, 0, a, 0.0, 0.0, 0.0);
        let exact = 1.0 / (a * a * a * std::f64::consts::PI);
        assert!(
            (rho - exact).abs() / exact < 1e-10,
            "rho={rho} exact={exact}"
        );
    }

    #[test]
    fn one_s_density_at_bohr_radius() {
        // |psi_1s(a)|^2 = e^{-2}/(pi a^3) (Bethe & Salpeter eq. 3.12)
        let a = a_h();
        let rho = density_at(1, 0, 0, a, a, 0.0, 0.0);
        let exact = (-2.0f64).exp() / (a * a * a * std::f64::consts::PI);
        assert!((rho - exact).abs() / exact < 1e-10);
    }

    #[test]
    fn radial_normalization_is_unity() {
        // int R^2 r^2 dr = 1 for every (n,l) (Bethe & Salpeter §3.1);
        // integrate to 4x the plot window so the exponential tail is dead
        let a = a_h();
        for (n, l) in [(1, 0), (2, 0), (2, 1), (3, 0), (3, 1), (3, 2)] {
            let hi = 4.0 * extent_fm(n, a);
            let steps = 200_000;
            let dr = hi / steps as f64;
            let mut s = 0.0f64;
            for i in 0..steps {
                let r0 = i as f64 * dr;
                let r1 = r0 + dr;
                let f0 = radial(n, l, a, r0);
                let f1 = radial(n, l, a, r1);
                s += 0.5 * dr * (r0 * r0 * f0 * f0 + r1 * r1 * f1 * f1);
            }
            assert!((s - 1.0).abs() < 5e-3, "n={n} l={l} norm={s}");
        }
    }

    #[test]
    fn angular_normalization_is_unity() {
        // int |Y_lm|^2 dOmega = 1 (Arfken §19.4)
        for (l, m) in [(0, 0), (1, 0), (1, 1), (2, 0), (2, 2), (3, 1)] {
            let steps = 20_000;
            let dth = std::f64::consts::PI / steps as f64;
            let mut s = 0.0f64;
            for i in 0..steps {
                let t0 = i as f64 * dth;
                let t1 = t0 + dth;
                let f0 = angular_density(l, m, t0.cos());
                let f1 = angular_density(l, m, t1.cos());
                s += 2.0 * std::f64::consts::PI * 0.5 * dth * (f0 * t0.sin() + f1 * t1.sin());
            }
            assert!((s - 1.0).abs() < 1e-3, "l={l} m={m} norm={s}");
        }
    }

    #[test]
    fn radial_nodes_at_analytic_positions() {
        // 2s node at rho = 2 (L_1^1(rho) = 2 - rho, rho = r/a) -> r = 2a;
        // 3s nodes at rho = 3 +- sqrt(3) (roots of L_2^1, Bethe & Salpeter
        // 1957 §3.2 table) with rho = 2r/(3a) -> r = 1.90a, 7.10a
        let a = a_h();
        assert!(radial(2, 0, a, 2.0 * a).abs() < 1e-12 * radial(2, 0, a, a).abs().max(1.0));
        for rho in [3.0 - 3.0f64.sqrt(), 3.0 + 3.0f64.sqrt()] {
            let r = 0.5 * rho * 3.0 * a;
            let scale = radial(3, 0, a, 3.0 * a).abs();
            assert!(radial(3, 0, a, r).abs() < 1e-9 * scale, "rho={rho}");
        }
    }

    #[test]
    fn radial_peaks_at_analytic_positions() {
        // 2p outer max of 4 pi r^2 R^2 at r = 4a; 3d at r = 9a
        // (Bethe & Salpeter 1957 §3.2, most-probable radii)
        let a = a_h();
        let p2 = peak_radius(2, 1, a);
        assert!((p2 - 4.0 * a).abs() / a < 0.01, "2p peak {p2}/a");
        let p3 = peak_radius(3, 2, a);
        assert!((p3 - 9.0 * a).abs() / a < 0.01, "3d peak {p3}/a");
        // 1s peak at a
        let p1 = peak_radius(1, 0, a);
        assert!((p1 - a).abs() / a < 0.01, "1s peak {p1}/a");
    }

    #[test]
    fn two_p_z_has_angular_node_in_the_equatorial_plane() {
        // |Y_10|^2 ~ cos^2(theta): zero in the xy plane, max on z (p-orbital
        // node, standard textbook result)
        let a = a_h();
        let r = 4.0 * a;
        let along_z = density_at(2, 1, 0, a, 0.0, 0.0, r);
        let in_plane = density_at(2, 1, 0, a, r, 0.0, 0.0);
        assert!(in_plane < 1e-30 * along_z, "in={in_plane} z={along_z}");
        // m = +-1 states vanish on the z axis instead
        let m1_axis = density_at(2, 1, 1, a, 0.0, 0.0, r);
        let m1_plane = density_at(2, 1, 1, a, r, 0.0, 0.0);
        assert!(m1_axis < 1e-30 * m1_plane);
    }

    #[test]
    fn cartesian_integral_of_one_s_is_unity() {
        // full 3D check of density_at (radial x angular) for the smooth 1s:
        // int |psi|^2 d^3r over a cube containing > 99.99 % of the density
        let a = a_h();
        let half = 10.0 * a;
        let steps = 64;
        let d = 2.0 * half / steps as f64;
        let mut s = 0.0f64;
        for i in 0..steps {
            let x = -half + (i as f64 + 0.5) * d;
            for j in 0..steps {
                let y = -half + (j as f64 + 0.5) * d;
                for k in 0..steps {
                    let z = -half + (k as f64 + 0.5) * d;
                    s += density_at(1, 0, 0, a, x, y, z);
                }
            }
        }
        let volume = d * d * d;
        let total = s * volume;
        // cube truncation (-10a..10a) loses < 1e-4; grid coarseness adds ~1%
        assert!((total - 1.0).abs() < 0.02, "total={total}");
    }

    #[test]
    fn slice_grid_shape_and_peak() {
        let a = a_h();
        let (g, half) = slice_grid(2, 1, 0, a, "xz", 64);
        assert_eq!(g.len(), 64 * 64);
        assert!((half - 16.0 * a).abs() < 1e-9);
        // 2p m=0: |psi|^2 = R^2 cos^2 ~ r^2 e^{-r/a} peaks on the z axis at
        // r = 2a (the 4a maximum is of the RADIAL distribution r^2 R^2, a
        // different quantity - see peak_radius test)
        let mut max = (0.0f64, 0usize, 0usize);
        for (i, v) in g.iter().enumerate() {
            if *v > max.0 {
                max = (*v, i / 64, i % 64);
            }
        }
        assert!(max.1 < 32 - 3 || max.1 > 32 + 3, "peak row {}", max.1);
        assert!((max.2 as i32 - 32).abs() <= 1, "peak col {}", max.2);
        // cell centres never hit z = 0 exactly, so the equatorial density is
        // suppressed by (0.5 step / 4a)^2 ~ 1e-3, not exactly zero
        assert!(g[32 * 64 + 60] < 1e-3 * max.0);
    }
}
