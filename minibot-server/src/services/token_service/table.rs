use super::TokenService;
use crate::util::table::{Error as TableError, Index, Table, Uniqueness};
use async_trait::async_trait;
use rand::RngCore;

fn make_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::encode_config(&bytes, base64::URL_SAFE_NO_PAD)
}

#[derive(Clone, Debug)]
pub struct Entry<T> {
    token: String,
    value: T,
}

pub struct TableTokenService<T> {
    table: Table<Entry<T>>,
    token_index: Index<Entry<T>, String>,
}

impl<T> TableTokenService<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new() -> Result<Self, TableError> {
        let mut table: Table<Entry<T>> = Table::new();
        let token_index = table.add_index_borrowed(Uniqueness::Unique, |v| &v.token)?;

        Ok(TableTokenService { table, token_index })
    }
}

#[async_trait]
impl<T> TokenService<T> for TableTokenService<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Return a token for the given info value. This token must be a url-safe
    /// string. `self.from_token()` must return the same value.
    async fn to_token(&self, value: T) -> anyhow::Result<String> {
        let token = make_token();
        self.table.add(Entry {
            token: token.clone(),
            value,
        })?;
        Ok(token)
    }

    /// Return a value of type T for a given token.
    ///
    /// A real implementation must ensure that the token has not been modified
    /// externally, or return an error otherwise.
    async fn from_token(&self, token: &str) -> anyhow::Result<Option<T>> {
        let mut values = self.token_index.get_values(token)?;
        match values.pop() {
            Some(entry) => Ok(Some(entry.value)),
            None => Ok(None),
        }
    }
}
