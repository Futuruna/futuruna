This directory holds authored Futuruna fixtures that model the language as a
library-consumer surface rather than only as standalone programs.

The dedicated blocking runner is:

```bash
./scripts/downstream-canary.sh
```

These fixtures should:

- import local Futuruna libraries through flat and qualified imports
- rely on exported types, values, and functions across file boundaries
- include pure, stateful, and effect-heavy consumer families
- stress nested import flattening and imported free-function usage
- remain owned by this repository rather than pulling in external codebases

Library helper files should stay side-effect free except for deliberate
import-safe declarations and pure derived values. Mark importable helpers with:

```runa
-- library-hygiene: importable
```

The downstream runner enforces that marked files do not execute script-like
top-level flows such as `@ print`, loops, sends, stream subscriptions, or other
obvious import-time side effects.

It also runs:

```bash
runa lint-library --imports tests/downstream
```

That import-graph check requires every plain or qualified imported helper to be
marked importable and rejects imported script/demo bodies before codegen.
