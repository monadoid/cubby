use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Add DPoP key fields to pods table
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .add_column(ColumnDef::new(Alias::new("dpop_private_jwk")).text().null())
                .add_column(ColumnDef::new(Alias::new("dpop_public_jwk_thumbprint")).string().null())
                .add_column(ColumnDef::new(Alias::new("dpop_key_created_at")).timestamp_with_time_zone().null())
                .add_column(ColumnDef::new(Alias::new("dpop_key_rotated_at")).timestamp_with_time_zone().null())
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Remove DPoP key fields from pods table
        m.alter_table(
            Table::alter()
                .table(Alias::new("pods"))
                .drop_column(Alias::new("dpop_private_jwk"))
                .drop_column(Alias::new("dpop_public_jwk_thumbprint"))
                .drop_column(Alias::new("dpop_key_created_at"))
                .drop_column(Alias::new("dpop_key_rotated_at"))
                .to_owned(),
        )
        .await?;

        Ok(())
    }
}
