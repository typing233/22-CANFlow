pub mod trait_def;
pub mod socketcan;
pub mod vcan;
pub mod replay;
pub mod reconnect;
pub mod privilege;
pub mod parsers;

pub use trait_def::CanAdapter;
pub use socketcan::SocketCanAdapter;
pub use replay::ReplayAdapter;
pub use reconnect::ReconnectingAdapter;
pub use privilege::PrivilegeLevel;

use canflow_types::*;

pub fn build_adapter(config: &AdapterConfig, interface_id: InterfaceId) -> Result<Box<dyn CanAdapter>> {
    let adapter: Box<dyn CanAdapter> = match &config.kind {
        AdapterKind::SocketCan { interface } | AdapterKind::VirtualCan { interface } => {
            Box::new(SocketCanAdapter::new(interface, interface_id)?)
        }
        AdapterKind::LogReplay { path, format, loop_forever } => {
            Box::new(ReplayAdapter::new(
                path.clone(),
                format.clone(),
                *loop_forever,
                interface_id,
            ))
        }
    };

    let adapter = Box::new(ReconnectingAdapter::new(adapter, config.reconnect.clone()));
    Ok(adapter)
}
