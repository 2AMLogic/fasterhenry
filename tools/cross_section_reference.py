#!/usr/bin/env python3
"""Independent two-dimensional skin-effect reference for a rectangular trace.

Solves the magneto-quasi-static current distribution in the cross-section of
an infinitely long, straight, isolated rectangular conductor and writes the
per-unit-length internal impedance ``z'(f) = R'(f) + jω·ΔL'(f)`` as JSON. It
is the oracle for ``fasterhenry/tests/width_graded_skin_validation.rs``
(issue #32): unlike the one-dimensional slab formula it resolves the current
crowding toward the *edges* of a finite-width trace, which is exactly what
width-graded filament grids are meant to capture.

Physics (nothing here is shared with the fasterhenry crate)
-----------------------------------------------------------

For a conductor that is uniform along ``z`` and carries a total current ``I``
along ``z``, with no other conductors present, the axial field obeys

    J(x, y) / σ + jω A(x, y) = V'            (inside the conductor)
    A(x, y) = -(μ0 / 2π) ∬ J(x', y') ln|r - r'| dx' dy'

where ``V'`` is the (uniform) voltage drop per unit length and ``A`` the
axial vector potential of the two-dimensional free-space Green's function.
This is the exact quasi-static problem (displacement current neglected, as
in FastHenry); it contains the diffusion equation ``∇²J = jωμ0σJ`` inside the
conductor together with the open-space exterior field, so no artificial
outer boundary is needed.

Discretization: piecewise-constant ``J`` on a tensor-product mesh of
rectangular cells, Galerkin (cell-averaged) testing. The cell-pair
coefficients ``(1/(AᵢAⱼ)) ∬∬ ln|r - r'|`` are evaluated in closed form from
the fourth antiderivative

    F(u, v) = (6u²v² − u⁴ − v⁴)/48 · ln(u² + v²)
              + (u³v·atan(v/u) + uv³·atan(u/v))/6 − 25u²v²/48,

    ∂⁴F / ∂u²∂v² = ln √(u² + v²),

(verified symbolically; see ``_self_test``) for nearby pairs, and from the
second-order moment expansion of the harmonic kernel for well-separated
pairs, where the closed form would lose digits to cancellation. The two
mirror symmetries of the rectangle (``J`` is even in ``x`` and ``y``) reduce
the unknowns to one quadrant.

The mesh is *not* fasterhenry's geometric progression: cell faces follow a
blended sine map that clusters cells toward the edges and faces. Each
reference value is computed on four successively doubled meshes (``--levels``,
at least three), Richardson-extrapolated, and reported with its convergence
estimate (finest value, the change from the previous level, and the observed
order), so a consumer can see how well the oracle itself is converged before
trusting a comparison against it.

Output quantities, per unit length, for each frequency:

* ``r_ohm_per_m``      — ``R'(f)``
* ``dl_h_per_m``       — ``ΔL'(f) = X'(f)/ω − L'_DC``, the change of the
  internal inductance from its uniform-current (DC) value. The absolute
  2D inductance carries an arbitrary additive constant (the log kernel's
  reference length); the *change* is physical and independent of it.

Usage::

    python3 tools/cross_section_reference.py --out target/cross_section_reference.json

Only numpy is required. Default settings take about half a minute on a laptop.
"""

from __future__ import annotations

import argparse
import json
import math
import time

import numpy as np

MU0 = 4.0e-7 * math.pi

# ---------------------------------------------------------------------------
# The fixture. Must match `wide_strip()` in
# fasterhenry/tests/width_graded_skin_validation.rs.
# ---------------------------------------------------------------------------

WIDTH = 5.0e-3  # m
THICKNESS = 200.0e-6  # m
SIGMA = 5.8e7  # S/m, copper
# t/δ values: weak (δ = 3.3 t) through strong (δ = t/10) skin effect.
THICKNESS_PER_DELTA = [0.3, 1.0, 3.0, 10.0]

# Quadrant cell counts (across the half-width, across the half-thickness) of
# the coarsest mesh; each further level doubles both.
BASE_QUADRANT = (16, 4)
LEVELS = 4
# Share of the uniform map in the blended sine map (0 = pure sine clustering).
BLEND = 0.25
# Pairs whose centre distance exceeds this multiple of the larger cell
# extent use the moment expansion instead of the closed form.
FAR_FACTOR = 12.0


def frequency_for_delta(delta: float, sigma: float = SIGMA) -> float:
    """Frequency at which the skin depth sqrt(2/(ω μ0 σ)) equals `delta`."""
    return 1.0 / (math.pi * MU0 * sigma * delta * delta)


