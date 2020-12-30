#!/bin/bash

prev_exe="benchmarks-0.3.0.exe"
current_exe="benchmarks-0.3.0.exe"

benchmarks=(
  "bench_copypaste_math_expression"
  "bench_copypaste_tutorial"
  "bench_cursor_navigation"
  "bench_each_line_references_prev_line"
  "bench_each_line_references_first_line"
  "bench_each_line_references_first_line_then_modify_first_line"
  "bench_insert_matrix"
  "bench_typing_the_tutorial"
  "bench_line_uses_var_from_prev_line"
  "bench_line_uses_var_from_prev_line_then_modify_first_line"
  "bench_select_all_mathy_text"
  "bench_select_all_simple_text"
)
iteration_counts=( 100 100 1000 100 100 100 10 100 100 1000 )
len=${#benchmarks[@]}

for (( i=0; i < ${len}; i++ ));
do
   #params="${benchmarks[$i]} ${iteration_counts[$i]}"
   params="${benchmarks[$i]} 1"
   hyperfine.exe --warmup 3 "${prev_exe} ${params}" "${current_exe} ${params}"
   # --show-output
done