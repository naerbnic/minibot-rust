mod migrations {
    use refinery::embed_migrations;
    embed_migrations!("../migrations");
}

pub fn apply_migrations(connection_url: &str) {
    let mut client = postgres::Client::connect(connection_url, postgres::tls::NoTls).unwrap();
    self::migrations::migrations::runner().run(&mut client).unwrap();
}