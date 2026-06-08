use canflow_types::CanFrame;

pub const CANFLOW_PLUGIN_ABI_VERSION: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginFrame {
    pub timestamp_ns: u64,
    pub id: u32,
    pub dlc: u8,
    pub data: [u8; 8],
    pub is_extended: u8,
    pub is_error: u8,
    pub is_remote: u8,
    pub _pad: u8,
}

impl From<&CanFrame> for PluginFrame {
    fn from(frame: &CanFrame) -> Self {
        Self {
            timestamp_ns: frame.timestamp_ns,
            id: frame.id.raw_id(),
            dlc: frame.dlc,
            data: frame.data,
            is_extended: frame.id.is_extended() as u8,
            is_error: frame.is_error as u8,
            is_remote: frame.is_remote as u8,
            _pad: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginSeverity {
    Info = 0,
    Warning = 1,
    Critical = 2,
}

#[repr(C)]
pub struct PluginAlert {
    pub severity: PluginSeverity,
    pub message: *const std::ffi::c_char,
    pub details_json: *const std::ffi::c_char,
}

#[repr(C)]
pub struct PluginVtable {
    pub abi_version: u32,
    pub name: *const std::ffi::c_char,
    pub version: *const std::ffi::c_char,

    pub create: unsafe extern "C" fn(config_json: *const std::ffi::c_char) -> *mut std::ffi::c_void,
    pub destroy: unsafe extern "C" fn(state: *mut std::ffi::c_void),
    pub ingest: unsafe extern "C" fn(
        state: *mut std::ffi::c_void,
        frame: *const PluginFrame,
        out_alerts: *mut PluginAlert,
        max_alerts: u32,
    ) -> u32,
    pub tick: unsafe extern "C" fn(
        state: *mut std::ffi::c_void,
        out_alerts: *mut PluginAlert,
        max_alerts: u32,
    ) -> u32,
    pub reset: unsafe extern "C" fn(state: *mut std::ffi::c_void),
}

pub type PluginInitFn = unsafe extern "C" fn() -> *const PluginVtable;
pub const PLUGIN_INIT_SYMBOL: &[u8] = b"canflow_plugin_init\0";
