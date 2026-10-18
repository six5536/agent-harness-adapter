#!/bin/sh
# `mytool hook`, bridged: Claude Code's hook input on stdin, in every agent.
if grep -q 'rm -rf'; then
  echo "mytool: no rm -rf here" >&2
  exit 2
fi
