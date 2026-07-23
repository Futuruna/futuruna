# Futuruna Compatibility Guides

This directory records compatibility-impacting Futuruna changes release by
release.

The compatibility policy in [../compatibility-policy.md](../compatibility-policy.md)
defines what counts as a source, behavioral, verification, or artifact-facing
change. This directory is where those changes are actually recorded over time.

## Purpose

Pull requests and `td` tasks are not enough durable release history for stable
users. A compatibility guide is the release-facing ledger for:

- stable surface breaks
- deprecations and migration windows
- bug-fix exceptions that intentionally bypass staged deprecation
- preview or experimental changes that users are likely to notice

## File Layout

Use one file per active release line or release series.

Current convention:

- `0.1.x.md` for the active 0.1.x line

If Futuruna adopts a different release numbering scheme later, prefer stable
version-like filenames over ad hoc prose names.

## When To Update A Guide

Update the current guide when a change touches a stable surface or when a
preview/experimental change is important enough that users should see it in
release-facing notes.

Typical triggers:

- source compatibility changes
- behavioral compatibility changes
- verification compatibility changes
- stable bug-fix exceptions
- deprecations
- preview feature changes with migration or usage impact

Pure refactors, diagnostics wording changes, internal compiler cleanups, and
other explicitly unstable internal changes do not need guide entries unless they
have visible user impact.

## Entry Template

Each guide should contain these sections where relevant:

- `Stable Surface Changes`
- `Deprecations`
- `Bug-Fix Exceptions`
- `Preview And Experimental Notes`

Each entry should say:

- what changed
- which compatibility category it touched
- whether it is a normal staged change or a bug-fix exception
- what users need to change, if anything
- where the durable regression/canary/proof guard lives when that matters

## Review Discipline

When a stable surface changes:

1. update the current compatibility guide
2. mention that guide entry in the PR/review notes
3. keep the entry factual and short

If no guide update is needed, the PR should say why.

## Current Guide

- [0.1.x.md](0.1.x.md)
