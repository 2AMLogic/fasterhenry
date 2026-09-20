#!/usr/bin/env python3
"""Independent PyPEEC reference for the fasterhenry validation fixture.

Builds the M0 validation fixture — the 2-turn square spiral also used by
klayout-tools (docs/design/mom-validation.md §5) — as a PyPEEC voxel
geometry (1 µm voxels), solves it with a 1 A current source across the
terminals at DC and at 1 kHz, and writes the terminal impedance as JSON:

    {"pypeec_version": ..., "r_ohm": ..., "l_h": ..., ...}

Usage:
    python3 tools/pypeec_reference.py [--out OUT.json] [--voxel-um 1.0]

The fasterhenry test `tests/spiral_validation.rs` reads that JSON and
asserts agreement within 2 % on L. CI regenerates it (nothing generated is
committed).

PyPEEC is MPL-2.0 (Thomas Guillod, Dartmouth College); this script is an
independent consumer of its public solver API.
"""

from __future__ import annotations

import argparse
import json
import time

import numpy as np

import pypeec
from pypeec.run import mesher, solver

# ---------------------------------------------------------------------------
# The fixture: identical to klayout-tools' square_spiral_segments(2, 60, 15).
# ---------------------------------------------------------------------------


def square_spiral_segments(turns: int, start_len_um: float, pitch_um: float):
    """Right-angle square spiral centrelines, in winding order (µm)."""
    directions = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)]
    pos = (0.0, 0.0)
    length = start_len_um
    segments = []
    for k in range(turns * 4):
        direction = directions[k % 4]
        end = (pos[0] + direction[0] * length, pos[1] + direction[1] * length)
        segments.append((pos, end))
        pos = end
        if k % 2 == 1:
            length += pitch_um
    return segments


W_UM = 2.0  # trace width
T_UM = 2.0  # trace thickness
SIGMA = 5.8e7  # copper, S/m
FREQ_AC = 1.0e3  # low-frequency solve for L (skin depth 66 µm >> cross-section)


def build_geometry(voxel_um: float) -> dict:
    segments = square_spiral_segments(2, 60.0, 15.0)
    w = W_UM
    rects = []  # (x0, x1, y0, y1) in µm, conductor footprints
    for (x0, y0), (x1, y1) in segments:
        if abs(x1 - x0) > abs(y1 - y0):  # horizontal
            rects.append((min(x0, x1) - w / 2, max(x0, x1) + w / 2, y0 - w / 2, y0 + w / 2))
        else:  # vertical
            rects.append((x0 - w / 2, x0 + w / 2, min(y0, y1) - w / 2, max(y0, y1) + w / 2))

    # Voxel centres sit at half-integer multiples of the voxel size, spanning
    # the metal exactly: with integer-µm segment coordinates and w = 2 µm,
    # a 1 µm voxel pitch puts two full columns inside every trace and the
    # boundary-inclusiveness ambiguity disappears.
    i0, i1 = int(np.floor(min(r[0] for r in rects))), int(np.ceil(max(r[1] for r in rects)))
    j0, j1 = int(np.floor(min(r[2] for r in rects))), int(np.ceil(max(r[3] for r in rects)))
    nx, ny = i1 - i0, j1 - j0
    nz = max(1, int(round(T_UM / voxel_um)))

    def centre(along: int, base: int) -> float:
        return base + (along + 0.5) * voxel_um

    idx = {"src": [], "sink": [], "wire": []}
    r_src = (0.0, 0.0)  # spiral start
    r_sink = segments[-1][1]  # spiral end
    r_pad = 2.1 * voxel_um
    for j in range(ny):
        for i in range(nx):
            x = centre(i, i0)
            y = centre(j, j0)
            near_src = np.hypot(x - r_src[0], y - r_src[1]) <= r_pad
            near_sink = np.hypot(x - r_sink[0], y - r_sink[1]) <= r_pad
            in_wire = any(r[0] <= x <= r[1] and r[2] <= y <= r[3] for r in rects)
            if near_src and in_wire:
                tag = "src"
            elif near_sink and in_wire:
                tag = "sink"
            elif in_wire:
                tag = "wire"
            else:
                continue
            for k in range(nz):
                idx[tag].append(i + nx * j + nx * ny * k)

    return {
        "mesh_type": "voxel",
        "data_voxelize": {
            "param": {
                "n": [nx, ny, nz],
                "d": [voxel_um * 1e-6, voxel_um * 1e-6, T_UM * 1e-6 / nz],
                "c": [
                    (i0 + nx * voxel_um / 2) * 1e-6,
                    (j0 + ny * voxel_um / 2) * 1e-6,
                    T_UM * 1e-6 / 2,
                ],
            },
            "domain_index": idx,
        },
        "data_point": {"check_cloud": False, "filter_cloud": False, "pts_cloud": []},
        "data_resampling": {
            "use_reduce": False,
            "use_resample": False,
            "resampling_factor": [1, 1, 1],
        },
        "data_conflict": {"resolve_rules": False, "resolve_random": False, "conflict_rules": []},
        "data_integrity": {
            "check_integrity": True,
            "domain_connected": {
                "conductor": {"domain_group": [["src", "wire", "sink"]], "connected": True},
            },
            "domain_adjacent": {},
        },
    }


