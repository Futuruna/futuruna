# Storage Canary

This tier is run by `./scripts/storage-canary.sh`, not the generic
`./scripts/canary.sh` loop.

The rollback fixture is intentionally expected to fail at runtime. The script
runs it, then runs a separate checker fixture against the same temporary SQLite
database to prove the transaction guard rolled back the failed write while
keeping the earlier committed row.
