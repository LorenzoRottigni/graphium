use axum::Json;
use graphium_macro::node;

node! {
    pub async fn migrate_products_table(ctx: &crate::context::Context) -> Result<(), String> {
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
        .map_err(|e| format!("products table migration failed: {e}"))?;
        Ok(())
    }
}

node! {
    pub async fn product_create(
        ctx: &crate::context::Context,
        new_product: &crate::models::NewProduct,
    ) -> crate::models::Product {
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
        .map_err(|e| format!("create product failed: {e}"));
        created.unwrap()
    }
}

node! {
    pub async fn product_get_by_id(
        ctx: &crate::context::Context,
        product_id: i64,
    ) -> Result<Option<crate::models::Product>, String> {
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
        .map_err(|e| format!("get product failed: {e}"))?;
        Ok(product)
    }
}

node! {
    pub async fn product_list(
        ctx: &crate::context::Context,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::models::Product>, String> {
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
        .map_err(|e| format!("list products failed: {e}"))?;
        Ok(items)
    }
}

node! {
    pub async fn product_update_price(
        ctx: &crate::context::Context,
        product_id: i64,
        price_cents: i64,
    ) -> Result<Option<crate::models::Product>, String> {
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
        .map_err(|e| format!("update product price failed: {e}"))?;
        Ok(updated)
    }
}

node! {
    pub async fn product_delete(
        ctx: &crate::context::Context,
        product_id: i64,
    ) -> Result<u64, String> {
        let result = sqlx::query(
            r#"
            DELETE FROM products
            WHERE id = $1
            "#,
        )
        .bind(product_id)
        .execute(&ctx.pool)
        .await
        .map_err(|e| format!("delete product failed: {e}"))?;
        Ok(result.rows_affected())
    }
}

node! {
    pub async fn check_product_does_not_exist(
        ctx: &crate::context::Context,
        product_input: &crate::models::NewProduct,
    ) {
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
        .map_err(|e| format!("check product existence failed: {e}"));
        match existing {
            Ok(Some(_)) => panic!("product with the same name already exists"),
            Ok(None) => {}
            Err(e) => panic!("{e}"),
        }
    }
}

node! {
    pub async fn get_product_input(
        name: String,
        price: String,
    ) -> crate::models::NewProduct {
        crate::models::NewProduct {
            name,
            price: price.parse::<i64>().expect("invalid price format"),
        }
    }
}

node! {
    pub async fn validate_product_input_data(
        product_input: &crate::models::NewProduct,
    ) {
        if product_input.name.trim().is_empty() {
            panic!("product name cannot be empty");
        }
        if product_input.price <= 0 {
            panic!("price must be greater than zero");
        }
    }
}

node! {
    pub async fn serialize_product(
        product: crate::models::Product,
    ) -> Json<crate::models::Product> {
        Json(product)
    }
}

node! {
    pub async fn product_update(
        ctx: &crate::context::Context,
        product_id: i64,
        update: crate::models::UpdateProduct,
    ) -> Result<Option<crate::models::Product>, String> {
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
        .map_err(|e| format!("update product failed: {e}"))?;
        Ok(updated)
    }
}

node! {
    pub async fn unwrap_result_option_product(
        product: Result<Option<crate::models::Product>, String>,
    ) -> crate::models::Product {
        match product {
            Ok(Some(product)) => product,
            Ok(None) => panic!("product not found"),
            Err(e) => panic!("{e}"),
        }
    }
}

node! {
    pub async fn unwrap_result_products(
        products: Result<Vec<crate::models::Product>, String>,
    ) -> Vec<crate::models::Product> {
        match products {
            Ok(products) => products,
            Err(e) => panic!("{e}"),
        }
    }
}

node! {
    pub async fn unwrap_result_rows_affected(
        rows: Result<u64, String>,
    ) -> u64 {
        match rows {
            Ok(rows) => rows,
            Err(e) => panic!("{e}"),
        }
    }
}

node! {
    pub async fn rows_affected_to_delete_result(
        rows: u64,
    ) -> crate::models::DeleteResult {
        crate::models::DeleteResult { deleted: rows > 0 }
    }
}

node! {
    pub async fn serialize_products(
        products: Vec<crate::models::Product>,
    ) -> Json<Vec<crate::models::Product>> {
        Json(products)
    }
}

node! {
    pub async fn serialize_delete_result(
        result: crate::models::DeleteResult,
    ) -> Json<crate::models::DeleteResult> {
        Json(result)
    }
}
