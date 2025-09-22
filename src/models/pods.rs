use loco_rs::prelude::*;
use sea_orm::PaginatorTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::_entities::pods::{self, ActiveModel, Entity, Model};

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePodParams {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePodParams {
    pub name: Option<String>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, insert: bool) -> std::result::Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            let mut this = self;
            // Only set ID if it's not already provided
            if matches!(this.id, ActiveValue::NotSet) {
                this.id = ActiveValue::Set(Uuid::new_v4());
            }
            Ok(this)
        } else if self.updated_at.is_unchanged() {
            let mut this = self;
            this.updated_at = ActiveValue::Set(chrono::Utc::now().into());
            Ok(this)
        } else {
            Ok(self)
        }
    }
}

impl Model {
    /// Find pod by ID and user ownership
    pub async fn find_by_id_and_user(
        db: &DatabaseConnection,
        id: &str,
        user_id: Uuid,
    ) -> ModelResult<Self> {
        let parse_uuid = Uuid::parse_str(id).map_err(|e| ModelError::Any(e.into()))?;
        let pod = pods::Entity::find()
            .filter(
                model::query::condition()
                    .eq(pods::Column::Id, parse_uuid)
                    .eq(pods::Column::UserId, user_id)
                    .build(),
            )
            .one(db)
            .await?;
        pod.ok_or_else(|| ModelError::EntityNotFound)
    }

    /// Find pod by user ID
    pub async fn find_by_user(db: &DatabaseConnection, user_id: Uuid) -> ModelResult<Option<Self>> {
        let pod = pods::Entity::find()
            .filter(
                model::query::condition()
                    .eq(pods::Column::UserId, user_id)
                    .build(),
            )
            .one(db)
            .await?;
        Ok(pod)
    }

    /// Check if user already has a pod
    pub async fn user_has_pod(db: &DatabaseConnection, user_id: Uuid) -> ModelResult<bool> {
        let count = pods::Entity::find()
            .filter(
                model::query::condition()
                    .eq(pods::Column::UserId, user_id)
                    .build(),
            )
            .count(db)
            .await?;
        Ok(count > 0)
    }

    /// Create pod with CSS provisioning data
    pub async fn create_with_css_data(
        db: &DatabaseConnection,
        user_id: Uuid,
        params: &CreatePodParams,
        css_result: &crate::data::solid_server::CssProvisioningResult,
    ) -> ModelResult<Self> {
        let active_model = ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            name: ActiveValue::Set(Some(params.name.clone())),
            link: ActiveValue::Set(Some(css_result.pod_base_url.clone())),
            user_id: ActiveValue::Set(Some(user_id)),
            css_account_token: ActiveValue::Set(Some(css_result.account_token.clone())),
            css_client_id: ActiveValue::Set(Some(css_result.client_id.clone())),
            css_client_secret: ActiveValue::Set(Some(css_result.client_secret.clone())),
            css_client_resource_url: ActiveValue::Set(Some(css_result.client_resource_url.clone())),
            webid: ActiveValue::Set(Some(css_result.web_id.clone())),
            css_email: ActiveValue::Set(Some(css_result.css_email.clone())),
            ..Default::default()
        };
        
        active_model.insert(db).await.map_err(ModelError::from)
    }
}

impl ActiveModel {}

impl Entity {}
