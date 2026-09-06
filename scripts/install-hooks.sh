#!/bin/sh
# Install the local pre-commit AI guard hook (mirror of the CI ai-guard job).
# Run once after cloning:  sh scripts/install-hooks.sh
set -e

ROOT=$(git rev-parse --show-toplevel)
HOOKS="$ROOT/.git/hooks"
mkdir -p "$HOOKS"

cat > "$HOOKS/pre-commit" <<'HOOK'
#!/bin/sh
# AI guard: block AI/agent files from entering git — even via `git add -f`.
# Mirror of the CI ai-guard job (PLAN.md § 14).
PATTERNS='(^|/)\.ai/|^AGENTS\.md$|^CLAUDE\.md$|^GEMINI\.md$|(^|/)\.aider|(^|/)\.cursor/|^\.cursorrules$|^context/'

STAGED=$(git diff --cached --name-only --diff-filter=ACR)
BLOCKED=$(printf '%s\n' "$STAGED" | grep -E "$PATTERNS" || true)

if [ -n "$BLOCKED" ]; then
    echo "ERROR: AI/agent files must never be committed (PLAN.md § 13.12 / § 14):" >&2
    printf '%s\n' "$BLOCKED" | sed 's/^/  /' >&2
    echo "Unstage them with: git restore --staged <file>" >&2
    exit 1
fi

# Warn (do not fail) about untracked-but-present AI files on disk.
if [ -d "$PWD/.ai" ]; then
    TRACKED_AI=$(git ls-files | grep -E "$PATTERNS" || true)
    if [ -n "$TRACKED_AI" ]; then
        echo "WARNING: tracked AI-related files detected:" >&2
        printf '%s\n' "$TRACKED_AI" | sed 's/^/  /' >&2
    fi
fi

exit 0
HOOK

chmod +x "$HOOKS/pre-commit"
echo "Installed pre-commit AI guard hook at $HOOKS/pre-commit"
