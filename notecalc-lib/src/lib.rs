#![feature(ptr_offset_from, const_if_match, const_fn, const_panic, drain_filter)]

mod calc;
mod matrix;
mod shunting_yard;
mod token_parser;
mod units;

pub mod editor;
pub mod renderer;

#[repr(C)]
#[derive(Debug)]
pub enum TextStyle {
    Normal,
    Bold,
    Underline,
    Italy,
}

#[derive(Debug)]
pub struct RenderTextMsg<'a> {
    pub text: &'a [char],
    pub row: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug)]
pub enum OutputMessage<'a> {
    SetStyle(TextStyle),
    SetColor(Color),
    RenderText(RenderTextMsg<'a>),
    //RenderRectangle
}
