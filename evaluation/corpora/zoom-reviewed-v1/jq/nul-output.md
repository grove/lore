      * `--raw-output0`:

        Like `-r` but jq will print NUL instead of newline after each output.
        This can be useful when the values being output can contain newlines.
        When the output value contains NUL, jq exits with non-zero code.
