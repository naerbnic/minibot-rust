mod accessor_result;
mod error;
mod index_set;
mod index_store;
mod table_core;

use accessor_result::AccessorResult;
pub use error::{Error, Result};
use index_store::IndexStore;
pub use index_store::Uniqueness;
use table_core::TableCore;

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

    pub fn add_index_borrowed<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Result<Index<T, K>>
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple_add_get() -> Result<()> {
        let table = Table::new();
        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("goodbye".to_string())?;

        assert_eq!(Some("goodbye".to_string()), table.get(id2)?);
        assert_eq!(Some("hello".to_string()), table.get(id1)?);

        Ok(())
    }

    #[test]
    fn test_index_lookup() -> Result<()> {
        let mut table = Table::<String>::new();
        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v)?;

        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("goodbye".to_string())?;

        assert_eq!(
            vec![id1],
            content_index
                .get_entries("hello")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            vec![id2],
            content_index
                .get_entries("goodbye")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn test_late_index() -> Result<()> {
        let mut table = Table::<String>::new();

        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("goodbye".to_string())?;

        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v)?;

        assert_eq!(
            vec![id1],
            content_index
                .get_entries("hello")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            vec![id2],
            content_index
                .get_entries("goodbye")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn test_equal_index() -> Result<()> {
        let mut table = Table::<String>::new();

        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v)?;

        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("hello".to_string())?;

        assert_ne!(id1, id2);

        assert_eq!(vec![id1, id2], content_index.get_ids("hello")?);
        assert_eq!(Vec::<u64>::new(), content_index.get_ids("goodbye")?);
        Ok(())
    }

    #[test]
    fn test_unique_index() -> Result<()> {
        let mut table = Table::<String>::new();

        let _content_index = table.add_index_borrowed(Uniqueness::Unique, |v| v)?;

        table.add("hello".to_string())?;
        assert!(matches!(
            table.add("hello".to_string()),
            Err(Error::AlreadyExists)
        ));
        Ok(())
    }
}
