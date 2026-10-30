# Synthetic RGB metadata fixtures

These are generated test profiles from xgpui's isolated ICC validation, copied
byte-for-byte from `extensions/crates/xgpui-color/tests/reference` in the sibling
repository. They are not vendor calibration profiles and do not describe this
monitor. They contain no image pixels.

Both use Adobe RGB primary chromaticities. `parametric-rgb.icc` uses quantized
sRGB-style parametric curves; `sampled-rgb.icc` uses independent 257-entry red,
green and blue tables generated from exponents 2, 2.2 and 1.8. The source generator
is `validation/icc-import-probe/generate.py` (Little CMS 2.19). The library tests
retain independent 120-digit arithmetic on the actual serialized tags.

The lab embeds these fixtures, parses them once before entering the UI loop,
and feeds generated pixels plus resolved metadata to xgpui-media. No image
file decoding, runtime file access, or network access is involved. These fixtures
are separate from the CIE dataset and its license; the repository's application
license applies to these generated assets.


`sRGB2014.icc` is an unmodified ICC-authored profile mirrored by libavif at
commit `9b07a9d7a874b159bb4e14b80a6f08bb469d06f7`. Its original copyright tag
is preserved; it is distributed under the ICC profile terms at
https://www.color.org/profileview/ (not the Compact profiles' CC0 license).
`srgb2014-manifest.json` pins the URL, size and SHA-256. Its sampled curves and
slightly non-neutral relative PCS matrix are preserved exactly before D65
adaptation. This is the same fixture covered by the library's independent
precision corpus, not a claim about every revision named sRGB2014.
