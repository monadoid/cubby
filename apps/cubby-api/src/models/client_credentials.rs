use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveModelTrait, ActiveValue, ConnectionTrait, DbErr, EntityTrait, ModelTrait, QueryFilter, ColumnTrait};
use serde_json::{json, Value};
use uuid::Uuid;

pub use super::_entities::client_credentials::{self, ActiveModel, Entity, Model};

#[derive(Debug)]
pub struct CreateParams<'a> {
    pub user_id: Uuid,
    pub client_id: &'a str,
    pub client_secret_last_four: Option<&'a str>,
    pub description: Option<&'a str>,
    pub scopes: &'a [String],
    pub status: &'a str,
    pub trusted_metadata: Option<&'a Value>,
}

#[derive(Debug)]
pub struct UpdateSecretParams<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub client_secret_last_four: Option<&'a str>,
    pub status: Option<&'a str>,
}

impl Model {
    pub fn scopes(&self) -> Vec<String> {
        self.scopes
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn list_for_user(db: &DatabaseConnection, user_id: Uuid) -> ModelResult<Vec<Model>> {
        Entity::find()
            .filter(client_credentials::Column::UserId.eq(user_id))
            .all(db)
            .await
            .map_err(ModelError::from)
    }

    pub async fn find_by_id_and_user(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
    ) -> ModelResult<Model> {
        Entity::find()
            .filter(client_credentials::Column::Id.eq(id))
            .filter(client_credentials::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or(ModelError::EntityNotFound)
    }

    pub async fn delete_by_id_and_user(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
    ) -> ModelResult<()> {
        let model = Self::find_by_id_and_user(db, id, user_id).await?;
        model.delete(db).await.map(|_| ()).map_err(ModelError::from)
    }
}

pub async fn create(db: &DatabaseConnection, params: CreateParams<'_>) -> ModelResult<Model> {
    let scopes_json = json!(params.scopes);
    let active = ActiveModel {
        id: ActiveValue::set(Uuid::new_v4()),
        user_id: ActiveValue::set(params.user_id),
        client_id: ActiveValue::set(params.client_id.to_owned()),
        client_secret_last_four: ActiveValue::set(
            params.client_secret_last_four.map(str::to_owned),
        ),
        description: ActiveValue::set(params.description.map(str::to_owned)),
        scopes: ActiveValue::set(scopes_json.into()),
        status: ActiveValue::set(params.status.to_owned()),
        trusted_metadata: ActiveValue::set(params.trusted_metadata.cloned()),
        ..Default::default()
    };

    active.insert(db).await.map_err(ModelError::from)
}

pub async fn update_secret(
    db: &DatabaseConnection,
    params: UpdateSecretParams<'_>,
) -> ModelResult<Model> {
    let model = Model::find_by_id_and_user(db, params.id, params.user_id).await?;
    let mut active = model.into_active_model();
    if let Some(last_four) = params.client_secret_last_four {
        active.client_secret_last_four = ActiveValue::set(Some(last_four.to_owned()));
    }
    if let Some(status) = params.status {
        active.status = ActiveValue::set(status.to_owned());
    }
    active.updated_at = ActiveValue::set(Utc::now().into());
    active.update(db).await.map_err(ModelError::from)
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if !insert && self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}
