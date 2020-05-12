use crate::calc::{divide_op, multiply_op, CalcResult};

#[derive(Debug, Clone)]
pub struct MatrixData {
    // column major storing
    pub cells: Vec<CalcResult>,
    pub row_count: usize,
    pub col_count: usize,
}

impl MatrixData {
    pub fn new(cells: Vec<CalcResult>, row_count: usize, col_count: usize) -> MatrixData {
        MatrixData {
            cells,
            row_count,
            col_count,
        }
    }

    pub fn cell(&self, row: usize, col: usize) -> &CalcResult {
        &self.cells[row * self.col_count + col]
    }

    pub fn is_vector(&self) -> bool {
        self.cells.len() == 1
    }

    pub fn neg(&self) -> MatrixData {
        todo!()
    }

    pub fn mult_scalar(&self, scalar: &CalcResult) -> Option<CalcResult> {
        let cells: Option<Vec<CalcResult>> = self
            .cells
            .iter()
            .map(|cell| multiply_op(scalar, cell))
            .collect();
        cells.map(|it| CalcResult::Matrix(MatrixData::new(it, self.row_count, self.col_count)))
    }

    pub fn div_scalar(&self, scalar: &CalcResult) -> Option<CalcResult> {
        let cells: Option<Vec<CalcResult>> = self
            .cells
            .iter()
            .map(|cell| divide_op(cell, scalar))
            .collect();
        cells.map(|it| CalcResult::Matrix(MatrixData::new(it, self.row_count, self.col_count)))
    }

    // TODO inplace
    pub fn transposed(&self) -> MatrixData {
        let mut result = MatrixData::new(
            Vec::with_capacity(self.cells.len()),
            self.col_count,
            self.row_count,
        );
        for _ in 0..self.cells.len() {
            result.cells.push(CalcResult::hack_empty());
        }
        for (i, cell) in self.cells.iter().enumerate() {
            let row_i = i % result.row_count;
            let col_i = i / result.row_count;
            result.cells[row_i * result.col_count + col_i] = cell.clone();
        }

        return result;
    }
}
