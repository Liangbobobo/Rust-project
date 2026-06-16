use wdk_sys::{
    ntddk::{KeStackAttachProcess, KeUnstackDetachProcess},
    KAPC_STATE, PRKPROCESS,
};

/// A wrapper for managing the attachment to a process context in the Windows kernel.
pub struct ProcessAttach {
    /// The APC (Asynchronous Procedure Call) state used to manage process attachment.
    apc_state: KAPC_STATE,

    /// Indicates whether the process is currently attached.
    attached: bool,
}

impl ProcessAttach {
    /// Create a new `ProcessAttach`.
    ///
    /// # Arguments
    ///
    /// * `target_process` - A pointer to the target process (`PRKPROCESS`) to attach to.
    #[inline]
    pub fn new(target_process: PRKPROCESS) -> Self {
        let mut apc_state = unsafe { core::mem::zeroed::<KAPC_STATE>() };

        unsafe {
            KeStackAttachProcess(target_process, &mut apc_state);
        }

        Self {
            apc_state,
            attached: true,
        }
    }

    /// Manually detaches from the process context.
    #[inline]
    pub fn detach(&mut self) {
        if self.attached {
            unsafe {
                KeUnstackDetachProcess(&mut self.apc_state);
            }

            self.attached = false;
        }
    }
}

impl Drop for ProcessAttach {
    fn drop(&mut self) {
        // If it is still attached, it unattaches when it leaves the scope.
        if self.attached {
            unsafe {
                KeUnstackDetachProcess(&mut self.apc_state);
            }
        }
    }
}
