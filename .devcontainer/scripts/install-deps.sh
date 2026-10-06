#!/bin/sh
set -e

# System packages for the devcontainer.
apt-get update && apt-get install -y --no-install-recommends \
    jq \
    bubblewrap \
    socat \
 && apt-get clean -y \
 && rm -rf /var/lib/apt/lists/*