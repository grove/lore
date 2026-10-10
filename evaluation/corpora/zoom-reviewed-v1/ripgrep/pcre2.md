* ripgrep has optional support for switching its regex engine to use PCRE2.
  Among other things, this makes it possible to use look-around and
  backreferences in your patterns, which are not supported in ripgrep's default
  regex engine. PCRE2 support can be enabled with `-P/--pcre2` (use PCRE2
  always) or `--auto-hybrid-regex` (use PCRE2 only if needed). An alternative
  syntax is provided via the `--engine (default|pcre2|auto)` option.
