use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};
use serde_json::json;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let backend = m.get_database_backend();
        let conn = m.get_connection();

        // Ensure existing rows have an auth_id before we tighten constraints
        conn.execute(Statement::from_string(
            backend,
            "UPDATE users SET auth_id = COALESCE(auth_id, id::text)".to_owned(),
        ))
        .await?;

        // Drop legacy password column – credentials are now owned by Stytch
        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .drop_column(Users::Password)
                .to_owned(),
        )
        .await?;

        // auth_id becomes our stable binding to the Stytch user id
        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .modify_column(
                    ColumnDef::new(Users::AuthId)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .to_owned(),
        )
        .await?;

        // Table storing metadata about issued client credentials (secrets are not persisted)
        m.create_table(
            Table::create()
                .table(ClientCredentials::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ClientCredentials::Id)
                        .uuid()
                        .not_null()
                        .primary_key()
                        .default(Expr::cust("gen_random_uuid()")),
                )
                .col(
                    ColumnDef::new(ClientCredentials::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                )
                .col(
                    ColumnDef::new(ClientCredentials::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                )
                .col(ColumnDef::new(ClientCredentials::UserId).uuid().not_null())
                .col(
                    ColumnDef::new(ClientCredentials::ClientId)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(ClientCredentials::ClientSecretLastFour)
                        .string()
                        .null(),
                )
                .col(
                    ColumnDef::new(ClientCredentials::Description)
                        .string()
                        .null(),
                )
                .col(
                    ColumnDef::new(ClientCredentials::Scopes)
                        .json_binary()
                        .not_null()
                        .default(SimpleExpr::Value(Value::Json(Some(Box::new(json!([])))))),
                )
                .col(
                    ColumnDef::new(ClientCredentials::Status)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ClientCredentials::TrustedMetadata)
                        .json_binary()
                        .null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_client_credentials_user")
                        .from(ClientCredentials::Table, ClientCredentials::UserId)
                        .to(Users::Table, Users::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(ClientCredentials::Table).to_owned())
            .await?;

        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .modify_column(ColumnDef::new(Users::AuthId).string().null())
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Users::Table)
                .add_column(ColumnDef::new(Users::Password).string().null())
                .to_owned(),
        )
        .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Password,
    AuthId,
}

#[derive(DeriveIden)]
enum ClientCredentials {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    UserId,
    ClientId,
    ClientSecretLastFour,
    Description,
    Scopes,
    Status,
    TrustedMetadata,
}
