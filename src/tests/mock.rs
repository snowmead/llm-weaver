use async_trait::async_trait;
use redis::ToRedisArgs;
use serde::de::DeserializeOwned;
use tiktoken_rs::p50k_base;

use crate::{
	types::{LoomError, StorageError, WeaveError},
	Config, ContextMessage, Llm, Loom, TapestryChestHandler, TapestryFragment, TapestryId,
};

#[derive(Default, Clone, Debug)]
pub struct BasicId(pub String);

impl TapestryId for BasicId {
	fn base_key(&self) -> String {
		self.0.clone()
	}
}

#[derive(Clone)]
pub struct PromptRequest {
	_role: String,
	_content: String,
	_account_id: String,
}

impl<T: Config> From<ContextMessage<T>> for PromptRequest {
	fn from(msg: ContextMessage<T>) -> Self {
		Self {
			_role: msg.role.to_string(),
			_content: msg.content,
			_account_id: msg.account_id.unwrap_or("".to_string()),
		}
	}
}

#[derive(Clone, Debug)]
pub struct PromptParameters<T: Config> {
	/// Maximum number of tokens to generate.
	pub max_tokens: <T::PromptModel as Llm<T>>::Tokens,
}

#[derive(Default, PartialEq, Eq, Clone, Debug, Copy)]
pub enum Models {
	#[default]
	GPT4,
	_GPT4_32K,
}

#[async_trait]
impl<T: Config> Llm<T> for Models {
	type Tokens = u16;
	type Request = PromptRequest;
	type Parameters = PromptParameters<T>;
	type Response = String;

	fn name(&self) -> &'static str {
		match self {
			Self::GPT4 => "gpt4",
			Self::_GPT4_32K => "gpt4-32k",
		}
	}

	fn max_context_length(&self) -> Self::Tokens {
		match self {
			Self::GPT4 => 8_192,
			Self::_GPT4_32K => 32_768,
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
		_msgs: Vec<Self::Request>,
		_params: &Self::Parameters,
	) -> crate::Result<Self::Response> {
		Ok("response: pong".to_string())
	}
}

#[derive(Default, Clone, Debug)]
pub struct TestConfig;

impl Config for TestConfig {
	type PromptModel = Models;
	type SummaryModel = Models;
	type TapestryChest = TapestryChestMock;

	fn convert_prompt_tokens_to_summary_model_tokens(
		tokens: crate::types::PromptModelTokens<Self>,
	) -> crate::types::SummaryModelTokens<Self> {
		tokens
	}
}

impl Loom<TestConfig> for TestConfig {}

pub struct TapestryChestMock;

#[async_trait]
impl<T: Config> TapestryChestHandler<T> for TapestryChestMock {
	type Error = StorageError;

	async fn save_tapestry_fragment<TID: TapestryId>(
		_tapestry_id: TID,
		_tapestry_fragment: TapestryFragment<T>,
		_increment: bool,
	) -> crate::Result<()> {
		Ok(())
	}

	async fn save_tapestry_metadata<TID: TapestryId, M: ToRedisArgs + Send + Sync>(
		_tapestry_id: TID,
		_metadata: M,
	) -> crate::Result<()> {
		Ok(())
	}

	async fn get_tapestry_fragment<TID: TapestryId>(
		_tapestry_id: TID,
		_instance: Option<u64>,
	) -> crate::Result<Option<TapestryFragment<T>>> {
		Ok(Some(TapestryFragment::default()))
	}

	async fn get_tapestry_metadata<TID: TapestryId, M: DeserializeOwned + Default>(
		_tapestry_id: TID,
		_instance: Option<u64>,
	) -> crate::Result<Option<M>> {
		Ok(Some(M::default()))
	}

	async fn delete_tapestry_fragment<TID: TapestryId>(_tapestry_id: TID) -> crate::Result<()> {
		Ok(())
	}
}
