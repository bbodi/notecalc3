#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]

use crate::shunting_yard::ShuntingYard;
use crate::token_parser::Token;
use crate::units::consts::{create_prefixes, init_units};
use crate::units::units::Units;
use crate::units::{Unit, UnitPrefixes};
use smallvec::SmallVec;

mod calc;
mod matrix;
mod shunting_yard;
mod token_parser;
mod units;

fn main() {}
//
// #[derive(Debug, Clone)]
// pub enum Panel {
//     Editor,
//     Results,
// }
//
// struct NoteCalcEditor {
//     text_editor: TextEditor,
//     text_editor_buffer: TextBuffer,
// }
//
// struct ResultWindow {
//     text_editor: TextEditor,
//     text_editor_buffer: TextBuffer,
// }
//
// struct App<'a> {
//     desktop_window: DesktopWindow,
//     menu: Menu,
//     button: NormalButton,
//     buttons: ElementsCounted<NormalButton>,
//     pub send_mail_button: NormalButton,
//     pub email_input: TextInput,
//     pub email_signal: Signal,
//     notecalc_editor: NoteCalcEditor,
//     // result_window: ResultWindow,
//     dock: Dock<Panel>,
//     dock_items: DockItem<Panel>,
//     units: Units<'a>,
//     results: Vec<Option<CalcResult<'a>>>,
//     result_lines: Text,
//     pub view: ScrollView,
//     pub view2: ScrollView,
// }
//
// // main_app!(App);
//
//
//
// impl<'a> App<'a> {
//     pub fn new(cx: &mut Cx) -> Self {
//         let scale = 2.0;
//         set_widget_style(
//             cx,
//             &StyleOptions {
//                 scale,
//                 ..StyleOptions::default()
//             },
//         );
//         Self::layout_main().set(
//             cx,
//             Layout {
//                 padding: Padding {
//                     l: 10.,
//                     t: 0.,
//                     r: 10.,
//                     b: 0.,
//                 },
//                 new_line_padding: 4. * scale,
//                 line_wrap: LineWrap::MaxSize(550.),
//                 ..Layout::default()
//             },
//         );
//
//         Self::text_style_body().set(
//             cx,
//             TextStyle {
//                 // font_size: 10.0,
//                 // height_factor: 2.0,
//                 line_spacing: 0.0,
//                 ..TextEditor::text_style_editor_text().get(cx) // ..Theme::text_style_fixed().get(cx)
//             },
//         );
//
//         Self::selected_result_color().set(cx, color("#b"));
//
//         Self {
//             result_lines: Text::new(cx),
//             view: ScrollView::new(cx),
//             view2: ScrollView::new(cx),
//             desktop_window: DesktopWindow::new(cx),
//             button: NormalButton::new(cx),
//             buttons: ElementsCounted::new(NormalButton::new(cx)),
//             menu: Menu::main(vec![Menu::sub(
//                 "Example",
//                 vec![Menu::line(), Menu::item("Quit Example", Cx::command_quit())],
//             )]),
//             send_mail_button: NormalButton::new(cx),
//             email_signal: cx.new_signal(),
//             notecalc_editor: NoteCalcEditor {
//                 text_editor_buffer: TextBuffer::from_utf8("asd+12μm"),
//                 text_editor: TextEditor::new(cx),
//             },
//             // result_window: ResultWindow {
//             //     text_editor_buffer: TextBuffer::from_utf8(""),
//             //     text_editor: TextEditor::new(cx),
//             // },
//             email_input: TextInput::new(
//                 cx,
//                 TextInputOptions {
//                     multiline: false,
//                     read_only: false,
//                     empty_message: "Enter email".to_string(),
//                 },
//             ),
//             dock: Dock::new(cx),
//             dock_items: DockItem::Splitter {
//                 axis: Axis::Vertical,
//                 align: SplitterAlign::First,
//                 pos: 300.0,
//                 first: Box::new(DockItem::TabControl {
//                     current: 0,
//                     previous: 0,
//                     tabs: vec![DockTab {
//                         closeable: false,
//                         title: "Editor".to_string(),
//                         item: Panel::Editor,
//                     }],
//                 }),
//                 last: Box::new(DockItem::TabControl {
//                     current: 0,
//                     previous: 0,
//                     tabs: vec![DockTab {
//                         closeable: false,
//                         title: "Results".to_string(),
//                         item: Panel::Results,
//                     }],
//                 }),
//             },
//             units: Units::new(),
//             results: Vec::with_capacity(256),
//         }
//     }
//
//     fn handle_app(&'a mut self, cx: &mut Cx, event: &mut Event) {
//         let mut shit = vec![];
//         match event {
//             Event::Construct => {
//                 self.units.units = init_units(&self.units.prefixes);
//                 // self.result_window.text_editor.draw_line_numbers = false;
//                 // self.result_window.text_editor.line_number_width = 0.0;
//                 // self.result_window.text_editor.read_only = true;
//                 // draw_cursor_row kikapcsolni?
//                 App::editor_content_changed(
//                     cx,
//                     &mut self.notecalc_editor.text_editor_buffer,
//                     &mut self.results,
//                     &self.units,
//                 );
//             }
//             _ => {}
//         }
//
//         self.desktop_window.handle_desktop_window(cx, event);
//
//         let mut dock_walker = self.dock.walker(&mut self.dock_items);
//         while let Some(item) = dock_walker.walk_handle_dock(cx, event) {
//             match item {
//                 Panel::Editor => {
//                     self.view2.handle_scroll_bars(cx, event);
//                     cx.redraw_child_area(makepad_render::Area::All);
//
//                     match self.notecalc_editor.text_editor.handle_text_editor(
//                         cx,
//                         event,
//                         &mut self.notecalc_editor.text_editor_buffer,
//                     ) {
//                         TextEditorEvent::Change => App::editor_content_changed(
//                             cx,
//                             &mut self.notecalc_editor.text_editor_buffer,
//                             &mut shit,
//                             &self.units,
//                         ),
//                         TextEditorEvent::None => {}
//                         TextEditorEvent::AutoFormat => {}
//                         TextEditorEvent::LagChange => {}
//                         TextEditorEvent::KeyFocus => {}
//                         TextEditorEvent::KeyFocusLost => {}
//                         TextEditorEvent::Escape => {}
//                         TextEditorEvent::Search(_) => {}
//                         TextEditorEvent::Decl(_) => {}
//                     }
//                 }
//                 Panel::Results => {
//                     self.view.handle_scroll_bars(cx, event);
//
//                     // self.notecalc_editor.text_editor_buffer.
//
//                     let cursor_row = self
//                         .notecalc_editor
//                         .text_editor_buffer
//                         .offset_to_text_pos(
//                             self.notecalc_editor
//                                 .text_editor
//                                 .cursors
//                                 .get_last_cursor_head(),
//                         )
//                         .row;
//
//                     if self.view.view.view_id.is_some() {
//                         dbg!(self.view2.get_scroll_pos(cx));
//                         dbg!(self.view2.get_rect(cx));
//                         dbg!(self.view2.get_scroll_pos(cx));
//                         self.view.set_scroll_pos(cx, self.view2.get_scroll_pos(cx));
//                         dbg!(self.view.get_scroll_pos(cx));
//                     }
//                     // self.view.scroll_into_view(
//                     //     cx,
//                     //     Rect {
//                     //         x: 0.0,
//                     //         y: y,
//                     //         w: 1.0,
//                     //         h: 1.0,
//                     //     },
//                     // );
//                     // self.result_window.text_editor.handle_text_editor(
//                     //     cx,
//                     //     event,
//                     //     &mut self.result_window.text_editor_buffer,
//                     // );
//                 }
//             }
//         }
//
//         self.results.extend(shit.into_iter());
//
//         // if let TextEditorEvent::Change = self.email_input.handle_text_input(cx, event) {
//         //     println!("asd1");
//         //     let email = self.email_input.get_value();
//         //
//         //     //self.view.redraw_view_area(cx);
//         // }
//         //
//         // if let ButtonEvent::Clicked = self.button.handle_normal_button(cx, event) {
//         //     println!("CLICKED");
//         // }
//         // for button in self.buttons.iter() {
//         //     button.handle_normal_button(cx, event);
//         // }
//     }
//
//     fn editor_content_changed<'units>(
//         cx: &mut Cx,
//         text_editor_buffer: &mut TextBuffer,
//         // result_window: &mut ResultWindow,
//         line_results: &mut Vec<Option<CalcResult<'units>>>,
//         units: &'units Units,
//     ) {
//         line_results.clear();
//         PlainTokenizer::update_token_chunks(text_editor_buffer, line_results, &units);
//         cx.redraw_child_area(makepad_render::Area::All);
//     }
//
//     pub fn layout_main() -> LayoutId {
//         uid!()
//     }
//
//     pub fn text_style_body() -> TextStyleId {
//         uid!()
//     }
//
//     pub fn selected_result_color() -> ColorId {
//         uid!()
//     }
//
//     fn draw_app(&mut self, cx: &mut Cx) {
//         if self
//             .desktop_window
//             .begin_desktop_window(cx, Some(&self.menu))
//             .is_err()
//         {
//             return;
//         };
//
//         // self.email_input.draw_text_input(cx);
//
//         let mut dock_walker = self.dock.walker(&mut self.dock_items);
//         while let Some(item) =
//             dock_walker.walk_draw_dock(cx, |cx, tab_control, tab, selected| match tab.item {
//                 _ => tab_control.draw_tab(cx, &tab.title, selected, tab.closeable),
//             })
//         {
//             match item {
//                 Panel::Editor => {
//                     if self
//                         .view2
//                         .begin_view(cx, Self::layout_main().get(cx))
//                         .is_err()
//                     {
//                         return;
//                     };
//                     App::draw_text_editor(&mut self.notecalc_editor, cx);
//                     self.view2.end_view(cx);
//                 }
//                 Panel::Results => {
//                     if self
//                         .view
//                         .begin_view(cx, Self::layout_main().get(cx))
//                         .is_err()
//                     {
//                         return;
//                     };
//
//                     cx.move_turtle(0.0, 27.);
//                     for (i, result) in self.results.iter().enumerate() {
//                         let str = result
//                             .as_ref()
//                             .map(|it| it.to_string())
//                             .unwrap_or(" ".to_string());
//
//                         let t = &mut self.result_lines;
//                         t.text_style = TextEditor::text_style_editor_text().get(cx);
//                         // t.text_style = Self::text_style_body().get(cx);
//                         let cursor_row = self
//                             .notecalc_editor
//                             .text_editor_buffer
//                             .offset_to_text_pos(
//                                 self.notecalc_editor
//                                     .text_editor
//                                     .cursors
//                                     .get_last_cursor_head(),
//                             )
//                             .row;
//                         if cursor_row == i {
//                             t.color = Self::selected_result_color().get(cx);
//                         } else {
//                             t.color = Theme::color_text_deselected_defocus().get(cx);
//                         }
//                         t.draw_text(cx, &str);
//
//                         // cx.move_turtle(0.0, 14.);
//                         cx.turtle_new_line();
//                     }
//                     self.view.end_view(cx);
//                     // App::draw_result_window(&mut self.result_window, cx)
//                 }
//             }
//         }
//
//         // self.button.draw_normal_button(cx, "Hello");
//         // for i in 0..1000 {
//         //     self.buttons
//         //         .get_draw(cx)
//         //         .draw_normal_button(cx, &format!("{}", i));
//         // }
//         self.desktop_window.end_desktop_window(cx);
//     }
//
//     fn draw_text_editor(editor: &mut NoteCalcEditor, cx: &mut Cx) {
//         TextEditor::gutter_width().set(cx, 45.0);
//         editor
//             .text_editor
//             .begin_text_editor(cx, &editor.text_editor_buffer);
//         for (index, token_chunk) in editor
//             .text_editor_buffer
//             .token_chunks
//             .iter_mut()
//             .enumerate()
//         {
//             editor.text_editor.draw_chunk(
//                 cx,
//                 index,
//                 &editor.text_editor_buffer.flat_text,
//                 token_chunk,
//                 &editor.text_editor_buffer.markers,
//             );
//         }
//         editor
//             .text_editor
//             .end_text_editor(cx, &editor.text_editor_buffer);
//     }
//
//     fn draw_result_window(widget: &mut ResultWindow, cx: &mut Cx) {
//         TextEditor::gutter_width().set(cx, 0.0);
//         widget
//             .text_editor
//             .begin_text_editor(cx, &widget.text_editor_buffer);
//         for (index, token_chunk) in widget
//             .text_editor_buffer
//             .token_chunks
//             .iter_mut()
//             .enumerate()
//         {
//             widget.text_editor.draw_chunk(
//                 cx,
//                 index,
//                 &widget.text_editor_buffer.flat_text,
//                 token_chunk,
//                 &widget.text_editor_buffer.markers,
//             );
//         }
//         widget
//             .text_editor
//             .end_text_editor(cx, &widget.text_editor_buffer);
//     }
// }
//
// pub struct PlainTokenizer {}
//
// impl PlainTokenizer {
//     pub fn new() -> PlainTokenizer {
//         PlainTokenizer {}
//     }
//
//     pub fn update_token_chunks<'units>(
//         text_buffer: &mut TextBuffer,
//         line_results: &mut Vec<Option<CalcResult<'units>>>,
//         units: &'units Units,
//     ) {
//         if text_buffer.needs_token_chunks() && text_buffer.lines.len() > 0 {
//             // in this case flat_text is empty
//
//             let mut line_offset = 0;
//             let mut pair_stack = Vec::new();
//             for line in &text_buffer.lines {
//                 let mut tokens: Vec<Token> = vec![];
//                 token_parser::TokenParser::parse_line(&line, &[], &[], &mut tokens, units);
//
//                 text_buffer.flat_text.extend_from_slice(line);
//                 text_buffer.flat_text.push('\n');
//                 for token in &tokens {
//                     let (token_ptr, token_type) = match token {
//                         Token::StringLiteral(ptr) => (*ptr, TokenType::String),
//                         // Token::UnitOfMeasure(ptr, unit) => (*ptr, TokenType::BuiltinType),
//                         Token::Variable(_) => panic!(),
//                         Token::NumberLiteral(num) => (num.ptr, TokenType::Number),
//                         Token::Operator(ptr) => (ptr.ptr, TokenType::Operator),
//                     };
//                     let start = unsafe { token_ptr.as_ptr().offset_from(line.as_ptr()) } as usize;
//                     let end = start + token_ptr.len();
//
//                     TokenChunk::push_with_pairing(
//                         &mut text_buffer.token_chunks,
//                         &mut pair_stack,
//                         'a',
//                         line_offset + start,
//                         line_offset + end,
//                         token_type,
//                     );
//                 }
//                 line_offset += line.len();
//                 // text_buffer.flat_text.push('\n');
//                 TokenChunk::push_with_pairing(
//                     &mut text_buffer.token_chunks,
//                     &mut pair_stack,
//                     'a',
//                     line_offset,
//                     line_offset + 1,
//                     TokenType::Newline,
//                 );
//                 line_offset += 1; // newline
//
//                 //
//                 // Shunting yard
//                 //
//
//                 let mut shunting_output = vec![];
//                 ShuntingYard::shunting_yard(tokens, &[], &mut shunting_output);
//                 let mut result_stack = evaluate_tokens(&mut shunting_output, units);
//                 line_results.push(result_stack.pop());
//             }
//             text_buffer.flat_text.push(' '); // for eof
//             TokenChunk::push_with_pairing(
//                 &mut text_buffer.token_chunks,
//                 &mut pair_stack,
//                 'a',
//                 line_offset,
//                 line_offset + 1,
//                 TokenType::Eof,
//             );
//         }
//     }
// }
