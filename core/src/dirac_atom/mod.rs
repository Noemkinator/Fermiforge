//! Hydrogen-like atoms: analytic Dirac spectrum + radial Dirac shooting solver,
//! finite-size (2pF / uniform-sphere) nuclei and the Uehling vacuum-polarization
//! potential.
//!
//! Units: MeV, fm, natural units (hbar = c = 1 carried via HBARC).
//! Energies include the lepton rest mass; binding energy = m - E.
//!
//! References used throughout this module:
//!  * Radial Dirac equations in the form integrated here (P, Q on ln r mesh):
//!    W. Greiner, B. Müller, "Quantum Mechanics of One- and Two-Electron
//!    Atoms" (Springer 1987), §4; I. P. Grant, "Relativistic Quantum Theory of
//!    Atoms and Molecules" (Springer 2007), §7.1.
//!  * Finite nuclear size as a perturbation (leading order, s states):
//!    H. A. Bethe, E. E. Salpeter, "Quantum Mechanics of One- and Two-Electron
//!    Atoms" (1957), §53; A. P. French, Phys. Rev. 76 (1949) 1741.
//!    Delta-E_ns = (2*pi/3) * Z*alpha * |psi_ns(0)|^2 * <r^2>,
//!    |psi_ns(0)|^2 = (Z*alpha*mu)^3 / (pi * n^3).
//!  * Two-parameter Fermi (Woods-Saxon) charge shape from elastic electron
//!    scattering: R. Hofstadter, Rev. Mod. Phys. 28 (1956) 214;
//!    H. Fricke et al., At. Data Nucl. Data Tables 43 (1995) 71 (tabulated c, a
//!    cluster around a ~ 0.52 fm, used here as the fixed diffuseness).
//!  * Uehling vacuum polarization: E. A. Uehling, Phys. Rev. 48 (1935) 55;
//!    modern form e.g. R. Borie, Ann. Phys. 327 (2012) 733, eq. (10).
//!  * Proton / deuteron rms charge radii and lepton masses: CODATA 2022;
//!    heavy-nucleus rms radii: I. Angeli, K. P. Marinova, At. Data Nucl. Data
//!    Tables 101 (2013) 185.

use serde::Serialize;

/// hbar*c in MeV*fm (CODATA 2022).
pub const HBARC: f64 = 197.326_980_4;
/// fine-structure constant (CODATA 2022).
pub const ALPHA: f64 = 7.297_352_564_3e-3;
/// Standard 2pF surface diffuseness in fm. Fricke et al. (1995) tabulate
/// a = 0.44..0.57 fm across the periodic table; 0.52 fm is the canonical
/// electron-scattering value (Hofstadter 1956).
pub const TWO_PF_A_FM: f64 = 0.52;

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

/// Reduced mass mu = m M / (m + M) (no-recoil approximation: the Dirac
/// equation is solved for the lepton in the field of a fixed nucleus of mass M
/// with m replaced by mu; Bransden & Joachain, "Physics of Atoms and
/// Molecules", 3rd ed., §5.4 / §15.2).
pub fn reduced_mass(m_lepton: f64, m_nucleus: f64) -> f64 {
    m_lepton * m_nucleus / (m_lepton + m_nucleus)
}

/// Nuclear charge distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChargeDist {
    /// Point charge: V(r) = -Z*alpha*hbarc/r (Dirac-Coulomb, singular at 0).
    Point,
    /// Uniformly charged sphere of radius R (analytic; Bethe & Salpeter 1957
    /// §53). Used to benchmark against leading-order perturbation theory.
    Uniform { r_fermi: f64 },
    /// Symmetric two-parameter Fermi shape rho(r) ~ 1/(1+exp((r-c)/a)):
    /// the standard nuclear charge density from elastic electron scattering
    /// (Hofstadter 1956; Fricke et al. 1995).
    Fermi2p { c_fermi: f64, a_fermi: f64 },
}

