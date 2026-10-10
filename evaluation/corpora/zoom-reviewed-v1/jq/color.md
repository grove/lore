
      * `--color-output` / `-C` and `--monochrome-output` / `-M`:

        By default, jq outputs colored JSON if writing to a
        terminal. You can force it to produce color even if writing to
        a pipe or a file using `-C`, and disable color with `-M`.
        When the `NO_COLOR` environment variable is not empty, jq disables
        colored output by default, but you can enable it by `-C`.
