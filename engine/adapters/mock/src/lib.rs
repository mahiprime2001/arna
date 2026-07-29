//! wse-adapter-mock — the reference adapter. Entirely in memory, no OS. It
//! exists to (a) prove the engine and the boundary work with zero platform
//! code, and (b) serve as the executable definition of "conforming": every
//! real adapter must behave like this one where the contract is mandatory.
//!
//! A mock is *trivially* sealed — there is no host for it to leak to — so it
//! reports `sealed: true`. Real adapters must actively prove their seal.

use std::collections::{HashMap, HashSet};

use wse_common::*;
use wse_contract::{
    ApplicationsCapability, ClipboardCapability, ContractVersion, DevicesCapability,
    IsolationAttestation, StorageCapability, WindowsCapability, WorkspaceAdapter, WorkspaceDef,
    CONTRACT_VERSION,
};

#[derive(Default)]
struct MockWorkspace {
    exists: bool,
    running: bool,
    windows: Vec<Window>,
    next_window: u32,
    /// SPEC §9.1 — the workspace's own clipboard, isolated per workspace.
    clipboard: Option<ClipboardItem>,
    /// SPEC §8 — the workspace's own persistent resources, isolated per
    /// workspace. A deleted ResourceId is removed and never reused.
    resources: HashMap<ResourceId, (ResourceMetadata, Vec<u8>)>,
    /// SPEC §12 — devices made available to this workspace, and the handles
    /// currently held. Host machine camera/mic are never surfaced here (§7.3).
    devices: HashMap<DeviceId, DeviceDescriptor>,
    device_handles: HashSet<DeviceId>,
}

#[derive(Default)]
pub struct MockAdapter {
    workspaces: HashMap<WorkspaceId, MockWorkspace>,
}

impl MockAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    fn ws(&self, id: &WorkspaceId) -> Result<&MockWorkspace> {
        self.workspaces
            .get(id)
            .filter(|w| w.exists)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))
    }

    fn ws_mut(&mut self, id: &WorkspaceId) -> Result<&mut MockWorkspace> {
        self.workspaces
            .get_mut(id)
            .filter(|w| w.exists)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))
    }
}

impl WorkspaceAdapter for MockAdapter {
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    fn capabilities(&self) -> CapabilitySet {
        // The mock provides the two capabilities its behaviour actually
        // implements — launching apps and window metadata — and declares
        // nothing it doesn't (SPEC §18.2). No clipboard, storage, devices, etc.
        CapabilitySet::none()
            .with(Capability::Applications)
            .with(Capability::Windows)
            .with(Capability::Clipboard)
            .with(Capability::Storage)
            .with(Capability::Devices)
    }

    fn create(&mut self, def: &WorkspaceDef) -> Result<()> {
        self.workspaces.insert(
            def.id.clone(),
            MockWorkspace {
                exists: true,
                ..Default::default()
            },
        );
        Ok(())
    }

    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation> {
        let w = self.ws_mut(id)?;
        w.running = true;
        Ok(IsolationAttestation {
            sealed: true,
            details: vec!["mock: no host to leak to".into()],
        })
    }

    fn stop(&mut self, id: &WorkspaceId) -> Result<()> {
        let w = self.ws_mut(id)?;
        w.running = false;
        Ok(())
    }

    fn destroy(&mut self, id: &WorkspaceId) -> Result<()> {
        // SPEC §5.5 — irrecoverable: drop the record entirely.
        self.workspaces
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))
    }

    fn applications(&mut self) -> Option<&mut dyn ApplicationsCapability> {
        Some(self)
    }

    fn windows(&mut self) -> Option<&mut dyn WindowsCapability> {
        Some(self)
    }

    fn clipboard(&mut self) -> Option<&mut dyn ClipboardCapability> {
        Some(self)
    }

    fn storage(&mut self) -> Option<&mut dyn StorageCapability> {
        Some(self)
    }

    fn devices(&mut self) -> Option<&mut dyn DevicesCapability> {
        Some(self)
    }
}

impl DevicesCapability for MockAdapter {
    fn device_attach(
        &mut self,
        id: &WorkspaceId,
        class: DeviceClass,
        name: String,
    ) -> Result<DeviceDescriptor> {
        let ws = self.ws_mut(id)?;
        let desc = DeviceDescriptor {
            id: DeviceId::new(),
            class,
            name,
            metadata: HashMap::new(),
        };
        ws.devices.insert(desc.id.clone(), desc.clone());
        Ok(desc)
    }

