#!/usr/bin/env bash
# Build and test the tools that use the kit against this checkout, before a
# release (PLAN-009 D9-18). Their repositories are private, so their URLs are
# not committed: set SMLLM_REPO and SOKF_REPO to the clone URLs. Checkouts
# land in consumers/ (gitignored) and are tested as they are; BRANCH checks
# out that branch in each first.
#
#   SMLLM_REPO=git@github.com:owner/smllm.git SOKF_REPO=git@github.com:owner/sokf.git \
#     scripts/validate-consumers.sh [name...]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/consumers"
mkdir -p "$dir"

# The kit for every consumer: this checkout, without editing their manifests.
patch="patch.crates-io.agent-harness-kit.path=\"$root\""

validate() {
  local name="$1" url="$2"
  if [ -z "$url" ]; then
    echo "error: set the clone URL of $name (${name^^}_REPO)" >&2
    return 2
  fi
  local co="$dir/$name"
  # An existing checkout is the developer's (the port is worked on there):
  # it is tested as it is, never reset.
  [ -d "$co/.git" ] || git clone --quiet "$url" "$co"
  [ -n "${BRANCH:-}" ] && git -C "$co" checkout --quiet "$BRANCH"
  echo "== $name ($(git -C "$co" rev-parse --abbrev-ref HEAD) @ $(git -C "$co" rev-parse --short HEAD))"
  (
    cd "$co"
    cargo --config "$patch" clippy --workspace --all-targets -- -D warnings
    cargo --config "$patch" nextest run --workspace
  )
}

names=("$@")
[ ${#names[@]} -eq 0 ] && names=(smllm sokf)
for name in "${names[@]}"; do
  case "$name" in
    smllm) validate smllm "${SMLLM_REPO:-}" ;;
    sokf) validate sokf "${SOKF_REPO:-}" ;;
    *) echo "error: no consumer named $name" >&2; exit 2 ;;
  esac
done
echo "all consumers pass"
