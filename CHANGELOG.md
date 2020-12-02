# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Features
  - Mobile support for index.html and notecalc itself
    - typing using the native virtual keyboard
    - scrolling
  - Comments are supported (e.g. `12 + 3 // it should be 15`)
  - High DPI rendering
  - Hovering effect for the scrollbar
  - Cursor changes to 'w-resize' when above the right gutter to highlight that it is draggable
  - Line count limitation was increased from 64 to 128
  - Default precision limitation both from the result panel and from the inserted line references was removed. 
    It caused problems since very small numbers could appear as "0" due to roundings.
    It might be inconvenient in some situations but will be fixed when rounding specifiers will be implemented.
  - In case a line reference was rendered outside of the editor area, '...' did not appear at the edge of the editor
### Fixes
  - Firefox tracking protection issue for webfont.js
  - variable-reference pulsing pulsed continously when the rught gutter was dragged
  - the result panel is automatically resizing with respect to browser width, required space for results and required space for the editor panel's content
  - The binary and hex representation gives Err for quantities and percentages
  - lines that are too long and are covered by the result panel renders dots on the panel. Some fixes
    - the dots for matrices are now rendered for all the matrix's rows
    - sometimes the dots were not rendering for matrixes, fixed
  - '…' was not rendered correctly for Matrices
  - Ctrl-t is not absorbed by the frontend anymore
  - Undoing (Ctrl-z) a selection removal with DEL button was buggy and only the first line of the selection was
    reparsed  
  - Pulses (which highlights the usage places of variable/lineref under the cursor)
    appeared too slowly when navigating with cursor or mouse, making it more difficult
    for the user to notice it immediately.
  - Space is automatically inserted in front of a just-inserted line reference
  - Right gutter alignment was a render pass late to the input which caused it
  - Cursor was not visible at the right edge of the editor
  - global static RESULT_BUFFER is now part of the lib and not of the frontend (introduces some unsafe code but less trouble with the borrow checker)
### Removed
  - local.html was removed, you can access now the debug functionality and local wasm file 
  via `host/notecalc?debug` 


## [1.0.0] - 2020-11-19