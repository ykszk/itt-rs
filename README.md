[image_tagging_tool](https://github.com/ykszk/image_tagging_tool) implemented in rust.
Could be useful when a single binary is preffered over python scripts because of no interpreter requirements.

By specifying [labelme](https://github.com/wkentaro/labelme)'s directory for `tag_dir`, you can edit `flags` elements in labelme files.

Currently, only text and labelme formats are supported for tags.

# Development
First, make sure to clone submodules.

## Run
```console
cargo run -- python/static/demo/settings.toml --open
```