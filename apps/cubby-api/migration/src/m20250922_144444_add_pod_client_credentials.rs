use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Add client credentials fields to pods table
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .add_column(ColumnDef::new(Alias::new("css_account_token")).string().null())
                .add_column(ColumnDef::new(Alias::new("css_client_id")).string().null())
                .add_column(ColumnDef::new(Alias::new("css_client_secret")).string().null())
                .add_column(ColumnDef::new(Alias::new("css_client_resource_url")).string().null())
                .add_column(ColumnDef::new(Alias::new("webid")).string().null())
                .add_column(ColumnDef::new(Alias::new("css_email")).string().null())
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Remove client credentials fields from pods table
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .drop_column(Alias::new("css_account_token"))
                .drop_column(Alias::new("css_client_id"))
                .drop_column(Alias::new("css_client_secret"))
                .drop_column(Alias::new("css_client_resource_url"))
                .drop_column(Alias::new("webid"))
                .drop_column(Alias::new("css_email"))
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}

