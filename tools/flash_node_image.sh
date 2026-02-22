#!/usr/bin/env bash
#
# flash_node_image.sh - helper for writing a generic node-agent image to SD/USB.
#
# Usage:
#   NODE_NAME="Field Node 01" WIFI_SSID="FarmWiFi" WIFI_PASSWORD="secret" \
#     ./tools/flash_node_image.sh /path/to/base-image.img /dev/diskX
#
# The script:
#   1) Validates the base image and target device.
#   2) Writes the image to the device using `dd` (macOS) or `pv`+`dd` if available.
#   3) Mounts the boot partition and injects a first-boot config file consumed by the node agent.
#      - Sets the node display name.
#      - Seeds Wi-Fi credentials for provisioning (if provided).
#      - Drops an empty `ssh` file to enable SSH on first boot.
#
# Safe defaults: it refuses to run unless the target device path looks like a removable disk.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: NODE_NAME=\"Field Node\" WIFI_SSID=\"ssid\" WIFI_PASSWORD=\"pwd\" $0 <base.img> <device>"
  exit 1
fi

BASE_IMAGE=$1
TARGET_DEV=$2

if [[ ! -f "$BASE_IMAGE" ]]; then
  echo "Base image not found: $BASE_IMAGE"
  exit 1
fi

if [[ ! "$TARGET_DEV" =~ ^/dev/(disk|sd) ]]; then
  echo "Refusing to write to non-removable device: $TARGET_DEV"
  exit 1
fi

echo "About to flash $BASE_IMAGE to $TARGET_DEV"
read -r -p "This will erase $TARGET_DEV. Continue? [y/N] " reply
if [[ ! "$reply" =~ ^[Yy]$ ]]; then
  echo "Aborted."
  exit 1
fi

if command -v pv >/dev/null 2>&1; then
  pv "$BASE_IMAGE" | sudo dd of="$TARGET_DEV" bs=4M conv=fsync status=progress
else
  sudo dd if="$BASE_IMAGE" of="$TARGET_DEV" bs=4M conv=fsync status=progress
fi

echo "Syncing..."
sync

echo "Re-reading partition table..."
if command -v partprobe >/dev/null 2>&1; then
  sudo partprobe "$TARGET_DEV" || true
fi

BOOT_MNT=$(mktemp -d)
BOOT_PART="${TARGET_DEV}1"
echo "Mounting boot partition $BOOT_PART to $BOOT_MNT"
sudo mount "$BOOT_PART" "$BOOT_MNT"

NODE_NAME=${NODE_NAME:-"Field Node"}
WIFI_SSID=${WIFI_SSID:-""}
WIFI_PASSWORD=${WIFI_PASSWORD:-""}

if [[ -f "$BOOT_MNT/config.txt" ]]; then
  if sudo grep -Eq '^\\s*dtparam=spi=on\\s*$' "$BOOT_MNT/config.txt"; then
    echo "SPI already enabled in config.txt"
  else
    echo "Enabling SPI (dtparam=spi=on) in config.txt"
    echo "" | sudo tee -a "$BOOT_MNT/config.txt" >/dev/null
    echo "# FarmDashboard: enable SPI for ADS1263" | sudo tee -a "$BOOT_MNT/config.txt" >/dev/null
    echo "dtparam=spi=on" | sudo tee -a "$BOOT_MNT/config.txt" >/dev/null
  fi
else
  echo "WARNING: boot config.txt not found; cannot auto-enable SPI"
fi

cat <<EOF | sudo tee "$BOOT_MNT/node-agent-firstboot.json" >/dev/null
{
  "node": {
    "node_name": "${NODE_NAME}"
  },
  "wifi": {
    "ssid": "${WIFI_SSID}",
    "password": "${WIFI_PASSWORD}"
  }
}
EOF

sudo touch "$BOOT_MNT/ssh"
sync
sudo umount "$BOOT_MNT"
rmdir "$BOOT_MNT"

echo "Image flashed. Insert the SD/USB into the node; on first boot the agent will pick up node-agent-firstboot.json and bring up Wi-Fi if provided."