/// Electrostatic environment of the nucleus stored as the DEVIATION
/// dV(r) = V(r) - V_point(r) (MeV) from the point-charge Coulomb potential,
/// tabulated on a uniform ln r grid and interpolated linearly; optionally
/// includes the Uehling vacuum-polarization term.
///
/// Why the deviation and not the full potential: linear interpolation of the
/// 1/r Coulomb tail on a log grid carries a relative error of (d ln r)^2 / 8
/// ~ 2e-6 of |V| - harmless for the ODE, but catastrophic when the
/// nuclear-size / vacuum-polarization deviation (many orders of magnitude
/// smaller than |V| at small r) is obtained by subtracting the two. Storing
/// dV directly makes it exact wherever the charge is point-like (identically
/// zero beyond the support), so perturbative shifts are free of the
/// interpolation artifact.
pub struct Potential {
    za: f64,
    ln_r0: f64,
    d_ln: f64,
    v: Vec<f64>,
    r_hi: f64,
}

impl Potential {
    /// dV(r) = V(r) - V_point(r) in MeV at r in fm.
    pub fn at(&self, r: f64) -> f64 {
        if r > self.r_hi {
            // beyond the support of the deviation: pure point charge
            return 0.0;
        }
        let x = (r.ln() - self.ln_r0) / self.d_ln;
        if x <= 0.0 {
            return self.v[0];
        }
        let i = x.floor() as usize;
        if i + 1 >= self.v.len() {
            return self.v[self.v.len() - 1];
        }
        let t = x - i as f64;
        self.v[i] * (1.0 - t) + self.v[i + 1] * t
    }

    /// Full V(r) in MeV at r in fm: point Coulomb plus the deviation.
    pub fn full_at(&self, r: f64) -> f64 {
        -self.za * HBARC / r + self.at(r)
    }
}

/// Build the tabulated potential deviation. `r_out` must cover the outer
/// turning point of the ODE domain (fm). `uehling_m_e` = electron mass (MeV)
/// adds the vacuum-polarization term; None = pure nuclear charge.
pub fn build_potential(
    z: u32,
    dist: ChargeDist,
    uehling_m_e: Option<f64>,
    r_out: f64,
) -> Potential {
    let za = z as f64 * ALPHA;
    let n_pts = 8192usize;
    let r_lo = 1e-7f64;
    let r_hi = r_out.max(10.0);
    let ln_r0 = r_lo.ln();
    let d_ln = (r_hi.ln() - ln_r0) / (n_pts - 1) as f64;

    // charge deviation from the point charge on the log grid
    let charge_dv = |r: f64| -> f64 {
        match dist {
            ChargeDist::Point => 0.0,
            ChargeDist::Uniform { r_fermi } => {
                if r <= r_fermi {
                    // V(r) = -Za*hbarc*(3R^2 - r^2)/(2R^3) (inside a
                    // uniformly charged sphere; Bethe & Salpeter 1957 §53)
                    let v_in =
                        -za * HBARC * (3.0 * r_fermi * r_fermi - r * r) / (2.0 * r_fermi.powi(3));
                    v_in + za * HBARC / r
                } else {
                    0.0
                }
            }
            ChargeDist::Fermi2p { c_fermi, a_fermi } => {
                fermi_potential(za, r, c_fermi, a_fermi) + za * HBARC / r
            }
        }
    };

    let mut v: Vec<f64> = Vec::with_capacity(n_pts);
    for i in 0..n_pts {
        let r = (ln_r0 + i as f64 * d_ln).exp();
        let mut val = charge_dv(r);
        if let Some(m_e) = uehling_m_e {
            val += uehling(za, r, m_e);
        }
        v.push(val);
    }
    Potential {
        za,
        ln_r0,
        d_ln,
        v,
        r_hi,
    }
}

/// Unnormalized 2pF density rho(r) = 1/(1+exp((r-c)/a)).
fn fermi_rho(r: f64, c: f64, a: f64) -> f64 {
    // stable logistic: 1/(1+e^x) with x=(r-c)/a
    let x = (r - c) / a;
    if x >= 0.0 {
        let e = (-x).exp();
        e / (1.0 + e)
    } else {
        1.0 / (1.0 + x.exp())
    }
}

