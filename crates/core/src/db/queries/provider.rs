use rusqlite::{params, Connection};

use crate::error::{CoreError, Result};
use crate::models::provider::{Model, Provider};

const PROVIDER_COLUMNS: &str = "id, name, base_url, auth_type, docs_url";
const MODEL_COLUMNS: &str =
    "id, provider_id, name, model_type, capabilities_json, default_params_json";

fn map_provider(row: &rusqlite::Row) -> rusqlite::Result<Provider> {
    Ok(Provider {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        auth_type: row.get(3)?,
        docs_url: row.get(4)?,
    })
}

fn map_model(row: &rusqlite::Row) -> rusqlite::Result<Model> {
    Ok(Model {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        name: row.get(2)?,
        model_type: row.get(3)?,
        capabilities_json: row.get(4)?,
        default_params_json: row.get(5)?,
    })
}

pub fn list_providers(conn: &Connection) -> Result<Vec<Provider>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROVIDER_COLUMNS} FROM provider ORDER BY name"
    ))?;
    let rows = stmt.query_map([], map_provider)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn list_models(conn: &Connection, provider_id: Option<String>) -> Result<Vec<Model>> {
    if let Some(pid) = provider_id {
        let mut stmt = conn.prepare(&format!(
            "SELECT {MODEL_COLUMNS} FROM model WHERE provider_id = ?1 ORDER BY name"
        ))?;
        let rows = stmt.query_map(params![pid], map_model)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    } else {
        let mut stmt = conn.prepare(&format!(
            "SELECT {MODEL_COLUMNS} FROM model ORDER BY provider_id, name"
        ))?;
        let rows = stmt.query_map([], map_model)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(CoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn seed_two_providers() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('p1', 'Alpha')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO provider (id, name) VALUES ('p2', 'Bravo')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('m1', 'p1', 'M1', 'image')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('m2', 'p1', 'M2', 'video')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO model (id, provider_id, name, model_type) \
             VALUES ('m3', 'p2', 'M3', 'image')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn list_providers_orders_by_name() {
        let conn = seed_two_providers();
        let list = list_providers(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Alpha");
        assert_eq!(list[1].name, "Bravo");
    }

    #[test]
    fn list_models_filters_by_provider() {
        let conn = seed_two_providers();
        let p1 = list_models(&conn, Some("p1".into())).unwrap();
        assert_eq!(p1.len(), 2);
        assert!(p1.iter().all(|m| m.provider_id == "p1"));

        let all = list_models(&conn, None).unwrap();
        assert_eq!(all.len(), 3);
    }
}
