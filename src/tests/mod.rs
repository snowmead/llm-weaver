mod mock;

#[cfg(test)]
mod tests {
	use crate::{
		tests::mock,
		types::{WrapperRole, USER_ROLE},
		LlmConfig, Loom,
	};

	#[tokio::test]
	async fn test_basic_weave() {
		let response = mock::TestConfig::weave(
			LlmConfig::<mock::TestConfig, mock::Models> {
				model: mock::Models::GPT4,
				params: mock::PromptParameters { max_tokens: 500 },
			},
			LlmConfig::<mock::TestConfig, mock::Models> {
				model: mock::Models::_GPT4_32K,
				params: mock::PromptParameters { max_tokens: 10_000 },
			},
			mock::BasicId("some_id".to_string()),
			"You are a great helper called bob".to_string(),
			vec![mock::TestConfig::build_context_message(
				WrapperRole::from(USER_ROLE.to_string()),
				"ping".to_string(),
				None,
			)],
		)
		.await
		.unwrap();

		assert_eq!(response, "response: pong".to_string());
	}
}
