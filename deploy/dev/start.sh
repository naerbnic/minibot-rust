#/bin/sh

SCRIPT_HOME="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
declare -r SCRIPT_HOME

exec "${CARGO:-cargo}" run \
    --manifest-path "${SCRIPT_HOME}/../../tools/devtool/Cargo.toml" \
    --bin dev-start \
    -- \
    --script-home "${SCRIPT_HOME}" "$@"