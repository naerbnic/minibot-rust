use super::{
    accessor_result::AccessorResult,
    error::{Error, Result},
    index_set::IndexUpdater,
};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::sync::RwLock;

fn entry_finder<'a, T, F, K, Q>(
    accessor: F,
    rows: &'a BTreeMap<u64, T>,
    value: &'a Q,
) -> impl Fn(&u64) -> std::cmp::Ordering + 'a
where
    F: for<'b> Fn(&'b T) -> AccessorResult<'b, K> + 'a,
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    move |target_id| {
        let target_cow = accessor(rows.get(target_id).unwrap());
        let target = target_cow.as_ref();
        target.borrow().cmp(value)
    }
}

fn entry_cmp<'a, T, F, K>(
    accessor: F,
    rows: &'a BTreeMap<u64, T>,
) -> impl Fn(&u64, &u64) -> std::cmp::Ordering + 'a
where
    F: for<'b> Fn(&'b T) -> AccessorResult<'b, K> + 'a,
    K: Ord,
{
    move |left_id, right_id| {
        let left_cow = accessor(rows.get(left_id).unwrap());
        let left = left_cow.as_ref();
        let right_cow = accessor(rows.get(right_id).unwrap());
        let right = right_cow.as_ref();
        left.cmp(right)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Uniqueness {
    Unique,
    NotUnique,
}

pub struct IndexStore<T, K> {
    accessor: Box<dyn for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync>,
    entries: Vec<u64>,
    unique: Uniqueness,
}

impl<T, K> IndexStore<T, K>
where
    K: Ord,
{
    pub fn new<F>(rows: &BTreeMap<u64, T>, unique: Uniqueness, accessor: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync + 'static,
    {
        let mut entries = rows.keys().cloned().collect::<Vec<_>>();

        entries.sort_by(entry_cmp(&accessor, &rows));

        IndexStore {
            accessor: Box::new(accessor),
            entries,
            unique,
        }
    }

    fn find_range<Q>(&self, rows: &BTreeMap<u64, T>, key: &Q) -> std::ops::Range<usize>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let finder = entry_finder(&self.accessor, rows, key);

        match self.entries.binary_search_by(&finder) {
            Ok(idx) => {
                let mut start = idx;
                loop {
                    if start == 0 {
                        break;
                    }
                    if let std::cmp::Ordering::Equal = finder(&self.entries[start - 1]) {
                        start -= 1;
                    } else {
                        break;
                    }
                }
                let mut end = idx + 1;
                loop {
                    if end == self.entries.len() {
                        break;
                    }
                    if let std::cmp::Ordering::Equal = finder(&self.entries[end]) {
                        end += 1;
                    } else {
                        break;
                    }
                }

                std::ops::Range { start, end }
            }
            Err(idx) => std::ops::Range {
                start: idx,
                end: idx,
            },
        }
    }

    pub fn get_entries<Q>(&self, rows: &BTreeMap<u64, T>, value: &Q) -> Result<Vec<u64>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let range = self.find_range(rows, value);

        Ok(self.entries[range].iter().copied().collect())
    }

    fn check_add(&self, rows: &BTreeMap<u64, T>, value: &T) -> Result<()> {
        if let Uniqueness::Unique = self.unique {
            let new_entry_key_cow = (self.accessor)(value);
            let new_entry_key = new_entry_key_cow.as_ref();
            let range = self.find_range(rows, new_entry_key);

            if range.len() != 0 {
                return Err(Error::AlreadyExists);
            }
        }

        Ok(())
    }

    fn check_update(&self, rows: &BTreeMap<u64, T>, id: u64, new_value: &T) -> Result<()> {
        if let Uniqueness::Unique = self.unique {
            let new_entry_key_cow = (self.accessor)(new_value);
            let new_entry_key = new_entry_key_cow.as_ref();
            let range = self.find_range(rows, new_entry_key);

            if range.len() != 0 {
                assert_eq!(range.len(), 1);
                // It can only exist if the only equal value is the same id, since removing it
                // will make it empty, and ensure uniqueness again.
                if self.entries[range][0] != id {
                    return Err(Error::AlreadyExists);
                }
            }
        }

        Ok(())
    }

    fn check_remove(&self, _rows: &BTreeMap<u64, T>, _id: u64) -> Result<()> {
        Ok(())
    }

    fn add_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let new_entry_key_cow = (self.accessor)(rows.get(&id).unwrap());
        let new_entry_key = new_entry_key_cow.as_ref();
        let range = self.find_range(rows, new_entry_key);

        self.entries.insert(range.end, id);
        Ok(())
    }

    fn update_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()> {
        let old_entry_key_cow = (self.accessor)(old_entry);
        let old_entry_key = old_entry_key_cow.as_ref();
        let range = self.find_range(rows, old_entry_key);

        let index = self.entries[range.clone()]
            .iter()
            .position(|i| i == &id)
            .unwrap();
        self.entries.remove(range.start + index);
        self.add_entry(rows, id)
    }

    fn remove_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let old_entry_key_cow = (self.accessor)(rows.get(&id).unwrap());
        let old_entry_key = old_entry_key_cow.as_ref();
        let range = self.find_range(rows, old_entry_key);

        let index = self.entries[range.clone()]
            .iter()
            .position(|i| i == &id)
            .unwrap();
        self.entries.remove(range.start + index);
        Ok(())
    }
}

impl<T, K> IndexUpdater<T> for RwLock<IndexStore<T, K>>
where
    T: Send + Sync,
    K: Ord + Sync + 'static,
{
    fn check_add(&self, rows: &BTreeMap<u64, T>, value: &T) -> Result<()> {
        let guard = self.read().unwrap();
        guard.check_add(rows, value)
    }

    fn check_update(&self, rows: &BTreeMap<u64, T>, id: u64, new_value: &T) -> Result<()> {
        let guard = self.read().unwrap();
        guard.check_update(rows, id, new_value)
    }

    fn check_remove(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let guard = self.read().unwrap();
        guard.check_remove(rows, id)
    }

    fn add_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let mut guard = self.write().unwrap();
        guard.add_entry(rows, id)
    }

    fn update_entry(&self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()> {
        let mut guard = self.write().unwrap();
        guard.update_entry(rows, id, old_entry)
    }

    fn remove_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let mut guard = self.write().unwrap();
        guard.remove_entry(rows, id)
    }
}