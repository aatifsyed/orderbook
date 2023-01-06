use std::collections::{BTreeMap, VecDeque};
use tap::Tap as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmpty<T>(T);

impl<T> NonEmpty<T> {
    pub fn vec(value: T) -> NonEmpty<Vec<T>> {
        NonEmpty::<Vec<T>>::new(value)
    }
    pub fn vecdeque(value: T) -> NonEmpty<VecDeque<T>> {
        NonEmpty::<VecDeque<T>>::new(value)
    }
}

impl<T> NonEmpty<Vec<T>> {
    pub fn new(value: T) -> Self {
        Self(vec![value])
    }
    pub fn push(&mut self, value: T) {
        self.0.push(value)
    }
    pub fn pop(mut self) -> (Option<Self>, T) {
        let value = self.0.pop().expect("inner vec is never empty");
        match self.0.len() {
            0 => (None, value),
            _ => (Some(self), value),
        }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> std::slice::Iter<T> {
        self.0.iter()
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
    pub fn pop_back(mut self) -> (Option<Self>, T) {
        let value = self.0.pop_back().expect("inner vecdeque is never empty");
        match self.0.len() {
            0 => (None, value),
            _ => (Some(self), value),
        }
    }
    pub fn pop_front(mut self) -> (Option<Self>, T) {
        let value = self.0.pop_front().expect("inner vecdeque is never empty");
        match self.0.len() {
            0 => (None, value),
            _ => (Some(self), value),
        }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn front(&self) -> &T {
        self.0.front().expect("inner vecdeque is never empty")
    }
    pub fn back(&self) -> &T {
        self.0.back().expect("inner vecdeque is never empty")
    }
    pub fn iter(&self) -> std::collections::vec_deque::Iter<T> {
        self.0.iter()
    }
}

pub trait BTreeMapExt<KeyT, ValueT> {
    fn min(&self) -> Option<&KeyT>;
    fn max(&self) -> Option<&KeyT>;
    fn min_key_value_mut(&mut self) -> Option<(&KeyT, &mut ValueT)>;
    fn max_key_value_mut(&mut self) -> Option<(&KeyT, &mut ValueT)>;
    fn insert_uncontended(&mut self, key: KeyT, value: ValueT)
    where
        KeyT: Ord;
}

impl<KeyT, ValueT> BTreeMapExt<KeyT, ValueT> for BTreeMap<KeyT, ValueT> {
    fn min(&self) -> Option<&KeyT> {
        self.keys().next()
    }
    fn max(&self) -> Option<&KeyT> {
        self.keys().next_back()
    }
    fn min_key_value_mut(&mut self) -> Option<(&KeyT, &mut ValueT)> {
        self.iter_mut().next()
    }
    fn max_key_value_mut(&mut self) -> Option<(&KeyT, &mut ValueT)> {
        self.iter_mut().next_back()
    }
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

#[cfg(test)]
mod tests {
    use std::iter::FromIterator;

    use super::*;
    #[test]
    fn test() {
        let mut map = BTreeMap::from_iter([(3, ()), (4, ()), (1, ()), (2, ())]);
        assert!(matches!(map.min_key_value_mut(), Some((1, _))));
        assert!(matches!(map.max_key_value_mut(), Some((4, _))));
        assert!(matches!(
            BTreeMap::<(), ()>::new().max_key_value_mut(),
            None
        ));
    }
}
