use sqlx::{
    PgPool,
    migrate::{MigrateError, Migrator},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;

        Ok(Self { pool })
    }

    pub fn connect_lazy(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect_lazy(database_url)?;

        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<(), MigrateError> {
        MIGRATOR.run(&self.pool).await
    }

    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresStore;

    #[tokio::test]
    async fn lazy_postgres_store_does_not_require_live_database() {
        let store = PostgresStore::connect_lazy("postgres://vessel:vessel@127.0.0.1:5432/vessel");

        assert!(store.is_ok());
    }
}
