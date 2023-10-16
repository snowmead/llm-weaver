#![feature(async_closure)]
#![feature(associated_type_defaults)]
#![feature(more_qualified_paths)]

use std::{
	fmt::{Debug, Display},
	marker::PhantomData,
};

use async_openai::{
	config::OpenAIConfig,
	error::OpenAIError,
	types::{
		ChatCompletionRequestMessage, ChatCompletionRequestMessageArgs,
		CreateChatCompletionRequestArgs, CreateChatCompletionResponse, Role,
	},
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use models::Tokens;
use serde::{Deserialize, Serialize};
use storage::{StorageError, TapestryChest};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument};

use crate::models::Token;

use self::models::Models;

mod storage;

pub use storage::TapestryChestHandler;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait Get<T> {
	fn get() -> T;
}

/// Represents a unique identifier for any arbitrary entity.
///
/// The `TapestryId` trait abstractly represents identifiers, providing a method for
/// generating a standardized key, which can be utilized across various implementations
/// in the library, such as the [`TapestryChestHandler`].
///
/// ```ignore
/// use loreweaver::{TapestryId, Get};
/// use std::fmt::{Debug, Display};
///
/// struct MyTapestryId {
///     id: String,
///     sub_id: String,
///     // ...
/// }
///
/// impl TapestryId for MyTapestryId {
///     fn base_key(&self) -> String {
///         format!("{}:{}", self.id, self.sub_id)
///     }
/// }
pub trait TapestryId: Debug + Display + Clone + Send + Sync + 'static {
	/// Returns the base key.
	///
	/// This method should produce a unique string identifier, that will serve as a key for
	/// associated objects or data within [`TapestryChestHandler`].
	fn base_key(&self) -> String;
}

/// A trait consisting of the main configuration parameters for [`Loreweaver`].
pub trait Config {
	/// Maximum percentage of tokens allowed to generate a summary.
	///
	/// This is not a fixed amount of tokens since the maximum amount of context tokens can change
	/// depending on the model or custom max tokens.
	const SUMMARY_PERCENTAGE: f32 = 0.1;
	/// The sampling temperature, between 0 and 1. Higher values like 0.8 will make the output more
	/// random, while lower values like 0.2 will make it more focused and deterministic. If set to
	/// 0, the model will use log probability to automatically increase the temperature until
	/// certain thresholds are hit.
	///
	/// Defaults to `0.0`
	const TEMPRATURE: f32 = 0.0;
	/// Number between -2.0 and 2.0. Positive values penalize new tokens based on whether they
	/// appear in the text so far, increasing the model's likelihood to talk about new topics.
	///
	/// Defaults to `0.0`
	const PRESENCE_PENALTY: f32 = 0.0;
	/// Number between -2.0 and 2.0. Positive values penalize new tokens based on their existing
	/// frequency in the text so far, decreasing the model's likelihood to repeat the same line
	/// verbatim.
	///
	/// Defaults to `0.0`
	const FREQUENCY_PENALTY: f32 = 0.0;

	/// Getter for GPT model to use.
	///
	/// Defaults to [`models::DefaultModel`]
	type Model: Get<Models> = models::DefaultModel;
	/// Storage handler implementation for storing and retrieving tapestry fragments.
	///
	/// This can simply be a struct that implements [`TapestryChestHandler`] utilizing the default
	/// implementation which uses Redis as the storage backend.
	///
	/// If you wish to implement your own storage backend, you can implement the methods from the
	/// trait. [`Loreweaver`] does not care how you store the data and retrieve your data.
	///
	/// Defaults to [`TapestryChest`]. Using this default requires you to supply the `hostname`,
	/// `port` and `credentials` to connect to your instance.
	type TapestryChest: TapestryChestHandler = TapestryChest;
}

/// Context message that represent a single message in a [`StoryPart`].
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ContextMessage {
	pub role: String,
	pub account_id: String,
	pub content: String,
	pub timestamp: String,
}

