      * `--exit-status` / `-e`:

        Sets the exit status of jq to 0 if the last output value was
        neither `false` nor `null`, 1 if the last output value was
        either `false` or `null`, or 4 if no valid result was ever
        produced.  Normally jq exits with 2 if there was any usage
        problem or system error, 3 if there was a jq program compile
        error, or 0 if the jq program ran.
