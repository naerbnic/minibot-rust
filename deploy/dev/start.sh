#/bin/sh

SCRIPT_HOME="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
declare -r SCRIPT_HOME

cd "${SCRIPT_HOME}"
exec "${CARGO:-cargo}" run --bin dev-start -- "$@"