use crate::editor::{FirstModifiedRowIndex, Line, MAX_LINE_LEN};
use crate::{OutputMessage, RenderTextMsg};

pub fn render<'editor>(
    editor_content: &'editor [Line],
    row_index: FirstModifiedRowIndex,
    output: &mut Vec<OutputMessage<'editor>>,
) {
    for (row_i, line) in editor_content.iter().enumerate() {
        output.push(OutputMessage::RenderText(RenderTextMsg {
            text: &line.get_chars()[0..line.len()],
            row: row_i,
            column: 0,
        }))
    }
}
