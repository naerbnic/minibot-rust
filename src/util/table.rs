use futures::lock::Mutex;
use std::any::Any;
use std::collections::BTreeMap;

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

// This is the core of the Table implementation, which requires a mutable reference to mutate it.
struct TableCore<T> {
    next_id: u64,
    rows: BTreeMap<u64, T>,
    indexes: BTreeMap<String, Box<dyn Index<T>>>,
}

trait Index<T> {
    fn add_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
    fn get_entries(
        &self,
        rows: &BTreeMap<u64, T>,
        value: &(dyn Any + 'static + Send + Sync),
    ) -> Result<Vec<u64>>;
    fn update_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()>;
    fn remove_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()>;
}

impl<T: Clone> TableCore<T> {
    pub fn new(indexes: BTreeMap<String, Box<dyn Index<T>>>) -> Self {
        TableCore {
            next_id: 0,
            rows: BTreeMap::new(),
            indexes,
        }
    }

    async fn add_entry(&mut self, value: T) -> Result<u64> {
        let new_id = self.next_id;
        self.next_id += 1;
        self.rows.insert(new_id, value);

        for index in self.indexes.values_mut() {
            index.add_entry(&self.rows, new_id)?;
        }

        Ok(new_id)
    }

    async fn get_entry(&self, id: u64) -> Result<Option<T>> {
        Ok(self.rows.get(&id).cloned())
    }

    async fn get_entries_by_index(
        &self,
        index_name: &str,
        value: &(dyn Any + 'static + Send + Sync),
    ) -> Result<Vec<(u64, T)>> {
        let index = self
            .indexes
            .get(index_name)
            .ok_or_else(|| Error::UnknownIndex(index_name.to_string()))?;

        let found_entries = index.get_entries(&self.rows, value)?;
        Ok(found_entries
            .into_iter()
            .map(|i| (i, self.rows.get(&i).unwrap().clone()))
            .collect())
    }

    async fn update_entry(&mut self, id: u64, value: T) -> Result<()> {
        match self.rows.insert(id, value) {
            Some(old) => {
                for index in self.indexes.values_mut() {
                    index.update_entry(&self.rows, id, &old)?;
                }
                Ok(())
            }
            None => {
                self.rows.remove(&id).unwrap();
                return Err(Error::UpdatedNonexistentEntry(id));
            }
        }
    }

    async fn remove_entry(&mut self, id: u64) -> Result<T> {
        if !self.rows.contains_key(&id) {
            return Err(Error::RemovingNonexistentId(id));
        }
        for index in self.indexes.values_mut() {
            index.remove_entry(&self.rows, id)?;
        }
        Ok(self.rows.remove(&id).unwrap())
    }
}

struct IndexImpl<T, K> {
    accessor: Box<dyn for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync>,
    entries: Vec<u64>,
}

impl<T, K> IndexImpl<T, K>
where
    K: Ord,
{
    fn find_range(&self, key: &K, rows: &BTreeMap<u64, T>) -> std::ops::Range<usize> {
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
}

fn entry_finder<'a, T, F, K>(
    accessor: F,
    rows: &'a BTreeMap<u64, T>,
    value: &'a K,
) -> impl Fn(&u64) -> std::cmp::Ordering + 'a
where
    F: for<'b> Fn(&'b T) -> AccessorResult<'b, K> + 'a,
    K: Ord,
{
    move |target_id| {
        let target_cow = accessor(rows.get(target_id).unwrap());
        let target = target_cow.as_ref();
        target.cmp(value)
    }
}

fn entry_cmp<'a, T, F, K>(
    accessor: F,
    rows: &'a BTreeMap<u64, T>,
) -> impl Fn(&u64, &u64) -> std::cmp::Ordering + 'a
where
    F: for<'b> Fn(&'b T) -> AccessorResult<'b, K> + 'a,
    K: Clone + Ord,
{
    move |left_id, right_id| {
        let left_cow = accessor(rows.get(left_id).unwrap());
        let left = left_cow.as_ref();
        let right_cow = accessor(rows.get(right_id).unwrap());
        let right = right_cow.as_ref();
        left.cmp(right)
    }
}

