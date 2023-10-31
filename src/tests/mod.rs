mod mock;

#[cfg(test)]
mod tests {
	use async_trait::async_trait;
	use num_traits::FromPrimitive;
	use tiktoken_rs::p50k_base;

	use crate::{
		types::{LoomError, WeaveError},
		Config, Llm,
	};

	#[derive(Default, PartialEq, Eq, Clone, Debug, Copy)]
	pub enum Models {
		#[default]
		GPT4,
	}

	#[derive(Clone)]
	pub struct PromptRequest(ChatCompletionRequestMessage);

	impl<T: Config> From<ContextMessage<T>> for PromptRequest {
		fn from(msg: ContextMessage<T>) -> Self {
			let mut builder = ChatCompletionRequestMessageArgs::default();

			// get Role from WrapperRole(Role)
			let WrapperRole::Role(role) = msg.role;

			builder.role(role).content(msg.content.clone());

			if let Some(ref account_id) = msg.account_id {
				builder.name(account_id);
			}

			let request = builder.build().map_err(|e| {
				error!("Failed to build ChatCompletionRequestMessageArgs: {}", e);
			});

			match request {
				Ok(request) => Self(request),
				Err(e) => panic!("Failed to build ChatCompletionRequestMessageArgs: {:?}", e),
			}
		}
	}

	impl From<PromptRequest> for ChatCompletionRequestMessage {
		fn from(val: PromptRequest) -> Self {
			val.0
		}
	}

	#[async_trait]
	impl<T: Config> Llm<T> for Models {
		type Tokens = u16;
		type Request = PromptRequest;
		type Parameters = PromptParameters<T>;
		type Response = PromptResponse;

		fn name(&self) -> &'static str {
			match self {
				Self::GPT4 => "gpt-4",
			}
		}

		fn max_context_length(&self) -> Self::Tokens {
			match self {
				Self::GPT4 => Self::Tokens::from_u32(8_192).unwrap(),
			}
		}

		fn count_tokens(content: String) -> crate::Result<Self::Tokens> {
			let bpe = p50k_base().unwrap();
			let tokens = bpe.encode_with_special_tokens(&content.to_string());

			tokens.len().try_into().map_err(|_| {
				LoomError::from(WeaveError::BadConfig(format!(
					"Number of tokens exceeds max tokens for model: {}",
					content
				)))
				.into()
			})
		}

		async fn prompt(
			&self,
			msgs: Vec<Self::Request>,
			params: &Self::Parameters,
		) -> crate::Result<Self::Response> {
		}
	}

	#[derive(Default, Clone, Debug)]
	struct TestConfig;

	impl Config for TestConfig {
		type PromptModel = MockPromptModel;
		type SummaryModel = MockSummaryModel;
		type TapestryChest = MockTapestryChest;

		fn convert_prompt_tokens_to_summary_model_tokens(
			tokens: crate::types::PromptModelTokens<Self>,
		) -> crate::types::SummaryModelTokens<Self> {
			tokens
		}
	}
}
