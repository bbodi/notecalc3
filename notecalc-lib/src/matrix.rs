use crate::calc::CalcResult;
use bigdecimal::BigDecimal;

#[derive(Debug, Clone)]
pub struct MatrixData<'units> {
    // column major storing
    pub cols: Vec<Vec<CalcResult<'units>>>,
}

impl<'units> MatrixData<'units> {
    pub fn new(cells: Vec<CalcResult<'units>>) -> MatrixData<'units> {
        MatrixData {
            cols: vec![cells.to_vec()],
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
