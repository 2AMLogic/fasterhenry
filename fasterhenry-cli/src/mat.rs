//! MATLAB level-4 (MAT v4) file writer for impedance sweeps — the format
//! of FastHenry's `Zc.mat`, written from the published on-disk
//! specification (a file format is not code; nothing here is derived from
//! any MATLAB or FastHenry source).
//!
//! # Layout
//!
//! A MAT v4 file is a sequence of matrices, each:
//!
//! | field | type | value here |
//! |---|---|---|
//! | `mopt` | int32 | `0` — native byte order, IEEE, double precision (the type lives in `mopt`'s tens digit), full numeric matrix |
//! | `m`, `n` | int32 ×2 | the matrix shape |
//! | `imagf` | int32 | `1` for complex matrices (real block, then imaginary block), `0` otherwise |
//! | `namlen` | int32 | the variable-name length in bytes |
//! | `name` | bytes | the variable name (exactly `namlen` bytes) |
//! | `data` | doubles | column-major: all real parts, then all imaginary parts |
//!
//! Every matrix variable is 2-D in MAT v4, so a sweep is written as one
//! `Zc_1 … Zc_K` (n_ports × n_ports, complex) per frequency plus a
//! `freqs` (1 × K, Hz) row vector. Octave (`load -mat4-binary`) and
//! SciPy (`scipy.io.loadmat`) both read the result; the layout round-trip
//! is asserted in CI against `scipy.io.loadmat`.

use std::io::{self, Write};

use fasterhenry::SweepResult;

/// Writes `result` as MAT v4 variables: `Zc_1 … Zc_K` (complex, ohms) and
/// `freqs` (Hz).
pub fn write_zc_mat<W: Write>(writer: &mut W, result: &SweepResult) -> io::Result<()> {
    let n = result.ports.len();
    let frequencies: Vec<f64> = result.frequencies_hz.clone();
    write_matrix(
        writer,
        "freqs",
        1,
        frequencies.len(),
        false,
        &frequencies,
        &[],
    )?;
    for (index, z) in result.impedance_ohm.iter().enumerate() {
        let mut real = Vec::with_capacity(n * n);
        let mut imaginary = Vec::with_capacity(n * n);
        for column in 0..n {
            for row in 0..n {
                let entry = &z[(row, column)];
                real.push(entry.re);
                imaginary.push(entry.im);
            }
        }
        write_matrix(
            writer,
            &format!("Zc_{}", index + 1),
            n,
            n,
            true,
            &real,
            &imaginary,
        )?;
    }
    Ok(())
}

/// One MAT v4 matrix record; complex matrices write the real block then
/// the imaginary block, both column-major.
fn write_matrix<W: Write>(
    writer: &mut W,
    name: &str,
    m: usize,
    n: usize,
    complex: bool,
    real: &[f64],
    imaginary: &[f64],
) -> io::Result<()> {
    let name_bytes = name.as_bytes();
    let mut header = Vec::with_capacity(20 + name_bytes.len());
    header.extend_from_slice(&0i32.to_le_bytes()); // mopt: double, full, native order
    header.extend_from_slice(&(m as i32).to_le_bytes());
    header.extend_from_slice(&(n as i32).to_le_bytes());
    header.extend_from_slice(&(complex as i32).to_le_bytes()); // imagf
    header.extend_from_slice(&(name_bytes.len() as i32).to_le_bytes()); // namlen
    writer.write_all(&header)?;
    writer.write_all(name_bytes)?;
    for value in real {
        writer.write_all(&value.to_le_bytes())?;
    }
    if complex {
        for value in imaginary {
            writer.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}
