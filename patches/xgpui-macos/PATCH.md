# macOS blur argument encoding fix

This is the crates.io source of `xgpui-macos 2.0.1`, kept locally to fix the debug-build crash in `remove_layer_background`.

The only backend behavior change uses the typed `CALayer::setBackgroundColor(None)` setter instead of sending an Objective-C object-typed `nil` to a `CGColorRef` argument. The manifest adds `objc2-quartz-core 0.3.2` for that setter. A regression test exercises the layer and recursive sublayer calls with objc2 encoding checks enabled, without opening a window.

The crate retains its Apache-2.0 license. All remaining app dependencies use published registry versions. Remove this directory and the root `[patch.crates-io]` entry once a published backend includes the fix. No Cargo registry cache files were modified.
