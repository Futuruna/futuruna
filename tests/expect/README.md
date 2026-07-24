# Futuruna Expectation Suite

This directory holds compiletest-style compiler expectations.

Each `.runa` case declares what compiler command should be run and what result
is expected:

```runa
-- expect-command: check
-- expect-status: fail
-- expect-stderr: undefined function `missing`
= value = missing()
```

Supported directives:

- `-- expect-command: check|run|interp|emit-rust|emit-fir|verify`
- `-- expect-status: pass|fail`
- `-- expect-stdout: text that must appear on stdout`
- `-- expect-stderr: text that must appear on stderr`
- `-- expect-stdout-file: path/to/stdout.golden`
- `-- expect-stderr-file: path/to/stderr.golden`
- `-- expect-skip: reason`

Golden file paths are resolved relative to the `.runa` case. Golden files check
the whole output channel exactly, except CRLF and LF newlines are treated the
same. They can be combined with substring assertions when a case needs both a
stable full snapshot and a few high-signal markers.

Use this suite for minimized compiler-facing contracts: diagnostics,
pass-specific output, and run/fail behavior. Use `tests/canary/` for realistic
multi-subsystem workflows and `tests/downstream/` for library-consumer
contracts.
