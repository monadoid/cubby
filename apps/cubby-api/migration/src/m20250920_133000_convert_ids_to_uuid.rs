use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let backend = m.get_database_backend();
        let conn = m.get_connection();

        conn.execute(Statement::from_string(
            backend,
            r#"CREATE EXTENSION IF NOT EXISTS "pgcrypto""#.to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies DROP CONSTRAINT IF EXISTS fk_movies_user_id".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "DROP INDEX IF EXISTS uq_users_pid".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users DROP CONSTRAINT IF EXISTS users_pkey".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users DROP COLUMN id".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users RENAME COLUMN pid TO id".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users ALTER COLUMN id SET DEFAULT gen_random_uuid()".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users ALTER COLUMN id SET NOT NULL".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE users ADD PRIMARY KEY (id)".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies ADD COLUMN new_id uuid DEFAULT gen_random_uuid()".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "UPDATE movies SET new_id = gen_random_uuid() WHERE new_id IS NULL".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies ALTER COLUMN new_id SET NOT NULL".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies DROP CONSTRAINT IF EXISTS movies_pkey".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies DROP COLUMN id".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies RENAME COLUMN new_id TO id".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies ALTER COLUMN id SET DEFAULT gen_random_uuid()".to_owned(),
        ))
        .await?;

        conn.execute(Statement::from_string(
            backend,
            "ALTER TABLE movies ADD PRIMARY KEY (id)".to_owned(),
        ))
        .await?;

        m.create_foreign_key(
            ForeignKey::create()
                .name("fk_movies_user_id")
                .from(Alias::new("movies"), Alias::new("user_id"))
                .to(Alias::new("users"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade)
                .on_update(ForeignKeyAction::NoAction)
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "m20250920_133000_convert_ids_to_uuid cannot be reverted".to_owned(),
        ))
    }
}
