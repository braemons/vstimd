//! Input-device queries.

use super::convert::input_device_to_proto;
use super::response::ok_body;
use crate::proto;
use crate::scene::SceneState;

impl SceneState {
    pub(super) fn cmd_list_input_devices(&self) -> proto::Response {
        let devices = self.runtime.input.devices.iter().map(input_device_to_proto).collect();
        ok_body(proto::response::Body::InputDeviceList(proto::ListInputDevicesResponse { devices }))
    }
}
