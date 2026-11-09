#!/usr/bin/env bash
# Create a quota'd APFS volume for Replica. Usage: setup-volume.sh [container-disk] [quota] [name]
set -euo pipefail
CONTAINER="${1:-}"
QUOTA="${2:-100g}"
NAME="${3:-Work}"
if [[ -z "$CONTAINER" ]]; then
  echo "Find your APFS container with: diskutil apfs list   (look for 'APFS Container Reference', e.g. disk3)" >&2
  echo "Usage: $0 disk3 100g Work" >&2
  exit 1
fi
diskutil apfs addVolume "$CONTAINER" APFS "$NAME" -quota "$QUOTA"
mkdir -p "/Volumes/$NAME/replica"
touch "/Volumes/$NAME/.metadata_never_index"
# Turn off Spotlight indexing on this volume only. Needs sudo.
sudo mdutil -i off "/Volumes/$NAME"
echo "Spotlight indexing is off for /Volumes/$NAME (this volume only)."
echo "Volume ready: /Volumes/$NAME  (export REPLICA_ROOT=/Volumes/$NAME/replica)"
