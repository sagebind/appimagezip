# AppImageZip
Pure Rust implementation of the [AppImage specification][AppImageSpec] that uses Zip as the backing image file system.

This is a work in progress, and is an experimental state.

## Overview
Compresses an _AppDir_ into a Zip archive, and prepends the Zip with a bootstrap ELF executable that mounts the AppDir using FUSE and executes `AppRun`. ELF allows arbirary appended data, and Zip allows arbitrary prepended data, so AppImages made this way are automatically valid ELF executables and valid Zip archives.

The Zip format, while not as optimal or full-featured as SquashFS (used in the AppImage reference implementation), has the benefit of being a popular and portable format with tons of tools out there that can read and write them.

## Compiling
Some unusual build steps are needed to compile AppImageZip, so all of the rules are handled using a Makefile:

```
make
```

## Usage
See `appimagezip --help` for more details.


[AppImageSpec]: https://github.com/AppImage/AppImageSpec
