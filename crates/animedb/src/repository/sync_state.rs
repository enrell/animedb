use super::common::*;
use crate::error::{Error, Result};
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};

pub struct SyncStateRepository<'a> {
    pub conn: &'a Connection,
}

impl<'a> SyncStateRepository<'a> {
    pub fn load_sync_state(&self, source: SourceName, scope: &str) -> Result<PersistedSyncState> {
        self.conn
            .query_row(
                r#"
            SELECT source, scope, cursor_json, last_success_at, last_error, last_page, mode
            FROM sync_state
            WHERE source = ?1 AND scope = ?2
            "#,
                params![source.as_str(), scope],
                |row| {
                    let source = parse_source(row.get_ref(0)?.as_str()?)
                        .map_err(|err| rusqlite_decode_error(0, err))?;
                    let scope = row.get::<_, String>(1)?;
                    let cursor = row
                        .get::<_, Option<String>>(2)?
                        .map(|value| serde_json::from_str::<SyncCursor>(&value))
                        .transpose()
                        .map_err(|err| rusqlite_decode_error(2, err))?;
                    let mode_str: String = row.get(6)?;
                    let mode = match mode_str.as_str() {
                        "full" => SyncMode::Full,
                        "incremental" => SyncMode::Incremental,
                        other => {
                            return Err(rusqlite_decode_error(
                                6,
                                Error::Validation(format!("unsupported sync mode: {other}")),
                            ));
                        }
                    };

                    Ok(PersistedSyncState {
                        source,
                        scope,
                        cursor,
                        last_success_at: row.get(3)?,
                        last_error: row.get(4)?,
                        last_page: row.get(5)?,
                        mode,
                    })
                },
            )
            .optional()?
            .ok_or(Error::NotFound)
    }

    pub fn save_sync_state(&self, state: PersistedSyncState) -> Result<()> {
        let cursor_json = state
            .cursor
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.conn.execute(
            r#"
        INSERT INTO sync_state (
            source,
            scope,
            cursor_json,
            last_success_at,
            last_error,
            last_page,
            mode
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(source, scope) DO UPDATE SET
            cursor_json = excluded.cursor_json,
            last_success_at = excluded.last_success_at,
            last_error = excluded.last_error,
            last_page = excluded.last_page,
            mode = excluded.mode
        "#,
            params![
                state.source.as_str(),
                state.scope,
                cursor_json,
                state.last_success_at,
                state.last_error,
                state.last_page,
                state.mode.as_str(),
            ],
        )?;

        Ok(())
    }
}
