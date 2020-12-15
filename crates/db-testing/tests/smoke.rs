use minibot_db_testing::TestDb;

#[tokio::test]
async fn main() -> anyhow::Result<()> {
    let db = TestDb::new_docker()?;

    let handle = db.handle().await?;

    let _conn = handle.get().await?;

    eprintln!("Got connection!");

    Ok(())
}
