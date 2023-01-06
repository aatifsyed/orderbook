use std::collections::{BTreeMap, VecDeque};
use tap::Tap as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmpty<T>(T);

impl<T> NonEmpty<T> {
    pub fn vecdeque(value: T) -> NonEmpty<VecDeque<T>> {
        NonEmpty::<VecDeque<T>>::new(value)
    }
}

impl<T> NonEmpty<VecDeque<T>> {
    pub fn new(value: T) -> Self {
        Self(VecDeque::new().tap_mut(|it| it.push_back(value)))
    }
    pub fn push_back(&mut self, value: T) {
        self.0.push_back(value)
    }
    pub fn push_front(&mut self, value: T) {
        self.0.push_front(value)
    }
    pub fn pop_front(mut self) -> (Option<Self>, T) {
        let value = self.0.pop_front().expect("inner vecdeque is never empty");
        match self.0.len() {
            0 => (None, value),
            _ => (Some(self), value),
        }
    }
    pub fn front(&self) -> &T {
        self.0.front().expect("inner vecdeque is never empty")
    }
    pub fn iter(&self) -> std::collections::vec_deque::Iter<T> {
        self.0.iter()
    }
    /// # Panics
    /// - If no items match `condition`
    /// - If multiple items match `condition`
    pub fn pop_once_by(self, condition: impl FnMut(&T) -> bool) -> (Option<Self>, T) {
        let (mut matching, rest) = self.0.into_iter().partition::<VecDeque<_>, _>(condition);
        assert_eq!(1, matching.len(), "unexpected number of matching items");
        let t = matching.remove(0).unwrap();
        match rest.len() {
            0 => (None, t),
            _ => (Some(Self(rest)), t),
        }
    }
}

pub trait BTreeMapExt<KeyT, ValueT> {
    fn insert_uncontended(&mut self, key: KeyT, value: ValueT)
    where
        KeyT: Ord;
}

impl<KeyT, ValueT> BTreeMapExt<KeyT, ValueT> for BTreeMap<KeyT, ValueT> {
    fn insert_uncontended(&mut self, key: KeyT, value: ValueT)
    where
        KeyT: Ord,
    {
        let clobbered = self.insert(key, value);
        if clobbered.is_some() {
            panic!("key was contended")
        }
    }
}
