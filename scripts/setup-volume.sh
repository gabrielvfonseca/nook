#!/usr/bin/env bash
# Create a quota'd APFS volume for Nook. Usage: setup-volume.sh [container-disk] [quota] [name]
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
mkdir -p "/Volumes/$NAME/nook"
touch "/Volumes/$NAME/.metadata_never_index"
echo "Volume ready: /Volumes/$NAME  (export NOOK_ROOT=/Volumes/$NAME/nook)"
