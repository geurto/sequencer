#!/bin/bash
set -euo pipefail

# Nothing to set up currently — the image used to start sshd here, which is no
# longer installed. Kept as the container's entrypoint hook.

exec "$@"