/// Represents a single part of a story containing a list of messages along with other metadata.
///
/// ChatGPT can only hold a limited amount of tokens in a the entire message history/context.
/// Therefore, at every [`Loom::prompt`] execution, we must keep track of the number of
/// `context_tokens` in the current story part and if it exceeds the maximum number of tokens
/// allowed for the current GPT [`Models`], then we must generate a summary of the current story
/// part and use that as the starting point for the next story part. This is one of the biggest
/// challenges for Loreweaver to keep a consistent narrative throughout the many story parts.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TapestryFragment {
	/// Total number of _GPT tokens_ in the story part.
	pub context_tokens: Tokens,
	/// List of [`ContextMessage`]s in the story part.
	pub context_messages: Vec<ContextMessage>,
}

/// A trait that defines all of the public associated methods that [`Loreweaver`] implements.
///
/// This is the machine that drives all of the core methods that should be used across any service
/// that needs to prompt ChatGPT and receive a response.
///
/// The implementations should handle all form of validation and usage tracking all while
/// abstracting the logic from the services calling them.
#[async_trait]
pub trait Loom<T: Config> {
	/// Represents an object to use for constructing [`Loom::RequestMessages`] from.
	type Message;
	/// Represents the request message type used to prompt a certain LLM.
	///
	/// This varies between LLMs and their libraries.
	type RequestMessages: IntoIterator;
	/// Represents the response type returned by the LLM library.
	type Response;

	/// Prompt Loreweaver for a response for [`WeavingID`].
	///
	/// Prompts ChatGPT with the current [`StoryPart`] and the `msg`.
	///
	/// If 80% of the maximum number of tokens allowed in a message history for the configured
	/// ChatGPT [`Models`] has been reached, a summary will be generated instead of the current
	/// message history and saved to the cloud. A new message history will begin.
	///
	/// # Parameters
	///
	/// - `tapestry_id`: The [`TapestryId`] to prompt and save context messages to.
	/// - `system`: The system message to prompt ChatGPT with.
	/// - `override_max_context_tokens`: Override the maximum number of context tokens allowed for
	///  the current [`Models`].
	/// - `msg`: The user message to prompt ChatGPT with.
	/// - `account_id`: An optional arbitrary representation of an account id. This will be used as
	///   the `name` parameter when prompting ChatGPT. Leaving it at `None` will leave the `name`
	///   parameter empty.
	async fn weave<TID: TapestryId>(
		tapestry_id: TID,
		system: String,
		override_max_context_tokens: Option<Tokens>,
		msg: String,
		account_id: Option<String>,
	) -> Result<String>;

	/// Build the message/messages to prompt ChatGPT with.
	fn build_messages(msg: Vec<Self::Message>) -> Result<Self::RequestMessages>;

	/// The action to query ChatGPT with the supplied configurations and messages.
	async fn prompt(msgs: &mut Self::RequestMessages, max_tokens: Tokens)
		-> Result<Self::Response>;

	/// Get the content from the response.
	async fn get_content(res: &Self::Response) -> Result<String>;

	/// Maximum number tokens and words allowed for response.
	///
	/// None is returned if the `context_tokens` exceed maximum amount of available tokens.
	fn tokens_available(model: Models, custom_max_tokens: Option<Tokens>) -> Tokens;
}

#[derive(Debug, thiserror::Error)]
enum LoomError {
	Weave(#[from] WeaveError),
	Storage(#[from] StorageError),
}

impl Display for LoomError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Weave(e) => write!(f, "{}", e),
			Self::Storage(e) => write!(f, "{}", e),
		}
	}
}

/// The bread & butter of Loreweaver.
///
/// All core functionality is implemented by this struct.
pub struct Loreweaver<T: Config>(PhantomData<T>);

#[derive(Debug, thiserror::Error)]
enum WeaveError {
	/// Failed to prompt OpenAI.
	FailedPromptOpenAI(OpenAIError),
	/// Failed to get content from OpenAI response.
	FailedToGetContent,
	/// A bad OpenAI role was supplied.
	BadOpenAIRole(String),
}

impl Display for WeaveError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::FailedPromptOpenAI(e) => write!(f, "Failed to prompt OpenAI: {}", e),
			Self::FailedToGetContent => write!(f, "Failed to get content from OpenAI response"),
			Self::BadOpenAIRole(role) => write!(f, "Bad OpenAI role: {}", role),
		}
	}
}

