# External conformance cases

An external conformance case asks whether independently observed output agrees
with a supported, valid rule-corpus calculation. It is evidence about the whole
calculation boundary, not another way to define the calculation.

## Three different checks

- A scenario states an invariant over selected source facts and expected rule
  behavior. It may be synthetic and need not have an external observation.
- An audit searches or summarizes a rule space. It can discover configurations
  but does not establish agreement with an external system.
- An external conformance case records an independent source reference,
  declared assumptions, facts that remain unknown, observed outputs, and the
  outputs calculated from domain source facts. Comparison is allowed only when
  the implementation reports that the path is supported and its source input is
  valid.

External conformance cases remain `.scenario.runa` files. The distinction is in
the typed data and evaluation rules in `examples/conformance.runa`, not in a new
execution mode. The model also fits the non-legal chemistry example, where a
public balanced equation is compared with atom counts calculated by the corpus.
That second use does not reveal a need for dedicated syntax or a
`.conformance.runa` suffix.

## Required separation

1. Build the domain input exclusively from source facts and explicitly declared
   assumptions.
2. Run the ordinary corpus calculation.
3. Build `ConformanceCalculatedOutput` values only from that result.
4. Record external answers separately as `ConformanceObservedOutput` values.
5. Call `conformance_evaluate` after both sides exist.

The observed values must never occur in the source-fact binding or in a helper
that creates it. Matching values do not count as conformance when the
calculation is unsupported, source input is invalid, an unknown fact blocks the
comparison, an output is missing, units differ, or identifiers are duplicated.

An unknown fact stays a `ConformanceUnknownFact`. It may block comparison, be
outside the compared outputs, or be linked to a named declared assumption. The
last option makes assumption-bound evidence visible instead of silently turning
an inferred value into a documented fact.

## Boundary evidence

The anonymized 2025 Personskat case is the first production case:

- `personskat-2025-aarsopgoerelse.scenario.runa` checks the interpreter-facing
  invariant and keeps the private PDF outside the repository.
- `runa check --backend` compiles the same typed case through generated Rust.
- `tests/calculate_cli.rs` obtains canonical source facts, invokes Personskat
  from JSON, hydrates the generated XLSX workbook with the same facts, invokes
  it again, and requires identical JSON and XLSX results before checking the
  externally observed amounts.

The raw private document is not a fixture. Its anonymized source facts,
assumptions, unknowns, and observed outputs are reviewable code; personal
identifiers and the source PDF remain excluded.

## When syntax would be justified

A dedicated file suffix or language declaration should be reconsidered only if
conformance cases need runner-level discovery, standardized machine reports, or
cross-backend orchestration that cannot be expressed by typed rules plus the
existing scenario and calculation commands. Tax and chemistry currently share
the same model without those additions.