# ---------------------------------------------------------------------------
# The kernel
# ---------------------------------------------------------------------------


def _antiderivative(u: np.ndarray, v: np.ndarray) -> np.ndarray:
    """F(u, v) with ∂⁴F/∂u²∂v² = ln √(u² + v²); even in u and in v."""
    u = np.abs(u)
    v = np.abs(v)
    u2, v2 = u * u, v * v
    r2 = u2 + v2
    with np.errstate(divide="ignore", invalid="ignore"):
        log_term = np.where(r2 > 0.0, (6.0 * u2 * v2 - u2 * u2 - v2 * v2) / 48.0 * np.log(r2), 0.0)
        atan_vu = np.where(u > 0.0, u2 * u * v * np.arctan(v / u), 0.0)
        atan_uv = np.where(v > 0.0, u * v2 * v * np.arctan(u / v), 0.0)
    return log_term + (atan_vu + atan_uv) / 6.0 - 25.0 / 48.0 * u2 * v2


def _interval_terms(a0, a1, b0, b1):
    """The four endpoint differences and signs of ∫_a ∫_b g(x − x')."""
    return ((a1 - b0, 1.0), (a0 - b1, 1.0), (a1 - b1, -1.0), (a0 - b0, -1.0))


def mean_log_exact(xi, yi, xj, yj):
    """(1/(AᵢAⱼ)) ∬∬ ln|r − r'| over cells i and j, in closed form.

    Each argument is a pair ``(lo, hi)`` of broadcastable arrays.
    """
    total = 0.0
    for u, su in _interval_terms(xi[0], xi[1], xj[0], xj[1]):
        for v, sv in _interval_terms(yi[0], yi[1], yj[0], yj[1]):
            total = total + (su * sv) * _antiderivative(u, v)
    area = (xi[1] - xi[0]) * (yi[1] - yi[0]) * (xj[1] - xj[0]) * (yj[1] - yj[0])
    return total / area


def mean_log_far(xi, yi, xj, yj):
    """Second-order moment expansion of the same average, for far pairs.

    ln r is harmonic, so the average over two independent uniform cells is
    ln R + f_xx·(σx² − σy²)/2 with f_xx = (Y² − X²)/R⁴ and σx² the variance
    of the x-offset, (aᵢ² + aⱼ²)/12. The next term is O((extent/R)⁴).
    """
    dx = 0.5 * (xi[0] + xi[1] - xj[0] - xj[1])
    dy = 0.5 * (yi[0] + yi[1] - yj[0] - yj[1])
    r2 = dx * dx + dy * dy
    ax2 = (xi[1] - xi[0]) ** 2 + (xj[1] - xj[0]) ** 2
    ay2 = (yi[1] - yi[0]) ** 2 + (yj[1] - yj[0]) ** 2
    return 0.5 * np.log(r2) + (dy * dy - dx * dx) / (r2 * r2) * (ax2 - ay2) / 24.0


def mean_log(xi, yi, xj, yj):
    """Cell-pair average of ln|r − r'|, choosing the accurate evaluation."""
    dx = 0.5 * (xi[0] + xi[1] - xj[0] - xj[1])
    dy = 0.5 * (yi[0] + yi[1] - yj[0] - yj[1])
    extent = np.maximum(
        np.maximum(xi[1] - xi[0], yi[1] - yi[0]),
        np.maximum(xj[1] - xj[0], yj[1] - yj[0]),
    )
    shape = np.broadcast_shapes(dx.shape, dy.shape, extent.shape)
    far = np.broadcast_to(np.hypot(dx, dy) > FAR_FACTOR * extent, shape)
    result = np.empty(shape)
    b = lambda a: np.broadcast_to(a, shape)  # noqa: E731
    xi_, yi_, xj_, yj_ = ((b(p[0]), b(p[1])) for p in (xi, yi, xj, yj))
    sel = lambda pair, m: (pair[0][m], pair[1][m])  # noqa: E731
    result[far] = mean_log_far(sel(xi_, far), sel(yi_, far), sel(xj_, far), sel(yj_, far))
    near = ~far
    result[near] = mean_log_exact(sel(xi_, near), sel(yi_, near), sel(xj_, near), sel(yj_, near))
    return result


# ---------------------------------------------------------------------------
# The mesh and the solve
# ---------------------------------------------------------------------------


def faces(half_extent: float, count: int) -> np.ndarray:
    """`count + 1` faces on [0, half_extent], clustered toward half_extent.

    s ↦ half_extent·(BLEND·s + (1 − BLEND)·sin(πs/2)): the sine term has zero
    slope at s = 1, so the cells shrink quadratically toward the surface.
    """
    s = np.linspace(0.0, 1.0, count + 1)
    return half_extent * (BLEND * s + (1.0 - BLEND) * np.sin(0.5 * math.pi * s))