/// Wrapper around [`async_openai::types::types::Role`] for custom implementation.
enum WrapperRole {
	Role(Role),
}

impl From<WrapperRole> for Role {
	fn from(role: WrapperRole) -> Self {
		match role {
			WrapperRole::Role(role) => role,
		}
	}
}

impl From<String> for WrapperRole {
	fn from(role: String) -> Self {
		match role.as_str() {
			"system" => Self::Role(Role::System),
			"assistant" => Self::Role(Role::Assistant),
			"user" => Self::Role(Role::User),
			_ => panic!("Bad OpenAI role"),
		}
	}
}

/// Token to word ratio.
///
/// Every token equates to 75% of a word.
const TOKEN_WORD_RATIO: f32 = 0.75;

lazy_static! {
	/// The OpenAI client to interact with the OpenAI API.
	static ref OPENAI_CLIENT: RwLock<async_openai::Client<OpenAIConfig>> = RwLock::new(async_openai::Client::new());
}

pub struct MessageParams {
	role: Role,
	content: String,
	name: Option<String>,
}

const SYSTEM_ROLE: &str = "system";
const ASSISTANT_ROLE: &str = "assistant";
const USER_ROLE: &str = "user";

type LoomMessage<T> = <Loreweaver<T> as Loom<T>>::Message;
type LoomRequestMessages<T> = <Loreweaver<T> as Loom<T>>::RequestMessages;
type LoomResponse<T> = <Loreweaver<T> as Loom<T>>::Response;

#[async_trait]
impl<T: Config> Loom<T> for Loreweaver<T> {
	type Message = MessageParams;
	type RequestMessages = Vec<ChatCompletionRequestMessage>;
	type Response = CreateChatCompletionResponse;

