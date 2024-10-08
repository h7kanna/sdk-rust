use anyhow::anyhow;
use restate_sdk::prelude::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[restate_sdk::object]
#[name = "Failing"]
pub(crate) trait Failing {
    #[name = "terminallyFailingCall"]
    async fn terminally_failing_call(error_message: String) -> HandlerResult<()>;
    #[name = "callTerminallyFailingCall"]
    async fn call_terminally_failing_call(error_message: String) -> HandlerResult<String>;
    #[name = "failingCallWithEventualSuccess"]
    async fn failing_call_with_eventual_success() -> HandlerResult<i32>;
    #[name = "terminallyFailingSideEffect"]
    async fn terminally_failing_side_effect(error_message: String) -> HandlerResult<()>;
    #[name = "sideEffectSucceedsAfterGivenAttempts"]
    async fn side_effect_succeeds_after_given_attempts(minimum_attempts: i32)
        -> HandlerResult<i32>;
    #[name = "sideEffectFailsAfterGivenAttempts"]
    async fn side_effect_fails_after_given_attempts(
        retry_policy_max_retry_count: i32,
    ) -> HandlerResult<i32>;
}

#[derive(Clone, Default)]
pub(crate) struct FailingImpl {
    eventual_success_calls: Arc<AtomicI32>,
    eventual_success_side_effects: Arc<AtomicI32>,
    eventual_failure_side_effects: Arc<AtomicI32>,
}

impl Failing for FailingImpl {
    async fn terminally_failing_call(
        &self,
        _: ObjectContext<'_>,
        error_message: String,
    ) -> HandlerResult<()> {
        Err(TerminalError::new(error_message).into())
    }

    async fn call_terminally_failing_call(
        &self,
        mut context: ObjectContext<'_>,
        error_message: String,
    ) -> HandlerResult<String> {
        let uuid = context.rand_uuid().to_string();
        context
            .object_client::<FailingClient>(uuid)
            .terminally_failing_call(error_message)
            .call()
            .await?;

        unreachable!("This should be unreachable")
    }

    async fn failing_call_with_eventual_success(&self, _: ObjectContext<'_>) -> HandlerResult<i32> {
        let current_attempt = self.eventual_success_calls.fetch_add(1, Ordering::SeqCst) + 1;

        if current_attempt >= 4 {
            self.eventual_success_calls.store(0, Ordering::SeqCst);
            Ok(current_attempt)
        } else {
            Err(anyhow!("Failed at attempt ${current_attempt}").into())
        }
    }

    async fn terminally_failing_side_effect(
        &self,
        context: ObjectContext<'_>,
        error_message: String,
    ) -> HandlerResult<()> {
        context
            .run(|| async move { Err(TerminalError::new(error_message))? })
            .await?;

        unreachable!("This should be unreachable")
    }

    async fn side_effect_succeeds_after_given_attempts(
        &self,
        context: ObjectContext<'_>,
        minimum_attempts: i32,
    ) -> HandlerResult<i32> {
        let cloned_counter = Arc::clone(&self.eventual_success_side_effects);
        let success_attempt = context
            .run(|| async move {
                let current_attempt = cloned_counter.fetch_add(1, Ordering::SeqCst) + 1;

                if current_attempt >= minimum_attempts {
                    cloned_counter.store(0, Ordering::SeqCst);
                    Ok(current_attempt)
                } else {
                    Err(anyhow!("Failed at attempt {current_attempt}"))?
                }
            })
            .retry_policy(
                RunRetryPolicy::new()
                    .initial_delay(Duration::from_millis(10))
                    .exponentiation_factor(1.0),
            )
            .name("failing_side_effect")
            .await?;

        Ok(success_attempt)
    }

    async fn side_effect_fails_after_given_attempts(
        &self,
        context: ObjectContext<'_>,
        retry_policy_max_retry_count: i32,
    ) -> HandlerResult<i32> {
        let cloned_counter = Arc::clone(&self.eventual_failure_side_effects);
        if context
            .run(|| async move {
                let current_attempt = cloned_counter.fetch_add(1, Ordering::SeqCst) + 1;
                Err::<(), _>(anyhow!("Failed at attempt {current_attempt}").into())
            })
            .retry_policy(
                RunRetryPolicy::new()
                    .initial_delay(Duration::from_millis(10))
                    .exponentiation_factor(1.0)
                    .max_attempts(retry_policy_max_retry_count as u32),
            )
            .await
            .is_err()
        {
            Ok(self.eventual_failure_side_effects.load(Ordering::SeqCst))
        } else {
            Err(TerminalError::new("Expecting the side effect to fail!"))?
        }
    }
}
