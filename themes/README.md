# Preset themes

Drop any of these snippets into your `$CONFIG_DIR/spotuify/config.yml`. Only the
`theme:` block is present — you can freely add `behavior:` / `keybindings:` in
the same file, or just keep it as-is and use the built-in defaults for the
other sections.

| File | Palette |
|------|---------|
| `spotify-green.yml` | Built-in default. Spotify accent on terminal defaults. |
| `gruvbox-dark.yml` | Warm retro — yellow active, red error, green progress. |
| `solarized-dark.yml` | Ethan Schoonover's Solarized Dark. |
| `nord.yml` | Cool arctic blue palette. |
| `monokai.yml` | The Sublime Text classic — green active, pink error. |

## Install

```bash
cp themes/nord.yml ~/.config/spotuify/config.yml   # Linux / macOS
# or, on macOS specifically, the resolved path is:
cp themes/nord.yml "~/Library/Application Support/io.spotuify/config.yml"
```

If you already have a `config.yml` with `behavior:` or `keybindings:` blocks,
just copy the `theme:` section across by hand. Spotuify merges missing fields
with the built-in defaults, so you only need to override the fields you care
about.
