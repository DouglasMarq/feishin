#!/usr/bin/env bash
# Blocks Write/Edit on protected files (.env files, Cargo.lock, skills-lock.json, build output).
# Requires jq.

jq --version >/dev/null 2>&1 || { echo >&2 "jq is required but not installed. Aborting."; exit 2; }

FILE_PATH=$(jq -r '.tool_input.file_path // empty')

case "$FILE_PATH" in
  *.env.example) ;;
  .env | .env.* | */.env | */.env.* | *Cargo.lock | *skills-lock.json | \
  rust/target/* | */rust/target/* )
    echo "BLOCK: Protected file - .env files, lock files and build output must not be edited by Claude (let cargo update Cargo.lock)" >&2
    exit 2
    ;;
esac
exit 0
