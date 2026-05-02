#!/usr/bin/env bash
set -euo pipefail

repo_url="${AGENTICS_REPO_URL:-https://github.com/rymccue/agentics}"
ref="${AGENTICS_REF:-master}"
bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"

if ! command -v git >/dev/null 2>&1; then
  echo "agentics installer: git is required." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "agentics installer: Rust cargo is required." >&2
  echo "Install Rust from https://rustup.rs, then rerun this installer." >&2
  exit 1
fi

echo "Installing agentics from ${repo_url} (${ref})..."
cargo install --git "${repo_url}" --branch "${ref}" --locked --force agentics

if ! command -v agentics >/dev/null 2>&1; then
  echo "agentics installed to ${bin_dir}, but that directory is not on PATH." >&2
  echo "Add this to your shell profile:" >&2
  echo "  export PATH=\"${bin_dir}:\$PATH\"" >&2
  exit 1
fi

echo "Installed $(agentics --version)"
echo "Run 'agentics docs' for local usage docs."
