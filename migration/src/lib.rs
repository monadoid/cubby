#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;

mod m20250920_121508_movies;
mod m20250920_123552_add_user_id_to_movies;
mod m20250920_133000_convert_ids_to_uuid;
mod m20250920_145957_drop_name_from_users;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_users::Migration),
            Box::new(m20250920_121508_movies::Migration),
            Box::new(m20250920_123552_add_user_id_to_movies::Migration),
            Box::new(m20250920_133000_convert_ids_to_uuid::Migration),
            Box::new(m20250920_145957_drop_name_from_users::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}