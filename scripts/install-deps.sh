#!/usr/bin/env bash
set -euo pipefail

if [[ "${OSTYPE}" == "linux"* ]]; then
    sudo apt-get update
    sudo apt-get install -y lldb
fi
