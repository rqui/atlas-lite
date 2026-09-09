#!/usr/bin/env bash
set -euo pipefail

# Development-only interactive intake for the Atlas Lite NVS configuration.
# It intentionally does not write a file, echo a secret, or send a secret in a
# command argument. The target-side serial receiver is a later wiring task.

printf '%s\n' 'Atlas Lite setup (development intake)'
printf 'Device ID: '
read -r DEVICE_ID
printf 'Wi-Fi SSID: '
read -r WIFI_SSID
printf 'Wi-Fi password (input hidden): '
read -r -s WIFI_CREDENTIALS
printf '\nAtlas URL: '
read -r ATLAS_URL
printf 'Atlas API token (input hidden): '
read -r -s ATLAS_TOKEN
printf '\n'

# Keep the shell from treating unused input as an error while deliberately
# avoiding diagnostics that could disclose values.
: "${DEVICE_ID:?device ID is required}"
: "${WIFI_SSID:?Wi-Fi SSID is required}"
: "${ATLAS_URL:?Atlas URL is required}"
: "${ATLAS_TOKEN:?Atlas API token is required}"
: "${WIFI_CREDENTIALS:-}"

printf '%s\n' 'physical-write=pending reason=serial-provisioning-receiver-not-wired'
printf '%s\n' 'Clear credentials through the future Settings factory-reset action; do not create removable-media config files.'
exit 2
