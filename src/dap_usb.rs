//! CMSIS-DAP v2 transport. One full-speed USB packet carries one DAP message.

use embassy_usb::{
    Builder, Handler,
    driver::{Driver, Endpoint, EndpointIn, EndpointOut},
    msos,
    types::StringIndex,
};
use static_cell::StaticCell;

use crate::{PACKET, board::Usb};

struct InterfaceString(StringIndex);

impl Handler for InterfaceString {
    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&str> {
        (index == self.0).then_some("CMSIS-DAP v2 Interface")
    }
}

pub struct DapUsb {
    rx: <Usb as Driver<'static>>::EndpointOut,
    tx: <Usb as Driver<'static>>::EndpointIn,
}

impl DapUsb {
    pub fn new(builder: &mut Builder<'static, Usb>) -> Self {
        static STRING: StaticCell<InterfaceString> = StaticCell::new();
        let index = builder.string();
        let (rx, tx) = {
            let mut function = builder.function(0xff, 0, 0);
            function.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
            function.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
                "DeviceInterfaceGUIDs",
                msos::PropertyData::RegMultiSz(&["{CDB3B5AD-293B-4663-AA36-1AAE46463776}"]),
            ));
            let mut interface = function.interface();
            let mut alt = interface.alt_setting(0xff, 0, 0, Some(index));
            let rx = alt.endpoint_bulk_out(None, PACKET as u16);
            let tx = alt.endpoint_bulk_in(None, PACKET as u16);
            (rx, tx)
        };
        builder.handler(STRING.init(InterfaceString(index)));
        Self { rx, tx }
    }

    pub async fn run(mut self) {
        let mut request = [0; PACKET];
        loop {
            self.rx.wait_enabled().await;
            while let Ok(length) = self.rx.read(&mut request).await {
                if length == 0 {
                    continue;
                }
                if self
                    .tx
                    .write(discovery_reply(&request[..length]))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

// Advertise no target transport. The serial SWD ID command is independent.
fn discovery_reply(request: &[u8]) -> &'static [u8] {
    match request {
        [0x00, 0x04, ..] => b"\x00\x062.0.0\0",
        [0x00, 0xf0, ..] => &[0x00, 1, 0],
        [0x00, 0xfe, ..] => &[0x00, 1, 1],
        [0x00, 0xff, ..] => &[0x00, 2, PACKET as u8, 0],
        [0x00, ..] => &[0x00, 0],
        [0x02, ..] => &[0x02, 0], // No target connection.
        _ => &[0xff],
    }
}