    fn device_detach(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<bool> {
        let ws = self.ws_mut(id)?;
        ws.device_handles.remove(device);
        Ok(ws.devices.remove(device).is_some())
    }

    fn device_enumerate(&self, id: &WorkspaceId) -> Result<Vec<DeviceDescriptor>> {
        Ok(self.ws(id)?.devices.values().cloned().collect())
    }

    fn device_request(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<DeviceHandle> {
        let ws = self.ws_mut(id)?;
        if !ws.devices.contains_key(device) {
            // Non-available -> NotFound (undetectable, §12.1/§6.5).
            return Err(WseError::NotFound(format!("device {device}")));
        }
        ws.device_handles.insert(device.clone());
        Ok(DeviceHandle {
            device: device.clone(),
        })
    }

    fn device_release(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<bool> {
        Ok(self.ws_mut(id)?.device_handles.remove(device))
    }

    fn device_state(&self, id: &WorkspaceId) -> Result<CapabilityState> {
        // Availability-derived: no devices -> Unavailable, else Available.
        let ws = self.ws(id)?;
        Ok(if ws.devices.is_empty() {
            CapabilityState::Unavailable
        } else {
            CapabilityState::Available
        })
    }
}

impl ApplicationsCapability for MockAdapter {
    fn launch(&mut self, id: &WorkspaceId, app: &AppSpec) -> Result<Window> {
        let w = self.ws_mut(id)?;
        if !w.running {
            return Err(WseError::InvalidState {
                operation: "launch",
                state: WorkspaceState::Created,
            });
        }
        w.next_window += 1;
        let window = Window {
            id: WindowId::new(),
            app: app.id.clone(),
            title: app.name.clone(),
            bounds: Bounds {
                x: 40 * w.next_window as i32,
                y: 40 * w.next_window as i32,
                w: 900,
                h: 600,
            },
            focused: true,
        };
        // Only the newest window is focused.
        for existing in &mut w.windows {
            existing.focused = false;
        }
        w.windows.push(window.clone());
        Ok(window)
    }
}

impl WindowsCapability for MockAdapter {
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>> {
        Ok(self.ws(id)?.windows.clone())
    }
}

impl ClipboardCapability for MockAdapter {
    fn clipboard_peek(&self, id: &WorkspaceId) -> Result<Option<ClipboardItem>> {
        Ok(self.ws(id)?.clipboard.clone())
    }

    fn clipboard_put(&mut self, id: &WorkspaceId, data: ClipboardItem) -> Result<()> {
        self.ws_mut(id)?.clipboard = Some(data);
        Ok(())
    }
}

impl StorageCapability for MockAdapter {
    fn resource_create(
        &mut self,
        id: &WorkspaceId,
        name: String,
        kind: ResourceKind,
    ) -> Result<ResourceMetadata> {
        let ws = self.ws_mut(id)?;
        let meta = ResourceMetadata {
            id: ResourceId::new(),
            name,
            kind,
            size: 0,
        };
        ws.resources.insert(meta.id.clone(), (meta.clone(), Vec::new()));
        Ok(meta)
    }

    fn resource_write(
        &mut self,
        id: &WorkspaceId,
        resource: &ResourceId,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let ws = self.ws_mut(id)?;
        let entry = ws
            .resources
            .get_mut(resource)
            // deletion is terminal / unknown id -> NotFound (I3).
            .ok_or_else(|| WseError::NotFound(format!("resource {resource}")))?;
        entry.0.size = bytes.len() as u64;
        entry.1 = bytes;
        Ok(())
    }

    fn resource_read(&self, id: &WorkspaceId, resource: &ResourceId) -> Result<Vec<u8>> {
        let ws = self.ws(id)?;
        ws.resources
            .get(resource)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| WseError::NotFound(format!("resource {resource}")))
    }

    fn resource_delete(&mut self, id: &WorkspaceId, resource: &ResourceId) -> Result<bool> {
        let ws = self.ws_mut(id)?;
        Ok(ws.resources.remove(resource).is_some())
    }

    fn resource_list(&self, id: &WorkspaceId) -> Result<Vec<ResourceMetadata>> {
        Ok(self
            .ws(id)?
            .resources
            .values()
            .map(|(meta, _)| meta.clone())
            .collect())
    }
}