	#[instrument]
	async fn weave<TID: TapestryId>(
		tapestry_id: TID,
		system: String,
		override_max_context_tokens: Option<Tokens>,
		msg: String,
		account_id: Option<String>,
	) -> Result<String> {
		// ensure that the custom max tokens is not greater than the model's max tokens
		if let Some(custom_max_tokens) = override_max_context_tokens {
			let model = T::Model::get();
			if custom_max_tokens > model.max_context_tokens() {
				return Err(Box::new(WeaveError::BadOpenAIRole(format!(
					"Custom max tokens cannot be greater than model {} max tokens: {}",
					model.name(),
					model.max_context_tokens()
				))))
			}
		}

		// system request message pre built to extend to vecs within this function
		let system_req_msg = <Loreweaver<T> as Loom<T>>::build_messages(vec![LoomMessage::<T> {
			role: Role::System,
			content: system.clone(),
			name: None,
		}])?;

		// get latest tapestry fragment instance from storage
		let story_part = T::TapestryChest::get_tapestry_fragment(tapestry_id.clone(), None)
			.await?
			.unwrap_or_default();

		// number of tokens available according to the configured model or custom max tokens
		let tokens_available = <Loreweaver<T> as Loom<T>>::tokens_available(
			T::Model::get(),
			override_max_context_tokens,
		);

		// base request messages
		// in the case where we generate a summary or simply go straight to prompting for a
		// response, we need to build this iterator of request messages
		let request_messages = system_req_msg.clone().into_iter().chain(
			story_part
				.context_messages
				.clone()
				.into_iter()
				.map(|msg: ContextMessage| {
					ChatCompletionRequestMessageArgs::default()
						.content(msg.content.clone())
						.role(Into::<WrapperRole>::into(msg.role.clone()))
						.name(match msg.role.as_str() {
							"system" => "".to_string(),
							"assistant" | "user" => msg.account_id.clone(),
							_ => WeaveError::BadOpenAIRole(msg.role.clone()).to_string(),
						})
						.build()
						.unwrap()
				})
				.collect::<Vec<ChatCompletionRequestMessage>>(),
		);

		// generate summary and start new tapestry instance if context tokens are exceed maximum +
		// the new message token count exceed the amount of allowed tokens
		let (summarized, mut story_part, mut request_messages) = match tokens_available <=
			story_part.context_tokens + msg.count_tokens()
		{
			true => {
				let tokens_left = tokens_available - story_part.context_tokens;
				let words_summary = tokens_left as f32 * TOKEN_WORD_RATIO;

				let mut gen_summary_prompt = request_messages.clone().into_iter().chain(
					vec![ChatCompletionRequestMessageArgs::default()
						.role(Role::System)
						.content(format!("Generate a summary of the entire adventure so far. Respond with {} words or less", words_summary))
						.build()
						.map_err(|e| {
							error!("Failed to build ChatCompletionRequestMessageArgs: {}", e);
							e
						})?]
				).collect();

				let res = <Loreweaver<T> as Loom<T>>::prompt(&mut gen_summary_prompt, tokens_left)
					.await?;

				let summary_response_content =
					<Loreweaver<T> as Loom<T>>::get_content(&res).await?;

				let summary_req_msg =
					<Loreweaver<T> as Loom<T>>::build_messages(vec![LoomMessage::<T> {
						role: Role::System,
						content: format!("\n\"\"\"\n {}", summary_response_content),
						name: None,
					}])?;

				(
					true,
					TapestryFragment {
						context_tokens: summary_response_content.count_tokens(),
						context_messages: vec![
							ContextMessage {
								role: SYSTEM_ROLE.to_string(),
								account_id: Default::default(),
								content: system,
								timestamp: chrono::Utc::now().to_rfc3339(),
							},
							ContextMessage {
								role: SYSTEM_ROLE.to_string(),
								account_id: Default::default(),
								content: summary_response_content,
								timestamp: chrono::Utc::now().to_rfc3339(),
							},
						],
					},
					system_req_msg
						.into_iter()
						.chain(summary_req_msg)
						.collect::<LoomRequestMessages<T>>(),
				)
			},
			false => (false, story_part, request_messages.collect()),
		};

		let max_tokens = tokens_available - story_part.context_tokens - msg.count_tokens();

		let account_id = account_id.clone().unwrap_or("".to_string());

		// add new user message to request_messages which will be used to prompt with
		// also include the system message to indicate how many words the response should be
		request_messages.extend(vec![
			ChatCompletionRequestMessageArgs::default()
				.content(msg.clone())
				.role(Role::User)
				.name(account_id.clone())
				.build()
				.map_err(|e| {
					error!("Failed to build ChatCompletionRequestMessageArgs: {}", e);
					e
				})?,
			ChatCompletionRequestMessageArgs::default()
				.content(format!(
					"Respond with {} words or less",
					max_tokens as f32 * TOKEN_WORD_RATIO
				))
				.role(Role::System)
				.build()
				.map_err(|e| {
					error!("Failed to build ChatCompletionRequestMessageArgs: {}", e);
					e
				})?,
		]);

		// get response object from prompt
		let res = <Loreweaver<T> as Loom<T>>::prompt(&mut request_messages, max_tokens)
			.await
			.map_err(|e| {
				error!("Failed to prompt ChatGPT: {}", e);
				e
			})?;

		// get response content from prompt
		let response_content =
			<Loreweaver<T> as Loom<T>>::get_content(&res).await.map_err(|e| {
				error!("Failed to get content from ChatGPT response: {}", e);
				e
			})?;

		// add new user message to the story_part to save to storage
		story_part.context_messages.push(ContextMessage {
			role: USER_ROLE.to_string(),
			account_id: account_id.clone(),
			content: msg.clone(),
			timestamp: chrono::Utc::now().to_rfc3339(),
		});

		// push response to the story_part to save to storage
		story_part.context_messages.push(ContextMessage {
			role: ASSISTANT_ROLE.to_string(),
			account_id: account_id.clone(),
			content: response_content.clone(),
			timestamp: chrono::Utc::now().to_rfc3339(),
		});

		debug!("Saving story part: {:?}", story_part.context_messages);

		// save tapestry fragment to storage
		// when summarized, the story_part will be saved to a new instance of the tapestry fragment
		T::TapestryChest::save_tapestry_fragment(tapestry_id, story_part, summarized)
			.await
			.map_err(|e| {
				error!("Failed to save story part: {}", e);
				e
			})?;

		Ok(response_content)
	}

