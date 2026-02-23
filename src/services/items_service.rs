use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::db::organization::{get_organization_from_request, set_current_organization, set_current_user};
use crate::middleware::AuthenticatedUser;
use crate::models::ItemModel;
use crate::proto::common::Empty;
use crate::proto::items::items_service_server::ItemsService;
use crate::proto::items::{
    ChangeItemOwnershipReq, ConvertItemTypeReq, ConvertItemTypeRes, CreateItemReq, CreateItemRes,
    DeleteItemReq, GetItemReq, GetItemRes, Item, ListItemsReq, ListItemsRes, MoveItemReq,
    SearchByBarcodeReq, UpdateItemReq, UpdateItemRes,
};

pub struct ItemsServiceImpl {
    pool: PgPool,
}

impl ItemsServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn get_authenticated_user<T>(request: &Request<T>) -> Result<AuthenticatedUser, Status> {
        request
            .extensions()
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))
    }

    fn model_to_proto(model: &ItemModel) -> Item {
        Item {
            id: model.id.clone(),
            parent_id: model.parent_id.clone().unwrap_or_default(),
            owner_type: model.owner_type.clone(),
            organization_id: model.organization_id.clone().unwrap_or_default(),
            user_id: model.user_id.clone().unwrap_or_default(),
            name: model.name.clone(),
            barcode: model.barcode.clone().unwrap_or_default(),
            category: model.category.clone().unwrap_or_default(),
            description: model.description.clone().unwrap_or_default(),
            image_url: model.image_url.clone().unwrap_or_default(),
            url: model.url.clone().unwrap_or_default(),
            item_type: model.item_type.clone(),
            quantity: model.quantity,
            created_at: model.created_at.clone(),
            updated_at: model.updated_at.clone(),
        }
    }

    async fn setup_dual_rls(
        &self,
        auth_user: &AuthenticatedUser,
    ) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, Status> {
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;
        set_current_user(&mut conn, &auth_user.user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set user context: {}", e)))?;
        Ok(conn)
    }
}

#[tonic::async_trait]
impl ItemsService for ItemsServiceImpl {
    async fn create_item(
        &self,
        request: Request<CreateItemReq>,
    ) -> Result<Response<CreateItemRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        if req.name.is_empty() {
            return Err(Status::invalid_argument("name is required"));
        }

