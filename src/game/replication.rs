use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::lua::instance::{InstanceRef, WeakInstanceRef};

#[derive(Clone)]
pub struct DirtyInstance {
    pub id: u64,
    pub revision: u64,
    pub instance: WeakInstanceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyInstanceSnapshot {
    pub id: u64,
    pub revision: u64,
}

struct DirtyEntry {
    revision: u64,
    instance: WeakInstanceRef,
}

#[derive(Default)]
pub struct WorldReplicationState {
    current_revision: u64,
    dirty_instances: HashMap<u64, DirtyEntry>,
}

impl WorldReplicationState {
    pub fn mark(&mut self, instance: &InstanceRef) {
        let id = instance.lock().unwrap().id.0;
        self.current_revision = self.current_revision.saturating_add(1);
        self.dirty_instances.insert(
            id,
            DirtyEntry {
                revision: self.current_revision,
                instance: Arc::downgrade(instance),
            },
        );
    }

    pub fn mark_many<I>(&mut self, instances: I)
    where
        I: IntoIterator<Item = InstanceRef>,
    {
        for instance in instances {
            self.mark(&instance);
        }
    }

    pub fn latest_revision(&self) -> u64 {
        self.current_revision
    }

    pub fn snapshot_dirty_instances(&self) -> Vec<DirtyInstanceSnapshot> {
        let mut entries = self
            .dirty_instances
            .iter()
            .map(|(&id, entry)| DirtyInstanceSnapshot {
                id,
                revision: entry.revision,
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| (entry.revision, entry.id));
        entries
    }

    pub fn restore_dirty_instances<I>(&mut self, current_revision: u64, entries: I)
    where
        I: IntoIterator<Item = (DirtyInstanceSnapshot, WeakInstanceRef)>,
    {
        self.current_revision = current_revision;
        self.dirty_instances.clear();
        for (entry, instance) in entries {
            self.current_revision = self.current_revision.max(entry.revision);
            self.dirty_instances.insert(
                entry.id,
                DirtyEntry {
                    revision: entry.revision,
                    instance,
                },
            );
        }
    }

    pub fn changes_since(&self, revision: u64) -> Vec<DirtyInstance> {
        self.dirty_instances
            .iter()
            .filter_map(|(&id, entry)| {
                (entry.revision > revision).then(|| DirtyInstance {
                    id,
                    revision: entry.revision,
                    instance: entry.instance.clone(),
                })
            })
            .collect()
    }
}

pub type SharedWorldReplicationState = Arc<Mutex<WorldReplicationState>>;
