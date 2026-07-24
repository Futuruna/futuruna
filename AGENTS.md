# Agent Instructions

This project uses **td** for issue tracking. Run `td usage --new-session` at conversation start or after a context reset.

## Quick Reference

```bash
td ready              # Find available work
td show <id>          # View issue details
td start <id>         # Claim/start work
td log "message"      # Record progress
td handoff <id>       # Capture handoff context
td review <id>        # Submit completed work for review
td approve <id>       # Approve reviewed work from another session
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN TD INTEGRATION v:1 profile:minimal -->
## TD Issue Tracker

This project uses **td** for issue tracking. Run `td usage --new-session` at conversation start or after a context reset.

### Quick Reference

```bash
td ready              # Find available work
td show <id>          # View issue details
td start <id>         # Claim/start work
td log "message"      # Record progress
td handoff <id>       # Capture handoff context
td review <id>        # Submit completed work for review
td approve <id>       # Approve reviewed work from another session
td reject <id>        # Return reviewed work to open
```

### Rules

- Use `td` for all task tracking; do not use TodoWrite, TaskCreate, or markdown task lists for project work.
- Run `td usage --new-session` at conversation start or after a context reset.
- Use `td log` and `td handoff` for persistent work context.
- Completed implementation work should go through `td review`; a different session should use `td approve` or `td reject`.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END TD INTEGRATION -->
