use axum::Json;
use graphium::node;

use crate::models::ApiError;

fn parse_price_cents(input: &str) -> Result<i64, ApiError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(ApiError::bad_request("missing price"));
    }

    // Accept either:
    // - integer cents: "123"
    // - decimal price: "1.23" (or "1,23")
    let normalized = s.replace(',', ".");
    if !normalized.contains('.') {
        return normalized
            .parse::<i64>()
            .map_err(|_| ApiError::bad_request("invalid price format (expected cents integer or decimal like 1.23)"));
    }

    let mut parts = normalized.splitn(2, '.');
    let whole = parts.next().unwrap_or("0");
    let frac = parts.next().unwrap_or("0");
    let whole: i64 = whole
        .parse()
        .map_err(|_| ApiError::bad_request("invalid price format"))?;

    let mut frac = frac.to_string();
    if frac.len() > 2 {
        return Err(ApiError::bad_request(
            "invalid price format (too many decimal places)",
        ));
    }
    while frac.len() < 2 {
        frac.push('0');
    }
    let frac: i64 = frac
        .parse()
        .map_err(|_| ApiError::bad_request("invalid price format"))?;

    let cents = whole
        .checked_mul(100)
        .and_then(|v| v.checked_add(frac))
        .ok_or_else(|| ApiError::bad_request("price out of range"))?;
    Ok(cents)
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn migrate_products_table(ctx: &crate::context::Context) -> Result<(), ApiError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS products (
                id BIGSERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                price_cents BIGINT NOT NULL
            );
            "#,
        )
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("products table migration failed: {e}")))?;
        Ok(())
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_create(
        ctx: &crate::context::Context,
        new_product: Result<crate::models::NewProduct, ApiError>,
    ) -> Result<crate::models::Product, ApiError> {
        let new_product = new_product?;
        let created = sqlx::query_as::<_, crate::models::Product>(
            r#"
            INSERT INTO products (name, price_cents)
            VALUES ($1, $2)
            RETURNING id, name, price_cents AS price
            "#,
        )
        .bind(&new_product.name)
        .bind(&new_product.price)
        .fetch_one(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("create product failed: {e}")))?;
        Ok(created)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_get_by_id(
        ctx: &crate::context::Context,
        product_id: i64,
    ) -> Result<Option<crate::models::Product>, ApiError> {
        let product = sqlx::query_as::<_, crate::models::Product>(
            r#"
            SELECT id, name, price_cents AS price
            FROM products
            WHERE id = $1
            "#,
        )
        .bind(product_id)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("get product failed: {e}")))?;
        Ok(product)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_list(
        ctx: &crate::context::Context,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::models::Product>, ApiError> {
        let items = sqlx::query_as::<_, crate::models::Product>(
            r#"
            SELECT id, name, price_cents AS price
            FROM products
            ORDER BY id
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("list products failed: {e}")))?;
        Ok(items)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_update_price(
        ctx: &crate::context::Context,
        product_id: i64,
        price_cents: i64,
    ) -> Result<Option<crate::models::Product>, ApiError> {
        let updated = sqlx::query_as::<_, crate::models::Product>(
            r#"
            UPDATE products
            SET price_cents = $2
            WHERE id = $1
            RETURNING id, name, price_cents AS price
            "#,
        )
        .bind(product_id)
        .bind(price_cents)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("update product price failed: {e}")))?;
        Ok(updated)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_delete(
        ctx: &crate::context::Context,
        product_id: i64,
    ) -> Result<u64, ApiError> {
        let result = sqlx::query(
            r#"
            DELETE FROM products
            WHERE id = $1
            "#,
        )
        .bind(product_id)
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("delete product failed: {e}")))?;
        Ok(result.rows_affected())
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn check_product_does_not_exist(
        ctx: &crate::context::Context,
        product_input: Result<crate::models::NewProduct, ApiError>,
    ) -> Result<crate::models::NewProduct, ApiError> {
        let product_input = product_input?;
        let existing = sqlx::query_as::<_, crate::models::Product>(
            r#"
            SELECT id, name, price_cents AS price
            FROM products
            WHERE name = $1
            "#,
        )
        .bind(&product_input.name)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("check product existence failed: {e}")))?;

        if existing.is_some() {
            return Err(ApiError::conflict("product with the same name already exists"));
        }
        Ok(product_input)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn get_product_input(
        name: String,
        price: String,
    ) -> Result<crate::models::NewProduct, ApiError> {
        let price = parse_price_cents(&price)?;
        Ok(crate::models::NewProduct { name, price })
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn validate_product_input_data(
        product_input: Result<crate::models::NewProduct, ApiError>,
    ) -> Result<crate::models::NewProduct, ApiError> {
        let product_input = product_input?;
        if product_input.name.trim().is_empty() {
            return Err(ApiError::bad_request("product name cannot be empty"));
        }
        if product_input.price <= 0 {
            return Err(ApiError::bad_request("price must be greater than zero"));
        }
        Ok(product_input)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn serialize_product(
        product: Result<crate::models::Product, ApiError>,
    ) -> Result<Json<crate::models::Product>, ApiError> {
        Ok(Json(product?))
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn product_update(
        ctx: &crate::context::Context,
        product_id: i64,
        update: crate::models::UpdateProduct,
    ) -> Result<Option<crate::models::Product>, ApiError> {
        let updated = sqlx::query_as::<_, crate::models::Product>(
            r#"
            UPDATE products
            SET
                name = COALESCE($2, name),
                price_cents = COALESCE($3, price_cents)
            WHERE id = $1
            RETURNING id, name, price_cents AS price
            "#,
        )
        .bind(product_id)
        .bind(&update.name)
        .bind(update.price)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::internal(format!("update product failed: {e}")))?;
        Ok(updated)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn unwrap_result_option_product(
        product: Result<Option<crate::models::Product>, ApiError>,
    ) -> Result<crate::models::Product, ApiError> {
        match product? {
            Some(product) => Ok(product),
            None => Err(ApiError::not_found("product not found")),
        }
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn unwrap_result_products(
        products: Result<Vec<crate::models::Product>, ApiError>,
    ) -> Result<Vec<crate::models::Product>, ApiError> {
        Ok(products?)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn unwrap_result_rows_affected(
        rows: Result<u64, ApiError>,
    ) -> Result<u64, ApiError> {
        Ok(rows?)
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn rows_affected_to_delete_result(
        rows: Result<u64, ApiError>,
    ) -> Result<crate::models::DeleteResult, ApiError> {
        let rows = rows?;
        Ok(crate::models::DeleteResult { deleted: rows > 0 })
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn serialize_products(
        products: Result<Vec<crate::models::Product>, ApiError>,
    ) -> Result<Json<Vec<crate::models::Product>>, ApiError> {
        Ok(Json(products?))
    }
}

node! {
    #[metrics("performance", "errors", "count", "success_rate", "fail_rate")]
    pub async fn serialize_delete_result(
        result: Result<crate::models::DeleteResult, ApiError>,
    ) -> Result<Json<crate::models::DeleteResult>, ApiError> {
        Ok(Json(result?))
    }
}
