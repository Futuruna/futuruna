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
- `-- expect-skip: reason`

Use this suite for minimized compiler-facing contracts: diagnostics,
pass-specific output, and run/fail behavior. Use `tests/canary/` for realistic
multi-subsystem workflows and `tests/downstream/` for library-consumer
contracts.

