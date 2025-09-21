use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .add_column_if_not_exists(
                    ColumnDef::new(Users::AuthId)
                        .string()
                        .null(),
                )
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .drop_column(Users::AuthId)
                .to_owned(),
        )
        .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    AuthId,
}
