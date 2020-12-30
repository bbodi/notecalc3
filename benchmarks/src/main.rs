use std::env;
use std::str::FromStr;

use typing::*;

use crate::copy_paste::{bench_copypaste_math_expression, bench_copypaste_tutorial};
use crate::cursor_navigation::bench_cursor_navigation;
use crate::linerefs::{
    bench_each_line_references_first_line,
    bench_each_line_references_first_line_then_modify_first_line,
    bench_each_line_references_prev_line,
};
use crate::matrix::bench_insert_matrix;
use crate::select_all::{bench_select_all_mathy_text, bench_select_all_simple_text};
use crate::variables::{
    bench_line_uses_var_from_prev_line, bench_line_uses_var_from_prev_line_then_modify_first_line,
};

mod copy_paste;
mod cursor_navigation;
mod linerefs;
mod matrix;
mod select_all;
mod typing;
mod variables;

fn main() {
    let args: Vec<String> = env::args().collect();

    let iteration_count = args
        .get(2)
        .map(|it| it.as_str())
        .and_then(|it| usize::from_str(it).ok())
        .unwrap_or(2);
    let benchmark_name = args
        .get(1)
        .map(|it| it.as_str())
        .unwrap_or("bench_line_uses_var_from_prev_line");
    match benchmark_name {
        "bench_copypaste_math_expression" => bench_copypaste_math_expression(iteration_count),
        "bench_copypaste_tutorial" => bench_copypaste_tutorial(iteration_count),
        "bench_cursor_navigation" => bench_cursor_navigation(iteration_count),
        "bench_each_line_references_prev_line" => {
            bench_each_line_references_prev_line(iteration_count)
        }
        "bench_each_line_references_first_line" => {
            bench_each_line_references_first_line(iteration_count)
        }
        "bench_each_line_references_first_line_then_modify_first_line" => {
            bench_each_line_references_first_line_then_modify_first_line(iteration_count)
        }
        "bench_insert_matrix" => bench_insert_matrix(iteration_count),
        "bench_typing_the_tutorial" => bench_typing_the_tutorial(iteration_count),
        "bench_select_all_mathy_text" => bench_select_all_mathy_text(iteration_count),
        "bench_select_all_simple_text" => bench_select_all_simple_text(iteration_count),
        "bench_line_uses_var_from_prev_line" => bench_line_uses_var_from_prev_line(iteration_count),
        "bench_line_uses_var_from_prev_line_then_modify_first_line" => {
            bench_line_uses_var_from_prev_line_then_modify_first_line(iteration_count)
        }
        _ => {
            println!("Valid program names are: {:?}", &[""]);
        }
    }
}