	fn build_messages(msgs: Vec<LoomMessage<T>>) -> Result<LoomRequestMessages<T>> {
		msgs.into_iter()
			.map(|msg: LoomMessage<T>| {
				ChatCompletionRequestMessageArgs::default()
					.role(msg.role)
					.content(msg.content)
					.name(msg.name.unwrap_or_default())
					.build()
					.map_err(|e| {
						error!("Failed to build ChatCompletionRequestMessageArgs: {}", e);
						e.into()
					})
			})
			.collect()
	}

	async fn prompt(
		msgs: &mut LoomRequestMessages<T>,
		max_tokens: Tokens,
	) -> Result<LoomResponse<T>> {
		let request = CreateChatCompletionRequestArgs::default()
			.model(T::Model::get().name())
			.messages(msgs.to_owned())
			.max_tokens(max_tokens)
			.temperature(T::TEMPRATURE)
			.presence_penalty(T::PRESENCE_PENALTY)
			.frequency_penalty(T::FREQUENCY_PENALTY)
			.build()?;

		OPENAI_CLIENT.read().await.chat().create(request).await.map_err(|e| {
			error!("Failed to prompt OpenAI: {}", e);
			WeaveError::FailedPromptOpenAI(e).into()
		})
	}

	async fn get_content(res: &LoomResponse<T>) -> Result<String> {
		res.choices[0]
			.clone()
			.message
			.content
			.ok_or(WeaveError::FailedToGetContent.into())
	}

	fn tokens_available(model: Models, custom_max_tokens: Option<Tokens>) -> Tokens {
		(custom_max_tokens.unwrap_or(model.max_context_tokens()) as f32 * T::SUMMARY_PERCENTAGE)
			as Tokens
	}
}

pub mod models {
	use clap::{builder::PossibleValue, ValueEnum};
	use tiktoken_rs::p50k_base;

	use crate::Get;

	/// Tokens are a ChatGPT concept which represent normally a third of a word (or 75%).
	pub type Tokens = u16;

	/// Tokens are a ChatGPT concept which represent normally a third of a word (or 75%).
	///
	/// This trait auto implements some basic utility methods for counting the number of tokens from
	/// a string.
	pub trait Token: ToString {
		/// Count the number of tokens in the string.
		fn count_tokens(&self) -> Tokens {
			let bpe = p50k_base().unwrap();
			let tokens = bpe.encode_with_special_tokens(&self.to_string());

			tokens.len() as Tokens
		}
	}

	/// Implement the trait for String.
	///
	/// This is done so that we can call `count_tokens` on a String.
	impl Token for String {}

	/// The ChatGPT language models that are available to use.
	#[derive(PartialEq, Eq, Clone, Debug, Copy)]
	pub enum Models {
		GPT3,
		GPT4,
	}

	/// Clap value enum implementation for argument parsing.
	impl ValueEnum for Models {
		fn value_variants<'a>() -> &'a [Self] {
			&[Self::GPT3, Self::GPT4]
		}

		fn to_possible_value(&self) -> Option<PossibleValue> {
			Some(match self {
				Self::GPT3 => PossibleValue::new(Self::GPT3.name()),
				Self::GPT4 => PossibleValue::new(Self::GPT4.name()),
			})
		}
	}

	impl Models {
		/// Get the model name.
		pub fn name(&self) -> &'static str {
			match self {
				Self::GPT3 => "gpt-3.5-turbo",
				Self::GPT4 => "gpt-4",
			}
		}

		/// Maximum number of tokens that can be processed at once by ChatGPT.
		pub fn max_context_tokens(&self) -> Tokens {
			match self {
				Self::GPT3 => 4_096,
				Self::GPT4 => 8_192,
			}
		}
	}

	pub struct DefaultModel;

	impl Get<Models> for DefaultModel {
		fn get() -> Models {
			Models::GPT3
		}
	}
}
