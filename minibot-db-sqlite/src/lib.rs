#[macro_use]
extern crate diesel;

mod schema;
mod models;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}