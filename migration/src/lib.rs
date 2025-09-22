#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;

mod m20250920_121508_movies;
mod m20250920_123552_add_user_id_to_movies;
mod m20250920_133000_convert_ids_to_uuid;
mod m20250920_145957_drop_name_from_users;
mod m20250921_183731_add_auth_id;
mod m20250921_190534_add_client_credentials;
mod m20250921_205200_remove_uuid_defaults;
mod m20250922_132601_pods;
mod m20250922_132803_add_user_id_to_pods_and_convert_id_to_uuid;
mod m20250922_144444_add_pod_client_credentials;
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
            Box::new(m20250921_183731_add_auth_id::Migration),
            Box::new(m20250921_190534_add_client_credentials::Migration),
            Box::new(m20250921_205200_remove_uuid_defaults::Migration),
            Box::new(m20250922_132601_pods::Migration),
            Box::new(m20250922_132803_add_user_id_to_pods_and_convert_id_to_uuid::Migration),
            Box::new(m20250922_144444_add_pod_client_credentials::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}