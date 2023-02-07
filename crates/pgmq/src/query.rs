pub const TABLE_PREFIX: &str = r#"pgmq"#;

pub fn init_queue(name: &str) -> Vec<String> {
    vec![
        create_meta(),
        create_queue(name),
        create_index(name),
        create_archive(name),
        insert_meta(name),
    ]
}

pub fn destory_queue(name: &str) -> Vec<String> {
    vec![
        drop_queue(name),
        delete_queue_index(name),
        drop_queue_archive(name),
        delete_queue_metadata(name),
    ]
}

pub fn create_queue(name: &str) -> String {
    format!(
        "
        CREATE TABLE IF NOT EXISTS {TABLE_PREFIX}_{name} (
            msg_id BIGSERIAL,
            read_ct INT DEFAULT 0,
            enqueued_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            vt TIMESTAMP WITH TIME ZONE,
            message JSON
        );
        "
    )
}

pub fn create_archive(name: &str) -> String {
    format!(
        "
        CREATE TABLE IF NOT EXISTS {TABLE_PREFIX}_{name}_archive (
            msg_id BIGSERIAL,
            read_ct INT DEFAULT 0,
            enqueued_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            deleted_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc'),
            vt TIMESTAMP WITH TIME ZONE,
            message JSON
        );
        "
    )
}

pub fn create_meta() -> String {
    format!(
        "
        CREATE TABLE IF NOT EXISTS {TABLE_PREFIX}_meta (
            queue_name VARCHAR UNIQUE,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT (now() at time zone 'utc')
        );
        "
    )
}

pub fn drop_queue(name: &str) -> String {
    format!(
        "
        DROP TABLE IF EXISTS {TABLE_PREFIX}_{name};
        "
    )
}

pub fn delete_queue_index(name: &str) -> String {
    format!(
        "
        DROP INDEX IF EXISTS {TABLE_PREFIX}_{name}.vt_idx_{name};
        "
    )
}

pub fn delete_queue_metadata(name: &str) -> String {
    format!(
        "
        DO $$
        BEGIN
           IF EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_name = '{TABLE_PREFIX}_meta')
            THEN
              DELETE
              FROM {TABLE_PREFIX}_meta
              WHERE queue_name = '{name}';
           END IF;
        END $$;
        "
    )
}

pub fn drop_queue_archive(name: &str) -> String {
    format!(
        "
        DROP TABLE IF EXISTS {TABLE_PREFIX}_{name}_archive;
        "
    )
}

pub fn insert_meta(name: &str) -> String {
    format!(
        "
        INSERT INTO {TABLE_PREFIX}_meta (queue_name)
        VALUES ('{name}')
        ON CONFLICT
        DO NOTHING;
        "
    )
}

pub fn create_index(name: &str) -> String {
    format!(
        "
        CREATE INDEX IF NOT EXISTS vt_idx_{name} ON {TABLE_PREFIX}_{name} (vt ASC);
        "
    )
}

pub fn enqueue(name: &str, message: &serde_json::Value) -> String {
    // TOOO: vt should be now() + delay
    format!(
        "
        INSERT INTO {TABLE_PREFIX}_{name} (vt, message)
        VALUES (now() at time zone 'utc', '{message}'::json)
        RETURNING msg_id;
        "
    )
}
pub fn enqueue_str(name: &str, message: &str) -> String {
    // TOOO: vt should be now() + delay
    format!(
        "
        INSERT INTO {TABLE_PREFIX}_{name} (vt, message)
        VALUES (now() at time zone 'utc', '{message}'::json)
        RETURNING msg_id;
        "
    )
}

pub fn read(name: &str, vt: &i32) -> String {
    format!(
        "
    WITH cte AS
        (
            SELECT *
            FROM {TABLE_PREFIX}_{name}
            WHERE vt <= now() at time zone 'utc'
            ORDER BY msg_id ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
    UPDATE {TABLE_PREFIX}_{name}
    SET 
        vt = (now() at time zone 'utc' + interval '{vt} seconds'),
        read_ct = read_ct + 1
    WHERE msg_id = (select msg_id from cte)
    RETURNING *;
    "
    )
}

pub fn delete(name: &str, msg_id: &i64) -> String {
    format!(
        "
        DELETE FROM {TABLE_PREFIX}_{name}
        WHERE msg_id = {msg_id};
        "
    )
}

pub fn archive(name: &str, msg_id: &i64) -> String {
    format!(
        "
        WITH archived AS (
            DELETE FROM {TABLE_PREFIX}_{name}
            WHERE msg_id = {msg_id}
            RETURNING msg_id, vt, read_ct, enqueued_at, message
        )
        INSERT INTO {TABLE_PREFIX}_{name}_archive (msg_id, vt, read_ct, enqueued_at, message)
        SELECT msg_id, vt, read_ct, enqueued_at, message 
        FROM archived;
        "
    )
}

pub fn pop(name: &str) -> String {
    format!(
        "
        WITH cte AS
            (
                SELECT *
                FROM {TABLE_PREFIX}_{name}
                WHERE vt <= now() at time zone 'utc'
                ORDER BY msg_id ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
        DELETE from {TABLE_PREFIX}_{name}
        WHERE msg_id = (select msg_id from cte)
        RETURNING *;
        "
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let query = create_queue("yolo");
        assert!(query.contains("pgmq_yolo"));
    }

    #[test]
    fn test_enqueue() {
        let msg = serde_json::json!({
            "foo": "bar"
        });
        let query = enqueue("yolo", &msg);
        assert!(query.contains("pgmq_yolo"));
        assert!(query.contains("{\"foo\":\"bar\"}"));
    }

    #[test]
    fn test_read() {
        let qname = "myqueue";
        let vt: i32 = 20;

        let query = read(&qname, &vt);

        assert!(query.contains(&qname));
        assert!(query.contains(&vt.to_string()));
    }

    #[test]
    fn test_delete() {
        let qname = "myqueue";
        let msg_id: i64 = 42;

        let query = delete(&qname, &msg_id);

        assert!(query.contains(&qname));
        assert!(query.contains(&msg_id.to_string()));
    }
}