        let owner_type = if req.owner_type.is_empty() {
            "org"
        } else {
            &req.owner_type
        };
        if owner_type != "org" && owner_type != "personal" {
            return Err(Status::invalid_argument(
                "owner_type must be 'org' or 'personal'",
            ));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let parent_id: Option<&str> = if req.parent_id.is_empty() {
            None
        } else {
            Some(&req.parent_id)
        };
        let barcode: Option<&str> = if req.barcode.is_empty() {
            None
        } else {
            Some(&req.barcode)
        };
        let category: Option<&str> = if req.category.is_empty() {
            None
        } else {
            Some(&req.category)
        };
        let description: Option<&str> = if req.description.is_empty() {
            None
        } else {
            Some(&req.description)
        };
        let image_url: Option<&str> = if req.image_url.is_empty() {
            None
        } else {
            Some(&req.image_url)
        };
        let url: Option<&str> = if req.url.is_empty() {
            None
        } else {
            Some(&req.url)
        };
        let item_type = if req.item_type.is_empty() {
            "item"
        } else {
            &req.item_type
        };
        if item_type != "folder" && item_type != "item" {
            return Err(Status::invalid_argument(
                "item_type must be 'folder' or 'item'",
            ));
        }
        let quantity = if req.quantity == 0 { 1 } else { req.quantity };

        // Set org_id or user_id based on owner_type
        let (org_id_val, user_id_val): (Option<&str>, Option<&str>) = if owner_type == "org" {
            (Some(&organization_id), None)
        } else {
            (None, Some(&auth_user.user_id))
        };

        let model: ItemModel = sqlx::query_as(
            "INSERT INTO items (parent_id, owner_type, organization_id, user_id, name, barcode, category, description, image_url, url, item_type, quantity) \
             VALUES ($1::uuid, $2, $3::uuid, $4::uuid, $5, $6, $7, $8, $9, $10, $11, $12) \
             RETURNING id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text",
        )
        .bind(parent_id)
        .bind(owner_type)
        .bind(org_id_val)
        .bind(user_id_val)
        .bind(&req.name)
        .bind(barcode)
        .bind(category)
        .bind(description)
        .bind(image_url)
        .bind(url)
        .bind(item_type)
        .bind(quantity)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(CreateItemRes {
            item: Some(Self::model_to_proto(&model)),
        }))
    }

    async fn get_item(
        &self,
        request: Request<GetItemReq>,
    ) -> Result<Response<GetItemRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let model: Option<ItemModel> = sqlx::query_as(
            "SELECT id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text \
             FROM items WHERE id = $1::uuid",
        )
        .bind(&req.id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match model {
            Some(m) => Ok(Response::new(GetItemRes {
                item: Some(Self::model_to_proto(&m)),
            })),
            None => Err(Status::not_found("Item not found")),
        }
    }

    async fn update_item(
        &self,
        request: Request<UpdateItemReq>,
    ) -> Result<Response<UpdateItemRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let barcode: Option<&str> = if req.barcode.is_empty() {
            None
        } else {
            Some(&req.barcode)
        };
        let category: Option<&str> = if req.category.is_empty() {
            None
        } else {
            Some(&req.category)
        };
        let description: Option<&str> = if req.description.is_empty() {
            None
        } else {
            Some(&req.description)
        };
        let image_url: Option<&str> = if req.image_url.is_empty() {
            None
        } else {
            Some(&req.image_url)
        };
        let url: Option<&str> = if req.url.is_empty() {
            None
        } else {
            Some(&req.url)
        };

        let model: Option<ItemModel> = sqlx::query_as(
            "UPDATE items SET name = $1, barcode = $2, category = $3, description = $4, \
             image_url = $5, url = $6, quantity = $7, updated_at = NOW() \
             WHERE id = $8::uuid \
             RETURNING id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text",
        )
        .bind(&req.name)
        .bind(barcode)
        .bind(category)
        .bind(description)
        .bind(image_url)
        .bind(url)
        .bind(req.quantity)
        .bind(&req.id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match model {
            Some(m) => Ok(Response::new(UpdateItemRes {
                item: Some(Self::model_to_proto(&m)),
            })),
            None => Err(Status::not_found("Item not found")),
        }
    }

    async fn delete_item(
        &self,
        request: Request<DeleteItemReq>,
    ) -> Result<Response<Empty>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let rows_affected = sqlx::query("DELETE FROM items WHERE id = $1::uuid")
            .bind(&req.id)
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .rows_affected();

        if rows_affected == 0 {
            return Err(Status::not_found("Item not found"));
        }

        Ok(Response::new(Empty {}))
    }

    async fn list_items(
        &self,
        request: Request<ListItemsReq>,
    ) -> Result<Response<ListItemsRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        // parent_id filter
        let parent_filter = if req.parent_id.is_empty() {
            conditions.push("parent_id IS NULL".to_string());
            None
        } else {
            conditions.push(format!("parent_id = ${}::uuid", param_idx));
            param_idx += 1;
            Some(req.parent_id.clone())
        };

        // owner_type filter
        let owner_type_filter = if !req.owner_type.is_empty() {
            conditions.push(format!("owner_type = ${}", param_idx));
            param_idx += 1;
            Some(req.owner_type.clone())
        } else {
            None
        };

        // category filter
        let category_filter = if !req.category.is_empty() {
            conditions.push(format!("category = ${}", param_idx));
            // param_idx += 1; // last param, no need to increment
            Some(req.category.clone())
        } else {
            None
        };

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text \
             FROM items {} ORDER BY name ASC",
            where_clause
        );

        let mut query = sqlx::query_as::<_, ItemModel>(&sql);
        if let Some(ref v) = parent_filter {
            query = query.bind(v);
        }
        if let Some(ref v) = owner_type_filter {
            query = query.bind(v);
        }
        if let Some(ref v) = category_filter {
            query = query.bind(v);
        }

        let models: Vec<ItemModel> = query
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let items: Vec<Item> = models.iter().map(Self::model_to_proto).collect();
        Ok(Response::new(ListItemsRes { items }))
    }

    async fn move_item(
        &self,
        request: Request<MoveItemReq>,
    ) -> Result<Response<Empty>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let new_parent_id: Option<&str> = if req.new_parent_id.is_empty() {
            None
        } else {
            Some(&req.new_parent_id)
        };

        let rows_affected = sqlx::query(
            "UPDATE items SET parent_id = $1::uuid, updated_at = NOW() WHERE id = $2::uuid",
        )
        .bind(new_parent_id)
        .bind(&req.id)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .rows_affected();

        if rows_affected == 0 {
            return Err(Status::not_found("Item not found"));
        }

        Ok(Response::new(Empty {}))
    }

    async fn change_item_ownership(
        &self,
        request: Request<ChangeItemOwnershipReq>,
    ) -> Result<Response<Empty>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        tracing::info!(
            "ChangeItemOwnership called: id={}, new_owner_type={}, user_id={}, org_id={}",
            req.id, req.new_owner_type, auth_user.user_id, auth_user.org_id
        );

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }

        let (new_org_id, new_user_id): (Option<&str>, Option<&str>) =
            match req.new_owner_type.as_str() {
                "org" => (Some(&auth_user.org_id), None),
                "personal" => (None, Some(&auth_user.user_id)),
                _ => {
                    return Err(Status::invalid_argument(
                        "new_owner_type must be 'org' or 'personal'",
                    ))
                }
            };

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let rows_affected = sqlx::query(
            r#"WITH RECURSIVE descendants AS (
                SELECT id FROM items WHERE id = $1::uuid
                UNION ALL
                SELECT i.id FROM items i JOIN descendants d ON i.parent_id = d.id
            )
            UPDATE items SET
                owner_type = $2,
                organization_id = $3::uuid,
                user_id = $4::uuid,
                updated_at = NOW()
            WHERE id IN (SELECT id FROM descendants)"#,
        )
        .bind(&req.id)
        .bind(&req.new_owner_type)
        .bind(new_org_id)
        .bind(new_user_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .rows_affected();

        tracing::info!(
            "ChangeItemOwnership result: id={}, rows_affected={}",
            req.id, rows_affected
        );

        if rows_affected == 0 {
            return Err(Status::not_found("Item not found"));
        }

        Ok(Response::new(Empty {}))
    }

    async fn search_by_barcode(
        &self,
        request: Request<SearchByBarcodeReq>,
    ) -> Result<Response<ListItemsRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.barcode.is_empty() {
            return Err(Status::invalid_argument("barcode is required"));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        let models: Vec<ItemModel> = sqlx::query_as(
            "SELECT id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text \
             FROM items WHERE barcode = $1 ORDER BY name ASC",
        )
        .bind(&req.barcode)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let items: Vec<Item> = models.iter().map(Self::model_to_proto).collect();
        Ok(Response::new(ListItemsRes { items }))
    }

    async fn convert_item_type(
        &self,
        request: Request<ConvertItemTypeReq>,
    ) -> Result<Response<ConvertItemTypeRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.id.is_empty() {
            return Err(Status::invalid_argument("id is required"));
        }
        if req.new_item_type != "folder" && req.new_item_type != "item" {
            return Err(Status::invalid_argument(
                "new_item_type must be 'folder' or 'item'",
            ));
        }

        let mut conn = self.setup_dual_rls(&auth_user).await?;

        // 現在のアイテムを取得（parent_id確認用）
        let current: Option<ItemModel> = sqlx::query_as(
            "SELECT id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text \
             FROM items WHERE id = $1::uuid",
        )
        .bind(&req.id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let current = current.ok_or_else(|| Status::not_found("Item not found"))?;

        // 同種への変換はno-op
        if current.item_type == req.new_item_type {
            return Ok(Response::new(ConvertItemTypeRes {
                item: Some(Self::model_to_proto(&current)),
                children_moved: 0,
            }));
        }

        let mut children_moved: i32 = 0;

        // Folder → Item: 子アイテムを親フォルダに昇格
        if current.item_type == "folder" && req.new_item_type == "item" {
            let parent_id_for_children: Option<&str> = current.parent_id.as_deref();

            let result = sqlx::query(
                "UPDATE items SET parent_id = $1::uuid, updated_at = NOW() \
                 WHERE parent_id = $2::uuid",
            )
            .bind(parent_id_for_children)
            .bind(&req.id)
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

            children_moved = result.rows_affected() as i32;
        }

        // item_type を更新
        let model: Option<ItemModel> = sqlx::query_as(
            "UPDATE items SET item_type = $1, updated_at = NOW() \
             WHERE id = $2::uuid \
             RETURNING id::text, parent_id::text, owner_type, organization_id::text, user_id::text, \
             name, barcode, category, description, image_url, url, item_type, quantity, \
             created_at::text, updated_at::text",
        )
        .bind(&req.new_item_type)
        .bind(&req.id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match model {
            Some(m) => Ok(Response::new(ConvertItemTypeRes {
                item: Some(Self::model_to_proto(&m)),
                children_moved,
            })),
            None => Err(Status::internal("Update failed unexpectedly")),
        }
    }
}
