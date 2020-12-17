use minibot_db_testing::TestDb;

#[tokio::test]
async fn main() -> anyhow::Result<()> {
    let db = TestDb::new_docker()?;

    let handle = db.handle().await?;

    let conn = handle.get().await?;

    drop(conn);

    Ok(())
}