impl<T, K> Index<T> for IndexImpl<T, K>
where
    T: Send + Sync,
    K: Ord + Sync + 'static,
{
    fn add_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64) -> Result<()> {
        let new_entry_key_cow = (self.accessor)(rows.get(&id).unwrap());
        let new_entry_key = new_entry_key_cow.as_ref();
        let range = self.find_range(new_entry_key, rows);

        self.entries.insert(range.end, id);
        Ok(())
    }

    fn get_entries(
        &self,
        rows: &BTreeMap<u64, T>,
        value: &(dyn Any + 'static + Send + Sync),
    ) -> Result<Vec<u64>> {
        let value_ref = value.downcast_ref::<K>().ok_or(Error::WrongIndexType)?;

        let range = self.find_range(value_ref, rows);

        Ok(self.entries[range].iter().copied().collect())
    }

    fn update_entry(&mut self, rows: &BTreeMap<u64, T>, id: u64, old_entry: &T) -> Result<()> {
        let old_entry_key_cow = (self.accessor)(old_entry);
        let old_entry_key = old_entry_key_cow.as_ref();
        let range = self.find_range(old_entry_key, rows);

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
        let range = self.find_range(old_entry_key, rows);

        let index = self.entries[range.clone()]
            .iter()
            .position(|i| i == &id)
            .unwrap();
        self.entries.remove(range.start + index);
        Ok(())
    }
}

pub struct Table<T>(Mutex<TableCore<T>>);

impl<T: Clone> Table<T> {
    pub fn builder() -> TableBuilder<T> {
        TableBuilder {
            indexes: BTreeMap::new(),
        }
    }

    pub async fn add(&self, value: T) -> Result<u64> {
        let mut guard = self.0.lock().await;
        guard.add_entry(value).await
    }

    pub async fn get(&self, id: u64) -> Result<Option<T>> {
        let guard = self.0.lock().await;
        guard.get_entry(id).await
    }

    pub async fn get_by_index<V>(&self, index_name: &str, value: &V) -> Result<Vec<(u64, T)>>
    where
        V: Any + 'static + Send + Sync,
    {
        let guard = self.0.lock().await;
        guard.get_entries_by_index(index_name, value).await
    }

    pub async fn update(&self, id: u64, new_value: T) -> Result<()> {
        let mut guard = self.0.lock().await;
        guard.update_entry(id, new_value).await
    }

    pub async fn remove(&self, id: u64) -> Result<T> {
        let mut guard = self.0.lock().await;
        guard.remove_entry(id).await
    }
}

pub struct TableBuilder<T> {
    indexes: BTreeMap<String, Box<dyn Index<T>>>,
}

impl<T> TableBuilder<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn add_index_inner<F, K>(&mut self, name: &str, accessor: F) -> &mut Self
    where
        F: for<'a> Fn(&'a T) -> AccessorResult<'a, K> + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        let index = Box::new(IndexImpl {
            accessor: Box::new(accessor),
            entries: Vec::new(),
        });
        self.indexes.insert(name.to_string(), index);
        self
    }

    pub fn add_index_borrowed<F, K>(&mut self, name: &str, accessor: F) -> &mut Self
    where
        F: for<'a> Fn(&'a T) -> &K + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        self.add_index_inner(name, move |t| AccessorResult::Borrowed(accessor(t)))
    }

    pub fn add_index_owned<F, K>(&mut self, name: &str, accessor: F) -> &mut Self
    where
        F: for<'a> Fn(&'a T) -> K + Send + Sync + 'static,
        K: Ord + Sync + 'static,
    {
        self.add_index_inner(name, move |t| -> AccessorResult<K> {
            AccessorResult::Owned(accessor(t))
        })
    }

    pub fn build(&mut self) -> Table<T> {
        Table(Mutex::new(TableCore::new(std::mem::replace(
            &mut self.indexes,
            BTreeMap::new(),
        ))))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    fn make_table() -> Table<String> {
        Table::<String>::builder()
            .add_index_borrowed("string", |v| &*v)
            .build()
    }

    #[tokio::test]
    async fn simple_add_remove() -> Result<()> {
        let table = make_table();
        let id1 = table.add("hello".to_string()).await?;
        let id2 = table.add("goodbye".to_string()).await?;

        assert_eq!(Some("goodbye".to_string()), table.get(id2).await?);
        assert_eq!(Some("hello".to_string()), table.get(id1).await?);

        Ok(())
    }
}