class Quadrant:
    """Cells of the x ≥ 0, y ≥ 0 quadrant and their symmetrized kernel."""

    def __init__(self, width: float, thickness: float, nx: int, ny: int):
        fx, fy = faces(0.5 * width, nx), faces(0.5 * thickness, ny)
        x0, y0 = np.meshgrid(fx[:-1], fy[:-1], indexing="ij")
        x1, y1 = np.meshgrid(fx[1:], fy[1:], indexing="ij")
        self.x = (x0.ravel(), x1.ravel())
        self.y = (y0.ravel(), y1.ravel())
        self.area = (self.x[1] - self.x[0]) * (self.y[1] - self.y[0])
        self.count = self.area.size
        self.full_area = width * thickness

    def kernel(self, chunk: int = 256) -> np.ndarray:
        """Σ over the four mirror images of −(μ0/2π)·⟨ln|r − r'|⟩, in H/m."""
        n = self.count
        k = np.zeros((n, n))
        xj, yj = self.x, self.y
        images = [
            (xj, yj),
            ((-xj[1], -xj[0]), yj),
            (xj, (-yj[1], -yj[0])),
            ((-xj[1], -xj[0]), (-yj[1], -yj[0])),
        ]
        for start in range(0, n, chunk):
            rows = slice(start, min(start + chunk, n))
            xi = (self.x[0][rows, None], self.x[1][rows, None])
            yi = (self.y[0][rows, None], self.y[1][rows, None])
            for (ix, iy) in images:
                k[rows] += mean_log(xi, yi, (ix[0][None, :], ix[1][None, :]), (iy[0][None, :], iy[1][None, :]))
        return -MU0 / (2.0 * math.pi) * k

    def solve(self, sigma: float, frequencies: list[float]):
        """(R', ΔL') per unit length at each frequency, and L'_DC."""
        k = self.kernel()
        resistance = 1.0 / (sigma * self.area)  # Ω/m of each cell
        # Uniform current (DC): every cell carries I·Aᵢ/A of the total, four
        # images per quadrant cell.
        share = self.area / self.full_area
        l_dc = 4.0 * share @ k @ share
        out = []
        for f in frequencies:
            omega = 2.0 * math.pi * f
            m = 1j * omega * k
            m[np.diag_indices_from(m)] += resistance
            current = np.linalg.solve(m, np.ones(self.count, dtype=complex))
            z = 1.0 / (4.0 * current.sum())  # V' = 1 V/m over the total
            out.append((z.real, z.imag / omega - l_dc))
        return out, l_dc


def _self_test() -> None:
    """Closed form vs. brute-force quadrature, and far vs. exact."""
    rng = np.random.default_rng(1)
    g, w = np.polynomial.legendre.leggauss(24)

    def brute(xi, yi, xj, yj):
        def pts(lo, hi):
            return 0.5 * (hi - lo) * g + 0.5 * (hi + lo), 0.5 * w
        (px, wx), (py, wy) = pts(*xi), pts(*yi)
        (qx, vx), (qy, vy) = pts(*xj), pts(*yj)
        X = px[:, None, None, None] - qx[None, None, :, None]
        Y = py[None, :, None, None] - qy[None, None, None, :]
        W = wx[:, None, None, None] * wy[None, :, None, None] * vx[None, None, :, None] * vy[None, None, None, :]
        return float((W * 0.5 * np.log(X * X + Y * Y)).sum())

    for _ in range(5):
        xi = np.sort(rng.uniform(0, 1, 2))
        yi = np.sort(rng.uniform(0, 1, 2))
        xj = np.sort(rng.uniform(2, 3, 2))
        yj = np.sort(rng.uniform(-1, 1, 2))
        exact = float(mean_log_exact(xi, yi, xj, yj))
        assert abs(exact - brute(xi, yi, xj, yj)) < 1e-9, (exact, brute(xi, yi, xj, yj))
    # The self term, where brute-force quadrature does not converge, against
    # Maxwell's geometric mean distance of a square from itself, 0.44705 a.
    sq = (np.array(0.0), np.array(1.0))
    gmd_square = math.exp(float(mean_log_exact(sq, sq, sq, sq)))
    assert abs(gmd_square - 0.447049) < 1e-6, gmd_square
    # Moment expansion at the switch-over distance.
    xi, yi = (np.array(0.0), np.array(1.0)), (np.array(0.0), np.array(0.3))
    xj, yj = (np.array(FAR_FACTOR), np.array(FAR_FACTOR + 1.0)), (np.array(2.0), np.array(2.2))
    err = abs(float(mean_log_far(xi, yi, xj, yj) - mean_log_exact(xi, yi, xj, yj)))
    assert err < 1e-6, err


