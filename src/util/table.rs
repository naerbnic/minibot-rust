use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, Weak};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Passed the wrong value type into an index.")]
    WrongIndexType,

    #[error("Unknown index: {0}")]
    UnknownIndex(String),

    #[error("Tried to update nonexistent entry: {0}")]
    UpdatedNonexistentEntry(u64),

    #[error("Tried to remove nonexistent entry: {0}")]
    RemovingNonexistentId(u64),

    #[error("Entry already exists.")]
    AlreadyExists,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A helper type similar to Cow, but can only access the reference of the containing value.
/// Does not require Clone on T.
enum AccessorResult<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> AccessorResult<'_, T> {
    fn as_ref(&self) -> &T {
        match self {
            AccessorResult::Borrowed(v) => v,
            AccessorResult::Owned(v) => v,
        }
    }
}

struct IndexSet<T>(Vec<Weak<dyn IndexUpdater<T>>>);

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
    pub fn insert<K>(&mut self, index: &IndexStoreHandle<T, K>)
    where
        K: Ord + Sync + 'static,
    {
        let weak_handle = Arc::downgrade(index);
        self.0.push(weak_handle);
    }
}

// This is the core of the Table implementation, which requires a mutable reference to mutate it.
struct TableCore<T> {
    next_id: u64,
    rows: BTreeMap<u64, T>,
    indexes: IndexSet<T>,
}

/// An internal trait that allows a table to update an index.
///
/// Each operation returns a boxed operation which allows for a rollback for this operation
trait IndexUpdater<T> {
    fn check_add(&self, rows: &BTreeMap<u64, T>, value: &T) -> Result<()>;
    fn check_update(&self, rows: &BTreeMap<u64, T>, id: u64, new_value: &T) -> Result<()>;
    fn check_remove(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;

    fn add_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
    fn update_entry(&self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()>;
    fn remove_entry(&self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
}

impl<T: Clone> TableCore<T> {
    pub fn new() -> Self {
        TableCore {
            next_id: 0,
            rows: BTreeMap::new(),
            indexes: IndexSet::new(),
        }
    }

    fn add_entry(&mut self, value: T) -> Result<u64> {
        let new_id = self.next_id;
        if self.rows.contains_key(&new_id) {
            return Err(Error::AlreadyExists);
        }

        let rows = &self.rows;
        self.indexes.apply(|index| index.check_add(rows, &value))?;
        self.rows.insert(new_id, value);
        self.next_id += 1;
        let rows = &self.rows;
        self.indexes.apply(|index| index.add_entry(rows, new_id))?;
        Ok(new_id)
    }

    fn get_entry(&self, id: u64) -> Result<Option<T>> {
        Ok(self.rows.get(&id).cloned())
    }

    fn update_entry(&mut self, id: u64, value: T) -> Result<()> {
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

    fn remove_entry(&mut self, id: u64) -> Result<T> {
        if !self.rows.contains_key(&id) {
            return Err(Error::RemovingNonexistentId(id));
        }

        let rows = &self.rows;
        self.indexes.apply(|index| index.check_remove(rows, id))?;
        self.indexes.apply(|index| index.remove_entry(rows, id))?;
        Ok(self.rows.remove(&id).unwrap())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Uniqueness {
    Unique,
    NotUnique,
}

struct IndexStore<T, K> {
    accessor: Box<dyn for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync>,
    entries: Vec<u64>,
    unique: Uniqueness,
}

impl<T, K> IndexStore<T, K>
where
    K: Ord,
{
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
        if let Uniqueness::NotUnique = self.unique {
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
        if let Uniqueness::NotUnique = self.unique {
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

    fn add_index_inner<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Index<T, K>
    where
        F: for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        let new_table_handle = self.0.clone();
        let mut guard = self.0.write().unwrap();

        let mut entries = guard.rows.keys().cloned().collect::<Vec<_>>();

        entries.sort_by(entry_cmp(&accessor, &guard.rows));

        let store = IndexStore {
            accessor: Box::new(accessor),
            entries,
            unique,
        };

        let store_handle = Arc::new(RwLock::new(store));

        guard.indexes.insert(&store_handle);

        Index {
            table: new_table_handle,
            index: store_handle,
        }
    }

    pub fn add_index_borrowed<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Index<T, K>
    where
        F: for<'a> Fn(&'a T) -> &K + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        self.add_index_inner(unique, move |t| AccessorResult::Borrowed(accessor(t)))
    }

    pub fn add_index_owned<F, K>(&mut self, unique: Uniqueness, accessor: F) -> Index<T, K>
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
    T: Clone,
    K: Ord,
{
    pub fn get_entries<Q>(&self, value: &Q) -> Result<Vec<(u64, T)>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        // Order is important here to avoid deadlock: Grab the table then the index.
        let table_guard = self.table.read().unwrap();
        let index_guard = self.index.read().unwrap();

        let rows = &table_guard.rows;

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
        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v);

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

        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v);

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

    fn test_equal_index() -> Result<()> {
        let mut table = Table::<String>::new();

        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v);

        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("hello".to_string())?;

        assert_ne!(id1, id2);

        assert_eq!(
            vec![id1, id2],
            content_index
                .get_entries("hello")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            Vec::<u64>::new(),
            content_index
                .get_entries("goodbye")?
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        Ok(())
    }
}
