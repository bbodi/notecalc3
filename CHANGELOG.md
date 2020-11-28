# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
  - Mobile support for index.html and notecalc itself
    - typing using the native virtual keyboard
    - scrolling
  - High DPI rendering
  - Hovering effect for the scrollbar
  - Cursor changes to 'w-resize' when above the right gutter to highlight that it is draggable
### Fix
  - Firefox tracking protection issue for webfont.js
  - variable-reference pulsing pulsed continously when the rught gutter was dragged
  - the result panel is automatically resizing with respect to browser width, required space for results and required space for the editor panel's content
  - The binary and hex representation gives Err for quantities and percentages
  - lines that are too long and are covered by the result panel renders dots on the panel. Some fixes
    - the dots for matrices are now rendered for all the matrix's rows
    - sometimes the dots were not rendering for matrixes, fixed
  - '…' was not rendered correctly for Matrices
  - ctrl-t is not absorbed by the frontend   
### Removed
  - local.html was removed, you can access now the debug functionality and local wasm file 
  via `host/notecalc?debug` 


## [1.0.0] - 2020-11-19