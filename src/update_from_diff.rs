pub trait UpdateFromDiff<Diff> {
    fn update_from_diff(&mut self, diff: Diff);
}

impl<T> UpdateFromDiff<Option<T>> for T {
    fn update_from_diff(&mut self, diff: Option<T>) {
        if let Some(new_value) = diff {
            *self = new_value;
        }
    }
}
