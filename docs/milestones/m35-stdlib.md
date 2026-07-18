# M35: Stdlib Expansion

**Tagline:** "The missing 30%."

**Status:** In progress.

## Goal

Add regex, datetime, random, and sleep builtins. These are the most
commonly needed functions that currently require `@ rust {}` escape hatches.

## Sub-steps

### Sub-step 1: random_float, random_choice, shuffle

**Change:** Use shared internal pseudo-random state for float/list operations.
Interpreter uses xorshift. Codegen uses std random or the existing PRNG.

**Test:** Each new builtin has a test.

### Sub-step 2: sleep(ms)

**Change:** Blocking sleep. Async-aware: tokio::time::sleep in async,
std::thread::sleep in sync.

**Test:** sleep(0) doesn't crash.

### Sub-step 3: now(), time_diff

**Change:** now() returns Unix timestamp in ms. time_diff(a, b) returns
difference. Auto-dep chrono if needed, or use std::time.

**Test:** now() returns a reasonable value.

### Sub-step 4: regex builtins

**Change:** regex_match, regex_find, regex_find_all, regex_replace.
Auto-dep `regex = "1"`.

**Test:** Each regex builtin works in interpreter and compiled mode.
