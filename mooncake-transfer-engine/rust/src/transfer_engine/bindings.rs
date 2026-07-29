//! Minimal declarations for the stable Transfer Engine C API used by Rust.

use std::ffi::{c_char, c_int, c_void};

pub type TransferEngineHandle = *mut c_void;

#[repr(C)]
pub struct transfer_request_t {
    pub opcode: c_int,
    pub source: *mut c_void,
    pub target_id: i32,
    pub target_offset: u64,
    pub length: u64,
}

#[repr(C)]
pub struct buffer_entry_t {
    pub addr: *mut c_void,
    pub length: usize,
}

#[repr(C)]
pub struct transfer_status_t {
    pub status: c_int,
    pub transferred_bytes: u64,
}

unsafe extern "C" {
    pub fn createTransferEngine(
        metadata_conn_string: *const c_char,
        local_server_name: *const c_char,
        ip_or_host_name: *const c_char,
        rpc_port: u64,
        auto_discover: c_int,
    ) -> TransferEngineHandle;
    pub fn destroyTransferEngine(engine: TransferEngineHandle);
    pub fn discoverTopology(engine: TransferEngineHandle) -> c_int;
    pub fn getLocalIpAndPort(
        engine: TransferEngineHandle,
        buf_out: *mut c_char,
        buf_len: usize,
    ) -> c_int;
    pub fn installTransport(
        engine: TransferEngineHandle,
        proto: *const c_char,
        args: *mut *mut c_void,
    ) -> *mut c_void;
    pub fn openSegment(engine: TransferEngineHandle, segment_name: *const c_char) -> i32;
    pub fn closeSegment(engine: TransferEngineHandle, segment_id: i32) -> c_int;
    pub fn warmupEfaSegment(engine: TransferEngineHandle, segment_name: *const c_char) -> c_int;
    pub fn registerLocalMemory(
        engine: TransferEngineHandle,
        addr: *mut c_void,
        length: usize,
        location: *const c_char,
        remote_accessible: c_int,
    ) -> c_int;
    pub fn unregisterLocalMemory(engine: TransferEngineHandle, addr: *mut c_void) -> c_int;
    pub fn registerLocalMemoryBatch(
        engine: TransferEngineHandle,
        buffer_list: *mut buffer_entry_t,
        buffer_len: usize,
        location: *const c_char,
    ) -> c_int;
    pub fn unregisterLocalMemoryBatch(
        engine: TransferEngineHandle,
        addr_list: *mut *mut c_void,
        addr_len: usize,
    ) -> c_int;
    pub fn allocateBatchID(engine: TransferEngineHandle, batch_size: usize) -> u64;
    pub fn submitTransfer(
        engine: TransferEngineHandle,
        batch_id: u64,
        entries: *mut transfer_request_t,
        count: usize,
    ) -> c_int;
    pub fn getTransferStatus(
        engine: TransferEngineHandle,
        batch_id: u64,
        task_id: usize,
        status: *mut transfer_status_t,
    ) -> c_int;
    pub fn freeBatchID(engine: TransferEngineHandle, batch_id: u64) -> c_int;
    pub fn syncSegmentCache(engine: TransferEngineHandle) -> c_int;
}
