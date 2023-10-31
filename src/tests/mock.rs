use async_trait::async_trait;
use redis::ToRedisArgs;
use serde::de::DeserializeOwned;

use crate::{types::StorageError, Config, TapestryChestHandler, TapestryFragment, TapestryId};

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
}
