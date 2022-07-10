use std::{fmt, rc::Rc};

pub struct FastEqRc<T>(Rc<T>);

impl<T> FastEqRc<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }
}

impl<T> AsRef<T> for FastEqRc<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Clone for FastEqRc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: fmt::Debug> fmt::Debug for FastEqRc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Default> Default for FastEqRc<T> {
    fn default() -> Self {
        Self(Rc::default())
    }
}

impl<T> std::ops::Deref for FastEqRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> PartialEq for FastEqRc<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> crate::update_from_diff::UpdateFromDiff<Option<T>> for FastEqRc<T> {
    fn update_from_diff(&mut self, diff: Option<T>) {
        if let Some(new_value) = diff {
            *self = FastEqRc::new(new_value)
        }
    }
}
