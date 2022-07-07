use ic_interfaces::execution_environment::{
    ExecutionParameters, SubnetAvailableMemory, WasmExecutionOutput,
};
use ic_replicated_state::{
    canister_state::WASM_PAGE_SIZE_IN_BYTES, page_map::PageDeltaSerialization, ExecutionState,
    Global, Memory, NumWasmPages, PageIndex,
};
use ic_system_api::{
    sandbox_safe_system_state::{SandboxSafeSystemState, SystemStateChanges},
    ApiType,
};
use ic_types::{methods::FuncRef, NumBytes};
use serde::{Deserialize, Serialize};

use super::id::MemoryId;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Round(pub u64);

#[derive(Serialize, Deserialize, Clone)]
pub struct SandboxExecInput {
    pub func_ref: FuncRef,
    pub api_type: ApiType,
    pub globals: Vec<Global>,
    pub canister_current_memory_usage: NumBytes,
    pub execution_parameters: ExecutionParameters,
    pub subnet_available_memory: SubnetAvailableMemory,
    pub next_wasm_memory_id: MemoryId,
    pub next_stable_memory_id: MemoryId,
    // View of the system_state that is safe for the sandboxed process to
    // access.
    pub sandox_safe_system_state: SandboxSafeSystemState,
    pub wasm_reserved_pages: NumWasmPages,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SandboxExecOutput {
    pub wasm: WasmExecutionOutput,
    pub state: Option<StateModifications>,
    pub execute_total_duration: std::time::Duration,
    pub execute_run_duration: std::time::Duration,
}

/// Describes the memory changes performed by execution.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryModifications {
    pub page_delta: PageDeltaSerialization,
    pub size: NumWasmPages,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StateModifications {
    /// The state of the global variables after execution.
    pub globals: Vec<Global>,

    /// Modifications in the Wasm memory.
    pub wasm_memory: MemoryModifications,

    /// Modifications in the stable memory.
    pub stable_memory: MemoryModifications,

    pub system_state_changes: SystemStateChanges,
}

impl StateModifications {
    pub fn new(
        globals: Vec<Global>,
        wasm_memory: &Memory,
        stable_memory: &Memory,
        wasm_memory_delta: &[PageIndex],
        stable_memory_delta: &[PageIndex],
        system_state_changes: SystemStateChanges,
    ) -> Self {
        let wasm_memory = MemoryModifications {
            page_delta: wasm_memory.page_map.serialize_delta(wasm_memory_delta),
            size: wasm_memory.size,
        };

        let stable_memory = MemoryModifications {
            page_delta: stable_memory.page_map.serialize_delta(stable_memory_delta),
            size: stable_memory.size,
        };

        StateModifications {
            globals,
            wasm_memory,
            stable_memory,
            system_state_changes,
        }
    }

    /// Returns bytes allocated since the given old execution state.
    /// The result is a pair `(total_allocated_bytes, message_allocated_bytes)`.
    /// The first number consists of the bytes allocated in the Wasm memory, the
    /// stable memory, and the new messages. The second number is the allocated
    /// bytes of the new messages.
    pub fn allocated_bytes(&self, execution_state: &ExecutionState) -> (NumBytes, NumBytes) {
        let old_wasm_pages = execution_state.wasm_memory.size;
        let new_wasm_pages = self.wasm_memory.size;
        let added_wasm_pages = new_wasm_pages.max(old_wasm_pages) - old_wasm_pages;
        let added_wasm_bytes = added_wasm_pages
            .get()
            .saturating_mul(WASM_PAGE_SIZE_IN_BYTES) as u64;

        let old_stable_pages = execution_state.stable_memory.size;
        let new_stable_pages = self.stable_memory.size;
        let added_stable_pages = new_stable_pages.max(old_stable_pages) - old_stable_pages;
        let added_stable_bytes = added_stable_pages
            .get()
            .saturating_mul(WASM_PAGE_SIZE_IN_BYTES) as u64;

        let added_message_bytes = self.system_state_changes.allocated_request_bytes().get();

        let added_total_bytes = added_wasm_bytes
            .saturating_add(added_stable_bytes)
            .saturating_add(added_message_bytes);
        (
            NumBytes::from(added_total_bytes as u64),
            NumBytes::from(added_message_bytes as u64),
        )
    }
}
