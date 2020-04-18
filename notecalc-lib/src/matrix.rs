use crate::calc::CalcResult;
use bigdecimal::BigDecimal;

#[derive(Debug, Clone)]
pub struct MatrixData<'units> {
    // column major storing
    pub cols: Vec<CalcResult<'units>>,
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
            cols: cells,
            row_count,
            col_count,
        }
    }

    pub fn is_vector(&self) -> bool {
        self.cols.len() == 1
    }

    pub fn neg(&self) -> MatrixData {
        todo!()
    }

    pub fn mult_scalar(&self, n: &BigDecimal) -> MatrixData {
        todo!()
    }
}
