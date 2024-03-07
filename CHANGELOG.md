The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
### Breaking Changes
### Features
### Changed
### Fixed


## 0.4.0 - 2024-03-07
### Breaking Changes
  - Empty matrices or matrices with invalid content are now considered as a valid matrix instead of plain text, so the matrix editor can be used on them
  - Syntax of Line References has been changed to `{3}` instead of {`&[3]`} (the previous syntax caused ambiguity during matrix parsing)
    This change should not affect users, since they use ALT+up/down to select and insert Line References
  - Matrix resizing now requires ``alt+ctrl`` keys instead of just `alt`
  - Autocompletion (ctrl space instead of tabs).
  - Regions (paragraphs under different #Headers) are now have the same alignments across all the 
    document
  - `sum` variable is not set to zero anymore at headers. You can do that explicitly by writing `sum = 0`
  - `$` is now a prefix operator, so the expression `12$` is now invalid and should be written as `$12`
### Features
  - #Headers appears in the Result area as well
  - Name of the navigation tabs are now can be set: It gets the value of the first line if it is a `#Header`
  - Word selection by double click
  - When it comes to binary operations, single row matrices now work as arrays, meaning that the operation is applied
    to all of their elements.
    ```
    [2  3  4] + 1  ---> [3  4  5]
    ```
    #### Usecase
    Let's say you want to compare job offers, so you make an array of offers (`daily rate`).  
    You calculate what those offers mean as a monthly income, including taxes etc.  
    In the `Comparison` part you can see the summary of your calculations, what
    offer would end up in what monthly salary, and how they compare to your current salary.  
    Now if you got a new offer, it is enough just to add that offer to the `daily rate` array,
    and all the information (monthly profit after taxes and comparison to your current salary) will
    appear to this new offer.
    ```
    daily rate = [$90  $100  $120]                    █ [    90 $     100 $     120 $]
    annual gross = (daily rate * 20) * 12             █ [21 600 $  24 000 $  28 800 $]
    tax free = $10k                                   █ 10 000 $
    tax base = annual gross - tax free                █ [11 600 $  14 000 $  18 800 $]
    tax = tax base * 40%                              █ [ 4 640 $   5 600 $   7 520 $]
    net = annual gross - tax                          █ [16 960 $  18 400 $  21 280 $]
    monthly net = net / 12                            █ [ 1 413 $   1 533 $   1 773 $]
    current monthly salary = $1000                    █  1 000 $
                                                      █
    # Comparison                                      █
    daily rate                                        █ [   90 $    100 $    120 $]
    monthly net                                       █ [1 413 $  1 533 $  1 773 $]
    monthly net is what % on current monthly salary   █ [   41 %     53 %     77 %]
  
    ```
    All operators now work on Matrices when it makes sense.
  - Percentage operators work on quantities now: ``5m is 25% of what ==> 20 m``
  - Pasting back Exported text to the editor chops down the Result part
  - User defined functions
    - Pressing `ctrl-b` on a funtion invocation jumps to its definition (same as for Line References and variables)
  - Indent and Unindent lines (tab/shift+tab)
      - Better editor handling, which paves the road for multiple cursors
  - Better handling of period of times in calculations  
      So far every quantity was stored in the base unit (e.g. 10 years was stored as 315 576 000 seconds).  
      It caused precision problems in expressions like `(10k/year cost + 3k/month tax) * 10 years`,  
      resulting in a wrong output (`459 999.9` instead of `460 000`)
  - `&`, `|` and `~` can be used as bitwise operators
  - Smart output units e.g.: By default the result was in seconds for `500$ / 20$/hour`, now it is hours.
  - Only the changed digits are pulsed when a result had changed (it helps to identify changes in a number when the user 
    explores different scenarios, or helps to compare binary results )
  - Performance impr: Result calculation occurs only when the user stops typing for 150 ms
  - If there is not enough space for the editor content + its results, the user is allowed to
    move the right gutter wherever she wants (if there is enough space, NoteCalc will automatically align
    it to show both the editor and result contents)
  - Typing closing parenthesis works similar to IntelliJ, if a closing Parentheses is already there,
    it will be overwritten.
  - Settings modal window
  - Smart indentation when typing Enter (it preserves the actual line's indentation)
  - It is now possible to insert Line References inside matrices, however its rendering is still not supported (so e.g. `{23}` will appear instead of the value of LineRef 23).
  - Function parameter hints
  - If fractional decimals are missing due to low precision set by the user, trailing zeroes are not trimmed, which hopefully rings some bells to the user
  - Added remainder operator (both `rem` and `mod` work)
  - Line References can be copied and pasted
  - Bool type (true, false) 
  - Conditional Expressions (``if <expr> then <expr> else <expr>``, e.g. ``(if true then 2 else 3) + (if false then 4 else 5)`` == 7)
  - Comparison operators (`<`, `>`, `<=`, `>=`, `==`, `!=`)
  - If multiple Assignments are selected, then pressing ctrl-space (autocompletion) brings up an action "Align", which aligns
  the lines on the '=' char
  - Specifiers: 50km as number = 50, 0.2 as percent = 20%
  - Headers `/sum`, show result, bold, sum etc
      Sum Headers create a variable implicitly
  - "Copy as plain text" action in autocompletion
  - If a line result is copied (by pressing ctrl-c without having a selection), the copied result is flashing to highligh what was copied
### Changed
  - Beside `//`, the ` character behaves as a comment as well (explain that ' is used for ignoring single numbers, e.g. 12+3 '4, here the 4 is ignored)
  - Clicking on a result on the Result Panel does not insert a reference to the result any more (it was easy to misclick and as not that useful feature)
  - `ctrl-b` (jumping to the definition line of the variable the cursor is standing on) now scrolls a little bit above the target line if it is out of the visible area
### Fixed
  - Thousand grouping was affected by the minus sign which resulted 
    in wrong grouping when having multiple results, e.g.:
    ```
      fuel     -40k   █   -40 000
      vacation -100k  █ - 100 000 // there is an extra space after '-'
      clothing -10k   █   -10 000
    ```  
  - Fix selecting complex expressions when stepped into matrix  
    When the user stepped into a matrix, the selected cell's content was determined by a primitive algorithm which did not take into account
    the AST, but naively parsed the string and looked for commas or semi colons for cell separators. Obviously this solution did not work
    when the cell content was a complex expressions with function calls etc inside.
    The new solution uses the AST to determine what is inside a matrix cell.
  - Clicking into the result area did nothing so far, now it put the cursos into the clicked line
  - Bin or Hex representation of a float is now an Error
  - Referenced matrices were not visible due to the reference-highlighting covered them
  - Referenced matrices were rendered outside the editor
  - Empty matrices (`[]`) inside parenthesis were not evaluated correctly
  - Added new rendering layers so rendered rectangles can be positioned better (e.g. where to put matrix editor background to not be hidden by cursor highlighter nor function bg color etc)
  - Builtin variable `sum` was not parsed correctly if it was followed by some special chars
  - Saving the content into the #hash of the URL does not create browser history entries anymore (does not work under chrome)
  - Units can be applied to function results (e.g. `sin(pi() rad)` now works)
  - Operator precedence were calculated wrongly when a quantity was involved in the expression (e.g. `2*3 + (4/g)*5g*6` was calculated as `((2*3) + (4/g)*5g)*6`)
  - Line numbers were rendered even below the last physical line
  - Variables did not require full name match (e.g. if variable `a` existed, the string `a_b` would have matched it)
  - If value of a LineRef was 'Err' (aka the result of a referenced line was 'Err'), it was rendered even when was covered by the result panel
  - Changing the vlaue of a LineRef caused pulsing even when the LineRef was not visible due to the result panel
  - Line Reference pulsing was not moved when scrolling
  - Line reference pulsing command was duplicated in the RenderBucket when cursor movement occurred due to mouse clicking
  - If Line Reference Selector (alt + up/down) went out of screen, it did not move the scrollbar
  - Modifying a LineReference source result by pasting (ctrl-v), it did not refresh the line reference pulsings' attributes
  - Already rendered scrollbar were not cleared when a a text was inserted which had enough vertical space
  - LineReferences were rendered without vertical alignment
  - Fix an error when unit conversion happens between the same units but in different representation (ml*m ==> m^4)
  - Fraction numbers were not pulsed when changed
  - Fix parsing expressions like '12 km is x% of...'


## [0.3.0] - 2020-12-21
### Breaking Changes
- Spaces are not allowed in hex numbers anymore but undorscores are.
  It caused problems since 0xAA B could not be unambiguously parsed (e.g. it can be 0xAAB or 0xAA B, where 'B' is a unit for 'bytes', so make it explicit by writing 0xAA_B).
### Features
- Dark Theme ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/1e4fbe68e3e3e0f2b369b29d853881df/D10nAPmBHz.gif)
- Render command optimizations, reducing render command count by around 40%
  (gutters, line number- and result backround 
  are now rendered only once as a big rectangle and not line by line).
- Active references (variables and line refs in the cursor's row) are now just underlined
  and the referenced lines are not highlighted fully only their left and right gutter.
  (Look at the image below)
- Results where a denominator is a unit (e.g. 5 / year) are now rendered in a more user-friendly way (e.g. instead of `5 year^-1` it is `5 / year`)
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/b22cf68c1bb85d890bf1363e55e94435/0.0.3_unit_denom.png)
- Automatic closing parenthesis/braces insertion when the opening one is typed
- Automatic closing parenthesis/braces deletion when the opening one is deleted
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/a2b3699e56eb91dd74909091aa903098/0.0.3_parens.gif)
- Automatic parenthesis/braces wrapping around selected text if opening one is typed while
a text is selected
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/a4f947f869a4d4537043cc962551da8f/0.0.3_wrap.gif)
- Matching parenthesis are highlighted if the cursor is inside them
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/9efb7959ddc5fde61f395f79f2027d9c/0.0.3_paren_hilight.gif)
- The note can be saved with ``ctrl-s`` (Though it is still saved automatically when there is no user interaction)
- It is possible to apply units directly on Line References or Variables, e.g.
  ```
  var = 12
  var km
  ```
  The result of the second line will be ``12 km``
- Parsing improvement: Now the parser are smarter in deciding what is an operator (e.g. 'in' in `12 m in cm`),
  a unit (`12 in` which is 12 inch), or a simple string (`I downloaded 12 GB in 3 seconds`).
  Now it is even possible to evaluate this string `12 in in in`, which is equivalent of `12 inch in inch`
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/1f851d6b80e265dec9b6d055e2ae90b0/0.0.3_smar_parsing.png)
- Percentage calculations
  ![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/7941d87348496a4c9d3841a18a03431f/0.0.3_percentages.png)
- Max row count was increased from 128 to 256
- e (Euler's Number) was added, currently as a function (`e()`)
- Invalid argument types for functions are highlighted as errors (See image below)
- new functions:  
![img](https://trello-attachments.s3.amazonaws.com/558a94779b3b3c5d89efeaa6/5fe0ae9f0b9791732fdf6901/e36e81db4efafb92695d8eb7f833a526/0.0.3_functions.png)
  - abs(num)
  - ln(num)
  - lg(num)
  - log(num, num)
  - sin(angle) -> num
  - cos(angle) -> num
  - tan(angle) -> num
  - asin(num) -> angle
  - acos(num) -> angle
  - atan(num) -> angle

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
- Parenthesis have different color than operators.
- Remove `Vec` and dynamic allocation from unit parsing and operations.
- Replaced all unit `RefCells` to `Rc`, it was a mistake to use `RefCells` in the first place.
- Matrices cannot be deleted anymore by `DEL` or `BACKSPACE`, ctrl is needed

### Fixed
- Dead characters (e.g. '^' on a hungarian keyboard) were not possible to type
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
- u64 values can be parsed correctly (e.g. 0xFFFFFFFFFFFFFFFF)
- Bitwise operations now work on u64 values
- Negative numbers can be presented in binary and hex form
- Line reference pulsing was called each time a cursor was pressed on the line ref's line, causing
flickering
- 'Line normalization' (the process where the referenced lines' id is normalized to the referenced line actual line number)
  could be undo with ctrl-z
- Bitwise shifting operators (`<<` and `>>`) used wrapping, resulted in unwanted situations. Now it is an error
to use larger shift operands than the underlying integer representation (u64 currently)

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
