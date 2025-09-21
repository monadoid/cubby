use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::users::{self, ActiveModel, Entity, Model};

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterParams {
    pub email: String,
    pub password: String,
}

#[derive(Debug)]
pub struct UpsertFromStytch<'a> {
    pub id: Uuid,
    pub auth_id: &'a str,
    pub email: &'a str,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for super::_entities::users::ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            let mut this = self;
            // Only set ID if it's not already provided
            if matches!(this.id, ActiveValue::NotSet) {
                this.id = ActiveValue::Set(Uuid::new_v4());
            }
            this.api_key = ActiveValue::Set(format!("lo-{}", Uuid::new_v4()));
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// finds a user by the provided email
    ///
    /// # Errors
    ///
    /// When could not find user by the given token or DB query error
    pub async fn find_by_email(db: &DatabaseConnection, email: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::Email, email)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// finds a user by the provided id
    ///
    /// # Errors
    ///
    /// When could not find user  or DB query error
    pub async fn find_by_id(db: &DatabaseConnection, id: &str) -> ModelResult<Self> {
        let parse_uuid = Uuid::parse_str(id).map_err(|e| ModelError::Any(e.into()))?;
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::Id, parse_uuid)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_by_auth_id(db: &DatabaseConnection, auth_id: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::AuthId, auth_id)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    pub async fn find_by_api_key(db: &DatabaseConnection, api_key: &str) -> ModelResult<Self> {
        let user = users::Entity::find()
            .filter(
                model::query::condition()
                    .eq(users::Column::ApiKey, api_key)
                    .build(),
            )
            .one(db)
            .await?;
        user.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// Ensures we have a local user record representing the provided Stytch user.
    pub async fn upsert_from_stytch(
        db: &DatabaseConnection,
        params: UpsertFromStytch<'_>,
    ) -> ModelResult<Self> {
        if let Ok(existing) = Self::find_by_auth_id(db, params.auth_id).await {
            if existing.email == params.email {
                return Ok(existing);
            }

            let mut active = existing.into_active_model();
            active.email = ActiveValue::set(params.email.to_owned());
            return active.update(db).await.map_err(ModelError::from);
        }

        if let Ok(existing) = Self::find_by_email(db, params.email).await {
            let mut active = existing.into_active_model();
            active.auth_id = ActiveValue::set(params.auth_id.to_owned());
            return active.update(db).await.map_err(ModelError::from);
        }

        tracing::debug!(uuid = %params.id, email = %params.email, "Creating user with specific UUID");
        
        let active_model = users::ActiveModel {
            id: ActiveValue::set(params.id),
            email: ActiveValue::set(params.email.to_owned()),
            auth_id: ActiveValue::set(params.auth_id.to_owned()),
            ..Default::default()
        };
        
        let result = active_model.insert(db).await.map_err(ModelError::from)?;
        
        tracing::debug!(created_uuid = %result.id, "User created with UUID");
        
        Ok(result)
    }
}
