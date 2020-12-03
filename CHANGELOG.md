The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2020-12-03
### Breaking Changes
`--` does not set the `sum` variable to zero, but every header does
### Features
  - Mobile support for index.html and notecalc itself
    - typing using the native virtual keyboard
    - scrolling
  - High DPI rendering
  - Headers (`# Header`) and region-specific alignments (different regions 
    under different headers can have different alignments, making the
    result output more ergonomic).
  - Comments are supported (e.g. `12 + 3 // it should be 15`)
  - Hovering effect for the scrollbar
  - Cursor changes to 'w-resize' when above the right gutter to highlight that it is draggable
  - Line count limitation was increased from 64 to 128
  - Default precision limitation both from the result panel and from the inserted line references was removed. 
    It caused problems since very small numbers could appear as "0" due to roundings.
    It might be inconvenient in some situations but will be fixed when rounding specifiers will be implemented.
    Though, NoteCalc tries its best to render a compact form (e.g. removes repetends etc)
### Changed
  - Tutorial is updated with the new features and now stored in GIT
  - Leangains demo is updated 
### Fixes
  - In case a line reference was rendered outside of the editor area, '...' did not appear at the edge of the editor
  - Firefox tracking protection issue for webfont.js
  - Variable-reference pulsing pulsed continously when the right gutter was dragged
  - The result panel is automatically resizing now with respect to browser width, required space for results and required space for the editor panel's content
  - The binary and hex representation gives Err for quantities and percentages
  - Lines that are too long and are covered by the result panel renders dots ('...') on the panel. 
  - The dots for matrices are now rendered for all the matrix's rows
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
  - Global static RESULT_BUFFER is now part of the lib and not of the frontend (introduces some unsafe code but less trouble with the borrow checker)
  - Fixed error when variable name is empty
### Removed
  - local.html was removed, you can access now the debug functionality and local wasm file 
  via `host/notecalc?debug` 


## [0.1.0] - 2020-11-19