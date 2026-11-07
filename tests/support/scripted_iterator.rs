use std::{cell::Cell, collections::VecDeque, rc::Rc};

pub struct Scripted<T> {
    steps: VecDeque<Option<T>>,
    calls: Rc<Cell<usize>>,
    stopped: bool,
}

pub struct Source<T> {
    iter: Scripted<T>,
    starts: Rc<Cell<usize>>,
}

pub fn source<T>(
    steps: impl IntoIterator<Item = Option<T>>,
) -> (Source<T>, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let source = Source {
        iter: Scripted {
            steps: steps.into_iter().collect(),
            calls: calls.clone(),
            stopped: false,
        },
        starts: starts.clone(),
    };
    (source, starts, calls)
}

impl<T> IntoIterator for Source<T> {
    type Item = T;
    type IntoIter = Scripted<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.starts.set(self.starts.get() + 1);
        self.iter
    }
}

impl<T> Iterator for Scripted<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        assert!(!self.stopped, "consumer polled past the first None");
        self.calls.set(self.calls.get() + 1);
        let value = self.steps.pop_front().flatten();
        self.stopped = value.is_none();
        value
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        panic!("consumer unexpectedly queried size_hint")
    }

    fn map<B, F>(self, _convert: F) -> std::iter::Map<Self, F>
    where
        F: FnMut(Self::Item) -> B,
    {
        panic!("consumer unexpectedly invoked an overridden map")
    }

    fn fold<B, F>(self, _initial: B, _combine: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        panic!("consumer unexpectedly invoked an overridden fold")
    }
}
