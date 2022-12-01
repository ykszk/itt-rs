[image_tagging_tool](https://github.com/ykszk/image_tagging_tool) implemented in rust.
Could be useful when a single binary is preferred to python scripts.

By specifying [labelme](https://github.com/wkentaro/labelme)'s directory for `tag_dir`, you can edit `flags` elements in labelme's json files.

# Usage
```console
image_tagging settings.toml
```

See [ykszk/image_tagging_tooll](https://github.com/ykszk/image_tagging_tool) for [settings](https://github.com/ykszk/image_tagging_tool#settings) and [data formats](https://github.com/ykszk/image_tagging_tool#dataformat).

## Options
- `--open`: open the web browser
- `--ignore_missing`: ignore images with no tag files or database entry.

# Development
Make sure to clone submodules first.

## Run
```console
cargo run -- python/static/demo/settings.toml --open
```