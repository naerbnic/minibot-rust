use super::error::Result;
use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

/// An internal trait that allows a table to update an index.
///
/// Each operation returns a boxed operation which allows for a rollback for this operation
pub trait IndexUpdater<T> {
    fn check_add(&self, rows: &BTreeMap<u64, T>, value: &T) -> Result<()>;
    fn check_update(&self, rows: &BTreeMap<u64, T>, id: u64, new_value: &T) -> Result<()>;
    fn check_remove(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;

    fn add_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
    fn update_entry(&self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()>;
    fn remove_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
}

pub struct IndexSet<T>(Vec<Weak<dyn IndexUpdater<T> + Send + Sync>>);

impl<T> IndexSet<T> {
    pub fn new() -> Self {
        IndexSet(Vec::new())
    }

    fn retain_valid_indexes(&mut self) {
        self.0.retain(|index| index.upgrade().is_some());
    }

    pub fn apply<F>(&mut self, check_f: F) -> Result<()>
    where
        F: Fn(&dyn IndexUpdater<T>) -> Result<()>,
    {
        for index in &mut self.0 {
            if let Some(index) = index.upgrade() {
                check_f(&*index)?;
            }
        }

        self.retain_valid_indexes();

        Ok(())
    }
}

impl<T> IndexSet<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn insert<H>(&mut self, index: &Arc<H>)
    where
        H: IndexUpdater<T> + Send + Sync + 'static,
    {
        let weak_handle = Arc::downgrade(index);
        self.0.push(weak_handle);
    }
}
