#[derive(Debug)]
pub struct OnceCellContent<T>(pub T);

unsafe impl<T> Send for OnceCellContent<T> {}
unsafe impl<T> Sync for OnceCellContent<T> {}