TOLERANCE = {
    "parallel_sweep": {"n_jobs": 0, "n_threads": None},
    "integral_simplify": 20.0,
    "biot_savart": "face",
    "dense_options": {
        "method": "fft",
        "split": True,
        "fft_options": {
            "library": "SciPy",
            "scipy_worker": -1,
            "fftw_thread": -1,
            "fftw_cache": True,
            "fftw_timeout": 100.0,
            "fftw_byte_align": 16,
        },
    },
    "factorization_options": {
        "schur": True,
        "library": "SuperLU",
        "pyamg_options": {"tol": 1.0e-6, "solver": "root", "krylov": None},
        "pardiso_options": {"thread_pardiso": -1, "thread_mkl": -1},
    },
    "solver_options": {
        "coupling": "direct",
        "status_options": {
            "ignore_status": False,
            "ignore_res": True,
            "rel_tol": 1.0e-3,
            "abs_tol": 1.0e-9,
        },
        "power_options": {
            "stop": True,
            "n_min": 4,
            "n_cmp": 3,
            "rel_tol": 1.0e-4,
            "abs_tol": 1.0e-10,
        },
        "direct_options": {
            "solver": "gmres",
            "rel_tol": 1.0e-6,
            "abs_tol": 1.0e-12,
            "n_inner": 20,
            "n_outer": 20,
        },
        "segregated_options": {
            "rel_tol": 1.0e-6,
            "abs_tol": 1.0e-12,
            "relax_electric": 1.0,
            "relax_magnetic": 1.0,
            "n_min": 2,
            "n_max": 20,
            "iter_electric_options": {
                "solver": "gmres",
                "rel_tol": 1.0e-6,
                "abs_tol": 1.0e-12,
                "n_inner": 20,
                "n_outer": 20,
            },
            "iter_magnetic_options": {
                "solver": "gmres",
                "rel_tol": 1.0e-6,
                "abs_tol": 1.0e-12,
                "n_inner": 20,
                "n_outer": 20,
            },
        },
    },
    "condition_options": {
        "check": True,
        "tolerance_electric": 1.0e15,
        "tolerance_magnetic": 1.0e15,
        "norm_options": {"t_accuracy": 2, "n_iter_max": 25},
    },
}


def problem(rho):
    material_val = {"copper": {"rho_re": rho, "rho_im": 0.0}}
    source_val = {
        "src": {"I_re": 1.0, "I_im": 0.0, "Y_re": 0.0, "Y_im": 0.0},
        "sink": {"V_re": 0.0, "V_im": 0.0, "Z_re": 0.0, "Z_im": 0.0},
    }
    return {
        "material_def": {
            "copper": {
                "domain_list": ["src", "wire", "sink"],
                "material_type": "electric",
                "orientation_type": "isotropic",
                "var_type": "lumped",
            },
        },
        "source_def": {
            "src": {
                "domain_list": ["src"],
                "source_type": "current",
                "var_type": "lumped",
            },
            "sink": {
                "domain_list": ["sink"],
                "source_type": "voltage",
                "var_type": "lumped",
            },
        },
        "material_val": material_val,
        "source_val": source_val,
        "sweep_solver": {
            "sim_dc": {
                "init": None,
                "param": {"freq": 0.0, "material_val": material_val, "source_val": source_val},
            },
            "sim_ac": {
                "init": "sim_dc",
                "param": {"freq": FREQ_AC, "material_val": material_val, "source_val": source_val},
            },
        },
    }


def impedance(data_solution, tag: str) -> complex:
    sweep = data_solution["data_sweep"][tag]
    if not sweep["solution_ok"]:
        raise RuntimeError(f"pypeec sweep {tag} did not converge: {sweep['solver_status']}")
    values = sweep["source_values"]["src"]
    return complex(values["V"]) / complex(values["I"])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default="tools/pypeec_reference.json")
    parser.add_argument("--voxel-um", type=float, default=1.0)
    args = parser.parse_args()

    started = time.time()
    geometry = build_geometry(args.voxel_um)
    data_voxel = mesher.run(geometry)
    n_voxel = sum(len(v) for v in geometry["data_voxelize"]["domain_index"].values())
    print(f"voxels: {n_voxel} conductor voxels in grid {geometry['data_voxelize']['param']['n']}")

    data_solution = solver.run(data_voxel, problem(1.0 / SIGMA), TOLERANCE)
    z_dc = impedance(data_solution, "sim_dc")
    z_ac = impedance(data_solution, "sim_ac")
    inductance = z_ac.imag / (2.0 * np.pi * FREQ_AC)

    result = {
        "pypeec_version": getattr(pypeec, "__version__", "unknown"),
        "fixture": "2-turn square spiral, 60 um start side, 15 um pitch, 2x2 um copper",
        "voxel_size_um": args.voxel_um,
        "conductor_voxels": n_voxel,
        "freq_ac_hz": FREQ_AC,
        "z_dc_ohm": {"re": z_dc.real, "im": z_dc.imag},
        "z_ac_ohm": {"re": z_ac.real, "im": z_ac.imag},
        "r_ohm": z_dc.real,
        "l_h": inductance,
        "runtime_s": time.time() - started,
    }
    with open(args.out, "w", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2)
    print(f"PyPEEC: R = {z_dc.real:.6e} ohm, L = {inductance:.6e} H ({inductance * 1e9:.4f} nH)")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
