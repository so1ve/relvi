# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/so1ve/relvi/compare/relvi-v0.1.1...relvi-v0.1.2) - 2026-09-30

### Added

- *(clipboard, emoji)* split `Enter` and `Ctrl + C` behavior
- use zwp virtual keyboard protocol instead of wtype and remove
- insert emoji into the previous application

### Fixed

- reset state when reopening launcher
- keep pickers open after copying
- do not trigger `value-changed` to avoid focus change

## [0.1.1](https://github.com/so1ve/relvi/compare/relvi-v0.1.0...relvi-v0.1.1) - 2026-09-30

### Added

- emoji picker
- refine clipboard history ui
- *(clipboard)* support previewing images
- refine clipboard view ui
- clipboard history

### Fixed

- keep the selected row fully visible while scrolling
- correctly calculate selection when scrolled out of bounds

### Other

- update description
- introduce tracing for logging
- use `kbd` for keyboard hints
- extract `keybindings` macro and use `attach` for a better method name
- cleanup reductant build step
- add release-plz
- configure renovate
