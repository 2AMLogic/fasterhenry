# docs

| Document | Contents |
|---|---|
| [`validation.md`](validation.md) | Measured agreement with independent references (PyPEEC, Greenhouse closed forms, skin-effect and ground-plane oracles), and how to regenerate each. |
| [`benchmarks.md`](benchmarks.md) | Performance: the committed criterion suite, the dense/iterative scaling measurement, and the head-to-head against FastHenry. |
| [`input-format.md`](input-format.md) | The JSON problem document the CLI reads: every field of `geometry`, `ports`, `discretization`, `coupling` and `frequencies_hz`, with units, defaults and worked examples. |
| [`output-formats.md`](output-formats.md) | The CLI's `--zc-mat` (MAT v4) and `--spice` output formats. |
| [`fasthenry-compat.md`](fasthenry-compat.md) | Field-by-field audit of the deck reader against the public FastHenry `.inp` format: supported, differs, or rejected, with the reason for each. |
| [`ROADMAP.md`](ROADMAP.md) | Where the project stands, what comes next and why, and what is blocked. |
