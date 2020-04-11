mod accessor_result;
mod error;
mod index_set;
mod index_store;
mod table;
mod table_core;

pub use error::{Error, Result};
pub use index_store::Uniqueness;
pub use table::{Index, Table};

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

        assert_eq!(vec![id1], content_index.get_ids("hello")?);
        assert_eq!(vec![id2], content_index.get_ids("goodbye")?);
        Ok(())
    }

    #[test]
    fn test_late_index() -> Result<()> {
        let mut table = Table::<String>::new();

        let id1 = table.add("hello".to_string())?;
        let id2 = table.add("goodbye".to_string())?;

        let content_index = table.add_index_borrowed(Uniqueness::NotUnique, |v| v)?;

        assert_eq!(vec![id1], content_index.get_ids("hello")?);
        assert_eq!(vec![id2], content_index.get_ids("goodbye")?);
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
