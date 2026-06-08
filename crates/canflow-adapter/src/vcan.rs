use crate::socketcan::SocketCanAdapter;
use crate::trait_def::CanAdapter;
use canflow_types::*;

pub fn create_vcan_adapter(interface: &str, interface_id: InterfaceId) -> Result<impl CanAdapter> {
    SocketCanAdapter::new(interface, interface_id)
}
