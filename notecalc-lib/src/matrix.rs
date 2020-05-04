use crate::calc::{divide_op, multiply_op, CalcResult};
use crate::units::units::Units;
use bigdecimal::BigDecimal;

#[derive(Debug, Clone)]
pub struct MatrixData<'units> {
    // column major storing
    pub cells: Vec<CalcResult<'units>>,
    pub row_count: usize,
    pub col_count: usize,
}

impl<'units> MatrixData<'units> {
    pub fn new(
        cells: Vec<CalcResult<'units>>,
        row_count: usize,
        col_count: usize,
    ) -> MatrixData<'units> {
        MatrixData {
            cells,
            row_count,
            col_count,
        }
    }

    pub fn cell(&self, row: usize, col: usize) -> &CalcResult<'units> {
        &self.cells[row * self.col_count + col]
    }

    pub fn is_vector(&self) -> bool {
        self.cells.len() == 1
    }

    pub fn neg(&self) -> MatrixData {
        todo!()
    }

    pub fn mult_scalar(&self, scalar: &CalcResult<'units>) -> Option<CalcResult<'units>> {
        let cells: Option<Vec<CalcResult>> = self
            .cells
            .iter()
            .map(|cell| multiply_op(scalar, cell))
            .collect();
        cells.map(|it| CalcResult::Matrix(MatrixData::new(it, self.row_count, self.col_count)))
    }

    pub fn div_scalar(&self, scalar: &CalcResult<'units>) -> Option<CalcResult<'units>> {
        let cells: Option<Vec<CalcResult>> = self
            .cells
            .iter()
            .map(|cell| divide_op(cell, scalar))
            .collect();
        cells.map(|it| CalcResult::Matrix(MatrixData::new(it, self.row_count, self.col_count)))
    }

    // TODO inplace
    pub fn transposed(&self) -> MatrixData<'units> {
        let mut result = MatrixData::new(
            Vec::with_capacity(self.cells.len()),
            self.col_count,
            self.row_count,
        );
        for _ in 0..self.cells.len() {
            result.cells.push(CalcResult::empty());
        }
        for (i, cell) in self.cells.iter().enumerate() {
            let row_i = i % result.row_count;
            let col_i = i / result.row_count;
            result.cells[row_i * result.col_count + col_i] = cell.clone();
        }

        return result;
    }
}