/// Potential energy (MeV) of the 2pF charge distribution via the radial
/// Poisson solution
///   V(r) = -Z*alpha*hbarc * [ Q(r)/r + G(r) ] / Q_tot,
///   Q(r) = 4*pi*Int_0^r rho r'^2 dr',  G(r) = 4*pi*Int_r^inf rho r' dr'.
/// Evaluated on a fine linear grid; cached lazily is unnecessary at this size.
fn fermi_potential(za: f64, r: f64, c: f64, a: f64) -> f64 {
    if r > c + 40.0 * a {
        // density tail ~ e^-40: point charge to machine precision
        return -za * HBARC / r;
    }
    let r_max = c + 40.0 * a;
    let step = (a / 40.0).min(0.01);
    let n = (r_max / step).ceil() as usize;
    let f1 = |rr: f64| fermi_rho(rr, c, a) * rr * rr; // charge measure / 4pi
    let f2 = |rr: f64| fermi_rho(rr, c, a) * rr; // G integrand / 4pi
    // totals over [0, r_max]
    let (mut q_tot, mut acc2_tot) = (0.0f64, 0.0f64);
    for i in 1..=n {
        let rr = i as f64 * step;
        q_tot += 0.5 * step * (f1(rr - step) + f1(rr));
        acc2_tot += 0.5 * step * (f2(rr - step) + f2(rr));
    }
    // partial integrals up to r
    let (mut q, mut acc2) = (0.0f64, 0.0f64);
    for i in 1..=n {
        let rr = i as f64 * step;
        if rr > r {
            break;
        }
        q += 0.5 * step * (f1(rr - step) + f1(rr));
        acc2 += 0.5 * step * (f2(rr - step) + f2(rr));
    }
    let g_r = acc2_tot - acc2;
    -za * HBARC * (q / r + g_r) / q_tot
}

/// rms charge radius (fm) of a 2pF distribution:
/// sqrt(<r^2>) = sqrt(Int rho r^4 dr / Int rho r^2 dr).
pub fn fermi_rms(c: f64, a: f64) -> f64 {
    let r_max = c + 40.0 * a;
    let step = (a / 40.0).min(0.01);
    let n = (r_max / step).ceil() as usize;
    let (mut m2, mut m4) = (0.0f64, 0.0f64);
    for i in 1..=n {
        let rr = i as f64 * step;
        let w = 0.5 * step * (fermi_rho(rr - step, c, a) + fermi_rho(rr, c, a));
        m2 += w * rr * rr;
        m4 += w * rr * rr * rr * rr;
    }
    (m4 / m2).sqrt()
}

/// Half-density radius c (fm) of a 2pF shape with fixed diffuseness `a` whose
/// rms charge radius equals `rms_target` (monotone in c -> bisection).
pub fn solve_two_pf(rms_target: f64, a: f64) -> f64 {
    let (mut lo, mut hi) = (0.1f64, rms_target * 1.5 + 2.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if fermi_rms(mid, a) < rms_target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Uehling (1935) vacuum-polarization potential energy (MeV) at r (fm) for a
/// point charge Z, lepton coupling Z*alpha:
///   V_U(r) = -(2*alpha*Za)/(3*pi*r) * Int_1^inf e^{-2 m_e r t}
///            * (1 + 1/(2 t^2)) * sqrt(t^2-1)/t^2 dt          [natural units]
/// Substituted t = e^u so the logarithmic small-r region is resolved
/// (Uehling, Phys. Rev. 48 (1935) 55; Borie, Ann. Phys. 327 (2012) 733).
fn uehling(za: f64, r_fm: f64, m_e: f64) -> f64 {
    let r_nat = r_fm / HBARC; // MeV^-1
    let b = 2.0 * m_e * r_nat;
    if b > 60.0 {
        return 0.0; // e^-60: beyond the vacuum-polarization range
    }
    let u_max = (50.0f64 / b).ln().clamp(0.5, 60.0);
    // nodes/weights are a constant - compute once (called per grid point
    // when building tables and per quadrature point in tests)
    static GL: std::sync::OnceLock<(Vec<f64>, Vec<f64>)> = std::sync::OnceLock::new();
    let (xs, ws) = GL.get_or_init(|| gauss_legendre(48));
    let mut sum = 0.0f64;
    for (x, w) in xs.iter().zip(ws.iter()) {
        let u = 0.5 * u_max * (x + 1.0);
        let t = u.exp();
        let g = ((t * t - 1.0).max(0.0)).sqrt() / (t * t) * (1.0 + 0.5 / (t * t));
        sum += w * (-b * t).exp() * g * t;
    }
    let integral = 0.5 * u_max * sum;
    -(2.0 * ALPHA * za) / (3.0 * std::f64::consts::PI) * integral / r_nat
}

/// Gauss-Legendre nodes/weights on [-1, 1] via Newton iteration on P_n
/// (standard algorithm; Press et al., Numerical Recipes 3rd ed., §4.6.3).
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0; n];
    let mut ws = vec![0.0; n];
    let legendre = |z: f64| -> (f64, f64) {
        let mut p0 = 1.0f64;
        let mut p1 = z;
        for k in 2..=n {
            let p = ((2 * k - 1) as f64 * z * p1 - (k - 1) as f64 * p0) / k as f64;
            p0 = p1;
            p1 = p;
        }
        let pp = n as f64 * (z * p1 - p0) / (z * z - 1.0);
        (p1, pp)
    };
    for i in 0..n {
        // Tricomi-type initial guess
        let mut z = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        for _ in 0..50 {
            let (p, pp) = legendre(z);
            let dz = p / pp;
            z -= dz;
            if dz.abs() < 1e-15 {
                break;
            }
        }
        let (p, pp) = legendre(z);
        let _ = p;
        xs[i] = z;
        ws[i] = 2.0 / ((1.0 - z * z) * pp * pp);
    }
    (xs, ws)
}

