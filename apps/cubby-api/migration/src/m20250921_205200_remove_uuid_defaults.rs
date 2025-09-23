use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let backend = m.get_database_backend();
        let conn = m.get_connection();

        // Remove default UUID generation from users table so we can control UUIDs
        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users ALTER COLUMN id DROP DEFAULT".to_owned(),
        ))
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let backend = m.get_database_backend();
        let conn = m.get_connection();

        // Restore default UUID generation
        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users ALTER COLUMN id SET DEFAULT gen_random_uuid()".to_owned(),
        ))
        .await?;

        Ok(())
    }
}