use std::error::Error;

use tokio::sync::broadcast;
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::{constants::BUS_PREFIX, procfs};

/// Evdev commands define all the different ways to interact with [EventDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    Other,
}

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct DBusInterface {
    info: procfs::device::Device,
    tx: broadcast::Sender<Command>,
}

impl DBusInterface {
    fn new(tx: broadcast::Sender<Command>, info: procfs::device::Device) -> DBusInterface {
        DBusInterface { tx, info }
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Source.EventDevice")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.info.name.clone())
    }

    #[dbus_interface(property)]
    async fn handlers(&self) -> fdo::Result<Vec<String>> {
        Ok(self.info.handlers.clone())
    }

    #[dbus_interface(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.info.phys_path.clone())
    }

    #[dbus_interface(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.info.sysfs_path.clone())
    }

    #[dbus_interface(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.info.unique_id.clone())
    }
}

/// [EventDevice] represents an input device using the input subsystem.
pub struct EventDevice {
    dbus: Connection,
    handler: String,
    info: procfs::device::Device,
    rx: broadcast::Receiver<Command>,
    tx: broadcast::Sender<Command>,
}

impl EventDevice {
    pub fn new(conn: Connection, handler: String) -> Result<Self, Box<dyn Error>> {
        // Create a communication channel
        let (tx, rx) = broadcast::channel(32);

        // Parse information about this device from procfs
        let mut info: Option<procfs::device::Device> = None;
        let devices = procfs::device::get_all()?;
        for device in devices {
            for name in device.handlers.clone() {
                if name != handler {
                    continue;
                }
                info = Some(device.clone());
            }
        }
        let Some(info) = info else {
            return Err("Failed to find device information".into());
        };

        Ok(Self {
            dbus: conn,
            rx,
            tx,
            info,
            handler,
        })
    }

    /// Run the source device handler
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        self.listen_on_dbus().await?;
        Ok(())
    }

    /// Creates a DBus object
    async fn listen_on_dbus(&self) -> Result<(), Box<dyn Error>> {
        let path = get_dbus_path(self.handler.clone());
        let iface = DBusInterface::new(self.tx.clone(), self.info.clone());
        self.dbus.object_server().at(path, iface).await?;
        Ok(())
    }
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/devices/source/{}", BUS_PREFIX, handler.clone())
}