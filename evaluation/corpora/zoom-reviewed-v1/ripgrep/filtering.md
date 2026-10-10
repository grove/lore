* Like other tools specialized to code search, ripgrep defaults to
  [recursive search](GUIDE.md#recursive-search) and does [automatic
  filtering](GUIDE.md#automatic-filtering). Namely, ripgrep won't search files
  ignored by your `.gitignore`/`.ignore`/`.rgignore` files, it won't search
  hidden files and it won't search binary files. Automatic filtering can be
  disabled with `rg -uuu`.
