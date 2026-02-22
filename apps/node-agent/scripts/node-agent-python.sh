#!/bin/bash
set -euo pipefail

py_tag="$(/usr/bin/python3 -c 'import sys; print(f"{sys.version_info.major}{sys.version_info.minor}")')"
vendor="/opt/node-agent/vendor/py${py_tag}"

if [ -d "${vendor}" ]; then
  export PYTHONPATH="${vendor}:/opt/node-agent"
else
  export PYTHONPATH="/opt/node-agent/vendor:/opt/node-agent"
fi

export PYTHONUNBUFFERED=1
exec /usr/bin/python3 "$@"

