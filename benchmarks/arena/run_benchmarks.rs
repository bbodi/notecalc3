use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let prev_version_exe_path = args.get(1).map(|it| it.as_str()).unwrap();
    let current_version_exe_path = args.get(2).map(|it| it.as_str()).unwrap();
    let benchmarks_and_iterations = [
        ("bench_copypaste_math_expression", 10),
        ("bench_copypaste_tutorial", 100),
        ("bench_cursor_navigation", 1000),
        ("bench_each_line_references_prev_line", 10),
        ("bench_each_line_references_first_line", 10),
        (
            "bench_each_line_references_first_line_then_modify_first_line",
            10,
        ),
        ("bench_insert_matrix", 50),
        ("bench_typing_the_tutorial", 1),
        ("bench_line_uses_var_from_prev_line", 20),
        (
            "bench_line_uses_var_from_prev_line_then_modify_first_line",
            50,
        ),
        ("bench_select_all_mathy_text", 20),
        ("bench_select_all_simple_text", 20),
    ];

    for benchmarks_and_iteration in &benchmarks_and_iterations {
        Command::new("hyperfine.exe")
            .arg("--warmup")
            .arg("3")
            .arg(format!(
                "{} {} {}",
                prev_version_exe_path, benchmarks_and_iteration.0, benchmarks_and_iteration.1
            ))
            .arg(format!(
                "{} {} {}",
                current_version_exe_path, benchmarks_and_iteration.0, benchmarks_and_iteration.1
            ))
            .status()
            .expect("failed to execute process");
    }
}
