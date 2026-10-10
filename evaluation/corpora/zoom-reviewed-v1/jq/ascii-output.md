      * `--ascii-output` / `-a`:

        jq usually outputs non-ASCII Unicode codepoints as UTF-8, even
        if the input specified them as escape sequences (like
        "\u03bc"). Using this option, you can force jq to produce pure
        ASCII output with every non-ASCII character replaced with the
        equivalent escape sequence.
