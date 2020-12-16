The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
### Breaking Changes
### Features
### Changed
### Fixed


## [Unreleased]
### Breaking Changes
- Spaces is not allowed in hex numbers anymore but undorscores are.
  It caused problems since 0xAA B could not be unambiguously parsed (e.g. it can be 0xAAB or 0xAA bytes, so make it explicit by writing 0xAA_B).
### Features
- Dark Theme
- Render command optimizations, reducing render command count by around 40%
  (gutters, line number- and result abckround now
  are rendered only once as a big rectangle and not line by line).
- Active references (variables and line refs in the cursor's row) are now just underlined
  and the referenced lines are not highlighted fully only their left and right gutter.
  [gif](https://twitter.com/bodidev/status/1337363000261554182)
- Automatic closing parenthesis/braces insertion when the opening one is typed
- Automatic closing parenthesis/braces deletion when the opening one is deleted
- Automatic parenthesis/braces wrapping around selected text if opening one is typed while
a text is selected
  [gif](https://twitter.com/bodidev/status/1338427762831470592)
- The note can be saved with ``ctrl-s`` (Though it is still saved automatically when there is no user interaction)
- Results where a denominator is a unit (e.g. 5 / year) are now rendered in a moe user-friendly way (e.g. instead of `5 year^-1` it is `5 / year`)
- It is possible to apply units directly on Line References or Variables, e.g.
  ```
  var = 12
  var km
  ```
  The result of the second line will be ``12 km``
- Parsing improvement: Now the parser are smarter in deciding what is an operator (e.g. 'in' in `12 m in cm`),
  a unit (`12 in` which is 12 inch), or a simple string (`12 GB in 3 seconds`).
  Now it is even possible to evaluate this string `12 in in in`, which is equivalent of `12 inch in inch`

### Changed
- When opening a not empty note, the result panel now tries
to be as close to the editor as possible to have a better
  overview about calculations and their results.
- GitHub and website links were added to the NoteCalc page
- Strings at the frontend are now rendered char by char. It is necessary
to be able to place the cursor at the right place, since the text rendering
  does not guarantee that a single char takes exactly 'char-width' pixels.
- Overlay canvas (the canvas above the app canvas) was removed. It was used to draw
  overlay effects (e.g. pulsing), but it was problematic since it needed alpha blending,
  which wasn't always nice and console frontend support is limited.
  Now pulses are rendered above "BelowText" layer and below the "Text" layer.
- underscore is allowed in binary numbers (e.g. ``0b11_00``)

### Fixed
- Longest visible result length was calculated wrongly when there were multiple headers
in a note, which affected the result panel size.
- `sum` variable get emptied at #Headers
- Char width at the fonrend is now integer, ceiling upward. It caused issues
with rendering (widths of recatngles were float as well and did not always fill up
  the required space)
- Underlines and line reference background rectangles were rendered even if they
were outside of the editor area
- `ctrl-x` did not copy the selected text
- Units in the denominator behaved buggy. now expressions like this works well
  ```
  tax A = 50 000/month
  tax B = 50 000/year
  (tax A + tax B) * (1 year)
  ```
- Matrices cannot be deleted anymore by `DEL` or `BACKSPACE`, ctrl is needed
- u64 values can be parsed correctly (e.g. 0xFFFFFFFFFFFFFFFF)
- Bitwise operations now work on u64 values
- Negative numbers can be presented in binary and hex form
- Line reference pulsing was called each time a cursor was pressed on the line ref's line, causing
flickering
- 'Line normalization' (the process where the referenced lines' id is normalized to the referenced line actual line number)
  could be undo with ctrl-z

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