#!/usr/bin/env bash
# One-time setup: point git at the hooks committed in Backend/.githooks.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"

chmod +x "$repo_root/Backend/.githooks/pre-commit"
git -C "$repo_root" config core.hooksPath Backend/.githooks

echo "Git hooks configured: core.hooksPath -> Backend/.githooks"
