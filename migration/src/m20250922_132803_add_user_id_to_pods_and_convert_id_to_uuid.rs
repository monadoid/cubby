use loco_rs::schema::*;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Ensure Postgres can generate UUID values
        m.get_connection()
            .execute(Statement::from_string(
                m.get_database_backend(),
                r#"CREATE EXTENSION IF NOT EXISTS "pgcrypto""#.to_owned(),
            ))
            .await?;

        // Drop the auto-increment id column and add UUID id column
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .drop_column(Alias::new("id"))
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .add_column(
                    ColumnDef::new(Alias::new("id"))
                        .uuid()
                        .not_null()
                        .primary_key()
                        .default(Expr::cust("gen_random_uuid()"))
                )
                .to_owned(),
        )
        .await?;

        // Add user_id column (nullable since users can exist without pods)
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .add_column(ColumnDef::new(Alias::new("user_id")).uuid().null())
                .to_owned(),
        )
        .await?;

        // Add foreign key constraint to users table
        m.create_foreign_key(
            ForeignKey::create()
                .name("fk_pods_user_id")
                .from(Alias::new("pods"), Alias::new("user_id"))
                .to(Alias::new("users"), Alias::new("id"))
                .on_delete(ForeignKeyAction::SetNull)  // Allow user deletion without deleting pods
                .on_update(ForeignKeyAction::NoAction)
                .to_owned(),
        )
        .await?;

        // Add unique constraint to ensure one pod per user
        m.create_index(
            Index::create()
                .name("uq_pods_user_id")
                .table(Alias::new("pods"))
                .col(Alias::new("user_id"))
                .unique()
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Drop unique constraint
        m.drop_index(
            Index::drop()
                .name("uq_pods_user_id")
                .table(Alias::new("pods"))
                .to_owned(),
        )
        .await?;

        // Drop foreign key
        m.drop_foreign_key(
            ForeignKey::drop()
                .name("fk_pods_user_id")
                .table(Alias::new("pods"))
                .to_owned(),
        )
        .await?;

        // Drop user_id column
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .drop_column(Alias::new("user_id"))
                .to_owned(),
        )
        .await?;

        // Drop UUID id column and recreate auto-increment id
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .drop_column(Alias::new("id"))
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .add_column(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .not_null()
                        .auto_increment()
                        .primary_key()
                )
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}
