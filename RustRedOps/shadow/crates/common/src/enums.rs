/// Represents different types of callbacks available in the system.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub enum Callbacks {
    #[default]
    /// The default callback type for process creation events.
    PsSetCreateProcessNotifyRoutine,

    /// Callback for thread creation events.
    PsSetCreateThreadNotifyRoutine,

    /// Callback for image loading events.
    PsSetLoadImageNotifyRoutine,

    /// Callback for registry operations (using `CmRegisterCallbackEx`).
    CmRegisterCallbackEx,

    /// Callback related to process object operations (using `ObRegisterCallbacks`).
    ObProcess,

    /// Callback related to thread object operations (using `ObRegisterCallbacks`).
    ObThread,
}

/// Defines different operational modes or options for controlling behavior.
#[derive(Debug, Default)]
pub enum Options {
    /// Option to hide the process or thread.
    #[default]
    Hide,

    /// Option to apply protection to the process or thread.
    Protection,
}

/// Represents the type of protocol used in network communication (TCP/UDP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Transmission Control Protocol (TCP), which is connection-oriented and reliable.
    TCP,

    /// User Datagram Protocol (UDP), which is connectionless and less reliable.
    UDP,
}

/// Represents whether the port is local or remote in the context of network communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// Represents a local port on the current machine.
    LOCAL,

    /// Represents a remote port on a different machine.
    REMOTE,
}
