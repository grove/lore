Shell `alias`es and shell functions can not be used for command execution via `fd -x` or
`fd -X`. In `zsh`, you can make the alias global via `alias -g myalias="…"`. In `bash`,
you can use `export -f my_function` to make available to child processes. You would still
need to call `fd -x bash -c 'my_function "$1"' bash`. For other use cases or shells, use
a (temporary) shell script.
