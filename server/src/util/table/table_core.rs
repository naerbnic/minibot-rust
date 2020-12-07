use super::{
    accessor_result::AccessorResult,
    error::{Error, Result},
    index_set::IndexSet,
    index_store::{IndexStore, Uniqueness},
};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

// This is the core of the Table implementation, which requires a mutable reference to mutate it.
pub struct TableCore<T> {
    next_id: u64,
    rows: BTreeMap<u64, T>,
    indexes: IndexSet<T>,
}

impl<T> TableCore<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        TableCore {
            next_id: 0,
            rows: BTreeMap::new(),
            indexes: IndexSet::new(),
        }
    }

    pub fn rows(&self) -> &BTreeMap<u64, T> {
        &self.rows
    }

    pub fn add_entry(&mut self, value: T) -> Result<u64> {
        let new_id = self.next_id;
        assert!(!self.rows.contains_key(&new_id));

        let rows = &self.rows;
        self.indexes.apply(|index| index.check_add(rows, &value))?;
        self.rows.insert(new_id, value);
        self.next_id += 1;
        let rows = &self.rows;
        self.indexes.apply(|index| index.add_entry(rows, new_id))?;
        Ok(new_id)
    }

    pub fn get_entry(&self, id: u64) -> Result<Option<T>> {
        Ok(self.rows.get(&id).cloned())
    }

    pub fn update_entry(&mut self, id: u64, value: T) -> Result<()> {
        if !self.rows.contains_key(&id) {
            return Err(Error::UpdatedNonexistentEntry(id));
        }

        let rows = &self.rows;
        self.indexes
            .apply(|index| index.check_update(rows, id, &value))?;
        let old = self.rows.insert(id, value).unwrap();
        let rows = &self.rows;
        self.indexes
            .apply(|index| index.update_entry(rows, id, &old))?;

        Ok(())
    }

    pub fn remove_entry(&mut self, id: u64) -> Result<T> {
        if !self.rows.contains_key(&id) {
            return Err(Error::RemovingNonexistentId(id));
        }

        let rows = &self.rows;
        self.indexes.apply(|index| index.check_remove(rows, id))?;
        self.indexes.apply(|index| index.remove_entry(rows, id))?;
        Ok(self.rows.remove(&id).unwrap())
    }

    pub fn get_ids(&self) -> Vec<u64> {
        self.rows.keys().cloned().collect()
    }

    pub fn add_index_inner<F, K>(
        &mut self,
        unique: Uniqueness,
        accessor: F,
    ) -> Result<Arc<RwLock<IndexStore<T, K>>>>
    where
        F: for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        let store = IndexStore::new(&self.rows, unique, accessor)?;

        let store_handle = Arc::new(RwLock::new(store));

        self.indexes.insert(&store_handle);

        Ok(store_handle)
    }
}
