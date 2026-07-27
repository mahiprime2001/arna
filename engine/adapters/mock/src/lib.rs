//! wse-adapter-mock — the reference adapter. Entirely in memory, no OS. It
//! exists to (a) prove the engine and the boundary work with zero platform
//! code, and (b) serve as the executable definition of "conforming": every
//! real adapter must behave like this one where the contract is mandatory.
//!
//! A mock is *trivially* sealed — there is no host for it to leak to — so it
//! reports `sealed: true`. Real adapters must actively prove their seal.

use std::collections::HashMap;

use wse_common::*;
use wse_contract::{
    CapabilitySet, ContractVersion, IsolationAttestation, WorkspaceAdapter, WorkspaceDef,
    CONTRACT_VERSION,
};

#[derive(Default)]
struct MockWorkspace {
    exists: bool,
    running: bool,
    windows: Vec<Window>,
    next_window: u32,
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
        // The mock provides isolation (trivially) but no real hardware features
        // and no native OS apps — declared honestly (SPEC §18.2).
        CapabilitySet::default()
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

    fn launch(&mut self, id: &WorkspaceId, app: &AppSpec) -> Result<Window> {
        let w = self.ws_mut(id)?;
        if !w.running {
            return Err(WseError::NotRunning(id.clone()));
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

    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>> {
        Ok(self.ws(id)?.windows.clone())
    }
}
