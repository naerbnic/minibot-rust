#![allow(dead_code)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod crud;
mod db_handle;
mod model;
mod schema;

embed_migrations!();
