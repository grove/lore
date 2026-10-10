Enabling the PCRE2 feature works with a stable Rust compiler and will
attempt to automatically find and link with your system's PCRE2 library via
`pkg-config`. If one doesn't exist, then ripgrep will build PCRE2 from source
using your system's C compiler and then statically link it into the final
executable. Static linking can be forced even when there is an available PCRE2
system library by either building ripgrep with the MUSL target or by setting
`PCRE2_SYS_STATIC=1`.
