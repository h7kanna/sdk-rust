use restate_sdk::prelude::*;

#[restate_sdk::object]
trait Counter {
    #[shared]
    async fn get() -> Result<u64, TerminalError>;
    async fn add(val: u64) -> Result<u64, TerminalError>;
    async fn increment() -> Result<u64, TerminalError>;
    async fn reset() -> Result<(), TerminalError>;
}

struct CounterImpl;

const COUNT: &str = "count";

impl Counter for CounterImpl {
    async fn get(&self, ctx: SharedObjectContext<'_>) -> Result<u64, TerminalError> {
        Ok(ctx.get::<u64>(COUNT).await?.unwrap_or(0))
    }

    async fn add(&self, ctx: ObjectContext<'_>, val: u64) -> Result<u64, TerminalError> {
        let current = ctx.get::<u64>(COUNT).await?.unwrap_or(0);
        let new = current + val;
        ctx.set(COUNT, new);
        Ok(new)
    }

    async fn increment(&self, ctx: ObjectContext<'_>) -> Result<u64, TerminalError> {
        self.add(ctx, 1).await
    }

    async fn reset(&self, ctx: ObjectContext<'_>) -> Result<(), TerminalError> {
        ctx.clear(COUNT);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    HttpServer::new(Endpoint::builder().bind(CounterImpl.serve()).build())
        .listen_and_serve("0.0.0.0:9080".parse().unwrap())
        .await;
}