def richardson(series: list[float]) -> dict:
    """Second-order Richardson extrapolation of the last two mesh levels.

    Refuses (raises) unless the order observed over the last three levels is
    close to two: a reference outside its asymptotic range must not be
    written, because its uncertainty estimate would be meaningless.
    """
    a, b, c = series[-3:]
    if not (a != b != c):
        raise SystemExit(f"degenerate refinement series {series}")
    ratio = (a - b) / (b - c)
    order = math.log2(ratio) if ratio > 0 else float("nan")
    if not 1.6 <= order <= 2.5:
        raise SystemExit(f"refinement series {series} is not second order (observed {order:.3f})")
    extrapolated = c + (c - b) / 3.0
    return {
        "value": extrapolated,
        "finest": c,
        "series": series,
        "observed_order": order,
        # The whole last refinement step, about three times the Richardson
        # correction actually applied: a deliberately conservative bound.
        "uncertainty": abs(c - b) / abs(extrapolated),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", default="target/cross_section_reference.json")
    parser.add_argument("--levels", type=int, default=LEVELS)
    parser.add_argument("--base-nx", type=int, default=BASE_QUADRANT[0])
    parser.add_argument("--base-ny", type=int, default=BASE_QUADRANT[1])
    args = parser.parse_args()
    if args.levels < 3:
        parser.error("at least three levels are needed to observe the convergence order")

    _self_test()
    frequencies = [frequency_for_delta(THICKNESS / p) for p in THICKNESS_PER_DELTA]

    # The whole cross-section's mean log distance in closed form, against
    # which every mesh's uniform-current (DC) inductance is checked: the cells
    # tile the rectangle, so the two agree up to the far-pair expansion.
    whole_x = (np.array(-0.5 * WIDTH), np.array(0.5 * WIDTH))
    whole_y = (np.array(-0.5 * THICKNESS), np.array(0.5 * THICKNESS))
    l_dc_exact = -MU0 / (2.0 * math.pi) * float(mean_log_exact(whole_x, whole_y, whole_x, whole_y))

    levels = []
    for level in range(args.levels):
        nx, ny = args.base_nx << level, args.base_ny << level
        started = time.time()
        values, l_dc = Quadrant(WIDTH, THICKNESS, nx, ny).solve(SIGMA, frequencies)
        elapsed = time.time() - started
        dc_error = abs(l_dc - l_dc_exact) / abs(l_dc_exact)
        print(f"quadrant {nx:4d} x {ny:3d} ({4 * nx * ny:6d} cells, {elapsed:6.1f} s), "
              f"L'_DC vs closed form {dc_error:.1e}")
        if dc_error > 1e-7:
            raise SystemExit(f"mesh DC inductance is {dc_error:.2e} off the closed form")
        for p, (r, dl) in zip(THICKNESS_PER_DELTA, values):
            print(f"   t/δ = {p:5.1f}  R' = {r:.7e} Ω/m  ΔL' = {dl:.7e} H/m")
        levels.append(values)

    points = []
    for index, (p, f) in enumerate(zip(THICKNESS_PER_DELTA, frequencies)):
        r = richardson([lv[index][0] for lv in levels])
        dl = richardson([lv[index][1] for lv in levels])
        points.append({"thickness_per_delta": p, "frequency_hz": f, "r_ohm_per_m": r, "dl_h_per_m": dl})
        print(
            f"t/δ = {p:5.1f}: R' = {r['value']:.6e} ± {100 * r['uncertainty']:.3f} % "
            f"(order {r['observed_order']:.2f}),  ΔL' = {dl['value']:.6e} ± {100 * dl['uncertainty']:.3f} % "
            f"(order {dl['observed_order']:.2f})"
        )

    result = {
        "generator": "tools/cross_section_reference.py",
        "width_m": WIDTH,
        "thickness_m": THICKNESS,
        "sigma_s_per_m": SIGMA,
        "r_dc_ohm_per_m": 1.0 / (SIGMA * WIDTH * THICKNESS),
        "l_dc_h_per_m": l_dc_exact,
        "mesh": {
            "quadrant_cells": [[args.base_nx << k, args.base_ny << k] for k in range(args.levels)],
            "blend": BLEND,
            "far_factor": FAR_FACTOR,
        },
        "points": points,
    }
    with open(args.out, "w") as stream:
        json.dump(result, stream, indent=2)
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
