// Copyright 2024 KVCache.AI
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod bindings;

use anyhow::{anyhow, bail, Result};
use std::ffi::{c_void, CStr, CString};

pub type BatchId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcodeEnum {
    Read = 0,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatusEnum {
    Waiting = 0,
    Pending,
    Invalid,
    Canceled,
    Completed,
    Timeout,
    Failed,
}

pub struct TransferRequest {
    pub opcode: OpcodeEnum,
    pub source: *mut c_void,
    pub target_id: i32,
    pub target_offset: u64,
    pub length: u64,
}

pub struct BufferEntry {
    pub addr: *mut c_void,
    pub length: u64,
}

pub struct TransferEngine {
    engine: bindings::TransferEngineHandle,
}

impl TransferEngine {
    pub fn new(
        metadata_uri: &str,
        local_server_name: &str,
        advertised_host: &str,
        rpc_port: u64,
    ) -> Result<Self> {
        let metadata_uri_c =
            CString::new(metadata_uri).map_err(|_| anyhow!("CString::new failed"))?;
        let local_server_name_c =
            CString::new(local_server_name).map_err(|_| anyhow!("CString::new failed"))?;
        let advertised_host_c =
            CString::new(advertised_host).map_err(|_| anyhow!("CString::new failed"))?;

        let engine = unsafe {
            bindings::createTransferEngine(
                metadata_uri_c.as_ptr(),
                local_server_name_c.as_ptr(),
                advertised_host_c.as_ptr(),
                rpc_port,
                0, // disable auto_discover
            )
        };
        if engine.is_null() {
            bail!("Failed to create TransferEngine")
        }

        Ok(Self { engine })
    }

    pub fn discover_topology(&self) -> Result<()> {
        let ret = unsafe { bindings::discoverTopology(self.engine) };
        if ret != 0 {
            bail!("Failed to discover topology")
        } else {
            Ok(())
        }
    }

    pub fn install_transport(&self, proto: &str) -> Result<()> {
        let proto_c = CString::new(proto).map_err(|_| anyhow!("CString::new failed"))?;
        let ret = unsafe {
            bindings::installTransport(self.engine, proto_c.as_ptr(), std::ptr::null_mut())
        };
        if ret.is_null() {
            bail!("Failed to install transport '{}'", proto)
        } else {
            Ok(())
        }
    }

    pub fn local_session_name(&self) -> Result<String> {
        let mut buffer = [0i8; 256];
        let ret =
            unsafe { bindings::getLocalIpAndPort(self.engine, buffer.as_mut_ptr(), buffer.len()) };
        if ret != 0 {
            bail!("Failed to get local Transfer Engine session")
        }
        let session = unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .map_err(|error| anyhow!("Invalid local Transfer Engine session: {error}"))?;
        Ok(session.to_owned())
    }

    /// Register a local memory region with Transfer Engine.
    ///
    /// # Safety
    ///
    /// `addr..addr + length` must remain allocated and accessible until the
    /// matching call to [`Self::unregister_local_memory`]. No transfer using
    /// the region may still be in flight when it is unregistered.
    pub unsafe fn register_local_memory(
        &self,
        addr: *mut c_void,
        length: usize,
        location: &str,
    ) -> Result<()> {
        let location_c = CString::new(location).map_err(|_| anyhow!("CString::new failed"))?;
        let ret = unsafe {
            bindings::registerLocalMemory(self.engine, addr, length, location_c.as_ptr(), 1)
        };
        if ret < 0 {
            bail!("Failed to register local memory")
        } else {
            Ok(())
        }
    }

    /// Unregister a local memory region.
    ///
    /// # Safety
    ///
    /// `addr` must identify a live region previously registered with this
    /// engine, and no transfer using it may still be in flight.
    pub unsafe fn unregister_local_memory(&self, addr: *mut c_void) -> Result<()> {
        let ret = unsafe { bindings::unregisterLocalMemory(self.engine, addr) };
        if ret < 0 {
            bail!("Failed to unregister local memory")
        } else {
            Ok(())
        }
    }

    /// Register several local memory regions.
    ///
    /// # Safety
    ///
    /// Every region must satisfy the safety contract of
    /// [`Self::register_local_memory`].
    pub unsafe fn register_local_memory_batch(
        &self,
        buffer_list: &[BufferEntry],
        location: &str,
    ) -> Result<()> {
        if buffer_list.is_empty() {
            return Ok(());
        }
        let location_c = CString::new(location).map_err(|_| anyhow!("CString::new failed"))?;
        let mut buffer_list_c = Vec::with_capacity(buffer_list.len());
        let buffer_len_c = buffer_list.len();
        for entry in buffer_list {
            buffer_list_c.push(bindings::buffer_entry_t {
                addr: entry.addr,
                length: entry.length as usize,
            });
        }
        let ret = unsafe {
            bindings::registerLocalMemoryBatch(
                self.engine,
                buffer_list_c.as_mut_ptr(),
                buffer_len_c,
                location_c.as_ptr(),
            )
        };
        if ret < 0 {
            bail!("Failed to register local memory")
        } else {
            Ok(())
        }
    }

    /// Unregister several local memory regions.
    ///
    /// # Safety
    ///
    /// Every region must satisfy the safety contract of
    /// [`Self::unregister_local_memory`].
    pub unsafe fn unregister_local_memory_batch(&self, buffer_list: &[BufferEntry]) -> Result<()> {
        if buffer_list.is_empty() {
            return Ok(());
        }
        let mut addr_list: Vec<*mut c_void> = buffer_list.iter().map(|entry| entry.addr).collect();
        let addr_len = buffer_list.len();
        let ret = unsafe {
            bindings::unregisterLocalMemoryBatch(self.engine, addr_list.as_mut_ptr(), addr_len)
        };
        if ret < 0 {
            bail!("Failed to unregister local memory")
        } else {
            Ok(())
        }
    }

    pub fn allocate_batch_id(&self, batch_size: usize) -> Result<BatchId> {
        let ret = unsafe { bindings::allocateBatchID(self.engine, batch_size) };
        if ret == u64::MAX {
            bail!("Failed to allocate batch ID")
        } else {
            Ok(ret as BatchId)
        }
    }

    /// Submit a transfer batch.
    ///
    /// # Safety
    ///
    /// Every request source must remain valid and registered until the
    /// transfer completes. Target offsets must address registered memory in
    /// the corresponding remote segment.
    pub unsafe fn submit_transfer(
        &self,
        batch_id: BatchId,
        requests: &[TransferRequest],
    ) -> Result<()> {
        if requests.is_empty() {
            return Ok(());
        }
        let mut requests_c = Vec::with_capacity(requests.len());
        for request in requests {
            requests_c.push(bindings::transfer_request_t {
                opcode: request.opcode as i32,
                source: request.source,
                target_id: request.target_id,
                target_offset: request.target_offset,
                length: request.length,
            })
        }
        let ret = unsafe {
            bindings::submitTransfer(
                self.engine,
                batch_id,
                requests_c.as_mut_ptr(),
                requests.len(),
            )
        };
        if ret != 0 {
            bail!("Failed to submit transfer")
        } else {
            Ok(())
        }
    }

    pub fn get_transfer_status(&self, batch_id: BatchId, task_id: u64) -> Result<(i32, u64)> {
        let mut status = bindings::transfer_status_t {
            status: 0,
            transferred_bytes: 0,
        };
        let ret = unsafe {
            bindings::getTransferStatus(self.engine, batch_id, task_id as usize, &mut status)
        };
        if ret != 0 {
            bail!("Failed to get transfer status")
        } else {
            Ok((status.status, status.transferred_bytes))
        }
    }

    pub fn free_batch_id(&self, batch_id: BatchId) -> Result<()> {
        let ret = unsafe { bindings::freeBatchID(self.engine, batch_id) };
        if ret != 0 {
            bail!("Failed to free batch ID")
        } else {
            Ok(())
        }
    }

    pub fn open_segment(&self, name: String) -> Result<i32> {
        let name_c = CString::new(name).map_err(|_| anyhow!("CString::new failed"))?;
        let ret = unsafe { bindings::openSegment(self.engine, name_c.as_ptr()) };
        if ret < 0 {
            bail!("Failed to get segment ID")
        } else {
            Ok(ret)
        }
    }

    pub fn close_segment(&self, segment_id: i32) -> Result<()> {
        let ret = unsafe { bindings::closeSegment(self.engine, segment_id) };
        if ret < 0 {
            bail!("Failed to close segment with ID {}", segment_id)
        } else {
            Ok(())
        }
    }

    /// Eagerly establish EFA endpoints to `segment_name` so the first
    /// `submit_transfer` doesn't pay the serial fi_av_insert cost. No-op on
    /// non-EFA transports. Call after `open_segment` (and after the metadata
    /// has the peer's NIC list published).
    pub fn warmup_efa_segment(&self, name: &str) -> Result<()> {
        let name_c = CString::new(name).map_err(|_| anyhow!("CString::new failed"))?;
        let ret = unsafe { bindings::warmupEfaSegment(self.engine, name_c.as_ptr()) };
        if ret < 0 {
            bail!("warmupEfaSegment failed for {}: {}", name, ret)
        } else {
            Ok(())
        }
    }

    pub fn sync_segment_cache(&self) -> Result<()> {
        let ret = unsafe { bindings::syncSegmentCache(self.engine) };
        if ret < 0 {
            bail!("Failed to synchronize segment cache")
        } else {
            Ok(())
        }
    }
}

impl Drop for TransferEngine {
    fn drop(&mut self) {
        unsafe {
            bindings::destroyTransferEngine(self.engine);
        }
    }
}

// TransferEngine's C++ implementation internally synchronizes its shared
// state. The raw handle is owned by this value and destroyed only after the
// last Rust reference is dropped.
unsafe impl Send for TransferEngine {}
unsafe impl Sync for TransferEngine {}