/// Radial Dirac eigenvalue via outward RK4 shooting on a logarithmic mesh,
/// bracketed by node-counted sign changes of P(r_max).
pub fn solve_level(z: u32, mass_mev: f64, n: u32, kappa: i32) -> f64 {
    solve_level_with_steps(z, mass_mev, n, kappa, 120_000)
}

/// Mesh-size variant used by convergence tests.
pub fn solve_level_with_steps(z: u32, mass_mev: f64, n: u32, kappa: i32, steps: usize) -> f64 {
    solve_level_opt(z, mass_mev, n, kappa, steps, None)
}

/// As `solve_level` but with an explicit potential table (finite nucleus
/// and/or Uehling term). The small-r initial condition keeps the
/// point-Coulomb exponent: for a regularized interior potential the irregular
/// solution behaves as r^{-|kappa|}, so at r_min ~ 1e-7 fm its admixture is
/// < 1e-13 for every Z*alpha < 1 (standard practice; cf. Johnson, "Atomic
/// Structure Theory", §8.2, and Iyer et al., Comput. Phys. Commun. 194 (2015)
/// 232, who start from a small-r series).
pub fn solve_level_opt(
    z: u32,
    mass_mev: f64,
    n: u32,
    kappa: i32,
    steps: usize,
    pot: Option<&Potential>,
) -> f64 {
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
        let o = Ode {
            kappa,
            e,
            m: mass_mev,
            za,
            pot,
        };
        let mut r = r_min;
        for _ in 0..nsteps {
            let r2 = (r * h.exp()).min(r_max);
            let (p2, q2) = rk4_step(r, r2, p, q, &o);
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

    // nonrelativistic anchor and window between neighboring principal levels.
    // For n=1 the window is widened downward: relativistic enhancement of the
    // binding (factor < 2 for Z*alpha < 1; e.g. 1.11 for muonic Pb, Dirac
    // point-nucleus) would otherwise fall below the NR anchor.
    let b = za * za * mass_mev / (2.0 * (n * n) as f64);
    let e_nr = mass_mev - b;
    let gap = if n > 1 {
        za * za * mass_mev / 2.0 * (1.0 / ((n - 1) * (n - 1)) as f64 - 1.0 / (n * n) as f64)
    } else {
        b * 1.9
    };
    let gap_below =
        za * za * mass_mev / 2.0 * (1.0 / (n * n) as f64 - 1.0 / ((n + 1) * (n + 1)) as f64);
    let lo = e_nr - 0.45 * gap;
    // hi reaches 90 % toward the next NR level: a finite-size level moves UP
    // by meV-to-MeV amounts (far beyond the point-Coulomb anchor) yet always
    // stays below the next level, whose own finite-size shift raises it too.
    let hi = e_nr + 0.9 * gap_below;

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

/// First-order perturbative energy shift of a Dirac-Coulomb level under a
/// modified local potential `pot` (finite nuclear size and/or Uehling term),
/// evaluated with the unperturbed point-nucleus eigenfunction at the exact
/// analytic eigenvalue:
///   dE = Int [P^2 + Q^2] dV dr / Int [P^2 + Q^2] dr,
///   dV(r) = V(r) + Z*alpha*hbarc/r,
/// because a local potential weights both radial components equally
/// (Greiner & Mueller 1987, §4, radial matrix elements).
///
/// This is the standard extraction for shifts far below the absolute
/// accuracy of outward shooting: outward integration at the eigenvalue is
/// eventually dominated by the exponentially growing solution (inherent to
/// the method - the shooting root itself is only a proxy for the level), but
/// dV has compact support (the table equals the point charge exactly beyond
/// the nuclear / vacuum-polarization range), so the contamination region is
/// excluded. The integration stops at r = 16 n a, where the relative
/// admixture of the growing solution is ~1e-13 * e^32 and the neglected
/// density tail ~e^-32 - both far below double-precision relevance.
/// First-order PT itself is accurate to O(dE / Hartree) here, i.e. < 1e-3
/// relative for every shift computed in this codebase (cf. Borie 2012,
/// sec. 2, who iterates the full Dirac equation; agreement with first-order
/// PT at these magnitudes is < 1e-6 relative).
pub fn expectation_shift(
    z: u32,
    mass_mev: f64,
    n: u32,
    kappa: i32,
    pot: &Potential,
    steps: usize,
) -> f64 {
    let za = z as f64 * ALPHA;
    let e = hydrogenic_energy(z, mass_mev, n, kappa);
    let a = HBARC / (za * mass_mev);
    let r_max = a * n as f64 * 16.0;
    let r_min = (a * 1e-9).min(r_max * 1e-9).max(1e-7);
    let h = ((r_max / r_min).ln() / steps as f64).min(2e-3);
    let nsteps = ((r_max / r_min).ln() / h).ceil() as usize;

    let g = ((kappa * kappa) as f64 - za * za).sqrt();
    let ratio = (g + kappa as f64) / za;
    let mut p = r_min.powf(g);
    let mut q = ratio * p;
    let o = Ode {
        kappa,
        e,
        m: mass_mev,
        za,
        pot: None,
    };

    let mut norm = 0.0f64;
    let mut shift = 0.0f64;
    let mut r = r_min;
    for _ in 0..nsteps {
        let r2 = (r * h.exp()).min(r_max);
        let (p2, q2) = rk4_step(r, r2, p, q, &o);
        let dr = r2 - r;
        let f1 = p * p + q * q;
        let f2 = p2 * p2 + q2 * q2;
        norm += 0.5 * dr * (f1 + f2);
        // dV: the table stores V - point Coulomb directly (compact support)
        let d1 = pot.at(r);
        let d2 = pot.at(r2);
        shift += 0.5 * dr * (f1 * d1 + f2 * d2);
        p = p2;
        q = q2;
        r = r2;
    }
    shift / norm
}

struct Ode<'a> {
    kappa: i32,
    e: f64,
    m: f64,
    za: f64,
    pot: Option<&'a Potential>,
}

fn rk4_step(r: f64, r_new: f64, p: f64, q: f64, o: &Ode) -> (f64, f64) {
    // derivative w.r.t. x = ln r of (P, Q); radial Dirac equations (Greiner &
    // Mueller 1987 §4): dP/dr = -kappa/r P + (E-V+m) Q,
    //                          dQ/dr =  kappa/r Q - (E-V-m) P
    let (kappa, e, m, za) = (o.kappa, o.e, o.m, o.za);
    let deriv = |rr: f64, pp: f64, qq: f64| -> (f64, f64) {
        let v = match o.pot {
            Some(t) => t.full_at(rr),
            None => -za * HBARC / rr,
        };
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

    const M_E: f64 = 0.510_998_95; // electron, MeV (CODATA 2022)
    const M_MU: f64 = 105.658_375_5; // muon, MeV (CODATA 2022)

    fn rel_err(num: f64, exact: f64) -> f64 {
        ((num - exact) / exact).abs()
    }

    /// outer domain edge for test potentials (covers r_max of these levels)
    fn r_out(z: u32, m: f64, n: u32) -> f64 {
        HBARC / (z as f64 * ALPHA * m) * (n * n) as f64 * 51.0
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

    // ---- finite nuclear size ------------------------------------------------

    /// Leading-order perturbation theory shift of an ns level (Bethe &
    /// Salpeter 1957 §53; French 1949): dE = (2/3)(Z a)^3 a * (Z a m)^3/(n^3)
    /// * <r^2> = (2/3)(Z a)^4 m^3 <r^2> / n^3, natural units.
    fn lo_shift_mev(z: u32, m: f64, n: u32, r2_fm2: f64) -> f64 {
        let za = z as f64 * ALPHA;
        let r2_nat = r2_fm2 / (HBARC * HBARC);
        (2.0 / 3.0) * za.powi(4) * m.powi(3) * r2_nat / (n * n * n) as f64
    }

    #[test]
    fn finite_size_shift_matches_leading_order_pt() {
        // Muonic hydrogen 1s on a uniform sphere R = 0.3 fm, first-order PT
        // with the exact Dirac density against the leading-order formula
        // (Bethe & Salpeter 1957 §53; French 1949). R/a = 1.2e-3 makes
        // higher orders in (R/a) < 1e-5 relative, and relativistic
        // corrections to the LO formula are O((Z alpha)^2) = 5e-5.
        //
        // (Outward shooting cannot resolve meV-scale shifts at all: at the
        // eigenvalue P(r_max) ~ e^-47 for this level while the RK4 noise is
        // ~1e-16 * e^+47 - the shooting root carries an O(eV) uncertainty.
        // expectation_shift avoids the contamination region entirely.)
        let r = 0.3f64;
        let pot = build_potential(
            1,
            ChargeDist::Uniform { r_fermi: r },
            None,
            r_out(1, M_MU, 1),
        );
        let shift = expectation_shift(1, M_MU, 1, -1, &pot, 160_000);
        let expected = lo_shift_mev(1, M_MU, 1, 3.0 / 5.0 * r * r); // <r^2> = 3R^2/5
        assert!(shift > 0.0, "finite size must reduce binding, got {shift}");
        assert!(
            rel_err(shift, expected) < 0.02,
            "shift={shift:.3e} expected={expected:.3e}"
        );
    }

    #[test]
    fn muonic_hydrogen_2s_finite_size_coefficient() {
        // Literature anchor: the finite-size contribution to the muonic
        // hydrogen 2S-2P Lamb shift is -5.227 meV/fm^2 * <r_p^2> (reduced
        // muon mass), Pohl & Antognini, Ann. Rev. Nucl. Part. Sci. 63 (2013)
        // 343, eq. (3.13). With the infinite-mass muon used here it scales by
        // (105.658/95.02)^3 = 1.375 -> 7.19 meV/fm^2; the LO formula itself
        // gives 7.15 meV/fm^2.
        let rp = 0.8407f64; // proton rms charge radius, fm (CODATA 2022)
        let r = (5.0 / 3.0f64).sqrt() * rp; // uniform sphere, same <r^2>
        let pot = build_potential(
            1,
            ChargeDist::Uniform { r_fermi: r },
            None,
            r_out(1, M_MU, 2),
        );
        let shift = expectation_shift(1, M_MU, 2, -1, &pot, 160_000);
        // 1 meV = 1e-9 MeV
        let coeff_mev_per_fm2 = shift * 1e9 / (rp * rp); // meV/fm^2
        assert!(
            (coeff_mev_per_fm2 - 7.15).abs() < 0.15,
            "coefficient {coeff_mev_per_fm2} meV/fm^2 vs 7.15"
        );
        // and against the analytic LO formula (identical machinery check):
        let analytic = lo_shift_mev(1, M_MU, 2, rp * rp) * 1e9;
        assert!(
            rel_err(shift * 1e9, analytic) < 0.02,
            "num={} analytic={analytic}",
            shift * 1e9
        );
    }

    #[test]
    fn two_pf_rms_solver_reproduces_tabulated_pb208() {
        // Pb-208 rms charge radius 5.5009 fm (Angeli & Marinova, ADT 101
        // (2013) 185). The solver must invert c(a=0.52) to machine accuracy.
        let target = 5.5009f64;
        let c = solve_two_pf(target, TWO_PF_A_FM);
        assert!((fermi_rms(c, TWO_PF_A_FM) - target).abs() < 1e-6);
        // cross-check: Fricke et al. (1995) tabulate c ~ 6.68 fm for Pb-208
        assert!((c - 6.68).abs() < 0.05, "c={c}");
    }

    #[test]
    fn finite_potential_matches_coulomb_outside_nucleus() {
        let pot = build_potential(
            82,
            ChargeDist::Fermi2p {
                c_fermi: 6.68,
                a_fermi: 0.52,
            },
            None,
            400.0,
        );
        let v = pot.full_at(30.0);
        let pt = -82.0 * ALPHA * HBARC / 30.0;
        assert!(rel_err(v, pt) < 2e-3, "v={v} pt={pt}");
        // deviation vanishes outside the charge
        assert!(pot.at(30.0).abs() < 2e-3 * pt.abs(), "dv={}", pot.at(30.0));
        // uniform sphere interior: V(r) = -Za*hbarc*(3R^2-r^2)/(2R^3)
        // (Bethe & Salpeter §53) - checked at r = R/10 where reconstructing
        // V = point + dV does not cancel orders of magnitude
        let pot = build_potential(82, ChargeDist::Uniform { r_fermi: 7.0 }, None, 400.0);
        let r = 0.7f64;
        let v_in = -82.0 * ALPHA * HBARC * (3.0 * 49.0 - r * r) / (2.0 * 343.0);
        let v0 = pot.full_at(r);
        assert!(rel_err(v0, v_in) < 1e-4, "v0={v0} v_in={v_in}");
    }

    #[test]
    fn heavy_muonic_finite_nucleus_reduces_binding() {
        // Pb-208 2pF (c from the Angeli radius via the solver). The 1s
        // finite-size shift of muonic lead is one of the largest known
        // (tens of percent of the point binding; cf. Engel et al., At. Data
        // Nucl. Data Tables 16 (1975) 413, muonic X-ray data set).
        let c = solve_two_pf(5.5009, TWO_PF_A_FM);
        let pot = build_potential(
            82,
            ChargeDist::Fermi2p {
                c_fermi: c,
                a_fermi: TWO_PF_A_FM,
            },
            None,
            r_out(82, M_MU, 1),
        );
        let e_fs = solve_level_opt(82, M_MU, 1, -1, 160_000, Some(&pot));
        let e_pt = hydrogenic_energy(82, M_MU, 1, -1);
        let shift_rel = (e_fs - e_pt) / (M_MU - e_pt);
        assert!(
            shift_rel > 0.02 && shift_rel < 0.5,
            "unexpected 1s shift fraction {shift_rel}"
        );
    }

    // ---- Uehling vacuum polarization ----------------------------------------

    #[test]
    fn uehling_potential_small_r_asymptotic() {
        // For 2 m_e r << 1 the integral -> ln(1/(2 m_e r)) - gamma
        // (Uehling 1935); check at r = 1 fm where the asymptotic is < 2 % off.
        let pot = build_potential(1, ChargeDist::Point, Some(M_E), 20_000.0);
        let v_u = pot.at(1.0); // the deviation IS the Uehling term here
        let b = 2.0 * M_E / HBARC; // 2 m_e r at r=1 fm, natural units
        let approx =
            -(2.0 * ALPHA * ALPHA / (3.0 * std::f64::consts::PI)) * (-b.ln() - 0.577_21) * HBARC;
        assert!(v_u < 0.0, "Uehling must be attractive, got {v_u}");
        assert!(
            rel_err(v_u, approx) < 0.05,
            "v_u={v_u:.6} approx={approx:.6}"
        );
    }

    #[test]
    fn uehling_2s_electronic_hydrogen_matches_published() {
        // Strongest external anchor for the Uehling machinery: the VP shift
        // of the ordinary hydrogen 2S level is -1.122e-7 eV (Wikipedia,
        // "Uehling potential", quoting Greiner & Reinhardt, "Quantum
        // Electrodynamics"; original Uehling 1935; the same value follows
        // from the Kallen-Pauli -27 MHz). First-order PT with the exact
        // Dirac 2S density; Wichmann-Kroll and two-loop VP are < 1 %.
        let pot = build_potential(1, ChargeDist::Point, Some(M_E), r_out(1, M_E, 2));
        let shift = expectation_shift(1, M_E, 2, -1, &pot, 160_000);
        let published = -1.122e-13; // eV -> MeV
        assert!(shift < 0.0, "Uehling must increase binding, got {shift}");
        assert!(
            rel_err(shift, published) < 0.03,
            "shift={:.4e} MeV vs published {published:.4e} MeV",
            shift
        );
    }

    #[test]
    fn uehling_2s_muonic_hydrogen_magnitude() {
        // Muonic hydrogen 2S: the VP range 1/(2 m_e) = 193 fm is comparable
        // to the 2S Bohr radius (256 fm), so the muon orbits deep inside the
        // polarization cloud and the shift is meV-scale - five orders above
        // the electronic value. (The sub-meV number often quoted for muonic
        // hydrogen is the 2S-2P Lamb-shift contribution, where the 2S and
        // 2P matrix elements largely cancel; Pohl & Antognini, ARNPS 63
        // (2013) 343.) Cross-checked against an independent nonrelativistic
        // quadrature of the same potential - the absolute muonic magnitude
        // is deliberately NOT anchored to a recalled literature value.
        let pot = build_potential(1, ChargeDist::Point, Some(M_E), r_out(1, M_MU, 2));
        let shift = expectation_shift(1, M_MU, 2, -1, &pot, 160_000);
        assert!(shift < 0.0, "Uehling attractive: {shift}");
        // independent nonrel quadrature with psi_200 = (2 - r/a) e^{-r/2a}
        // / sqrt(32 pi a^3) (standard hydrogenic 2S, e.g. Bethe & Salpeter)
        let a = HBARC / (ALPHA * M_MU);
        let n_int = 400_000usize;
        let (r_lo, r_hi) = (1e-3f64, 40.0 * a);
        let dr = (r_hi - r_lo) / n_int as f64;
        let norm_psi = 1.0 / (32.0 * std::f64::consts::PI * a.powi(3));
        let mut q = 0.0f64;
        for i in 0..n_int {
            let rm = r_lo + (i as f64 + 0.5) * dr;
            let y = rm / a;
            let psi2 = norm_psi * (2.0 - y) * (2.0 - y) * (-y).exp();
            q += psi2 * uehling(ALPHA, rm, M_E) * 4.0 * std::f64::consts::PI * rm * rm * dr;
        }
        assert!(
            rel_err(shift, q) < 0.05,
            "PT={shift:.4e} MeV quadrature={q:.4e} MeV"
        );
    }

    #[test]
    fn finite_size_and_uehling_have_opposite_sign() {
        // Textbook: vacuum polarization screens the charge (more bound), the
        // finite size spreads it (less bound). Muonic hydrogen 2S, first-
        // order PT (expectation_shift) - robust for both tiny shifts.
        let rp = 0.8407f64;
        let r = (5.0 / 3.0f64).sqrt() * rp;
        let pot_fs = build_potential(
            1,
            ChargeDist::Uniform { r_fermi: r },
            None,
            r_out(1, M_MU, 2),
        );
        let pot_u = build_potential(1, ChargeDist::Point, Some(M_E), r_out(1, M_MU, 2));
        let e_fs = expectation_shift(1, M_MU, 2, -1, &pot_fs, 160_000);
        let e_u = expectation_shift(1, M_MU, 2, -1, &pot_u, 160_000);
        assert!(e_fs > 0.0 && e_u < 0.0, "fs={e_fs} ueh={e_u}");
    }

    #[test]
    fn reduced_mass_matches_definition() {
        // mu = m M/(m+M); muonic hydrogen: 94.9647 MeV (CODATA 2022 masses:
        // m_mu = 105.6583755, m_p = 938.27208816)
        let m_p = 938.272_088_16;
        let mu = reduced_mass(M_MU, m_p);
        assert!((mu - 94.964_7).abs() < 0.001, "mu={mu}");
    }
}
