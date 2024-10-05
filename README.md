# Exr2Ultra-HDR

A Rust program to convert OpenEXR images to Ultra HDR-compliant JPEG images.

Works great with Blender, just make sure you specify the input color space as Blender does not put the coefficients directly inside the file (maybe feature request ?).

## Features
- Automatically or Manually selecting the input and output color spaces and white points
- Change the exposure
- Output images as regular JPEG or PNG
- Output gain map as PNG or JPEG
- Output Ultra HDR JPEG
- Warnings in case something might go wrong

## Todo List
- While down-converting color spaces, is clipping the xy values a preferable solution ?
- Tone mapping for regular outputs ?
- Proper MPF encoding. Can't be bothered to do that as most Google tools like the Android built-in viewer and Chrome does not seem to care at all. (currently using a pre-made MPF block with a bunch of zeroes instead of correct values)
- Chromaticities input from CLI
