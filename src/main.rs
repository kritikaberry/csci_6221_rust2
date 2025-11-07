use redis::AsyncCommands;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    // Connect to local Redis (must be running)
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_multiplexed_async_connection().await?;

    println!("✅ Connected to Redis");

    // Set and get a test key
    let _: () = con.set("greeting", "Hello Redis!").await?;
    let msg: String = con.get("greeting").await?;

    println!("📦 Retrieved from Redis: {}", msg);

    Ok(())
}
