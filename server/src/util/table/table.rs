use super::{
    accessor_result::AccessorResult, error::Result, index_store::IndexStore,
    index_store::Uniqueness, table_core::TableCore,
};

use std::borrow::Borrow;
use std::sync::{Arc, RwLock};

type TableCoreHandle<T> = Arc<RwLock<TableCore<T>>>;
type IndexStoreHandle<T, K> = Arc<RwLock<IndexStore<T, K>>>;
pub struct Table<T>(TableCoreHandle<T>);

impl<T> Table<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Table(Arc::new(RwLock::new(TableCore::new())))
    }

    pub fn add(&self, value: T) -> Result<u64> {
        let mut guard = self.0.write().unwrap();
        guard.add_entry(value)
    }

    pub fn get(&self, id: u64) -> Result<Option<T>> {
        let guard = self.0.read().unwrap();
        guard.get_entry(id)
    }

    pub fn update(&self, id: u64, new_value: T) -> Result<()> {
        let mut guard = self.0.write().unwrap();
        guard.update_entry(id, new_value)
    }

    pub fn remove(&self, id: u64) -> Result<T> {
        let mut guard = self.0.write().unwrap();
        guard.remove_entry(id)
    }

    fn add_index_inner<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Result<Index<T, K>>
    where
        F: for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        let new_table_handle = self.0.clone();
        let mut guard = self.0.write().unwrap();
        let store_handle = guard.add_index_inner(unique, accessor)?;

        Ok(Index {
            table: new_table_handle,
            index: store_handle,
        })
    }

    pub fn add_index_borrowed<F, K>(
        &mut self,
        unique: Uniqueness,
        accessor: F,
    ) -> Result<Index<T, K>>
    where
        F: for<'a> Fn(&'a T) -> &K + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        self.add_index_inner(unique, move |t| AccessorResult::Borrowed(accessor(t)))
    }

    pub fn add_index_owned<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Result<Index<T, K>>
    where
        F: for<'a> Fn(&'a T) -> K + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        self.add_index_inner(unique, move |t| -> AccessorResult<K> {
            AccessorResult::Owned(accessor(t))
        })
    }
}

pub struct Index<T, K> {
    /// A reference to the internals of the table, under lock.
    ///
    /// This also has a handle to the index stores, however we only call methods on this that
    /// do not directly access the index, so accessing this mutex should be fine.
    table: TableCoreHandle<T>,
    index: IndexStoreHandle<T, K>,
}

impl<T, K> Index<T, K>
where
    T: Clone + Send + Sync + 'static,
    K: Ord,
{
    pub fn get_ids<Q>(&self, value: &Q) -> Result<Vec<u64>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        // Order is important here to avoid deadlock: Grab the table then the index.
        let table_guard = self.table.read().unwrap();
        let index_guard = self.index.read().unwrap();

        let rows = table_guard.rows();

        Ok(index_guard.get_entries(rows, value)?)
    }

    pub fn get_values<Q>(&self, value: &Q) -> Result<Vec<T>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        // Order is important here to avoid deadlock: Grab the table then the index.
        let table_guard = self.table.read().unwrap();
        let index_guard = self.index.read().unwrap();

        let rows = table_guard.rows();

        let ids = index_guard.get_entries(rows, value)?;
        Ok(ids
            .into_iter()
            .map(|id| rows.get(&id).cloned().unwrap())
            .collect())
    }

    pub fn get_entries<Q>(&self, value: &Q) -> Result<Vec<(u64, T)>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        // Order is important here to avoid deadlock: Grab the table then the index.
        let table_guard = self.table.read().unwrap();
        let index_guard = self.index.read().unwrap();

        let rows = table_guard.rows();

        let ids = index_guard.get_entries(rows, value)?;
        Ok(ids
            .into_iter()
            .map(|id| (id, rows.get(&id).cloned().unwrap()))
            .collect())
    }
}
