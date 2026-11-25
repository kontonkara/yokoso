//! The clipboard, shared with every other program: text copied out, and
//! text pasted in, read from its owner without ever blocking the loop.

use std::io::{Read, Write};

use smithay_client_toolkit::{
    data_device_manager::{
        DataDeviceManagerState, WritePipe,
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{CopyPasteSource, DataSourceHandler},
    },
    reexports::{
        calloop::PostAction,
        client::{
            Connection, QueueHandle,
            protocol::{
                wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource,
                wl_seat::WlSeat, wl_surface::WlSurface,
            },
        },
    },
};

use crate::{
    app::{App, Shell, Ui},
    input::Input,
};

/// The kinds of text we offer, and look for, best first.
const TEXT: [&str; 4] = ["text/plain;charset=utf-8", "UTF8_STRING", "text/plain", "STRING"];

#[derive(Default)]
pub struct Clipboard {
    pub(crate) manager: Option<DataDeviceManagerState>,
    pub(crate) device: Option<DataDevice>,
    /// What we offer while it is ours.
    source: Option<CopyPasteSource>,
    text: String,
}

impl Clipboard {
    pub(crate) fn new(manager: Option<DataDeviceManagerState>) -> Self {
        Self {
            manager,
            ..Self::default()
        }
    }

    pub(crate) fn seat<S: Shell>(&mut self, seat: &WlSeat, qh: &QueueHandle<App<S>>) {
        if self.device.is_none()
            && let Some(manager) = &self.manager
        {
            self.device = Some(manager.get_data_device(qh, seat));
        }
    }
}

impl<S: Shell> Ui<S> {
    /// Puts `text` on the clipboard, as a press or a key just now allows.
    pub fn copy(&mut self, text: String) {
        let serial = self.serial;
        let clipboard = &mut self.clipboard;
        let (Some(manager), Some(device)) = (&clipboard.manager, &clipboard.device) else {
            return;
        };
        let source = manager.create_copy_paste_source(&self.qh, TEXT);
        source.set_selection(device, serial);
        clipboard.source = Some(source);
        clipboard.text = text;
    }

    /// Asks for the clipboard's text: it comes to `key` as `Input::Paste`.
    pub fn paste(&mut self, key: S::Key) {
        let Some(offer) = self
            .clipboard
            .device
            .as_ref()
            .and_then(|device| device.data().selection_offer())
        else {
            return;
        };
        let Some(mime) = offer.with_mime_types(|offered| {
            TEXT.iter()
                .find(|wanted| offered.iter().any(|kind| kind == *wanted))
                .map(|kind| kind.to_string())
        }) else {
            return;
        };
        let Ok(pipe) = offer.receive(mime) else { return };
        let mut read = Vec::new();
        let _ = self.handle.insert_source(pipe, move |_, file, app| {
            let mut chunk = [0u8; 4096];
            // SAFETY: the file is read, never closed or replaced, here.
            match unsafe { file.get_mut() }.read(&mut chunk) {
                Ok(0) => {
                    let text = String::from_utf8_lossy(&read).into_owned();
                    app.shell.input(&mut app.ui, key, &Input::Paste(text));
                    PostAction::Remove
                }
                Ok(n) => {
                    read.extend_from_slice(&chunk[..n]);
                    PostAction::Continue
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => PostAction::Continue,
                Err(_) => PostAction::Remove,
            }
        });
    }
}

impl<S: Shell> DataSourceHandler for App<S> {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}

    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: String, mut fd: WritePipe) {
        let _ = fd.write_all(self.ui.clipboard.text.as_bytes());
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        // Another program owns the clipboard now.
        self.ui.clipboard.source = None;
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl<S: Shell> DataDeviceHandler for App<S> {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64, _: &WlSurface) {}

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl<S: Shell> DataOfferHandler for App<S> {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}

    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}
