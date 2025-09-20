use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Ensure Postgres can generate UUID values without application involvement.
        m.get_connection()
            .execute(Statement::from_string(
                m.get_database_backend(),
                r#"CREATE EXTENSION IF NOT EXISTS "pgcrypto""#.to_owned(),
            ))
            .await?;

        m.get_connection()
            .execute(Statement::from_string(
                m.get_database_backend(),
                "ALTER TABLE users ALTER COLUMN pid SET DEFAULT gen_random_uuid()".to_owned(),
            ))
            .await?;

        m.create_index(
            Index::create()
                .name("uq_users_pid")
                .table(Alias::new("users"))
                .col(Alias::new("pid"))
                .unique()
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Alias::new("movies"))
                .add_column(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                .to_owned(),
        )
        .await?;

        m.create_foreign_key(
            ForeignKey::create()
                .name("fk_movies_user_id")
                .from(Alias::new("movies"), Alias::new("user_id"))
                .to(Alias::new("users"), Alias::new("pid"))
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::NoAction)
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_foreign_key(
            ForeignKey::drop()
                .name("fk_movies_user_id")
                .table(Alias::new("movies"))
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Alias::new("movies"))
                .drop_column(Alias::new("user_id"))
                .to_owned(),
        )
        .await?;

        m.drop_index(
            Index::drop()
                .name("uq_users_pid")
                .table(Alias::new("users"))
                .to_owned(),
        )
        .await?;

        m.get_connection()
            .execute(Statement::from_string(
                m.get_database_backend(),
                "ALTER TABLE users ALTER COLUMN pid DROP DEFAULT".to_owned(),
            ))
            .await?;

        Ok(())
    }
}
