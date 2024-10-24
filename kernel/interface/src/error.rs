use std::ffi::NulError;

use thiserror::Error;
use valthrun_driver_shared::IO_MAX_DEREF_COUNT;

#[derive(Error, Debug)]
pub enum KInterfaceError {
    #[error("初始化返回了无效的状态代码 ({0:X})")]
    InitializeInvalidStatus(u32),

    #[error("内核驱动版本太低 (已加载版本: {driver_version_string}, 需要版本: {requested_version_string})")]
    DriverTooOld {
        driver_version: u32,
        driver_version_string: String,

        requested_version: u32,
        requested_version_string: String,
    },

    #[error("内核驱动 (已加载版本: {driver_version_string}) 比预期的 {requested_version_string} 更新，并且不支持请求的版本")]
    DriverTooNew {
        driver_version: u32,
        driver_version_string: String,

        requested_version: u32,
        requested_version_string: String,
    },

    #[error("内核接口路径包含无效字符")]
    DeviceInvalidPath(NulError),

    #[error("内核接口不可用: {0}")]
    DeviceUnavailable(windows::core::Error),

    #[error("未能加载 um 驱动程序: {0}")]
    DriverLoadingError(#[from] libloading::Error),

    #[error("请求失败 (DeviceIoControl)")]
    RequestFailed,

    #[error("提供了 {provided} 个偏移量，但只支持 {limit} 个")]
    TooManyOffsets { provided: usize, limit: usize },

    #[error("在 0x{target_address:X} 处读取失败 ({resolved_offset_count}/{offset_count})")]
    InvalidAddress {
        target_address: u64,

        resolved_offsets: [u64; IO_MAX_DEREF_COUNT],
        resolved_offset_count: usize,

        offsets: [u64; IO_MAX_DEREF_COUNT],
        offset_count: usize,
    },

    #[error("目标进程已经不存在")]
    ProcessDoesNotExists,

    #[error("could not identify process as the name is not ubiquitous")]
    ProcessNotUbiquitous,

    #[error("the requested memory access mode is unavailable")]
    AccessModeUnavailable,

    #[error("unknown data store error")]
    Unknown,
}

pub type KResult<T> = std::result::Result<T, KInterfaceError>;
