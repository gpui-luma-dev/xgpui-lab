# xgpui-macos

macOS windowing and input backend for **xgpui**, using AppKit and the shared
Apple rendering services in `xgpui-apple`.

## When to use this crate

`xgpui-platform` constructs the macOS platform for ordinary applications. Use
this crate directly when integrating platform services or backend-specific APIs.
Native GUI initialization must follow macOS main-thread requirements.

For application setup, use the matching framework packages:

```toml
[dependencies]
gpui = { package = "xgpui", version = "1.0.0" }
gpui_platform = { package = "xgpui-platform", version = "1.0.0" }
```

The registry package is `xgpui-macos`; its Rust library name is
`gpui_macos`. Keep framework packages on a compatible xgpui release.

## Features

| Feature | Purpose |
| --- | --- |
| `font-kit` | Font discovery through the Apple backend. |
| `runtime_shaders` | Runtime Metal shader compilation. |
| `screen-capture` | Optional macOS screen capture integration. |

## Documentation

- [Local development and platform prerequisites](https://github.com/gpui-luma-dev/xgpui/blob/main/docs/LOCAL_DEVELOPMENT.md)
- [xgpui API additions and compatibility](https://github.com/gpui-luma-dev/xgpui/blob/main/docs/extensions/gpui-public-api.md)
- [Release preparation](https://github.com/gpui-luma-dev/xgpui/blob/main/docs/PUBLISHING.md)
- [Repository](https://github.com/gpui-luma-dev/xgpui) · [Website](https://xgpui.dev/)

## Versioning and provenance

The xgpui framework version is `1.0.0`, independent of the upstream release.
This maintained downstream package derives from Zed's `gpui_macos` crate via
[gpui-unofficial](https://github.com/gpui-archipelago/gpui-unofficial).
The accepted capture is `release/1.25.3` at commit
`39d276b1500d503ec6d37efcaaa0dc0c00730576`; its Zed source is `v1.25.3`
(`8dd255c3eafb5231056fe4e6f601cedd011f1c93`).

Licensed under Apache-2.0. xgpui is independent of Zed Industries.
